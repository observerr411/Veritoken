#![cfg(test)]

use crate::{PropertyMeta, PropertyToken, PropertyTokenClient};
use compliance_engine::{ComplianceEngine, ComplianceEngineClient};
use kyc_registry::{KycRegistry, KycRegistryClient};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Env, String,
};

struct Harness {
    env: Env,
    token: PropertyTokenClient<'static>,
    kyc: KycRegistryClient<'static>,
    compliance: ComplianceEngineClient<'static>,
    verifier: Address,
    admin: Address,
}

fn meta(env: &Env) -> PropertyMeta {
    PropertyMeta {
        property_id: String::from_str(env, "PROP-1"),
        legal_name: String::from_str(env, "123 Main St LLC"),
        jurisdiction: String::from_str(env, "US-NY"),
        address: String::from_str(env, "123 Main St"),
        total_valuation_usd: 10_000_000_000_000, // 1,000,000 USD at 7 decimals
        total_shares: 1_000,
        property_type: String::from_str(env, "residential"),
        ipfs_title_hash: String::from_str(env, "Qm..."),
        kyc_tier_required: 1,
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

    // Property token — constructor args passed atomically at register time
    let token_id = env.register(
        PropertyToken,
        (
            admin.clone(),
            kyc_id.clone(),
            compliance_id.clone(),
            meta(&env),
        ),
    );
    let token = PropertyTokenClient::new(&env, &token_id);

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
        self.approve_kyc_with_tier(addr, 1);
    }

    fn approve_kyc_with_tier(&self, addr: &Address, tier: u32) {
        self.kyc.approve(
            &self.verifier,
            addr,
            &tier,
            &0,
            &String::from_str(&self.env, "US"),
        );
    }
}

#[test]
fn test_metadata() {
    let h = setup();
    assert_eq!(h.token.decimals(), 0);
    assert_eq!(h.token.total_shares(), 1_000);
    assert_eq!(
        h.token.get_meta().property_id,
        String::from_str(&h.env, "PROP-1")
    );
}

#[test]
fn test_mint_and_transfer() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    assert!(h.token.try_mint(&Address::generate(&h.env), &10).is_err());

    h.token.mint(&alice, &100);
    assert_eq!(h.token.balance(&alice), 100);

    h.token.transfer(&alice, &bob, &40);
    assert_eq!(h.token.balance(&alice), 60);
    assert_eq!(h.token.balance(&bob), 40);
}

#[test]
fn test_mint_rejects_recipient_below_required_tier() {
    let h = setup();
    let alice = Address::generate(&h.env);
    h.approve_kyc_with_tier(&alice, 0);

    assert!(h.token.try_mint(&alice, &100).is_err());
}

#[test]
fn test_mint_rejects_blocklisted_recipient() {
    let h = setup();
    let alice = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.compliance.add_to_blocklist(&alice);

    assert!(h.token.try_mint(&alice, &100).is_err());
}

#[test]
fn test_mint_rejects_when_compliance_paused() {
    let h = setup();
    let alice = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.compliance.pause();

    assert!(h.token.try_mint(&alice, &100).is_err());
}

#[test]
fn test_transfer_insufficient_shares() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.token.mint(&alice, &10);
    assert!(h.token.try_transfer(&alice, &bob, &11).is_err());
}

#[test]
fn test_dividend_distribution() {
    let h = setup();
    let alice = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.token.mint(&alice, &100);

    // Deposit 1000 stroops over 1000 total shares => 1 per share.
    h.token.deposit_dividend(&1_000);
    assert_eq!(h.token.pending_dividend(&alice), 100);

    let claimed = h.token.claim_dividend(&alice);
    assert_eq!(claimed, 100);
    assert_eq!(h.token.pending_dividend(&alice), 0);

    // Claiming again yields nothing.
    assert_eq!(h.token.claim_dividend(&alice), 0);
}

#[test]
fn test_deposit_dividend_requires_shares() {
    let h = setup();
    // total_shares is 1000 from meta, so deposit works even before mint.
    h.token.deposit_dividend(&1_000);
    let alice = Address::generate(&h.env);
    assert_eq!(h.token.pending_dividend(&alice), 0);
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
fn test_transfer_snapshots_dividends() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.token.mint(&alice, &100);

    h.token.deposit_dividend(&1_000); // 1 per share
                                      // Alice accrued 100 before transferring all her shares.
    h.token.transfer(&alice, &bob, &100);

    // Bob just received shares; he must not inherit Alice's accrued dividend.
    assert_eq!(h.token.pending_dividend(&bob), 0);
    // Alice keeps the 100 she accrued while she held the shares.
    assert_eq!(h.token.pending_dividend(&alice), 100);
    assert_eq!(h.token.claim_dividend(&alice), 100);

    // A dividend declared after the transfer accrues to Bob, not Alice.
    h.token.deposit_dividend(&1_000);
    assert_eq!(h.token.pending_dividend(&bob), 100);
    assert_eq!(h.token.pending_dividend(&alice), 0);
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
            PropertyToken,
            (
                non_admin.clone(),
                Address::generate(&env2),
                Address::generate(&env2),
                meta(&env2),
            ),
        );
        let client2 = PropertyTokenClient::new(&env2, &token_id2);
        assert!(client2.try_update_kyc_registry(&Address::generate(&env2)).is_err());
    }

    // Admin succeeds
    h.token.update_kyc_registry(&new_kyc);

    // Minting now fails because the new registry has no approvals
    let alice = Address::generate(&h.env);
    h.approve_kyc(&alice); // approved in OLD registry only
    assert!(h.token.try_mint(&alice, &10).is_err());
}

#[test]
fn test_update_compliance_engine_admin_only() {
    let h = setup();

    // Non-admin: separate env, no auths mocked
    {
        let env2 = Env::default();
        let non_admin = Address::generate(&env2);
        let token_id2 = env2.register(
            PropertyToken,
            (
                non_admin.clone(),
                Address::generate(&env2),
                Address::generate(&env2),
                meta(&env2),
            ),
        );
        let client2 = PropertyTokenClient::new(&env2, &token_id2);
        assert!(client2.try_update_compliance_engine(&Address::generate(&env2)).is_err());
    }

    // Deploy a second compliance engine and pause it
    let ce2_id = h.env.register(ComplianceEngine, ());
    let ce2 = ComplianceEngineClient::new(&h.env, &ce2_id);
    ce2.initialize(&h.admin);
    ce2.pause();

    // Admin can update
    h.token.update_compliance_engine(&ce2_id);

    // Mints through the paused engine are now blocked
    let alice = Address::generate(&h.env);
    h.approve_kyc(&alice);
    assert!(h.token.try_mint(&alice, &10).is_err());
}
