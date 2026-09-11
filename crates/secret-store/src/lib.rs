//! Capability-scoped secret broker. Handles and capabilities are safe to pass
//! across an agent boundary; secret bytes are only available inside a broker
//! callback and are zeroized on drop.

use browsai_profiles::ProfileId;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;
use zeroize::Zeroizing;

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct SecretHandle(Uuid);

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct CapabilityReference(Uuid);

#[derive(Clone, Debug)]
pub struct SecretCapability {
    pub reference: CapabilityReference,
    pub handle: SecretHandle,
    pub profile: ProfileId,
    pub origin: String,
    pub purpose: String,
    pub one_time: bool,
    pub expires_at_unix_seconds: Option<u64>,
}

#[derive(Debug, PartialEq)]
pub enum SecretError {
    UnknownHandle,
    InvalidCapability,
    OriginMismatch,
    ProfileMismatch,
    AlreadyConsumed,
    ExpiredCapability,
    InvalidSecret,
    CorruptRecord,
    UnsupportedSnapshotVersion,
    ConsentRequired,
    PolicyDenied,
}

/// Broker-side issuance constraints. `None` means unrestricted for that
/// dimension; callers can still require explicit consent per grant.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SecretPolicy {
    allowed_purposes: Option<BTreeSet<String>>,
    max_lifetime_seconds: Option<u64>,
}

impl SecretPolicy {
    pub fn allow_purposes<I, S>(purposes: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            allowed_purposes: Some(purposes.into_iter().map(Into::into).collect()),
            max_lifetime_seconds: None,
        }
    }

    pub fn with_max_lifetime_seconds(mut self, seconds: u64) -> Self {
        self.max_lifetime_seconds = Some(seconds);
        self
    }
}

/// Non-sensitive lifecycle facts emitted by the broker.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SecretEvent {
    Stored {
        profile: ProfileId,
        origin: String,
    },
    Granted {
        profile: ProfileId,
        origin: String,
        purpose: String,
        one_time: bool,
    },
    Released {
        profile: ProfileId,
        origin: String,
        purpose: String,
        one_time: bool,
    },
    Expired {
        profile: ProfileId,
        origin: String,
        purpose: String,
    },
    Revoked {
        profile: ProfileId,
        origin: String,
    },
}

struct SecretRecord {
    profile: ProfileId,
    origin: String,
    nonce: [u8; 12],
    ciphertext: Vec<u8>,
}

const SNAPSHOT_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PersistedSecretRecord {
    handle: SecretHandle,
    profile: ProfileId,
    origin: String,
    nonce: [u8; 12],
    ciphertext: Vec<u8>,
}

/// Encrypted broker state suitable for persistence or recovery. The key is
/// intentionally not part of this structure and must be supplied out of band.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SecretStoreSnapshot {
    pub schema_version: u32,
    records: Vec<PersistedSecretRecord>,
    consumed: Vec<CapabilityReference>,
    #[serde(default)]
    policy: SecretPolicy,
}

pub struct SecretBroker {
    records: HashMap<SecretHandle, SecretRecord>,
    consumed: std::collections::HashSet<CapabilityReference>,
    events: Vec<SecretEvent>,
    key: Zeroizing<[u8; 32]>,
    policy: SecretPolicy,
}

impl Default for SecretBroker {
    fn default() -> Self {
        let mut key = [0u8; 32];
        key[..16].copy_from_slice(Uuid::new_v4().as_bytes());
        key[16..].copy_from_slice(Uuid::new_v4().as_bytes());
        Self::new(key)
    }
}

impl SecretBroker {
    pub fn new(key: [u8; 32]) -> Self {
        Self {
            records: HashMap::new(),
            consumed: std::collections::HashSet::new(),
            events: Vec::new(),
            key: Zeroizing::new(key),
            policy: SecretPolicy::default(),
        }
    }

    pub fn with_policy(key: [u8; 32], policy: SecretPolicy) -> Self {
        let mut broker = Self::new(key);
        broker.policy = policy;
        broker
    }

    pub fn set_policy(&mut self, policy: SecretPolicy) {
        self.policy = policy;
    }

