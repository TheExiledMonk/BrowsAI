//! Consequence and confirmation policy. Policy decisions are made outside an
//! LLM and are applied before native input is dispatched.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Consequence {
    Read,
    Navigate,
    LocalWrite,
    RemoteWrite,
    Send,
    Publish,
    Delete,
    Purchase,
    Transfer,
    Authenticate,
    PermissionChange,
    SecurityChange,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Reversibility {
    LocalReversible,
    RemoteReversible,
    RemoteIrreversible,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum PolicyDecision {
    Allow,
    RequireConfirmation,
    Deny,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransactionClassification {
    pub consequence: Consequence,
    pub reversibility: Reversibility,
    pub decision: PolicyDecision,
    pub reason: String,
    pub confidence: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ConfirmationState {
    Pending,
    Approved,
    Denied,
    Expired,
    Escalated,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConfirmationRequest {
    pub id: Uuid,
    pub owner: String,
    pub summary: String,
    pub classification: TransactionClassification,
    pub state: ConfirmationState,
    pub expires_at_tick: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConfirmationEvent {
    pub id: Uuid,
    pub actor: Option<String>,
    pub state: ConfirmationState,
    pub tick: u64,
}

#[derive(Clone, Debug, Default)]
pub struct ConfirmationQueue {
    requests: BTreeMap<Uuid, ConfirmationRequest>,
    events: Vec<ConfirmationEvent>,
}

impl ConfirmationQueue {
    pub fn enqueue(
        &mut self,
        owner: impl Into<String>,
        summary: impl Into<String>,
        classification: TransactionClassification,
        timeout_ticks: u64,
        now: u64,
    ) -> Uuid {
        let id = Uuid::new_v4();
        self.requests.insert(
            id,
            ConfirmationRequest {
                id,
                owner: owner.into(),
                summary: summary.into(),
                classification,
                state: ConfirmationState::Pending,
                expires_at_tick: now.saturating_add(timeout_ticks.max(1)),
            },
        );
        self.events.push(ConfirmationEvent {
            id,
            actor: None,
            state: ConfirmationState::Pending,
            tick: now,
        });
        id
    }
    pub fn get(&self, id: Uuid) -> Option<&ConfirmationRequest> {
        self.requests.get(&id)
    }
    pub fn approve(&mut self, id: Uuid, actor: &str, now: u64) -> bool {
        self.transition(id, actor, ConfirmationState::Approved, now)
    }
    pub fn deny(&mut self, id: Uuid, actor: &str, now: u64) -> bool {
        self.transition(id, actor, ConfirmationState::Denied, now)
    }
    pub fn escalate(&mut self, id: Uuid, actor: &str, now: u64) -> bool {
        self.transition(id, actor, ConfirmationState::Escalated, now)
    }
    pub fn expire(&mut self, now: u64) {
        for request in self.requests.values_mut() {
            if request.state == ConfirmationState::Pending && request.expires_at_tick <= now {
                request.state = ConfirmationState::Expired;
                self.events.push(ConfirmationEvent {
                    id: request.id,
                    actor: None,
                    state: ConfirmationState::Expired,
                    tick: now,
                });
            }
        }
    }
    fn transition(&mut self, id: Uuid, actor: &str, state: ConfirmationState, now: u64) -> bool {
        let Some(request) = self.requests.get_mut(&id) else {
            return false;
        };
        if request.state != ConfirmationState::Pending || request.owner != actor {
            return false;
        };
        if request.expires_at_tick <= now {
            request.state = ConfirmationState::Expired;
            return false;
        }
        request.state = state;
        self.events.push(ConfirmationEvent {
            id,
            actor: Some(actor.to_owned()),
            state,
            tick: now,
        });
        true
    }

    pub fn drain_events(&mut self) -> impl Iterator<Item = ConfirmationEvent> + '_ {
        self.events.drain(..)
    }
}

#[derive(Clone, Debug)]
pub struct TransactionPolicy {
    pub require_confirmation_for_unknown: bool,
    pub allow_delete: bool,
    pub allow_financial: bool,
    pub allow_security_changes: bool,
}

impl Default for TransactionPolicy {
    fn default() -> Self {
        Self {
            require_confirmation_for_unknown: true,
            allow_delete: false,
            allow_financial: false,
            allow_security_changes: false,
        }
    }
}

impl TransactionPolicy {
    pub fn classify(
        &self,
        consequence: Consequence,
        reversibility: Reversibility,
    ) -> TransactionClassification {
        let (decision, reason) = match consequence {
            Consequence::Delete if !self.allow_delete => {
                (PolicyDecision::Deny, "delete is disabled by policy")
            }
            Consequence::Purchase | Consequence::Transfer if !self.allow_financial => (
                PolicyDecision::RequireConfirmation,
                "financial effect requires explicit confirmation",
            ),
            Consequence::SecurityChange | Consequence::PermissionChange
                if !self.allow_security_changes =>
            {
                (
                    PolicyDecision::RequireConfirmation,
                    "security or permission change requires explicit confirmation",
                )
            }
            Consequence::Send | Consequence::Publish | Consequence::Authenticate => (
                PolicyDecision::RequireConfirmation,
                "external side effect requires explicit confirmation",
            ),
            Consequence::Unknown if self.require_confirmation_for_unknown => (
                PolicyDecision::RequireConfirmation,
                "unknown effect cannot be treated as safe",
            ),
            _ => (PolicyDecision::Allow, "policy permits this action"),
        };
        TransactionClassification {
            consequence,
            reversibility,
            decision,
            reason: reason.into(),
            confidence: if matches!(consequence, Consequence::Unknown) {
                0.5
            } else {
                1.0
            },
        }
    }
}
