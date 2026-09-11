use browsai_profiles::ProfileManager;
use browsai_storage::{
    OriginStorage, StorageError, StorageEvent, StorageKind, StorageWriteRequest,
};

#[test]
fn storage_is_partitioned_by_profile_origin_namespace_and_kind() {
    let mut profiles = ProfileManager::default();
    let work = profiles.create("work");
    let personal = profiles.create("personal");
    let mut storage = OriginStorage::default();
    storage.set(
        &work,
        "https://example.test",
        "app",
        StorageKind::Local,
        "token",
        "work-value",
    );
    assert_eq!(
        storage.get(
            &work,
            "https://example.test",
            "app",
            StorageKind::Local,
            "token"
        ),
        Some("work-value")
    );
    assert_eq!(
        storage.get(
            &personal,
            "https://example.test",
            "app",
            StorageKind::Local,
            "token"
        ),
        None
    );
    assert_eq!(
        storage.get(
            &work,
            "https://other.test",
            "app",
            StorageKind::Local,
            "token"
        ),
        None
    );
    assert_eq!(
        storage.get(
            &work,
            "https://example.test",
            "app",
            StorageKind::Session,
            "token"
        ),
        None
    );
}

#[test]
fn storage_enforces_quotas_and_clones_session_namespaces() {
    let profile = ProfileManager::default().create("work");
    let mut storage = OriginStorage::default();
    assert!(storage
        .set_with_quota(StorageWriteRequest {
            profile: profile.clone(),
            origin: "https://example.test".into(),
            namespace: "tab-1".into(),
            kind: StorageKind::Session,
            key: "key".into(),
            value: "value".into(),
            quota_bytes: 9,
        })
        .is_ok());
    assert_eq!(
        storage.set_with_quota(StorageWriteRequest {
            profile: profile.clone(),
            origin: "https://example.test".into(),
            namespace: "tab-1".into(),
            kind: StorageKind::Session,
            key: "another".into(),
            value: "value".into(),
            quota_bytes: 9,
        }),
        Err(StorageError::QuotaExceeded)
    );
    assert!(storage.clone_session_namespace(&profile, "https://example.test", "tab-1", "tab-2"));
    assert_eq!(
        storage.get(
            &profile,
            "https://example.test",
            "tab-2",
            StorageKind::Session,
            "key"
        ),
        Some("value")
    );
}

#[test]
fn storage_round_trips_local_session_values_and_generations() {
    let profile = ProfileManager::default().create("work");
    let mut storage = OriginStorage::default();
    storage.set(
        &profile,
        "https://example.test",
        "app",
        StorageKind::Local,
        "theme",
        "dark",
    );
    storage.set(
        &profile,
        "https://example.test",
        "tab-1",
        StorageKind::Session,
        "step",
        "2",
    );
    let restored = OriginStorage::from_json(&storage.to_json().unwrap()).unwrap();
    assert_eq!(
        restored.get(
            &profile,
            "https://example.test",
            "app",
            StorageKind::Local,
            "theme"
        ),
        Some("dark")
    );
    assert_eq!(
        restored.get(
            &profile,
            "https://example.test",
            "tab-1",
            StorageKind::Session,
            "step"
        ),
        Some("2")
    );
    assert_eq!(
        restored.generation(&profile, "https://example.test", "app", StorageKind::Local),
        1
    );
}

#[test]
fn storage_mutations_are_observable_and_quota_rejections_are_silent() {
    let profile = ProfileManager::default().create("work");
    let mut storage = OriginStorage::default();
    assert_eq!(
        storage.set_with_quota(StorageWriteRequest {
            profile: profile.clone(),
            origin: "https://example.test".into(),
            namespace: "app".into(),
            kind: StorageKind::Local,
            key: "large".into(),
            value: "value".into(),
            quota_bytes: 1,
        }),
        Err(StorageError::QuotaExceeded)
    );
    storage.set(
        &profile,
        "https://example.test",
        "app",
        StorageKind::Local,
        "key",
        "value",
    );
    storage.remove(
        &profile,
        "https://example.test",
        "app",
        StorageKind::Local,
        "key",
    );
    let events: Vec<_> = storage.drain_events().collect();
    assert!(matches!(events[0], StorageEvent::Set { .. }));
    assert!(matches!(events[1], StorageEvent::Removed { .. }));
    assert_eq!(events.len(), 2);
}
