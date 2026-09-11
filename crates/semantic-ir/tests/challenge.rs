use browsai_agent_tree::AgentRenderTree;
use browsai_challenge_observer::{ChallengeKind, ChallengeObservation, ChallengeObservationSet};
use browsai_provenance::{ProvenanceSource, SourceKind};
use browsai_semantic_ir::SemanticIr;
use url::Url;

#[test]
fn challenge_observations_become_agent_tree_summary_nodes() {
    let url = Url::parse("https://example.test/login").unwrap();
    let observations = ChallengeObservationSet {
        observations: vec![
            ChallengeObservation {
                provider: "hcaptcha".into(),
                kind: ChallengeKind::Captcha,
                url: url.clone(),
                evidence: vec![],
                confidence: 0.95,
                provenance: vec![ProvenanceSource {
                    kind: SourceKind::Dom,
                    reference: "test".into(),
                    detail: None,
                }],
            },
            ChallengeObservation {
                provider: "cloudflare".into(),
                kind: ChallengeKind::BrowserChallenge,
                url: url.clone(),
                evidence: vec![],
                confidence: 0.9,
                provenance: vec![ProvenanceSource {
                    kind: SourceKind::NetworkResponse,
                    reference: "test".into(),
                    detail: None,
                }],
            },
        ],
    };
    let tree = AgentRenderTree::new_page("https://example.test/login");
    let ir = SemanticIr::from_structural(tree).with_challenge_observations(&observations);
    assert!(ir.challenge_present());
    let providers = ir.challenge_providers();
    assert!(providers.contains(&"hcaptcha".to_string()));
    assert!(providers.contains(&"cloudflare".to_string()));
}
