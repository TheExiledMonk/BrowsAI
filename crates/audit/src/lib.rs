//! Append-only, secret-safe audit journal.

use browsai_input::ActionType;
use browsai_provenance::ActionProvenance;
use browsai_transactions::TransactionClassification;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AuditEvent {
    Action {
        provenance: ActionProvenance,
        classification: TransactionClassification,
        parameter_names: Vec<String>,
        native_event_count: u32,
    },
    CapabilityUsed {
        capability_reference: String,
        scope: String,
    },
    PermissionDecision {
        permission: String,
        origin: String,
        decision: String,
    },
    SecretBrokerOperation {
        opaque_reference: String,
        operation: String,
    },
    Navigation {
        url: String,
    },
    Download {
        opaque_reference: String,
    },
    Upload {
        opaque_reference: String,
    },
    Recovery {
        component: String,
        outcome: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AuditRecord {
    pub sequence: u64,
    pub audit_id: String,
    pub event: AuditEvent,
    pub previous_integrity: u64,
    pub integrity: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditJournal {
    records: Vec<AuditRecord>,
    next_sequence: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuditError {
    Serialization,
    Integrity,
}

impl AuditJournal {
    pub fn append(&mut self, audit_id: impl Into<String>, event: AuditEvent) -> &AuditRecord {
        let audit_id = audit_id.into();
        let previous_integrity = self.records.last().map_or(0, |record| record.integrity);
        let record = AuditRecord {
            sequence: self.next_sequence,
            audit_id,
            event,
            previous_integrity,
            integrity: 0,
        };
        let mut record = record;
        record.integrity = integrity(&record);
        self.next_sequence += 1;
        self.records.push(record);
        self.records.last().expect("record was just appended")
    }
    pub fn records(&self) -> &[AuditRecord] {
        &self.records
    }
    pub fn len(&self) -> usize {
        self.records.len()
    }
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn records_for<'a>(
        &'a self,
        audit_id: &'a str,
    ) -> impl Iterator<Item = &'a AuditRecord> + 'a {
        self.records
            .iter()
            .filter(move |record| record.audit_id == audit_id)
    }

    pub fn replay(&self, mut receive: impl FnMut(&AuditRecord)) -> Result<(), AuditError> {
        if !self.verify_integrity() {
            return Err(AuditError::Integrity);
        }
        for record in &self.records {
            receive(record);
        }
        Ok(())
    }

    pub fn retain_last(&mut self, max_records: usize) -> usize {
        let removed = self.records.len().saturating_sub(max_records);
        if removed == 0 {
            return 0;
        }
        self.records.drain(..removed);
        let mut previous = 0;
        for (sequence, record) in self.records.iter_mut().enumerate() {
            record.sequence = sequence as u64;
            record.previous_integrity = previous;
            record.integrity = integrity(record);
            previous = record.integrity;
        }
        self.next_sequence = self.records.len() as u64;
        removed
    }

    pub fn verify_integrity(&self) -> bool {
        let mut previous = 0;
        for (expected_sequence, record) in self.records.iter().enumerate() {
            if record.sequence != expected_sequence as u64
                || record.previous_integrity != previous
                || integrity(record) != record.integrity
            {
                return false;
            }
            previous = record.integrity;
        }
        self.next_sequence == self.records.len() as u64
    }

    pub fn to_json(&self) -> Result<String, AuditError> {
        serde_json::to_string(self).map_err(|_| AuditError::Serialization)
    }

    pub fn from_json(value: &str) -> Result<Self, AuditError> {
        let journal: Self = serde_json::from_str(value).map_err(|_| AuditError::Serialization)?;
        if !journal.verify_integrity() {
            return Err(AuditError::Integrity);
        }
        Ok(journal)
    }
}

fn integrity(record: &AuditRecord) -> u64 {
    let bytes = serde_json::to_vec(&(
        record.sequence,
        &record.audit_id,
        &record.event,
        record.previous_integrity,
    ))
    .expect("audit record serializes");
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

pub fn safe_parameter_names(parameters: &serde_json::Value) -> Vec<String> {
    parameters
        .as_object()
        .map(|object| object.keys().cloned().collect())
        .unwrap_or_default()
}

pub fn action_event(
    provenance: ActionProvenance,
    classification: TransactionClassification,
    action_type: &ActionType,
    parameters: &serde_json::Value,
    native_event_count: u32,
) -> AuditEvent {
    let mut parameter_names = safe_parameter_names(parameters);
    parameter_names.push(format!("action:{action_type:?}"));
    AuditEvent::Action {
        provenance,
        classification,
        parameter_names,
        native_event_count,
    }
}
