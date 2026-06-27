#![no_std]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]

//! Property Token — fractional ownership of real estate.
//! Each token = 1 share out of total_shares. Dividends distributed in XLM/USDC.
//! Minting is admin-gated and still enforces active KYC, the configured minimum
//! KYC tier, and mint-time compliance checks for pause/blocklist rules.

#[cfg(test)]
mod test;

use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror, panic_with_error, symbol_short,
    Address, Env, String,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum PropertyError {
    AlreadyInitialized = 1,
    NegativeShares = 2,
    InsufficientShares = 3,
    NoShares = 4,
    KycNotApproved = 5,
    KycTierTooLow = 6,
    CompliancePaused = 7,
    Blocklisted = 8,
    TransferBlocked = 9,
}

#[contracttype]
pub enum DataKey {
    Admin,
    KycRegistry,
    ComplianceEngine,
    PropertyMeta,
    Balance(Address),
    TotalShares,
    DividendPool,
    ClaimedDividend(Address),
    Unclaimed(Address),
    DividendPerShare,
    /// SEP-41 delegated-transfer allowance: (owner, spender) → AllowanceValue.
    Allowance(AllowanceKey),
}

#[contracttype]
#[derive(Clone)]
pub struct AllowanceKey {
    pub from: Address,
    pub spender: Address,
}

#[contracttype]
#[derive(Clone)]
pub struct AllowanceValue {
    pub amount: i128,
    pub expiration_ledger: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct PropertyMeta {
    pub property_id: String,
    pub legal_name: String,
    pub jurisdiction: String,
    pub address: String,
    pub total_valuation_usd: i128,
    pub total_shares: i128,
    pub property_type: String,   // "residential" | "commercial" | "land"
    pub ipfs_title_hash: String, // off-chain title document anchor
    pub kyc_tier_required: u32,  // minimum KYC tier for shareholders
}

const DAY_IN_LEDGERS: u32 = 17280;
const BUMP: u32 = 365 * DAY_IN_LEDGERS;
const THRESHOLD: u32 = BUMP - DAY_IN_LEDGERS;

#[contract]
pub struct PropertyToken;

#[contractimpl]
impl PropertyToken {
    /// Constructor — called atomically at deploy time via `stellar contract deploy -- --admin ...`.
    /// Eliminates the deploy→initialize front-running window.
    pub fn __constructor(
        env: Env,
        admin: Address,
        kyc_registry: Address,
        compliance_engine: Address,
        meta: PropertyMeta,
    ) {
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::KycRegistry, &kyc_registry);
        env.storage()
            .instance()
            .set(&DataKey::ComplianceEngine, &compliance_engine);
        env.storage()
            .instance()
            .set(&DataKey::TotalShares, &meta.total_shares);
        env.storage().instance().set(&DataKey::DividendPool, &0i128);
        env.storage()
            .instance()
            .set(&DataKey::DividendPerShare, &0i128);
        env.storage().instance().set(&DataKey::PropertyMeta, &meta);
    }

