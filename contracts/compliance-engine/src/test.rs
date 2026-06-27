#![cfg(test)]

use crate::{ComplianceEngine, ComplianceEngineClient, ComplianceRules};
use kyc_registry::{KycRegistry, KycRegistryClient};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env, String,
};

fn setup() -> (Env, ComplianceEngineClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);

    // A dummy KYC registry address suffices for tests that don't use jurisdiction checks.
    let kyc_id = env.register(KycRegistry, ());
    let kyc = KycRegistryClient::new(&env, &kyc_id);
    kyc.initialize(&admin);

    let contract_id = env.register(ComplianceEngine, ());
    let client = ComplianceEngineClient::new(&env, &contract_id);
    client.initialize(&admin, &kyc_id);
    (env, client, admin)
}

fn rules(
    max_transfer_amount: i128,
    min_holding_period: u64,
    max_holders: u32,
    paused: bool,
) -> ComplianceRules {
    ComplianceRules {
        max_transfer_amount,
        min_holding_period,
        max_holders,
        require_same_jurisdiction: false,
        paused,
    }
}

#[test]
fn test_default_rules_allow_transfer() {
    let (env, client, _admin) = setup();
    let from = Address::generate(&env);
    let to = Address::generate(&env);
    assert!(client.can_transfer(&from, &to, &1_000));
}

#[test]
fn test_pause_blocks_all_transfers() {
    let (env, client, _admin) = setup();
    let from = Address::generate(&env);
    let to = Address::generate(&env);

    client.pause();
    assert!(!client.can_transfer(&from, &to, &1));

    client.unpause();
    assert!(client.can_transfer(&from, &to, &1));
}

#[test]
fn test_blocklist() {
    let (env, client, _admin) = setup();
    let from = Address::generate(&env);
    let to = Address::generate(&env);

    assert!(!client.is_blocklisted(&from));
    client.add_to_blocklist(&from);
    assert!(client.is_blocklisted(&from));
    assert!(!client.can_transfer(&from, &to, &1));
    // The receiver being blocked also blocks
    assert!(!client.can_transfer(&to, &from, &1));

    client.remove_from_blocklist(&from);
    assert!(!client.is_blocklisted(&from));
    assert!(client.can_transfer(&from, &to, &1));
}

#[test]
fn test_max_transfer_amount() {
    let (env, client, _admin) = setup();
    let from = Address::generate(&env);
    let to = Address::generate(&env);

    client.set_rules(&rules(100, 0, 0, false));
    assert!(client.can_transfer(&from, &to, &100));
    assert!(!client.can_transfer(&from, &to, &101));
}

#[test]
fn test_min_holding_period() {
    let (env, client, _admin) = setup();
    let from = Address::generate(&env);
    let to = Address::generate(&env);

    client.set_rules(&rules(0, 1_000, 0, false));

    env.ledger().set_timestamp(5_000);
    client.register_holder(&from);
    assert_eq!(client.holder_count(), 1);

    // Not enough time elapsed
    env.ledger().set_timestamp(5_500);
    assert!(!client.can_transfer(&from, &to, &1));

    // Past holding period
    env.ledger().set_timestamp(6_001);
    assert!(client.can_transfer(&from, &to, &1));
}

#[test]
fn test_register_holder_is_idempotent() {
    let (env, client, _admin) = setup();
    let holder = Address::generate(&env);
    client.register_holder(&holder);
    client.register_holder(&holder);
    assert_eq!(client.holder_count(), 1);
}

#[test]
fn test_unregister_holder_decrements_count() {
    let (env, client, _admin) = setup();
    let holder = Address::generate(&env);
    client.register_holder(&holder);
    assert_eq!(client.holder_count(), 1);

    client.unregister_holder(&holder);
    assert_eq!(client.holder_count(), 0);
}

#[test]
fn test_max_holders_blocks_new_holder_but_allows_existing_holder() {
    let (env, client, _admin) = setup();
    let holder1 = Address::generate(&env);
    let holder2 = Address::generate(&env);
    let new_holder = Address::generate(&env);

    client.set_rules(&rules(0, 0, 2, false));
    client.register_holder(&holder1);
    client.register_holder(&holder2);
    assert_eq!(client.holder_count(), 2);

    assert!(!client.can_transfer(&holder1, &new_holder, &1));
    assert!(client.can_transfer(&holder1, &holder2, &1));
}

#[test]
fn test_only_admin_can_set_rules() {
    let env = Env::default();
    let admin = Address::generate(&env);

    let kyc_id = env.register(KycRegistry, ());
    let kyc = KycRegistryClient::new(&env, &kyc_id);
    // initialize KYC with admin auth
    env.mock_all_auths();
    kyc.initialize(&admin);

    let contract_id = env.register(ComplianceEngine, ());
    let client = ComplianceEngineClient::new(&env, &contract_id);
    client.initialize(&admin, &kyc_id);

    // Remove blanket auth — subsequent calls have no auth, so require_admin should fail
    env.set_auths(&[]);
    let res = client.try_set_rules(&rules(0, 0, 0, true));
    assert!(res.is_err());
}

#[test]
fn test_require_same_jurisdiction_blocks_cross_jurisdiction_transfer() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);

    let kyc_id = env.register(KycRegistry, ());
    let kyc = KycRegistryClient::new(&env, &kyc_id);
    kyc.initialize(&admin);
    let verifier = Address::generate(&env);
    kyc.add_verifier(&verifier);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    // alice = US, bob = GB
    kyc.approve(&verifier, &alice, &1, &0, &String::from_str(&env, "US"));
    kyc.approve(&verifier, &bob, &1, &0, &String::from_str(&env, "GB"));

    let ce_id = env.register(ComplianceEngine, ());
    let ce = ComplianceEngineClient::new(&env, &ce_id);
    ce.initialize(&admin, &kyc_id);

    ce.set_rules(&ComplianceRules {
        max_transfer_amount: 0,
        min_holding_period: 0,
        max_holders: 0,
        require_same_jurisdiction: true,
        paused: false,
    });

    // Cross-jurisdiction: blocked
    assert!(!ce.can_transfer(&alice, &bob, &100));

    // Same jurisdiction: allowed
    let carol = Address::generate(&env);
    kyc.approve(&verifier, &carol, &1, &0, &String::from_str(&env, "US"));
    assert!(ce.can_transfer(&alice, &carol, &100));
}
