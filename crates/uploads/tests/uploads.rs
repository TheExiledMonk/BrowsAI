use browsai_permissions::{Permission, PermissionManager};
use browsai_profiles::ProfileManager;
use browsai_uploads::{authorize, UploadError, UploadManager, UploadState};

#[test]
fn upload_manager_tracks_bounded_progress_and_cancellation() {
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let mut permissions = PermissionManager::default();
    permissions.set(&profile, "https://example.test", Permission::Uploads, true);
    let handle = authorize(
        &permissions,
        profile.clone(),
        "https://example.test",
        "photo.png",
        "image/png",
        10,
        100,
    )
    .unwrap();
    let mut manager = UploadManager::default();
    let id = manager.add(handle);
    assert_eq!(manager.get(id).unwrap().state, UploadState::Pending);
    assert_eq!(manager.progress(id, 4).unwrap().sent, 4);
    assert_eq!(
        manager.progress(id, 20).unwrap().state,
        UploadState::Completed
    );
    assert_eq!(manager.progress(id, 1), Err(UploadError::InvalidState));

    let cancelled = authorize(
        &permissions,
        profile,
        "https://example.test",
        "cancel.png",
        "image/png",
        10,
        100,
    )
    .unwrap();
    let cancelled_id = manager.add(cancelled);
    assert_eq!(
        manager.cancel(cancelled_id).unwrap().state,
        UploadState::Cancelled
    );
}

#[test]
fn upload_state_transitions_emit_ordered_events() {
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let mut permissions = PermissionManager::default();
    permissions.set(&profile, "https://example.test", Permission::Uploads, true);
    let handle = authorize(
        &permissions,
        profile,
        "https://example.test",
        "report.txt",
        "text/plain",
        4,
        10,
    )
    .unwrap();
    let id = handle.id;
    let mut manager = UploadManager::default();
    manager.add(handle);
    manager.progress(id, 4).unwrap();
    let events: Vec<_> = manager.drain_events().collect();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].state, UploadState::Pending);
    assert_eq!(events[1].state, UploadState::Completed);
    assert_eq!(events[1].sent, 4);
}
