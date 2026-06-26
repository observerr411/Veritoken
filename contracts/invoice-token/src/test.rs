#![cfg(test)]

use crate::{InvoiceMeta, InvoiceToken, InvoiceTokenClient};
use compliance_engine::{ComplianceEngine, ComplianceEngineClient};
use kyc_registry::{KycRegistry, KycRegistryClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

struct Harness {
    env: Env,
    token: InvoiceTokenClient<'static>,
    kyc: KycRegistryClient<'static>,
    compliance: ComplianceEngineClient<'static>,
    verifier: Address,
    #[allow(dead_code)]
    admin: Address,
}

fn meta(env: &Env) -> InvoiceMeta {
    InvoiceMeta {
        invoice_id: String::from_str(env, "INV-001"),
        issuer: String::from_str(env, "Acme Corp"),
        debtor: String::from_str(env, "Globex"),
        face_value_usd: 1_000_000_000_000, // 100,000 USD at 7 decimals
        discount_rate_bps: 250,
        due_date: 1_900_000_000,
        currency: String::from_str(env, "USD"),
        ipfs_doc_hash: String::from_str(env, "Qm..."),
    }
}

fn setup() -> Harness {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);

    let kyc_id = env.register(KycRegistry, ());
    let kyc = KycRegistryClient::new(&env, &kyc_id);
    kyc.initialize(&admin);
    let verifier = Address::generate(&env);
    kyc.add_verifier(&verifier);

    let compliance_id = env.register(ComplianceEngine, ());
    let compliance = ComplianceEngineClient::new(&env, &compliance_id);
    compliance.initialize(&admin);

    // Invoice token — constructor args passed atomically at register time
    let token_id = env.register(
        InvoiceToken,
        (
            admin.clone(),
            kyc_id.clone(),
            compliance_id.clone(),
            meta(&env),
        ),
    );
    let token = InvoiceTokenClient::new(&env, &token_id);

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
    assert_eq!(
        h.token.name(),
        String::from_str(&h.env, "Veritoken Invoice")
    );
    assert_eq!(
        h.token.get_meta().invoice_id,
        String::from_str(&h.env, "INV-001")
    );
    assert!(!h.token.is_settled());
}

#[test]
fn test_issue_requires_kyc() {
    let h = setup();
    let holder = Address::generate(&h.env);

    assert!(h.token.try_issue(&holder, &1_000).is_err());

    h.approve_kyc(&holder);
    h.token.issue(&holder, &1_000);
    assert_eq!(h.token.balance(&holder), 1_000);
    assert_eq!(h.token.total_supply(), 1_000);
}

#[test]
fn test_settle_then_redeem() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);
    h.token.issue(&holder, &1_000);

    // Cannot redeem before settlement
    assert!(h.token.try_redeem(&holder, &500).is_err());

    h.token.settle();
    assert!(h.token.is_settled());

    h.token.redeem(&holder, &600);
    assert_eq!(h.token.balance(&holder), 400);
    assert_eq!(h.token.total_supply(), 400);
}

#[test]
fn test_cannot_issue_after_settle() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);
    h.token.settle();
    assert!(h.token.try_issue(&holder, &1).is_err());
}

#[test]
fn test_redeem_insufficient_balance() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);
    h.token.issue(&holder, &100);
    h.token.settle();
    assert!(h.token.try_redeem(&holder, &101).is_err());
}

#[test]
fn test_non_deployer_cannot_reinitialize() {
    let h = setup();
    let attacker = Address::generate(&h.env);
    let kyc_id = Address::generate(&h.env);
    let ce_id = Address::generate(&h.env);
    // initialize must always panic — the constructor has already run
    let result = h
        .token
        .try_initialize(&attacker, &kyc_id, &ce_id, &meta(&h.env));
    assert!(result.is_err());
}

#[test]
fn test_transfer_requires_kyc_on_both_parties() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.token.issue(&alice, &1_000);

    // Bob has no KYC — transfer must fail
    assert!(h.token.try_transfer(&alice, &bob, &100).is_err());

    h.approve_kyc(&bob);
    h.token.transfer(&alice, &bob, &100);
    assert_eq!(h.token.balance(&alice), 900);
    assert_eq!(h.token.balance(&bob), 100);
}

#[test]
fn test_transfer_blocked_when_settled() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.token.issue(&alice, &500);
    h.token.settle();

    assert!(h.token.try_transfer(&alice, &bob, &100).is_err());
}

#[test]
fn test_approve_and_transfer_from() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let spender = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.token.issue(&alice, &1_000);

    let expiry = h.env.ledger().sequence() + 1_000;
    h.token.approve(&alice, &spender, &300, &expiry);
    assert_eq!(h.token.allowance(&alice, &spender), 300);

    h.token.transfer_from(&spender, &alice, &bob, &200);
    assert_eq!(h.token.balance(&alice), 800);
    assert_eq!(h.token.balance(&bob), 200);
    assert_eq!(h.token.allowance(&alice, &spender), 100);
}

#[test]
fn test_transfer_from_insufficient_allowance() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let spender = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.token.issue(&alice, &1_000);

    let expiry = h.env.ledger().sequence() + 1_000;
    h.token.approve(&alice, &spender, &50, &expiry);
    assert!(h
        .token
        .try_transfer_from(&spender, &alice, &bob, &51)
        .is_err());
}

#[test]
fn test_redeem_blocked_when_compliance_paused() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);
    h.token.issue(&holder, &100);
    h.token.settle();

    h.compliance.pause();
    assert!(h.token.try_redeem(&holder, &50).is_err());
}
