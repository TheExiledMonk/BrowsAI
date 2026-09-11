use browsai_identity_vault::IdentityVault;
use browsai_profiles::ProfileManager;
use std::collections::BTreeMap;

#[test]
fn identity_fields_are_released_individually_and_once() {
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let mut fields = BTreeMap::new();
    fields.insert("email".into(), b"alice@example.test".to_vec());
    let mut vault = IdentityVault::default();
    let id = vault.create(profile.clone(), "https://example.test", fields);
    let capability = vault
        .authorize_field(&id, "email", profile.clone(), "https://example.test")
        .unwrap();
    assert_eq!(
        vault
            .with_field(&capability, &profile, "https://example.test", |value| value
                .to_vec())
            .unwrap(),
        b"alice@example.test"
    );
}

#[test]
fn identity_authorization_requires_consent_and_emits_non_sensitive_audit_events() {
    let profile = ProfileManager::default().create("work");
    let mut fields = BTreeMap::new();
    fields.insert("email".into(), b"alice@example.test".to_vec());
    let mut vault = IdentityVault::default();
    let id = vault.create(profile.clone(), "https://example.test", fields);
    assert!(vault
        .authorize_field_with_consent(&id, "email", profile.clone(), "https://example.test", false,)
        .is_err());
    let capability = vault
        .authorize_field_with_consent(&id, "email", profile.clone(), "https://example.test", true)
        .unwrap();
    vault
        .with_field(&capability, &profile, "https://example.test", |_| ())
        .unwrap();
    let events: Vec<_> = vault.drain_events().collect();
    assert_eq!(events[0].action, "created");
    assert_eq!(events[1].action, "field_authorized");
    assert_eq!(events[2].action, "field_released");
}
