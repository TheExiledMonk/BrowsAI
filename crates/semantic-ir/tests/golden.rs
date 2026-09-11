use browsai_agent_tree::{AgentNode, AgentRenderTree, NodeState, StructuralRole};
use browsai_provenance::{Confidence, ProvenanceSource, SourceKind};
use browsai_semantic_ir::{SemanticGoldenFixture, SemanticIr};

#[test]
fn repository_semantic_golden_fixture_validates_roles_visibility_confidence_and_provenance() {
    let fixture: SemanticGoldenFixture = serde_json::from_str(include_str!(
        "../../../test-sites/semantic-golden/basic-page.json"
    ))
    .unwrap();
    let roles = [
        StructuralRole::Page,
        StructuralRole::Heading,
        StructuralRole::Paragraph,
        StructuralRole::Button,
    ];
    let mut tree = AgentRenderTree::new_page("Example");
    for (index, role) in roles.into_iter().enumerate().skip(1) {
        tree.nodes.push(AgentNode {
            id: format!("node-{index}"),
            origin: None,
            identity_key: None,
            structural_role: role,
            semantic_role: None,
            application_type: None,
            name: None,
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
    }
    for node in &mut tree.nodes {
        node.provenance = vec![
            ProvenanceSource {
                kind: SourceKind::Dom,
                reference: node.id.clone(),
                detail: None,
            },
            ProvenanceSource {
                kind: SourceKind::Layout,
                reference: node.id.clone(),
                detail: None,
            },
        ];
    }
    assert!(SemanticIr::from_structural(tree)
        .validate_golden(&fixture)
        .is_empty());
}
