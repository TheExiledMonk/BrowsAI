use browsai_permissions::{Permission, PermissionDecision, PermissionManager};
use browsai_profiles::ProfileId;
use std::path::{Component, Path, PathBuf};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileCapability {
    pub reference: Uuid,
    pub profile: ProfileId,
    pub origin: String,
    root: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FileError {
    Permission(PermissionDecision),
    InvalidPath,
    EmptyRoot,
    Io,
}

pub fn grant_root(
    permissions: &PermissionManager,
    profile: &ProfileId,
    origin: impl Into<String>,
    root: impl Into<PathBuf>,
) -> Result<FileCapability, FileError> {
    let origin = origin.into();
    let decision = permissions.check(profile, &origin, Permission::Files);
    if !matches!(decision, PermissionDecision::Granted) {
        return Err(FileError::Permission(decision));
    }
    let root = root.into();
    if root.as_os_str().is_empty() {
        return Err(FileError::EmptyRoot);
    }
    Ok(FileCapability {
        reference: Uuid::new_v4(),
        profile: profile.clone(),
        origin,
        root,
    })
}

pub fn resolve(
    capability: &FileCapability,
    relative: impl AsRef<Path>,
) -> Result<PathBuf, FileError> {
    let relative = relative.as_ref();
    if relative.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(FileError::InvalidPath);
    }
    Ok(capability.root.join(relative))
}

/// Resolves an existing path while enforcing the capability root after symlink expansion.
pub fn resolve_existing(
    capability: &FileCapability,
    relative: impl AsRef<Path>,
) -> Result<PathBuf, FileError> {
    let candidate = resolve(capability, relative)?;
    let root = std::fs::canonicalize(&capability.root).map_err(|_| FileError::InvalidPath)?;
    let resolved = std::fs::canonicalize(candidate).map_err(|_| FileError::InvalidPath)?;
    if !resolved.starts_with(&root) {
        return Err(FileError::InvalidPath);
    }
    Ok(resolved)
}

pub fn directory_size(root: impl AsRef<Path>) -> Result<u64, FileError> {
    fn size(path: &Path) -> Result<u64, FileError> {
        let mut total: u64 = 0;
        for entry in std::fs::read_dir(path).map_err(|_| FileError::Io)? {
            let entry = entry.map_err(|_| FileError::Io)?;
            let metadata = std::fs::symlink_metadata(entry.path()).map_err(|_| FileError::Io)?;
            if metadata.is_dir() {
                total = total.saturating_add(size(&entry.path())?);
            } else if metadata.is_file() {
                total = total.saturating_add(metadata.len());
            }
        }
        Ok(total)
    }
    size(root.as_ref())
}

pub fn within_quota(root: impl AsRef<Path>, max_bytes: u64) -> Result<bool, FileError> {
    Ok(directory_size(root)? <= max_bytes)
}

pub fn create_temporary(capability: &FileCapability, prefix: &str) -> Result<PathBuf, FileError> {
    if prefix.is_empty() || prefix.chars().any(char::is_control) || prefix.contains('/') {
        return Err(FileError::InvalidPath);
    }
    std::fs::create_dir_all(&capability.root).map_err(|_| FileError::Io)?;
    let path = capability
        .root
        .join(format!(".{prefix}-{}", Uuid::new_v4()));
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|_| FileError::Io)?;
    Ok(path)
}

pub fn cleanup_temporary(path: impl AsRef<Path>) -> Result<(), FileError> {
    std::fs::remove_file(path).map_err(|_| FileError::Io)
}
