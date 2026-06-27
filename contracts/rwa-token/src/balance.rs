#![cfg_attr(not(test), deny(clippy::unwrap_used))]

use soroban_sdk::{Address, Env};

use crate::storage_types::{
    DataKey, BALANCE_BUMP_AMOUNT, BALANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT,
    INSTANCE_LIFETIME_THRESHOLD,
};
use crate::RwaError;

pub fn read_balance(env: &Env, addr: Address) -> i128 {
    let key = DataKey::Balance(addr);
    if let Some(balance) = env.storage().persistent().get::<DataKey, i128>(&key) {
        env.storage().persistent().extend_ttl(
            &key,
            BALANCE_LIFETIME_THRESHOLD,
            BALANCE_BUMP_AMOUNT,
        );
        balance
    } else {
        0
    }
}

pub fn write_balance(env: &Env, addr: Address, amount: i128) {
    let key = DataKey::Balance(addr);
    env.storage().persistent().set(&key, &amount);
    env.storage()
        .persistent()
        .extend_ttl(&key, BALANCE_LIFETIME_THRESHOLD, BALANCE_BUMP_AMOUNT);
}

pub fn receive_balance(env: &Env, addr: Address, amount: i128) {
    let balance = read_balance(env, addr.clone());
    write_balance(env, addr, balance + amount);
}

pub fn spend_balance(env: &Env, addr: Address, amount: i128) {
    let balance = read_balance(env, addr.clone());
    if balance < amount {
        soroban_sdk::panic_with_error!(env, RwaError::InsufficientBalance);
    }
    write_balance(env, addr, balance - amount);
}

pub fn read_total_supply(env: &Env) -> i128 {
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
    env.storage()
        .instance()
        .get(&DataKey::TotalSupply)
        .unwrap_or(0)
}

pub fn write_total_supply(env: &Env, supply: i128) {
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
    env.storage().instance().set(&DataKey::TotalSupply, &supply);
}
