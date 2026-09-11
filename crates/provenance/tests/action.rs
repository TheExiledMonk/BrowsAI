use browsai_provenance::ActionProvenance;
use browsai_transactions::{Consequence, PolicyDecision, Reversibility};

#[test]
fn action_provenance_keeps_policy_and_resolution_explicit() {
    let provenance = ActionProvenance {
        action_id: "a1".into(),
        agent_intent: "save invoice".into(),
        target_reference: "button:save".into(),
        resolved_node_id: Some("dom:9".into()),
        native_event_count: 3,
        consequence: Consequence::Unknown,
        reversibility: Reversibility::Unknown,
        policy_decision: PolicyDecision::RequireConfirmation,
        capability_reference: None,
        audit_id: Some("audit-1".into()),
        actor_id: Some("agent-a".into()),
        user_id: Some("user-a".into()),
    };
    assert_eq!(provenance.resolved_node_id.as_deref(), Some("dom:9"));
    assert_eq!(
        provenance.policy_decision,
        PolicyDecision::RequireConfirmation
    );
}
