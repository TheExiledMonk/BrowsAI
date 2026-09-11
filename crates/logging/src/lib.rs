use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LogEvent {
    pub level: LogLevel,
    pub correlation_id: Uuid,
    pub message: String,
    pub fields: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default)]
pub struct SecretSafeLogger {
    events: Vec<LogEvent>,
}

impl SecretSafeLogger {
    pub fn log(
        &mut self,
        level: LogLevel,
        correlation_id: Uuid,
        message: impl Into<String>,
        fields: BTreeMap<String, String>,
    ) {
        let fields = fields
            .into_iter()
            .map(|(key, value)| {
                let sensitive = [
                    "password",
                    "secret",
                    "token",
                    "private_key",
                    "card_number",
                    "totp",
                ]
                .iter()
                .any(|name| key.to_ascii_lowercase().contains(name));
                (
                    key,
                    if sensitive {
                        "[REDACTED]".into()
                    } else {
                        value
                    },
                )
            })
            .collect();
        self.events.push(LogEvent {
            level,
            correlation_id,
            message: redact_message(&message.into()),
            fields,
        });
    }
    pub fn events(&self) -> &[LogEvent] {
        &self.events
    }

    pub fn events_for(&self, correlation_id: Uuid) -> impl Iterator<Item = &LogEvent> {
        self.events
            .iter()
            .filter(move |event| event.correlation_id == correlation_id)
    }

    pub fn drain(&mut self) -> impl Iterator<Item = LogEvent> + '_ {
        self.events.drain(..)
    }

    pub fn debug_trace(&self) -> String {
        serde_json::to_string(&self.events).expect("log events serialize")
    }

    pub fn from_json(value: &str) -> Result<Self, serde_json::Error> {
        Ok(Self {
            events: serde_json::from_str(value)?,
        })
    }
}

fn redact_message(message: &str) -> String {
    let lower = message.to_ascii_lowercase();
    if [
        "password",
        "secret",
        "token",
        "private key",
        "card number",
        "totp",
    ]
    .iter()
    .any(|name| lower.contains(name))
    {
        "[REDACTED]".into()
    } else {
        message.into()
    }
}
