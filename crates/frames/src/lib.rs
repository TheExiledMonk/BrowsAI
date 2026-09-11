use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use url::Url;
use uuid::Uuid;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct FrameId(Uuid);
impl FrameId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}
impl Default for FrameId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub id: FrameId,
    pub parent: Option<FrameId>,
    pub url: Url,
    pub origin: String,
    pub child_frames: Vec<FrameId>,
}

/// Agent-visible scope for a frame subtree. The scope contains frame
/// topology and origin metadata, but does not grant access to another origin.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentFrameScope {
    pub frame: FrameId,
    pub origin: String,
    pub parent: Option<FrameId>,
    pub descendants: Vec<FrameId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FrameError {
    MissingFrame,
    CrossOriginDenied,
    DuplicateFrame,
    InvalidSnapshot,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FrameTree {
    frames: BTreeMap<FrameId, Frame>,
    root: FrameId,
}

impl FrameTree {
    pub fn new(url: Url) -> Self {
        let id = FrameId::new();
        let origin = url.origin().ascii_serialization();
        let root = Frame {
            id: id.clone(),
            parent: None,
            url,
            origin,
            child_frames: vec![],
        };
        let mut frames = BTreeMap::new();
        frames.insert(id.clone(), root);
        Self { frames, root: id }
    }
    pub fn root(&self) -> &FrameId {
        &self.root
    }
    pub fn add_child(&mut self, parent: &FrameId, url: Url) -> Result<FrameId, FrameError> {
        let parent_frame = self
            .frames
            .get_mut(parent)
            .ok_or(FrameError::MissingFrame)?;
        let id = FrameId::new();
        let frame = Frame {
            id: id.clone(),
            parent: Some(parent.clone()),
            origin: url.origin().ascii_serialization(),
            url,
            child_frames: vec![],
        };
        parent_frame.child_frames.push(id.clone());
        if self.frames.insert(id.clone(), frame).is_some() {
            return Err(FrameError::DuplicateFrame);
        }
        Ok(id)
    }
    pub fn get(&self, id: &FrameId) -> Option<&Frame> {
        self.frames.get(id)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_json(value: &str) -> Result<Self, FrameError> {
        let tree: Self = serde_json::from_str(value).map_err(|_| FrameError::InvalidSnapshot)?;
        if !tree.frames.contains_key(&tree.root)
            || tree.frames.values().any(|frame| {
                frame
                    .parent
                    .as_ref()
                    .is_some_and(|parent| !tree.frames.contains_key(parent))
                    || frame
                        .child_frames
                        .iter()
                        .any(|child| !tree.frames.contains_key(child))
            })
        {
            return Err(FrameError::InvalidSnapshot);
        }
        Ok(tree)
    }

    pub fn scoped_ids(&self, root: &FrameId) -> Result<Vec<FrameId>, FrameError> {
        if !self.frames.contains_key(root) {
            return Err(FrameError::MissingFrame);
        }
        let mut ids = vec![root.clone()];
        self.collect_descendants(root, &mut ids);
        Ok(ids)
    }

    pub fn agent_scope(&self, root: &FrameId) -> Result<AgentFrameScope, FrameError> {
        let frame = self.frames.get(root).ok_or(FrameError::MissingFrame)?;
        Ok(AgentFrameScope {
            frame: frame.id.clone(),
            origin: frame.origin.clone(),
            parent: frame.parent.clone(),
            descendants: self.scoped_ids(root)?.into_iter().skip(1).collect(),
        })
    }

    pub fn can_query(&self, from: &FrameId, target: &FrameId) -> Result<(), FrameError> {
        self.require_access(from, target)
    }

    pub fn can_action(&self, from: &FrameId, target: &FrameId) -> Result<(), FrameError> {
        self.require_access(from, target)
    }

    pub fn navigate(&mut self, id: &FrameId, url: Url) -> Result<(), FrameError> {
        let child_ids = self
            .frames
            .get(id)
            .ok_or(FrameError::MissingFrame)?
            .child_frames
            .clone();
        let mut descendants = Vec::new();
        for child in child_ids {
            self.collect_descendants(&child, &mut descendants);
            descendants.push(child);
        }
        for descendant in descendants {
            self.frames.remove(&descendant);
        }
        let frame = self.frames.get_mut(id).ok_or(FrameError::MissingFrame)?;
        frame.url = url;
        frame.origin = frame.url.origin().ascii_serialization();
        frame.child_frames.clear();
        Ok(())
    }

    fn collect_descendants(&self, id: &FrameId, descendants: &mut Vec<FrameId>) {
        if let Some(frame) = self.frames.get(id) {
            for child in &frame.child_frames {
                self.collect_descendants(child, descendants);
                descendants.push(child.clone());
            }
        }
    }

    pub fn can_access(&self, from: &FrameId, to: &FrameId) -> Result<bool, FrameError> {
        let from = self.frames.get(from).ok_or(FrameError::MissingFrame)?;
        let to = self.frames.get(to).ok_or(FrameError::MissingFrame)?;
        Ok(from.origin == to.origin)
    }
    pub fn require_access(&self, from: &FrameId, to: &FrameId) -> Result<(), FrameError> {
        if self.can_access(from, to)? {
            Ok(())
        } else {
            Err(FrameError::CrossOriginDenied)
        }
    }
}
