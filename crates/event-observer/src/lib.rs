//! Ordered, bounded change stream primitives for browser and agent events.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ChangeEvent {
    NodeAdded {
        node_id: String,
    },
    NodeRemoved {
        node_id: String,
    },
    NodeChanged {
        node_id: String,
    },
    StyleChanged {
        node_id: String,
    },
    LayoutChanged {
        node_id: String,
    },
    RuntimeTaskQueued {
        task_id: String,
    },
    MicrotaskQueued {
        task_id: String,
    },
    NavigationStarted {
        url: String,
    },
    NavigationCompleted {
        url: String,
    },
    RequestStarted {
        request_id: String,
    },
    RequestCompleted {
        request_id: String,
    },
    RequestFailed {
        request_id: String,
        error: String,
    },
    InputDispatched {
        event: String,
    },
    StorageChanged {
        origin: String,
        key: String,
    },
    PermissionChanged {
        origin: String,
        permission: String,
    },
    AgentStateChanged {
        session_id: String,
        state: String,
    },
    DialogOpened {
        node_id: String,
    },
    DialogClosed {
        node_id: String,
    },
    FocusChanged {
        node_id: Option<String>,
    },
    DownloadStarted {
        download_id: String,
    },
    PermissionRequested {
        permission: String,
    },
    CredentialRequired {
        origin: String,
    },
    TransactionDetected {
        category: String,
    },
    ChallengeObserved {
        provider: String,
        kind: String,
        url: String,
    },
    ChallengeCleared {
        provider: String,
    },
    ProfileInconsistency {
        fingerprint_id: String,
        kind: String,
        declared: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TraceEntry {
    pub sequence: u64,
    pub event: ChangeEvent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TraceError {
    NonMonotonicSequence,
    Serialization,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EventTrace {
    entries: Vec<TraceEntry>,
    next_sequence: u64,
}

impl EventTrace {
    pub fn record(&mut self, event: ChangeEvent) -> u64 {
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        self.entries.push(TraceEntry { sequence, event });
        sequence
    }
    pub fn entries(&self) -> &[TraceEntry] {
        &self.entries
    }
    pub fn replay(&self, mut receive: impl FnMut(&TraceEntry)) -> Result<(), TraceError> {
        let mut previous = None;
        for entry in &self.entries {
            if previous.is_some_and(|sequence| entry.sequence <= sequence) {
                return Err(TraceError::NonMonotonicSequence);
            }
            previous = Some(entry.sequence);
            receive(entry);
        }
        Ok(())
    }
    pub fn to_json(&self) -> Result<String, TraceError> {
        serde_json::to_string(self).map_err(|_| TraceError::Serialization)
    }
    pub fn from_json(value: &str) -> Result<Self, TraceError> {
        let mut trace: Self = serde_json::from_str(value).map_err(|_| TraceError::Serialization)?;
        let mut previous = None;
        for entry in &trace.entries {
            if previous.is_some_and(|sequence| entry.sequence <= sequence) {
                return Err(TraceError::NonMonotonicSequence);
            }
            previous = Some(entry.sequence);
        }
        trace.next_sequence = trace
            .entries
            .iter()
            .map(|entry| entry.sequence)
            .max()
            .map_or(0, |sequence| sequence.saturating_add(1));
        Ok(trace)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamError {
    pub dropped: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamBatch {
    pub events: Vec<ChangeEvent>,
    pub dropped: u64,
}

#[derive(Clone, Debug)]
pub struct ChangeStream {
    capacity: usize,
    dropped: u64,
    events: VecDeque<ChangeEvent>,
}

impl ChangeStream {
    pub fn bounded(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            dropped: 0,
            events: VecDeque::new(),
        }
    }
    pub fn publish(&mut self, event: ChangeEvent) {
        if self.events.len() == self.capacity {
            self.events.pop_front();
            self.dropped += 1;
        }
        self.events.push_back(event);
    }
    pub fn dropped(&self) -> u64 {
        self.dropped
    }
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
    pub fn drain(&mut self) -> Result<Vec<ChangeEvent>, StreamError> {
        let events = self.events.drain(..).collect();
        if self.dropped == 0 {
            Ok(events)
        } else {
            let error = StreamError {
                dropped: self.dropped,
            };
            self.dropped = 0;
            Err(error)
        }
    }

    /// Drain retained events while reporting overflow loss without discarding the retained batch.
    pub fn drain_with_loss(&mut self) -> StreamBatch {
        StreamBatch {
            events: self.events.drain(..).collect(),
            dropped: std::mem::take(&mut self.dropped),
        }
    }
    pub fn replay(&self) -> impl Iterator<Item = &ChangeEvent> {
        self.events.iter()
    }
}

impl Default for ChangeStream {
    fn default() -> Self {
        Self::bounded(256)
    }
}
