use browsai_agent_runtime::{AgentRuntime, AuthorityDecision, ContentAssessment};
use browsai_sandbox::Capability;

const PROMPT_INJECTION_FIXTURE: &str =
    include_str!("../../../test-sites/malicious-content/prompt-injection.html");
const SECRET_EXFILTRATION_FIXTURE: &str =
    include_str!("../../../test-sites/malicious-content/secret-exfiltration.html");

#[test]
fn malicious_fixtures_are_untrusted_and_cannot_change_authority() {
    let mut runtime = AgentRuntime::default();
    let session = runtime.open([Capability::Render], Default::default());
    runtime.start(session).unwrap();
    for fixture in [PROMPT_INJECTION_FIXTURE, SECRET_EXFILTRATION_FIXTURE] {
        let page = runtime
            .capture_page_data(session, fixture, "malicious-fixture")
            .unwrap();
        let assessment: ContentAssessment = page.assess();
        assert!(assessment.suspicious);
        let authority = browsai_agent_runtime::PageContentAuthority::evaluate(&page);
        assert_eq!(
            authority.decision,
            AuthorityDecision::DeniedUntrustedContent
        );
        assert!(!authority.may_grant_capabilities);
        assert!(!authority.may_change_system_instructions);
        assert!(!authority.may_satisfy_confirmation);
        assert_eq!(
            runtime.evaluate_page_script(session, &page.text),
            Err(browsai_agent_runtime::AgentRuntimeError::CapabilityDenied(
                Capability::ExecutePageScript
            ))
        );
    }
}
