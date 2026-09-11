use browsai_file_broker::{
    cleanup_temporary, create_temporary, directory_size, grant_root, resolve_existing,
    within_quota, FileError,
};
use browsai_permissions::{Permission, PermissionManager};
use browsai_profiles::ProfileManager;
use std::fs;

#[cfg(unix)]
#[test]
fn existing_resolution_rejects_symlink_escape_from_capability_root() {
    use std::os::unix::fs::symlink;

    let root = std::env::temp_dir().join(format!("browsai-sandbox-{}", uuid::Uuid::new_v4()));
    let outside = root.with_extension("outside");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("secret.txt"), b"secret").unwrap();
    symlink(&outside, root.join("link")).unwrap();

    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let mut permissions = PermissionManager::default();
    permissions.set(&profile, "https://example.test", Permission::Files, true);
    let capability = grant_root(&permissions, &profile, "https://example.test", &root).unwrap();
    assert_eq!(
        resolve_existing(&capability, "link/secret.txt"),
        Err(FileError::InvalidPath)
    );
    fs::remove_dir_all(&root).unwrap();
    fs::remove_dir_all(&outside).unwrap();
}

#[test]
fn temporary_files_are_unique_and_quota_accounting_is_root_scoped() {
    let root = std::env::temp_dir().join(format!("browsai-quota-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("data.bin"), [1u8, 2, 3]).unwrap();
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let mut permissions = PermissionManager::default();
    permissions.set(&profile, "https://example.test", Permission::Files, true);
    let capability = grant_root(&permissions, &profile, "https://example.test", &root).unwrap();
    assert_eq!(directory_size(&root).unwrap(), 3);
    assert!(within_quota(&root, 3).unwrap());
    assert!(!within_quota(&root, 2).unwrap());
    let temporary = create_temporary(&capability, "upload").unwrap();
    assert!(temporary.starts_with(&root));
    assert!(cleanup_temporary(&temporary).is_ok());
    assert!(cleanup_temporary(&temporary).is_err());
    fs::remove_dir_all(&root).unwrap();
}
