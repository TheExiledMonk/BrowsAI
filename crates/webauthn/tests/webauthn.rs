use browsai_profiles::ProfileManager;
use browsai_webauthn::{WebAuthnBroker, WebAuthnError};

#[test]
fn webauthn_binds_passkeys_to_profile_and_rp_origin() {
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let mut broker = WebAuthnBroker::default();
    let handle = broker
        .register(profile.clone(), "https://example.test/", b"user".to_vec())
        .unwrap();
    assert!(broker
        .begin_assertion(&handle, &profile, "https://other.test/", true)
        .is_err());
    assert!(broker
        .begin_assertion(&handle, &profile, "https://example.test/", true)
        .is_ok());
    assert_eq!(
        broker.register(profile, "javascript:bad", vec![]),
        Err(WebAuthnError::InvalidRpOrigin)
    );
}

#[test]
fn assertion_challenges_are_single_use() {
    let profile = ProfileManager::default().create("work");
    let mut broker = WebAuthnBroker::default();
    let handle = broker
        .register(profile.clone(), "https://example.test/", b"user".to_vec())
        .unwrap();
    let challenge = broker
        .begin_assertion(&handle, &profile, "https://example.test/", true)
        .unwrap();
    assert!(broker.complete_assertion(&challenge).is_ok());
    assert_eq!(
        broker.complete_assertion(&challenge),
        Err(WebAuthnError::UnknownChallenge)
    );
}
