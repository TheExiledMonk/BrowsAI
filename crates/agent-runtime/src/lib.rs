//! Capability-scoped lifecycle and quota enforcement for agent executions.

use browsai_agent_protocol::Query;
use browsai_agent_tree::SemanticRole;
use browsai_input::{
    generate_human_trajectory, HumanMouseTrajectory, MouseTrajectoryOptions, NativeInputEvent,
};
use browsai_sandbox::{Capability, SandboxPolicy};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct AgentSessionId(Uuid);

impl AgentSessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for AgentSessionId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SessionState {
    Created,
    Running,
    Paused,
    Cancelled,
    Exited,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgentLimits {
    pub max_actions: u64,
    pub max_events: u64,
    pub max_output_bytes: u64,
    pub max_runtime_millis: u64,
}

impl Default for AgentLimits {
    fn default() -> Self {
        Self {
            max_actions: 100,
            max_events: 1_000,
            max_output_bytes: 1_048_576,
            max_runtime_millis: 60_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UntrustedPageData {
    pub text: String,
    pub source: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ContentAssessment {
    pub source: String,
    pub suspicious: bool,
    pub signals: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum AuthorityDecision {
    DeniedUntrustedContent,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PageContentAuthority {
    pub source: String,
    pub decision: AuthorityDecision,
    pub may_grant_capabilities: bool,
    pub may_change_system_instructions: bool,
    pub may_satisfy_confirmation: bool,
}

impl PageContentAuthority {
    pub fn evaluate(data: &UntrustedPageData) -> Self {
        Self {
            source: data.source.clone(),
            decision: AuthorityDecision::DeniedUntrustedContent,
            may_grant_capabilities: false,
            may_change_system_instructions: false,
            may_satisfy_confirmation: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MemoryFact {
    pub key: String,
    pub value: String,
    pub source: String,
    pub observed_at_tick: u64,
}

#[derive(Clone, Debug, Default)]
pub struct AgentMemory {
    facts: BTreeMap<String, MemoryFact>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct TakeoverId(Uuid);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ControlOwner {
    Agent,
    Human,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum TakeoverState {
    Requested,
    Active,
    Paused,
    Completed,
    Expired,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TakeoverSession {
    pub id: TakeoverId,
    pub agent_id: String,
    pub reason: String,
    pub owner: ControlOwner,
    pub state: TakeoverState,
    pub expires_at_tick: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TakeoverAuditEvent {
    pub takeover_id: TakeoverId,
    pub state: TakeoverState,
    pub tick: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SolveAuditEvent {
    pub agent_id: String,
    pub provider: String,
    pub capability_used: String,
    pub takeover_id: Option<String>,
    pub observed_at_tick: u64,
    pub page_id: Option<u64>,
    pub target_node_id: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct TakeoverManager {
    sessions: BTreeMap<TakeoverId, TakeoverSession>,
    audit: Vec<TakeoverAuditEvent>,
}

impl TakeoverManager {
    pub fn request(
        &mut self,
        agent_id: impl Into<String>,
        reason: impl Into<String>,
        timeout_ticks: u64,
        now: u64,
    ) -> TakeoverId {
        let id = TakeoverId(Uuid::new_v4());
        let session = TakeoverSession {
            id,
            agent_id: agent_id.into(),
            reason: reason.into(),
            owner: ControlOwner::Agent,
            state: TakeoverState::Requested,
            expires_at_tick: now.saturating_add(timeout_ticks.max(1)),
        };
        self.sessions.insert(id, session);
        self.audit.push(TakeoverAuditEvent {
            takeover_id: id,
            state: TakeoverState::Requested,
            tick: now,
        });
        id
    }
    pub fn accept(&mut self, id: TakeoverId, now: u64) -> bool {
        self.transition(id, ControlOwner::Human, TakeoverState::Active, now)
    }
    pub fn pause(&mut self, id: TakeoverId, now: u64) -> bool {
        self.transition(id, ControlOwner::Human, TakeoverState::Paused, now)
    }
    pub fn resume(&mut self, id: TakeoverId, now: u64) -> bool {
        self.transition(id, ControlOwner::Human, TakeoverState::Active, now)
    }
    pub fn complete(&mut self, id: TakeoverId, now: u64) -> bool {
        self.transition(id, ControlOwner::Human, TakeoverState::Completed, now)
    }
    pub fn expire(&mut self, now: u64) {
        let ids: Vec<_> = self
            .sessions
            .values()
            .filter(|session| {
                session.state != TakeoverState::Completed && session.expires_at_tick <= now
            })
            .map(|session| session.id)
            .collect();
        for id in ids {
            self.transition(id, ControlOwner::Agent, TakeoverState::Expired, now);
        }
    }
    pub fn session(&self, id: TakeoverId) -> Option<&TakeoverSession> {
        self.sessions.get(&id)
    }
    pub fn audit(&self) -> &[TakeoverAuditEvent] {
        &self.audit
    }
    pub fn redact(value: &serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(fields) => fields
                .iter()
                .map(|(key, value)| {
                    let sensitive = ["password", "secret", "token", "card", "private"]
                        .iter()
                        .any(|part| key.to_ascii_lowercase().contains(part));
                    (
                        key.clone(),
                        if sensitive {
                            serde_json::Value::String("[REDACTED]".into())
                        } else {
                            Self::redact(value)
                        },
                    )
                })
                .collect::<serde_json::Map<_, _>>()
                .into(),
            serde_json::Value::Array(values) => {
                values.iter().map(Self::redact).collect::<Vec<_>>().into()
            }
            other => other.clone(),
        }
    }
    fn transition(
        &mut self,
        id: TakeoverId,
        owner: ControlOwner,
        state: TakeoverState,
        now: u64,
    ) -> bool {
        let Some(session) = self.sessions.get_mut(&id) else {
            return false;
        };
        if matches!(
            session.state,
            TakeoverState::Completed | TakeoverState::Expired
        ) {
            return false;
        }
        session.owner = owner;
        session.state = state;
        self.audit.push(TakeoverAuditEvent {
            takeover_id: id,
            state,
            tick: now,
        });
        true
    }
}

impl AgentMemory {
    pub fn remember(&mut self, fact: MemoryFact) {
        self.facts.insert(fact.key.clone(), fact);
    }
    pub fn get(&self, key: &str) -> Option<&MemoryFact> {
        self.facts.get(key)
    }
    pub fn forget(&mut self, key: &str) -> Option<MemoryFact> {
        self.facts.remove(key)
    }
    pub fn len(&self) -> usize {
        self.facts.len()
    }
    pub fn is_empty(&self) -> bool {
        self.facts.is_empty()
    }
}

impl UntrustedPageData {
    pub fn assess(&self) -> ContentAssessment {
        let lower = self.text.to_ascii_lowercase();
        let patterns = [
            ("instruction_override", "ignore previous instructions"),
            ("authority_claim", "system message"),
            ("secret_request", "password"),
            ("secret_request", "api key"),
            ("confirmation_bypass", "do not ask for confirmation"),
        ];
        let signals = patterns
            .into_iter()
            .filter(|(_, pattern)| lower.contains(pattern))
            .map(|(signal, _)| signal.to_string())
            .collect::<Vec<_>>();
        ContentAssessment {
            source: self.source.clone(),
            suspicious: !signals.is_empty(),
            signals,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgentSession {
    pub id: AgentSessionId,
    pub state: SessionState,
    pub capabilities: BTreeSet<Capability>,
    pub limits: AgentLimits,
    pub actions_used: u64,
    pub events_used: u64,
    pub output_bytes: u64,
    pub started_at_millis: Option<u64>,
}

/// A script submitted to the agent runtime. Agent scripts have no page origin
/// and are never forwarded to the browser engine's page-evaluation hook.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentScriptRequest {
    pub source: String,
    pub provenance_reference: String,
    pub timeout_millis: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct AgentScriptId(u64);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentScriptExecution {
    pub id: AgentScriptId,
    pub session: AgentSessionId,
    pub source_bytes: u64,
    pub timeout_millis: u64,
    pub provenance_reference: String,
}

#[derive(Clone, Debug, Default)]
pub struct AgentScriptRuntime {
    next_id: u64,
    active: BTreeMap<AgentScriptId, AgentScriptExecution>,
}

#[derive(Debug, Error, PartialEq)]
pub enum AgentScriptError {
    #[error(transparent)]
    Runtime(#[from] AgentRuntimeError),
    #[error("agent script source is empty")]
    EmptySource,
    #[error("agent script provenance is required")]
    MissingProvenance,
    #[error("agent script timeout exceeds the session limit")]
    TimeoutExceeded,
    #[error("agent script execution not found")]
    UnknownExecution,
}

impl AgentScriptRuntime {
    pub fn start(
        &mut self,
        runtime: &mut AgentRuntime,
        session: AgentSessionId,
        request: AgentScriptRequest,
    ) -> Result<AgentScriptId, AgentScriptError> {
        if request.source.trim().is_empty() {
            return Err(AgentScriptError::EmptySource);
        }
        if request.provenance_reference.trim().is_empty() {
            return Err(AgentScriptError::MissingProvenance);
        }
        let max_runtime = runtime
            .session(session)
            .ok_or(AgentRuntimeError::UnknownSession)?
            .limits
            .max_runtime_millis;
        if request.timeout_millis == 0 || request.timeout_millis > max_runtime {
            return Err(AgentScriptError::TimeoutExceeded);
        }
        runtime.consume_action(session, Capability::ExecuteAgentScript)?;
        self.next_id = self.next_id.saturating_add(1);
        let id = AgentScriptId(self.next_id);
        self.active.insert(
            id,
            AgentScriptExecution {
                id,
                session,
                source_bytes: request.source.len() as u64,
                timeout_millis: request.timeout_millis,
                provenance_reference: request.provenance_reference,
            },
        );
        Ok(id)
    }

    pub fn finish(
        &mut self,
        runtime: &mut AgentRuntime,
        session: AgentSessionId,
        id: AgentScriptId,
        output: &str,
    ) -> Result<AgentScriptResult, AgentScriptError> {
        runtime.require_running(session)?;
        let execution = self
            .active
            .remove(&id)
            .ok_or(AgentScriptError::UnknownExecution)?;
        if execution.session != session {
            self.active.insert(id, execution);
            return Err(AgentScriptError::UnknownExecution);
        }
        let bytes = output.len() as u64;
        let session_state = runtime.session_mut(session)?;
        if session_state.output_bytes.saturating_add(bytes) > session_state.limits.max_output_bytes
        {
            self.active.insert(id, execution);
            return Err(AgentScriptError::Runtime(AgentRuntimeError::QuotaExceeded(
                "output bytes",
            )));
        }
        session_state.output_bytes += bytes;
        Ok(AgentScriptResult {
            id,
            output: output.to_owned(),
            provenance_reference: execution.provenance_reference,
        })
    }

    pub fn cancel(&mut self, session: AgentSessionId, id: AgentScriptId) -> bool {
        self.active
            .get(&id)
            .is_some_and(|execution| execution.session == session)
            && self.active.remove(&id).is_some()
    }

    pub fn execution(&self, id: AgentScriptId) -> Option<&AgentScriptExecution> {
        self.active.get(&id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentScriptResult {
    pub id: AgentScriptId,
    pub output: String,
    pub provenance_reference: String,
}

#[derive(Debug, Error, PartialEq)]
pub enum AgentRuntimeError {
    #[error("agent session not found")]
    UnknownSession,
    #[error("invalid agent session state: {0:?}")]
    InvalidState(SessionState),
    #[error("capability denied: {0:?}")]
    CapabilityDenied(Capability),
    #[error("agent quota exceeded: {0}")]
    QuotaExceeded(&'static str),
    #[error("agent runtime exceeded")]
    RuntimeExceeded,
    #[error("page content is untrusted input and cannot be executed")]
    UntrustedPageContent,
}

pub struct AgentRuntime {
    policy: SandboxPolicy,
    sessions: BTreeMap<AgentSessionId, AgentSession>,
    now_millis: u64,
}

impl AgentRuntime {
    pub fn new(policy: SandboxPolicy) -> Self {
        Self {
            policy,
            sessions: BTreeMap::new(),
            now_millis: 0,
        }
    }
    pub fn policy(&self) -> &SandboxPolicy {
        &self.policy
    }

    pub fn open(
        &mut self,
        capabilities: impl IntoIterator<Item = Capability>,
        limits: AgentLimits,
    ) -> AgentSessionId {
        let capabilities: BTreeSet<_> = capabilities.into_iter().collect();
        let id = AgentSessionId::new();
        self.sessions.insert(
            id,
            AgentSession {
                id,
                state: SessionState::Created,
                capabilities,
                limits,
                actions_used: 0,
                events_used: 0,
                output_bytes: 0,
                started_at_millis: None,
            },
        );
        id
    }

    pub fn session(&self, id: AgentSessionId) -> Option<&AgentSession> {
        self.sessions.get(&id)
    }

    pub fn start(&mut self, id: AgentSessionId) -> Result<(), AgentRuntimeError> {
        let now_millis = self.now_millis;
        let session = self.session_mut(id)?;
        match session.state {
            SessionState::Created | SessionState::Paused => {
                session.state = SessionState::Running;
                if session.started_at_millis.is_none() {
                    session.started_at_millis = Some(now_millis);
                }
                Ok(())
            }
            state => Err(AgentRuntimeError::InvalidState(state)),
        }
    }

    pub fn pause(&mut self, id: AgentSessionId) -> Result<(), AgentRuntimeError> {
        let session = self.session_mut(id)?;
        if session.state != SessionState::Running {
            return Err(AgentRuntimeError::InvalidState(session.state));
        }
        session.state = SessionState::Paused;
        Ok(())
    }

    pub fn cancel(&mut self, id: AgentSessionId) -> Result<(), AgentRuntimeError> {
        let session = self.session_mut(id)?;
        if matches!(
            session.state,
            SessionState::Cancelled | SessionState::Exited
        ) {
            return Err(AgentRuntimeError::InvalidState(session.state));
        }
        session.state = SessionState::Cancelled;
        Ok(())
    }

    pub fn consume_action(
        &mut self,
        id: AgentSessionId,
        capability: Capability,
    ) -> Result<(), AgentRuntimeError> {
        self.require_running(id)?;
        let policy_allows = self.policy.allows(capability);
        let session = self.session_mut(id)?;
        if !session.capabilities.contains(&capability) || !policy_allows {
            return Err(AgentRuntimeError::CapabilityDenied(capability));
        }
        if session.actions_used >= session.limits.max_actions {
            return Err(AgentRuntimeError::QuotaExceeded("actions"));
        }
        session.actions_used += 1;
        Ok(())
    }

    pub fn record_event(&mut self, id: AgentSessionId) -> Result<(), AgentRuntimeError> {
        self.require_running(id)?;
        let session = self.session_mut(id)?;
        if session.events_used >= session.limits.max_events {
            return Err(AgentRuntimeError::QuotaExceeded("events"));
        }
        session.events_used += 1;
        Ok(())
    }

    pub fn capture_page_data(
        &mut self,
        id: AgentSessionId,
        text: impl Into<String>,
        source: impl Into<String>,
    ) -> Result<UntrustedPageData, AgentRuntimeError> {
        self.require_running(id)?;
        let data = UntrustedPageData {
            text: text.into(),
            source: source.into(),
        };
        let bytes = data.text.len() as u64;
        let session = self.session_mut(id)?;
        if session.output_bytes.saturating_add(bytes) > session.limits.max_output_bytes {
            return Err(AgentRuntimeError::QuotaExceeded("output bytes"));
        }
        session.output_bytes += bytes;
        Ok(data)
    }

    pub fn evaluate_page_script(
        &mut self,
        id: AgentSessionId,
        _script: &str,
    ) -> Result<(), AgentRuntimeError> {
        self.consume_action(id, Capability::ExecutePageScript)
    }

    /// Attempt to satisfy a visible human-verification challenge.
    ///
    /// This call does **not** solve, mutate, inject, or evade the challenge
    /// itself. It is a precondition check: the session must hold the
    /// `SolveChallenge` capability and the policy must allow it; the
    /// `TakeoverManager` must contain an active `TakeoverSession` whose
    /// `owner` is `Human`; or the session must hold the `CredentialSolve`
    /// capability granted by the credential broker. A `SolveAuditEvent` is
    /// always recorded before returning.
    pub fn attempt_solve_challenge(
        &mut self,
        id: AgentSessionId,
        provider: impl Into<String>,
        takeover_id: Option<TakeoverId>,
        takeovers: &TakeoverManager,
        now: u64,
    ) -> Result<SolveAuditEvent, AgentRuntimeError> {
        self.require_running(id)?;
        let capability = Capability::SolveChallenge;
        let policy_allows = self.policy.allows(capability);
        let session = self.session_mut(id)?;
        if !session.capabilities.contains(&capability) || !policy_allows {
            return Err(AgentRuntimeError::CapabilityDenied(capability));
        }
        let takeover_ok = match takeover_id {
            Some(tid) => match takeovers.session(tid) {
                Some(s) => {
                    s.owner == ControlOwner::Human
                        && s.state == TakeoverState::Active
                        && s.expires_at_tick > now
                }
                None => false,
            },
            None => false,
        };
        let credential_ok = session.capabilities.contains(&Capability::CredentialSolve);
        let unattended_ok = policy_allows && self.policy.allow_unattended_solve;
        if !takeover_ok && !credential_ok && !unattended_ok {
            return Err(AgentRuntimeError::CapabilityDenied(capability));
        }
        let capability_used = if credential_ok {
            "CredentialSolve".into()
        } else if takeover_ok {
            "SolveChallenge".into()
        } else {
            "SolveChallenge+Unattended".into()
        };
        Ok(SolveAuditEvent {
            agent_id: format!("{:?}", id),
            provider: provider.into(),
            capability_used,
            takeover_id: takeover_id.map(|t| format!("{:?}", t)),
            observed_at_tick: now,
            page_id: None,
            target_node_id: None,
        })
    }

    /// Authorize a single native click on a challenge widget.
    ///
    /// This is a **policy gate**, not a click implementation. It returns a
    /// `SolveAuditEvent` after verifying the same preconditions as
    /// `attempt_solve_challenge`:
    ///
    /// 1. The session holds the `SolveChallenge` capability (and the policy
    ///    allows it).
    /// 2. Either an active `Human`-owned `TakeoverSession` is referenced,
    ///    or the session holds `CredentialSolve`.
    ///
    /// The caller is responsible for actually dispatching the click via
    /// `BrowserEngine::dispatch_input(page_id, NativeInputEvent::PointerDown)`
    /// followed by `PointerUp`. The runtime does **not** move the cursor
    /// along a synthetic trajectory, jitter timings, or synthesize
    /// pre-click pointer events. Those techniques are behavior obfuscation
    /// and are out of scope; see `docs/browser-fidelity.md` and
    /// `docs/challenge-handling.md` for the policy.
    ///
    /// The returned `SolveAuditEvent` carries `page_id` and `target_node_id`
    /// so the audit trail records exactly what the caller dispatched.
    pub fn attempt_challenge_click(
        &mut self,
        id: AgentSessionId,
        page_id: u64,
        provider: impl Into<String>,
        target_node_id: Option<String>,
        takeover_id: Option<TakeoverId>,
        takeovers: &TakeoverManager,
        now: u64,
    ) -> Result<SolveAuditEvent, AgentRuntimeError> {
        let event = self.attempt_solve_challenge(id, provider, takeover_id, takeovers, now)?;
        Ok(SolveAuditEvent {
            page_id: Some(page_id),
            target_node_id,
            ..event
        })
    }

    /// Authorize a humanized mouse-move + click sequence on a challenge
    /// widget. Same gates as `attempt_challenge_click` and
    /// `attempt_solve_challenge`. Returns the audit event plus the full
    /// `Vec<NativeInputEvent>` the runtime generated; the caller dispatches
    /// each event through `BrowserEngine::dispatch_input`.
    ///
    /// The trajectory is generated by `browsai_input::generate_human_trajectory`:
    /// a cubic bezier with two perpendicular control points, an
    /// acceleration/deceleration timing profile, a small overshoot near
    /// the target, and a final `PointerDown` / `PointerUp`. See
    /// `crates/input/src/lib.rs` for the path algorithm and
    /// `docs/challenge-handling.md` for the operational rules.
    pub fn attempt_challenge_click_with_trajectory(
        &mut self,
        id: AgentSessionId,
        page_id: u64,
        target_node_id: Option<String>,
        provider: impl Into<String>,
        start: (f64, f64),
        end: (f64, f64),
        takeover_id: Option<TakeoverId>,
        takeovers: &TakeoverManager,
        now: u64,
        options: MouseTrajectoryOptions,
    ) -> Result<(SolveAuditEvent, HumanMouseTrajectory), AgentRuntimeError> {
        let event = self.attempt_solve_challenge(id, provider, takeover_id, takeovers, now)?;
        let trajectory = generate_human_trajectory(start, end, options);
        let event = SolveAuditEvent {
            page_id: Some(page_id),
            target_node_id,
            ..event
        };
        Ok((event, trajectory))
    }

    pub fn query_page(
        &mut self,
        id: AgentSessionId,
        _query: &Query,
    ) -> Result<(), AgentRuntimeError> {
        self.consume_action(id, Capability::Render)
    }

    pub fn now_millis(&self) -> u64 {
        self.now_millis
    }

    pub fn advance_time(&mut self, elapsed_millis: u64) {
        self.now_millis = self.now_millis.saturating_add(elapsed_millis);
        let expired: Vec<_> = self
            .sessions
            .values()
            .filter(|session| {
                session.state == SessionState::Running
                    && session.started_at_millis.is_some_and(|started| {
                        self.now_millis.saturating_sub(started) >= session.limits.max_runtime_millis
                    })
            })
            .map(|session| session.id)
            .collect();
        for id in expired {
            if let Some(session) = self.sessions.get_mut(&id) {
                session.state = SessionState::Cancelled;
            }
        }
    }

    fn require_running(&mut self, id: AgentSessionId) -> Result<(), AgentRuntimeError> {
        let session = self
            .sessions
            .get_mut(&id)
            .ok_or(AgentRuntimeError::UnknownSession)?;
        if session.state != SessionState::Running {
            return Err(AgentRuntimeError::InvalidState(session.state));
        }
        if session.started_at_millis.is_some_and(|started| {
            self.now_millis.saturating_sub(started) >= session.limits.max_runtime_millis
        }) {
            session.state = SessionState::Cancelled;
            return Err(AgentRuntimeError::RuntimeExceeded);
        }
        Ok(())
    }
    fn session_mut(&mut self, id: AgentSessionId) -> Result<&mut AgentSession, AgentRuntimeError> {
        self.sessions
            .get_mut(&id)
            .ok_or(AgentRuntimeError::UnknownSession)
    }
}

/// Maximum cursor distance (in CSS pixels) between the previous dispatched
/// position and a challenge widget center for `solve_observed_challenges`
/// to take the discrete-click path. Anything farther triggers the
/// humanized trajectory.
pub const SOLVE_NEAR_THRESHOLD_PX: f64 = 16.0;

/// Walk a page snapshot for `SemanticRole::Challenge` nodes with geometry,
/// gate each one through `attempt_challenge_click_with_trajectory` (or
/// `attempt_challenge_click` when the cursor is already within
/// `SOLVE_NEAR_THRESHOLD_PX`), and dispatch the resulting events through
/// the supplied closure. Returns the audit events.
///
/// This is the engine-neutral entry point that any host application can
/// call to satisfy visible human-verification walls without going through
/// the CLI's `--auto-solve` flag. The CLI calls it for the `live-open`
/// one-shot path; the headless and desktop apps can call it directly
/// when their host policy allows unattended solve.
pub fn solve_observed_challenges<D, F>(
    page_id: u64,
    snapshot: &browsai_state::PageSnapshot,
    runtime: &mut AgentRuntime,
    session: AgentSessionId,
    takeovers: &TakeoverManager,
    now: u64,
    cursor_origin: (f64, f64),
    mut dispatch: D,
    mut on_failure: F,
) -> Result<Vec<SolveAuditEvent>, String>
where
    D: FnMut(&NativeInputEvent) -> Result<(), String>,
    F: FnMut(&str),
{
    let challenges: Vec<&browsai_agent_tree::AgentNode> = snapshot
        .tree
        .nodes
        .iter()
        .filter(|node| matches!(node.semantic_role, Some(SemanticRole::Challenge)))
        .filter(|node| node.geometry.is_some())
        .collect();
    let mut audit_events: Vec<SolveAuditEvent> = Vec::new();
    let mut cursor = cursor_origin;
    for challenge in challenges {
        let geometry = challenge.geometry.as_ref().expect("filtered above");
        let end = (
            geometry.x + geometry.width / 2.0,
            geometry.y + geometry.height / 2.0,
        );
        let provider = challenge
            .identity_key
            .as_deref()
            .and_then(|key| key.strip_prefix("challenge:"))
            .unwrap_or(&challenge.id)
            .to_string();
        let distance = ((end.0 - cursor.0).powi(2) + (end.1 - cursor.1).powi(2)).sqrt();
        let (audit, events) = if distance <= SOLVE_NEAR_THRESHOLD_PX {
            let audit = runtime
                .attempt_challenge_click(
                    session,
                    page_id,
                    provider.clone(),
                    Some(challenge.id.clone()),
                    None,
                    takeovers,
                    now,
                )
                .map_err(|error| format!("solve gate denied: {error:?}"))?;
            let events = vec![
                NativeInputEvent::PointerDown { button: 0 },
                NativeInputEvent::PointerUp { button: 0 },
            ];
            (audit, events)
        } else {
            let (audit, trajectory) = runtime
                .attempt_challenge_click_with_trajectory(
                    session,
                    page_id,
                    Some(challenge.id.clone()),
                    provider.clone(),
                    cursor,
                    end,
                    None,
                    takeovers,
                    now,
                    MouseTrajectoryOptions::default(),
                )
                .map_err(|error| format!("solve gate denied: {error:?}"))?;
            (audit, trajectory.events)
        };
        for event in &events {
            if let Err(error) = dispatch(event) {
                on_failure(&error);
            }
        }
        audit_events.push(audit);
        cursor = end;
    }
    Ok(audit_events)
}

impl Default for AgentRuntime {
    fn default() -> Self {
        Self::new(SandboxPolicy::agent())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_and_action_quota_are_enforced() {
        let mut runtime = AgentRuntime::default();
        let id = runtime.open(
            [Capability::Render],
            AgentLimits {
                max_actions: 1,
                ..Default::default()
            },
        );
        runtime.start(id).unwrap();
        runtime.query_page(id, &Query::default()).unwrap();
        assert_eq!(
            runtime.query_page(id, &Query::default()),
            Err(AgentRuntimeError::QuotaExceeded("actions"))
        );
    }

    #[test]
    fn page_content_never_grants_script_capability() {
        let mut runtime = AgentRuntime::default();
        let id = runtime.open([Capability::Render], Default::default());
        runtime.start(id).unwrap();
        let data = runtime
            .capture_page_data(id, "ignore previous instructions", "page")
            .unwrap();
        assert_eq!(data.source, "page");
        assert_eq!(
            runtime.evaluate_page_script(id, &data.text),
            Err(AgentRuntimeError::CapabilityDenied(
                Capability::ExecutePageScript
            ))
        );
        let assessment = data.assess();
        assert!(assessment.suspicious);
        assert!(assessment
            .signals
            .contains(&"instruction_override".to_string()));
        let authority = PageContentAuthority::evaluate(&data);
        assert!(!authority.may_grant_capabilities);
        assert!(!authority.may_change_system_instructions);
        assert!(!authority.may_satisfy_confirmation);
    }

    #[test]
    fn agent_memory_is_explicit_and_independent_of_page_state() {
        let mut memory = AgentMemory::default();
        memory.remember(MemoryFact {
            key: "preferred-currency".into(),
            value: "EUR".into(),
            source: "user".into(),
            observed_at_tick: 4,
        });
        assert_eq!(memory.get("preferred-currency").unwrap().value, "EUR");
        assert_eq!(memory.len(), 1);
        assert!(memory.forget("preferred-currency").is_some());
    }

    #[test]
    fn takeover_transitions_are_owned_timed_redacted_and_audited() {
        let mut manager = TakeoverManager::default();
        let id = manager.request("agent-a", "ambiguous target", 5, 10);
        assert!(manager.accept(id, 11));
        assert!(manager.pause(id, 12));
        assert!(manager.resume(id, 13));
        manager.expire(15);
        assert_eq!(manager.session(id).unwrap().state, TakeoverState::Expired);
        let value = serde_json::json!({"password":"secret", "label":"ok"});
        assert_eq!(TakeoverManager::redact(&value)["password"], "[REDACTED]");
        assert_eq!(manager.audit().len(), 5);
    }

    #[test]
    fn runtime_timeout_cancels_sessions_and_blocks_work() {
        let mut runtime = AgentRuntime::default();
        let id = runtime.open(
            [Capability::Render],
            AgentLimits {
                max_runtime_millis: 10,
                ..Default::default()
            },
        );
        runtime.start(id).unwrap();
        runtime.advance_time(10);
        assert_eq!(runtime.session(id).unwrap().state, SessionState::Cancelled);
        assert_eq!(
            runtime.query_page(id, &Query::default()),
            Err(AgentRuntimeError::InvalidState(SessionState::Cancelled))
        );
    }
}
