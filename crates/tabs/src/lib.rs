use browsai_navigation::NavigationState;
use browsai_profiles::ProfileId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use url::Url;
use uuid::Uuid;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct TabId(Uuid);
impl TabId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}
impl Default for TabId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tab {
    pub id: TabId,
    pub profile: ProfileId,
    pub navigation: NavigationState,
    pub active: bool,
    pub background: bool,
    pub generation: u64,
    pub execution: TabExecutionPolicy,
}

/// Scheduling limits applied to page work based on tab visibility.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TabExecutionPolicy {
    pub max_tasks_per_tick: u32,
    pub minimum_timer_interval_millis: u32,
}

impl TabExecutionPolicy {
    fn foreground() -> Self {
        Self {
            max_tasks_per_tick: 1_000,
            minimum_timer_interval_millis: 16,
        }
    }

    fn background() -> Self {
        Self {
            max_tasks_per_tick: 50,
            minimum_timer_interval_millis: 1_000,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TabSnapshot {
    pub id: TabId,
    pub profile: ProfileId,
    pub url: Url,
    pub generation: u64,
    pub active: bool,
    pub background: bool,
    pub execution: TabExecutionPolicy,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TabManager {
    tabs: BTreeMap<TabId, Tab>,
    active: Option<TabId>,
}

impl TabManager {
    pub fn open(&mut self, profile: ProfileId, url: Url) -> TabId {
        let id = TabId::new();
        let active = self.active.is_none();
        if active {
            self.active = Some(id.clone());
        }
        self.tabs.insert(
            id.clone(),
            Tab {
                id: id.clone(),
                profile,
                navigation: NavigationState::new(url),
                active,
                background: !active,
                generation: 0,
                execution: if active {
                    TabExecutionPolicy::foreground()
                } else {
                    TabExecutionPolicy::background()
                },
            },
        );
        id
    }
    pub fn get(&self, id: &TabId) -> Option<&Tab> {
        self.tabs.get(id)
    }
    pub fn get_mut(&mut self, id: &TabId) -> Option<&mut Tab> {
        self.tabs.get_mut(id)
    }

    pub fn navigate(&mut self, id: &TabId, url: Url) -> Option<Url> {
        let tab = self.tabs.get_mut(id)?;
        tab.navigation.start(url.clone());
        tab.navigation.complete();
        tab.generation += 1;
        Some(url)
    }

    pub fn back(&mut self, id: &TabId) -> Option<Url> {
        let tab = self.tabs.get_mut(id)?;
        let url = tab.navigation.back()?;
        tab.navigation.complete();
        tab.generation += 1;
        Some(url)
    }

    pub fn forward(&mut self, id: &TabId) -> Option<Url> {
        let tab = self.tabs.get_mut(id)?;
        let url = tab.navigation.forward()?;
        tab.navigation.complete();
        tab.generation += 1;
        Some(url)
    }
    pub fn activate(&mut self, id: &TabId) -> bool {
        if !self.tabs.contains_key(id) {
            return false;
        }
        for tab in self.tabs.values_mut() {
            tab.active = false;
            tab.background = true;
            tab.execution = TabExecutionPolicy::background();
        }
        if let Some(tab) = self.tabs.get_mut(id) {
            tab.active = true;
            tab.background = false;
            tab.execution = TabExecutionPolicy::foreground();
        }
        self.active = Some(id.clone());
        true
    }
    pub fn close(&mut self, id: &TabId) -> Option<Tab> {
        let removed = self.tabs.remove(id);
        if self.active.as_ref() == Some(id) {
            self.active = self.tabs.keys().next().cloned();
            if let Some(active) = self.active.clone() {
                self.activate(&active);
            }
        }
        removed
    }
    pub fn active(&self) -> Option<&Tab> {
        self.active.as_ref().and_then(|id| self.tabs.get(id))
    }
    pub fn list(&self) -> impl Iterator<Item = &Tab> {
        self.tabs.values()
    }

    pub fn for_profile<'a>(&'a self, profile: &'a ProfileId) -> impl Iterator<Item = &'a Tab> {
        self.tabs
            .values()
            .filter(move |tab| &tab.profile == profile)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Loads persisted tabs and repairs stale active/background flags at the
    /// boundary so recovery cannot produce multiple focused tabs.
    pub fn from_json(value: &str) -> Result<Self, serde_json::Error> {
        let mut manager: Self = serde_json::from_str(value)?;
        manager.normalize_active();
        Ok(manager)
    }

    pub fn active_id(&self) -> Option<&TabId> {
        self.active.as_ref()
    }

    pub fn snapshot(&self, id: &TabId) -> Option<TabSnapshot> {
        self.tabs.get(id).map(|tab| TabSnapshot {
            id: tab.id.clone(),
            profile: tab.profile.clone(),
            url: tab.navigation.current.clone(),
            generation: tab.generation,
            active: tab.active,
            background: tab.background,
            execution: tab.execution.clone(),
        })
    }

    pub fn agent_scope(&self, profile: &ProfileId) -> Vec<TabSnapshot> {
        self.for_profile(profile)
            .filter_map(|tab| self.snapshot(&tab.id))
            .collect()
    }

    fn normalize_active(&mut self) {
        self.active = self
            .active
            .clone()
            .filter(|id| self.tabs.contains_key(id))
            .or_else(|| self.tabs.keys().next().cloned());
        for tab in self.tabs.values_mut() {
            let active = self.active.as_ref() == Some(&tab.id);
            tab.active = active;
            tab.background = !active;
            tab.execution = if active {
                TabExecutionPolicy::foreground()
            } else {
                TabExecutionPolicy::background()
            };
        }
    }
}
