use browsai_agent_tree::StructuralRole;
use browsai_dom_observer::{DomMutation, NodeKind, RawDocument, RawNode};
use browsai_layout_observer::LayoutSnapshot;
use browsai_semantic_compiler::SemanticCompiler;
use std::collections::BTreeMap;

#[test]
fn agent_tree_represents_frame_and_shadow_scopes_without_pixels() {
    let mut document = RawDocument::new("https://example.test/");
    document
        .apply(DomMutation::Insert {
            node: RawNode::document(1),
        })
        .unwrap();
    for node in [
        RawNode {
            id: 2,
            kind: NodeKind::Frame,
            name: Some("iframe".into()),
            attributes: BTreeMap::new(),
            text: None,
            parent: Some(1),
            children: vec![],
            event_listeners: Default::default(),
            provenance: vec![],
        },
        RawNode {
            id: 3,
            kind: NodeKind::ShadowRoot,
            name: None,
            attributes: BTreeMap::new(),
            text: None,
            parent: Some(1),
            children: vec![],
            event_listeners: Default::default(),
            provenance: vec![],
        },
    ] {
        document.apply(DomMutation::Insert { node }).unwrap();
    }
    let result = SemanticCompiler::default().compile(&document, &LayoutSnapshot::default());
    assert!(result
        .tree
        .nodes
        .iter()
        .any(|node| node.structural_role == StructuralRole::Frame));
    assert!(result
        .tree
        .nodes
        .iter()
        .any(|node| node.structural_role == StructuralRole::ShadowRoot));
}
