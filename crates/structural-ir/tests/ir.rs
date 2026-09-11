use browsai_agent_tree::{AgentNode, AgentRenderTree, NodeState, StructuralRole};
use browsai_provenance::Confidence;
use browsai_structural_ir::{StructuralIr, STRUCTURAL_IR_VERSION};

#[test]
fn compiler_prunes_invisible_nodes_and_round_trips_schema() {
    let mut tree = AgentRenderTree::new_page("Example");
    tree.nodes.push(AgentNode {
        id: "hidden".into(),
        origin: None,
        identity_key: Some("hidden".into()),
        structural_role: StructuralRole::Paragraph,
        semantic_role: None,
        application_type: None,
        name: Some("hidden".into()),
        value: None,
        description: None,
        state: NodeState {
            visible: false,
            enabled: false,
            ..Default::default()
        },
        geometry: None,
        relationships: vec![],
        actions: vec![],
        children: vec![],
        provenance: vec![],
        confidence: Confidence::DIRECT,
        generation: 0,
    });
    let ir = StructuralIr::compile(&tree, false);
    assert_eq!(ir.schema_version, STRUCTURAL_IR_VERSION);
    assert!(ir.node("hidden").is_none());
    let decoded = StructuralIr::from_json(&ir.to_json().unwrap()).unwrap();
    assert_eq!(decoded, ir);
}

#[test]
fn compiler_prunes_invisible_subtrees_without_detached_visible_nodes() {
    let mut tree = AgentRenderTree::new_page("Example");
    tree.nodes.push(AgentNode {
        id: "hidden-parent".into(),
        origin: None,
        identity_key: None,
        structural_role: StructuralRole::Region,
        semantic_role: None,
        application_type: None,
        name: None,
        value: None,
        description: None,
        state: NodeState {
            visible: false,
            enabled: true,
            ..Default::default()
        },
        geometry: None,
        relationships: vec![],
        actions: vec![],
        children: vec!["visible-child".into()],
        provenance: vec![],
        confidence: Confidence::DIRECT,
        generation: 0,
    });
    tree.nodes[0].children.push("hidden-parent".into());
    tree.nodes.push(AgentNode {
        id: "visible-child".into(),
        origin: None,
        identity_key: None,
        structural_role: StructuralRole::Text,
        semantic_role: None,
        application_type: None,
        name: Some("child".into()),
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
        generation: 0,
    });
    let ir = StructuralIr::compile(&tree, false);
    assert!(ir.node("hidden-parent").is_none());
    assert!(ir.node("visible-child").is_none());
}
