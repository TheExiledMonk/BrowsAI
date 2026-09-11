use browsai_profiles::ProfileManager;
use browsai_totp::TotpBroker;

#[test]
fn totp_is_generated_inside_broker_and_not_returned_as_storage_state() {
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let mut broker = TotpBroker::default();
    let id = broker.save_seed(
        profile.clone(),
        "https://example.test",
        b"GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ",
    );
    let capability = broker
        .capability(&id, profile.clone(), "https://example.test")
        .unwrap();
    let code = broker
        .with_code(&capability, &profile, "https://example.test", 59, |code| {
            code.to_owned()
        })
        .unwrap();
    assert_eq!(code, "287082");
}

#[test]
fn invalid_totp_seed_is_rejected_instead_of_using_an_empty_key() {
    let profile = ProfileManager::default().create("work");
    let mut broker = TotpBroker::default();
    let id = broker.save_seed(profile.clone(), "https://example.test", b"not base32!");
    let capability = broker
        .capability(&id, profile.clone(), "https://example.test")
        .unwrap();
    assert_eq!(
        broker.with_code(&capability, &profile, "https://example.test", 0, |_| {
            "unused"
        }),
        Err(browsai_secret_store::SecretError::InvalidSecret)
    );
}
