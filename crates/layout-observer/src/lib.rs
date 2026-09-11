//! Engine-neutral layout facts used by hit testing and semantic rendering.

use browsai_dom_observer::EngineNodeId;
use browsai_provenance::ProvenanceSource;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub fn contains(self, x: f64, y: f64) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }

    pub fn intersects(self, other: Self) -> bool {
        self.x < other.x + other.width
            && other.x < self.x + self.width
            && self.y < other.y + other.height
            && other.y < self.y + self.height
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayoutBox {
    pub node_id: EngineNodeId,
    pub rect: Rect,
    pub visible: bool,
    pub clipped: bool,
    pub z_index: i32,
    pub pointer_events: bool,
    pub disabled: bool,
    #[serde(default)]
    pub provenance: Vec<ProvenanceSource>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LayoutFacts {
    pub scroll_offset_x: f64,
    pub scroll_offset_y: f64,
    pub transform: [f64; 6],
    pub overlap: Vec<EngineNodeId>,
    pub hit_regions: Vec<Rect>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LayoutSnapshot {
    pub generation: u64,
    pub viewport: Rect,
    pub boxes: BTreeMap<EngineNodeId, LayoutBox>,
    pub dirty_regions: Vec<Rect>,
    #[serde(default)]
    pub facts: BTreeMap<EngineNodeId, LayoutFacts>,
}

impl LayoutSnapshot {
    pub fn insert(&mut self, layout: LayoutBox) {
        if let Some(previous) = self.boxes.get(&layout.node_id) {
            self.dirty_regions.push(previous.rect);
        }
        self.dirty_regions.push(layout.rect);
        self.boxes.insert(layout.node_id, layout);
        self.generation += 1;
    }

    pub fn set_facts(&mut self, node_id: EngineNodeId, facts: LayoutFacts) {
        self.facts.insert(node_id, facts);
        self.generation += 1;
    }

    pub fn facts(&self, node_id: EngineNodeId) -> Option<&LayoutFacts> {
        self.facts.get(&node_id)
    }

    pub fn remove(&mut self, node_id: EngineNodeId) -> Option<LayoutBox> {
        let removed = self.boxes.remove(&node_id)?;
        self.dirty_regions.push(removed.rect);
        self.generation += 1;
        Some(removed)
    }

    pub fn invalidate(&mut self, node_id: EngineNodeId) -> bool {
        let Some(layout) = self.boxes.get(&node_id) else {
            return false;
        };
        self.dirty_regions.push(layout.rect);
        true
    }

    pub fn drain_dirty_regions(&mut self) -> Vec<Rect> {
        std::mem::take(&mut self.dirty_regions)
    }
    pub fn get(&self, node_id: EngineNodeId) -> Option<&LayoutBox> {
        self.boxes.get(&node_id)
    }

    /// Returns the topmost actionable hit target according to visibility,
    /// clipping, pointer-events, disabled state, and z-order.
    pub fn hit_test(&self, x: f64, y: f64) -> Option<EngineNodeId> {
        let viewport_limited = self.viewport.width > 0.0 && self.viewport.height > 0.0;
        if viewport_limited && !self.viewport.contains(x, y) {
            return None;
        }
        self.boxes
            .values()
            .filter(|layout| {
                let hit_region = self.facts.get(&layout.node_id).and_then(|facts| {
                    facts
                        .hit_regions
                        .iter()
                        .find(|region| region.contains(x, y))
                });
                layout.visible
                    && !layout.clipped
                    && layout.pointer_events
                    && !layout.disabled
                    && layout.rect.contains(x, y)
                    && self.facts.get(&layout.node_id).map_or(true, |facts| {
                        facts.hit_regions.is_empty() || hit_region.is_some()
                    })
            })
            .max_by_key(|layout| (layout.z_index, layout.node_id))
            .map(|layout| layout.node_id)
    }
}
