use browsai_sandbox::{Capability, SandboxPolicy};

#[test]
fn renderer_and_agent_policies_do_not_share_privileged_capabilities() {
    let renderer = SandboxPolicy::renderer("https://example.test");
    let agent = SandboxPolicy::agent();
    assert!(renderer.allows(Capability::Render));
    assert!(!renderer.allows(Capability::UseCredential));
    assert!(agent.allows(Capability::ExecuteAgentScript));
    assert!(!agent.allows(Capability::ExecutePageScript));
    assert!(!agent.allows(Capability::SpawnProcess));
}

#[test]
fn capability_request_origin_is_checked_by_scheme_host_and_port() {
    let policy = SandboxPolicy::renderer("https://example.test/page");
    assert!(policy.allows_origin("https://example.test/another-page"));
    assert!(!policy.allows_origin("http://example.test/another-page"));
}
