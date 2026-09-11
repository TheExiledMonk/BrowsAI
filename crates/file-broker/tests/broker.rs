use browsai_file_broker::{grant_root, resolve, FileError};
use browsai_permissions::{Permission, PermissionManager};
use browsai_profiles::ProfileManager;

#[test]
fn file_access_requires_grant_and_rejects_escape_paths() {
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let mut permissions = PermissionManager::default();
    assert!(grant_root(&permissions, &profile, "https://example.test", "/sandbox").is_err());
    permissions.set(&profile, "https://example.test", Permission::Files, true);
    let capability =
        grant_root(&permissions, &profile, "https://example.test", "/sandbox").unwrap();
    assert_eq!(
        resolve(&capability, "report.pdf").unwrap().to_str(),
        Some("/sandbox/report.pdf")
    );
    assert_eq!(
        resolve(&capability, "../etc/passwd"),
        Err(FileError::InvalidPath)
    );
}
