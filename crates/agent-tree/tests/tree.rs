use browsai_agent_tree::{AgentRenderTree, StructuralRole};

#[test]
fn page_tree_has_stable_root_reference() {
    let tree = AgentRenderTree::new_page("Example");
    assert_eq!(tree.root, tree.nodes[0].id);
    assert_eq!(tree.nodes[0].structural_role, StructuralRole::Page);
    assert_eq!(tree.nodes[0].confidence.0, 1.0);
}
