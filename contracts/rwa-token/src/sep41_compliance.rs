#![cfg(test)]

use crate::{ComplianceMetadata, RwaToken, RwaTokenClient};
use compliance_engine::{ComplianceEngine, ComplianceEngineClient, ComplianceRules};
use kyc_registry::{KycRegistry, KycRegistryClient};
use soroban_sdk::{testutils::{Address as _, Ledger as _}, Address, Env, String};

/// Test harness for SEP-41 compliance tests
#[allow(dead_code)]
struct Sep41Harness {
    env: Env,
    token: RwaTokenClient<'static>,
    kyc: KycRegistryClient<'static>,
    compliance: ComplianceEngineClient<'static>,
    verifier: Address,
    admin: Address,
}

fn setup_sep41() -> Sep41Harness {
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
    compliance.initialize(&admin, &kyc_id);

    // RWA token
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
            Option::<ComplianceMetadata>::None,
        ),
    );
    let token = RwaTokenClient::new(&env, &token_id);

    Sep41Harness {
        env,
        token,
        kyc,
        compliance,
        verifier,
        admin,
    }
}

impl Sep41Harness {
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

// ── name ──────────────────────────────────────────────────────────────

#[test]
fn sep41_name() {
    let h = setup_sep41();
    assert_eq!(
        h.token.name(),
        String::from_str(&h.env, "Veritoken RWA")
    );
}

// ── symbol ────────────────────────────────────────────────────────────

#[test]
fn sep41_symbol() {
    let h = setup_sep41();
    assert_eq!(h.token.symbol(), String::from_str(&h.env, "VTRWA"));
}

// ── decimals ──────────────────────────────────────────────────────────

#[test]
fn sep41_decimals() {
    let h = setup_sep41();
    assert_eq!(h.token.decimals(), 7);
}

// ── total_supply ──────────────────────────────────────────────────────

#[test]
fn sep41_total_supply_initial() {
    let h = setup_sep41();
    assert_eq!(h.token.total_supply(), 0);
}

#[test]
fn sep41_total_supply_after_mint() {
    let h = setup_sep41();
    let alice = Address::generate(&h.env);
    h.approve_kyc(&alice);

    h.token.mint(&alice, &1_000);
    assert_eq!(h.token.total_supply(), 1_000);

    h.token.mint(&alice, &500);
    assert_eq!(h.token.total_supply(), 1_500);
}

#[test]
fn sep41_total_supply_after_transfer() {
    let h = setup_sep41();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    h.token.mint(&alice, &1_000);
    assert_eq!(h.token.total_supply(), 1_000);

    h.token.transfer(&alice, &bob, &600);
    assert_eq!(h.token.total_supply(), 1_000);
}

#[test]
fn sep41_total_supply_after_burn() {
    let h = setup_sep41();
    let alice = Address::generate(&h.env);
    h.approve_kyc(&alice);

    h.token.mint(&alice, &1_000);
    assert_eq!(h.token.total_supply(), 1_000);

    h.token.burn(&alice, &300);
    assert_eq!(h.token.total_supply(), 700);
}

// ── balance ────────────────────────────────────────────────────────────

#[test]
fn sep41_balance_of_nonexistent_account() {
    let h = setup_sep41();
    let unknown = Address::generate(&h.env);
    assert_eq!(h.token.balance(&unknown), 0);
}

#[test]
fn sep41_balance_after_mint() {
    let h = setup_sep41();
    let alice = Address::generate(&h.env);
    h.approve_kyc(&alice);

    h.token.mint(&alice, &1_000);
    assert_eq!(h.token.balance(&alice), 1_000);
}

#[test]
fn sep41_balance_after_transfer() {
    let h = setup_sep41();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    h.token.mint(&alice, &1_000);
    h.token.transfer(&alice, &bob, &400);

    assert_eq!(h.token.balance(&alice), 600);
    assert_eq!(h.token.balance(&bob), 400);
}

// ── transfer ──────────────────────────────────────────────────────────

#[test]
fn sep41_transfer_happy_path() {
    let h = setup_sep41();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    h.token.mint(&alice, &1_000);
    h.token.transfer(&alice, &bob, &350);

    assert_eq!(h.token.balance(&alice), 650);
    assert_eq!(h.token.balance(&bob), 350);
}

#[test]
fn sep41_transfer_insufficient_balance() {
    let h = setup_sep41();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    h.token.mint(&alice, &500);
    let res = h.token.try_transfer(&alice, &bob, &600);
    assert!(res.is_err());
}

#[test]
fn sep41_transfer_zero_amount() {
    let h = setup_sep41();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    h.token.mint(&alice, &1_000);
    let res = h.token.try_transfer(&alice, &bob, &0);
    assert!(res.is_err());
}

#[test]
fn sep41_transfer_requires_kyc_sender() {
    let h = setup_sep41();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.token.mint(&alice, &1_000);

    let res = h.token.try_transfer(&alice, &bob, &100);
    assert!(res.is_err());
}

#[test]
fn sep41_transfer_requires_kyc_receiver() {
    let h = setup_sep41();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.token.mint(&alice, &1_000);

    let res = h.token.try_transfer(&alice, &bob, &100);
    assert!(res.is_err());
}

// ── transfer_from ─────────────────────────────────────────────────────

#[test]
fn sep41_transfer_from_with_valid_allowance() {
    let h = setup_sep41();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let spender = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    h.token.mint(&alice, &1_000);
    let expiration = h.env.ledger().sequence() + 1_000;
    h.token.approve(&alice, &spender, &400, &expiration);
    h.token.transfer_from(&spender, &alice, &bob, &250);

    assert_eq!(h.token.balance(&alice), 750);
    assert_eq!(h.token.balance(&bob), 250);
}

#[test]
fn sep41_transfer_from_with_expired_allowance() {
    let h = setup_sep41();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let spender = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    h.token.mint(&alice, &1_000);
    let expiration = h.env.ledger().sequence() + 10;
    h.token.approve(&alice, &spender, &400, &expiration);

    // Advance ledger past expiration
    h.env.ledger().set_sequence_number(expiration + 1);

    let res = h.token.try_transfer_from(&spender, &alice, &bob, &100);
    assert!(res.is_err());
}

#[test]
fn sep41_transfer_from_insufficient_allowance() {
    let h = setup_sep41();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let spender = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    h.token.mint(&alice, &1_000);
    let expiration = h.env.ledger().sequence() + 1_000;
    h.token.approve(&alice, &spender, &200, &expiration);

    let res = h.token.try_transfer_from(&spender, &alice, &bob, &300);
    assert!(res.is_err());
}

// ── approve ───────────────────────────────────────────────────────────

#[test]
fn sep41_approve_new() {
    let h = setup_sep41();
    let alice = Address::generate(&h.env);
    let spender = Address::generate(&h.env);

    let expiration = h.env.ledger().sequence() + 500;
    h.token.approve(&alice, &spender, &1_000, &expiration);
    assert_eq!(h.token.allowance(&alice, &spender), 1_000);
}

#[test]
fn sep41_approve_update() {
    let h = setup_sep41();
    let alice = Address::generate(&h.env);
    let spender = Address::generate(&h.env);

    let expiration = h.env.ledger().sequence() + 500;
    h.token.approve(&alice, &spender, &1_000, &expiration);
    assert_eq!(h.token.allowance(&alice, &spender), 1_000);

    h.token.approve(&alice, &spender, &2_000, &expiration);
    assert_eq!(h.token.allowance(&alice, &spender), 2_000);
}

#[test]
fn sep41_approve_revoke_with_zero() {
    let h = setup_sep41();
    let alice = Address::generate(&h.env);
    let spender = Address::generate(&h.env);

    let expiration = h.env.ledger().sequence() + 500;
    h.token.approve(&alice, &spender, &1_000, &expiration);
    assert_eq!(h.token.allowance(&alice, &spender), 1_000);

    h.token.approve(&alice, &spender, &0, &expiration);
    assert_eq!(h.token.allowance(&alice, &spender), 0);
}

// ── allowance ─────────────────────────────────────────────────────────

#[test]
fn sep41_allowance_active() {
    let h = setup_sep41();
    let alice = Address::generate(&h.env);
    let spender = Address::generate(&h.env);

    let expiration = h.env.ledger().sequence() + 1_000;
    h.token.approve(&alice, &spender, &500, &expiration);
    assert_eq!(h.token.allowance(&alice, &spender), 500);
}

#[test]
fn sep41_allowance_expired() {
    let h = setup_sep41();
    let alice = Address::generate(&h.env);
    let spender = Address::generate(&h.env);

    let expiration = h.env.ledger().sequence() + 50;
    h.token.approve(&alice, &spender, &500, &expiration);
    assert_eq!(h.token.allowance(&alice, &spender), 500);

    h.env.ledger().set_sequence_number(expiration + 1);
    assert_eq!(h.token.allowance(&alice, &spender), 0);
}

#[test]
fn sep41_allowance_nonexistent() {
    let h = setup_sep41();
    let alice = Address::generate(&h.env);
    let spender = Address::generate(&h.env);

    assert_eq!(h.token.allowance(&alice, &spender), 0);
}

// ── burn ──────────────────────────────────────────────────────────────

#[test]
fn sep41_burn() {
    let h = setup_sep41();
    let alice = Address::generate(&h.env);
    h.approve_kyc(&alice);

    h.token.mint(&alice, &1_000);
    assert_eq!(h.token.total_supply(), 1_000);

    h.token.burn(&alice, &250);
    assert_eq!(h.token.balance(&alice), 750);
    assert_eq!(h.token.total_supply(), 750);
}

#[test]
fn sep41_burn_insufficient_balance() {
    let h = setup_sep41();
    let alice = Address::generate(&h.env);
    h.approve_kyc(&alice);

    h.token.mint(&alice, &500);
    let res = h.token.try_burn(&alice, &600);
    assert!(res.is_err());
}

// ── burn_from ─────────────────────────────────────────────────────────

#[test]
fn sep41_burn_from_with_allowance() {
    let h = setup_sep41();
    let alice = Address::generate(&h.env);
    let spender = Address::generate(&h.env);
    h.approve_kyc(&alice);

    h.token.mint(&alice, &1_000);
    let expiration = h.env.ledger().sequence() + 1_000;
    h.token.approve(&alice, &spender, &400, &expiration);
    h.token.burn_from(&spender, &alice, &150);

    assert_eq!(h.token.balance(&alice), 850);
    assert_eq!(h.token.total_supply(), 850);
    assert_eq!(h.token.allowance(&alice, &spender), 250);
}

#[test]
fn sep41_burn_from_insufficient_allowance() {
    let h = setup_sep41();
    let alice = Address::generate(&h.env);
    let spender = Address::generate(&h.env);
    h.approve_kyc(&alice);

    h.token.mint(&alice, &1_000);
    let expiration = h.env.ledger().sequence() + 1_000;
    h.token.approve(&alice, &spender, &100, &expiration);

    let res = h.token.try_burn_from(&spender, &alice, &150);
    assert!(res.is_err());
}
