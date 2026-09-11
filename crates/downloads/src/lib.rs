use browsai_permissions::{Permission, PermissionDecision, PermissionManager};
use browsai_profiles::ProfileId;
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DownloadHandle {
    pub id: Uuid,
    pub profile: ProfileId,
    pub origin: String,
    pub filename: String,
    pub bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DownloadState {
    Pending,
    InProgress,
    Completed,
    Cancelled,
    Quarantined,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DownloadRecord {
    pub handle: DownloadHandle,
    pub state: DownloadState,
    pub received: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DownloadEvent {
    pub id: Uuid,
    pub state: DownloadState,
    pub received: u64,
}

#[derive(Clone, Debug, Default)]
pub struct DownloadManager {
    records: std::collections::BTreeMap<Uuid, DownloadRecord>,
    events: Vec<DownloadEvent>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DownloadError {
    Permission(PermissionDecision),
    InvalidFilename,
    TooLarge,
    NotFound,
    InvalidState,
}

pub fn normalize_filename(value: impl Into<String>) -> Result<String, DownloadError> {
    let filename = value.into().trim().to_owned();
    if filename.is_empty()
        || filename == "."
        || filename == ".."
        || filename.contains('/')
        || filename.contains('\\')
        || filename.chars().any(char::is_control)
    {
        return Err(DownloadError::InvalidFilename);
    }
    Ok(filename)
}

impl DownloadManager {
    pub fn add(&mut self, handle: DownloadHandle) -> Uuid {
        let id = handle.id;
        self.records.insert(
            id,
            DownloadRecord {
                handle,
                state: DownloadState::Pending,
                received: 0,
            },
        );
        self.events.push(DownloadEvent {
            id,
            state: DownloadState::Pending,
            received: 0,
        });
        id
    }

    pub fn progress(&mut self, id: Uuid, bytes: u64) -> Result<&DownloadRecord, DownloadError> {
        let record = self.records.get_mut(&id).ok_or(DownloadError::NotFound)?;
        if matches!(
            record.state,
            DownloadState::Completed | DownloadState::Cancelled | DownloadState::Quarantined
        ) {
            return Err(DownloadError::InvalidState);
        }
        record.state = DownloadState::InProgress;
        record.received = record
            .received
            .saturating_add(bytes)
            .min(record.handle.bytes);
        if record.received == record.handle.bytes {
            record.state = DownloadState::Completed;
        }
        self.events.push(DownloadEvent {
            id,
            state: record.state,
            received: record.received,
        });
        Ok(record)
    }

    pub fn cancel(&mut self, id: Uuid) -> Result<&DownloadRecord, DownloadError> {
        let record = self.records.get_mut(&id).ok_or(DownloadError::NotFound)?;
        if matches!(
            record.state,
            DownloadState::Completed | DownloadState::Cancelled | DownloadState::Quarantined
        ) {
            return Err(DownloadError::InvalidState);
        }
        record.state = DownloadState::Cancelled;
        self.events.push(DownloadEvent {
            id,
            state: record.state,
            received: record.received,
        });
        Ok(record)
    }

    pub fn quarantine(&mut self, id: Uuid) -> Result<&DownloadRecord, DownloadError> {
        let record = self.records.get_mut(&id).ok_or(DownloadError::NotFound)?;
        if matches!(
            record.state,
            DownloadState::Completed | DownloadState::Cancelled
        ) {
            return Err(DownloadError::InvalidState);
        }
        record.state = DownloadState::Quarantined;
        self.events.push(DownloadEvent {
            id,
            state: record.state,
            received: record.received,
        });
        Ok(record)
    }

    pub fn get(&self, id: Uuid) -> Option<&DownloadRecord> {
        self.records.get(&id)
    }

    pub fn drain_events(&mut self) -> impl Iterator<Item = DownloadEvent> + '_ {
        self.events.drain(..)
    }
}

pub fn begin(
    permissions: &PermissionManager,
    profile: ProfileId,
    origin: impl Into<String>,
    filename: impl Into<String>,
    bytes: u64,
    max_bytes: u64,
) -> Result<DownloadHandle, DownloadError> {
    let origin = origin.into();
    let decision = permissions.check(&profile, &origin, Permission::Downloads);
    if !matches!(decision, PermissionDecision::Granted) {
        return Err(DownloadError::Permission(decision));
    }
    let filename = normalize_filename(filename)?;
    if bytes > max_bytes {
        return Err(DownloadError::TooLarge);
    }
    Ok(DownloadHandle {
        id: Uuid::new_v4(),
        profile,
        origin,
        filename,
        bytes,
    })
}
