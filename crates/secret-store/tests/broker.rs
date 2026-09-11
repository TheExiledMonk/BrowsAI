use browsai_profiles::ProfileManager;
use browsai_secret_store::{SecretBroker, SecretError, SecretEvent, SecretPolicy};
use std::time::Duration;

#[test]
fn secret_requires_matching_profile_and_origin_and_supports_one_time_use() {
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let other = profiles.create("other");
    let mut broker = SecretBroker::default();
    let handle = broker.store(profile.clone(), "https://example.test", b"plaintext-secret");
    assert!(matches!(
        broker.grant(&handle, other, "https://example.test", "login", true),
        Err(SecretError::ProfileMismatch)
    ));
    let capability = broker
        .grant(
            &handle,
            profile.clone(),
            "https://example.test",
            "login",
            true,
        )
        .unwrap();
    let length = broker
        .with_secret(&capability, &profile, "https://example.test", |secret| {
            secret.len()
        })
        .unwrap();
    assert_eq!(length, 16);
    assert_eq!(
        broker.with_secret(&capability, &profile, "https://example.test", |_| 0),
        Err(SecretError::AlreadyConsumed)
    );
}

#[test]
fn capability_transport_is_opaque_and_never_contains_secret_bytes() {
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let mut broker = SecretBroker::default();
    let handle = broker.store(profile.clone(), "https://example.test", b"do-not-leak-this");
    let capability = broker
        .grant(&handle, profile, "https://example.test", "login", true)
        .unwrap();
    let token = SecretBroker::reference_token(&capability);
    let encoded = serde_json::to_string(&capability.reference).unwrap();
    assert_eq!(token, encoded.trim_matches('"'));
    assert!(!format!("{capability:?}").contains("do-not-leak-this"));
    assert!(!encoded.contains("do-not-leak-this"));
}

#[test]
fn expiring_capabilities_are_rejected_after_their_deadline() {
    let profile = ProfileManager::default().create("work");
    let mut broker = SecretBroker::default();
    let handle = broker.store(profile.clone(), "https://example.test", b"secret");
    let capability = broker
        .grant_for(
            &handle,
            profile.clone(),
            "https://example.test",
            "test",
            false,
            Some(Duration::ZERO),
        )
        .unwrap();
    assert_eq!(
        broker.with_secret(&capability, &profile, "https://example.test", |_| ()),
        Err(SecretError::ExpiredCapability)
    );
}

#[test]
fn revoking_a_handle_erases_the_broker_record() {
    let profile = ProfileManager::default().create("work");
    let mut broker = SecretBroker::default();
    let handle = broker.store(profile.clone(), "https://example.test", b"secret");
    let capability = broker
        .grant(
            &handle,
            profile.clone(),
            "https://example.test",
            "test",
            false,
        )
        .unwrap();
    assert!(broker.revoke(&handle));
    assert!(!broker.revoke(&handle));
    assert_eq!(
        broker.with_secret(&capability, &profile, "https://example.test", |_| ()),
        Err(SecretError::InvalidCapability)
    );
}

#[test]
fn tampering_with_capability_scope_cannot_rebind_a_secret() {
    let profile = ProfileManager::default().create("work");
    let mut broker = SecretBroker::default();
    let handle = broker.store(profile.clone(), "https://example.test", b"secret");
    let mut capability = broker
        .grant(
            &handle,
            profile.clone(),
            "https://example.test",
            "test",
            false,
        )
        .unwrap();
    capability.origin = "https://attacker.test".into();
    assert_eq!(
        broker.with_secret(&capability, &profile, "https://attacker.test", |_| ()),
        Err(SecretError::OriginMismatch)
    );
}

