#![no_std]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]

#[cfg(test)]
mod test;

use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror, panic_with_error, symbol_short,
    Address, Env, String, Vec,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum KycError {
    AlreadyInitialized = 1,
    NotVerifier = 2,
    NotApproved = 3,
    NoRecord = 4,
}

#[contracttype]
pub enum DataKey {
    Admin,
    KycStatus(Address),
    VerifierList,
    VerifierCount,
}

#[contracttype]
#[derive(Clone, PartialEq)]
pub enum KycStatus {
    Pending,
    Approved,
    Rejected,
    Revoked,
}

#[contracttype]
#[derive(Clone)]
pub struct KycRecord {
    pub status: KycStatus,
    pub verifier: Address,
    pub tier: u32,   // 0=basic, 1=accredited, 2=institutional
    pub expiry: u64, // ledger timestamp; 0 = no expiry
    pub jurisdiction: String,
}

const DAY_IN_LEDGERS: u32 = 17280;
const BUMP: u32 = 30 * DAY_IN_LEDGERS;
const THRESHOLD: u32 = BUMP - DAY_IN_LEDGERS;

#[contract]
pub struct KycRegistry;

#[contractimpl]
impl KycRegistry {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(env, KycError::AlreadyInitialized);
        }
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    // ── Verifier management ──────────────────────────────────────────────────

    pub fn add_verifier(env: Env, verifier: Address) {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        Self::require_admin(&env);
        let mut list = Self::verifier_list(&env);
        if !list.contains(&verifier) {
            list.push_back(verifier.clone());
            env.storage().instance().set(&DataKey::VerifierList, &list);
            // Increment the count only when a new entry is actually added.
            let count: u32 = env
                .storage()
                .instance()
                .get(&DataKey::VerifierCount)
                .unwrap_or(0);
            env.storage()
                .instance()
                .set(&DataKey::VerifierCount, &(count + 1));
        } else {
            env.storage().instance().set(&DataKey::VerifierList, &list);
        }
        env.events().publish((symbol_short!("add_vrf"),), verifier);
    }

    pub fn remove_verifier(env: Env, verifier: Address) {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        Self::require_admin(&env);
        let list = Self::verifier_list(&env);
        let mut new_list: Vec<Address> = Vec::new(&env);
        let mut removed = false;
        for v in list.iter() {
            if v != verifier {
                new_list.push_back(v);
            } else {
                removed = true;
            }
        }
        env.storage()
            .instance()
            .set(&DataKey::VerifierList, &new_list);
        if removed {
            let count: u32 = env
                .storage()
                .instance()
                .get(&DataKey::VerifierCount)
                .unwrap_or(0);
            let new_count = if count > 0 { count - 1 } else { 0 };
            env.storage()
                .instance()
                .set(&DataKey::VerifierCount, &new_count);
        }
    }

    /// Returns the total number of registered verifiers.
    pub fn verifier_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::VerifierCount)
            .unwrap_or(0)
    }

    /// Returns the full verifier list (internal use; prefer `get_verifiers` from external callers).
    pub fn verifier_list_pub(env: Env) -> Vec<Address> {
        Self::verifier_list(&env)
    }

    /// Paged verifier query. `start` is a zero-based offset; `limit` is capped at 20.
    /// Returns an empty vec when `start` is beyond the end of the list.
    pub fn get_verifiers(env: Env, start: u32, limit: u32) -> Vec<Address> {
        let cap: u32 = 20;
        let effective_limit = if limit > cap { cap } else { limit };
        let list = Self::verifier_list(&env);
        let total = list.len();
        let mut result: Vec<Address> = Vec::new(&env);
        if start >= total {
            return result;
        }
        let end = (start + effective_limit).min(total);
        for i in start..end {
            result.push_back(list.get(i).unwrap());
        }
        result
    }

    // ── KYC operations ───────────────────────────────────────────────────────

    pub fn approve(
        env: Env,
        verifier: Address,
        subject: Address,
        tier: u32,
        expiry: u64,
        jurisdiction: String,
    ) {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        verifier.require_auth();
        Self::require_verifier(&env, &verifier);
        let record = KycRecord {
            status: KycStatus::Approved,
            verifier: verifier.clone(),
            tier,
            expiry,
            jurisdiction,
        };
        Self::write_record(&env, subject.clone(), record);
        env.events()
            .publish((symbol_short!("approved"), subject), verifier);
    }

    pub fn reject(env: Env, verifier: Address, subject: Address) {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        verifier.require_auth();
        Self::require_verifier(&env, &verifier);
        let mut record = Self::get_record_or_default(&env, subject.clone(), &verifier);
        record.status = KycStatus::Rejected;
        Self::write_record(&env, subject.clone(), record);
        env.events()
            .publish((symbol_short!("rejected"), subject), verifier);
    }

    pub fn revoke(env: Env, verifier: Address, subject: Address) {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        verifier.require_auth();
        Self::require_verifier(&env, &verifier);
        let mut record = Self::get_record_or_default(&env, subject.clone(), &verifier);
        record.status = KycStatus::Revoked;
        Self::write_record(&env, subject.clone(), record);
        env.events()
            .publish((symbol_short!("revoked"), subject), verifier);
    }

    /// Update only the `tier` field of an existing, Approved KYC record.
    /// Requires verifier auth and the subject must currently be Approved.
    /// Emits a `tier_upd` event with `(subject, new_tier)`.
    pub fn update_tier(env: Env, verifier: Address, subject: Address, new_tier: u32) {
        verifier.require_auth();
        Self::require_verifier(&env, &verifier);
        let mut record = env
            .storage()
            .persistent()
            .get::<DataKey, KycRecord>(&DataKey::KycStatus(subject.clone()))
            .expect("no KYC record for subject");
        if record.status != KycStatus::Approved {
            panic!("subject is not currently approved");
        }
        record.tier = new_tier;
        Self::write_record(&env, subject.clone(), record);
        env.events()
            .publish((symbol_short!("tier_upd"), subject), new_tier);
    }

    // ── Queries ──────────────────────────────────────────────────────────────

    /// Returns true if the address has an active, non-expired KYC approval.
    pub fn is_approved(env: Env, addr: Address) -> bool {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        let key = DataKey::KycStatus(addr);
        if let Some(record) = env.storage().persistent().get::<DataKey, KycRecord>(&key) {
            if record.status != KycStatus::Approved {
                return false;
            }
            if record.expiry != 0 && record.expiry < env.ledger().timestamp() {
                return false;
            }
            true
        } else {
            false
        }
    }

    pub fn get_record(env: &Env, addr: Address) -> KycRecord {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        env.storage()
            .persistent()
            .get(&DataKey::KycStatus(addr))
            .expect("no KYC record")
    }

    pub fn get_tier(env: Env, addr: Address) -> u32 {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        Self::get_record(&env, addr).tier
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

    fn require_verifier(env: &Env, verifier: &Address) {
        let list = Self::verifier_list(env);
        if !list.contains(verifier) {
            panic_with_error!(env, KycError::NotVerifier);
        }
    }

    fn verifier_list(env: &Env) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&DataKey::VerifierList)
            .unwrap_or_else(|| Vec::new(env))
    }

    fn get_record_or_default(env: &Env, addr: Address, verifier: &Address) -> KycRecord {
        env.storage()
            .persistent()
            .get(&DataKey::KycStatus(addr))
            .unwrap_or_else(|| KycRecord {
                status: KycStatus::Pending,
                verifier: verifier.clone(),
                tier: 0,
                expiry: 0,
                jurisdiction: String::from_str(env, ""),
            })
    }

    fn write_record(env: &Env, addr: Address, record: KycRecord) {
        let key = DataKey::KycStatus(addr);
        env.storage().persistent().set(&key, &record);
        env.storage().persistent().extend_ttl(&key, THRESHOLD, BUMP);
    }
}
