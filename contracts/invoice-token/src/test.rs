#![cfg(test)]

use crate::{InvoiceMeta, InvoiceToken, InvoiceTokenClient};
use compliance_engine::{ComplianceEngine, ComplianceEngineClient, ComplianceRules};
use kyc_registry::{KycRegistry, KycRegistryClient};
use soroban_sdk::{testutils::{Address as _, Ledger as _}, Address, Env, String, Vec};

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

fn inv_id(env: &Env) -> String {
    String::from_str(env, "INV-001")
}

fn meta(env: &Env) -> InvoiceMeta {
    InvoiceMeta {
        invoice_id: String::from_str(env, "INV-001"),
        issuer: String::from_str(env, "Acme Corp"),
        debtor: String::from_str(env, "Globex"),
        face_value_usd: 1_000_000_000_000,
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

    Harness { env, token, kyc, compliance, verifier, admin }
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

    fn make_invoice(&self, id: &str) -> InvoiceMeta {
        InvoiceMeta {
            invoice_id: String::from_str(&self.env, id),
            issuer: String::from_str(&self.env, "Issuer"),
            debtor: String::from_str(&self.env, "Debtor"),
            face_value_usd: 500_000_000_000,
            discount_rate_bps: 100,
            due_date: 1_900_000_000,
            currency: String::from_str(&self.env, "USD"),
            ipfs_doc_hash: String::from_str(&self.env, ""),
            transfer_fee_bps: 0,
            fee_recipient: None,
        }
    }
}

// ── Single-invoice tests (preserved behaviour) ────────────────────────────────

#[test]
fn test_issue_idempotency_holder_count() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    h.token.issue(&inv_id(&h.env), &holder, &1_000);
    assert_eq!(h.compliance.holder_count(), 1);

    h.token.issue(&inv_id(&h.env), &holder, &500);
    assert_eq!(h.compliance.holder_count(), 1);
    assert_eq!(h.token.balance(&holder, &inv_id(&h.env)), 1_500);
}

#[test]
fn test_transfer_before_due_date() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.token.issue(&inv_id(&h.env), &alice, &1_000);

    h.token.transfer(&inv_id(&h.env), &alice, &bob, &400);
    assert_eq!(h.token.balance(&alice, &inv_id(&h.env)), 600);
    assert_eq!(h.token.balance(&bob, &inv_id(&h.env)), 400);
}

#[test]
fn test_transfer_blocked_after_due_date() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.token.issue(&inv_id(&h.env), &alice, &1_000);

    h.env.ledger().set_timestamp(1_900_000_001);
    assert!(h
        .token
        .try_transfer(&inv_id(&h.env), &alice, &bob, &500)
        .is_err());
}

#[test]
fn test_transfer_from_blocked_after_due_date() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.token.issue(&inv_id(&h.env), &alice, &1_000);
    h.token
        .approve(&alice, &bob, &inv_id(&h.env), &500, &999_999_999);

    h.env.ledger().set_timestamp(1_900_000_001);
    assert!(h
        .token
        .try_transfer_from(&bob, &inv_id(&h.env), &alice, &bob, &500)
        .is_err());
}

#[test]
fn test_metadata() {
    let h = setup();
    assert_eq!(h.token.decimals(), 7);
    assert_eq!(h.token.name(), String::from_str(&h.env, "Veritoken Invoice"));
    assert_eq!(
        h.token.get_meta(&inv_id(&h.env)).invoice_id,
        String::from_str(&h.env, "INV-001")
    );
    assert!(!h.token.is_settled(&inv_id(&h.env)));
}

#[test]
fn test_issue_requires_kyc() {
    let h = setup();
    let holder = Address::generate(&h.env);

    assert!(h.token.try_issue(&inv_id(&h.env), &holder, &1_000).is_err());

    h.approve_kyc(&holder);
    h.token.issue(&inv_id(&h.env), &holder, &1_000);
    assert_eq!(h.token.balance(&holder, &inv_id(&h.env)), 1_000);
    assert_eq!(h.token.total_supply(&inv_id(&h.env)), 1_000);
}

#[test]
fn test_settle_then_redeem() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);
    h.token.issue(&inv_id(&h.env), &holder, &1_000);

    assert!(h
        .token
        .try_redeem(&inv_id(&h.env), &holder, &500)
        .is_err());

    h.token.settle(&inv_id(&h.env));
    assert!(h.token.is_settled(&inv_id(&h.env)));

    h.token.redeem(&inv_id(&h.env), &holder, &600);
    assert_eq!(h.token.balance(&holder, &inv_id(&h.env)), 400);
    assert_eq!(h.token.total_supply(&inv_id(&h.env)), 400);
}

