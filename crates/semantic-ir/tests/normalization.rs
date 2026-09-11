use browsai_agent_tree::{AgentNode, AgentRenderTree, NodeState, StructuralRole};
use browsai_provenance::Confidence;
use browsai_semantic_ir::{SemanticIr, SemanticIssue};

fn node(id: &str, role: StructuralRole, identity: Option<&str>, name: Option<&str>) -> AgentNode {
    AgentNode {
        id: id.into(),
        origin: None,
        identity_key: identity.map(str::to_owned),
        structural_role: role,
        semantic_role: None,
        application_type: None,
        name: name.map(str::to_owned),
        value: None,
        description: None,
        state: NodeState {
            visible: true,
            enabled: true,
            ..Default::default()
        },
        geometry: None,
        relationships: vec![],
        actions: vec![],
        children: vec![],
        provenance: vec![],
        confidence: Confidence::DIRECT,
        generation: 2,
    }
}

#[test]
fn normalization_collapses_labels_and_reports_ambiguity_and_missing_names() {
    let mut tree = AgentRenderTree::new_page("Example");
    tree.nodes.push(node(
        "first",
        StructuralRole::Button,
        Some("save"),
        Some("  Save   document "),
    ));
    tree.nodes
        .push(node("second", StructuralRole::Link, Some("save"), None));
    let mut ir = SemanticIr::from_structural(tree);

    let report = ir.normalize();
    assert_eq!(
        ir.node("first").unwrap().name.as_deref(),
        Some("Save document")
    );
    assert!(report.changed_nodes.contains(&"first".into()));
    assert!(report
        .issues
        .contains(&SemanticIssue::MissingAccessibleName {
            node_id: "second".into(),
            structural_role: StructuralRole::Link,
        }));
    assert!(report.issues.contains(&SemanticIssue::AmbiguousIdentity {
        identity_key: "save".into(),
        node_ids: vec!["first".into(), "second".into()],
    }));
}
