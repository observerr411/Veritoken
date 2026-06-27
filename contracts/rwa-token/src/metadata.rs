#![cfg_attr(not(test), deny(clippy::unwrap_used))]

use soroban_sdk::{Env, String};

use crate::storage_types::{
    DataKey, TokenMetadata, INSTANCE_BUMP_AMOUNT, INSTANCE_LIFETIME_THRESHOLD,
};

pub fn read_decimal(env: &Env) -> u32 {
    let meta = read_metadata(env);
    meta.decimal
}

pub fn read_name(env: &Env) -> String {
    let meta = read_metadata(env);
    meta.name
}

pub fn read_symbol(env: &Env) -> String {
    let meta = read_metadata(env);
    meta.symbol
}

pub fn read_metadata(env: &Env) -> TokenMetadata {
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
    env.storage()
        .instance()
        .get(&DataKey::Metadata)
        .expect("metadata must be set")
}

pub fn write_metadata(env: &Env, decimal: u32, name: String, symbol: String) {
    let meta = TokenMetadata {
        decimal,
        name,
        symbol,
    };
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
    env.storage().instance().set(&DataKey::Metadata, &meta);
}

pub fn read_asset_type(env: &Env) -> String {
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
    env.storage()
        .instance()
        .get(&DataKey::AssetType)
        .expect("asset type must be set")
}

pub fn write_asset_type(env: &Env, asset_type: String) {
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
    env.storage()
        .instance()
        .set(&DataKey::AssetType, &asset_type);
}
