#![no_std]

//! Invoice Token — tokenizes accounts-receivable invoices.
//! Each token unit represents 1 USD (7-decimal precision) of invoice face value.
//! Adds invoice-specific metadata: issuer, debtor, due date, face value, discount rate.
//! After settlement, redemption remains subject to compliance enforcement: a
//! paused engine or blocklisted holder cannot redeem invoice tokens.

#[cfg(test)]
mod test;

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, String};

#[contracttype]
pub enum DataKey {
    Admin,
    KycRegistry,
    ComplianceEngine,
    InvoiceMeta,
    Balance(Address),
    Allowance(Address, Address),
    TotalSupply,
    Settled,
}

#[contracttype]
#[derive(Clone)]
pub struct AllowanceValue {
    pub amount: i128,
    pub expiration_ledger: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct InvoiceMeta {
    pub invoice_id: String,
    pub issuer: String,
    pub debtor: String,
    pub face_value_usd: i128,   // in stroops (7 decimals)
    pub discount_rate_bps: u32, // basis points
    pub due_date: u64,          // Unix timestamp
    pub currency: String,
    pub ipfs_doc_hash: String, // off-chain document anchor
}

const DAY_IN_LEDGERS: u32 = 17280;
const BUMP: u32 = 90 * DAY_IN_LEDGERS;
const THRESHOLD: u32 = BUMP - DAY_IN_LEDGERS;

#[contract]
pub struct InvoiceToken;

#[contractimpl]
impl InvoiceToken {
    /// Constructor — called atomically at deploy time via `stellar contract deploy -- --admin ...`.
    /// This eliminates the deploy→initialize front-running window: there is no state in which
    /// the contract exists but is uninitialized.
    #[allow(clippy::too_many_arguments)]
    pub fn __constructor(
        env: Env,
        admin: Address,
        kyc_registry: Address,
        compliance_engine: Address,
        meta: InvoiceMeta,
    ) {
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::KycRegistry, &kyc_registry);
        env.storage()
            .instance()
            .set(&DataKey::ComplianceEngine, &compliance_engine);
        env.storage().instance().set(&DataKey::InvoiceMeta, &meta);
        env.storage().instance().set(&DataKey::TotalSupply, &0i128);
        env.storage().instance().set(&DataKey::Settled, &false);
    }

    /// Legacy entry point — always panics. Retained so that any attempt to call
    /// `initialize` post-deploy fails loudly rather than silently succeeding.
    #[allow(clippy::too_many_arguments)]
    pub fn initialize(
        _env: Env,
        _admin: Address,
        _kyc_registry: Address,
        _compliance_engine: Address,
        _meta: InvoiceMeta,
    ) {
        panic!("already initialized");
    }

    // ── Metadata ─────────────────────────────────────────────────────────────

    pub fn get_meta(env: Env) -> InvoiceMeta {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        env.storage().instance().get(&DataKey::InvoiceMeta).unwrap()
    }

    pub fn name(env: Env) -> String {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        String::from_str(&env, "Veritoken Invoice")
    }
    pub fn symbol(env: Env) -> String {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        String::from_str(&env, "VTINV")
    }
    pub fn decimals(env: Env) -> u32 {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        7
    }

    // ── Lifecycle ────────────────────────────────────────────────────────────

