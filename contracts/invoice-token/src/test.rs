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
        transfer_fee_bps: 0,
        fee_recipient: None,
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
    let result = h
        .token
        .try_initialize(&attacker, &kyc_id, &ce_id, &meta(&h.env));
    assert!(result.is_err());
}

// ── Transfer ─────────────────────────────────────────────────────────────────

#[test]
fn test_transfer_zero_fee() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.token.issue(&alice, &10_000);

    h.token.transfer(&alice, &bob, &3_000);

    assert_eq!(h.token.balance(&alice), 7_000);
    assert_eq!(h.token.balance(&bob), 3_000);
    assert_eq!(h.token.total_supply(), 10_000);
}

#[test]
fn test_transfer_with_fee_deducted_from_recipient() {
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

    let fee_collector = Address::generate(&env);
    kyc.approve(&verifier, &fee_collector, &1, &0, &String::from_str(&env, "US"));

    // 50 bps = 0.5% fee
    let fee_meta = InvoiceMeta {
        invoice_id: String::from_str(&env, "INV-FEE"),
        issuer: String::from_str(&env, "Acme"),
        debtor: String::from_str(&env, "Globex"),
        face_value_usd: 1_000_000_000_000,
        discount_rate_bps: 0,
        due_date: 1_900_000_000,
        currency: String::from_str(&env, "USD"),
        ipfs_doc_hash: String::from_str(&env, "Qm..."),
        transfer_fee_bps: 50,
        fee_recipient: Some(fee_collector.clone()),
    };

    let token_id = env.register(
        InvoiceToken,
        (admin.clone(), kyc_id.clone(), compliance_id.clone(), fee_meta),
    );
    let token = InvoiceTokenClient::new(&env, &token_id);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    kyc.approve(&verifier, &alice, &1, &0, &String::from_str(&env, "US"));
    kyc.approve(&verifier, &bob, &1, &0, &String::from_str(&env, "US"));

    token.issue(&alice, &10_000);

    // 50 bps of 10_000 = 5
    token.transfer(&alice, &bob, &10_000);

    assert_eq!(token.balance(&alice), 0);
    assert_eq!(token.balance(&bob), 9_995);     // receives amount - fee
    assert_eq!(token.balance(&fee_collector), 5); // receives fee
    assert_eq!(token.total_supply(), 10_000);
}

#[test]
fn test_transfer_fee_recipient_receives_exact_amount() {
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

    let fee_collector = Address::generate(&env);
    kyc.approve(&verifier, &fee_collector, &1, &0, &String::from_str(&env, "US"));

    // 100 bps = 1% fee
    let fee_meta = InvoiceMeta {
        invoice_id: String::from_str(&env, "INV-FEE2"),
        issuer: String::from_str(&env, "Acme"),
        debtor: String::from_str(&env, "Globex"),
        face_value_usd: 1_000_000_000_000,
        discount_rate_bps: 0,
        due_date: 1_900_000_000,
        currency: String::from_str(&env, "USD"),
        ipfs_doc_hash: String::from_str(&env, "Qm..."),
        transfer_fee_bps: 100,
        fee_recipient: Some(fee_collector.clone()),
    };

    let token_id = env.register(
        InvoiceToken,
        (admin.clone(), kyc_id.clone(), compliance_id.clone(), fee_meta),
    );
    let token = InvoiceTokenClient::new(&env, &token_id);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    kyc.approve(&verifier, &alice, &1, &0, &String::from_str(&env, "US"));
    kyc.approve(&verifier, &bob, &1, &0, &String::from_str(&env, "US"));

    token.issue(&alice, &5_000);

    // Two transfers — fee_collector accumulates across both
    token.transfer(&alice, &bob, &2_000); // fee = 20, bob gets 1980
    token.transfer(&bob, &alice, &1_000); // fee = 10, alice gets 990

    assert_eq!(token.balance(&fee_collector), 30); // 20 + 10
    assert_eq!(token.balance(&bob), 980);
    assert_eq!(token.balance(&alice), 3_990); // 3_000 remaining + 990 received
}

// ── update_meta ───────────────────────────────────────────────────────────────

#[test]
fn test_update_meta_before_settlement() {
    let h = setup();

    let updated = InvoiceMeta {
        invoice_id: String::from_str(&h.env, "INV-001"),
        issuer: String::from_str(&h.env, "Acme Corp"),
        debtor: String::from_str(&h.env, "Globex"),
        face_value_usd: 2_000_000_000_000, // amended face value
        discount_rate_bps: 300,
        due_date: 1_950_000_000, // extended due date
        currency: String::from_str(&h.env, "USD"),
        ipfs_doc_hash: String::from_str(&h.env, "Qm_new..."),
        transfer_fee_bps: 0,
        fee_recipient: None,
    };

    h.token.update_meta(&updated);

    let stored = h.token.get_meta();
    assert_eq!(stored.face_value_usd, 2_000_000_000_000);
    assert_eq!(stored.due_date, 1_950_000_000);
    assert_eq!(
        stored.ipfs_doc_hash,
        String::from_str(&h.env, "Qm_new...")
    );
}

#[test]
fn test_update_meta_blocked_after_settlement() {
    let h = setup();
    h.token.settle();

    let updated = InvoiceMeta {
        invoice_id: String::from_str(&h.env, "INV-001"),
        issuer: String::from_str(&h.env, "Acme Corp"),
        debtor: String::from_str(&h.env, "Globex"),
        face_value_usd: 999,
        discount_rate_bps: 0,
        due_date: 1,
        currency: String::from_str(&h.env, "USD"),
        ipfs_doc_hash: String::from_str(&h.env, "Qm_bad"),
        transfer_fee_bps: 0,
        fee_recipient: None,
    };

    assert!(h.token.try_update_meta(&updated).is_err());
}