#[test]
fn lifecycle_events_are_non_sensitive_and_drained() {
    let profile = ProfileManager::default().create("work");
    let mut broker = SecretBroker::default();
    let handle = broker.store(profile.clone(), "https://example.test", b"never-log-this");
    let capability = broker
        .grant(
            &handle,
            profile.clone(),
            "https://example.test",
            "login",
            true,
        )
        .unwrap();
    broker
        .with_secret(&capability, &profile, "https://example.test", |_| ())
        .unwrap();
    assert!(broker.revoke(&handle));

    let events = broker.drain_events();
    assert!(matches!(events.first(), Some(SecretEvent::Stored { .. })));
    assert!(events
        .iter()
        .any(|event| matches!(event, SecretEvent::Granted { purpose, .. } if purpose == "login")));
    assert!(events
        .iter()
        .any(|event| matches!(event, SecretEvent::Released { one_time: true, .. })));
    assert!(events
        .iter()
        .any(|event| matches!(event, SecretEvent::Revoked { .. })));
    let encoded = serde_json::to_string(&events).unwrap();
    let debug = format!("{events:?}");
    assert!(!encoded.contains("never-log-this"));
    assert!(!debug.contains("never-log-this"));
    assert!(broker.drain_events().is_empty());
}

#[test]
fn encrypted_snapshots_round_trip_with_key_and_reject_tampering() {
    let profile = ProfileManager::default().create("work");
    let key = [7u8; 32];
    let mut broker = SecretBroker::new(key);
    let handle = broker.store(profile.clone(), "https://example.test", b"persisted-secret");
    let snapshot = broker.export_snapshot();
    let encoded = serde_json::to_string(&snapshot).unwrap();
    assert!(!encoded.contains("persisted-secret"));

    let mut restored = SecretBroker::from_snapshot(snapshot, key).unwrap();
    let capability = restored
        .grant(
            &handle,
            profile.clone(),
            "https://example.test",
            "restore",
            false,
        )
        .unwrap();
    assert_eq!(
        restored
            .with_secret(&capability, &profile, "https://example.test", |value| value
                .to_vec())
            .unwrap(),
        b"persisted-secret".to_vec()
    );

    let mut tampered: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    tampered["records"][0]["ciphertext"][0] = serde_json::json!(0);
    let tampered = serde_json::from_value(tampered).unwrap();
    let mut corrupted = SecretBroker::from_snapshot(tampered, key).unwrap();
    let capability = corrupted
        .grant(
            &handle,
            profile.clone(),
            "https://example.test",
            "restore",
            false,
        )
        .unwrap();
    assert_eq!(
        corrupted.with_secret(&capability, &profile, "https://example.test", |_| ()),
        Err(SecretError::CorruptRecord)
    );
}

#[test]
fn issuance_requires_consent_and_enforces_purpose_and_lifetime_policy() {
    let profile = ProfileManager::default().create("work");
    let mut broker = SecretBroker::with_policy(
        [3u8; 32],
        SecretPolicy::allow_purposes(["login"]).with_max_lifetime_seconds(60),
    );
    let handle = broker.store(profile.clone(), "https://example.test", b"secret");
    assert!(matches!(
        broker.grant_with_consent(
            &handle,
            profile.clone(),
            "https://example.test",
            "login",
            true,
            false,
        ),
        Err(SecretError::ConsentRequired)
    ));
    assert!(matches!(
        broker.grant(
            &handle,
            profile.clone(),
            "https://example.test",
            "payment",
            true
        ),
        Err(SecretError::PolicyDenied)
    ));
    assert!(matches!(
        broker.grant_for(
            &handle,
            profile.clone(),
            "https://example.test",
            "login",
            true,
            Some(Duration::from_secs(61)),
        ),
        Err(SecretError::PolicyDenied)
    ));
    assert!(broker
        .grant_for(
            &handle,
            profile.clone(),
            "https://example.test",
            "login",
            true,
            Some(Duration::from_secs(60)),
        )
        .is_ok());

    let snapshot = broker.export_snapshot();
    let mut restored = SecretBroker::from_snapshot(snapshot, [3u8; 32]).unwrap();
    assert!(matches!(
        restored.grant(&handle, profile, "https://example.test", "payment", true),
        Err(SecretError::PolicyDenied)
    ));
}
