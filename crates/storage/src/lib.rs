//! Origin- and profile-partitioned storage primitives.

use browsai_profiles::ProfileId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum StorageKind {
    Local,
    Session,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageError {
    QuotaExceeded,
}

pub struct StorageWriteRequest {
    pub profile: ProfileId,
    pub origin: String,
    pub namespace: String,
    pub kind: StorageKind,
    pub key: String,
    pub value: String,
    pub quota_bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum StorageEvent {
    Set {
        profile: ProfileId,
        origin: String,
        namespace: String,
        kind: StorageKind,
        key: String,
    },
    Removed {
        profile: ProfileId,
        origin: String,
        namespace: String,
        kind: StorageKind,
        key: String,
    },
    SessionCloned {
        profile: ProfileId,
        origin: String,
        source_namespace: String,
        destination_namespace: String,
    },
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
struct StorageKey {
    profile: ProfileId,
    origin: String,
    namespace: String,
    kind: StorageKind,
}

#[derive(Clone, Debug, Default)]
pub struct OriginStorage {
    values: BTreeMap<StorageKey, BTreeMap<String, String>>,
    generations: BTreeMap<StorageKey, u64>,
    events: Vec<StorageEvent>,
}

#[derive(Serialize, Deserialize)]
struct StorageWire {
    entries: Vec<StorageEntry>,
}

#[derive(Serialize, Deserialize)]
struct StorageEntry {
    profile: ProfileId,
    origin: String,
    namespace: String,
    kind: StorageKind,
    values: BTreeMap<String, String>,
    generation: u64,
}

impl OriginStorage {
    fn key(profile: &ProfileId, origin: &str, namespace: &str, kind: StorageKind) -> StorageKey {
        StorageKey {
            profile: profile.clone(),
            origin: origin.to_owned(),
            namespace: namespace.to_owned(),
            kind,
        }
    }
    pub fn set(
        &mut self,
        profile: &ProfileId,
        origin: &str,
        namespace: &str,
        kind: StorageKind,
        key: impl Into<String>,
        value: impl Into<String>,
    ) {
        let storage_key = Self::key(profile, origin, namespace, kind);
        let key = key.into();
        self.values
            .entry(storage_key.clone())
            .or_default()
            .insert(key.clone(), value.into());
        *self.generations.entry(storage_key).or_default() += 1;
        self.events.push(StorageEvent::Set {
            profile: profile.clone(),
            origin: origin.into(),
            namespace: namespace.into(),
            kind,
            key,
        });
    }

    pub fn set_with_quota(&mut self, request: StorageWriteRequest) -> Result<(), StorageError> {
        let StorageWriteRequest {
            profile,
            origin,
            namespace,
            kind,
            key,
            value,
            quota_bytes,
        } = request;
        let storage_key = Self::key(&profile, &origin, &namespace, kind);
        let current = self
            .values
            .get(&storage_key)
            .map(|values| {
                values
                    .iter()
                    .filter(|(existing_key, _)| existing_key != &&key)
                    .map(|(existing_key, existing_value)| existing_key.len() + existing_value.len())
                    .sum::<usize>()
            })
            .unwrap_or(0);
        if current + key.len() + value.len() > quota_bytes {
            return Err(StorageError::QuotaExceeded);
        }
        self.values
            .entry(storage_key.clone())
            .or_default()
            .insert(key.clone(), value);
        *self.generations.entry(storage_key).or_default() += 1;
        self.events.push(StorageEvent::Set {
            profile,
            origin,
            namespace,
            kind,
            key,
        });
        Ok(())
    }

    pub fn clone_session_namespace(
        &mut self,
        profile: &ProfileId,
        origin: &str,
        source_namespace: &str,
        destination_namespace: &str,
    ) -> bool {
        let source = Self::key(profile, origin, source_namespace, StorageKind::Session);
        let Some(values) = self.values.get(&source).cloned() else {
            return false;
        };
        let destination = Self::key(profile, origin, destination_namespace, StorageKind::Session);
        self.values.insert(destination.clone(), values);
        let source_generation = self.generations.get(&source).copied().unwrap_or(0);
        self.generations.insert(destination, source_generation);
        self.events.push(StorageEvent::SessionCloned {
            profile: profile.clone(),
            origin: origin.to_owned(),
            source_namespace: source_namespace.to_owned(),
            destination_namespace: destination_namespace.to_owned(),
        });
        true
    }
    pub fn get(
        &self,
        profile: &ProfileId,
        origin: &str,
        namespace: &str,
        kind: StorageKind,
        key: &str,
    ) -> Option<&str> {
        self.values
            .get(&Self::key(profile, origin, namespace, kind))
            .and_then(|values| values.get(key))
            .map(String::as_str)
    }
    pub fn remove(
        &mut self,
        profile: &ProfileId,
        origin: &str,
        namespace: &str,
        kind: StorageKind,
        key: &str,
    ) -> Option<String> {
        let storage_key = Self::key(profile, origin, namespace, kind);
        let value = self
            .values
            .get_mut(&storage_key)
            .and_then(|values| values.remove(key));
        if value.is_some() {
            *self.generations.entry(storage_key).or_default() += 1;
            self.events.push(StorageEvent::Removed {
                profile: profile.clone(),
                origin: origin.to_owned(),
                namespace: namespace.to_owned(),
                kind,
                key: key.to_owned(),
            });
        }
        value
    }
    pub fn generation(
        &self,
        profile: &ProfileId,
        origin: &str,
        namespace: &str,
        kind: StorageKind,
    ) -> u64 {
        self.generations
            .get(&Self::key(profile, origin, namespace, kind))
            .copied()
            .unwrap_or(0)
    }

    pub fn drain_events(&mut self) -> impl Iterator<Item = StorageEvent> + '_ {
        self.events.drain(..)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        let mut keys = std::collections::BTreeSet::new();
        keys.extend(self.values.keys().cloned());
        keys.extend(self.generations.keys().cloned());
        let entries = keys
            .into_iter()
            .map(|key| StorageEntry {
                profile: key.profile.clone(),
                origin: key.origin.clone(),
                namespace: key.namespace.clone(),
                kind: key.kind,
                values: self.values.get(&key).cloned().unwrap_or_default(),
                generation: self.generations.get(&key).copied().unwrap_or(0),
            })
            .collect();
        serde_json::to_string(&StorageWire { entries })
    }

    pub fn from_json(value: &str) -> Result<Self, serde_json::Error> {
        let wire: StorageWire = serde_json::from_str(value)?;
        let mut storage = Self::default();
        for entry in wire.entries {
            let key = Self::key(&entry.profile, &entry.origin, &entry.namespace, entry.kind);
            if !entry.values.is_empty() {
                storage.values.insert(key.clone(), entry.values);
            }
            storage.generations.insert(key, entry.generation);
        }
        Ok(storage)
    }
}
