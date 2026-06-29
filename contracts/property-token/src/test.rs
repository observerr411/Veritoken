#![cfg(test)]

use crate::{PropertyMeta, PropertyToken, PropertyTokenClient};
use compliance_engine::{ComplianceEngine, ComplianceEngineClient, ComplianceRules};
use kyc_registry::{KycRegistry, KycRegistryClient};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Env, IntoVal, String,
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
    compliance.initialize(&admin, &kyc_id);

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
    h.token.deposit_dividend(&1_000, &2);
    assert_eq!(h.token.pending_dividend(&alice), 100);

    let claimed = h.token.claim_dividend(&alice);
    assert_eq!(claimed, 100);
    assert_eq!(h.token.pending_dividend(&alice), 0);

    // Claiming again yields nothing.
    assert_eq!(h.token.claim_dividend(&alice), 0);
}

#[test]
fn test_multi_round_dividend_with_partial_transfer() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    h.token.mint(&alice, &1_000);

    // First dividend round: Alice owns all 1000 shares.
    h.token.deposit_dividend(&1_000, &2);

    // Transfer 400 shares to Bob after the first round.
    h.token.transfer(&alice, &bob, &400);

    // Second dividend round: Alice owns 600, Bob owns 400.
    h.token.deposit_dividend(&1_000, &2);

    let alice_claimed = h.token.claim_dividend(&alice);
    let bob_claimed = h.token.claim_dividend(&bob);

    assert_eq!(alice_claimed, 1_600);
    assert_eq!(bob_claimed, 400);
}

#[test]
fn test_deposit_dividend_requires_shares() {
    let h = setup();
    // total_shares is 1000 from meta, so deposit works even before mint.
    h.token.deposit_dividend(&1_000, &2);
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
fn test_transfer_blocked_by_holding_period() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    // Configure a one-hour minimum holding period on the real compliance engine.
    h.compliance.set_rules(&ComplianceRules {
        max_transfer_amount: 0,
        min_holding_period: 3600,
        max_holders: 0,
        require_same_jurisdiction: false,
        paused: false,
    });

    // Minting registers alice as a holder at the current ledger timestamp.
    h.token.mint(&alice, &100);

    // A transfer immediately after minting is blocked: the holding period has
    // not elapsed.
    assert!(h.token.try_transfer(&alice, &bob, &10).is_err());

    // Advance past the holding period; the transfer now succeeds.
    h.env
        .ledger()
        .set_timestamp(h.env.ledger().timestamp() + 3601);
    h.token.transfer(&alice, &bob, &10);
    assert_eq!(h.token.balance(&bob), 10);
    assert_eq!(h.token.balance(&alice), 90);
}

#[test]
fn test_transfer_snapshots_dividends() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.token.mint(&alice, &100);

    h.token.deposit_dividend(&1_000, &2); // 1 per share
                                      // Alice accrued 100 before transferring all her shares.
    h.token.transfer(&alice, &bob, &100);

    // Bob just received shares; he must not inherit Alice's accrued dividend.
    assert_eq!(h.token.pending_dividend(&bob), 0);
    // Alice keeps the 100 she accrued while she held the shares.
    assert_eq!(h.token.pending_dividend(&alice), 100);
    assert_eq!(h.token.claim_dividend(&alice), 100);

    // A dividend declared after the transfer accrues to Bob, not Alice.
    h.token.deposit_dividend(&1_000, &2);
    assert_eq!(h.token.pending_dividend(&bob), 100);
    assert_eq!(h.token.pending_dividend(&alice), 0);
}

// ── Holder list tests ─────────────────────────────────────────────────────────

#[test]
fn test_holder_list_updated_on_mint() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    assert_eq!(h.token.holder_count(), 0);

    h.token.mint(&alice, &100);
    assert_eq!(h.token.holder_count(), 1);
    assert_eq!(h.token.get_holders(&0, &50).len(), 1);

    h.token.mint(&bob, &50);
    assert_eq!(h.token.holder_count(), 2);

    // Minting again to alice is idempotent — count stays at 2.
    h.token.mint(&alice, &10);
    assert_eq!(h.token.holder_count(), 2);
}

#[test]
fn test_holder_removed_when_balance_hits_zero() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    h.token.mint(&alice, &100);
    assert_eq!(h.token.holder_count(), 1);

    // Transfer entire balance — alice drops to 0, bob is added.
    h.token.transfer(&alice, &bob, &100);
    assert_eq!(h.token.holder_count(), 1);
    let holders = h.token.get_holders(&0, &50);
    assert_eq!(holders.len(), 1);
    assert_eq!(holders.get(0).unwrap(), bob);
}

