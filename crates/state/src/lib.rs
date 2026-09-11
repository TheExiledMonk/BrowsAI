use browsai_agent_tree::AgentRenderTree;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use url::Url;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PageSnapshot {
    pub schema_version: u32,
    pub id: u64,
    pub url: Url,
    pub tree: AgentRenderTree,
    pub focused_node: Option<String>,
    pub scroll_x: f64,
    pub scroll_y: f64,
    pub pending_network: u32,
    pub storage_generation: u64,
    pub semantic_generation: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PageState {
    pub url: Url,
    pub tree: AgentRenderTree,
    pub generation: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PageRuntimeState {
    pub normalized_values: BTreeMap<String, String>,
    pub autocomplete: Vec<String>,
    pub focused_node: Option<String>,
    pub scroll_x: f64,
    pub scroll_y: f64,
    pub history: Vec<NavigationRecord>,
    pub logical_time_millis: u64,
    pub page_locks: BTreeSet<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NavigationRecord {
    pub url: Url,
    pub observed_at_millis: u64,
}

impl PageRuntimeState {
    pub fn set_value(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.normalized_values.insert(key.into(), value.into());
    }
    pub fn record_autocomplete(&mut self, value: impl Into<String>, max_entries: usize) {
        let value = value.into();
        self.autocomplete.retain(|entry| entry != &value);
        self.autocomplete.push(value);
        if self.autocomplete.len() > max_entries {
            let excess = self.autocomplete.len() - max_entries;
            self.autocomplete.drain(..excess);
        }
    }
    pub fn focus(&mut self, node_id: Option<String>) {
        self.focused_node = node_id;
    }
    pub fn scroll_to(&mut self, x: f64, y: f64) {
        self.scroll_x = x.max(0.0);
        self.scroll_y = y.max(0.0);
    }
    pub fn record_navigation(&mut self, url: Url) {
        self.history.push(NavigationRecord {
            url,
            observed_at_millis: self.logical_time_millis,
        });
    }
    pub fn advance_time(&mut self, millis: u64) {
        self.logical_time_millis = self.logical_time_millis.saturating_add(millis);
    }
    pub fn acquire_lock(&mut self, lock: impl Into<String>) -> bool {
        self.page_locks.insert(lock.into())
    }
    pub fn release_lock(&mut self, lock: &str) -> bool {
        self.page_locks.remove(lock)
    }
    pub fn is_locked(&self, lock: &str) -> bool {
        self.page_locks.contains(lock)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StabilityState {
    pub pending_network: u32,
    pub pending_tasks: u32,
    pub last_mutation_generation: u64,
    pub quiet_generations: u64,
}

impl StabilityState {
    pub fn is_stable(&self, required_quiet_generations: u64) -> bool {
        self.pending_network == 0
            && self.pending_tasks == 0
            && self.quiet_generations >= required_quiet_generations
    }
    pub fn mutation(&mut self, generation: u64) {
        self.last_mutation_generation = generation;
        self.quiet_generations = 0;
    }
    pub fn tick_quiet(&mut self) {
        self.quiet_generations += 1;
    }
}

impl PageState {
    pub fn snapshot(&self, id: u64) -> PageSnapshot {
        PageSnapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            id,
            url: self.url.clone(),
            semantic_generation: self.tree.generation,
            tree: self.tree.clone(),
            focused_node: None,
            scroll_x: 0.0,
            scroll_y: 0.0,
            pending_network: 0,
            storage_generation: 0,
        }
    }

    pub fn page(
        &self,
        offset: usize,
        limit: usize,
        max_tokens: Option<usize>,
        expected_generation: Option<u64>,
    ) -> Result<SnapshotPage, SnapshotError> {
        if expected_generation.is_some_and(|generation| generation != self.tree.generation) {
            return Err(SnapshotError::Stale);
        }
        let start = offset.min(self.tree.nodes.len());
        let effective_limit = limit.max(1);
        let token_limit = max_tokens.map(|tokens| tokens.saturating_mul(4));
        let mut used = 0;
        let mut nodes = Vec::new();
        for node in self.tree.nodes.iter().skip(start).take(effective_limit) {
            let size = serde_json::to_string(node)
                .map_err(|_| SnapshotError::Serialization)?
                .len();
            if token_limit.is_some_and(|limit| !nodes.is_empty() && used + size > limit) {
                break;
            }
            used += size;
            nodes.push(node.clone());
        }
        let next_offset = (start + nodes.len()).min(self.tree.nodes.len());
        Ok(SnapshotPage {
            snapshot_id: self.tree.generation,
            offset: start,
            nodes,
            next_offset: (next_offset < self.tree.nodes.len()).then_some(next_offset),
            cursor: start,
            limit: effective_limit,
            truncated: next_offset < self.tree.nodes.len(),
            next_cursor: (next_offset < self.tree.nodes.len()).then_some(next_offset.to_string()),
            estimated_tokens: used / 4 + usize::from(used % 4 != 0),
        })
    }
}

pub const SNAPSHOT_SCHEMA_VERSION: u32 = 1;

/// Bounded immutable snapshot retention for replay and agent consumers.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SnapshotStore {
    capacity: usize,
    snapshots: BTreeMap<u64, PageSnapshot>,
}

impl SnapshotStore {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            snapshots: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, snapshot: PageSnapshot) {
        self.snapshots.insert(snapshot.id, snapshot);
        while self.snapshots.len() > self.capacity {
            let Some(oldest) = self.snapshots.keys().next().copied() else {
                break;
            };
            self.snapshots.remove(&oldest);
        }
    }

    pub fn get(&self, id: u64) -> Option<&PageSnapshot> {
        self.snapshots.get(&id)
    }

    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_json(value: &str) -> Result<Self, serde_json::Error> {
        let store: Self = serde_json::from_str(value)?;
        if store.capacity == 0
            || store
                .snapshots
                .values()
                .any(|snapshot| snapshot.schema_version != SNAPSHOT_SCHEMA_VERSION)
        {
            return serde_json::from_str("not a valid snapshot store");
        }
        Ok(store)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SnapshotPage {
    pub snapshot_id: u64,
    pub offset: usize,
    pub nodes: Vec<browsai_agent_tree::AgentNode>,
    pub next_offset: Option<usize>,
    #[serde(default)]
    pub cursor: usize,
    #[serde(default)]
    pub limit: usize,
    #[serde(default)]
    pub truncated: bool,
    #[serde(default)]
    pub next_cursor: Option<String>,
    pub estimated_tokens: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnapshotError {
    Stale,
    Serialization,
}
