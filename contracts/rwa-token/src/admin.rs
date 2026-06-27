#![cfg_attr(not(test), deny(clippy::unwrap_used))]

use soroban_sdk::{Address, Env};

use crate::storage_types::{DataKey, INSTANCE_BUMP_AMOUNT, INSTANCE_LIFETIME_THRESHOLD};

pub fn read_admin(env: &Env) -> Address {
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .expect("admin must be set")
}

pub fn write_admin(env: &Env, admin: &Address) {
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
    env.storage().instance().set(&DataKey::Admin, admin);
}
