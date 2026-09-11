use browsai_agent_tree::AgentRenderTree;
use browsai_diff::{diff, diff_tree, DiffOp, DiffStream, DiffStreamError};
use browsai_state::PageState;
use url::Url;

#[test]
fn equal_snapshots_have_no_diff() {
    let state = PageState {
        url: Url::parse("https://example.test/").unwrap(),
        tree: AgentRenderTree::new_page("Example"),
        generation: 1,
    };
    let a = state.snapshot(1);
    let b = a.clone();
    assert!(diff(&a, &b).is_empty());
}

#[test]
fn incremental_stream_coalesces_generation_and_reports_loss() {
    let mut stream = DiffStream::bounded(1);
    let first = stream.publish(7, vec![]);
    assert_eq!(
        stream.publish(
            7,
            vec![DiffOp::Add {
                path: "/x".into(),
                value: serde_json::Value::Null
            }]
        ),
        first
    );
    assert_eq!(stream.replay().next().unwrap().operations.len(), 1);
    stream.publish(8, vec![]);
    assert_eq!(stream.drain(), Err(DiffStreamError { dropped: 1 }));
    assert!(stream.drain().unwrap().is_empty());
}

#[test]
fn loss_aware_diff_drain_preserves_retained_entries() {
    let mut stream = DiffStream::bounded(2);
    stream.publish(1, vec![]);
    stream.publish(2, vec![]);
    stream.publish(3, vec![]);
    let batch = stream.drain_with_loss();
    assert_eq!(batch.dropped, 1);
    assert_eq!(batch.entries.len(), 2);
    assert_eq!(batch.entries[0].generation, 2);
    assert_eq!(batch.entries[1].generation, 3);
    assert_eq!(stream.drain_with_loss().dropped, 0);
}

#[test]
fn snapshot_metadata_only_changes_are_not_semantic_changes() {
    let state = PageState {
        url: Url::parse("https://example.test/").unwrap(),
        tree: AgentRenderTree::new_page("Example"),
        generation: 1,
    };
    let a = state.snapshot(1);
    let mut b = a.clone();
    b.id = 2;
    assert!(diff(&a, &b).is_empty());
}

#[test]
fn semantic_diff_matches_conceptual_identity_across_engine_ids() {
    let mut before_tree = AgentRenderTree::new_page("Example");
    let mut after_tree = AgentRenderTree::new_page("Example");
    before_tree.nodes[0].identity_key = Some("page".into());
    after_tree.nodes[0].identity_key = Some("page".into());
    before_tree.nodes[0].children.push("dom:10".into());
    after_tree.nodes[0].children.push("dom:99".into());
    let before_node = browsai_agent_tree::AgentNode {
        id: "dom:10".into(),
        origin: None,
        identity_key: Some("button:save".into()),
        structural_role: browsai_agent_tree::StructuralRole::Button,
        semantic_role: None,
        application_type: None,
        name: Some("Save".into()),
        value: None,
        description: None,
        state: Default::default(),
        geometry: None,
        relationships: vec![],
        actions: vec![],
        children: vec![],
        provenance: vec![],
        confidence: browsai_provenance::Confidence::DIRECT,
        generation: 0,
    };
    let mut after_node = before_node.clone();
    after_node.id = "dom:99".into();
    after_node.name = Some("Save changes".into());
    before_tree.nodes.push(before_node);
    after_tree.nodes.push(after_node);
    let state = PageState {
        url: Url::parse("https://example.test/").unwrap(),
        tree: before_tree,
        generation: 1,
    };
    let after_state = PageState {
        url: Url::parse("https://example.test/").unwrap(),
        tree: after_tree,
        generation: 2,
    };
    let semantic = diff_tree(&state.snapshot(1), &after_state.snapshot(2));
    assert!(semantic.added.is_empty());
    assert!(semantic.removed.is_empty());
    assert!(semantic
        .changed
        .iter()
        .any(|change| change.node_id == "dom:99"));
}
