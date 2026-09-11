use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ProcessState {
    Running,
    Crashed,
    Restarting,
    Stopped,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CrashReport {
    pub process: String,
    pub reason: String,
    pub restart_count: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SessionCheckpoint {
    pub id: Uuid,
    pub schema_version: u16,
    pub profiles: Vec<String>,
    pub windows: Vec<String>,
    pub tabs: Vec<String>,
    pub locks: Vec<String>,
    pub pending_confirmations: Vec<String>,
    #[serde(default)]
    pub state: SessionRecoveryState,
}

/// Secret-safe browser state that may be restored after a crash. Values are
/// identifiers, generations, and non-sensitive metadata; secret bytes and
/// live capabilities must be reacquired from their brokers.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SessionRecoveryState {
    pub profiles: Vec<RecoveryResource>,
    pub tabs: Vec<RecoveryResource>,
    pub cookies: Vec<RecoveryResource>,
    pub storage: Vec<RecoveryResource>,
    pub downloads: Vec<RecoveryResource>,
    pub workspaces: Vec<RecoveryResource>,
    pub agent_sessions: Vec<RecoveryResource>,
    pub locks: Vec<RecoveryResource>,
    pub pending_confirmations: Vec<RecoveryResource>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RecoveryResource {
    pub id: String,
    pub generation: u64,
    pub metadata: BTreeMap<String, String>,
    pub opaque_reference: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum RecoveryPhase {
    Profiles,
    Workspaces,
    Tabs,
    Navigation,
    Storage,
    AgentSessions,
    Locks,
    PendingConfirmations,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RecoveryStep {
    pub phase: RecoveryPhase,
    pub resource: RecoveryResource,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct RecoveryPlan {
    pub steps: Vec<RecoveryStep>,
}

pub trait RecoveryTarget {
    type Error;

    fn restore_profile(&mut self, resource: &RecoveryResource) -> Result<(), Self::Error>;
    fn restore_workspace(&mut self, resource: &RecoveryResource) -> Result<(), Self::Error>;
    fn restore_tab(&mut self, resource: &RecoveryResource) -> Result<(), Self::Error>;
    fn restore_navigation(&mut self, resource: &RecoveryResource) -> Result<(), Self::Error>;
    fn restore_storage(&mut self, resource: &RecoveryResource) -> Result<(), Self::Error>;
    fn restore_agent_session(&mut self, resource: &RecoveryResource) -> Result<(), Self::Error>;
    fn restore_lock(&mut self, resource: &RecoveryResource) -> Result<(), Self::Error>;
    fn restore_confirmation(&mut self, resource: &RecoveryResource) -> Result<(), Self::Error>;
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct RestoreReport {
    pub restored: usize,
    pub phases: Vec<RecoveryPhase>,
}

impl RecoveryPlan {
    pub fn restore<T: RecoveryTarget>(&self, target: &mut T) -> Result<RestoreReport, T::Error> {
        let mut report = RestoreReport::default();
        for step in &self.steps {
            match step.phase {
                RecoveryPhase::Profiles => target.restore_profile(&step.resource)?,
                RecoveryPhase::Workspaces => target.restore_workspace(&step.resource)?,
                RecoveryPhase::Tabs => target.restore_tab(&step.resource)?,
                RecoveryPhase::Navigation => target.restore_navigation(&step.resource)?,
                RecoveryPhase::Storage => target.restore_storage(&step.resource)?,
                RecoveryPhase::AgentSessions => target.restore_agent_session(&step.resource)?,
                RecoveryPhase::Locks => target.restore_lock(&step.resource)?,
                RecoveryPhase::PendingConfirmations => {
                    target.restore_confirmation(&step.resource)?
                }
            }
            report.restored += 1;
            if report.phases.last() != Some(&step.phase) {
                report.phases.push(step.phase);
            }
        }
        Ok(report)
    }
}

impl SessionRecoveryState {
    pub fn validate(&self) -> Result<(), RecoveryError> {
        let collections = [
            &self.profiles,
            &self.tabs,
            &self.cookies,
            &self.storage,
            &self.downloads,
            &self.workspaces,
            &self.agent_sessions,
            &self.locks,
            &self.pending_confirmations,
        ];
        if collections
            .iter()
            .flat_map(|resources| resources.iter())
            .any(|resource| {
                resource.id.trim().is_empty()
                    || resource.metadata.keys().any(|key| key.trim().is_empty())
                    || resource.metadata.iter().any(|(key, value)| {
                        let normalized = key.to_ascii_lowercase().replace(['_', '-', '.'], "");
                        [
                            "password",
                            "secret",
                            "token",
                            "seed",
                            "privatekey",
                            "cardnumber",
                        ]
                        .iter()
                        .any(|marker| {
                            normalized.contains(marker)
                                || value.to_ascii_lowercase().contains(marker)
                        })
                    })
            })
        {
            return Err(RecoveryError::InvalidState);
        }
        Ok(())
    }

    pub fn plan(&self) -> Result<RecoveryPlan, RecoveryError> {
        self.validate()?;
        let mut steps = Vec::new();
        let add = |steps: &mut Vec<RecoveryStep>, phase, resources: &[RecoveryResource]| {
            steps.extend(
                resources
                    .iter()
                    .cloned()
                    .map(|resource| RecoveryStep { phase, resource }),
            );
        };
        add(&mut steps, RecoveryPhase::Profiles, &self.profiles);
        add(&mut steps, RecoveryPhase::Workspaces, &self.workspaces);
        add(&mut steps, RecoveryPhase::Tabs, &self.tabs);
        add(&mut steps, RecoveryPhase::Navigation, &self.tabs);
        add(&mut steps, RecoveryPhase::Storage, &self.storage);
        add(
            &mut steps,
            RecoveryPhase::AgentSessions,
            &self.agent_sessions,
        );
        add(&mut steps, RecoveryPhase::Locks, &self.locks);
        add(
            &mut steps,
            RecoveryPhase::PendingConfirmations,
            &self.pending_confirmations,
        );
        Ok(RecoveryPlan { steps })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CheckpointEnvelope {
    pub checkpoint: SessionCheckpoint,
    pub checksum: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecoveryError {
    CorruptCheckpoint,
    UnsupportedVersion,
    Io,
    LockHeld,
    Serialization,
    InvalidState,
}

#[derive(Clone, Debug, Default)]
pub struct RecoveryStore {
    envelope: Option<CheckpointEnvelope>,
}

impl RecoveryStore {
    pub fn save(&mut self, checkpoint: SessionCheckpoint) {
        if checkpoint.state.validate().is_err() {
            return;
        }
        let checksum = checksum(&checkpoint);
        self.envelope = Some(CheckpointEnvelope {
            checkpoint,
            checksum,
        });
    }
    pub fn load(&self, supported_version: u16) -> Result<Option<SessionCheckpoint>, RecoveryError> {
        let Some(envelope) = &self.envelope else {
            return Ok(None);
        };
        if envelope.checkpoint.schema_version > supported_version {
            return Err(RecoveryError::UnsupportedVersion);
        }
        if checksum(&envelope.checkpoint) != envelope.checksum {
            return Err(RecoveryError::CorruptCheckpoint);
        }
        envelope.checkpoint.state.validate()?;
        Ok(Some(envelope.checkpoint.clone()))
    }
    pub fn clear(&mut self) {
        self.envelope = None;
    }
}

fn checksum(checkpoint: &SessionCheckpoint) -> u64 {
    let bytes = serde_json::to_vec(checkpoint).expect("checkpoint serializes");
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

#[derive(Clone, Debug)]
pub struct RecoveryFileOptions {
    pub supported_version: u16,
    pub backup_retention: usize,
}

impl Default for RecoveryFileOptions {
    fn default() -> Self {
        Self {
            supported_version: 1,
            backup_retention: 2,
        }
    }
}

pub struct FileRecoveryStore {
    path: PathBuf,
    options: RecoveryFileOptions,
}

impl FileRecoveryStore {
    pub fn new(path: impl Into<PathBuf>, options: RecoveryFileOptions) -> Self {
        Self {
            path: path.into(),
            options,
        }
    }

    pub fn save(&self, checkpoint: SessionCheckpoint) -> Result<(), RecoveryError> {
        let lock = self.lock()?;
        let result = self.save_locked(checkpoint);
        drop(lock);
        let _ = fs::remove_file(self.lock_path());
        result
    }

    pub fn load(&self) -> Result<Option<SessionCheckpoint>, RecoveryError> {
        if !self.path.exists() {
            return Ok(None);
        }
        let mut file = File::open(&self.path).map_err(|_| RecoveryError::Io)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|_| RecoveryError::Io)?;
        let envelope: CheckpointEnvelope =
            serde_json::from_slice(&bytes).map_err(|_| RecoveryError::Serialization)?;
        if envelope.checkpoint.schema_version > self.options.supported_version {
            return Err(RecoveryError::UnsupportedVersion);
        }
        if checksum(&envelope.checkpoint) != envelope.checksum {
            return Err(RecoveryError::CorruptCheckpoint);
        }
        envelope.checkpoint.state.validate()?;
        Ok(Some(envelope.checkpoint))
    }

    pub fn restore_backup(&self) -> Result<bool, RecoveryError> {
        let backup = self.backup_path(0);
        if !backup.exists() {
            return Ok(false);
        }
        let mut file = File::open(&backup).map_err(|_| RecoveryError::Io)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|_| RecoveryError::Io)?;
        let envelope: CheckpointEnvelope =
            serde_json::from_slice(&bytes).map_err(|_| RecoveryError::Serialization)?;
        if envelope.checkpoint.schema_version > self.options.supported_version {
            return Err(RecoveryError::UnsupportedVersion);
        }
        if checksum(&envelope.checkpoint) != envelope.checksum {
            return Err(RecoveryError::CorruptCheckpoint);
        }
        envelope.checkpoint.state.validate()?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|_| RecoveryError::Io)?;
        }
        let temporary = self
            .path
            .with_extension(format!("restore-tmp-{}", unique_suffix()));
        let mut restored = File::create(&temporary).map_err(|_| RecoveryError::Io)?;
        restored.write_all(&bytes).map_err(|_| RecoveryError::Io)?;
        restored.sync_all().map_err(|_| RecoveryError::Io)?;
        fs::rename(&temporary, &self.path).map_err(|_| RecoveryError::Io)?;
        Ok(true)
    }

    fn save_locked(&self, checkpoint: SessionCheckpoint) -> Result<(), RecoveryError> {
        if checkpoint.schema_version > self.options.supported_version {
            return Err(RecoveryError::UnsupportedVersion);
        }
        checkpoint.state.validate()?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|_| RecoveryError::Io)?;
        }
        let envelope = CheckpointEnvelope {
            checksum: checksum(&checkpoint),
            checkpoint,
        };
        let bytes = serde_json::to_vec(&envelope).map_err(|_| RecoveryError::Serialization)?;
        let temporary = self.path.with_extension(format!("tmp-{}", unique_suffix()));
        let mut file = File::create(&temporary).map_err(|_| RecoveryError::Io)?;
        file.write_all(&bytes).map_err(|_| RecoveryError::Io)?;
        file.sync_all().map_err(|_| RecoveryError::Io)?;
        if self.path.exists() {
            self.rotate_backups()?;
        }
        fs::rename(&temporary, &self.path).map_err(|_| RecoveryError::Io)
    }

    fn rotate_backups(&self) -> Result<(), RecoveryError> {
        if self.options.backup_retention == 0 {
            return Ok(());
        }
        for index in (1..self.options.backup_retention).rev() {
            let source = self.backup_path(index - 1);
            let destination = self.backup_path(index);
            if source.exists() {
                fs::rename(source, destination).map_err(|_| RecoveryError::Io)?;
            }
        }
        if self.path.exists() {
            fs::copy(&self.path, self.backup_path(0)).map_err(|_| RecoveryError::Io)?;
        }
        Ok(())
    }

    fn lock(&self) -> Result<File, RecoveryError> {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.lock_path())
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    RecoveryError::LockHeld
                } else {
                    RecoveryError::Io
                }
            })
    }
    fn lock_path(&self) -> PathBuf {
        self.path.with_extension("lock")
    }
    fn backup_path(&self, index: usize) -> PathBuf {
        self.path.with_extension(format!("bak{index}"))
    }
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}
