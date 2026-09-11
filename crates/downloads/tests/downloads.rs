use browsai_downloads::{begin, normalize_filename, DownloadError, DownloadManager, DownloadState};
use browsai_permissions::{Permission, PermissionManager};
use browsai_profiles::ProfileManager;

#[test]
fn download_manager_tracks_progress_cancellation_and_quarantine() {
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let mut permissions = PermissionManager::default();
    permissions.set(
        &profile,
        "https://example.test",
        Permission::Downloads,
        true,
    );
    let handle = begin(
        &permissions,
        profile.clone(),
        "https://example.test",
        "report.pdf",
        10,
        100,
    )
    .unwrap();
    let mut manager = DownloadManager::default();
    let id = manager.add(handle);
    assert_eq!(manager.get(id).unwrap().state, DownloadState::Pending);
    assert_eq!(manager.progress(id, 4).unwrap().received, 4);
    assert_eq!(
        manager.progress(id, 20).unwrap().state,
        DownloadState::Completed
    );
    assert_eq!(manager.progress(id, 1), Err(DownloadError::InvalidState));

    let cancelled_handle = begin(
        &permissions,
        profile.clone(),
        "https://example.test",
        "cancel.me",
        10,
        100,
    )
    .unwrap();
    let cancelled_id = manager.add(cancelled_handle);
    assert_eq!(
        manager.cancel(cancelled_id).unwrap().state,
        DownloadState::Cancelled
    );
    let quarantined_handle = begin(
        &permissions,
        profile.clone(),
        "https://example.test",
        "quarantine.me",
        10,
        100,
    )
    .unwrap();
    let quarantined_id = manager.add(quarantined_handle);
    assert_eq!(
        manager.quarantine(quarantined_id).unwrap().state,
        DownloadState::Quarantined
    );

    let second = begin(
        &permissions,
        profiles.create("second"),
        "https://example.test",
        "other.zip",
        10,
        100,
    );
    assert!(second.is_err());
}

#[test]
fn download_state_transitions_emit_ordered_events() {
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let mut permissions = PermissionManager::default();
    permissions.set(
        &profile,
        "https://example.test",
        Permission::Downloads,
        true,
    );
    let handle = begin(
        &permissions,
        profile,
        "https://example.test",
        "report.txt",
        4,
        10,
    )
    .unwrap();
    let id = handle.id;
    let mut manager = DownloadManager::default();
    manager.add(handle);
    manager.progress(id, 4).unwrap();
    let events: Vec<_> = manager.drain_events().collect();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].state, DownloadState::Pending);
    assert_eq!(events[1].state, DownloadState::Completed);
    assert_eq!(events[1].received, 4);
}

#[test]
fn download_filenames_are_normalized_before_handles_are_issued() {
    assert_eq!(normalize_filename("  report.pdf  ").unwrap(), "report.pdf");
    assert_eq!(
        normalize_filename("../secret.txt"),
        Err(DownloadError::InvalidFilename)
    );
    assert_eq!(
        normalize_filename("bad\nname"),
        Err(DownloadError::InvalidFilename)
    );
}
