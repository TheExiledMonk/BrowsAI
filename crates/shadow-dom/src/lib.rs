use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ShadowRootMode {
    Open,
    Closed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ShadowRoot {
    pub id: Uuid,
    pub host_node: u64,
    pub mode: ShadowRootMode,
    pub children: Vec<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ShadowEvent {
    RootAttached { root_id: Uuid, host_node: u64 },
    ChildAdded { root_id: Uuid, node: u64 },
    ChildRemoved { root_id: Uuid, node: u64 },
}

#[derive(Clone, Debug, Default)]
pub struct ShadowDomRegistry {
    roots: BTreeMap<Uuid, ShadowRoot>,
    selectors: BTreeMap<(Uuid, u64), Vec<String>>,
    events: Vec<ShadowEvent>,
}

impl ShadowDomRegistry {
    pub fn attach(&mut self, host_node: u64, mode: ShadowRootMode) -> Uuid {
        if let Some(existing) = self.roots.values().find(|root| root.host_node == host_node) {
            return existing.id;
        }
        let id = Uuid::new_v4();
        self.roots.insert(
            id,
            ShadowRoot {
                id,
                host_node,
                mode,
                children: vec![],
            },
        );
        self.events.push(ShadowEvent::RootAttached {
            root_id: id,
            host_node,
        });
        id
    }
    pub fn add_child(&mut self, root: &Uuid, node: u64) -> bool {
        let added = self
            .roots
            .get_mut(root)
            .map(|root| {
                if root.children.contains(&node) {
                    return false;
                }
                root.children.push(node);
                true
            })
            .unwrap_or(false);
        if added {
            self.events.push(ShadowEvent::ChildAdded {
                root_id: *root,
                node,
            });
        }
        added
    }

    pub fn remove_child(&mut self, root: &Uuid, node: u64) -> bool {
        let root_id = *root;
        let Some(root) = self.roots.get_mut(root) else {
            return false;
        };
        let before = root.children.len();
        root.children.retain(|child| *child != node);
        let removed = root.children.len() != before;
        if removed {
            self.selectors.remove(&(root_id, node));
            self.events
                .push(ShadowEvent::ChildRemoved { root_id, node });
        }
        removed
    }

    pub fn root_for_host(&self, host_node: u64) -> Option<&ShadowRoot> {
        self.roots.values().find(|root| root.host_node == host_node)
    }
    pub fn exposed_children(&self, root: &Uuid, privileged: bool) -> Option<Vec<u64>> {
        let root = self.roots.get(root)?;
        if matches!(root.mode, ShadowRootMode::Open) || privileged {
            Some(root.children.clone())
        } else {
            Some(vec![])
        }
    }

    /// Associates a simple composed-tree selector token with a shadow child.
    /// Selectors are metadata only; they never expose closed-root children.
    pub fn add_selector(&mut self, root: &Uuid, node: u64, selector: impl Into<String>) -> bool {
        let Some(shadow_root) = self.roots.get(root) else {
            return false;
        };
        if !shadow_root.children.contains(&node) {
            return false;
        }
        let selectors = self.selectors.entry((*root, node)).or_default();
        let selector = selector.into();
        if selectors.contains(&selector) {
            return false;
        }
        selectors.push(selector);
        true
    }

    pub fn query_selector(
        &self,
        root: &Uuid,
        selector: &str,
        privileged: bool,
    ) -> Option<Vec<u64>> {
        let children = self.exposed_children(root, privileged)?;
        Some(
            children
                .into_iter()
                .filter(|node| {
                    self.selectors
                        .get(&(*root, *node))
                        .is_some_and(|selectors| selectors.iter().any(|value| value == selector))
                })
                .collect(),
        )
    }

    pub fn drain_events(&mut self) -> impl Iterator<Item = ShadowEvent> + '_ {
        self.events.drain(..)
    }
}
