use browsai_ipc::ProcessKind;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum Capability {
    ReadNetwork,
    WriteNetwork,
    ReadFiles,
    WriteFiles,
    ReadClipboard,
    WriteClipboard,
    Render,
    ExecutePageScript,
    ExecuteAgentScript,
    UseCredential,
    SpawnProcess,
    AccessOtherOrigins,
    SolveChallenge,
    CredentialSolve,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct CapabilityScope {
    pub profile: String,
    pub origin: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CapabilityRequest {
    pub scope: CapabilityScope,
    pub capability: Capability,
    pub reason: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CapabilityDecision {
    Granted,
    Denied,
    Prompt,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CapabilityGrant {
    pub request: CapabilityRequest,
    pub expires_at_tick: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CapabilityError {
    PolicyDenied,
    InvalidScope,
}

#[derive(Clone, Debug, Default)]
pub struct CapabilityBroker {
    decisions: BTreeMap<(CapabilityScope, Capability), (CapabilityDecision, Option<u64>)>,
}

impl CapabilityBroker {
    pub fn request(
        &self,
        policy: &SandboxPolicy,
        request: &CapabilityRequest,
        now: u64,
    ) -> CapabilityDecision {
        if !policy.allows(request.capability)
            || (!policy.allowed_origins.is_empty() && !policy.allows_origin(&request.scope.origin))
        {
            return CapabilityDecision::Denied;
        }
        match self
            .decisions
            .get(&(request.scope.clone(), request.capability))
        {
            Some((decision, Some(expires))) if *expires <= now => CapabilityDecision::Prompt,
            Some((decision, _)) => *decision,
            None => CapabilityDecision::Prompt,
        }
    }
    pub fn grant(
        &mut self,
        policy: &SandboxPolicy,
        request: CapabilityRequest,
        lifetime_ticks: Option<u64>,
        now: u64,
    ) -> Result<CapabilityGrant, CapabilityError> {
        if request.scope.origin.is_empty() || request.scope.profile.is_empty() {
            return Err(CapabilityError::InvalidScope);
        }
        if !policy.allows(request.capability)
            || (!policy.allowed_origins.is_empty() && !policy.allows_origin(&request.scope.origin))
        {
            return Err(CapabilityError::PolicyDenied);
        }
        let expires = lifetime_ticks.map(|ticks| now.saturating_add(ticks.max(1)));
        self.decisions.insert(
            (request.scope.clone(), request.capability),
            (CapabilityDecision::Granted, expires),
        );
        Ok(CapabilityGrant {
            request,
            expires_at_tick: expires,
        })
    }
    pub fn deny(&mut self, request: &CapabilityRequest) {
        self.decisions.insert(
            (request.scope.clone(), request.capability),
            (CapabilityDecision::Denied, None),
        );
    }
    pub fn revoke(&mut self, scope: &CapabilityScope, capability: Capability) {
        self.decisions.remove(&(scope.clone(), capability));
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_memory_bytes: u64,
    pub max_cpu_millis: u64,
    pub max_children: u32,
    pub max_runtime_millis: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OsSandboxProfile {
    pub process_identity: ProcessKind,
    pub allowed_syscalls: BTreeSet<String>,
    pub filesystem_roots: BTreeSet<String>,
    pub network_egress: bool,
    pub limits: ResourceLimits,
    pub crash_containment: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SandboxPolicy {
    pub process: ProcessKind,
    pub capabilities: BTreeSet<Capability>,
    pub limits: ResourceLimits,
    pub allowed_origins: BTreeSet<String>,
    pub filesystem_roots: BTreeSet<String>,
    /// When true, the runtime may satisfy a visible human-verification wall
    /// without an active `Human`-owned `TakeoverSession`. The caller is
    /// still required to grant the `SolveChallenge` capability, the session
    /// quota still applies, and every attempt is still recorded as a
    /// `SolveAuditEvent`. This is intended for autonomous VPS deployments
    /// where a human takeover cannot be staged. Off by default.
    pub allow_unattended_solve: bool,
}

impl SandboxPolicy {
    pub fn renderer(origin: impl Into<String>) -> Self {
        Self {
            process: ProcessKind::Renderer,
            capabilities: [
                Capability::Render,
                Capability::ReadNetwork,
                Capability::WriteNetwork,
            ]
            .into_iter()
            .collect(),
            limits: ResourceLimits {
                max_memory_bytes: 512 * 1024 * 1024,
                max_cpu_millis: 10_000,
                max_children: 8,
                max_runtime_millis: 0,
            },
            allowed_origins: [origin.into()].into_iter().collect(),
            filesystem_roots: BTreeSet::new(),
            allow_unattended_solve: false,
        }
    }
    pub fn agent() -> Self {
        Self {
            process: ProcessKind::AgentRuntime,
            capabilities: [Capability::ExecuteAgentScript, Capability::Render]
                .into_iter()
                .collect(),
            limits: ResourceLimits {
                max_memory_bytes: 256 * 1024 * 1024,
                max_cpu_millis: 5_000,
                max_children: 0,
                max_runtime_millis: 60_000,
            },
            allowed_origins: BTreeSet::new(),
            filesystem_roots: BTreeSet::new(),
            allow_unattended_solve: false,
        }
    }
    pub fn autonomous_agent() -> Self {
        let mut policy = Self::agent();
        policy.capabilities.insert(Capability::SolveChallenge);
        policy.allow_unattended_solve = true;
        policy
    }
    pub fn allows(&self, capability: Capability) -> bool {
        self.capabilities.contains(&capability)
    }
    pub fn allows_origin(&self, origin: &str) -> bool {
        self.allowed_origins
            .iter()
            .any(|allowed| same_origin(allowed, origin))
    }

    pub fn os_profile(&self) -> OsSandboxProfile {
        let mut allowed_syscalls = BTreeSet::from([
            "read".into(),
            "write".into(),
            "futex".into(),
            "clock_gettime".into(),
            "exit".into(),
        ]);
        if self.capabilities.contains(&Capability::ReadFiles)
            || self.capabilities.contains(&Capability::WriteFiles)
        {
            allowed_syscalls.insert("openat".into());
            allowed_syscalls.insert("stat".into());
        }
        OsSandboxProfile {
            process_identity: self.process,
            allowed_syscalls,
            filesystem_roots: self.filesystem_roots.clone(),
            network_egress: self.capabilities.contains(&Capability::ReadNetwork)
                || self.capabilities.contains(&Capability::WriteNetwork),
            limits: self.limits.clone(),
            crash_containment: self.process != ProcessKind::Host,
        }
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.process == ProcessKind::AgentRuntime
            && self.capabilities.contains(&Capability::SpawnProcess)
        {
            return Err("agent runtime cannot spawn processes");
        }
        if self.process == ProcessKind::Renderer
            && (self.capabilities.contains(&Capability::ReadFiles)
                || self.capabilities.contains(&Capability::WriteFiles))
        {
            return Err("renderer cannot access files directly");
        }
        if self.limits.max_runtime_millis == 0 && self.process != ProcessKind::Renderer {
            return Err("non-renderer sandbox requires a runtime limit");
        }
        Ok(())
    }
}

fn same_origin(left: &str, right: &str) -> bool {
    let Ok(left) = url::Url::parse(left) else {
        return false;
    };
    let Ok(right) = url::Url::parse(right) else {
        return false;
    };
    left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
        && left.username().is_empty()
        && right.username().is_empty()
        && left.password().is_none()
        && right.password().is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capability_grants_are_policy_and_scope_bound() {
        let mut broker = CapabilityBroker::default();
        let policy = SandboxPolicy::renderer("https://example.test");
        let request = CapabilityRequest {
            scope: CapabilityScope {
                profile: "work".into(),
                origin: "https://example.test".into(),
            },
            capability: Capability::ReadNetwork,
            reason: "load page".into(),
        };
        assert_eq!(
            broker.request(&policy, &request, 0),
            CapabilityDecision::Prompt
        );
        broker.grant(&policy, request.clone(), Some(5), 0).unwrap();
        assert_eq!(
            broker.request(&policy, &request, 4),
            CapabilityDecision::Granted
        );
        assert_eq!(
            broker.request(&policy, &request, 5),
            CapabilityDecision::Prompt
        );
        broker.deny(&request);
        assert_eq!(
            broker.request(&policy, &request, 6),
            CapabilityDecision::Denied
        );
    }

    #[test]
    fn origin_policy_uses_origin_tuple_not_raw_url_path() {
        let policy = SandboxPolicy::renderer("https://example.test/app/");
        assert!(policy.allows_origin("https://example.test/other"));
        assert!(!policy.allows_origin("https://other.test/app/"));
        assert!(!policy.allows_origin("https://example.test:444/app/"));
        assert!(!policy.allows_origin("not-an-origin"));
    }

    #[test]
    fn os_profiles_make_identity_and_boundary_restrictions_explicit() {
        let renderer = SandboxPolicy::renderer("https://example.test");
        renderer.validate().unwrap();
        let profile = renderer.os_profile();
        assert_eq!(profile.process_identity, ProcessKind::Renderer);
        assert!(profile.network_egress);
        assert!(profile.crash_containment);
        assert!(!profile.allowed_syscalls.contains("openat"));

        let agent = SandboxPolicy::agent();
        agent.validate().unwrap();
        assert_eq!(
            agent.os_profile().process_identity,
            ProcessKind::AgentRuntime
        );
        assert!(!agent.os_profile().network_egress);
    }
}