#[test]
fn test_cannot_issue_after_settle() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);
    h.token.settle(&inv_id(&h.env));
    assert!(h.token.try_issue(&inv_id(&h.env), &holder, &1).is_err());
}

#[test]
fn test_redeem_insufficient_balance() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);
    h.token.issue(&inv_id(&h.env), &holder, &100);
    h.token.settle(&inv_id(&h.env));
    assert!(h
        .token
        .try_redeem(&inv_id(&h.env), &holder, &101)
        .is_err());
}

#[test]
fn test_redeem_blocked_when_compliance_paused() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);
    h.token.issue(&inv_id(&h.env), &holder, &1_000);
    h.token.settle(&inv_id(&h.env));
    h.compliance.pause();
    assert!(h
        .token
        .try_redeem(&inv_id(&h.env), &holder, &500)
        .is_err());
}

#[test]
fn test_redeem_blocked_for_blocklisted_holder() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);
    h.token.issue(&inv_id(&h.env), &holder, &1_000);
    h.token.settle(&inv_id(&h.env));
    h.compliance.add_to_blocklist(&holder);
    assert!(h
        .token
        .try_redeem(&inv_id(&h.env), &holder, &500)
        .is_err());
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

#[test]
fn test_transfer_blocked_by_holding_period() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    h.compliance.set_rules(&ComplianceRules {
        max_transfer_amount: 0,
        min_holding_period: 3600,
        max_holders: 0,
        require_same_jurisdiction: false,
        paused: false,
    });

    h.token.issue(&inv_id(&h.env), &alice, &1_000);
    assert!(h
        .token
        .try_transfer(&inv_id(&h.env), &alice, &bob, &100)
        .is_err());

    h.env.ledger().set_timestamp(h.env.ledger().timestamp() + 3601);
    h.token.transfer(&inv_id(&h.env), &alice, &bob, &100);
    assert_eq!(h.token.balance(&bob, &inv_id(&h.env)), 100);
    assert_eq!(h.token.balance(&alice, &inv_id(&h.env)), 900);
}

#[test]
fn test_update_kyc_registry_admin_only() {
    let h = setup();
    let new_kyc = Address::generate(&h.env);

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

    h.token.update_kyc_registry(&new_kyc);

    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);
    assert!(h.token.try_issue(&inv_id(&h.env), &holder, &1).is_err());
}

#[test]
fn test_update_compliance_engine_admin_only() {
    let h = setup();

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
        assert!(client2
            .try_update_compliance_engine(&Address::generate(&env2))
            .is_err());
    }

    let ce2_id = h.env.register(ComplianceEngine, ());
    let ce2 = ComplianceEngineClient::new(&h.env, &ce2_id);
    let kyc_id = h.env.register(KycRegistry, ());
    ce2.initialize(&h.admin, &kyc_id);
    ce2.pause();

    h.token.update_compliance_engine(&ce2_id);

    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);
    h.token.issue(&inv_id(&h.env), &holder, &100);
    h.token.settle(&inv_id(&h.env));
    assert!(h
        .token
        .try_redeem(&inv_id(&h.env), &holder, &50)
        .is_err());
}

// ── Multi-invoice tests ───────────────────────────────────────────────────────

#[test]
fn test_multi_invoice_independent_balances() {
    let h = setup();
    let id1 = inv_id(&h.env);
    let id2 = String::from_str(&h.env, "INV-002");

    h.token.create_invoice(&h.make_invoice("INV-002"));

    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    h.token.issue(&id1, &alice, &1_000);
    h.token.issue(&id2, &bob, &500);

    assert_eq!(h.token.balance(&alice, &id1), 1_000);
    assert_eq!(h.token.balance(&alice, &id2), 0);
    assert_eq!(h.token.balance(&bob, &id1), 0);
    assert_eq!(h.token.balance(&bob, &id2), 500);

    assert_eq!(h.token.total_supply(&id1), 1_000);
    assert_eq!(h.token.total_supply(&id2), 500);
}

