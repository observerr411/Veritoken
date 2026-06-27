#![cfg(test)]

use crate::{RwaToken, RwaTokenClient};
use compliance_engine::{ComplianceEngine, ComplianceEngineClient, ComplianceRules};
use kyc_registry::{KycRegistry, KycRegistryClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

struct Harness {
    env: Env,
    token: RwaTokenClient<'static>,
    kyc: KycRegistryClient<'static>,
    compliance: ComplianceEngineClient<'static>,
    verifier: Address,
    admin: Address,
}

fn setup() -> Harness {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);

    // KYC registry
    let kyc_id = env.register(KycRegistry, ());
    let kyc = KycRegistryClient::new(&env, &kyc_id);
    kyc.initialize(&admin);
    let verifier = Address::generate(&env);
    kyc.add_verifier(&verifier);

    // Compliance engine
    let compliance_id = env.register(ComplianceEngine, ());
    let compliance = ComplianceEngineClient::new(&env, &compliance_id);
    compliance.initialize(&admin);

    // RWA token — constructor args passed atomically at register time
    let token_id = env.register(
        RwaToken,
        (
            admin.clone(),
            7u32,
            String::from_str(&env, "Veritoken RWA"),
            String::from_str(&env, "VTRWA"),
            String::from_str(&env, "property"),
            kyc_id.clone(),
            compliance_id.clone(),
        ),
    );
    let token = RwaTokenClient::new(&env, &token_id);

    Harness {
        env,
        token,
        kyc,
        compliance,
        verifier,
        admin,
    }
}

impl Harness {
    fn approve_kyc(&self, addr: &Address) {
        self.kyc.approve(
            &self.verifier,
            addr,
            &1,
            &0,
            &String::from_str(&self.env, "US"),
        );
    }
}

#[test]
fn test_metadata() {
    let h = setup();
    assert_eq!(h.token.decimals(), 7);
    assert_eq!(h.token.name(), String::from_str(&h.env, "Veritoken RWA"));
    assert_eq!(h.token.symbol(), String::from_str(&h.env, "VTRWA"));
    assert_eq!(h.token.asset_type(), String::from_str(&h.env, "property"));
    assert_eq!(h.token.total_supply(), 0);
}

#[test]
fn test_mint_requires_kyc() {
    let h = setup();
    let user = Address::generate(&h.env);

    // Without KYC, mint should fail
    let res = h.token.try_mint(&user, &1_000);
    assert!(res.is_err());

    // With KYC, mint succeeds
    h.approve_kyc(&user);
    h.token.mint(&user, &1_000);
    assert_eq!(h.token.balance(&user), 1_000);
    assert_eq!(h.token.total_supply(), 1_000);
}

#[test]
fn test_transfer_happy_path() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    h.token.mint(&alice, &1_000);
    h.token.transfer(&alice, &bob, &400);

    assert_eq!(h.token.balance(&alice), 600);
    assert_eq!(h.token.balance(&bob), 400);
}

#[test]
fn test_transfer_blocked_without_kyc_on_receiver() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env); // no KYC
    h.approve_kyc(&alice);
    h.token.mint(&alice, &1_000);

    let res = h.token.try_transfer(&alice, &bob, &100);
    assert!(res.is_err());
}

#[test]
fn test_transfer_blocked_when_compliance_paused() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.token.mint(&alice, &1_000);

    h.compliance.pause();
    let res = h.token.try_transfer(&alice, &bob, &100);
    assert!(res.is_err());

    h.compliance.unpause();
    h.token.transfer(&alice, &bob, &100);
    assert_eq!(h.token.balance(&bob), 100);
}

#[test]
fn test_transfer_blocked_by_max_amount() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.token.mint(&alice, &1_000);

    h.compliance.set_rules(&ComplianceRules {
        max_transfer_amount: 50,
        min_holding_period: 0,
        max_holders: 0,
        require_same_jurisdiction: false,
        paused: false,
    });

    assert!(h.token.try_transfer(&alice, &bob, &51).is_err());
    h.token.transfer(&alice, &bob, &50);
    assert_eq!(h.token.balance(&bob), 50);
}

