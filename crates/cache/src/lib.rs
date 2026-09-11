use browsai_profiles::ProfileId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use url::Url;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CachedResponse {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct HttpCache {
    values: BTreeMap<(ProfileId, String, String), CachedResponse>,
    max_entries: usize,
}

impl Default for HttpCache {
    fn default() -> Self {
        Self::with_capacity(256)
    }
}

#[derive(Clone, Debug, Default)]
pub struct CacheStorage {
    caches: BTreeMap<(ProfileId, String, String), HttpCache>,
}

impl CacheStorage {
    pub fn open(&mut self, profile: &ProfileId, partition: &str, name: &str, max_entries: usize) {
        self.caches
            .entry((profile.clone(), partition.to_owned(), name.to_owned()))
            .or_insert_with(|| HttpCache::with_capacity(max_entries));
    }

    pub fn put(
        &mut self,
        profile: &ProfileId,
        partition: &str,
        name: &str,
        url: &Url,
        response: CachedResponse,
    ) -> bool {
        let Some(cache) =
            self.caches
                .get_mut(&(profile.clone(), partition.to_owned(), name.to_owned()))
        else {
            return false;
        };
        cache.put(profile, partition, url, response);
        true
    }

    pub fn match_response(
        &self,
        profile: &ProfileId,
        partition: &str,
        name: &str,
        url: &Url,
    ) -> Option<&CachedResponse> {
        self.caches
            .get(&(profile.clone(), partition.to_owned(), name.to_owned()))?
            .get(profile, partition, url)
    }

    pub fn delete_response(
        &mut self,
        profile: &ProfileId,
        partition: &str,
        name: &str,
        url: &Url,
    ) -> bool {
        let Some(cache) =
            self.caches
                .get_mut(&(profile.clone(), partition.to_owned(), name.to_owned()))
        else {
            return false;
        };
        let existed = cache.get(profile, partition, url).is_some();
        cache.invalidate(profile, partition, url);
        existed
    }

    pub fn delete_cache(&mut self, profile: &ProfileId, partition: &str, name: &str) -> bool {
        self.caches
            .remove(&(profile.clone(), partition.to_owned(), name.to_owned()))
            .is_some()
    }

    pub fn names(&self, profile: &ProfileId, partition: &str) -> Vec<String> {
        self.caches
            .keys()
            .filter(|(entry_profile, entry_partition, _)| {
                entry_profile == profile && entry_partition == partition
            })
            .map(|(_, _, name)| name.clone())
            .collect()
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        let mut caches = Vec::new();
        for ((profile, partition, name), cache) in &self.caches {
            let entries = cache
                .values
                .iter()
                .map(|((_, _, url), response)| CacheEntry {
                    profile: profile.clone(),
                    partition: partition.clone(),
                    url: url.clone(),
                    response: response.clone(),
                })
                .collect();
            caches.push(NamedCache {
                profile: profile.clone(),
                partition: partition.clone(),
                name: name.clone(),
                max_entries: cache.max_entries,
                entries,
            });
        }
        serde_json::to_string(&CacheStorageWire { caches })
    }

    pub fn from_json(value: &str) -> Result<Self, serde_json::Error> {
        let wire: CacheStorageWire = serde_json::from_str(value)?;
        let mut storage = Self::default();
        for cache in wire.caches {
            let key = (
                cache.profile.clone(),
                cache.partition.clone(),
                cache.name.clone(),
            );
            let mut restored = HttpCache::with_capacity(cache.max_entries);
            for entry in cache.entries {
                let url = Url::parse(&entry.url).map_err(|error| {
                    serde_json::Error::io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("invalid persisted cache URL: {error}"),
                    ))
                })?;
                restored.put(&entry.profile, &entry.partition, &url, entry.response);
            }
            storage.caches.insert(key, restored);
        }
        Ok(storage)
    }
}

impl HttpCache {
    pub fn with_capacity(max_entries: usize) -> Self {
        Self {
            values: BTreeMap::new(),
            max_entries: max_entries.max(1),
        }
    }
    pub fn put(
        &mut self,
        profile: &ProfileId,
        partition: &str,
        url: &Url,
        response: CachedResponse,
    ) {
        let key = (
            profile.clone(),
            partition.to_owned(),
            url.as_str().to_owned(),
        );
        if !self.values.contains_key(&key) && self.values.len() >= self.max_entries {
            // Grow instead of evicting live browser data. Geometric growth keeps
            // insertion amortized O(1) while retaining a small initial footprint.
            self.max_entries = self
                .max_entries
                .saturating_mul(2)
                .max(self.values.len().saturating_add(1));
        }
        self.values.insert(key, response);
    }
    pub fn get(&self, profile: &ProfileId, partition: &str, url: &Url) -> Option<&CachedResponse> {
        self.values.get(&(
            profile.clone(),
            partition.to_owned(),
            url.as_str().to_owned(),
        ))
    }
    pub fn invalidate(&mut self, profile: &ProfileId, partition: &str, url: &Url) {
        self.values.remove(&(
            profile.clone(),
            partition.to_owned(),
            url.as_str().to_owned(),
        ));
    }

    pub fn invalidate_partition(&mut self, profile: &ProfileId, partition: &str) {
        self.values
            .retain(|(entry_profile, entry_partition, _), _| {
                entry_profile != profile || entry_partition != partition
            });
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        let entries = self
            .values
            .iter()
            .map(|((profile, partition, url), response)| CacheEntry {
                profile: profile.clone(),
                partition: partition.clone(),
                url: url.clone(),
                response: response.clone(),
            })
            .collect();
        serde_json::to_string(&CacheWire {
            max_entries: self.max_entries,
            entries,
        })
    }

    pub fn from_json(value: &str) -> Result<Self, serde_json::Error> {
        let wire: CacheWire = serde_json::from_str(value)?;
        let max_entries = wire.max_entries.max(1);
        let mut cache = Self {
            values: wire
                .entries
                .into_iter()
                .map(|entry| ((entry.profile, entry.partition, entry.url), entry.response))
                .collect(),
            max_entries,
        };
        // Older snapshots may contain more entries than their recorded limit;
        // retain them and let the adaptive capacity catch up.
        cache.max_entries = cache.max_entries.max(cache.values.len());
        Ok(cache)
    }
}

#[derive(Serialize, Deserialize)]
struct CacheWire {
    max_entries: usize,
    entries: Vec<CacheEntry>,
}

#[derive(Serialize, Deserialize)]
struct CacheEntry {
    profile: ProfileId,
    partition: String,
    url: String,
    response: CachedResponse,
}

#[derive(Serialize, Deserialize)]
struct CacheStorageWire {
    caches: Vec<NamedCache>,
}

#[derive(Serialize, Deserialize)]
struct NamedCache {
    profile: ProfileId,
    partition: String,
    name: String,
    max_entries: usize,
    entries: Vec<CacheEntry>,
}
