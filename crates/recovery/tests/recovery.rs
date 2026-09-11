use browsai_recovery::{
    FileRecoveryStore, RecoveryError, RecoveryFileOptions, RecoveryPhase, RecoveryResource,
    RecoveryStore, RecoveryTarget, SessionCheckpoint, SessionRecoveryState,
};
use std::collections::BTreeMap;
use uuid::Uuid;

#[test]
fn recovery_round_trips_and_rejects_unsupported_versions() {
    let checkpoint = SessionCheckpoint {
        id: Uuid::new_v4(),
        schema_version: 1,
        profiles: vec!["work".into()],
        windows: vec!["window-1".into()],
        tabs: vec!["tab-1".into()],
        locks: vec![],
        pending_confirmations: vec!["payment-1".into()],
        state: Default::default(),
    };
    let mut store = RecoveryStore::default();
    store.save(checkpoint.clone());
    assert_eq!(store.load(1).unwrap(), Some(checkpoint));
    assert_eq!(store.load(0), Err(RecoveryError::UnsupportedVersion));
}

#[test]
fn file_store_writes_atomically_and_keeps_a_backup() {
    let directory = std::env::temp_dir().join(format!("browsai-recovery-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("session.json");
    let store = browsai_recovery::FileRecoveryStore::new(&path, Default::default());
    let first = SessionCheckpoint {
        id: Uuid::new_v4(),
        schema_version: 1,
        profiles: vec!["first".into()],
        windows: vec![],
        tabs: vec![],
        locks: vec![],
        pending_confirmations: vec![],
        state: Default::default(),
    };
    let second = SessionCheckpoint {
        profiles: vec!["second".into()],
        ..first.clone()
    };
    store.save(first.clone()).unwrap();
    store.save(second.clone()).unwrap();
    assert_eq!(store.load().unwrap(), Some(second));
    assert!(store.restore_backup().unwrap());
    assert_eq!(store.load().unwrap(), Some(first));
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn backup_rotation_keeps_the_primary_checkpoint_until_new_payload_is_ready() {
    let directory = std::env::temp_dir().join(format!("browsai-recovery-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("session.json");
    let store = FileRecoveryStore::new(&path, RecoveryFileOptions::default());
    let first = SessionCheckpoint {
        id: Uuid::new_v4(),
        schema_version: 1,
        profiles: vec!["first".into()],
        windows: vec![],
        tabs: vec![],
        locks: vec![],
        pending_confirmations: vec![],
        state: Default::default(),
    };
    let second = SessionCheckpoint {
        id: Uuid::new_v4(),
        profiles: vec!["second".into()],
        ..first.clone()
    };
    store.save(first.clone()).unwrap();
    store.save(second.clone()).unwrap();
    assert_eq!(store.load().unwrap().unwrap().id, second.id);
    assert!(store.restore_backup().unwrap());
    assert_eq!(store.load().unwrap().unwrap().id, first.id);
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn restore_backup_rejects_corruption_without_overwriting_primary() {
    let directory =
        std::env::temp_dir().join(format!("browsai-recovery-corrupt-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("session.json");
    let store = browsai_recovery::FileRecoveryStore::new(&path, Default::default());
    let first = SessionCheckpoint {
        id: Uuid::new_v4(),
        schema_version: 1,
        profiles: vec!["first".into()],
        windows: vec![],
        tabs: vec![],
        locks: vec![],
        pending_confirmations: vec![],
        state: Default::default(),
    };
    let second = SessionCheckpoint {
        profiles: vec!["second".into()],
        ..first.clone()
    };
    store.save(first).unwrap();
    store.save(second.clone()).unwrap();
    std::fs::write(path.with_extension("bak0"), b"corrupt").unwrap();
    assert_eq!(store.restore_backup(), Err(RecoveryError::Serialization));
    assert_eq!(store.load().unwrap(), Some(second));
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn typed_session_state_round_trips_without_secret_bytes() {
    let state = SessionRecoveryState {
        profiles: vec![RecoveryResource {
            id: "profile-work".into(),
            generation: 3,
            metadata: BTreeMap::from([("name".into(), "Work".into())]),
            opaque_reference: None,
        }],
        tabs: vec![RecoveryResource {
            id: "tab-1".into(),
            generation: 4,
            metadata: BTreeMap::from([("url".into(), "https://example.test".into())]),
            opaque_reference: Some("cookie-jar-handle".into()),
        }],
        ..Default::default()
    };
    let checkpoint = SessionCheckpoint {
        id: Uuid::new_v4(),
        schema_version: 1,
        profiles: vec!["profile-work".into()],
        windows: vec![],
        tabs: vec!["tab-1".into()],
        locks: vec![],
        pending_confirmations: vec![],
        state,
    };
    let mut store = RecoveryStore::default();
    store.save(checkpoint.clone());
    assert_eq!(store.load(1).unwrap(), Some(checkpoint));
}

#[test]
fn recovery_plan_orders_dependencies_and_rejects_secret_metadata() {
    let state = SessionRecoveryState {
        profiles: vec![RecoveryResource {
            id: "profile-1".into(),
            generation: 1,
            metadata: BTreeMap::new(),
            opaque_reference: None,
        }],
        tabs: vec![RecoveryResource {
            id: "tab-1".into(),
            generation: 2,
            metadata: BTreeMap::new(),
            opaque_reference: Some("tab-handle".into()),
        }],
        storage: vec![RecoveryResource {
            id: "storage-1".into(),
            generation: 3,
            metadata: BTreeMap::new(),
            opaque_reference: Some("storage-handle".into()),
        }],
        ..Default::default()
    };
    let plan = state.plan().unwrap();
    assert_eq!(plan.steps[0].phase, RecoveryPhase::Profiles);
    assert!(
        plan.steps
            .iter()
            .position(|step| step.phase == RecoveryPhase::Storage)
            .unwrap()
            > plan
                .steps
                .iter()
                .position(|step| step.phase == RecoveryPhase::Tabs)
                .unwrap()
    );

    let invalid = SessionRecoveryState {
        profiles: vec![RecoveryResource {
            id: "profile-1".into(),
            generation: 1,
            metadata: BTreeMap::from([("password".into(), "plaintext".into())]),
            opaque_reference: None,
        }],
        ..Default::default()
    };
    assert_eq!(invalid.plan(), Err(RecoveryError::InvalidState));
}

#[test]
fn recovery_plan_executes_resource_classes_in_dependency_order() {
    #[derive(Default)]
    struct Target(Vec<RecoveryPhase>);
    impl RecoveryTarget for Target {
        type Error = ();
        fn restore_profile(&mut self, _: &RecoveryResource) -> Result<(), Self::Error> {
            self.0.push(RecoveryPhase::Profiles);
            Ok(())
        }
        fn restore_workspace(&mut self, _: &RecoveryResource) -> Result<(), Self::Error> {
            self.0.push(RecoveryPhase::Workspaces);
            Ok(())
        }
        fn restore_tab(&mut self, _: &RecoveryResource) -> Result<(), Self::Error> {
            self.0.push(RecoveryPhase::Tabs);
            Ok(())
        }
        fn restore_navigation(&mut self, _: &RecoveryResource) -> Result<(), Self::Error> {
            self.0.push(RecoveryPhase::Navigation);
            Ok(())
        }
        fn restore_storage(&mut self, _: &RecoveryResource) -> Result<(), Self::Error> {
            self.0.push(RecoveryPhase::Storage);
            Ok(())
        }
        fn restore_agent_session(&mut self, _: &RecoveryResource) -> Result<(), Self::Error> {
            self.0.push(RecoveryPhase::AgentSessions);
            Ok(())
        }
        fn restore_lock(&mut self, _: &RecoveryResource) -> Result<(), Self::Error> {
            self.0.push(RecoveryPhase::Locks);
            Ok(())
        }
        fn restore_confirmation(&mut self, _: &RecoveryResource) -> Result<(), Self::Error> {
            self.0.push(RecoveryPhase::PendingConfirmations);
            Ok(())
        }
    }
    let state = SessionRecoveryState {
        profiles: vec![RecoveryResource {
            id: "p".into(),
            generation: 0,
            metadata: BTreeMap::new(),
            opaque_reference: None,
        }],
        workspaces: vec![RecoveryResource {
            id: "w".into(),
            generation: 0,
            metadata: BTreeMap::new(),
            opaque_reference: None,
        }],
        ..Default::default()
    };
    let plan = state.plan().unwrap();
    let mut target = Target::default();
    let report = plan.restore(&mut target).unwrap();
    assert_eq!(report.restored, 2);
    assert_eq!(
        target.0,
        vec![RecoveryPhase::Profiles, RecoveryPhase::Workspaces]
    );
}
