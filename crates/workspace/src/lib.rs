use browsai_profiles::ProfileId;
use browsai_tabs::{TabId, TabManager};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct WorkspaceId(Uuid);
impl WorkspaceId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}
impl Default for WorkspaceId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct WindowId(Uuid);
impl WindowId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}
impl Default for WindowId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Window {
    pub id: WindowId,
    pub tabs: Vec<TabId>,
    pub width: u32,
    pub height: u32,
    pub focused: bool,
}

#[derive(Clone, Debug)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub profile: ProfileId,
    pub tabs: TabManager,
    pub windows: BTreeMap<WindowId, Window>,
    locks: BTreeSet<String>,
    permissions: BTreeSet<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceCheckpoint {
    pub id: WorkspaceId,
    pub profile: ProfileId,
    pub tabs: TabManager,
    pub windows: BTreeMap<WindowId, Window>,
    pub locks: BTreeSet<String>,
    #[serde(default)]
    pub permissions: BTreeSet<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkspaceError {
    ProfileMismatch,
    TabProfileMismatch,
    InvalidCheckpoint,
    DuplicateTab,
}

impl Workspace {
    pub fn new(profile: ProfileId) -> Self {
        Self {
            id: WorkspaceId::new(),
            profile,
            tabs: TabManager::default(),
            windows: BTreeMap::new(),
            locks: BTreeSet::new(),
            permissions: BTreeSet::new(),
        }
    }
    pub fn create_window(&mut self, width: u32, height: u32) -> WindowId {
        let id = WindowId::new();
        self.windows.insert(
            id.clone(),
            Window {
                id: id.clone(),
                tabs: vec![],
                width,
                height,
                focused: self.windows.is_empty(),
            },
        );
        id
    }
    pub fn attach_tab(&mut self, window: &WindowId, tab: TabId) -> bool {
        let Some(tab_record) = self.tabs.get(&tab) else {
            return false;
        };
        if tab_record.profile != self.profile {
            return false;
        }
        if self
            .windows
            .values()
            .any(|existing| existing.tabs.iter().any(|id| id == &tab))
        {
            return false;
        }
        self.windows
            .get_mut(window)
            .map(|window| {
                window.tabs.push(tab);
                true
            })
            .unwrap_or(false)
    }

    pub fn focus_window(&mut self, id: &WindowId) -> bool {
        if !self.windows.contains_key(id) {
            return false;
        }
        for window in self.windows.values_mut() {
            window.focused = false;
        }
        self.windows
            .get_mut(id)
            .expect("window checked above")
            .focused = true;
        true
    }

    pub fn close_window(&mut self, id: &WindowId) -> Option<Window> {
        let removed = self.windows.remove(id);
        if removed.as_ref().is_some_and(|window| window.focused) {
            if let Some(next) = self.windows.keys().next().cloned() {
                let _ = self.focus_window(&next);
            }
        }
        removed
    }
    pub fn acquire_lock(&mut self, resource: impl Into<String>) -> bool {
        self.locks.insert(resource.into())
    }
    pub fn release_lock(&mut self, resource: &str) -> bool {
        self.locks.remove(resource)
    }
    pub fn is_locked(&self, resource: &str) -> bool {
        self.locks.contains(resource)
    }

    pub fn grant_permission(&mut self, permission: impl Into<String>) -> bool {
        self.permissions.insert(permission.into())
    }

    pub fn revoke_permission(&mut self, permission: &str) -> bool {
        self.permissions.remove(permission)
    }

    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.contains(permission)
    }

    pub fn checkpoint(&self) -> WorkspaceCheckpoint {
        WorkspaceCheckpoint {
            id: self.id.clone(),
            profile: self.profile.clone(),
            tabs: self.tabs.clone(),
            windows: self.windows.clone(),
            locks: self.locks.clone(),
            permissions: self.permissions.clone(),
        }
    }

    pub fn from_checkpoint(
        checkpoint: WorkspaceCheckpoint,
        expected_profile: &ProfileId,
    ) -> Result<Self, WorkspaceError> {
        if &checkpoint.profile != expected_profile {
            return Err(WorkspaceError::ProfileMismatch);
        }
        if checkpoint
            .tabs
            .list()
            .any(|tab| &tab.profile != expected_profile)
        {
            return Err(WorkspaceError::TabProfileMismatch);
        }
        let tab_ids = checkpoint
            .tabs
            .list()
            .map(|tab| tab.id.clone())
            .collect::<BTreeSet<_>>();
        let mut attached = BTreeSet::new();
        for window in checkpoint.windows.values() {
            for tab in &window.tabs {
                if !tab_ids.contains(tab) || !attached.insert(tab.clone()) {
                    return Err(WorkspaceError::InvalidCheckpoint);
                }
            }
        }
        let focused = checkpoint
            .windows
            .values()
            .filter(|window| window.focused)
            .count();
        if focused > 1 || (focused == 0 && !checkpoint.windows.is_empty()) {
            return Err(WorkspaceError::InvalidCheckpoint);
        }
        Ok(Self {
            id: checkpoint.id,
            profile: checkpoint.profile,
            tabs: checkpoint.tabs,
            windows: checkpoint.windows,
            locks: checkpoint.locks,
            permissions: checkpoint.permissions,
        })
    }
}