    /// Legacy entry point — always panics to prevent post-deploy initialization.
    pub fn initialize(
        env: Env,
        _admin: Address,
        _kyc_registry: Address,
        _compliance_engine: Address,
        _meta: PropertyMeta,
    ) {
        panic_with_error!(env, PropertyError::AlreadyInitialized);
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

    pub fn get_meta(env: Env) -> PropertyMeta {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        env.storage()
            .instance()
            .get(&DataKey::PropertyMeta)
            .expect("property meta must be set")
    }

    pub fn name(env: Env) -> String {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        String::from_str(&env, "Veritoken Property")
    }
    pub fn symbol(env: Env) -> String {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        String::from_str(&env, "VTPROP")
    }
    pub fn decimals(env: Env) -> u32 {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        0
    }

    // ── Share management ─────────────────────────────────────────────────────

    pub fn mint(env: Env, to: Address, shares: i128) {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        Self::require_admin(&env);
        Self::require_kyc(&env, &to);
        Self::require_tier(&env, &to);
        Self::check_mint_compliance(&env, &to, shares);
        if shares <= 0 {
            panic_with_error!(env, PropertyError::NegativeShares);
        }
        Self::accrue(&env, to.clone());
        let bal = Self::read_balance(&env, to.clone());
        Self::write_balance(&env, to.clone(), bal + shares);
        Self::reset_debt(&env, to.clone());
        Self::register_holder(&env, &to);
        env.events().publish((symbol_short!("mint"), to), shares);
    }

    pub fn transfer(env: Env, from: Address, to: Address, shares: i128) {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        from.require_auth();
        Self::require_kyc(&env, &from);
        Self::require_kyc(&env, &to);
        Self::check_compliance(&env, &from, &to, shares);
        if shares <= 0 {
            panic_with_error!(env, PropertyError::NegativeShares);
        }
        Self::accrue(&env, from.clone());
        Self::accrue(&env, to.clone());
        let from_bal = Self::read_balance(&env, from.clone());
        if from_bal < shares {
            panic_with_error!(env, PropertyError::InsufficientShares);
        }
        Self::write_balance(&env, from.clone(), from_bal - shares);
        let to_bal = Self::read_balance(&env, to.clone());
        Self::write_balance(&env, to.clone(), to_bal + shares);
        Self::reset_debt(&env, from.clone());
        Self::reset_debt(&env, to.clone());
        Self::register_holder(&env, &to);
        env.events()
            .publish((symbol_short!("transfer"), from, to), shares);
    }

    // ── SEP-41 Allowance / Delegated Transfer ───────────────────────────────

    /// Approve `spender` to transfer up to `amount` shares on behalf of `from`.
    /// The allowance expires at `expiration_ledger` (inclusive). Passing
    /// `amount = 0` revokes an existing allowance.
    pub fn approve(
        env: Env,
        from: Address,
        spender: Address,
        amount: i128,
        expiration_ledger: u32,
    ) {
        from.require_auth();
        if amount > 0 && expiration_ledger < env.ledger().sequence() {
            panic!("expiration_ledger is in the past");
        }
        let key = DataKey::Allowance(AllowanceKey {
            from: from.clone(),
            spender: spender.clone(),
        });
        let value = AllowanceValue {
            amount,
            expiration_ledger,
        };
        env.storage().temporary().set(&key, &value);
        if amount > 0 {
            let ttl = expiration_ledger - env.ledger().sequence();
            env.storage().temporary().extend_ttl(&key, ttl, ttl);
        }
        env.events().publish(
            (symbol_short!("approve"), from, spender),
            (amount, expiration_ledger),
        );
    }

    /// Returns the number of shares `spender` is allowed to transfer on behalf
    /// of `from`. Returns 0 if no allowance exists or it has expired.
    pub fn allowance(env: Env, from: Address, spender: Address) -> i128 {
        Self::read_allowance(&env, from, spender).amount
    }

    /// Transfer `shares` from `from` to `to` using a previously approved
    /// allowance. Runs the full compliance and dividend-snapshot logic.
    pub fn transfer_from(env: Env, spender: Address, from: Address, to: Address, shares: i128) {
        spender.require_auth();
        Self::require_kyc(&env, &from);
        Self::require_kyc(&env, &to);
        Self::check_compliance(&env, &from, &to, shares);
        if shares <= 0 {
            panic!("shares must be positive");
        }
        // Spend the allowance first — panics on insufficient/expired allowance.
        Self::spend_allowance(&env, from.clone(), spender, shares);
        // Snapshot accrued dividends for both parties before balances move.
        Self::accrue(&env, from.clone());
        Self::accrue(&env, to.clone());
        let from_bal = Self::read_balance(&env, from.clone());
        if from_bal < shares {
            panic!("insufficient shares");
        }
        Self::write_balance(&env, from.clone(), from_bal - shares);
        let to_bal = Self::read_balance(&env, to.clone());
        Self::write_balance(&env, to.clone(), to_bal + shares);
        Self::reset_debt(&env, from.clone());
        Self::reset_debt(&env, to.clone());
        Self::register_holder(&env, &to);
        env.events()
            .publish((symbol_short!("transfer"), from, to), shares);
    }

    // ── Dividends ────────────────────────────────────────────────────────────

    /// Deposit dividend amount (in stroops) to be distributed pro-rata.
    pub fn deposit_dividend(env: Env, amount: i128) {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        Self::require_admin(&env);
        let total: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalShares)
            .expect("total shares must be set");
        if total == 0 {
            panic_with_error!(env, PropertyError::NoShares);
        }
        let dps: i128 = env
            .storage()
            .instance()
            .get(&DataKey::DividendPerShare)
            .unwrap_or(0);
        let new_dps = dps + amount / total;
        env.storage()
            .instance()
            .set(&DataKey::DividendPerShare, &new_dps);
        let pool: i128 = env
            .storage()
            .instance()
            .get(&DataKey::DividendPool)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::DividendPool, &(pool + amount));
        env.events().publish((symbol_short!("div_dep"),), amount);
    }

    pub fn claim_dividend(env: Env, holder: Address) -> i128 {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        holder.require_auth();
        Self::accrue(&env, holder.clone());
        let key = DataKey::Unclaimed(holder.clone());
        let amount: i128 = env.storage().instance().get(&key).unwrap_or(0);
        if amount <= 0 {
            return 0;
        }
        env.storage().instance().set(&key, &0i128);
        let pool: i128 = env
            .storage()
            .instance()
            .get(&DataKey::DividendPool)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::DividendPool, &(pool - amount));
        env.events()
            .publish((symbol_short!("div_claim"), holder), amount);
        amount
    }

    pub fn pending_dividend(env: Env, holder: Address) -> i128 {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        let unclaimed: i128 = env
            .storage()
            .instance()
            .get(&DataKey::Unclaimed(holder.clone()))
            .unwrap_or(0);
        unclaimed + Self::accrued(&env, holder)
    }

    pub fn balance(env: Env, id: Address) -> i128 {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        Self::read_balance(&env, id)
    }
    pub fn total_shares(env: Env) -> i128 {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        env.storage()
            .instance()
            .get(&DataKey::TotalShares)
            .unwrap_or(0)
    }

    // ── Internals ────────────────────────────────────────────────────────────

    fn dps(env: &Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::DividendPerShare)
            .unwrap_or(0)
    }

    fn accrued(env: &Env, holder: Address) -> i128 {
        let bal = Self::read_balance(env, holder.clone());
        let debt: i128 = env
            .storage()
            .instance()
            .get(&DataKey::ClaimedDividend(holder))
            .unwrap_or(0);
        bal * Self::dps(env) - debt
    }

    fn accrue(env: &Env, holder: Address) {
        let owed = Self::accrued(env, holder.clone());
        if owed > 0 {
            let key = DataKey::Unclaimed(holder.clone());
            let unclaimed: i128 = env.storage().instance().get(&key).unwrap_or(0);
            env.storage().instance().set(&key, &(unclaimed + owed));
        }
        Self::reset_debt(env, holder);
    }

    fn reset_debt(env: &Env, holder: Address) {
        let bal = Self::read_balance(env, holder.clone());
        let debt = bal * Self::dps(env);
        env.storage()
            .instance()
            .set(&DataKey::ClaimedDividend(holder), &debt);
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
            panic_with_error!(env, PropertyError::KycNotApproved);
        }
    }

    fn require_tier(env: &Env, addr: &Address) {
        let registry: Address = env
            .storage()
            .instance()
            .get(&DataKey::KycRegistry)
            .expect("kyc registry must be set");
        let client = KycRegistryClient::new(env, &registry);
        let required = Self::get_meta(env.clone()).kyc_tier_required;
        let actual = client.get_tier(addr);
        if actual < required {
            panic_with_error!(env, PropertyError::KycTierTooLow);
        }
    }

    fn check_mint_compliance(env: &Env, to: &Address, shares: i128) {
        let engine: Address = env
            .storage()
            .instance()
            .get(&DataKey::ComplianceEngine)
            .expect("compliance engine must be set");
        let client = ComplianceEngineClient::new(env, &engine);
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if !client.can_transfer(&admin, to, &shares) {
            panic!("mint blocked by compliance");
        }
    }

    fn check_compliance(env: &Env, from: &Address, to: &Address, amount: i128) {
        let engine: Address = env
            .storage()
            .instance()
            .get(&DataKey::ComplianceEngine)
            .expect("compliance engine must be set");
        let client = ComplianceEngineClient::new(env, &engine);
        if !client.can_transfer(from, to, &amount) {
            panic_with_error!(env, PropertyError::TransferBlocked);
        }
    }

    fn register_holder(env: &Env, addr: &Address) {
        let engine: Address = env
            .storage()
            .instance()
            .get(&DataKey::ComplianceEngine)
            .expect("compliance engine must be set");
        let client = ComplianceEngineClient::new(env, &engine);
        client.register_holder(addr);
    }

    fn read_balance(env: &Env, addr: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Balance(addr))
            .unwrap_or(0)
    }

    fn write_balance(env: &Env, addr: Address, amount: i128) {
        let key = DataKey::Balance(addr);
        env.storage().persistent().set(&key, &amount);
        env.storage().persistent().extend_ttl(&key, THRESHOLD, BUMP);
    }

    // ── Allowance helpers ────────────────────────────────────────────────────

    fn read_allowance(env: &Env, from: Address, spender: Address) -> AllowanceValue {
        let key = DataKey::Allowance(AllowanceKey {
            from: from.clone(),
            spender: spender.clone(),
        });
        if let Some(val) = env
            .storage()
            .temporary()
            .get::<DataKey, AllowanceValue>(&key)
        {
            if val.expiration_ledger < env.ledger().sequence() {
                AllowanceValue {
                    amount: 0,
                    expiration_ledger: val.expiration_ledger,
                }
            } else {
                val
            }
        } else {
            AllowanceValue {
                amount: 0,
                expiration_ledger: 0,
            }
        }
    }

    fn spend_allowance(env: &Env, from: Address, spender: Address, amount: i128) {
        let allowance = Self::read_allowance(env, from.clone(), spender.clone());
        if allowance.amount < amount {
            panic!("insufficient allowance");
        }
        let new_amount = allowance.amount - amount;
        // Rewrite without bumping TTL — expiration is unchanged.
        let key = DataKey::Allowance(AllowanceKey {
            from: from.clone(),
            spender: spender.clone(),
        });
        let value = AllowanceValue {
            amount: new_amount,
            expiration_ledger: allowance.expiration_ledger,
        };
        env.storage().temporary().set(&key, &value);
        if new_amount > 0 {
            let current = env.ledger().sequence();
            if allowance.expiration_ledger > current {
                let ttl = allowance.expiration_ledger - current;
                env.storage().temporary().extend_ttl(&key, ttl, ttl);
            }
        }
    }
}

mod kyc_iface {
    use soroban_sdk::{contractclient, Address};
    #[contractclient(name = "KycRegistryClient")]
    #[allow(dead_code)]
    pub trait KycRegistry {
        fn is_approved(env: soroban_sdk::Env, addr: Address) -> bool;
        fn get_tier(env: soroban_sdk::Env, addr: Address) -> u32;
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
