#![cfg(test)]

use crate::{KycRegistry, KycRegistryClient};
use soroban_sdk::{
    testutils::{storage::Instance, Address as _, Ledger},
    Address, Env, String,
};


fn setup() -> (Env, KycRegistryClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let contract_id = env.register(KycRegistry, ());
    let client = KycRegistryClient::new(&env, &contract_id);
    client.initialize(&admin);
    (env, client, admin)
}

#[test]
fn test_add_verifier_and_approve() {
    let (env, client, _admin) = setup();
    let verifier = Address::generate(&env);
    let subject = Address::generate(&env);

    client.add_verifier(&verifier);
    assert!(!client.is_approved(&subject));

    client.approve(&verifier, &subject, &1, &0, &String::from_str(&env, "US"));
    assert!(client.is_approved(&subject));
    assert_eq!(client.get_tier(&subject), 1);

}

#[test]
fn test_double_initialize_panics() {
    let (_env, client, admin) = setup();
    let res = client.try_initialize(&admin);
    assert!(res.is_err());
}

#[test]
fn test_unauthorized_verifier_cannot_approve() {
    let (env, client, _admin) = setup();
    let rogue = Address::generate(&env);
    let subject = Address::generate(&env);
    // rogue was never added as a verifier — must return an error
    let res = client.try_approve(&rogue, &subject, &0, &0, &String::from_str(&env, "US"));
    assert!(res.is_err());
}

#[test]
fn test_expiry_makes_approval_inactive() {
    let (env, client, _admin) = setup();
    let verifier = Address::generate(&env);
    let subject = Address::generate(&env);
    client.add_verifier(&verifier);

    env.ledger().set_timestamp(1_000);
    client.approve(
        &verifier,
        &subject,
        &0,
        &2_000, // expires at ts 2000
        &String::from_str(&env, "US"),
    );
    assert!(client.is_approved(&subject));

    // Advance past expiry
    env.ledger().set_timestamp(3_000);
    assert!(!client.is_approved(&subject));
}

#[test]
fn test_revoke_and_reject() {
    let (env, client, _admin) = setup();
    let verifier = Address::generate(&env);
    let subject = Address::generate(&env);
    client.add_verifier(&verifier);
    client.approve(&verifier, &subject, &0, &0, &String::from_str(&env, "US"));
    assert!(client.is_approved(&subject));

    client.revoke(&verifier, &subject);
    assert!(!client.is_approved(&subject));

    // Re-approve then reject
    client.approve(&verifier, &subject, &0, &0, &String::from_str(&env, "US"));
    assert!(client.is_approved(&subject));
    client.reject(&verifier, &subject);
    assert!(!client.is_approved(&subject));

    let record = client.get_record(&subject);
    assert!(matches!(record.status, crate::KycStatus::Rejected));
    assert_eq!(record.verifier, verifier);
    assert_eq!(record.tier, 0);
    assert_eq!(record.expiry, 0);
    assert_eq!(record.jurisdiction, String::from_str(&env, "US"));
}

#[test]
fn test_reject_without_existing_record_creates_terminal_record() {
    let (env, client, _admin) = setup();
    let verifier = Address::generate(&env);
    let subject = Address::generate(&env);
    client.add_verifier(&verifier);

    client.reject(&verifier, &subject);

    assert!(!client.is_approved(&subject));
    let record = client.get_record(&subject);
    assert!(matches!(record.status, crate::KycStatus::Rejected));
    assert_eq!(record.verifier, verifier);
    assert_eq!(record.tier, 0);
    assert_eq!(record.expiry, 0);
    assert_eq!(record.jurisdiction, String::from_str(&env, ""));
}

#[test]
fn test_revoke_without_existing_record_creates_terminal_record() {
    let (env, client, _admin) = setup();
    let verifier = Address::generate(&env);
    let subject = Address::generate(&env);
    client.add_verifier(&verifier);

    client.revoke(&verifier, &subject);

    assert!(!client.is_approved(&subject));
    let record = client.get_record(&subject);
    assert!(matches!(record.status, crate::KycStatus::Revoked));
    assert_eq!(record.verifier, verifier);
    assert_eq!(record.tier, 0);
    assert_eq!(record.expiry, 0);
    assert_eq!(record.jurisdiction, String::from_str(&env, ""));
}

#[test]
fn test_remove_verifier() {
    let (env, client, _admin) = setup();
    let verifier = Address::generate(&env);
    client.add_verifier(&verifier);
    client.remove_verifier(&verifier);

    let subject = Address::generate(&env);
    let res = client.try_approve(&verifier, &subject, &0, &0, &String::from_str(&env, "US"));
    assert!(res.is_err());
}