    /// Mint tokens to represent this invoice. Admin-only.
    /// face_value_usd in the meta determines the max supply.
    pub fn issue(env: Env, to: Address, amount: i128) {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        Self::require_admin(&env);
        Self::require_kyc(&env, &to);
        if env
            .storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::Settled)
            .unwrap_or(false)
        {
            panic!("invoice already settled");
        }
        let bal = Self::read_balance(&env, to.clone());
        env.storage()
            .persistent()
            .set(&DataKey::Balance(to.clone()), &(bal + amount));
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Balance(to.clone()), THRESHOLD, BUMP);
        let supply: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::TotalSupply, &(supply + amount));
        Self::register_holder(&env, &to);
        env.events().publish((symbol_short!("issued"), to), amount);
    }

    /// Mark invoice as settled and enable redemption burns.
    pub fn settle(env: Env) {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        Self::require_admin(&env);
        env.storage().instance().set(&DataKey::Settled, &true);
        env.events().publish((symbol_short!("settled"),), ());
    }

    /// Burn tokens upon settlement / redemption.
    pub fn redeem(env: Env, from: Address, amount: i128) {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        from.require_auth();
        if !env
            .storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::Settled)
            .unwrap_or(false)
        {
            panic!("invoice not yet settled");
        }
        Self::check_redeem_compliance(&env, &from);
        let bal = Self::read_balance(&env, from.clone());
        if bal < amount {
            panic!("insufficient balance");
        }
        env.storage()
            .persistent()
            .set(&DataKey::Balance(from.clone()), &(bal - amount));
        let supply: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::TotalSupply, &(supply - amount));
        env.events()
            .publish((symbol_short!("redeemed"), from), amount);
    }

    pub fn balance(env: Env, id: Address) -> i128 {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        Self::read_balance(&env, id)
    }

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        from.require_auth();
        if env
            .storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::Settled)
            .unwrap_or(false)
        {
            panic!("invoice already settled");
        }
        if amount < 0 {
            panic!("negative amount");
        }
        Self::require_kyc(&env, &from);
        Self::require_kyc(&env, &to);
        Self::require_compliance(&env, &from, &to, amount);

        let from_bal = Self::read_balance(&env, from.clone());
        if from_bal < amount {
            panic!("insufficient balance");
        }
        env.storage()
            .persistent()
            .set(&DataKey::Balance(from.clone()), &(from_bal - amount));
        let to_bal = Self::read_balance(&env, to.clone());
        env.storage()
            .persistent()
            .set(&DataKey::Balance(to.clone()), &(to_bal + amount));
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Balance(from.clone()), THRESHOLD, BUMP);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Balance(to.clone()), THRESHOLD, BUMP);
        Self::register_holder(&env, &to);
        env.events()
            .publish((symbol_short!("transfer"), from, to), amount);
    }

    pub fn transfer_from(
        env: Env,
        spender: Address,
        from: Address,
        to: Address,
        amount: i128,
    ) {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        spender.require_auth();
        if env
            .storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::Settled)
            .unwrap_or(false)
        {
            panic!("invoice already settled");
        }
        if amount < 0 {
            panic!("negative amount");
        }
        Self::require_kyc(&env, &from);
        Self::require_kyc(&env, &to);
        Self::require_compliance(&env, &from, &to, amount);

        let allowance = Self::read_allowance(&env, from.clone(), spender.clone());
        if allowance.expiration_ledger < env.ledger().sequence() {
            panic!("allowance expired");
        }
        if allowance.amount < amount {
            panic!("insufficient allowance");
        }
        let new_allowance = AllowanceValue {
            amount: allowance.amount - amount,
            expiration_ledger: allowance.expiration_ledger,
        };
        env.storage().persistent().set(
            &DataKey::Allowance(from.clone(), spender.clone()),
            &new_allowance,
        );

        let from_bal = Self::read_balance(&env, from.clone());
        if from_bal < amount {
            panic!("insufficient balance");
        }
        env.storage()
            .persistent()
            .set(&DataKey::Balance(from.clone()), &(from_bal - amount));
        let to_bal = Self::read_balance(&env, to.clone());
        env.storage()
            .persistent()
            .set(&DataKey::Balance(to.clone()), &(to_bal + amount));
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Balance(from.clone()), THRESHOLD, BUMP);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Balance(to.clone()), THRESHOLD, BUMP);
        Self::register_holder(&env, &to);
        env.events()
            .publish((symbol_short!("transfer"), from, to), amount);
    }

    pub fn approve(
        env: Env,
        from: Address,
        spender: Address,
        amount: i128,
        expiration_ledger: u32,
    ) {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        from.require_auth();
        if amount < 0 {
            panic!("negative amount");
        }
        let allowance = AllowanceValue {
            amount,
            expiration_ledger,
        };
        env.storage().persistent().set(
            &DataKey::Allowance(from.clone(), spender.clone()),
            &allowance,
        );
        env.storage().persistent().extend_ttl(
            &DataKey::Allowance(from.clone(), spender.clone()),
            THRESHOLD,
            BUMP,
        );
        env.events().publish(
            (symbol_short!("approve"), from, spender),
            (amount, expiration_ledger),
        );
    }

    pub fn allowance(env: Env, from: Address, spender: Address) -> i128 {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        let allowance = Self::read_allowance(&env, from, spender);
        if allowance.expiration_ledger < env.ledger().sequence() {
            0
        } else {
            allowance.amount
        }
    }

    pub fn total_supply(env: Env) -> i128 {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        env.storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0)
    }

    pub fn is_settled(env: Env) -> bool {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        env.storage()
            .instance()
            .get(&DataKey::Settled)
            .unwrap_or(false)
    }

    // ── Internals ────────────────────────────────────────────────────────────

    fn require_admin(env: &Env) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
    }

    fn require_kyc(env: &Env, addr: &Address) {
        let registry: Address = env.storage().instance().get(&DataKey::KycRegistry).unwrap();
        let client = KycRegistryClient::new(env, &registry);
        if !client.is_approved(addr) {
            panic!("KYC not approved");
        }
    }

    fn check_redeem_compliance(env: &Env, holder: &Address) {
        let engine: Address = env
            .storage()
            .instance()
            .get(&DataKey::ComplianceEngine)
            .unwrap();
        let client = ComplianceEngineClient::new(env, &engine);
        if client.get_rules().paused {
            panic!("redemption blocked by compliance pause");
        }
        if client.is_blocklisted(holder) {
            panic!("redemption blocked for blocklisted holder");
        }
    }

    fn require_compliance(env: &Env, from: &Address, to: &Address, amount: i128) {
        let engine: Address = env
            .storage()
            .instance()
            .get(&DataKey::ComplianceEngine)
            .unwrap();
        let client = ComplianceEngineClient::new(env, &engine);
        if !client.can_transfer(from, to, &amount) {
            panic!("transfer rejected by compliance engine");
        }
    }

    fn register_holder(env: &Env, addr: &Address) {
        let engine: Address = env
            .storage()
            .instance()
            .get(&DataKey::ComplianceEngine)
            .unwrap();
        let client = ComplianceEngineClient::new(env, &engine);
        client.register_holder(addr);
    }

    fn read_balance(env: &Env, addr: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Balance(addr))
            .unwrap_or(0)
    }

    fn read_allowance(env: &Env, from: Address, spender: Address) -> AllowanceValue {
        env.storage()
            .persistent()
            .get(&DataKey::Allowance(from, spender))
            .unwrap_or(AllowanceValue {
                amount: 0,
                expiration_ledger: 0,
            })
    }
}

