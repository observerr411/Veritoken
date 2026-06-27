#![cfg_attr(not(test), deny(clippy::unwrap_used))]

use soroban_sdk::{Address, Env};

use crate::storage_types::{DataKey, INSTANCE_BUMP_AMOUNT, INSTANCE_LIFETIME_THRESHOLD};
use crate::RwaError;

pub fn read_kyc_registry(env: &Env) -> Address {
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
    env.storage()
        .instance()
        .get(&DataKey::KycRegistry)
        .expect("kyc registry must be set")
}

pub fn write_kyc_registry(env: &Env, registry: &Address) {
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
    env.storage()
        .instance()
        .set(&DataKey::KycRegistry, registry);
}

/// Cross-contract call to the KYC registry to verify a holder is approved.
pub fn require_kyc(env: &Env, addr: &Address) {
    let registry = read_kyc_registry(env);
    let client = KycRegistryClient::new(env, &registry);
    if !client.is_approved(addr) {
        soroban_sdk::panic_with_error!(env, RwaError::KycNotApproved);
    }
}

mod kyc_registry_interface {
    use soroban_sdk::{contractclient, Address};

    #[contractclient(name = "KycRegistryClient")]
    #[allow(dead_code)]
    pub trait KycRegistryInterface {
        fn is_approved(env: soroban_sdk::Env, addr: Address) -> bool;
    }
}
use kyc_registry_interface::KycRegistryClient;