#[test]
fn test_get_holders_pagination() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let carol = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.approve_kyc(&carol);

    h.token.mint(&alice, &10);
    h.token.mint(&bob, &10);
    h.token.mint(&carol, &10);
    assert_eq!(h.token.holder_count(), 3);

    // Page size 2 starting from 0.
    assert_eq!(h.token.get_holders(&0, &2).len(), 2);
    // Page size 2 starting from 2 — only 1 remaining.
    assert_eq!(h.token.get_holders(&2, &2).len(), 1);
    // Out of range start returns empty.
    assert_eq!(h.token.get_holders(&10, &2).len(), 0);
}

#[test]
fn test_transfer_rejects_recipient_below_required_tier() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    // alice has tier 1 (meets requirement), bob has tier 0 (below requirement)
    h.approve_kyc_with_tier(&alice, 1);
    h.approve_kyc_with_tier(&bob, 0);
    h.token.mint(&alice, &100);
    // Transfer to bob must fail because his tier (0) is below kyc_tier_required (1)
    assert!(h.token.try_transfer(&alice, &bob, &50).is_err());
}

// ── transfer_from tier enforcement tests (#254) ───────────────────────────────

#[test]
fn test_transfer_from_rejects_recipient_below_required_tier() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let spender = Address::generate(&h.env);
    // alice has tier 1, bob has tier 0 (below kyc_tier_required=1)
    h.approve_kyc_with_tier(&alice, 1);
    h.approve_kyc_with_tier(&bob, 0);
    h.approve_kyc_with_tier(&spender, 1);
    h.token.mint(&alice, &100);
    h.token.approve(&alice, &spender, &50, &(h.env.ledger().sequence() + 100));
    assert!(h.token.try_transfer_from(&spender, &alice, &bob, &50).is_err());
}

#[test]
fn test_transfer_from_accepts_recipient_with_sufficient_tier() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let spender = Address::generate(&h.env);
    h.approve_kyc_with_tier(&alice, 1);
    h.approve_kyc_with_tier(&bob, 1);
    h.approve_kyc_with_tier(&spender, 1);
    h.token.mint(&alice, &100);
    h.token.approve(&alice, &spender, &50, &(h.env.ledger().sequence() + 100));
    h.token.transfer_from(&spender, &alice, &bob, &50);
    assert_eq!(h.token.balance(&bob), 50);
}

// ── property_type validation tests (#256) ─────────────────────────────────────

#[test]
#[should_panic]
fn test_invalid_property_type_panics_in_constructor() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let kyc_id = Address::generate(&env);
    let ce_id = Address::generate(&env);
    let mut bad_meta = meta(&env);
    bad_meta.property_type = String::from_str(&env, "warehouse");
    env.register(PropertyToken, (admin, kyc_id, ce_id, bad_meta));
}

#[test]
fn test_valid_property_types_accepted_in_constructor() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    for pt in ["residential", "commercial", "land"] {
        let kyc_id = env.register(KycRegistry, ());
        let kyc = KycRegistryClient::new(&env, &kyc_id);
        kyc.initialize(&admin);
        let compliance_id = env.register(ComplianceEngine, ());
        let compliance = ComplianceEngineClient::new(&env, &compliance_id);
        compliance.initialize(&admin, &kyc_id);
        let mut m = meta(&env);
        m.property_type = String::from_str(&env, pt);
        let token_id = env.register(PropertyToken, (admin.clone(), kyc_id, compliance_id, m));
        let token = PropertyTokenClient::new(&env, &token_id);
        assert_eq!(token.get_meta().property_type, String::from_str(&env, pt));
    }
}

#[test]
fn test_invalid_property_type_panics_in_update_meta() {
    let h = setup();
    let mut bad_meta = h.token.get_meta();
    bad_meta.property_type = String::from_str(&h.env, "warehouse");
    assert!(h.token.try_update_meta(&bad_meta).is_err());
}

