use browsai_agent_tree::{AgentNode, AgentRenderTree, NodeState, StructuralRole};
use browsai_provenance::Confidence;
use browsai_structural_ir::{StructuralIr, STRUCTURAL_IR_VERSION};
use serde::Deserialize;

#[derive(Deserialize)]
struct Golden {
    schema_version: u16,
    include_invisible: bool,
    expected_roles: Vec<String>,
}

#[test]
fn structural_ir_matches_repository_golden_fixture() {
    let golden: Golden = serde_json::from_str(include_str!(
        "../../../test-sites/semantic-golden/structural.json"
    ))
    .unwrap();
    let mut tree = AgentRenderTree::new_page("Example");
    tree.nodes[0].children.push("hidden".into());
    tree.nodes.push(AgentNode {
        id: "hidden".into(),
        origin: None,
        identity_key: None,
        structural_role: StructuralRole::Paragraph,
        semantic_role: None,
        application_type: None,
        name: Some("hidden".into()),
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
        children: vec![],
        provenance: vec![],
        confidence: Confidence::DIRECT,
        generation: 0,
    });
    let mut visible = AgentNode {
        id: "button".into(),
        origin: None,
        identity_key: Some("button:save".into()),
        structural_role: StructuralRole::Button,
        semantic_role: None,
        application_type: None,
        name: Some("Save".into()),
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
    };
    visible.children = vec![];
    tree.nodes[0].children.push("button".into());
    tree.nodes.push(visible);
    let ir = StructuralIr::compile(&tree, golden.include_invisible);
    assert_eq!(ir.schema_version, golden.schema_version);
    assert_eq!(ir.schema_version, STRUCTURAL_IR_VERSION);
    let actual = ir
        .tree
        .nodes
        .iter()
        .map(|node| format!("{:?}", node.structural_role))
        .collect::<Vec<_>>();
    assert_eq!(actual, golden.expected_roles);
}
