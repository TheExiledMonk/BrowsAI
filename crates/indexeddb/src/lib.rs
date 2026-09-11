use browsai_profiles::ProfileId;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
struct DatabaseKey {
    profile: ProfileId,
    origin: String,
    name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DatabaseInfo {
    pub name: String,
    pub version: u64,
    pub stores: BTreeSet<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IndexedDbError {
    NotFound,
    VersionTooLow,
    StoreMissing,
    TransactionInactive,
    QuotaExceeded,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum IndexedDbEvent {
    Opened {
        profile: ProfileId,
        origin: String,
        name: String,
        version: u64,
    },
    Upgraded {
        profile: ProfileId,
        origin: String,
        name: String,
        from_version: u64,
        to_version: u64,
    },
    Committed {
        profile: ProfileId,
        origin: String,
        name: String,
    },
    Aborted {
        profile: ProfileId,
        origin: String,
        name: String,
    },
}

#[derive(Clone, Debug, Default)]
struct Database {
    version: u64,
    stores: BTreeMap<String, BTreeMap<String, Vec<u8>>>,
}

#[derive(Clone, Debug)]
pub struct IndexedDb {
    databases: BTreeMap<DatabaseKey, Database>,
    max_bytes: usize,
    events: Vec<IndexedDbEvent>,
}

impl Default for IndexedDb {
    fn default() -> Self {
        Self::with_quota(16 * 1024 * 1024)
    }
}

pub struct Transaction<'a> {
    database: &'a mut Database,
    writes: BTreeMap<(String, String), Option<Vec<u8>>>,
    active: bool,
    max_bytes: usize,
    events: &'a mut Vec<IndexedDbEvent>,
    profile: ProfileId,
    origin: String,
    name: String,
}

impl IndexedDb {
    pub fn with_quota(max_bytes: usize) -> Self {
        Self {
            databases: BTreeMap::new(),
            max_bytes: max_bytes.max(1),
            events: Vec::new(),
        }
    }
    pub fn open(
        &mut self,
        profile: &ProfileId,
        origin: &str,
        name: &str,
        requested_version: u64,
        stores: &[String],
    ) -> Result<DatabaseInfo, IndexedDbError> {
        let key = DatabaseKey {
            profile: profile.clone(),
            origin: origin.to_owned(),
            name: name.to_owned(),
        };
        let (previous_version, version, store_names) = {
            let database = self.databases.entry(key).or_default();
            if requested_version < database.version {
                return Err(IndexedDbError::VersionTooLow);
            }
            let previous_version = database.version;
            database.version = requested_version.max(database.version).max(1);
            for store in stores {
                database.stores.entry(store.clone()).or_default();
            }
            (
                previous_version,
                database.version,
                database.stores.keys().cloned().collect::<BTreeSet<_>>(),
            )
        };
        if version > previous_version {
            self.events.push(IndexedDbEvent::Upgraded {
                profile: profile.clone(),
                origin: origin.to_owned(),
                name: name.to_owned(),
                from_version: previous_version,
                to_version: version,
            });
        } else {
            self.events.push(IndexedDbEvent::Opened {
                profile: profile.clone(),
                origin: origin.to_owned(),
                name: name.to_owned(),
                version,
            });
        }
        Ok(DatabaseInfo {
            name: name.to_owned(),
            version,
            stores: store_names,
        })
    }
    pub fn transaction<'a>(
        &'a mut self,
        profile: &ProfileId,
        origin: &str,
        name: &str,
    ) -> Result<Transaction<'a>, IndexedDbError> {
        let database = self
            .databases
            .get_mut(&DatabaseKey {
                profile: profile.clone(),
                origin: origin.to_owned(),
                name: name.to_owned(),
            })
            .ok_or(IndexedDbError::NotFound)?;
        Ok(Transaction {
            database,
            writes: BTreeMap::new(),
            active: true,
            max_bytes: self.max_bytes,
            events: &mut self.events,
            profile: profile.clone(),
            origin: origin.to_owned(),
            name: name.to_owned(),
        })
    }

    pub fn drain_events(&mut self) -> impl Iterator<Item = IndexedDbEvent> + '_ {
        self.events.drain(..)
    }
}

impl<'a> Transaction<'a> {
    pub fn get(&self, store: &str, key: &str) -> Result<Option<Vec<u8>>, IndexedDbError> {
        if !self.active {
            return Err(IndexedDbError::TransactionInactive);
        }
        Ok(self
            .database
            .stores
            .get(store)
            .ok_or(IndexedDbError::StoreMissing)?
            .get(key)
            .cloned())
    }
    pub fn put(
        &mut self,
        store: &str,
        key: impl Into<String>,
        value: Vec<u8>,
    ) -> Result<(), IndexedDbError> {
        if !self.active {
            return Err(IndexedDbError::TransactionInactive);
        }
        if !self.database.stores.contains_key(store) {
            return Err(IndexedDbError::StoreMissing);
        }
        self.writes
            .insert((store.to_owned(), key.into()), Some(value));
        Ok(())
    }
    pub fn delete(&mut self, store: &str, key: impl Into<String>) -> Result<(), IndexedDbError> {
        if !self.active {
            return Err(IndexedDbError::TransactionInactive);
        }
        if !self.database.stores.contains_key(store) {
            return Err(IndexedDbError::StoreMissing);
        }
        self.writes.insert((store.to_owned(), key.into()), None);
        Ok(())
    }
    pub fn commit(mut self) -> Result<(), IndexedDbError> {
        let mut proposed = self.database.clone();
        for ((store, key), value) in &self.writes {
            let values = proposed
                .stores
                .get_mut(store)
                .ok_or(IndexedDbError::StoreMissing)?;
            match value {
                Some(value) => {
                    values.insert(key.clone(), value.clone());
                }
                None => {
                    values.remove(key);
                }
            }
        }
        let bytes: usize = proposed
            .stores
            .values()
            .flat_map(|store| store.values())
            .map(Vec::len)
            .sum();
        if bytes > self.max_bytes {
            return Err(IndexedDbError::QuotaExceeded);
        }
        for ((store, key), value) in self.writes {
            let values = self
                .database
                .stores
                .get_mut(&store)
                .ok_or(IndexedDbError::StoreMissing)?;
            match value {
                Some(value) => {
                    values.insert(key, value);
                }
                None => {
                    values.remove(&key);
                }
            }
        }
        self.active = false;
        self.events.push(IndexedDbEvent::Committed {
            profile: self.profile.clone(),
            origin: self.origin.clone(),
            name: self.name.clone(),
        });
        Ok(())
    }
    pub fn abort(mut self) {
        self.active = false;
        self.events.push(IndexedDbEvent::Aborted {
            profile: self.profile.clone(),
            origin: self.origin.clone(),
            name: self.name.clone(),
        });
    }
}