#[test]
fn test_valid_property_type_accepted_in_update_meta() {
    let h = setup();
    let mut new_meta = h.token.get_meta();
    new_meta.property_type = String::from_str(&h.env, "commercial");
    h.token.update_meta(&new_meta);
    assert_eq!(h.token.get_meta().property_type, String::from_str(&h.env, "commercial"));
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
    let dummy_kyc = h.env.register(kyc_registry::KycRegistry, ());
    ce2.initialize(&h.admin, &dummy_kyc);
    ce2.pause();

    // Admin can update
    h.token.update_compliance_engine(&ce2_id);

    // Mints through the paused engine are now blocked
    let alice = Address::generate(&h.env);
    h.approve_kyc(&alice);
    assert!(h.token.try_mint(&alice, &10).is_err());
}

// ── Buyback tests ─────────────────────────────────────────────────────────────

#[test]
fn test_buyback_successful() {
    let h = setup();
    let alice = Address::generate(&h.env);
    h.approve_kyc(&alice);

    // Mint 100 shares to alice
    h.token.mint(&alice, &100);
    assert_eq!(h.token.balance(&alice), 100);
    assert_eq!(h.token.total_shares(), 1_000);

    // Deposit dividend before buyback
    h.token.deposit_dividend(&1_000, &2); // 1 per share
    assert_eq!(h.token.pending_dividend(&alice), 100);

    // Admin buys back 50 shares
    h.token.buyback(&alice, &50);

    // Balance and total shares decreased
    assert_eq!(h.token.balance(&alice), 50);
    assert_eq!(h.token.total_shares(), 950);

    // Alice still has her accrued dividend from before buyback
    assert_eq!(h.token.pending_dividend(&alice), 100);

    // She can still claim it
    let claimed = h.token.claim_dividend(&alice);
    assert_eq!(claimed, 100);
}

#[test]
fn test_buyback_insufficient_shares() {
    let h = setup();
    let alice = Address::generate(&h.env);
    h.approve_kyc(&alice);

    h.token.mint(&alice, &50);
    // Try to buy back 51 shares — should fail
    assert!(h.token.try_buyback(&alice, &51).is_err());
}

#[test]
fn test_buyback_removes_holder_on_zero_balance() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    h.token.mint(&alice, &50);
    h.token.mint(&bob, &30);
    assert_eq!(h.token.holder_count(), 2);

    // Buy back all of alice's shares
    h.token.buyback(&alice, &50);

    // Alice is removed from holder list
    assert_eq!(h.token.holder_count(), 1);
    let holders = h.token.get_holders(&0, &50);
    assert_eq!(holders.len(), 1);
    assert_eq!(holders.get(0).unwrap(), bob);
}

#[test]
fn test_buyback_non_admin_rejected() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let attacker = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&attacker);

    h.token.mint(&alice, &100);

    // Attacker cannot buyback
    let env2 = Env::default();
    let non_admin = Address::generate(&env2);
    let token_id2 = env2.register(
        PropertyToken,
        (
            Address::generate(&env2),
            Address::generate(&env2),
            Address::generate(&env2),
            meta(&env2),
        ),
    );
    let client2 = PropertyTokenClient::new(&env2, &token_id2);

    // Should fail because non_admin is not the admin
    assert!(client2.try_buyback(&non_admin, &50).is_err());
}

#[test]
fn test_buyback_rejects_kyc_unapproved_holder() {
    let h = setup();
    let alice = Address::generate(&h.env);
    h.approve_kyc(&alice);

    h.token.mint(&alice, &100);

    // Revoke alice's KYC by spinning up a new registry with no approvals
    let new_kyc_id = h.env.register(KycRegistry, ());
    let new_kyc = KycRegistryClient::new(&h.env, &new_kyc_id);
    new_kyc.initialize(&h.admin);
    h.token.update_kyc_registry(&new_kyc_id);

    // Buyback should now fail — alice no longer has active KYC
    assert!(h.token.try_buyback(&alice, &50).is_err());
}

#[test]
fn test_version_returns_nonempty() {
    let h = setup();
    let v = h.token.version();
    assert!(v.len() > 0);
}

#[test]
fn test_dividend_history_records_deposits() {
    let h = setup();
    let alice = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.token.mint(&alice, &500);

    assert_eq!(h.token.dividend_deposit_count(), 0);

    h.token.deposit_dividend(&1_000, &2);
    h.token.deposit_dividend(&2_000, &2);

    assert_eq!(h.token.dividend_deposit_count(), 2);

    let history = h.token.get_dividend_history(&0, &10);
    assert_eq!(history.len(), 2);

    let first = history.get(0).unwrap();
    assert_eq!(first.amount, 1_000);

    let second = history.get(1).unwrap();
    assert_eq!(second.amount, 2_000);
}

