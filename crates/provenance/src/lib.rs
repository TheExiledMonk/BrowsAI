use browsai_transactions::{Consequence, PolicyDecision, Reversibility};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SourceKind {
    Dom,
    Accessibility,
    Layout,
    ComputedStyle,
    Text,
    EventHandler,
    JavaScript,
    NetworkRequest,
    NetworkResponse,
    Storage,
    Url,
    VisualInference,
    AgentInference,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProvenanceSource {
    pub kind: SourceKind,
    pub reference: String,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EvidenceRecord {
    pub source: ProvenanceSource,
    pub confidence: Confidence,
    pub observed_at_millis: u64,
    pub detail: Option<String>,
    pub redacted: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ExplainabilityReport {
    pub subject: String,
    pub summary: String,
    pub evidence: Vec<EvidenceRecord>,
}

impl ExplainabilityReport {
    pub fn new(subject: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            subject: subject.into(),
            summary: summary.into(),
            evidence: vec![],
        }
    }
    pub fn add(
        &mut self,
        source: ProvenanceSource,
        confidence: Confidence,
        observed_at_millis: u64,
        detail: Option<String>,
        redacted: bool,
    ) {
        self.evidence.push(EvidenceRecord {
            source,
            confidence,
            observed_at_millis,
            detail: detail.map(|value| if redacted { "[REDACTED]".into() } else { value }),
            redacted,
        });
    }
    pub fn inferred(&self) -> bool {
        self.evidence
            .iter()
            .any(|record| record.confidence.is_inferred())
    }
    pub fn redact_text(value: &str) -> String {
        let lower = value.to_ascii_lowercase();
        if ["password", "secret", "token", "private key", "card number"]
            .iter()
            .any(|key| lower.contains(key))
        {
            "[REDACTED]".into()
        } else {
            value.into()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Confidence(pub f32);

impl Confidence {
    pub const DIRECT: Self = Self(1.0);
    pub const STRONG: Self = Self(0.9);
    pub const PROBABLE: Self = Self(0.75);
    pub const AMBIGUOUS: Self = Self(0.5);
    pub fn new(value: f32) -> Self {
        if value.is_nan() {
            Self(0.0)
        } else {
            Self(value.clamp(0.0, 1.0))
        }
    }
    pub fn is_inferred(self) -> bool {
        self.0 < 1.0
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActionProvenance {
    pub action_id: String,
    pub agent_intent: String,
    pub target_reference: String,
    pub resolved_node_id: Option<String>,
    pub native_event_count: u32,
    pub consequence: Consequence,
    pub reversibility: Reversibility,
    pub policy_decision: PolicyDecision,
    pub capability_reference: Option<String>,
    pub audit_id: Option<String>,
    #[serde(default)]
    pub actor_id: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
}
