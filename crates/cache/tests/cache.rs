use browsai_cache::{CacheStorage, CachedResponse, HttpCache};
use browsai_profiles::ProfileManager;
use std::collections::BTreeMap;
use url::Url;

#[test]
fn cache_is_partitioned_and_invalidatable() {
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let url = Url::parse("https://example.test/").unwrap();
    let mut cache = HttpCache::with_capacity(2);
    cache.put(
        &profile,
        "https://example.test",
        &url,
        CachedResponse {
            status: 200,
            headers: BTreeMap::new(),
            body: vec![1],
        },
    );
    assert!(cache.get(&profile, "https://other.test", &url).is_none());
    assert_eq!(
        cache
            .get(&profile, "https://example.test", &url)
            .unwrap()
            .body,
        vec![1]
    );
    cache.invalidate(&profile, "https://example.test", &url);
    assert!(cache.get(&profile, "https://example.test", &url).is_none());
}

#[test]
fn cache_capacity_grows_without_evicting_and_partitions_clear_as_a_unit() {
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let first = Url::parse("https://example.test/first").unwrap();
    let second = Url::parse("https://example.test/second").unwrap();
    let response = || CachedResponse {
        status: 200,
        headers: BTreeMap::new(),
        body: vec![1],
    };
    let mut cache = HttpCache::with_capacity(1);
    cache.put(&profile, "https://example.test", &first, response());
    cache.put(&profile, "https://example.test", &first, response());
    assert_eq!(cache.len(), 1);
    cache.put(&profile, "https://other.test", &second, response());
    assert_eq!(cache.len(), 2);
    assert!(cache
        .get(&profile, "https://example.test", &first)
        .is_some());
    assert!(cache.get(&profile, "https://other.test", &second).is_some());
    cache.invalidate_partition(&profile, "https://other.test");
    assert!(cache
        .get(&profile, "https://example.test", &first)
        .is_some());
    cache.invalidate_partition(&profile, "https://example.test");
    assert!(cache.is_empty());
}

#[test]
fn cache_round_trips_persisted_partitioned_entries() {
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let url = Url::parse("https://example.test/data").unwrap();
    let mut cache = HttpCache::with_capacity(4);
    cache.put(
        &profile,
        "https://example.test",
        &url,
        CachedResponse {
            status: 200,
            headers: [("content-type".into(), "application/json".into())]
                .into_iter()
                .collect(),
            body: b"{}".to_vec(),
        },
    );
    let restored = HttpCache::from_json(&cache.to_json().unwrap()).unwrap();
    assert_eq!(
        restored.get(&profile, "https://example.test", &url),
        cache.get(&profile, "https://example.test", &url)
    );
}

#[test]
fn default_cache_retains_multiple_entries() {
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let first = Url::parse("https://example.test/first").unwrap();
    let second = Url::parse("https://example.test/second").unwrap();
    let response = || CachedResponse {
        status: 200,
        headers: BTreeMap::new(),
        body: vec![1],
    };
    let mut cache = HttpCache::default();
    cache.put(&profile, "https://example.test", &first, response());
    cache.put(&profile, "https://example.test", &second, response());
    assert_eq!(cache.len(), 2);
    assert!(cache
        .get(&profile, "https://example.test", &first)
        .is_some());
}

#[test]
fn named_cache_storage_is_partitioned_and_supports_cache_api_operations() {
    let mut profiles = ProfileManager::default();
    let profile = profiles.create("work");
    let url = Url::parse("https://example.test/data").unwrap();
    let response = CachedResponse {
        status: 200,
        headers: BTreeMap::new(),
        body: b"cached".to_vec(),
    };
    let mut storage = CacheStorage::default();
    storage.open(&profile, "https://example.test", "runtime", 1);
    assert!(storage.put(
        &profile,
        "https://example.test",
        "runtime",
        &url,
        response.clone()
    ));
    assert_eq!(
        storage
            .match_response(&profile, "https://example.test", "runtime", &url)
            .unwrap()
            .body,
        b"cached"
    );
    assert!(storage
        .match_response(&profile, "https://other.test", "runtime", &url)
        .is_none());
    assert_eq!(
        storage.names(&profile, "https://example.test"),
        vec!["runtime"]
    );
    assert!(storage.delete_response(&profile, "https://example.test", "runtime", &url));
    assert!(storage.delete_cache(&profile, "https://example.test", "runtime"));
    assert!(storage.names(&profile, "https://example.test").is_empty());
}

#[test]
fn named_cache_storage_round_trips_persisted_namespaces() {
    let profile = ProfileManager::default().create("work");
    let url = Url::parse("https://example.test/data").unwrap();
    let mut storage = CacheStorage::default();
    storage.open(&profile, "https://example.test", "runtime", 4);
    assert!(storage.put(
        &profile,
        "https://example.test",
        "runtime",
        &url,
        CachedResponse {
            status: 201,
            headers: BTreeMap::new(),
            body: b"persisted".to_vec(),
        }
    ));
    let restored = CacheStorage::from_json(&storage.to_json().unwrap()).unwrap();
    assert_eq!(
        restored.names(&profile, "https://example.test"),
        vec!["runtime".to_string()]
    );
    assert_eq!(
        restored
            .match_response(&profile, "https://example.test", "runtime", &url)
            .unwrap()
            .body,
        b"persisted"
    );
}