#[test]
fn test_multi_invoice_independent_settlement() {
    let h = setup();
    let id1 = inv_id(&h.env);
    let id2 = String::from_str(&h.env, "INV-002");

    h.token.create_invoice(&h.make_invoice("INV-002"));

    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    h.token.issue(&id1, &alice, &1_000);
    h.token.issue(&id2, &bob, &500);

    // Settle only invoice 1
    h.token.settle(&id1);
    assert!(h.token.is_settled(&id1));
    assert!(!h.token.is_settled(&id2));

    // Can redeem from invoice 1
    h.token.redeem(&id1, &alice, &500);
    assert_eq!(h.token.balance(&alice, &id1), 500);

    // Cannot redeem from invoice 2 (not settled)
    assert!(h.token.try_redeem(&id2, &bob, &100).is_err());
}

#[test]
fn test_list_invoices_pagination() {
    let h = setup();

    // Constructor created INV-001
    let ids = h.token.list_invoices(&0, &10);
    assert_eq!(ids.len(), 1);

    h.token.create_invoice(&h.make_invoice("INV-002"));
    h.token.create_invoice(&h.make_invoice("INV-003"));
    h.token.create_invoice(&h.make_invoice("INV-004"));

    let all = h.token.list_invoices(&0, &10);
    assert_eq!(all.len(), 4);

    // First page: 2 items
    let page1 = h.token.list_invoices(&0, &2);
    assert_eq!(page1.len(), 2);

    // Second page: remaining 2 items
    let page2 = h.token.list_invoices(&2, &2);
    assert_eq!(page2.len(), 2);

    // Start beyond end: empty
    let empty = h.token.list_invoices(&10, &5);
    assert_eq!(empty.len(), 0);
}

#[test]
fn test_create_invoice_admin_only() {
    let h = setup();
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
    assert!(client2.try_create_invoice(&meta(&env2)).is_err());
}

#[test]
fn test_duplicate_invoice_id_rejected() {
    let h = setup();
    // INV-001 was already created by the constructor
    assert!(h.token.try_create_invoice(&meta(&h.env)).is_err());
}

#[test]
fn test_multi_invoice_transfer_only_within_invoice() {
    let h = setup();
    let id1 = inv_id(&h.env);
    let id2 = String::from_str(&h.env, "INV-002");

    h.token.create_invoice(&h.make_invoice("INV-002"));

    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    h.token.issue(&id1, &alice, &1_000);
    h.token.issue(&id2, &alice, &500);

    // Transfer on id1 doesn't affect id2 balance
    h.token.transfer(&id1, &alice, &bob, &300);
    assert_eq!(h.token.balance(&alice, &id1), 700);
    assert_eq!(h.token.balance(&alice, &id2), 500);
    assert_eq!(h.token.balance(&bob, &id1), 300);
    assert_eq!(h.token.balance(&bob, &id2), 0);
}

#[test]
fn test_version_returns_nonempty() {
    let h = setup();
    let v = h.token.version();
    assert!(v.len() > 0);
}

#[test]
fn test_partial_settle_proportional_redemption() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    // Issue 100 tokens against a 1,000,000,000,000-stroop face value
    let face = 1_000_000_000_000i128;
    let issued = 100i128;
    h.token.issue(&holder, &issued);

    // Partial settle for 60% of face value
    let settlement = face * 60 / 100;
    h.token.partial_settle(&settlement);

    assert_eq!(h.token.settlement_amount(), settlement);
    assert!(h.token.is_settled());

    // Holder can redeem up to issued * settlement / face = 60 tokens
    let max_redeemable = issued * settlement / face;
    h.token.redeem(&holder, &max_redeemable);
}

#[test]
fn test_partial_settle_blocks_over_proportional_redeem() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    let face = 1_000_000_000_000i128;
    h.token.issue(&holder, &100);
    h.token.partial_settle(&(face * 50 / 100));

    // Trying to redeem more than 50 (the proportional share) should fail
    assert!(h.token.try_redeem(&holder, &51).is_err());
}

#[test]
fn test_settle_sets_full_face_value() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    h.token.issue(&holder, &100);
    h.token.settle();

    let face = 1_000_000_000_000i128;
    assert_eq!(h.token.settlement_amount(), face);
    // Full settlement: holder can redeem all tokens
    h.token.redeem(&holder, &100);
}

#[test]
fn test_partial_settle_rejects_zero() {
    let h = setup();
    assert!(h.token.try_partial_settle(&0).is_err());
}

#[test]
fn test_partial_settle_rejects_excess() {
    let h = setup();
    let face = 1_000_000_000_000i128;
    assert!(h.token.try_partial_settle(&(face + 1)).is_err());
}
