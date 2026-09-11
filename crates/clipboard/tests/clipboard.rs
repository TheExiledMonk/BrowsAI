use browsai_clipboard::ClipboardBroker;
use browsai_permissions::{Permission, PermissionManager};
use browsai_profiles::ProfileManager;

#[test]
fn clipboard_requires_separate_read_and_write_grants() {
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let mut permissions = PermissionManager::default();
    let mut clipboard = ClipboardBroker::default();
    permissions.set(
        &profile,
        "https://example.test",
        Permission::ClipboardWrite,
        true,
    );
    clipboard
        .write(&permissions, &profile, "https://example.test", "value")
        .unwrap();
    assert!(clipboard
        .read(&permissions, &profile, "https://example.test")
        .is_err());
}

#[test]
fn gesture_bound_reads_are_one_time() {
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let mut permissions = PermissionManager::default();
    permissions.set(
        &profile,
        "https://example.test",
        Permission::ClipboardWrite,
        true,
    );
    permissions.set(
        &profile,
        "https://example.test",
        Permission::ClipboardRead,
        true,
    );
    let mut clipboard = ClipboardBroker::default();
    clipboard
        .write(&permissions, &profile, "https://example.test", "value")
        .unwrap();
    let token = clipboard.issue_user_gesture();
    assert_eq!(
        clipboard
            .read_with_gesture(
                &permissions,
                &profile,
                "https://example.test",
                token.clone()
            )
            .unwrap(),
        "value"
    );
    assert_eq!(
        clipboard.read_with_gesture(&permissions, &profile, "https://example.test", token),
        Err(browsai_clipboard::ClipboardError::MissingUserGesture)
    );
}

#[test]
fn clipboard_values_are_partitioned_by_profile_and_origin() {
    let mut profiles = ProfileManager::default();
    let first = profiles.create("first");
    let second = profiles.create("second");
    let mut permissions = PermissionManager::default();
    for profile in [&first, &second] {
        permissions.set(
            profile,
            "https://example.test",
            Permission::ClipboardWrite,
            true,
        );
        permissions.set(
            profile,
            "https://example.test",
            Permission::ClipboardRead,
            true,
        );
    }
    permissions.set(
        &first,
        "https://other.test",
        Permission::ClipboardRead,
        true,
    );
    let mut clipboard = ClipboardBroker::default();
    clipboard
        .write(&permissions, &first, "https://example.test", "first")
        .unwrap();
    clipboard
        .write(&permissions, &second, "https://example.test", "second")
        .unwrap();
    assert_eq!(
        clipboard
            .read(&permissions, &first, "https://example.test")
            .unwrap(),
        "first"
    );
    assert_eq!(
        clipboard
            .read(&permissions, &second, "https://example.test")
            .unwrap(),
        "second"
    );
    assert_eq!(
        clipboard
            .read(&permissions, &first, "https://other.test")
            .unwrap(),
        ""
    );
}
