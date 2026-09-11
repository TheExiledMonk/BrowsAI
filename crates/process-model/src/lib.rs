//! Explicit process topology, lifecycle states, and safe restart policy.

use browsai_ipc::{IpcRouter, ProcessKind};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::process::{Child, Command, Stdio};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ProcessState {
    Starting,
    Running,
    Stopping,
    Crashed,
    Stopped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RestartPolicy {
    Never,
    OnFailure,
    Always,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProcessSpec {
    pub kind: ProcessKind,
    pub state: ProcessState,
    pub restart: RestartPolicy,
    pub isolated: bool,
}

#[derive(Clone, Debug)]
pub struct ProcessTopology {
    protocol_version: u16,
    processes: BTreeMap<ProcessKind, ProcessSpec>,
    routes: BTreeSet<(ProcessKind, ProcessKind)>,
}

impl ProcessTopology {
    pub fn production(protocol_version: u16) -> Self {
        let kinds = [
            (ProcessKind::Host, RestartPolicy::Always, false),
            (ProcessKind::BrowserBroker, RestartPolicy::OnFailure, true),
            (
                ProcessKind::CredentialBroker,
                RestartPolicy::OnFailure,
                true,
            ),
            (ProcessKind::ProfileManager, RestartPolicy::OnFailure, true),
            (
                ProcessKind::PermissionManager,
                RestartPolicy::OnFailure,
                true,
            ),
            (ProcessKind::Network, RestartPolicy::OnFailure, true),
            (ProcessKind::FileBroker, RestartPolicy::OnFailure, true),
            (ProcessKind::DownloadBroker, RestartPolicy::OnFailure, true),
            (ProcessKind::Audit, RestartPolicy::OnFailure, true),
            (
                ProcessKind::WorkspaceManager,
                RestartPolicy::OnFailure,
                true,
            ),
            (ProcessKind::Renderer, RestartPolicy::OnFailure, true),
            (ProcessKind::Worker, RestartPolicy::OnFailure, true),
            (ProcessKind::AgentRuntime, RestartPolicy::OnFailure, true),
            (ProcessKind::Compositor, RestartPolicy::OnFailure, true),
        ];
        let processes = kinds
            .into_iter()
            .map(|(kind, restart, isolated)| {
                (
                    kind,
                    ProcessSpec {
                        kind,
                        state: ProcessState::Starting,
                        restart,
                        isolated,
                    },
                )
            })
            .collect();
        let routes = [
            (ProcessKind::Host, ProcessKind::BrowserBroker),
            (ProcessKind::Host, ProcessKind::CredentialBroker),
            (ProcessKind::Host, ProcessKind::ProfileManager),
            (ProcessKind::Host, ProcessKind::PermissionManager),
            (ProcessKind::Host, ProcessKind::Network),
            (ProcessKind::Host, ProcessKind::FileBroker),
            (ProcessKind::Host, ProcessKind::DownloadBroker),
            (ProcessKind::Host, ProcessKind::Audit),
            (ProcessKind::Host, ProcessKind::WorkspaceManager),
            (ProcessKind::Host, ProcessKind::AgentRuntime),
            (ProcessKind::Host, ProcessKind::Compositor),
            (ProcessKind::BrowserBroker, ProcessKind::Renderer),
            (ProcessKind::BrowserBroker, ProcessKind::Network),
            (ProcessKind::BrowserBroker, ProcessKind::ProfileManager),
            (ProcessKind::BrowserBroker, ProcessKind::PermissionManager),
            (ProcessKind::BrowserBroker, ProcessKind::Audit),
            (ProcessKind::BrowserBroker, ProcessKind::WorkspaceManager),
            (ProcessKind::WorkspaceManager, ProcessKind::Audit),
            (ProcessKind::WorkspaceManager, ProcessKind::BrowserBroker),
            (ProcessKind::AgentRuntime, ProcessKind::WorkspaceManager),
            (ProcessKind::Renderer, ProcessKind::Worker),
            (ProcessKind::Renderer, ProcessKind::BrowserBroker),
            (ProcessKind::AgentRuntime, ProcessKind::BrowserBroker),
            (ProcessKind::AgentRuntime, ProcessKind::PermissionManager),
            (ProcessKind::AgentRuntime, ProcessKind::FileBroker),
            (ProcessKind::AgentRuntime, ProcessKind::DownloadBroker),
            (ProcessKind::Compositor, ProcessKind::Renderer),
        ]
        .into_iter()
        .collect();
        Self {
            protocol_version,
            processes,
            routes,
        }
    }

    pub fn protocol_version(&self) -> u16 {
        self.protocol_version
    }
    pub fn process(&self, kind: ProcessKind) -> Option<&ProcessSpec> {
        self.processes.get(&kind)
    }
    pub fn route_allowed(&self, source: ProcessKind, destination: ProcessKind) -> bool {
        self.routes.contains(&(source, destination))
    }

    pub fn router(&self) -> IpcRouter {
        let mut router = IpcRouter::new(self.protocol_version);
        for &(source, destination) in &self.routes {
            router.allow(source, destination);
        }
        router
    }

    pub fn mark_running(&mut self, kind: ProcessKind) -> bool {
        self.processes
            .get_mut(&kind)
            .map(|p| {
                p.state = ProcessState::Running;
                true
            })
            .unwrap_or(false)
    }

    pub fn mark_crashed(&mut self, kind: ProcessKind) -> RestartPolicy {
        self.processes
            .get_mut(&kind)
            .map(|p| {
                p.state = ProcessState::Crashed;
                p.restart
            })
            .unwrap_or(RestartPolicy::Never)
    }

    pub fn restart_after_crash(&mut self, kind: ProcessKind) -> bool {
        match self.processes.get_mut(&kind) {
            Some(process)
                if process.state == ProcessState::Crashed
                    && process.restart != RestartPolicy::Never =>
            {
                process.state = ProcessState::Starting;
                true
            }
            _ => false,
        }
    }
}

impl Default for ProcessTopology {
    fn default() -> Self {
        Self::production(1)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProcessLaunch {
    pub kind: ProcessKind,
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug, Error)]
pub enum ProcessSupervisorError {
    #[error("process kind is not defined: {0:?}")]
    UnknownProcess(ProcessKind),
    #[error("process is already running: {0:?}")]
    AlreadyRunning(ProcessKind),
    #[error("host process cannot be launched by its own supervisor")]
    HostLaunchDenied,
    #[error("process launch failed: {0}")]
    Spawn(#[from] io::Error),
    #[error("IPC route rejected: {0:?}")]
    Ipc(browsai_ipc::IpcError),
    #[error("no launch specification is available for process: {0:?}")]
    NoLaunchSpec(ProcessKind),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProcessHandle {
    pub kind: ProcessKind,
    pub pid: u32,
}

/// Owns child-process lifetimes and routes IPC through the topology allowlist.
///
/// Launches start with a cleared environment and detached standard streams. Secrets therefore
/// cannot accidentally cross the process boundary through inherited environment variables or
/// console output; privileged data must use an explicitly authorized broker route.
pub struct ProcessSupervisor {
    topology: ProcessTopology,
    children: BTreeMap<ProcessKind, Child>,
    launches: BTreeMap<ProcessKind, ProcessLaunch>,
    channels: BTreeMap<(ProcessKind, ProcessKind), browsai_ipc::IpcChannel>,
}

impl ProcessSupervisor {
    pub fn new(topology: ProcessTopology) -> Self {
        Self {
            topology,
            children: BTreeMap::new(),
            launches: BTreeMap::new(),
            channels: BTreeMap::new(),
        }
    }

    pub fn topology(&self) -> &ProcessTopology {
        &self.topology
    }

    pub fn launch(
        &mut self,
        request: ProcessLaunch,
    ) -> Result<ProcessHandle, ProcessSupervisorError> {
        if request.kind == ProcessKind::Host {
            return Err(ProcessSupervisorError::HostLaunchDenied);
        }
        let process = self
            .topology
            .process(request.kind)
            .ok_or(ProcessSupervisorError::UnknownProcess(request.kind))?;
        if self.children.contains_key(&request.kind) || process.state == ProcessState::Running {
            return Err(ProcessSupervisorError::AlreadyRunning(request.kind));
        }

        let child = Command::new(&request.program)
            .args(&request.args)
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        let handle = ProcessHandle {
            kind: request.kind,
            pid: child.id(),
        };
        let kind = request.kind;
        self.launches.insert(kind, request);
        self.children.insert(handle.kind, child);
        self.topology.mark_running(kind);
        Ok(handle)
    }

    pub fn poll(
        &mut self,
        kind: ProcessKind,
    ) -> Result<Option<std::process::ExitStatus>, ProcessSupervisorError> {
        let child = self
            .children
            .get_mut(&kind)
            .ok_or(ProcessSupervisorError::UnknownProcess(kind))?;
        let status = child.try_wait()?;
        if status.is_some() {
            self.children.remove(&kind);
            self.topology.mark_crashed(kind);
        }
        Ok(status)
    }

    pub fn stop(&mut self, kind: ProcessKind) -> Result<(), ProcessSupervisorError> {
        let mut child = self
            .children
            .remove(&kind)
            .ok_or(ProcessSupervisorError::UnknownProcess(kind))?;
        child.kill()?;
        child.wait()?;
        self.launches.remove(&kind);
        if let Some(process) = self.topology.processes.get_mut(&kind) {
            process.state = ProcessState::Stopped;
        }
        Ok(())
    }

    pub fn restart_after_crash(
        &mut self,
        kind: ProcessKind,
    ) -> Result<ProcessHandle, ProcessSupervisorError> {
        if self.children.contains_key(&kind) {
            return Err(ProcessSupervisorError::AlreadyRunning(kind));
        }
        let request = self
            .launches
            .get(&kind)
            .cloned()
            .ok_or(ProcessSupervisorError::NoLaunchSpec(kind))?;
        if !self.topology.restart_after_crash(kind) {
            return Err(ProcessSupervisorError::UnknownProcess(kind));
        }
        self.launch(request)
    }

    pub fn route(
        &mut self,
        message: browsai_ipc::IpcEnvelope,
    ) -> Result<(), ProcessSupervisorError> {
        self.topology
            .router()
            .validate(&message)
            .map_err(ProcessSupervisorError::Ipc)?;
        self.channels
            .entry((message.source, message.destination))
            .or_insert_with(|| browsai_ipc::IpcChannel::bounded(128))
            .send(message)
            .map_err(ProcessSupervisorError::Ipc)
    }

    pub fn receive(
        &mut self,
        source: ProcessKind,
        destination: ProcessKind,
    ) -> Option<browsai_ipc::IpcEnvelope> {
        self.channels
            .get_mut(&(source, destination))
            .and_then(browsai_ipc::IpcChannel::receive)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use browsai_ipc::{IpcEnvelope, IpcMessage};
    use uuid::Uuid;

    #[test]
    fn renderer_has_no_privileged_secret_routes() {
        let topology = ProcessTopology::default();
        for destination in [
            ProcessKind::CredentialBroker,
            ProcessKind::ProfileManager,
            ProcessKind::PermissionManager,
            ProcessKind::FileBroker,
            ProcessKind::DownloadBroker,
            ProcessKind::WorkspaceManager,
            ProcessKind::Audit,
        ] {
            assert!(
                !topology.route_allowed(ProcessKind::Renderer, destination),
                "renderer unexpectedly reached {destination:?}"
            );
        }
        assert!(!topology.route_allowed(ProcessKind::AgentRuntime, ProcessKind::CredentialBroker));
        assert!(topology.process(ProcessKind::Audit).is_some());
        assert!(topology.process(ProcessKind::WorkspaceManager).is_some());
        assert!(topology.route_allowed(ProcessKind::Host, ProcessKind::Audit));
        assert!(topology.route_allowed(ProcessKind::BrowserBroker, ProcessKind::WorkspaceManager));
        assert!(!topology.route_allowed(ProcessKind::Renderer, ProcessKind::Audit));
    }

    #[test]
    fn renderer_compromise_cannot_cross_brokered_sensitive_boundaries() {
        let topology = ProcessTopology::default();
        let router = topology.router();
        for destination in [
            ProcessKind::CredentialBroker,
            ProcessKind::PermissionManager,
            ProcessKind::ProfileManager,
            ProcessKind::FileBroker,
            ProcessKind::WorkspaceManager,
        ] {
            let message = IpcEnvelope {
                protocol_version: 1,
                sequence: 0,
                request_id: Uuid::new_v4(),
                source: ProcessKind::Renderer,
                destination,
                workspace: "workspace".into(),
                profile: "profile".into(),
                origin: Some("https://attacker.test".into()),
                capability: None,
                payload: IpcMessage::Request {
                    method: "read".into(),
                    body: serde_json::json!({"reference": "opaque-capability"}),
                },
            };
            assert_eq!(
                router.validate(&message),
                Err(browsai_ipc::IpcError::Unauthorized)
            );
        }
    }

    #[test]
    fn router_enforces_topology_and_version() {
        let topology = ProcessTopology::default();
        let router = topology.router();
        let message = IpcEnvelope {
            protocol_version: 1,
            sequence: 0,
            request_id: Uuid::new_v4(),
            source: ProcessKind::Renderer,
            destination: ProcessKind::CredentialBroker,
            workspace: "w".into(),
            profile: "p".into(),
            origin: Some("https://example.test".into()),
            capability: None,
            payload: IpcMessage::Request {
                method: "read-secret".into(),
                body: serde_json::Value::Null,
            },
        };
        assert_eq!(
            router.validate(&message),
            Err(browsai_ipc::IpcError::Unauthorized)
        );
    }

    #[test]
    fn failed_isolated_processes_restart_without_reusing_running_state() {
        let mut topology = ProcessTopology::default();
        assert!(topology.mark_running(ProcessKind::Renderer));
        assert_eq!(
            topology.mark_crashed(ProcessKind::Renderer),
            RestartPolicy::OnFailure
        );
        assert!(topology.restart_after_crash(ProcessKind::Renderer));
        assert_eq!(
            topology.process(ProcessKind::Renderer).unwrap().state,
            ProcessState::Starting
        );
    }

    #[test]
    fn supervisor_launches_isolated_children_and_routes_only_allowed_ipc() {
        let mut supervisor = ProcessSupervisor::new(ProcessTopology::default());
        let program = std::env::current_exe().unwrap();
        let handle = supervisor
            .launch(ProcessLaunch {
                kind: ProcessKind::Renderer,
                program: program.to_string_lossy().into_owned(),
                args: vec!["--list".into()],
            })
            .unwrap();
        assert_eq!(handle.kind, ProcessKind::Renderer);
        assert!(handle.pid > 0);

        let allowed = IpcEnvelope {
            protocol_version: 1,
            sequence: 0,
            request_id: Uuid::new_v4(),
            source: ProcessKind::BrowserBroker,
            destination: ProcessKind::Renderer,
            workspace: "w".into(),
            profile: "p".into(),
            origin: Some("https://example.test".into()),
            capability: None,
            payload: IpcMessage::Request {
                method: "snapshot".into(),
                body: serde_json::Value::Null,
            },
        };
        supervisor.route(allowed.clone()).unwrap();
        assert_eq!(
            supervisor.receive(ProcessKind::BrowserBroker, ProcessKind::Renderer),
            Some(allowed.clone())
        );
        let denied = IpcEnvelope {
            destination: ProcessKind::CredentialBroker,
            ..allowed
        };
        assert!(matches!(
            supervisor.route(denied),
            Err(ProcessSupervisorError::Ipc(
                browsai_ipc::IpcError::Unauthorized
            ))
        ));
        supervisor.stop(ProcessKind::Renderer).unwrap();
        assert_eq!(
            supervisor
                .topology()
                .process(ProcessKind::Renderer)
                .unwrap()
                .state,
            ProcessState::Stopped
        );
    }
}
