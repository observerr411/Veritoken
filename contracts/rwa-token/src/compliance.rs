use soroban_sdk::{Address, Env, String, Symbol};

use crate::storage_types::{DataKey, INSTANCE_BUMP_AMOUNT, INSTANCE_LIFETIME_THRESHOLD};

pub fn read_compliance_engine(env: &Env) -> Address {
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
    env.storage()
        .instance()
        .get(&DataKey::ComplianceEngine)
        .unwrap()
}

pub fn write_compliance_engine(env: &Env, engine: &Address) {
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
    env.storage()
        .instance()
        .set(&DataKey::ComplianceEngine, engine);
}

pub fn write_metadata(env: &Env, key: Symbol, value: String) {
    env.storage()
        .instance()
        .set(&DataKey::ComplianceMeta(key), &value);
}

pub fn read_metadata(env: &Env, key: Symbol) -> String {
    env.storage()
        .instance()
        .get(&DataKey::ComplianceMeta(key))
        .unwrap_or_else(|| String::from_str(env, ""))
}

/// Cross-contract call to the compliance engine to validate a transfer.
pub fn check_transfer(env: &Env, from: &Address, to: &Address, amount: i128) {
    let engine = read_compliance_engine(env);
    let client = ComplianceEngineClient::new(env, &engine);
    if !client.can_transfer(from, to, &amount) {
        panic!("transfer blocked by compliance engine");
    }
}

pub fn register_holder(env: &Env, addr: &Address) {
    let engine = read_compliance_engine(env);
    let client = ComplianceEngineClient::new(env, &engine);
    client.register_holder(addr);
}

pub fn unregister_holder(env: &Env, addr: &Address) {
    let engine = read_compliance_engine(env);
    let client = ComplianceEngineClient::new(env, &engine);
    client.unregister_holder(addr);
}

mod compliance_interface {
    use soroban_sdk::{contractclient, Address};

    #[contractclient(name = "ComplianceEngineClient")]
    #[allow(dead_code)]
    pub trait ComplianceEngineInterface {
        fn can_transfer(env: soroban_sdk::Env, from: Address, to: Address, amount: i128) -> bool;
        fn register_holder(env: soroban_sdk::Env, addr: Address);
        fn unregister_holder(env: soroban_sdk::Env, addr: Address);
        fn holder_count(env: soroban_sdk::Env) -> u32;
    }
}
use compliance_interface::ComplianceEngineClient;