#[test]
fn test_instance_ttl_bump() {
    let (env, client, _admin) = setup();
    let contract_id = client.address.clone();

    // The constants defined in lib.rs:
    // const DAY_IN_LEDGERS: u32 = 17280;
    // const BUMP: u32 = 30 * DAY_IN_LEDGERS; // 518400 ledgers
    let bump = 30 * 17280;

    // Initially, calling add_verifier will bump the TTL to bump (518400 ledgers)
    let verifier = Address::generate(&env);
    client.add_verifier(&verifier);

    let initial_ttl = env.as_contract(&contract_id, || {
        env.storage().instance().get_ttl()
    });
    assert_eq!(initial_ttl, bump);

    // Advance the ledger sequence to decrease TTL below THRESHOLD
    // Let's advance by 30,000 ledgers.
    env.ledger().with_mut(|l| {
        l.sequence_number += 30_000;
    });

    let reduced_ttl = env.as_contract(&contract_id, || {
        env.storage().instance().get_ttl()
    });
    assert_eq!(reduced_ttl, initial_ttl - 30_000);

    // Calling a query function like is_approved should bump it back to BUMP
    let subject = Address::generate(&env);
    client.is_approved(&subject);

    let bumped_ttl = env.as_contract(&contract_id, || {
        env.storage().instance().get_ttl()
    });
    assert_eq!(bumped_ttl, bump);
}

#[test]
fn test_two_step_admin_transfer() {
    let (env, client, _admin) = setup();
    let new_admin = Address::generate(&env);

    client.propose_admin(&new_admin);
    client.accept_admin();

    // Verify new admin is set by calling add_verifier (admin-only)
    let verifier = Address::generate(&env);
    client.add_verifier(&verifier);
}

#[test]
fn test_accept_admin_fails_when_no_pending() {
    let (_env, client, _admin) = setup();
    let res = client.try_accept_admin();
    assert!(res.is_err());
}

// ── get_expiring_soon tests (#251) ────────────────────────────────────────────

#[test]
fn test_get_expiring_soon_returns_matching_records() {
    let (env, client, _admin) = setup();
    let verifier = Address::generate(&env);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    client.add_verifier(&verifier);

    env.ledger().set_timestamp(1_000);
    // alice expires at 2000 (within 1500s window from now=1000)
    client.approve(&verifier, &alice, &1, &2_000, &String::from_str(&env, "US"));
    // bob expires at 5000 (outside 1500s window)
    client.approve(&verifier, &bob, &1, &5_000, &String::from_str(&env, "US"));

    let results = client.get_expiring_soon(&1_500, &0, &10);
    assert_eq!(results.len(), 1);
    assert_eq!(results.get(0).unwrap().addr, alice);
}

#[test]
fn test_get_expiring_soon_excludes_revoked() {
    let (env, client, _admin) = setup();
    let verifier = Address::generate(&env);
    let alice = Address::generate(&env);
    client.add_verifier(&verifier);

    env.ledger().set_timestamp(1_000);
    client.approve(&verifier, &alice, &1, &2_000, &String::from_str(&env, "US"));
    // Revoke alice — she must not appear in results
    client.revoke(&verifier, &alice);

    let results = client.get_expiring_soon(&1_500, &0, &10);
    assert_eq!(results.len(), 0);
}

#[test]
fn test_get_expiring_soon_excludes_no_expiry() {
    let (env, client, _admin) = setup();
    let verifier = Address::generate(&env);
    let alice = Address::generate(&env);
    client.add_verifier(&verifier);

    env.ledger().set_timestamp(1_000);
    // expiry=0 means no expiry — must not appear in index
    client.approve(&verifier, &alice, &1, &0, &String::from_str(&env, "US"));

    let results = client.get_expiring_soon(&999_999, &0, &10);
    assert_eq!(results.len(), 0);
}

#[test]
fn test_get_expiring_soon_pagination() {
    let (env, client, _admin) = setup();
    let verifier = Address::generate(&env);
    client.add_verifier(&verifier);

    env.ledger().set_timestamp(1_000);
    let addrs: soroban_sdk::Vec<Address> = {
        let mut v = soroban_sdk::Vec::new(&env);
        for _ in 0..3 {
            let a = Address::generate(&env);
            client.approve(&verifier, &a, &1, &1_500, &String::from_str(&env, "US"));
            v.push_back(a);
        }
        v
    };

    let page0 = client.get_expiring_soon(&1_000, &0, &2);
    assert_eq!(page0.len(), 2);
    let page1 = client.get_expiring_soon(&1_000, &2, &2);
    assert_eq!(page1.len(), 1);
    let _ = addrs;
}
