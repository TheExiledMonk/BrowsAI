use browsai_permissions::{Permission, PermissionDecision, PermissionManager};
use browsai_profiles::ProfileId;
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UploadHandle {
    pub id: Uuid,
    pub profile: ProfileId,
    pub origin: String,
    pub filename: String,
    pub mime: String,
    pub bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UploadState {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UploadRecord {
    pub handle: UploadHandle,
    pub state: UploadState,
    pub sent: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UploadEvent {
    pub id: Uuid,
    pub state: UploadState,
    pub sent: u64,
}

#[derive(Clone, Debug, Default)]
pub struct UploadManager {
    records: std::collections::BTreeMap<Uuid, UploadRecord>,
    events: Vec<UploadEvent>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UploadError {
    Permission(PermissionDecision),
    InvalidFilename,
    TooLarge,
    NotFound,
    InvalidState,
}

pub fn normalize_filename(value: impl Into<String>) -> Result<String, UploadError> {
    let filename = value.into().trim().to_owned();
    if filename.is_empty()
        || filename == "."
        || filename == ".."
        || filename.contains('/')
        || filename.contains('\\')
        || filename.chars().any(char::is_control)
    {
        return Err(UploadError::InvalidFilename);
    }
    Ok(filename)
}

impl UploadManager {
    pub fn add(&mut self, handle: UploadHandle) -> Uuid {
        let id = handle.id;
        self.records.insert(
            id,
            UploadRecord {
                handle,
                state: UploadState::Pending,
                sent: 0,
            },
        );
        self.events.push(UploadEvent {
            id,
            state: UploadState::Pending,
            sent: 0,
        });
        id
    }

    pub fn progress(&mut self, id: Uuid, bytes: u64) -> Result<&UploadRecord, UploadError> {
        let record = self.records.get_mut(&id).ok_or(UploadError::NotFound)?;
        if matches!(
            record.state,
            UploadState::Completed | UploadState::Cancelled
        ) {
            return Err(UploadError::InvalidState);
        }
        record.state = UploadState::InProgress;
        record.sent = record.sent.saturating_add(bytes).min(record.handle.bytes);
        if record.sent == record.handle.bytes {
            record.state = UploadState::Completed;
        }
        self.events.push(UploadEvent {
            id,
            state: record.state,
            sent: record.sent,
        });
        Ok(record)
    }

    pub fn cancel(&mut self, id: Uuid) -> Result<&UploadRecord, UploadError> {
        let record = self.records.get_mut(&id).ok_or(UploadError::NotFound)?;
        if matches!(
            record.state,
            UploadState::Completed | UploadState::Cancelled
        ) {
            return Err(UploadError::InvalidState);
        }
        record.state = UploadState::Cancelled;
        self.events.push(UploadEvent {
            id,
            state: record.state,
            sent: record.sent,
        });
        Ok(record)
    }

    pub fn get(&self, id: Uuid) -> Option<&UploadRecord> {
        self.records.get(&id)
    }

    pub fn drain_events(&mut self) -> impl Iterator<Item = UploadEvent> + '_ {
        self.events.drain(..)
    }
}

pub fn authorize(
    permissions: &PermissionManager,
    profile: ProfileId,
    origin: impl Into<String>,
    filename: impl Into<String>,
    mime: impl Into<String>,
    bytes: u64,
    max_bytes: u64,
) -> Result<UploadHandle, UploadError> {
    let origin = origin.into();
    let decision = permissions.check(&profile, &origin, Permission::Uploads);
    if !matches!(decision, PermissionDecision::Granted) {
        return Err(UploadError::Permission(decision));
    }
    let filename = normalize_filename(filename)?;
    if bytes > max_bytes {
        return Err(UploadError::TooLarge);
    }
    Ok(UploadHandle {
        id: Uuid::new_v4(),
        profile,
        origin,
        filename,
        mime: mime.into(),
        bytes,
    })
}
