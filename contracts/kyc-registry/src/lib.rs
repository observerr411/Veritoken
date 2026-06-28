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
    InvalidJurisdiction = 5,
}

#[contracttype]
pub enum DataKey {
    Admin,
    PendingAdmin,
    KycStatus(Address),
    VerifierList,
    VerifierCount,
    ExpiryIndex(u32),
    ExpiryIndexCount,
}

#[contracttype]
#[derive(Clone)]
pub struct ExpiryEntry {
    pub expiry: u64,
    pub addr: Address,
}

#[contracttype]
#[derive(Clone)]
pub struct ExpiringRecord {
    pub addr: Address,
    pub record: KycRecord,
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

    pub fn propose_admin(env: Env, new_admin: Address) {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        Self::require_admin(&env);
        env.storage().instance().set(&DataKey::PendingAdmin, &new_admin);
        env.events().publish((symbol_short!("proposed"),), new_admin);
    }

    pub fn accept_admin(env: Env) {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        let pending: Address = env.storage().instance().get(&DataKey::PendingAdmin).expect("no pending admin");
        pending.require_auth();
        let old_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        env.storage().instance().set(&DataKey::Admin, &pending);
        env.storage().instance().remove(&DataKey::PendingAdmin);
        env.events().publish((symbol_short!("admin_set"),), (old_admin, pending));
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
        Self::validate_jurisdiction(&env, &jurisdiction);
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

    fn validate_jurisdiction(env: &Env, jurisdiction: &String) {
        if jurisdiction.len() != 2 {
            panic_with_error!(env, KycError::InvalidJurisdiction);
        }
        let mut bytes = [0u8; 2];
        jurisdiction.copy_into_slice(&mut bytes);
        if bytes[0] < b'A' || bytes[0] > b'Z' || bytes[1] < b'A' || bytes[1] > b'Z' {
            panic_with_error!(env, KycError::InvalidJurisdiction);
        }
    }

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
        if record.status == KycStatus::Approved && record.expiry != 0 {
            let idx: u32 = env.storage().instance().get(&DataKey::ExpiryIndexCount).unwrap_or(0);
            let entry = ExpiryEntry { expiry: record.expiry, addr: addr.clone() };
            let ik = DataKey::ExpiryIndex(idx);
            env.storage().persistent().set(&ik, &entry);
            env.storage().persistent().extend_ttl(&ik, THRESHOLD, BUMP);
            env.storage().instance().set(&DataKey::ExpiryIndexCount, &(idx + 1));
        }
        let key = DataKey::KycStatus(addr);
        env.storage().persistent().set(&key, &record);
        env.storage().persistent().extend_ttl(&key, THRESHOLD, BUMP);
    }

    pub fn get_expiring_soon(env: Env, within_seconds: u64, start: u32, limit: u32) -> Vec<ExpiringRecord> {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        let count: u32 = env.storage().instance().get(&DataKey::ExpiryIndexCount).unwrap_or(0);
        let now = env.ledger().timestamp();
        let capped = limit.min(50);
        let mut out: Vec<ExpiringRecord> = Vec::new(&env);
        let mut i = start;
        while i < count && out.len() < capped {
            if let Some(entry) = env.storage().persistent().get::<DataKey, ExpiryEntry>(&DataKey::ExpiryIndex(i)) {
                if entry.expiry > now && entry.expiry <= now + within_seconds {
                    if let Some(record) = env.storage().persistent().get::<DataKey, KycRecord>(&DataKey::KycStatus(entry.addr.clone())) {
                        if record.status == KycStatus::Approved {
                            out.push_back(ExpiringRecord { addr: entry.addr, record });
                        }
                    }
                }
            }
            i += 1;
        }
        out
    }
}
