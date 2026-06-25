#![no_std]

use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, String, Symbol};

mod admin;
mod allowance;
mod balance;
mod compliance;
mod kyc;
mod metadata;
mod storage_types;

#[cfg(test)]
mod test;

#[contract]
pub struct RwaToken;

#[contractimpl]
impl RwaToken {
    /// Initialize the RWA token with metadata and compliance configuration.
    #[allow(clippy::too_many_arguments)]
    pub fn initialize(
        env: Env,
        admin: Address,
        decimal: u32,
        name: String,
        symbol: String,
        asset_type: String, // "invoice" | "property" | "carbon_credit"
        kyc_registry: Address,
        compliance_engine: Address,
    ) {
        if storage_types::has_admin(&env) {
            panic!("already initialized");
        }
        admin::write_admin(&env, &admin);
        metadata::write_metadata(&env, decimal, name, symbol);
        metadata::write_asset_type(&env, asset_type);
        kyc::write_kyc_registry(&env, &kyc_registry);
        compliance::write_compliance_engine(&env, &compliance_engine);
        balance::write_total_supply(&env, 0);
    }

    // ── Admin ────────────────────────────────────────────────────────────────

    pub fn set_admin(env: Env, new_admin: Address) {
        let admin = admin::read_admin(&env);
        admin.require_auth();
        admin::write_admin(&env, &new_admin);
        env.events()
            .publish((symbol_short!("admin"),), (admin, new_admin));
    }

    // ── SEP-41 Token Interface ───────────────────────────────────────────────

    pub fn allowance(env: Env, from: Address, spender: Address) -> i128 {
        allowance::read_allowance(&env, from, spender).amount
    }

    pub fn approve(
        env: Env,
        from: Address,
        spender: Address,
        amount: i128,
        expiration_ledger: u32,
    ) {
        from.require_auth();
        allowance::write_allowance(
            &env,
            from.clone(),
            spender.clone(),
            amount,
            expiration_ledger,
        );
        env.events().publish(
            (symbol_short!("approve"), from, spender),
            (amount, expiration_ledger),
        );
    }

    pub fn balance(env: Env, id: Address) -> i128 {
        balance::read_balance(&env, id)
    }

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        kyc::require_kyc(&env, &from);
        kyc::require_kyc(&env, &to);
        compliance::check_transfer(&env, &from, &to, amount);
        let from_balance_before = balance::read_balance(&env, from.clone());
        let to_balance_before = balance::read_balance(&env, to.clone());
        balance::spend_balance(&env, from.clone(), amount);
        balance::receive_balance(&env, to.clone(), amount);
        if from != to {
            if amount > 0 && to_balance_before == 0 {
                compliance::register_holder(&env, &to);
            }
            if amount > 0 && from_balance_before == amount {
                compliance::unregister_holder(&env, &from);
            }
        }
        env.events()
            .publish((symbol_short!("transfer"), from, to), amount);
    }

    pub fn transfer_from(env: Env, spender: Address, from: Address, to: Address, amount: i128) {
        spender.require_auth();
        kyc::require_kyc(&env, &from);
        kyc::require_kyc(&env, &to);
        compliance::check_transfer(&env, &from, &to, amount);
        let from_balance_before = balance::read_balance(&env, from.clone());
        let to_balance_before = balance::read_balance(&env, to.clone());
        allowance::spend_allowance(&env, from.clone(), spender, amount);
        balance::spend_balance(&env, from.clone(), amount);
        balance::receive_balance(&env, to.clone(), amount);
        if from != to {
            if amount > 0 && to_balance_before == 0 {
                compliance::register_holder(&env, &to);
            }
            if amount > 0 && from_balance_before == amount {
                compliance::unregister_holder(&env, &from);
            }
        }
        env.events()
            .publish((symbol_short!("transfer"), from, to), amount);
    }

    pub fn burn(env: Env, from: Address, amount: i128) {
        from.require_auth();
        kyc::require_kyc(&env, &from);
        let from_balance_before = balance::read_balance(&env, from.clone());
        balance::spend_balance(&env, from.clone(), amount);
        if amount > 0 && from_balance_before == amount {
            compliance::unregister_holder(&env, &from);
        }
        let supply = balance::read_total_supply(&env);
        balance::write_total_supply(&env, supply - amount);
        env.events().publish((symbol_short!("burn"), from), amount);
    }

    pub fn burn_from(env: Env, spender: Address, from: Address, amount: i128) {
        spender.require_auth();
        let from_balance_before = balance::read_balance(&env, from.clone());
        allowance::spend_allowance(&env, from.clone(), spender, amount);
        balance::spend_balance(&env, from.clone(), amount);
        if amount > 0 && from_balance_before == amount {
            compliance::unregister_holder(&env, &from);
        }
        let supply = balance::read_total_supply(&env);
        balance::write_total_supply(&env, supply - amount);
        env.events().publish((symbol_short!("burn"), from), amount);
    }

    pub fn decimals(env: Env) -> u32 {
        metadata::read_decimal(&env)
    }

    pub fn name(env: Env) -> String {
        metadata::read_name(&env)
    }

    pub fn symbol(env: Env) -> String {
        metadata::read_symbol(&env)
    }

    pub fn total_supply(env: Env) -> i128 {
        balance::read_total_supply(&env)
    }

    // ── Minting (admin-only) ─────────────────────────────────────────────────

    pub fn mint(env: Env, to: Address, amount: i128) {
        let admin = admin::read_admin(&env);
        admin.require_auth();
        kyc::require_kyc(&env, &to);
        let previous_balance = balance::read_balance(&env, to.clone());
        balance::receive_balance(&env, to.clone(), amount);
        if amount > 0 && previous_balance == 0 {
            compliance::register_holder(&env, &to);
        }
        let supply = balance::read_total_supply(&env);
        balance::write_total_supply(&env, supply + amount);
        env.events().publish((symbol_short!("mint"), to), amount);
    }

    // ── RWA Compliance Metadata ──────────────────────────────────────────────

    pub fn asset_type(env: Env) -> String {
        metadata::read_asset_type(&env)
    }

    pub fn kyc_registry(env: Env) -> Address {
        kyc::read_kyc_registry(&env)
    }

    pub fn compliance_engine(env: Env) -> Address {
        compliance::read_compliance_engine(&env)
    }

    pub fn set_compliance_metadata(env: Env, key: Symbol, value: String) {
        let admin = admin::read_admin(&env);
        admin.require_auth();
        compliance::write_metadata(&env, key, value);
    }

    pub fn get_compliance_metadata(env: Env, key: Symbol) -> String {
        compliance::read_metadata(&env, key)
    }
}
