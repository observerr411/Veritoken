#![no_std]

#[cfg(test)]
mod test;

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, String, Vec};

#[contracttype]
pub enum DataKey {
    Admin,
    KycStatus(Address),
    VerifierList,
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
            panic!("already initialized");
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
        }
        env.storage().instance().set(&DataKey::VerifierList, &list);
        env.events().publish((symbol_short!("add_vrf"),), verifier);
    }

    pub fn remove_verifier(env: Env, verifier: Address) {
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);
        Self::require_admin(&env);
        let list = Self::verifier_list(&env);
        let mut new_list: Vec<Address> = Vec::new(&env);
        for v in list.iter() {
            if v != verifier {
                new_list.push_back(v);
            }
        }
        env.storage()
            .instance()
            .set(&DataKey::VerifierList, &new_list);
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
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
    }

    fn require_verifier(env: &Env, verifier: &Address) {
        let list = Self::verifier_list(env);
        if !list.contains(verifier) {
            panic!("not an authorized verifier");
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