#[test]
fn test_dividend_history_running_total_dps() {
    let h = setup();
    let alice = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.token.mint(&alice, &1_000);

    h.token.deposit_dividend(&1_000, &2);
    h.token.deposit_dividend(&2_000, &2);

    let history = h.token.get_dividend_history(&0, &10);
    assert_eq!(history.get(0).unwrap().running_total_dps, 1);
    assert_eq!(history.get(1).unwrap().running_total_dps, 3);
}

#[test]
fn test_dividend_history_pagination() {
    let h = setup();
    let alice = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.token.mint(&alice, &1_000);

    for _ in 0..5 {
        h.token.deposit_dividend(&100, &2);
    }

    let page = h.token.get_dividend_history(&2, &2);
    assert_eq!(page.len(), 2);
    assert_eq!(page.get(0).unwrap().amount, 100);
}

#[test]
fn test_dividend_history_empty_before_deposit() {
    let h = setup();
    let history = h.token.get_dividend_history(&0, &10);
    assert_eq!(history.len(), 0);
    assert_eq!(h.token.dividend_deposit_count(), 0);
}

// ── #278 Rent yield tracking tests ───────────────────────────────────────────

#[test]
fn test_claim_rent_yield_only_claims_rent() {
    let h = setup();
    let alice = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.token.mint(&alice, &1_000);

    // Deposit 1000 as Rent (type 0) and 2000 as Capital (type 1)
    h.token.deposit_dividend(&1_000, &0); // rent: 1 per share
    h.token.deposit_dividend(&2_000, &1); // capital: 2 per share

    let rent = h.token.claim_rent_yield(&alice);
    assert_eq!(rent, 1_000); // 1 per share * 1000 shares

    // Capital untouched
    let capital = h.token.claim_capital_return(&alice);
    assert_eq!(capital, 2_000); // 2 per share * 1000 shares
}

#[test]
fn test_claim_capital_return_only_claims_capital() {
    let h = setup();
    let alice = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.token.mint(&alice, &500);

    // total_shares from meta = 1000, so DPS = amount / 1000
    // deposit 1000 rent => DPS_rent = 1; deposit 2000 capital => DPS_cap = 2
    h.token.deposit_dividend(&1_000, &0); // rent DPS = 1
    h.token.deposit_dividend(&2_000, &1); // capital DPS = 2

    let capital = h.token.claim_capital_return(&alice);
    assert_eq!(capital, 1_000); // DPS=2 * 500 shares

    // Rent untouched
    let rent = h.token.claim_rent_yield(&alice);
    assert_eq!(rent, 500); // DPS=1 * 500 shares
}

#[test]
fn test_claim_dividend_claims_all_types() {
    let h = setup();
    let alice = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.token.mint(&alice, &100);

    // total_shares = 1000, so need multiples of 1000 for integer DPS > 0
    // deposit 1000 => DPS=1; 2000 => DPS=2; 3000 => DPS=3
    h.token.deposit_dividend(&1_000, &0); // rent DPS = 1
    h.token.deposit_dividend(&2_000, &1); // capital DPS = 2
    h.token.deposit_dividend(&3_000, &2); // other DPS = 3

    let total = h.token.claim_dividend(&alice);
    // total DPS = 6, 100 shares => 600
    assert_eq!(total, 600);
}

#[test]
fn test_mixed_deposits_independent_claiming() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    h.token.mint(&alice, &600);
    h.token.mint(&bob, &400);

    // Round 1: rent
    h.token.deposit_dividend(&1_000, &0);
    // Round 2: capital
    h.token.deposit_dividend(&1_000, &1);

    // Alice: 60% of rent + 60% of capital
    assert_eq!(h.token.claim_rent_yield(&alice), 600);
    assert_eq!(h.token.claim_capital_return(&alice), 600);

    // Bob: 40% of each
    assert_eq!(h.token.claim_rent_yield(&bob), 400);
    assert_eq!(h.token.claim_capital_return(&bob), 400);
}

#[test]
fn test_second_claim_yields_nothing() {
    let h = setup();
    let alice = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.token.mint(&alice, &100);

    h.token.deposit_dividend(&1_000, &0);

    h.token.claim_rent_yield(&alice);
    assert_eq!(h.token.claim_rent_yield(&alice), 0);

    h.token.deposit_dividend(&1_000, &1);
    h.token.claim_capital_return(&alice);
    assert_eq!(h.token.claim_capital_return(&alice), 0);
}

