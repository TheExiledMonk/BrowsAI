use browsai_frames::{FrameError, FrameTree};
use url::Url;

#[test]
fn frame_tree_enforces_origin_boundaries() {
    let mut tree = FrameTree::new(Url::parse("https://example.test/").unwrap());
    let root = tree.root().clone();
    let same = tree
        .add_child(&root, Url::parse("https://example.test/frame").unwrap())
        .unwrap();
    let cross = tree
        .add_child(&root, Url::parse("https://other.test/frame").unwrap())
        .unwrap();
    assert!(tree.can_access(&root, &same).unwrap());
    assert_eq!(
        tree.require_access(&root, &cross),
        Err(FrameError::CrossOriginDenied)
    );
}

#[test]
fn frame_navigation_updates_origin_and_invalidates_descendant_documents() {
    let mut tree = FrameTree::new(Url::parse("https://example.test/").unwrap());
    let root = tree.root().clone();
    let child = tree
        .add_child(&root, Url::parse("https://example.test/frame").unwrap())
        .unwrap();
    let grandchild = tree
        .add_child(&child, Url::parse("https://example.test/nested").unwrap())
        .unwrap();
    tree.navigate(&child, Url::parse("https://other.test/new").unwrap())
        .unwrap();
    assert_eq!(tree.get(&child).unwrap().origin, "https://other.test");
    assert!(tree.get(&grandchild).is_none());
    assert_eq!(
        tree.require_access(&root, &child),
        Err(FrameError::CrossOriginDenied)
    );
}

#[test]
fn frame_scopes_round_trip_and_reject_dangling_links() {
    let mut tree = FrameTree::new(Url::parse("https://example.test/").unwrap());
    let root = tree.root().clone();
    let child = tree
        .add_child(&root, Url::parse("https://example.test/frame").unwrap())
        .unwrap();
    let grandchild = tree
        .add_child(&child, Url::parse("https://other.test/nested").unwrap())
        .unwrap();
    assert_eq!(
        tree.scoped_ids(&child).unwrap(),
        vec![child.clone(), grandchild]
    );

    let encoded = tree.to_json().unwrap();
    let restored = FrameTree::from_json(&encoded).unwrap();
    assert_eq!(restored.scoped_ids(&root).unwrap().len(), 3);

    let mut invalid: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    invalid["root"] = serde_json::json!("00000000-0000-0000-0000-000000000000");
    assert!(matches!(
        FrameTree::from_json(&serde_json::to_string(&invalid).unwrap()),
        Err(FrameError::InvalidSnapshot)
    ));
}

#[test]
fn agent_scopes_and_cross_frame_operations_are_origin_bound() {
    let mut tree = FrameTree::new(Url::parse("https://example.test/").unwrap());
    let root = tree.root().clone();
    let same = tree
        .add_child(&root, Url::parse("https://example.test/embedded").unwrap())
        .unwrap();
    let cross = tree
        .add_child(&root, Url::parse("https://other.test/embedded").unwrap())
        .unwrap();
    let scope = tree.agent_scope(&root).unwrap();
    assert_eq!(scope.frame, root);
    assert_eq!(scope.descendants, vec![same.clone(), cross.clone()]);
    assert!(tree.can_query(&root, &same).is_ok());
    assert_eq!(
        tree.can_query(&root, &cross),
        Err(FrameError::CrossOriginDenied)
    );
    assert_eq!(
        tree.can_action(&root, &cross),
        Err(FrameError::CrossOriginDenied)
    );
}
