use browsai_agent_tree::{AgentNode, AgentNodeId};
use browsai_state::PageSnapshot;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::VecDeque;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum DiffOp {
    Add {
        path: String,
        value: Value,
    },
    Remove {
        path: String,
        value: Value,
    },
    Replace {
        path: String,
        old: Value,
        new: Value,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SemanticTreeDiff {
    pub added: Vec<AgentNode>,
    pub removed: Vec<AgentNode>,
    pub changed: Vec<NodeChange>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodeChange {
    pub node_id: AgentNodeId,
    pub before: AgentNode,
    pub after: AgentNode,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IncrementalDiff {
    pub sequence: u64,
    pub generation: u64,
    pub operations: Vec<DiffOp>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiffStreamError {
    pub dropped: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiffBatch {
    pub entries: Vec<IncrementalDiff>,
    pub dropped: u64,
}

#[derive(Clone, Debug)]
pub struct DiffStream {
    capacity: usize,
    next_sequence: u64,
    dropped: u64,
    entries: VecDeque<IncrementalDiff>,
}

impl DiffStream {
    pub fn bounded(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            next_sequence: 0,
            dropped: 0,
            entries: VecDeque::new(),
        }
    }

    pub fn publish(&mut self, generation: u64, operations: Vec<DiffOp>) -> u64 {
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        if let Some(previous) = self.entries.back_mut() {
            if previous.generation == generation {
                previous.operations.extend(operations);
                return previous.sequence;
            }
        }
        if self.entries.len() == self.capacity {
            self.entries.pop_front();
            self.dropped += 1;
        }
        self.entries.push_back(IncrementalDiff {
            sequence,
            generation,
            operations,
        });
        sequence
    }

    pub fn drain(&mut self) -> Result<Vec<IncrementalDiff>, DiffStreamError> {
        let entries = self.entries.drain(..).collect();
        if self.dropped == 0 {
            Ok(entries)
        } else {
            let error = DiffStreamError {
                dropped: self.dropped,
            };
            self.dropped = 0;
            Err(error)
        }
    }

    /// Drain retained diffs while preserving them alongside the overflow signal.
    pub fn drain_with_loss(&mut self) -> DiffBatch {
        DiffBatch {
            entries: self.entries.drain(..).collect(),
            dropped: std::mem::take(&mut self.dropped),
        }
    }

    pub fn replay(&self) -> impl Iterator<Item = &IncrementalDiff> {
        self.entries.iter()
    }
    pub fn dropped(&self) -> u64 {
        self.dropped
    }
}

impl Default for DiffStream {
    fn default() -> Self {
        Self::bounded(256)
    }
}

fn node_key(node: &AgentNode) -> String {
    node.identity_key.clone().unwrap_or_else(|| node.id.clone())
}

pub fn diff_tree(before: &PageSnapshot, after: &PageSnapshot) -> SemanticTreeDiff {
    let left: std::collections::BTreeMap<_, _> = before
        .tree
        .nodes
        .iter()
        .map(|node| (node_key(node), node))
        .collect();
    let right: std::collections::BTreeMap<_, _> = after
        .tree
        .nodes
        .iter()
        .map(|node| (node_key(node), node))
        .collect();
    let added = right
        .iter()
        .filter(|(key, _)| !left.contains_key(*key))
        .map(|(_, node)| (*node).clone())
        .collect();
    let removed = left
        .iter()
        .filter(|(key, _)| !right.contains_key(*key))
        .map(|(_, node)| (*node).clone())
        .collect();
    let changed = right
        .iter()
        .filter_map(|(key, after_node)| {
            let before_node = left.get(key)?;
            (before_node != after_node).then(|| NodeChange {
                node_id: after_node.id.clone(),
                before: (*before_node).clone(),
                after: (*after_node).clone(),
            })
        })
        .collect();
    SemanticTreeDiff {
        added,
        removed,
        changed,
    }
}

pub fn diff(a: &PageSnapshot, b: &PageSnapshot) -> Vec<DiffOp> {
    let semantic = diff_tree(a, b);
    let mut output = Vec::new();
    for node in semantic.added {
        output.push(DiffOp::Add {
            path: format!("/tree/nodes/{}", node.id),
            value: serde_json::to_value(node).expect("node serializes"),
        });
    }
    for node in semantic.removed {
        output.push(DiffOp::Remove {
            path: format!("/tree/nodes/{}", node.id),
            value: serde_json::to_value(node).expect("node serializes"),
        });
    }
    for change in semantic.changed {
        output.push(DiffOp::Replace {
            path: format!("/tree/nodes/{}", change.node_id),
            old: serde_json::to_value(change.before).expect("node serializes"),
            new: serde_json::to_value(change.after).expect("node serializes"),
        });
    }
    if a.url != b.url {
        output.push(DiffOp::Replace {
            path: "/url".into(),
            old: serde_json::to_value(&a.url).expect("url serializes"),
            new: serde_json::to_value(&b.url).expect("url serializes"),
        });
    }
    output
}
