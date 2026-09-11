use browsai_credentials::CredentialBroker;
use browsai_profiles::ProfileManager;
use browsai_secret_store::SecretError;

#[test]
fn credentials_are_origin_bound_and_password_is_broker_callback_only() {
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let mut broker = CredentialBroker::default();
    let id = broker.save_password(profile.clone(), "https://example.test", "alice", b"secret");
    let capability = broker
        .grant_password(&id, profile.clone(), "https://example.test", "login")
        .unwrap();
    assert_eq!(
        broker.with_password(&capability, &profile, "https://example.test", |password| {
            password.to_vec()
        }),
        Ok(b"secret".to_vec())
    );
    assert_eq!(
        broker.with_password(&capability, &profile, "https://example.test", |_| ()),
        Err(SecretError::AlreadyConsumed)
    );
}

#[test]
fn password_updates_revoke_old_capabilities_before_new_release() {
    let profile = ProfileManager::default().create("work");
    let mut broker = CredentialBroker::default();
    let id = broker.save_password(profile.clone(), "https://example.test", "alice", b"old");
    let old_capability = broker
        .grant_password(&id, profile.clone(), "https://example.test", "login")
        .unwrap();
    broker.update_password(&id, b"new").unwrap();
    assert_eq!(
        broker.with_password(&old_capability, &profile, "https://example.test", |_| ()),
        Err(SecretError::InvalidCapability)
    );
    let new_capability = broker
        .grant_password(&id, profile.clone(), "https://example.test", "login")
        .unwrap();
    assert_eq!(
        broker.with_password(
            &new_capability,
            &profile,
            "https://example.test",
            |password| { password.to_vec() }
        ),
        Ok(b"new".to_vec())
    );
}

#[test]
fn credential_snapshots_encrypt_passwords_and_restore_with_the_key() {
    let profile = ProfileManager::default().create("work");
    let key = [9u8; 32];
    let mut broker = CredentialBroker::new(key);
    let id = broker.save_password(
        profile.clone(),
        "https://example.test",
        "alice",
        b"snapshot-password",
    );
    let snapshot = broker.export_snapshot();
    let encoded = serde_json::to_string(&snapshot).unwrap();
    assert!(!encoded.contains("snapshot-password"));

    let mut restored = CredentialBroker::from_snapshot(snapshot.clone(), key).unwrap();
    let capability = restored
        .grant_password(&id, profile.clone(), "https://example.test", "restore")
        .unwrap();
    assert_eq!(
        restored
            .with_password(&capability, &profile, "https://example.test", |password| {
                password.to_vec()
            })
            .unwrap(),
        b"snapshot-password".to_vec()
    );

    let wrong_key = CredentialBroker::from_snapshot(snapshot, [8u8; 32]).unwrap();
    let mut wrong_key = wrong_key;
    let capability = wrong_key
        .grant_password(&id, profile.clone(), "https://example.test", "restore")
        .unwrap();
    assert_eq!(
        wrong_key.with_password(&capability, &profile, "https://example.test", |_| ()),
        Err(SecretError::CorruptRecord)
    );
}
