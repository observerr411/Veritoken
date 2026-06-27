#![no_std]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]

//! Invoice Token — tokenizes accounts-receivable invoices.
//! Each token unit represents 1 USD (7-decimal precision) of invoice face value.
//! Adds invoice-specific metadata: issuer, debtor, due date, face value, discount rate.
//! After settlement, redemption remains subject to compliance enforcement: a
//! paused engine or blocklisted holder cannot redeem invoice tokens.

#[cfg(test)]
mod test;

use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror, panic_with_error, symbol_short,
    Address, Env, String,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum InvoiceError {
    AlreadyInitialized = 1,
    AlreadySettled = 2,
    NotSettled = 3,
    InsufficientBalance = 4,
    NegativeAmount = 5,
    InsufficientAllowance = 6,
    AllowanceExpired = 7,
    KycNotApproved = 8,
    CompliancePaused = 9,
    Blocklisted = 10,
    TransferBlocked = 11,
}

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
    pub ipfs_doc_hash: String,      // off-chain document anchor
    pub transfer_fee_bps: u32,      // platform fee in basis points; 0 = no fee
    pub fee_recipient: Option<Address>, // receives transfer_fee_bps cut on each transfer
}

const DAY_IN_LEDGERS: u32 = 17280;
const BUMP: u32 = 90 * DAY_IN_LEDGERS;
const THRESHOLD: u32 = BUMP - DAY_IN_LEDGERS;

#[contract]
pub struct InvoiceToken;

