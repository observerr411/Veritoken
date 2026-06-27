#![cfg_attr(not(test), deny(clippy::unwrap_used))]

use soroban_sdk::{Address, Env};

use crate::storage_types::{AllowanceKey, AllowanceValue, DataKey};
use crate::RwaError;

pub fn read_allowance(env: &Env, from: Address, spender: Address) -> AllowanceValue {
    let key = DataKey::Allowance(AllowanceKey {
        from: from.clone(),
        spender: spender.clone(),
    });
    if let Some(allowance) = env
        .storage()
        .temporary()
        .get::<DataKey, AllowanceValue>(&key)
    {
        if allowance.expiration_ledger < env.ledger().sequence() {
            AllowanceValue {
                amount: 0,
                expiration_ledger: allowance.expiration_ledger,
            }
        } else {
            allowance
        }
    } else {
        AllowanceValue {
            amount: 0,
            expiration_ledger: 0,
        }
    }
}

pub fn write_allowance(
    env: &Env,
    from: Address,
    spender: Address,
    amount: i128,
    expiration_ledger: u32,
) {
    if amount > 0 && expiration_ledger < env.ledger().sequence() {
        soroban_sdk::panic_with_error!(env, RwaError::AllowanceExpired);
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
        env.storage().temporary().extend_ttl(
            &key,
            expiration_ledger - env.ledger().sequence(),
            expiration_ledger - env.ledger().sequence(),
        );
    }
}

pub fn spend_allowance(env: &Env, from: Address, spender: Address, amount: i128) {
    let allowance = read_allowance(env, from.clone(), spender.clone());
    if allowance.amount < amount {
        soroban_sdk::panic_with_error!(env, RwaError::InsufficientAllowance);
    }
    let new_amount = allowance.amount - amount;
    write_allowance(env, from, spender, new_amount, allowance.expiration_ledger);
}
