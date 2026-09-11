use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, VecDeque};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ProcessKind {
    Host,
    BrowserBroker,
    CredentialBroker,
    ProfileManager,
    PermissionManager,
    Network,
    FileBroker,
    DownloadBroker,
    Audit,
    WorkspaceManager,
    Renderer,
    Worker,
    AgentRuntime,
    Compositor,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IpcEnvelope {
    pub protocol_version: u16,
    pub sequence: u64,
    pub request_id: Uuid,
    pub source: ProcessKind,
    pub destination: ProcessKind,
    pub workspace: String,
    pub profile: String,
    pub origin: Option<String>,
    pub capability: Option<String>,
    pub payload: IpcMessage,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum IpcMessage {
    Handshake {
        protocol_version: u16,
    },
    Request {
        method: String,
        body: serde_json::Value,
    },
    Response {
        ok: bool,
        body: serde_json::Value,
    },
    Cancel {
        request_id: Uuid,
    },
    CrashNotice {
        process: ProcessKind,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IpcError {
    Unauthorized,
    VersionMismatch,
    Backpressure,
    OutOfOrder,
    RawSecretPayload,
}

#[derive(Clone, Debug)]
pub struct IpcRouter {
    version: u16,
    allowed: BTreeSet<(ProcessKind, ProcessKind)>,
}

impl IpcRouter {
    pub fn new(version: u16) -> Self {
        Self {
            version,
            allowed: BTreeSet::new(),
        }
    }
    pub fn allow(&mut self, source: ProcessKind, destination: ProcessKind) {
        self.allowed.insert((source, destination));
    }
    pub fn validate(&self, message: &IpcEnvelope) -> Result<(), IpcError> {
        if message.protocol_version != self.version {
            return Err(IpcError::VersionMismatch);
        }
        if let IpcMessage::Handshake { protocol_version } = message.payload {
            if protocol_version != self.version {
                return Err(IpcError::VersionMismatch);
            }
        }
        if !self
            .allowed
            .contains(&(message.source, message.destination))
        {
            return Err(IpcError::Unauthorized);
        }
        if matches!(
            &message.payload,
            IpcMessage::Request { body, .. } | IpcMessage::Response { body, .. }
                if contains_raw_secret_field(body)
        ) {
            return Err(IpcError::RawSecretPayload);
        }
        Ok(())
    }
}

fn contains_raw_secret_field(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(fields) => fields.iter().any(|(key, value)| {
            let key = key.to_ascii_lowercase();
            let normalized = key.replace(['_', '-', '.'], "");
            [
                "password",
                "secret",
                "token",
                "seed",
                "privatekey",
                "cardnumber",
                "cvv",
            ]
            .iter()
            .any(|sensitive| normalized.contains(sensitive))
                || contains_raw_secret_field(value)
        }),
        serde_json::Value::Array(values) => values.iter().any(contains_raw_secret_field),
        _ => false,
    }
}

#[derive(Clone, Debug)]
pub struct IpcChannel {
    capacity: usize,
    next_sequence: u64,
    queue: VecDeque<IpcEnvelope>,
}

impl IpcChannel {
    pub fn bounded(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            next_sequence: 0,
            queue: VecDeque::new(),
        }
    }
    pub fn send(&mut self, mut message: IpcEnvelope) -> Result<(), IpcError> {
        if self.queue.len() >= self.capacity {
            return Err(IpcError::Backpressure);
        }
        message.sequence = self.next_sequence;
        self.next_sequence += 1;
        self.queue.push_back(message);
        Ok(())
    }
    pub fn receive(&mut self) -> Option<IpcEnvelope> {
        self.queue.pop_front()
    }

    pub fn cancel(&mut self, request_id: Uuid) -> usize {
        let before = self.queue.len();
        self.queue
            .retain(|message| message.request_id != request_id);
        before - self.queue.len()
    }

    pub fn reconnect(&mut self) {
        self.queue.clear();
        self.next_sequence = 0;
    }
}
