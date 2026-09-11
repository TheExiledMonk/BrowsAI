use browsai_agent_runtime::{
    AgentLimits, AgentRuntime, AgentScriptError, AgentScriptRequest, AgentScriptRuntime,
};
use browsai_sandbox::Capability;

fn request() -> AgentScriptRequest {
    AgentScriptRequest {
        source: "1 + 1".into(),
        provenance_reference: "agent-task:1".into(),
        timeout_millis: 100,
    }
}

#[test]
fn agent_script_lifecycle_is_separate_from_page_evaluation() {
    let mut runtime = AgentRuntime::new(browsai_sandbox::SandboxPolicy::agent());
    let session = runtime.open(
        [Capability::ExecuteAgentScript],
        AgentLimits {
            max_runtime_millis: 500,
            ..Default::default()
        },
    );
    runtime.start(session).unwrap();
    let mut scripts = AgentScriptRuntime::default();
    let id = scripts.start(&mut runtime, session, request()).unwrap();
    let result = scripts
        .finish(&mut runtime, session, id, "2")
        .expect("agent output should complete");
    assert_eq!(result.output, "2");
    assert_eq!(result.provenance_reference, "agent-task:1");
    assert!(scripts.execution(id).is_none());
}

#[test]
fn agent_script_requires_provenance_and_agent_capability() {
    let mut runtime = AgentRuntime::default();
    let session = runtime.open([Capability::Render], Default::default());
    runtime.start(session).unwrap();
    let mut scripts = AgentScriptRuntime::default();
    let mut missing_provenance = request();
    missing_provenance.provenance_reference.clear();
    assert_eq!(
        scripts.start(&mut runtime, session, missing_provenance),
        Err(AgentScriptError::MissingProvenance)
    );
    assert_eq!(
        scripts.start(&mut runtime, session, request()),
        Err(AgentScriptError::Runtime(
            browsai_agent_runtime::AgentRuntimeError::CapabilityDenied(
                Capability::ExecuteAgentScript
            )
        ))
    );
}