    pub fn export_snapshot(&self) -> SecretStoreSnapshot {
        SecretStoreSnapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            records: self
                .records
                .iter()
                .map(|(handle, record)| PersistedSecretRecord {
                    handle: handle.clone(),
                    profile: record.profile.clone(),
                    origin: record.origin.clone(),
                    nonce: record.nonce,
                    ciphertext: record.ciphertext.clone(),
                })
                .collect(),
            consumed: self.consumed.iter().cloned().collect(),
            policy: self.policy.clone(),
        }
    }

    pub fn from_snapshot(
        snapshot: SecretStoreSnapshot,
        key: [u8; 32],
    ) -> Result<Self, SecretError> {
        if snapshot.schema_version != SNAPSHOT_SCHEMA_VERSION {
            return Err(SecretError::UnsupportedSnapshotVersion);
        }
        let mut broker = Self::new(key);
        for persisted in snapshot.records {
            broker.records.insert(
                persisted.handle,
                SecretRecord {
                    profile: persisted.profile,
                    origin: persisted.origin,
                    nonce: persisted.nonce,
                    ciphertext: persisted.ciphertext,
                },
            );
        }
        broker.consumed = snapshot.consumed.into_iter().collect();
        broker.policy = snapshot.policy;
        Ok(broker)
    }

    pub fn store(
        &mut self,
        profile: ProfileId,
        origin: impl Into<String>,
        secret: impl AsRef<[u8]>,
    ) -> SecretHandle {
        let handle = SecretHandle(Uuid::new_v4());
        let nonce = nonce_for(&handle);
        let cipher = ChaCha20Poly1305::new_from_slice(self.key.as_ref())
            .expect("ChaCha20Poly1305 accepts a 32-byte key");
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce), secret.as_ref())
            .expect("encryption with a valid nonce cannot fail");
        self.records.insert(
            handle.clone(),
            SecretRecord {
                profile: profile.clone(),
                origin: origin.into(),
                nonce,
                ciphertext,
            },
        );
        self.events.push(SecretEvent::Stored {
            profile,
            origin: self.records[&handle].origin.clone(),
        });
        handle
    }
    pub fn grant(
        &mut self,
        handle: &SecretHandle,
        profile: ProfileId,
        origin: impl Into<String>,
        purpose: impl Into<String>,
        one_time: bool,
    ) -> Result<SecretCapability, SecretError> {
        self.grant_for_with_consent(handle, profile, origin, purpose, one_time, None, true)
    }

    pub fn grant_with_consent(
        &mut self,
        handle: &SecretHandle,
        profile: ProfileId,
        origin: impl Into<String>,
        purpose: impl Into<String>,
        one_time: bool,
        consent: bool,
    ) -> Result<SecretCapability, SecretError> {
        self.grant_for_with_consent(handle, profile, origin, purpose, one_time, None, consent)
    }

    pub fn revoke(&mut self, handle: &SecretHandle) -> bool {
        let Some(record) = self.records.remove(handle) else {
            return false;
        };
        self.events.push(SecretEvent::Revoked {
            profile: record.profile,
            origin: record.origin,
        });
        true
    }

    pub fn grant_for(
        &mut self,
        handle: &SecretHandle,
        profile: ProfileId,
        origin: impl Into<String>,
        purpose: impl Into<String>,
        one_time: bool,
        lifetime: Option<Duration>,
    ) -> Result<SecretCapability, SecretError> {
        self.grant_for_with_consent(handle, profile, origin, purpose, one_time, lifetime, true)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn grant_for_with_consent(
        &mut self,
        handle: &SecretHandle,
        profile: ProfileId,
        origin: impl Into<String>,
        purpose: impl Into<String>,
        one_time: bool,
        lifetime: Option<Duration>,
        consent: bool,
    ) -> Result<SecretCapability, SecretError> {
        if !consent {
            return Err(SecretError::ConsentRequired);
        }
        let record = self.records.get(handle).ok_or(SecretError::UnknownHandle)?;
        let origin = origin.into();
        if record.profile != profile {
            return Err(SecretError::ProfileMismatch);
        }
        if record.origin != origin {
            return Err(SecretError::OriginMismatch);
        }
        let purpose = purpose.into();
        if self
            .policy
            .allowed_purposes
            .as_ref()
            .is_some_and(|allowed| !allowed.contains(&purpose))
        {
            return Err(SecretError::PolicyDenied);
        }
        if self
            .policy
            .max_lifetime_seconds
            .is_some_and(|maximum| lifetime.is_some_and(|value| value.as_secs() > maximum))
        {
            return Err(SecretError::PolicyDenied);
        }
        let capability = SecretCapability {
            reference: CapabilityReference(Uuid::new_v4()),
            handle: handle.clone(),
            profile,
            origin,
            purpose,
            one_time,
            expires_at_unix_seconds: lifetime
                .map(|duration| now_seconds().saturating_add(duration.as_secs())),
        };
        self.events.push(SecretEvent::Granted {
            profile: capability.profile.clone(),
            origin: capability.origin.clone(),
            purpose: capability.purpose.clone(),
            one_time: capability.one_time,
        });
        Ok(capability)
    }
    pub fn with_secret<T>(
        &mut self,
        capability: &SecretCapability,
        profile: &ProfileId,
        origin: &str,
        use_secret: impl FnOnce(&[u8]) -> T,
    ) -> Result<T, SecretError> {
        if &capability.profile != profile {
            return Err(SecretError::ProfileMismatch);
        }
        if capability.origin != origin {
            return Err(SecretError::OriginMismatch);
        }
        if capability.one_time && self.consumed.contains(&capability.reference) {
            return Err(SecretError::AlreadyConsumed);
        }
        if capability
            .expires_at_unix_seconds
            .is_some_and(|expires| expires <= now_seconds())
        {
            self.events.push(SecretEvent::Expired {
                profile: capability.profile.clone(),
                origin: capability.origin.clone(),
                purpose: capability.purpose.clone(),
            });
            return Err(SecretError::ExpiredCapability);
        }
        let record = self
            .records
            .get(&capability.handle)
            .ok_or(SecretError::InvalidCapability)?;
        if record.profile != capability.profile {
            return Err(SecretError::ProfileMismatch);
        }
        if record.origin != capability.origin {
            return Err(SecretError::OriginMismatch);
        }
        let cipher = ChaCha20Poly1305::new_from_slice(self.key.as_ref())
            .expect("ChaCha20Poly1305 accepts a 32-byte key");
        let plaintext = Zeroizing::new(
            cipher
                .decrypt(Nonce::from_slice(&record.nonce), record.ciphertext.as_ref())
                .map_err(|_| SecretError::CorruptRecord)?,
        );
        let result = use_secret(&plaintext);
        if capability.one_time {
            self.consumed.insert(capability.reference.clone());
        }
        self.events.push(SecretEvent::Released {
            profile: capability.profile.clone(),
            origin: capability.origin.clone(),
            purpose: capability.purpose.clone(),
            one_time: capability.one_time,
        });
        Ok(result)
    }

    pub fn drain_events(&mut self) -> Vec<SecretEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn reference_token(capability: &SecretCapability) -> String {
        capability.reference.0.to_string()
    }
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn nonce_for(handle: &SecretHandle) -> [u8; 12] {
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&handle.0.as_bytes()[..12]);
    nonce
}
