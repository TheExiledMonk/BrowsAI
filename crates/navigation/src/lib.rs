use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum NavigationStatus {
    Idle,
    Loading,
    Stopped,
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub url: Url,
    pub title: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NavigationState {
    pub current: Url,
    pub history: Vec<HistoryEntry>,
    pub index: usize,
    pub status: NavigationStatus,
    pub generation: u64,
}

impl NavigationState {
    pub fn new(url: Url) -> Self {
        Self {
            current: url.clone(),
            history: vec![HistoryEntry { url, title: None }],
            index: 0,
            status: NavigationStatus::Idle,
            generation: 0,
        }
    }
    pub fn start(&mut self, url: Url) {
        self.status = NavigationStatus::Loading;
        let same_document = self.current.origin() == url.origin()
            && self.current.path() == url.path()
            && self.current.query() == url.query()
            && self.current.fragment() != url.fragment();
        if self.current == url {
            self.history[self.index].url = url.clone();
        } else if !same_document {
            self.history.truncate(self.index + 1);
            self.history.push(HistoryEntry {
                url: url.clone(),
                title: None,
            });
            self.index += 1;
        } else {
            self.history[self.index].url = url.clone();
        }
        self.current = url;
        self.generation += 1;
    }
    pub fn complete(&mut self) {
        self.status = NavigationStatus::Idle;
    }
    pub fn fail(&mut self, reason: impl Into<String>) {
        self.status = NavigationStatus::Failed(reason.into());
    }

    pub fn reload(&mut self) -> Url {
        self.status = NavigationStatus::Loading;
        self.generation += 1;
        self.current.clone()
    }
    pub fn stop(&mut self) {
        if matches!(self.status, NavigationStatus::Loading) {
            self.status = NavigationStatus::Stopped;
        }
    }
    pub fn back(&mut self) -> Option<Url> {
        if self.index == 0 {
            return None;
        }
        self.index -= 1;
        self.current = self.history[self.index].url.clone();
        self.status = NavigationStatus::Loading;
        self.generation += 1;
        Some(self.current.clone())
    }
    pub fn forward(&mut self) -> Option<Url> {
        if self.index + 1 >= self.history.len() {
            return None;
        }
        self.index += 1;
        self.current = self.history[self.index].url.clone();
        self.status = NavigationStatus::Loading;
        self.generation += 1;
        Some(self.current.clone())
    }
    pub fn can_back(&self) -> bool {
        self.index > 0
    }
    pub fn can_forward(&self) -> bool {
        self.index + 1 < self.history.len()
    }

    pub fn is_cross_origin(&self, url: &Url) -> bool {
        self.current.origin() != url.origin()
    }
}
