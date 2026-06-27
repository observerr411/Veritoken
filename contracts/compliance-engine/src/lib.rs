#![no_std]

#[cfg(test)]
mod test;

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, Vec};

#[contracttype]
pub enum DataKey {
    Admin,
    KycRegistry,
    Rules,
    Blocklist,
    MaxTransfer,
    MinHoldingPeriod,
    MaxHolders,
    HolderCount,
    HolderSince(Address),
}

#[contracttype]
#[derive(Clone)]
pub struct ComplianceRules {
    pub max_transfer_amount: i128, // 0 = unlimited
    pub min_holding_period: u64,   // seconds; 0 = none
    pub max_holders: u32,          // 0 = unlimited
    pub require_same_jurisdiction: bool,
    pub paused: bool,
}

const DAY_IN_LEDGERS: u32 = 17280;
const BUMP: u32 = 30 * DAY_IN_LEDGERS;
const THRESHOLD: u32 = BUMP - DAY_IN_LEDGERS;

#[contract]
pub struct ComplianceEngine;

#[contractimpl]
impl ComplianceEngine {
    pub fn initialize(env: Env, admin: Address, kyc_registry: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::KycRegistry, &kyc_registry);
        let default_rules = ComplianceRules {
            max_transfer_amount: 0,
            min_holding_period: 0,
            max_holders: 0,
            require_same_jurisdiction: false,
            paused: false,
        };
        env.storage()
            .instance()
            .set(&DataKey::Rules, &default_rules);
        env.storage().instance().set(&DataKey::HolderCount, &0u32);
    }

    // ── Rule management ──────────────────────────────────────────────────────

    pub fn set_rules(env: Env, rules: ComplianceRules) {
        Self::require_admin(&env);
        env.storage().instance().set(&DataKey::Rules, &rules);
        env.events().publish((symbol_short!("rules_set"),), ());
    }

    pub fn get_rules(env: Env) -> ComplianceRules {
        env.storage().instance().get(&DataKey::Rules).unwrap()
    }

    pub fn add_to_blocklist(env: Env, addr: Address) {
        Self::require_admin(&env);
        let mut list = Self::blocklist(&env);
        if !list.contains(&addr) {
            list.push_back(addr.clone());
        }
        env.storage().instance().set(&DataKey::Blocklist, &list);
        env.events().publish((symbol_short!("blocked"),), addr);
    }

    pub fn remove_from_blocklist(env: Env, addr: Address) {
        Self::require_admin(&env);
        let list = Self::blocklist(&env);
        let mut new_list: Vec<Address> = Vec::new(&env);
        for a in list.iter() {
            if a != addr {
                new_list.push_back(a);
            }
        }
        env.storage().instance().set(&DataKey::Blocklist, &new_list);
    }

    pub fn is_blocklisted(env: Env, addr: Address) -> bool {
        Self::blocklist(&env).contains(&addr)
    }

    pub fn pause(env: Env) {
        Self::require_admin(&env);
        let mut rules: ComplianceRules = env.storage().instance().get(&DataKey::Rules).unwrap();
        rules.paused = true;
        env.storage().instance().set(&DataKey::Rules, &rules);
        env.events().publish((symbol_short!("paused"),), ());
    }

    pub fn unpause(env: Env) {
        Self::require_admin(&env);
        let mut rules: ComplianceRules = env.storage().instance().get(&DataKey::Rules).unwrap();
        rules.paused = false;
        env.storage().instance().set(&DataKey::Rules, &rules);
        env.events().publish((symbol_short!("unpaused"),), ());
    }

    // ── Transfer validation ──────────────────────────────────────────────────

    /// Called by asset tokens before every transfer. Returns true if the
    /// transfer is compliant with all configured rules.
    pub fn can_transfer(env: Env, from: Address, to: Address, amount: i128) -> bool {
        let rules: ComplianceRules = env.storage().instance().get(&DataKey::Rules).unwrap();

        if rules.paused {
            return false;
        }

        let blocklist = Self::blocklist(&env);
        if blocklist.contains(&from) || blocklist.contains(&to) {
            return false;
        }

        if rules.require_same_jurisdiction {
            let kyc_registry: Address = env
                .storage()
                .instance()
                .get(&DataKey::KycRegistry)
                .unwrap();
            let kyc = kyc_iface::KycRegistryClient::new(&env, &kyc_registry);
            let from_record = kyc.get_record(&from);
            let to_record = kyc.get_record(&to);
            if from_record.jurisdiction != to_record.jurisdiction {
                return false;
            }
        }

        if rules.max_transfer_amount > 0 && amount > rules.max_transfer_amount {
            return false;
        }

        if rules.min_holding_period > 0 {
            let key = DataKey::HolderSince(from.clone());
            if let Some(since) = env.storage().persistent().get::<DataKey, u64>(&key) {
                let elapsed = env.ledger().timestamp().saturating_sub(since);
                if elapsed < rules.min_holding_period {
                    return false;
                }
            }
        }

        if rules.max_holders > 0 {
            let key = DataKey::HolderSince(to.clone());
            if !env.storage().persistent().has(&key) {
                let count = Self::holder_count(env);
                if count >= rules.max_holders {
                    return false;
                }
            }
        }

        true
    }

    /// Called by rwa-token after a mint or transfer to register a new holder.
    pub fn register_holder(env: Env, addr: Address) {
        let key = DataKey::HolderSince(addr.clone());
        let is_new = !env.storage().persistent().has(&key);
        env.storage()
            .persistent()
            .set(&key, &env.ledger().timestamp());
        env.storage().persistent().extend_ttl(&key, THRESHOLD, BUMP);
        if is_new {
            let count: u32 = env
                .storage()
                .instance()
                .get(&DataKey::HolderCount)
                .unwrap_or(0);
            env.storage()
                .instance()
                .set(&DataKey::HolderCount, &(count + 1));
        }
    }

    /// Called by rwa-token after a transfer or burn that removes the last token from a holder.
    pub fn unregister_holder(env: Env, addr: Address) {
        let key = DataKey::HolderSince(addr.clone());
        if env.storage().persistent().has(&key) {
            env.storage().persistent().remove(&key);
            let count: u32 = env
                .storage()
                .instance()
                .get(&DataKey::HolderCount)
                .unwrap_or(0);
            let new_count = if count > 0 { count - 1 } else { 0 };
            env.storage()
                .instance()
                .set(&DataKey::HolderCount, &new_count);
        }
    }

    pub fn holder_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::HolderCount)
            .unwrap_or(0)
    }

    // ── Internals ────────────────────────────────────────────────────────────

    fn require_admin(env: &Env) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
    }

    fn blocklist(env: &Env) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&DataKey::Blocklist)
            .unwrap_or_else(|| Vec::new(env))
    }
}

mod kyc_iface {
    use soroban_sdk::{contractclient, contracttype, Address, String};

    #[contracttype]
    #[derive(Clone)]
    pub struct KycRecord {
        pub status: KycStatus,
        pub verifier: Address,
        pub tier: u32,
        pub expiry: u64,
        pub jurisdiction: String,
    }

    #[contracttype]
    #[derive(Clone)]
    pub enum KycStatus {
        Pending,
        Approved,
        Rejected,
        Revoked,
    }

    #[contractclient(name = "KycRegistryClient")]
    #[allow(dead_code)]
    pub trait KycRegistry {
        fn get_record(env: soroban_sdk::Env, addr: Address) -> KycRecord;
    }
}
