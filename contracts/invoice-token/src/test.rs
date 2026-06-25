#![cfg(test)]

use crate::{InvoiceMeta, InvoiceToken, InvoiceTokenClient};
use kyc_registry::{KycRegistry, KycRegistryClient};
use compliance_engine::{ComplianceEngine, ComplianceEngineClient};
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

    let token_id = env.register(InvoiceToken, ());
    let token = InvoiceTokenClient::new(&env, &token_id);
    token.initialize(&admin, &kyc_id, &compliance_id, &meta(&env));

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
fn test_transfer_success() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    
    h.token.issue(&alice, &1000);
    h.token.transfer(&alice, &bob, &400);
    
    assert_eq!(h.token.balance(&alice), 600);
    assert_eq!(h.token.balance(&bob), 400);
}

#[test]
fn test_transfer_kyc_rejection() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    
    h.approve_kyc(&alice);
    h.token.issue(&alice, &1000);
    
    // bob has no KYC
    assert!(h.token.try_transfer(&alice, &bob, &400).is_err());
}

#[test]
fn test_transfer_blocklist_rejection() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    
    h.token.issue(&alice, &1000);
    h.compliance.add_to_blocklist(&bob);
    
    assert!(h.token.try_transfer(&alice, &bob, &400).is_err());
}

#[test]
fn test_transfer_post_settlement_rejection() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    
    h.token.issue(&alice, &1000);
    h.token.settle();
    
    assert!(h.token.try_transfer(&alice, &bob, &400).is_err());
}
