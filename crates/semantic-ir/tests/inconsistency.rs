use browsai_agent_tree::AgentRenderTree;
use browsai_fingerprint::{FingerprintId, FingerprintInconsistency};
use browsai_semantic_ir::SemanticIr;

#[test]
fn profile_inconsistencies_appear_on_the_agent_tree() {
    let tree = AgentRenderTree::new_page("https://example.test/");
    let ir = SemanticIr::from_structural(tree);
    assert!(!ir.profile_inconsistency_present());
    let id = FingerprintId::new("chrome-with-firefox-brands");
    let inconsistencies = vec![FingerprintInconsistency::UserAgentClaimsBrowser {
        claimed: "chrome".into(),
        declared: "firefox brands".into(),
    }];
    let ir2 = ir.with_profile_inconsistencies(id.as_str(), &inconsistencies);
    assert!(ir2.profile_inconsistency_present());
    let providers = ir2.challenge_providers();
    assert!(providers.is_empty());
}