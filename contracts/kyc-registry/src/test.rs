#![cfg(test)]

use crate::{KycRegistry, KycRegistryClient};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
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
#[should_panic(expected = "not an authorized verifier")]
fn test_unauthorized_verifier_cannot_approve() {
    let (env, client, _admin) = setup();
    let rogue = Address::generate(&env);
    let subject = Address::generate(&env);
    // rogue was never added as a verifier
    client.approve(&rogue, &subject, &0, &0, &String::from_str(&env, "US"));
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
fn test_renew_extends_expiry() {
    let (env, client, _admin) = setup();
    let verifier = Address::generate(&env);
    let subject = Address::generate(&env);
    client.add_verifier(&verifier);

    // Approve with expiry at 2000
    client.approve(&verifier, &subject, &1, &2_000, &String::from_str(&env, "US"));
    assert!(client.is_approved(&subject));

    // Renew with a later expiry
    client.renew(&verifier, &subject, &5_000);

    let record = client.get_record(&subject);
    assert_eq!(record.expiry, 5_000);
    // Other fields unchanged
    assert!(matches!(record.status, crate::KycStatus::Approved));
    assert_eq!(record.tier, 1);
    assert_eq!(record.jurisdiction, String::from_str(&env, "US"));

    // Verify it's still approved and not expired
    env.ledger().set_timestamp(4_000);
    assert!(client.is_approved(&subject));
}

#[test]
#[should_panic(expected = "not an authorized verifier")]
fn test_unauthorized_verifier_cannot_renew() {
    let (env, client, _admin) = setup();
    let verifier = Address::generate(&env);
    let rogue = Address::generate(&env);
    let subject = Address::generate(&env);
    client.add_verifier(&verifier);

    // Approve with the real verifier
    client.approve(&verifier, &subject, &0, &2_000, &String::from_str(&env, "US"));

    // Attempt renewal by unauthorized verifier
    client.renew(&rogue, &subject, &5_000);
}
