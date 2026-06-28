#![no_std]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]

//! Carbon Credit Token — 1 token = 1 verified tonne of CO₂ equivalent retired.
//! Tokens are burned ("retired") to claim the carbon offset; retired credits
//! are permanently removed from circulation with an on-chain retirement receipt.
//! Minting is admin-gated and still enforces active KYC plus mint-time
//! compliance checks for pause/blocklist rules.

extern crate alloc;

#[cfg(test)]
mod test;

use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror, panic_with_error, symbol_short,
    Address, Env, String, Vec,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum CarbonError {
    AlreadyInitialized = 1,
    InsufficientBalance = 2,
    KycNotApproved = 3,
    CompliancePaused = 4,
    Blocklisted = 5,
    TransferBlocked = 6,
}

#[contracttype]
pub enum DataKey {
    Admin,
    PendingAdmin,
    KycRegistry,
    ComplianceEngine,
    ProjectMeta,
    Balance(Address),
    TotalSupply,
    TotalRetired,
    RetirementCount,
    Receipt(u32),
}

#[contracttype]
#[derive(Clone)]
pub struct ProjectMeta {
    pub project_id: String,
    pub standard: String, // "VCS" | "Gold Standard" | "CDM" | "ACR"
    pub vintage_year: u32,
    pub project_name: String,
    pub project_type: String, // "forestry" | "renewable" | "methane_capture"
    pub country: String,
    pub verifier: String,
    pub ipfs_cert_hash: String, // verification certificate
}

#[contracttype]
#[derive(Clone)]
pub struct RetirementReceipt {
    pub retiree: Address,
    pub amount: i128,
    pub timestamp: u64,
    pub beneficiary: String,
    pub retirement_reason: String,
}

const DAY_IN_LEDGERS: u32 = 17280;
const BUMP: u32 = 365 * DAY_IN_LEDGERS;
const THRESHOLD: u32 = BUMP - DAY_IN_LEDGERS;
const MAX_PAGE_SIZE: u32 = 100;

#[contract]
pub struct CarbonCreditToken;

#[contractimpl]
impl CarbonCreditToken {
    fn validate_project_type(env: &Env, pt: &String) {
        if *pt != String::from_str(env, "forestry")
            && *pt != String::from_str(env, "renewable")
            && *pt != String::from_str(env, "methane_capture")
        {
            panic!("invalid project_type");
        }
    }

