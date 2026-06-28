#![no_std]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]

//! Invoice Token — tokenizes accounts-receivable invoices.
//! Each token unit represents 1 USD (7-decimal precision) of invoice face value.
//! Supports multiple invoices within a single deployed contract, each indexed
//! by its invoice_id. Supply, settlement status, and balances are tracked
//! independently per invoice.

#[cfg(test)]
mod test;

use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror, panic_with_error, symbol_short,
    Address, Env, String, Vec,
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
    PastDueDate = 12,
    InvoiceNotFound = 13,
    InvoiceAlreadyExists = 14,
}

#[contracttype]
pub enum DataKey {
    Admin,
    PendingAdmin,
    KycRegistry,
    ComplianceEngine,
    InvoiceMeta,
    Balance(Address),
    Allowance(Address, Address),
    TotalSupply,
    Settled,
    SettlementAmount,
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
    pub ipfs_doc_hash: String,
    pub transfer_fee_bps: u32,
    pub fee_recipient: Option<Address>,
}

const DAY_IN_LEDGERS: u32 = 17280;
const BUMP: u32 = 90 * DAY_IN_LEDGERS;
const THRESHOLD: u32 = BUMP - DAY_IN_LEDGERS;

#[contract]
pub struct InvoiceToken;

#[contractimpl]
impl InvoiceToken {
    /// Constructor — sets admin/kyc/compliance and creates the first invoice
    /// atomically at deploy time.
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
        Self::do_create_invoice(&env, meta);
    }

    /// Legacy entry point — always panics.
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

    pub fn propose_admin(env: Env, new_admin: Address) {
        Self::require_admin(&env);
        env.storage()
            .instance()
            .set(&DataKey::PendingAdmin, &new_admin);
        env.events().publish((symbol_short!("proposed"),), new_admin);
    }

    pub fn accept_admin(env: Env) {
        let pending: Address = env
            .storage()
            .instance()
            .get(&DataKey::PendingAdmin)
            .expect("no pending admin");
        pending.require_auth();
        let old_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        env.storage().instance().set(&DataKey::Admin, &pending);
        env.storage().instance().remove(&DataKey::PendingAdmin);
        env.events()
            .publish((symbol_short!("admin_set"),), (old_admin, pending));
    }

    // ── Invoice management ────────────────────────────────────────────────────

    /// Create a new invoice within this contract. Admin-only.
    pub fn create_invoice(env: Env, meta: InvoiceMeta) {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        Self::require_admin(&env);
        Self::do_create_invoice(&env, meta);
    }

    /// List invoice IDs with pagination. Returns up to `limit` (capped at 50)
    /// IDs starting from `start` (zero-based offset).
    pub fn list_invoices(env: Env, start: u32, limit: u32) -> Vec<String> {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        let list: Vec<String> = env
            .storage()
            .instance()
            .get(&DataKey::InvoicesList)
            .unwrap_or_else(|| Vec::new(&env));
        let total = list.len();
        let cap: u32 = 50;
        let effective_limit = if limit > cap { cap } else { limit };
        let mut result: Vec<String> = Vec::new(&env);
        if start >= total {
            return result;
        }
        let end = (start + effective_limit).min(total);
        for i in start..end {
            result.push_back(list.get(i).unwrap());
        }
        result
    }

    // ── Metadata ─────────────────────────────────────────────────────────────

    pub fn get_meta(env: Env, invoice_id: String) -> InvoiceMeta {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        env.storage()
            .persistent()
            .get(&DataKey::InvoiceMeta(invoice_id))
            .expect("invoice not found")
    }

    /// Replace stored invoice metadata. Admin-only; panics if already settled.
    pub fn update_meta(env: Env, invoice_id: String, new_meta: InvoiceMeta) {
        Self::require_admin(&env);
        if env
            .storage()
            .persistent()
            .get::<DataKey, bool>(&DataKey::Settled(invoice_id.clone()))
            .unwrap_or(false)
        {
            panic!("invoice already settled");
        }
        env.storage()
            .persistent()
            .set(&DataKey::InvoiceMeta(invoice_id), &new_meta);
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

    /// Mint tokens for a specific invoice. Admin-only.
    pub fn issue(env: Env, invoice_id: String, to: Address, amount: i128) {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        Self::require_admin(&env);
        Self::require_kyc(&env, &to);
        if env
            .storage()
            .persistent()
            .get::<DataKey, bool>(&DataKey::Settled(invoice_id.clone()))
            .unwrap_or(false)
        {
            panic_with_error!(env, InvoiceError::AlreadySettled);
        }
        let bal = Self::read_balance(&env, to.clone(), invoice_id.clone());
        env.storage().persistent().set(
            &DataKey::Balance(to.clone(), invoice_id.clone()),
            &(bal + amount),
        );
        env.storage().persistent().extend_ttl(
            &DataKey::Balance(to.clone(), invoice_id.clone()),
            THRESHOLD,
            BUMP,
        );
        Self::register_holder(&env, &to);
        let supply: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::TotalSupply(invoice_id.clone()))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::TotalSupply(invoice_id.clone()), &(supply + amount));
        env.storage().persistent().extend_ttl(
            &DataKey::TotalSupply(invoice_id.clone()),
            THRESHOLD,
            BUMP,
        );
        env.events()
            .publish((symbol_short!("issued"), to), (invoice_id, amount));
    }

    /// Mark invoice as fully settled; equivalent to partial_settle(face_value_usd).
    pub fn settle(env: Env) {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        Self::require_admin(&env);
        let meta: InvoiceMeta = env.storage().instance().get(&DataKey::InvoiceMeta).unwrap();
        env.storage().instance().set(&DataKey::Settled, &true);
        env.storage()
            .instance()
            .set(&DataKey::SettlementAmount, &meta.face_value_usd);
        env.events().publish((symbol_short!("settled"),), ());
    }

    /// Mark invoice as partially settled with the given payment amount.
    /// Enables proportional redemption: each holder may redeem up to
    /// `balance * settlement_amount / total_supply` tokens.
    pub fn partial_settle(env: Env, settlement_amount: i128) {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        Self::require_admin(&env);
        if settlement_amount <= 0 {
            panic!("settlement_amount must be positive");
        }
        let meta: InvoiceMeta = env.storage().instance().get(&DataKey::InvoiceMeta).unwrap();
        if settlement_amount > meta.face_value_usd {
            panic!("settlement_amount exceeds face value");
        }
        env.storage().instance().set(&DataKey::Settled, &true);
        env.storage()
            .instance()
            .set(&DataKey::SettlementAmount, &settlement_amount);
        env.events()
            .publish((symbol_short!("p_settld"),), settlement_amount);
    }

    pub fn settlement_amount(env: Env) -> i128 {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        env.storage()
            .instance()
            .get(&DataKey::SettlementAmount)
            .unwrap_or(0)
    }

    /// Burn tokens upon settlement / redemption.
    /// Redemption is limited to the holder's proportional share of the settled amount.
    pub fn redeem(env: Env, from: Address, amount: i128) {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        from.require_auth();
        if !env
            .storage()
            .persistent()
            .get::<DataKey, bool>(&DataKey::Settled(invoice_id.clone()))
            .unwrap_or(false)
        {
            panic_with_error!(env, InvoiceError::NotSettled);
        }
        Self::check_redeem_compliance(&env, &from, amount);
        let bal = Self::read_balance(&env, from.clone(), invoice_id.clone());
        if bal < amount {
            panic_with_error!(env, InvoiceError::InsufficientBalance);
        }
        let settlement: i128 = env
            .storage()
            .instance()
            .get(&DataKey::SettlementAmount)
            .unwrap_or(0);
        if settlement > 0 {
            let total_supply: i128 = env
                .storage()
                .instance()
                .get(&DataKey::TotalSupply)
                .unwrap_or(0);
            if total_supply > 0 {
                let max_redeemable = bal * settlement / total_supply;
                if amount > max_redeemable {
                    panic!("exceeds proportional settlement");
                }
            }
        }
        env.storage()
            .persistent()
            .set(&DataKey::Balance(from.clone()), &(bal - amount));
        let supply: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::TotalSupply(invoice_id.clone()))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::TotalSupply(invoice_id.clone()), &(supply - amount));
        env.storage().persistent().extend_ttl(
            &DataKey::TotalSupply(invoice_id.clone()),
            THRESHOLD,
            BUMP,
        );
        env.events()
            .publish((symbol_short!("redeemed"), from), (invoice_id, amount));
    }

    /// SEP-41-style burn — destroys tokens from `from` for a specific invoice.
    pub fn burn(env: Env, invoice_id: String, from: Address, amount: i128) {
        from.require_auth();
        Self::require_kyc(&env, &from);
        Self::check_redeem_compliance(&env, &from, amount);
        let bal = Self::read_balance(&env, from.clone(), invoice_id.clone());
        if bal < amount {
            panic!("insufficient balance");
        }
        env.storage().persistent().set(
            &DataKey::Balance(from.clone(), invoice_id.clone()),
            &(bal - amount),
        );
        let supply: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::TotalSupply(invoice_id.clone()))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::TotalSupply(invoice_id.clone()), &(supply - amount));
        env.storage().persistent().extend_ttl(
            &DataKey::TotalSupply(invoice_id.clone()),
            THRESHOLD,
            BUMP,
        );
        env.events()
            .publish((symbol_short!("burn"), from), (invoice_id, amount));
    }

    /// SEP-41-style burn_from — destroys tokens from `from` on behalf of `spender`.
    pub fn burn_from(env: Env, spender: Address, invoice_id: String, from: Address, amount: i128) {
        spender.require_auth();
        Self::require_kyc(&env, &from);
        Self::check_redeem_compliance(&env, &from, amount);

        let allowance = Self::read_allowance(&env, from.clone(), spender.clone(), invoice_id.clone());
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
        env.storage().persistent().set(
            &DataKey::Allowance(from.clone(), spender.clone(), invoice_id.clone()),
            &new_allowance,
        );

        let bal = Self::read_balance(&env, from.clone(), invoice_id.clone());
        if bal < amount {
            panic!("insufficient balance");
        }
        env.storage().persistent().set(
            &DataKey::Balance(from.clone(), invoice_id.clone()),
            &(bal - amount),
        );
        let supply: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::TotalSupply(invoice_id.clone()))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::TotalSupply(invoice_id.clone()), &(supply - amount));
        env.storage().persistent().extend_ttl(
            &DataKey::TotalSupply(invoice_id.clone()),
            THRESHOLD,
            BUMP,
        );
        env.events()
            .publish((symbol_short!("burn"), from), (invoice_id, amount));
    }

    pub fn balance(env: Env, id: Address, invoice_id: String) -> i128 {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        Self::read_balance(&env, id, invoice_id)
    }

    pub fn transfer(env: Env, invoice_id: String, from: Address, to: Address, amount: i128) {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        from.require_auth();
        if env
            .storage()
            .persistent()
            .get::<DataKey, bool>(&DataKey::Settled(invoice_id.clone()))
            .unwrap_or(false)
        {
            panic!("invoice already settled");
        }
        let meta: InvoiceMeta = env
            .storage()
            .persistent()
            .get(&DataKey::InvoiceMeta(invoice_id.clone()))
            .expect("invoice not found");
        if env.ledger().timestamp() > meta.due_date {
            panic_with_error!(env, InvoiceError::PastDueDate);
        }
        if amount < 0 {
            panic_with_error!(env, InvoiceError::NegativeAmount);
        }
        Self::require_kyc(&env, &from);
        Self::require_kyc(&env, &to);
        Self::require_compliance(&env, &from, &to, amount);

        let from_bal = Self::read_balance(&env, from.clone(), invoice_id.clone());
        if from_bal < amount {
            panic_with_error!(env, InvoiceError::InsufficientBalance);
        }
        env.storage().persistent().set(
            &DataKey::Balance(from.clone(), invoice_id.clone()),
            &(from_bal - amount),
        );
        env.storage().persistent().extend_ttl(
            &DataKey::Balance(from.clone(), invoice_id.clone()),
            THRESHOLD,
            BUMP,
        );

        let to_bal = Self::read_balance(&env, to.clone(), invoice_id.clone());
        env.storage().persistent().set(
            &DataKey::Balance(to.clone(), invoice_id.clone()),
            &(to_bal + amount),
        );
        env.storage().persistent().extend_ttl(
            &DataKey::Balance(to.clone(), invoice_id.clone()),
            THRESHOLD,
            BUMP,
        );

        Self::register_holder(&env, &to);
        env.events()
            .publish((symbol_short!("transfer"), from, to), (invoice_id, amount));
    }

    pub fn transfer_from(
        env: Env,
        spender: Address,
        invoice_id: String,
        from: Address,
        to: Address,
        amount: i128,
    ) {
        spender.require_auth();
        if env
            .storage()
            .persistent()
            .get::<DataKey, bool>(&DataKey::Settled(invoice_id.clone()))
            .unwrap_or(false)
        {
            panic!("invoice already settled");
        }
        let meta: InvoiceMeta = env
            .storage()
            .persistent()
            .get(&DataKey::InvoiceMeta(invoice_id.clone()))
            .expect("invoice not found");
        if env.ledger().timestamp() > meta.due_date {
            panic_with_error!(env, InvoiceError::PastDueDate);
        }
        if amount < 0 {
            panic_with_error!(env, InvoiceError::NegativeAmount);
        }
        Self::require_kyc(&env, &from);
        Self::require_kyc(&env, &to);
        Self::require_compliance(&env, &from, &to, amount);

        let allowance =
            Self::read_allowance(&env, from.clone(), spender.clone(), invoice_id.clone());
        if allowance.expiration_ledger < env.ledger().sequence() {
            panic_with_error!(env, InvoiceError::AllowanceExpired);
        }
        if allowance.amount < amount {
            panic_with_error!(env, InvoiceError::InsufficientAllowance);
        }
        let new_allowance = AllowanceValue {
            amount: allowance.amount - amount,
            expiration_ledger: allowance.expiration_ledger,
        };
        env.storage().persistent().set(
            &DataKey::Allowance(from.clone(), spender.clone(), invoice_id.clone()),
            &new_allowance,
        );

        let from_bal = Self::read_balance(&env, from.clone(), invoice_id.clone());
        if from_bal < amount {
            panic_with_error!(env, InvoiceError::InsufficientBalance);
        }
        env.storage().persistent().set(
            &DataKey::Balance(from.clone(), invoice_id.clone()),
            &(from_bal - amount),
        );
        env.storage().persistent().extend_ttl(
            &DataKey::Balance(from.clone(), invoice_id.clone()),
            THRESHOLD,
            BUMP,
        );

        let to_bal = Self::read_balance(&env, to.clone(), invoice_id.clone());
        env.storage().persistent().set(
            &DataKey::Balance(to.clone(), invoice_id.clone()),
            &(to_bal + amount),
        );
        env.storage().persistent().extend_ttl(
            &DataKey::Balance(to.clone(), invoice_id.clone()),
            THRESHOLD,
            BUMP,
        );

        Self::register_holder(&env, &to);
        env.events()
            .publish((symbol_short!("transfer"), from, to), (invoice_id, amount));
    }

    pub fn approve(
        env: Env,
        from: Address,
        spender: Address,
        invoice_id: String,
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
            &DataKey::Allowance(from.clone(), spender.clone(), invoice_id.clone()),
            &allowance,
        );
        env.storage().persistent().extend_ttl(
            &DataKey::Allowance(from.clone(), spender.clone(), invoice_id.clone()),
            THRESHOLD,
            BUMP,
        );
        env.events().publish(
            (symbol_short!("approve"), from, spender),
            (invoice_id, amount, expiration_ledger),
        );
    }

    pub fn allowance(env: Env, from: Address, spender: Address, invoice_id: String) -> i128 {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        let allowance = Self::read_allowance(&env, from, spender, invoice_id);
        if allowance.expiration_ledger < env.ledger().sequence() {
            0
        } else {
            allowance.amount
        }
    }

    pub fn total_supply(env: Env, invoice_id: String) -> i128 {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        env.storage()
            .persistent()
            .get(&DataKey::TotalSupply(invoice_id))
            .unwrap_or(0)
    }

    pub fn is_settled(env: Env, invoice_id: String) -> bool {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        env.storage()
            .persistent()
            .get(&DataKey::Settled(invoice_id))
            .unwrap_or(false)
    }

    pub fn version(env: Env) -> String {
        String::from_str(&env, env!("CARGO_PKG_VERSION"))
    }

    // ── Internals ────────────────────────────────────────────────────────────

    fn do_create_invoice(env: &Env, meta: InvoiceMeta) {
        let invoice_id = meta.invoice_id.clone();
        if env
            .storage()
            .persistent()
            .has(&DataKey::InvoiceMeta(invoice_id.clone()))
        {
            panic_with_error!(env, InvoiceError::InvoiceAlreadyExists);
        }
        env.storage()
            .persistent()
            .set(&DataKey::InvoiceMeta(invoice_id.clone()), &meta);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::InvoiceMeta(invoice_id.clone()), THRESHOLD, BUMP);
        env.storage()
            .persistent()
            .set(&DataKey::TotalSupply(invoice_id.clone()), &0i128);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::TotalSupply(invoice_id.clone()), THRESHOLD, BUMP);
        env.storage()
            .persistent()
            .set(&DataKey::Settled(invoice_id.clone()), &false);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Settled(invoice_id.clone()), THRESHOLD, BUMP);
        let mut list: Vec<String> = env
            .storage()
            .instance()
            .get(&DataKey::InvoicesList)
            .unwrap_or_else(|| Vec::new(env));
        list.push_back(invoice_id.clone());
        env.storage().instance().set(&DataKey::InvoicesList, &list);
        env.events()
            .publish((symbol_short!("inv_crt"),), invoice_id);
    }

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

    fn read_balance(env: &Env, addr: Address, invoice_id: String) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Balance(addr, invoice_id))
            .unwrap_or(0)
    }

    fn read_allowance(
        env: &Env,
        from: Address,
        spender: Address,
        invoice_id: String,
    ) -> AllowanceValue {
        env.storage()
            .persistent()
            .get(&DataKey::Allowance(from, spender, invoice_id))
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
