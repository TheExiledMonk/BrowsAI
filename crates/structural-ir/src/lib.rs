use browsai_agent_tree::{AgentNode, AgentRenderTree, StructuralRole};
use serde::{Deserialize, Serialize};

pub const STRUCTURAL_IR_VERSION: u16 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StructuralIr {
    pub schema_version: u16,
    pub tree: AgentRenderTree,
}

impl StructuralIr {
    pub fn from_tree(tree: AgentRenderTree) -> Self {
        Self {
            schema_version: STRUCTURAL_IR_VERSION,
            tree,
        }
    }

    pub fn compile(tree: &AgentRenderTree, include_invisible: bool) -> Self {
        let mut compiled = tree.clone();
        if !include_invisible {
            let root = compiled.root.clone();
            let mut visible = std::collections::BTreeSet::new();
            let mut pending = vec![root.clone()];
            while let Some(id) = pending.pop() {
                let Some(node) = compiled.nodes.iter().find(|node| node.id == id) else {
                    continue;
                };
                if id != root && !node.state.visible {
                    continue;
                }
                if !visible.insert(id) {
                    continue;
                }
                pending.extend(node.children.iter().cloned());
            }
            compiled
                .nodes
                .retain(|node| node.id == root || visible.contains(&node.id));
            for node in &mut compiled.nodes {
                node.children.retain(|child| visible.contains(child));
            }
        }
        Self::from_tree(compiled)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_json(value: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(value)
    }
    pub fn role_count(&self, role: StructuralRole) -> usize {
        self.tree
            .nodes
            .iter()
            .filter(|node| node.structural_role == role)
            .count()
    }
    pub fn node(&self, id: &str) -> Option<&AgentNode> {
        self.tree.find(id)
    }
}