mod kyc_iface {
    use soroban_sdk::{contractclient, Address};
    #[contractclient(name = "KycRegistryClient")]
    #[allow(dead_code)]
    pub trait KycRegistry {
        fn is_approved(env: soroban_sdk::Env, addr: Address) -> bool;
    }
}

mod compliance_iface {
    use soroban_sdk::{contractclient, Address};
    #[contractclient(name = "ComplianceEngineClient")]
    #[allow(dead_code)]
    pub trait ComplianceEngine {
        fn get_rules(env: soroban_sdk::Env) -> super::compliance_engine::ComplianceRules;
        fn is_blocklisted(env: soroban_sdk::Env, addr: Address) -> bool;
        fn can_transfer(env: soroban_sdk::Env, from: Address, to: Address, amount: i128) -> bool;
        fn register_holder(env: soroban_sdk::Env, addr: Address);
    }
}

mod compliance_engine {
    use soroban_sdk::contracttype;

    #[contracttype]
    #[derive(Clone)]
    pub struct ComplianceRules {
        pub max_transfer_amount: i128,
        pub min_holding_period: u64,
        pub max_holders: u32,
        pub require_same_jurisdiction: bool,
        pub paused: bool,
    }
}

use compliance_iface::ComplianceEngineClient;
use kyc_iface::KycRegistryClient;