#[test]
fn test_max_holder_cap_blocks_new_holder_and_maintains_count() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let charlie = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.approve_kyc(&charlie);

    h.compliance.set_rules(&ComplianceRules {
        max_transfer_amount: 0,
        min_holding_period: 0,
        max_holders: 2,
        require_same_jurisdiction: false,
        paused: false,
    });

    h.token.mint(&alice, &1_000);
    assert_eq!(h.compliance.holder_count(), 1);

    h.token.transfer(&alice, &bob, &400);
    assert_eq!(h.compliance.holder_count(), 2);
    assert!(h.token.try_transfer(&alice, &charlie, &1).is_err());

    h.token.transfer(&alice, &bob, &600);
    assert_eq!(h.token.balance(&alice), 0);
    assert_eq!(h.compliance.holder_count(), 1);

    h.token.transfer(&bob, &charlie, &1);
    assert_eq!(h.compliance.holder_count(), 2);
    assert_eq!(h.token.balance(&charlie), 1);
}

#[test]
fn test_approve_and_transfer_from() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let spender = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.token.mint(&alice, &1_000);

    let expiration = h.env.ledger().sequence() + 1_000;
    h.token.approve(&alice, &spender, &300, &expiration);
    assert_eq!(h.token.allowance(&alice, &spender), 300);

    h.token.transfer_from(&spender, &alice, &bob, &200);
    assert_eq!(h.token.balance(&bob), 200);
    assert_eq!(h.token.balance(&alice), 800);
    assert_eq!(h.token.allowance(&alice, &spender), 100);
}

#[test]
fn test_burn_reduces_supply() {
    let h = setup();
    let alice = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.token.mint(&alice, &1_000);

    h.token.burn(&alice, &400);
    assert_eq!(h.token.balance(&alice), 600);
    assert_eq!(h.token.total_supply(), 600);
}

#[test]
fn test_set_admin() {
    let h = setup();
    let new_admin = Address::generate(&h.env);
    h.token.set_admin(&new_admin);
    // New admin can mint after KYC approval of a holder
    let user = Address::generate(&h.env);
    h.approve_kyc(&user);
    h.token.mint(&user, &1);
    assert_eq!(h.token.balance(&user), 1);
    let _ = &h.admin;
}

#[test]
fn test_compliance_metadata() {
    let h = setup();
    let key = soroban_sdk::symbol_short!("legal");
    h.token
        .set_compliance_metadata(&key, &String::from_str(&h.env, "prospectus-v1"));
    assert_eq!(
        h.token.get_compliance_metadata(&key),
        String::from_str(&h.env, "prospectus-v1")
    );
}

#[test]
fn test_non_deployer_cannot_reinitialize() {
    let h = setup();
    let attacker = Address::generate(&h.env);
    let kyc_id = h.token.kyc_registry();
    let ce_id = h.token.compliance_engine();
    // initialize must always panic — the constructor has already run
    let result = h.token.try_initialize(
        &attacker,
        &7,
        &String::from_str(&h.env, "Evil Token"),
        &String::from_str(&h.env, "EVIL"),
        &String::from_str(&h.env, "property"),
        &kyc_id,
        &ce_id,
    );
    assert!(result.is_err());
}

#[test]
fn test_mint_twice_same_address_holder_count_is_one() {
    let h = setup();
    let user = Address::generate(&h.env);
    h.approve_kyc(&user);

    h.token.mint(&user, &1_000);
    h.token.mint(&user, &500);

    assert_eq!(h.compliance.holder_count(), 1);
    assert_eq!(h.token.balance(&user), 1_500);
    assert_eq!(h.token.total_supply(), 1_500);
}
