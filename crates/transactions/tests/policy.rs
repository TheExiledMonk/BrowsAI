use browsai_transactions::{Consequence, PolicyDecision, Reversibility, TransactionPolicy};

#[test]
fn policy_does_not_assume_unknown_or_financial_actions_are_safe() {
    let policy = TransactionPolicy::default();
    assert_eq!(
        policy
            .classify(Consequence::Unknown, Reversibility::Unknown)
            .decision,
        PolicyDecision::RequireConfirmation
    );
    assert_eq!(
        policy
            .classify(Consequence::Purchase, Reversibility::RemoteIrreversible)
            .decision,
        PolicyDecision::RequireConfirmation
    );
    assert_eq!(
        policy
            .classify(Consequence::Delete, Reversibility::RemoteIrreversible)
            .decision,
        PolicyDecision::Deny
    );
}

#[test]
fn confirmations_are_owner_bound_expiring_and_escalatable() {
    let classification =
        TransactionPolicy::default().classify(Consequence::Purchase, Reversibility::Unknown);
    let mut queue = browsai_transactions::ConfirmationQueue::default();
    let id = queue.enqueue("agent-a", "Purchase order", classification, 5, 10);
    assert!(!queue.approve(id, "agent-b", 11));
    assert!(queue.escalate(id, "agent-a", 11));
    assert_eq!(
        queue.get(id).unwrap().state,
        browsai_transactions::ConfirmationState::Escalated
    );
}

#[test]
fn expired_confirmation_is_marked_expired_when_used_after_deadline() {
    let classification =
        TransactionPolicy::default().classify(Consequence::Send, Reversibility::Unknown);
    let mut queue = browsai_transactions::ConfirmationQueue::default();
    let id = queue.enqueue("agent-a", "Send message", classification, 2, 10);
    assert!(!queue.approve(id, "agent-a", 12));
    assert_eq!(
        queue.get(id).unwrap().state,
        browsai_transactions::ConfirmationState::Expired
    );
}

#[test]
fn confirmation_transitions_emit_replayable_events() {
    let classification =
        TransactionPolicy::default().classify(Consequence::Send, Reversibility::Unknown);
    let mut queue = browsai_transactions::ConfirmationQueue::default();
    let id = queue.enqueue("agent-a", "Send message", classification, 5, 10);
    assert!(queue.approve(id, "agent-a", 11));
    let events: Vec<_> = queue.drain_events().collect();
    assert_eq!(events.len(), 2);
    assert_eq!(
        events[0].state,
        browsai_transactions::ConfirmationState::Pending
    );
    assert_eq!(
        events[1].state,
        browsai_transactions::ConfirmationState::Approved
    );
    assert_eq!(events[1].actor.as_deref(), Some("agent-a"));
}
