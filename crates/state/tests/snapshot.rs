use browsai_agent_tree::AgentRenderTree;
use browsai_state::{PageState, SnapshotStore, SNAPSHOT_SCHEMA_VERSION};
use url::Url;

#[test]
fn snapshots_are_immutable_copies_of_page_state() {
    let state = PageState {
        url: Url::parse("https://example.test/").unwrap(),
        tree: AgentRenderTree::new_page("Example"),
        generation: 1,
    };
    let snapshot = state.snapshot(42);
    assert_eq!(snapshot.schema_version, SNAPSHOT_SCHEMA_VERSION);
    assert_eq!(snapshot.id, 42);
    assert_eq!(snapshot.url.as_str(), "https://example.test/");
    assert_eq!(snapshot.semantic_generation, 0);
}

#[test]
fn snapshot_store_is_bounded_schema_checked_and_round_trips() {
    let state = PageState {
        url: Url::parse("https://example.test/").unwrap(),
        tree: AgentRenderTree::new_page("Example"),
        generation: 1,
    };
    let mut store = SnapshotStore::new(2);
    store.insert(state.snapshot(1));
    store.insert(state.snapshot(2));
    store.insert(state.snapshot(3));
    assert_eq!(store.len(), 2);
    assert!(store.get(1).is_none());
    let restored = SnapshotStore::from_json(&store.to_json().unwrap()).unwrap();
    assert_eq!(restored, store);

    let mut invalid: serde_json::Value = serde_json::from_str(&store.to_json().unwrap()).unwrap();
    invalid["snapshots"]["2"]["schema_version"] = serde_json::json!(99);
    assert!(SnapshotStore::from_json(&serde_json::to_string(&invalid).unwrap()).is_err());
}
