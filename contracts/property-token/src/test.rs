#![cfg(test)]

use crate::{PropertyMeta, PropertyToken, PropertyTokenClient};
use compliance_engine::{ComplianceEngine, ComplianceEngineClient};
use kyc_registry::{KycRegistry, KycRegistryClient};
use soroban_sdk::{testutils::{Address as _, Ledger as _}, Address, Env, String};

struct Harness {
    env: Env,
    token: PropertyTokenClient<'static>,
    kyc: KycRegistryClient<'static>,
    compliance: ComplianceEngineClient<'static>,
    verifier: Address,
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

// ── Allowance / transfer_from tests ─────────────────────────────────────────

#[test]
fn test_approve_and_transfer_from() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let spender = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.token.mint(&alice, &100);

    // Alice approves spender for 60 shares, expiring far in the future.
    h.token.approve(&alice, &spender, &60, &1_000_000);
    assert_eq!(h.token.allowance(&alice, &spender), 60);

    // Spender moves 40 shares from Alice to Bob.
    h.token.transfer_from(&spender, &alice, &bob, &40);
    assert_eq!(h.token.balance(&alice), 60);
    assert_eq!(h.token.balance(&bob), 40);
    // Remaining allowance is 20.
    assert_eq!(h.token.allowance(&alice, &spender), 20);
}

#[test]
fn test_allowance_expiry() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let spender = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.token.mint(&alice, &100);

    // Approve with expiration at current ledger + 5.
    let current = h.env.ledger().sequence();
    h.token.approve(&alice, &spender, &50, &(current + 5));
    assert_eq!(h.token.allowance(&alice, &spender), 50);

    // Advance the ledger past the expiration.
    h.env.ledger().set_sequence_number(current + 10);

    // Allowance should now be 0 (expired).
    assert_eq!(h.token.allowance(&alice, &spender), 0);
    // transfer_from must fail since allowance is expired.
    assert!(h.token.try_transfer_from(&spender, &alice, &bob, &10).is_err());
}

#[test]
fn test_transfer_from_insufficient_allowance() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let spender = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.token.mint(&alice, &100);

    h.token.approve(&alice, &spender, &10, &1_000_000);
    // Attempt to transfer more than the approved amount.
    assert!(h.token.try_transfer_from(&spender, &alice, &bob, &20).is_err());
}

#[test]
fn test_transfer_from_snapshots_dividends() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let spender = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.token.mint(&alice, &100);

    // Deposit 1000 stroops → 1 per share. Alice accrues 100.
    h.token.deposit_dividend(&1_000);
    h.token.approve(&alice, &spender, &100, &1_000_000);

    // Spender moves all of Alice's shares to Bob.
    h.token.transfer_from(&spender, &alice, &bob, &100);

    // Bob must not inherit Alice's pre-transfer accrued dividend.
    assert_eq!(h.token.pending_dividend(&bob), 0);
    // Alice keeps her 100 stroop accrual.
    assert_eq!(h.token.pending_dividend(&alice), 100);

    // A dividend declared now accrues to Bob (current holder), not Alice.
    h.token.deposit_dividend(&1_000);
    assert_eq!(h.token.pending_dividend(&bob), 100);
    assert_eq!(h.token.pending_dividend(&alice), 100); // unchanged
}
