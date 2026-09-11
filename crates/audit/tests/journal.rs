use browsai_audit::{action_event, safe_parameter_names, AuditError, AuditEvent, AuditJournal};
use browsai_input::ActionType;
use browsai_provenance::ActionProvenance;
use browsai_transactions::{Consequence, PolicyDecision, Reversibility, TransactionPolicy};
use serde_json::json;

fn provenance() -> ActionProvenance {
    ActionProvenance {
        action_id: "a1".into(),
        agent_intent: "submit form".into(),
        target_reference: "button:submit".into(),
        resolved_node_id: Some("dom:2".into()),
        native_event_count: 3,
        consequence: Consequence::Unknown,
        reversibility: Reversibility::Unknown,
        policy_decision: PolicyDecision::RequireConfirmation,
        capability_reference: None,
        audit_id: Some("audit-1".into()),
        actor_id: Some("agent-a".into()),
        user_id: Some("user-a".into()),
    }
}

#[test]
fn journal_is_append_only_and_secret_safe() {
    let parameters = json!({ "password": "never-log-this", "text": "hello" });
    assert_eq!(safe_parameter_names(&parameters), vec!["password", "text"]);
    let classification =
        TransactionPolicy::default().classify(Consequence::Unknown, Reversibility::Unknown);
    let event = action_event(
        provenance(),
        classification,
        &ActionType::Submit,
        &parameters,
        3,
    );
    let mut journal = AuditJournal::default();
    journal.append("audit-1", event);
    let serialized = serde_json::to_string(journal.records()).unwrap();
    assert!(!serialized.contains("never-log-this"));
    assert_eq!(journal.records()[0].sequence, 0);
    assert_eq!(journal.len(), 1);
    assert!(matches!(
        journal.records()[0].event,
        AuditEvent::Action { .. }
    ));
    assert!(journal.verify_integrity());
    let exported = journal.to_json().unwrap();
    assert!(AuditJournal::from_json(&exported).is_ok());
    let tampered = exported.replace("audit-1", "tampered");
    assert_eq!(
        AuditJournal::from_json(&tampered),
        Err(AuditError::Integrity)
    );
}

#[test]
fn journal_supports_correlation_replay_and_bounded_retention() {
    let mut journal = AuditJournal::default();
    for id in ["a", "b", "a"] {
        journal.append(
            id,
            AuditEvent::Recovery {
                component: "session".into(),
                outcome: "ok".into(),
            },
        );
    }
    assert_eq!(journal.records_for("a").count(), 2);
    let mut replayed = Vec::new();
    journal
        .replay(|record| replayed.push(record.audit_id.clone()))
        .unwrap();
    assert_eq!(replayed, vec!["a", "b", "a"]);
    assert_eq!(journal.retain_last(2), 1);
    assert!(journal.verify_integrity());
    assert_eq!(journal.records()[0].audit_id, "b");
}
