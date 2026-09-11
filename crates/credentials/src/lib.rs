use browsai_profiles::ProfileId;
use browsai_secret_store::{
    SecretBroker, SecretCapability, SecretError, SecretHandle, SecretStoreSnapshot,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct CredentialId(Uuid);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CredentialRecord {
    pub id: CredentialId,
    pub profile: ProfileId,
    pub origin: String,
    pub username: String,
    password_handle: SecretHandle,
}

/// Versioned credential metadata plus authenticated encrypted secret records.
/// The encryption key is deliberately supplied out of band to restore.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CredentialSnapshot {
    pub schema_version: u32,
    pub records: Vec<CredentialRecord>,
    pub secrets: SecretStoreSnapshot,
}

const SNAPSHOT_SCHEMA_VERSION: u32 = 1;

#[derive(Default)]
pub struct CredentialBroker {
    secrets: SecretBroker,
    records: BTreeMap<CredentialId, CredentialRecord>,
}

impl CredentialBroker {
    pub fn new(key: [u8; 32]) -> Self {
        Self {
            secrets: SecretBroker::new(key),
            records: BTreeMap::new(),
        }
    }

    pub fn export_snapshot(&self) -> CredentialSnapshot {
        CredentialSnapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            records: self.records.values().cloned().collect(),
            secrets: self.secrets.export_snapshot(),
        }
    }

    pub fn from_snapshot(snapshot: CredentialSnapshot, key: [u8; 32]) -> Result<Self, SecretError> {
        if snapshot.schema_version != SNAPSHOT_SCHEMA_VERSION {
            return Err(SecretError::UnsupportedSnapshotVersion);
        }
        let secrets = SecretBroker::from_snapshot(snapshot.secrets, key)?;
        let records = snapshot
            .records
            .into_iter()
            .map(|record| (record.id.clone(), record))
            .collect();
        Ok(Self { secrets, records })
    }

    pub fn save_password(
        &mut self,
        profile: ProfileId,
        origin: impl Into<String>,
        username: impl Into<String>,
        password: impl AsRef<[u8]>,
    ) -> CredentialId {
        let origin = origin.into();
        let handle = self
            .secrets
            .store(profile.clone(), origin.clone(), password);
        let id = CredentialId(Uuid::new_v4());
        self.records.insert(
            id.clone(),
            CredentialRecord {
                id: id.clone(),
                profile,
                origin,
                username: username.into(),
                password_handle: handle,
            },
        );
        id
    }
    pub fn find(&self, profile: &ProfileId, origin: &str) -> Vec<&CredentialRecord> {
        self.records
            .values()
            .filter(|record| &record.profile == profile && record.origin == origin)
            .collect()
    }

    pub fn update_password(
        &mut self,
        id: &CredentialId,
        password: impl AsRef<[u8]>,
    ) -> Result<(), SecretError> {
        let record = self.records.get(id).ok_or(SecretError::UnknownHandle)?;
        let old_handle = record.password_handle.clone();
        let new_handle =
            self.secrets
                .store(record.profile.clone(), record.origin.clone(), password);
        self.secrets.revoke(&old_handle);
        self.records
            .get_mut(id)
            .expect("credential checked above")
            .password_handle = new_handle;
        Ok(())
    }
    pub fn grant_password(
        &mut self,
        id: &CredentialId,
        profile: ProfileId,
        origin: &str,
        purpose: impl Into<String>,
    ) -> Result<SecretCapability, SecretError> {
        self.grant_password_with_consent(id, profile, origin, purpose, true)
    }

    pub fn grant_password_with_consent(
        &mut self,
        id: &CredentialId,
        profile: ProfileId,
        origin: &str,
        purpose: impl Into<String>,
        consent: bool,
    ) -> Result<SecretCapability, SecretError> {
        let record = self.records.get(id).ok_or(SecretError::UnknownHandle)?;
        if record.profile != profile {
            return Err(SecretError::ProfileMismatch);
        }
        if record.origin != origin {
            return Err(SecretError::OriginMismatch);
        }
        self.secrets.grant_with_consent(
            &record.password_handle,
            profile,
            origin,
            purpose,
            true,
            consent,
        )
    }
    pub fn with_password<T>(
        &mut self,
        capability: &SecretCapability,
        profile: &ProfileId,
        origin: &str,
        use_password: impl FnOnce(&[u8]) -> T,
    ) -> Result<T, SecretError> {
        self.secrets
            .with_secret(capability, profile, origin, use_password)
    }
}