#[contractimpl]
impl InvoiceToken {
    /// Constructor — called atomically at deploy time via `stellar contract deploy -- --admin ...`.
    /// This eliminates the deploy→initialize front-running window.
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
        env: Env,
        _admin: Address,
        _kyc_registry: Address,
        _compliance_engine: Address,
        _meta: InvoiceMeta,
    ) {
        panic_with_error!(env, InvoiceError::AlreadyInitialized);
    }

    // ── Admin ─────────────────────────────────────────────────────────────────

    pub fn update_kyc_registry(env: Env, new_registry: Address) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        env.storage()
            .instance()
            .set(&DataKey::KycRegistry, &new_registry);
        env.events()
            .publish((symbol_short!("upd_kyc"),), new_registry);
    }

    pub fn update_compliance_engine(env: Env, new_engine: Address) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        env.storage()
            .instance()
            .set(&DataKey::ComplianceEngine, &new_engine);
        env.events()
            .publish((symbol_short!("upd_ce"),), new_engine);
    }

    // ── Metadata ─────────────────────────────────────────────────────────────

    pub fn get_meta(env: Env) -> InvoiceMeta {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        env.storage().instance().get(&DataKey::InvoiceMeta).unwrap()
    }

    /// Replace the stored invoice metadata. Admin-only; panics if already settled.
    pub fn update_meta(env: Env, new_meta: InvoiceMeta) {
        Self::require_admin(&env);
        if env
            .storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::Settled)
            .unwrap_or(false)
        {
            panic!("invoice already settled");
        }
        env.storage().instance().set(&DataKey::InvoiceMeta, &new_meta);
        env.events().publish((symbol_short!("upd_meta"),), ());
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
            panic_with_error!(env, InvoiceError::AlreadySettled);
        }
        let bal = Self::read_balance(&env, to.clone());
        env.storage()
            .persistent()
            .set(&DataKey::Balance(to.clone()), &(bal + amount));
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Balance(to.clone()), THRESHOLD, BUMP);
        Self::register_holder(&env, &to);
        let supply: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::TotalSupply, &(supply + amount));

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
            panic_with_error!(env, InvoiceError::NotSettled);
        }
        Self::check_redeem_compliance(&env, &from, amount);
        let bal = Self::read_balance(&env, from.clone());
        if bal < amount {
            panic_with_error!(env, InvoiceError::InsufficientBalance);
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

    /// SEP-41 burn — destroys `amount` tokens from `from`.
    /// Requires KYC for the holder and compliance checks (pause / blocklist).
    pub fn burn(env: Env, from: Address, amount: i128) {
        from.require_auth();
        Self::require_kyc(&env, &from);
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
        env.events().publish((symbol_short!("burn"), from), amount);
    }

    /// SEP-41 burn_from — destroys `amount` tokens from `from` on behalf of `spender`.
    /// Requires KYC for the holder, compliance checks (pause / blocklist), and
    /// consumes the spender's allowance.
    pub fn burn_from(env: Env, spender: Address, from: Address, amount: i128) {
        spender.require_auth();
        Self::require_kyc(&env, &from);
        Self::check_redeem_compliance(&env, &from);

        // Spend allowance
        let allowance = Self::read_allowance(&env, from.clone(), spender.clone());
        if allowance.amount < amount {
            panic!("insufficient allowance");
        }
        if allowance.expiration_ledger < env.ledger().sequence() {
            panic!("allowance expired");
        }
        let new_allowance = AllowanceValue {
            amount: allowance.amount - amount,
            expiration_ledger: allowance.expiration_ledger,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Allowance(from.clone(), spender.clone()), &new_allowance);

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
        env.events().publish((symbol_short!("burn"), from), amount);
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
            panic_with_error!(env, InvoiceError::NegativeAmount);
        }
        Self::require_kyc(&env, &from);
        Self::require_kyc(&env, &to);
        Self::require_compliance(&env, &from, &to, amount);

        let from_bal = Self::read_balance(&env, from.clone());
        if from_bal < amount {
            panic_with_error!(env, InvoiceError::InsufficientBalance);
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
            panic_with_error!(env, InvoiceError::NegativeAmount);
        }
        Self::require_kyc(&env, &from);
        Self::require_kyc(&env, &to);
        Self::require_compliance(&env, &from, &to, amount);

        let allowance = Self::read_allowance(&env, from.clone(), spender.clone());
        if allowance.expiration_ledger < env.ledger().sequence() {
            panic_with_error!(env, InvoiceError::AllowanceExpired);
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
            panic_with_error!(env, InvoiceError::InsufficientBalance);
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
        from.require_auth();
        if amount < 0 {
            panic_with_error!(env, InvoiceError::NegativeAmount);
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
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("admin must be set");
        admin.require_auth();
    }

    fn require_kyc(env: &Env, addr: &Address) {
        let registry: Address = env
            .storage()
            .instance()
            .get(&DataKey::KycRegistry)
            .expect("kyc registry must be set");
        let client = KycRegistryClient::new(env, &registry);
        if !client.is_approved(addr) {
            panic_with_error!(env, InvoiceError::KycNotApproved);
        }
    }

    fn check_redeem_compliance(env: &Env, holder: &Address, amount: i128) {
        let engine: Address = env
            .storage()
            .instance()
            .get(&DataKey::ComplianceEngine)
            .expect("compliance engine must be set");
        let client = ComplianceEngineClient::new(env, &engine);
        if !client.can_transfer(holder, holder, &amount) {
            panic!("redemption blocked by compliance");
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

    fn require_compliance(env: &Env, from: &Address, to: &Address, amount: i128) {
        let engine: Address = env
            .storage()
            .instance()
            .get(&DataKey::ComplianceEngine)
            .unwrap();
        let client = ComplianceEngineClient::new(env, &engine);
        if !client.can_transfer(from, to, &amount) {
            panic_with_error!(env, InvoiceError::TransferBlocked);
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
        fn get_rules(env: soroban_sdk::Env) -> super::compliance_engine_types::ComplianceRules;
        fn is_blocklisted(env: soroban_sdk::Env, addr: Address) -> bool;
        fn can_transfer(env: soroban_sdk::Env, from: Address, to: Address, amount: i128) -> bool;
        fn register_holder(env: soroban_sdk::Env, addr: Address);
    }
}

mod compliance_engine_types {
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