    /// Constructor — called atomically at deploy time via `stellar contract deploy -- --admin ...`.
    /// Eliminates the deploy→initialize front-running window.
    pub fn __constructor(
        env: Env,
        admin: Address,
        kyc_registry: Address,
        compliance_engine: Address,
        meta: ProjectMeta,
    ) {
        Self::validate_project_type(&env, &meta.project_type);
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::KycRegistry, &kyc_registry);
        env.storage()
            .instance()
            .set(&DataKey::ComplianceEngine, &compliance_engine);
        env.storage().instance().set(&DataKey::ProjectMeta, &meta);
        env.storage().instance().set(&DataKey::TotalSupply, &0i128);
        env.storage().instance().set(&DataKey::TotalRetired, &0i128);
        env.storage()
            .instance()
            .set(&DataKey::RetirementCount, &0u32);
    }

    /// Legacy entry point — always panics to prevent post-deploy initialization.
    pub fn initialize(
        env: Env,
        _admin: Address,
        _kyc_registry: Address,
        _compliance_engine: Address,
        _meta: ProjectMeta,
    ) {
        panic_with_error!(env, CarbonError::AlreadyInitialized);
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
        env.storage().instance().set(&DataKey::PendingAdmin, &new_admin);
        env.events().publish((symbol_short!("proposed"),), new_admin);
    }

    pub fn accept_admin(env: Env) {
        let pending: Address = env.storage().instance().get(&DataKey::PendingAdmin).expect("no pending admin");
        pending.require_auth();
        let old_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        env.storage().instance().set(&DataKey::Admin, &pending);
        env.storage().instance().remove(&DataKey::PendingAdmin);
        env.events().publish((symbol_short!("admin_set"),), (old_admin, pending));
    }

    // ── Metadata ─────────────────────────────────────────────────────────────

    pub fn get_meta(env: Env) -> ProjectMeta {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        env.storage().instance().get(&DataKey::ProjectMeta).unwrap()
    }

    /// Replace project metadata. Admin-only; project_id is immutable.
    pub fn update_meta(env: Env, new_meta: ProjectMeta) {
        Self::require_admin(&env);
        Self::validate_project_type(&env, &new_meta.project_type);
        let old_meta: ProjectMeta = env.storage().instance().get(&DataKey::ProjectMeta).unwrap();
        if new_meta.project_id != old_meta.project_id {
            panic!("project_id is immutable");
        }
        env.storage().instance().set(&DataKey::ProjectMeta, &new_meta);
        env.events().publish((symbol_short!("upd_meta"),), ());
    }

    pub fn name(env: Env) -> String {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        String::from_str(&env, "Veritoken Carbon Credit")
    }
    pub fn symbol(env: Env) -> String {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        String::from_str(&env, "VTCC")
    }
    pub fn decimals(env: Env) -> u32 {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        0
    }

    // ── Issuance ─────────────────────────────────────────────────────────────

    pub fn mint(env: Env, to: Address, amount: i128) {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        Self::require_admin(&env);
        Self::require_kyc(&env, &to);
        Self::check_mint_compliance(&env, &to);
        let bal = Self::read_balance(&env, to.clone());
        Self::write_balance(&env, to.clone(), bal + amount);
        let supply: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::TotalSupply, &(supply + amount));
        Self::register_holder(&env, &to);
        env.events().publish((symbol_short!("mint"), to), amount);
    }

    // ── Transfer ─────────────────────────────────────────────────────────────

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        from.require_auth();
        Self::require_kyc(&env, &from);
        Self::require_kyc(&env, &to);
        Self::check_compliance(&env, &from, &to, amount);
        let from_bal = Self::read_balance(&env, from.clone());
        if from_bal < amount {
            panic_with_error!(env, CarbonError::InsufficientBalance);
        }
        Self::write_balance(&env, from.clone(), from_bal - amount);
        let to_bal = Self::read_balance(&env, to.clone());
        Self::write_balance(&env, to.clone(), to_bal + amount);
        Self::register_holder(&env, &to);
        env.events()
            .publish((symbol_short!("transfer"), from, to), amount);
    }

    // ── Retirement ───────────────────────────────────────────────────────────

    /// Permanently burn tokens and record a retirement receipt on-chain.
    pub fn retire(
        env: Env,
        retiree: Address,
        amount: i128,
        beneficiary: String,
        reason: String,
    ) -> RetirementReceipt {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        retiree.require_auth();
        Self::require_kyc(&env, &retiree);
        let bal = Self::read_balance(&env, retiree.clone());
        if bal < amount {
            panic_with_error!(env, CarbonError::InsufficientBalance);
        }
        Self::write_balance(&env, retiree.clone(), bal - amount);
        let supply: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::TotalSupply, &(supply - amount));
        let retired: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalRetired)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::TotalRetired, &(retired + amount));

        let index: u32 = env
            .storage()
            .instance()
            .get(&DataKey::RetirementCount)
            .unwrap_or(0);
        let receipt = RetirementReceipt {
            retiree: retiree.clone(),
            amount,
            timestamp: env.ledger().timestamp(),
            beneficiary,
            retirement_reason: reason,
        };
        let key = DataKey::Receipt(index);
        env.storage().persistent().set(&key, &receipt);
        env.storage().persistent().extend_ttl(&key, THRESHOLD, BUMP);
        env.storage()
            .instance()
            .set(&DataKey::RetirementCount, &(index + 1));

        env.events()
            .publish((symbol_short!("retired"), retiree), amount);
        receipt
    }

    // ── Read API ─────────────────────────────────────────────────────────────

    pub fn retirement_count(env: Env) -> u32 {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        env.storage()
            .instance()
            .get(&DataKey::RetirementCount)
            .unwrap_or(0)
    }

    pub fn get_receipt(env: Env, index: u32) -> RetirementReceipt {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        env.storage()
            .persistent()
            .get(&DataKey::Receipt(index))
            .expect("receipt not found")
    }

    /// Returns up to `limit` receipts starting at `start`. Limit is capped at MAX_PAGE_SIZE.
    pub fn get_receipts(env: Env, start: u32, limit: u32) -> Vec<RetirementReceipt> {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        let count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::RetirementCount)
            .unwrap_or(0);
        let capped = limit.min(MAX_PAGE_SIZE);
        let end = (start + capped).min(count);
        let mut out = Vec::new(&env);
        for i in start..end {
            let r: RetirementReceipt = env
                .storage()
                .persistent()
                .get(&DataKey::Receipt(i))
                .expect("receipt not found");
            out.push_back(r);
        }
        out
    }

    /// Returns a JSON-formatted retirement certificate for the given receipt index.
    pub fn to_certificate_json(env: Env, index: u32) -> String {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        let receipt: RetirementReceipt = env
            .storage()
            .persistent()
            .get(&DataKey::Receipt(index))
            .expect("receipt not found");
        let meta: ProjectMeta = env.storage().instance().get(&DataKey::ProjectMeta).unwrap();

        fn push_soroban_str(out: &mut alloc::vec::Vec<u8>, s: &String) {
            let len = s.len() as usize;
            let start = out.len();
            out.resize(start + len, 0);
            s.copy_into_slice(&mut out[start..]);
        }

        fn push_u128(out: &mut alloc::vec::Vec<u8>, mut n: u128) {
            if n == 0 { out.push(b'0'); return; }
            let mut buf = [0u8; 39];
            let mut pos = 39usize;
            while n > 0 { pos -= 1; buf[pos] = b'0' + (n % 10) as u8; n /= 10; }
            out.extend_from_slice(&buf[pos..]);
        }

        fn push_i128(out: &mut alloc::vec::Vec<u8>, n: i128) {
            if n < 0 {
                out.push(b'-');
                push_u128(out, if n == i128::MIN { 170141183460469231731687303715884105728u128 } else { (-n) as u128 });
            } else {
                push_u128(out, n as u128);
            }
        }

        let mut out: alloc::vec::Vec<u8> = alloc::vec::Vec::new();
        out.extend_from_slice(b"{\"project_id\":\"");
        push_soroban_str(&mut out, &meta.project_id);
        out.extend_from_slice(b"\",\"standard\":\"");
        push_soroban_str(&mut out, &meta.standard);
        out.extend_from_slice(b"\",\"vintage_year\":");
        push_u128(&mut out, meta.vintage_year as u128);
        out.extend_from_slice(b",\"retiree\":\"");
        let retiree_str = receipt.retiree.to_string();
        push_soroban_str(&mut out, &retiree_str);
        out.extend_from_slice(b"\",\"amount\":");
        push_i128(&mut out, receipt.amount);
        out.extend_from_slice(b",\"timestamp\":");
        push_u128(&mut out, receipt.timestamp as u128);
        out.extend_from_slice(b",\"beneficiary\":\"");
        push_soroban_str(&mut out, &receipt.beneficiary);
        out.extend_from_slice(b"\",\"retirement_reason\":\"");
        push_soroban_str(&mut out, &receipt.retirement_reason);
        out.extend_from_slice(b"\"}");

        String::from_bytes(&env, &out)
    }

    pub fn balance(env: Env, id: Address) -> i128 {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        Self::read_balance(&env, id)
    }
    pub fn total_supply(env: Env) -> i128 {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        env.storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0)
    }
    pub fn total_retired(env: Env) -> i128 {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        env.storage()
            .instance()
            .get(&DataKey::TotalRetired)
            .unwrap_or(0)
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
            panic_with_error!(env, CarbonError::KycNotApproved);
        }
    }

    fn check_mint_compliance(env: &Env, to: &Address) {
        let engine: Address = env
            .storage()
            .instance()
            .get(&DataKey::ComplianceEngine)
            .expect("compliance engine must be set");
        let client = ComplianceEngineClient::new(env, &engine);
        if client.get_rules().paused {
            panic_with_error!(env, CarbonError::CompliancePaused);
        }
        if client.is_blocklisted(to) {
            panic_with_error!(env, CarbonError::Blocklisted);
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
            panic_with_error!(env, CarbonError::TransferBlocked);
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
