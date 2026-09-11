use browsai_service_worker::{FetchResponse, ServiceWorkerRegistry, WorkerState};
use url::Url;

#[test]
fn service_worker_registration_and_fetch_interception_are_scoped_by_url() {
    let scope = Url::parse("https://example.test/app/").unwrap();
    let script = Url::parse("https://example.test/sw.js").unwrap();
    let url = Url::parse("https://example.test/app/data").unwrap();
    let mut registry = ServiceWorkerRegistry::default();
    assert_eq!(
        registry.register(scope, script).state,
        WorkerState::Activated
    );
    registry.intercept(
        &url,
        FetchResponse {
            status: 200,
            body: b"cached".to_vec(),
        },
    );
    assert_eq!(registry.fetch(&url).unwrap().body, b"cached");
    let nested = Url::parse("https://example.test/app/nested/data").unwrap();
    registry.intercept(
        &nested,
        FetchResponse {
            status: 200,
            body: b"nested".to_vec(),
        },
    );
    assert_eq!(registry.fetch(&nested).unwrap().body, b"nested");
    let outside_scope = Url::parse("https://example.test/other/data").unwrap();
    registry.intercept(
        &outside_scope,
        FetchResponse {
            status: 200,
            body: b"must-not-intercept".to_vec(),
        },
    );
    assert!(registry.fetch(&outside_scope).is_none());
    assert!(registry
        .fetch(&Url::parse("https://other.test/data").unwrap())
        .is_none());
}

#[test]
fn service_worker_lifecycle_transitions_are_ordered_and_recoverable() {
    let scope = Url::parse("https://example.test/app/").unwrap();
    let script = Url::parse("https://example.test/sw.js").unwrap();
    let url = Url::parse("https://example.test/app/data").unwrap();
    let mut registry = ServiceWorkerRegistry::default();
    registry.register(scope.clone(), script);
    assert!(registry
        .transition(&scope, WorkerState::Installing)
        .is_none());
    assert!(registry
        .transition(&scope, WorkerState::Redundant)
        .is_some());
    assert_eq!(registry.get(&scope).unwrap().state, WorkerState::Redundant);
    let encoded = serde_json::to_vec(&registry).unwrap();
    let restored: ServiceWorkerRegistry = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(restored.get(&scope).unwrap().state, WorkerState::Redundant);
    assert!(registry.unregister(&scope));
    assert!(registry.get(&scope).is_none());
    assert!(registry.fetch(&url).is_none());
}

#[test]
fn only_activated_workers_control_fetches() {
    let scope = Url::parse("https://example.test/app/").unwrap();
    let url = Url::parse("https://example.test/app/data").unwrap();
    let mut registry = ServiceWorkerRegistry::default();
    registry.register(
        scope.clone(),
        Url::parse("https://example.test/sw.js").unwrap(),
    );
    registry.intercept(
        &url,
        FetchResponse {
            status: 200,
            body: b"cached".to_vec(),
        },
    );
    assert!(registry
        .transition(&scope, WorkerState::Redundant)
        .is_some());
    assert!(registry.fetch(&url).is_none());
}
