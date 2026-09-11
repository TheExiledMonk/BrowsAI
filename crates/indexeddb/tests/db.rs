use browsai_indexeddb::{IndexedDb, IndexedDbEvent};
use browsai_profiles::ProfileManager;

#[test]
fn indexeddb_isolated_by_profile_and_origin_and_commits_transactions() {
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let mut db = IndexedDb::with_quota(1024);
    db.open(
        &profile,
        "https://example.test",
        "app",
        1,
        &["records".into()],
    )
    .unwrap();
    let mut tx = db
        .transaction(&profile, "https://example.test", "app")
        .unwrap();
    tx.put("records", "one", b"value".to_vec()).unwrap();
    tx.commit().unwrap();
    let tx = db
        .transaction(&profile, "https://example.test", "app")
        .unwrap();
    assert_eq!(tx.get("records", "one").unwrap(), Some(b"value".to_vec()));
    assert!(db
        .transaction(&profile, "https://other.test", "app")
        .is_err());
}

#[test]
fn indexeddb_quota_rejects_transaction_atomically() {
    let profile = ProfileManager::default().create("work");
    let mut db = IndexedDb::with_quota(4);
    db.open(
        &profile,
        "https://example.test",
        "app",
        1,
        &["items".into()],
    )
    .unwrap();
    let mut transaction = db
        .transaction(&profile, "https://example.test", "app")
        .unwrap();
    transaction.put("items", "a", b"12345".to_vec()).unwrap();
    assert_eq!(
        transaction.commit(),
        Err(browsai_indexeddb::IndexedDbError::QuotaExceeded)
    );
    let transaction = db
        .transaction(&profile, "https://example.test", "app")
        .unwrap();
    assert_eq!(transaction.get("items", "a").unwrap(), None);
}

#[test]
fn indexeddb_reports_open_upgrade_and_transaction_lifecycle_events() {
    let profile = ProfileManager::default().create("work");
    let mut db = IndexedDb::default();
    db.open(
        &profile,
        "https://example.test",
        "app",
        1,
        &["items".into()],
    )
    .unwrap();
    db.open(&profile, "https://example.test", "app", 2, &[])
        .unwrap();
    db.transaction(&profile, "https://example.test", "app")
        .unwrap()
        .abort();
    let events: Vec<_> = db.drain_events().collect();
    assert!(matches!(
        events[0],
        IndexedDbEvent::Upgraded {
            from_version: 0,
            to_version: 1,
            ..
        }
    ));
    assert!(matches!(
        events[1],
        IndexedDbEvent::Upgraded {
            from_version: 1,
            to_version: 2,
            ..
        }
    ));
    assert!(matches!(events[2], IndexedDbEvent::Aborted { .. }));
}
