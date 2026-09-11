//! Engine-neutral computed-style and semantic visibility observations.

use browsai_dom_observer::EngineNodeId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum VisibilityState {
    Visible,
    DisplayNone,
    Hidden,
    Transparent,
    Clipped,
    PointerEventsNone,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ComputedStyle {
    pub display: String,
    pub visibility: String,
    pub opacity: f32,
    pub z_index: i32,
    pub pointer_events: bool,
    pub clipped: bool,
    pub pseudo_elements: BTreeMap<String, String>,
    pub media_state: BTreeMap<String, bool>,
}

impl Default for ComputedStyle {
    fn default() -> Self {
        Self {
            display: "block".into(),
            visibility: "visible".into(),
            opacity: 1.0,
            z_index: 0,
            pointer_events: true,
            clipped: false,
            pseudo_elements: BTreeMap::new(),
            media_state: BTreeMap::new(),
        }
    }
}

impl ComputedStyle {
    pub fn visibility_state(&self) -> VisibilityState {
        if self.display == "none" {
            VisibilityState::DisplayNone
        } else if self.visibility != "visible" {
            VisibilityState::Hidden
        } else if self.opacity <= 0.0 {
            VisibilityState::Transparent
        } else if self.clipped {
            VisibilityState::Clipped
        } else if !self.pointer_events {
            VisibilityState::PointerEventsNone
        } else {
            VisibilityState::Visible
        }
    }
    pub fn semantically_visible(&self) -> bool {
        matches!(self.visibility_state(), VisibilityState::Visible)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StyleSnapshot {
    pub generation: u64,
    pub nodes: BTreeMap<EngineNodeId, ComputedStyle>,
    pub dirty_nodes: Vec<EngineNodeId>,
}

impl StyleSnapshot {
    pub fn set(&mut self, node: EngineNodeId, style: ComputedStyle) {
        self.nodes.insert(node, style);
        self.dirty_nodes.push(node);
        self.generation += 1;
    }

    pub fn remove(&mut self, node: EngineNodeId) -> Option<ComputedStyle> {
        let removed = self.nodes.remove(&node)?;
        self.dirty_nodes.push(node);
        self.generation += 1;
        Some(removed)
    }

    pub fn drain_dirty_nodes(&mut self) -> Vec<EngineNodeId> {
        std::mem::take(&mut self.dirty_nodes)
    }
    pub fn get(&self, node: EngineNodeId) -> Option<&ComputedStyle> {
        self.nodes.get(&node)
    }
    pub fn visible(&self, node: EngineNodeId) -> bool {
        self.get(node)
            .is_some_and(ComputedStyle::semantically_visible)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn style_visibility_preserves_hit_testing_facts() {
        let mut snapshot = StyleSnapshot::default();
        let style = ComputedStyle {
            opacity: 0.0,
            z_index: 4,
            ..Default::default()
        };
        snapshot.set(2, style);
        assert_eq!(
            snapshot.get(2).unwrap().visibility_state(),
            VisibilityState::Transparent
        );
        assert!(!snapshot.visible(2));
        assert_eq!(snapshot.drain_dirty_nodes(), vec![2]);
    }
}
