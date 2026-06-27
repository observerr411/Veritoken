#![cfg(test)]

use crate::{InvoiceMeta, InvoiceToken, InvoiceTokenClient};
use compliance_engine::{ComplianceEngine, ComplianceEngineClient};
use kyc_registry::{KycRegistry, KycRegistryClient};
use soroban_sdk::{testutils::{Address as _, Events as _, Ledger as _}, Address, Env, IntoVal, String};

// ── Test harness ─────────────────────────────────────────────────────────────

#[allow(dead_code)]
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
        transfer_fee_bps: 0,
        fee_recipient: None,
    }
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

    let compliance_id = env.register(ComplianceEngine, ());
    let compliance = ComplianceEngineClient::new(&env, &compliance_id);
    compliance.initialize(&admin, &kyc_id);

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

#[test]
fn test_issue_idempotency_holder_count() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);
    
    // First issue
    h.token.issue(&holder, &1_000);
    assert_eq!(h.compliance.holder_count(), 1);
    
    // Second issue
    h.token.issue(&holder, &500);
    assert_eq!(h.compliance.holder_count(), 1);
    assert_eq!(h.token.balance(&holder), 1_500);
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

// ── Existing tests ────────────────────────────────────────────────────────────

#[test]
fn test_transfer_before_due_date() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.token.issue(&alice, &1_000);

    // Default timestamp (0) is before due_date (1_900_000_000) — transfer succeeds.
    h.token.transfer(&alice, &bob, &400);
    assert_eq!(h.token.balance(&alice), 600);
    assert_eq!(h.token.balance(&bob), 400);
}

#[test]
fn test_transfer_blocked_after_due_date() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.token.issue(&alice, &1_000);

    h.env.ledger().set_timestamp(1_900_000_001);
    assert!(h.token.try_transfer(&alice, &bob, &500).is_err());
}

#[test]
fn test_transfer_from_blocked_after_due_date() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.token.issue(&alice, &1_000);
    h.token.approve(&alice, &bob, &500, &999_999_999);

    h.env.ledger().set_timestamp(1_900_000_001);
    assert!(h.token.try_transfer_from(&bob, &alice, &bob, &500).is_err());
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
fn test_redeem_blocked_when_compliance_paused() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);
    h.token.issue(&holder, &1_000);
    h.token.settle();
    h.compliance.pause();
    assert!(h.token.try_redeem(&holder, &500).is_err());
}

#[test]
fn test_redeem_blocked_for_blocklisted_holder() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);
    h.token.issue(&holder, &1_000);
    h.token.settle();
    h.compliance.add_to_blocklist(&holder);
    assert!(h.token.try_redeem(&holder, &500).is_err());
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

// ── update_kyc_registry / update_compliance_engine tests ─────────────────────

#[test]
fn test_update_kyc_registry_admin_only() {
    let h = setup();
    let new_kyc = Address::generate(&h.env);

    // Non-admin: separate env, no auths mocked
    {
        let env2 = Env::default();
        let non_admin = Address::generate(&env2);
        let token_id2 = env2.register(
            InvoiceToken,
            (
                non_admin.clone(),
                Address::generate(&env2),
                Address::generate(&env2),
                meta(&env2),
            ),
        );
        let client2 = InvoiceTokenClient::new(&env2, &token_id2);
        assert!(client2.try_update_kyc_registry(&Address::generate(&env2)).is_err());
    }

    // Admin succeeds and the stored address is updated
    h.token.update_kyc_registry(&new_kyc);

    // Confirm the new registry is in effect: issuing to an already-KYC'd
    // address now fails because the new registry has no approvals.
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder); // approved in OLD registry
    assert!(h.token.try_issue(&holder, &1).is_err());
}

#[test]
fn test_update_compliance_engine_admin_only() {
    let h = setup();

    // Non-admin: separate env, no auths mocked
    {
        let env2 = Env::default();
        let non_admin = Address::generate(&env2);
        let token_id2 = env2.register(
            InvoiceToken,
            (
                non_admin.clone(),
                Address::generate(&env2),
                Address::generate(&env2),
                meta(&env2),
            ),
        );
        let client2 = InvoiceTokenClient::new(&env2, &token_id2);
        assert!(client2.try_update_compliance_engine(&Address::generate(&env2)).is_err());
    }

    // Admin can update; subsequent compliance checks use the new engine.
    // Deploy a second paused compliance engine.
    let ce2_id = h.env.register(ComplianceEngine, ());
    let ce2 = ComplianceEngineClient::new(&h.env, &ce2_id);
    let kyc_id = h.env.register(kyc_registry::KycRegistry, ());
    ce2.initialize(&h.admin, &kyc_id);
    ce2.pause();

    h.token.update_compliance_engine(&ce2_id);

    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);
    h.token.issue(&holder, &100); // issue bypasses compliance check (not a transfer)
    h.token.settle();
    // Redemption checks compliance engine for pause/blocklist — must now fail.
    assert!(h.token.try_redeem(&holder, &50).is_err());
}
