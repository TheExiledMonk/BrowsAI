//! Engine-neutral raw DOM observation contracts.
//!
//! The observer records what the browser engine produced. It does not execute
//! or rewrite page JavaScript and it does not assign semantic meaning to raw
//! nodes; those responsibilities belong to later IR/compiler layers.

use browsai_provenance::ProvenanceSource;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type EngineNodeId = u64;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum DocumentLifecycle {
    #[default]
    Initial,
    Loading,
    Interactive,
    Complete,
    Destroyed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NodeReference {
    pub node: EngineNodeId,
    pub generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum NodeKind {
    Document,
    Element,
    Text,
    Comment,
    ShadowRoot,
    Frame,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RawNode {
    pub id: EngineNodeId,
    pub kind: NodeKind,
    pub name: Option<String>,
    pub attributes: BTreeMap<String, String>,
    pub text: Option<String>,
    pub parent: Option<EngineNodeId>,
    pub children: Vec<EngineNodeId>,
    pub event_listeners: std::collections::BTreeSet<String>,
    #[serde(default)]
    pub provenance: Vec<ProvenanceSource>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ObservedSignals {
    pub form: bool,
    pub dialog: bool,
    pub frame: bool,
    pub shadow_root: bool,
    pub canvas: bool,
    pub media: Option<String>,
    pub accessibility_role: Option<String>,
    pub accessible_name: Option<String>,
    pub disabled: bool,
    pub focused: bool,
}

impl RawNode {
    pub fn document(id: EngineNodeId) -> Self {
        Self {
            id,
            kind: NodeKind::Document,
            name: None,
            attributes: BTreeMap::new(),
            text: None,
            parent: None,
            children: vec![],
            event_listeners: std::collections::BTreeSet::new(),
            provenance: vec![],
        }
    }
    pub fn element(
        id: EngineNodeId,
        name: impl Into<String>,
        parent: Option<EngineNodeId>,
    ) -> Self {
        Self {
            id,
            kind: NodeKind::Element,
            name: Some(name.into()),
            attributes: BTreeMap::new(),
            text: None,
            parent,
            children: vec![],
            event_listeners: std::collections::BTreeSet::new(),
            provenance: vec![],
        }
    }
    pub fn text(id: EngineNodeId, value: impl Into<String>, parent: Option<EngineNodeId>) -> Self {
        Self {
            id,
            kind: NodeKind::Text,
            name: None,
            attributes: BTreeMap::new(),
            text: Some(value.into()),
            parent,
            children: vec![],
            event_listeners: std::collections::BTreeSet::new(),
            provenance: vec![],
        }
    }

    pub fn observed_signals(&self) -> ObservedSignals {
        let tag = self
            .name
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let media = match tag.as_str() {
            "audio" => Some("audio".into()),
            "video" => Some("video".into()),
            "track" => Some("track".into()),
            _ => None,
        };
        ObservedSignals {
            form: tag == "form",
            dialog: tag == "dialog",
            frame: matches!(self.kind, NodeKind::Frame)
                || matches!(tag.as_str(), "iframe" | "frame"),
            shadow_root: matches!(self.kind, NodeKind::ShadowRoot),
            canvas: tag == "canvas",
            media,
            accessibility_role: self.attributes.get("role").cloned(),
            accessible_name: self
                .attributes
                .get("aria-label")
                .cloned()
                .or_else(|| self.attributes.get("title").cloned()),
            disabled: self.attributes.contains_key("disabled"),
            focused: self
                .attributes
                .get("data-focused")
                .is_some_and(|value| value == "true"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DomMutation {
    Insert {
        node: RawNode,
    },
    Remove {
        node: EngineNodeId,
    },
    SetAttribute {
        node: EngineNodeId,
        name: String,
        value: String,
    },
    RemoveAttribute {
        node: EngineNodeId,
        name: String,
    },
    SetText {
        node: EngineNodeId,
        value: String,
    },
    AddEventListener {
        node: EngineNodeId,
        event: String,
    },
    RemoveEventListener {
        node: EngineNodeId,
        event: String,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RawDocument {
    pub url: String,
    pub generation: u64,
    pub lifecycle: DocumentLifecycle,
    pub root: Option<EngineNodeId>,
    pub nodes: BTreeMap<EngineNodeId, RawNode>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DomObserverError {
    MissingNode(EngineNodeId),
    DuplicateNode(EngineNodeId),
    MissingParent(EngineNodeId),
    RootAlreadySet,
    StaleReference,
    DocumentDestroyed,
}

impl RawDocument {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            lifecycle: DocumentLifecycle::Initial,
            ..Self::default()
        }
    }

    pub fn apply(&mut self, mutation: DomMutation) -> Result<(), DomObserverError> {
        if self.lifecycle == DocumentLifecycle::Destroyed {
            return Err(DomObserverError::DocumentDestroyed);
        }
        match mutation {
            DomMutation::Insert { node } => {
                if self.nodes.contains_key(&node.id) {
                    return Err(DomObserverError::DuplicateNode(node.id));
                }
                if let Some(parent) = node.parent {
                    if !self.nodes.contains_key(&parent) {
                        return Err(DomObserverError::MissingParent(parent));
                    }
                } else if self.root.is_some() {
                    return Err(DomObserverError::RootAlreadySet);
                }
                let id = node.id;
                if let Some(parent) = node.parent {
                    self.nodes
                        .get_mut(&parent)
                        .expect("checked above")
                        .children
                        .push(id);
                } else {
                    self.root = Some(id);
                }
                self.nodes.insert(id, node);
            }
            DomMutation::Remove { node } => {
                let removed = self
                    .nodes
                    .get(&node)
                    .cloned()
                    .ok_or(DomObserverError::MissingNode(node))?;
                let mut removed_ids = Vec::new();
                self.collect_descendants(node, &mut removed_ids);
                removed_ids.push(node);
                for removed_id in removed_ids {
                    self.nodes.remove(&removed_id);
                }
                if let Some(parent) = removed.parent {
                    if let Some(parent_node) = self.nodes.get_mut(&parent) {
                        parent_node.children.retain(|child| *child != node);
                    }
                }
                if self.root == Some(node) {
                    self.root = None;
                }
            }
            DomMutation::SetAttribute { node, name, value } => {
                self.nodes
                    .get_mut(&node)
                    .ok_or(DomObserverError::MissingNode(node))?
                    .attributes
                    .insert(name, value);
            }
            DomMutation::RemoveAttribute { node, name } => {
                self.nodes
                    .get_mut(&node)
                    .ok_or(DomObserverError::MissingNode(node))?
                    .attributes
                    .remove(&name);
            }
            DomMutation::SetText { node, value } => {
                self.nodes
                    .get_mut(&node)
                    .ok_or(DomObserverError::MissingNode(node))?
                    .text = Some(value);
            }
            DomMutation::AddEventListener { node, event } => {
                self.nodes
                    .get_mut(&node)
                    .ok_or(DomObserverError::MissingNode(node))?
                    .event_listeners
                    .insert(event);
            }
            DomMutation::RemoveEventListener { node, event } => {
                self.nodes
                    .get_mut(&node)
                    .ok_or(DomObserverError::MissingNode(node))?
                    .event_listeners
                    .remove(&event);
            }
        }
        self.generation += 1;
        Ok(())
    }

    fn collect_descendants(&self, id: EngineNodeId, descendants: &mut Vec<EngineNodeId>) {
        if let Some(node) = self.nodes.get(&id) {
            for child in &node.children {
                self.collect_descendants(*child, descendants);
                descendants.push(*child);
            }
        }
    }

    pub fn node(&self, id: EngineNodeId) -> Option<&RawNode> {
        self.nodes.get(&id)
    }

    pub fn reference(&self, node: EngineNodeId) -> Option<NodeReference> {
        self.node(node).map(|_| NodeReference {
            node,
            generation: self.generation,
        })
    }
    pub fn resolve(&self, reference: NodeReference) -> Result<&RawNode, DomObserverError> {
        if reference.generation != self.generation {
            return Err(DomObserverError::StaleReference);
        }
        self.node(reference.node)
            .ok_or(DomObserverError::MissingNode(reference.node))
    }
    pub fn advance_lifecycle(&mut self, lifecycle: DocumentLifecycle) {
        self.lifecycle = lifecycle;
        self.generation += 1;
    }
    pub fn begin_navigation(&mut self, url: impl Into<String>) {
        self.url = url.into();
        self.root = None;
        self.nodes.clear();
        self.lifecycle = DocumentLifecycle::Loading;
        self.generation += 1;
    }
}
