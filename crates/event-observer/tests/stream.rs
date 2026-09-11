use browsai_event_observer::{ChangeEvent, ChangeStream, EventTrace, StreamError};

#[test]
fn bounded_stream_reports_loss_and_preserves_order() {
    let mut stream = ChangeStream::bounded(2);
    stream.publish(ChangeEvent::NodeAdded {
        node_id: "a".into(),
    });
    stream.publish(ChangeEvent::NodeChanged {
        node_id: "a".into(),
    });
    stream.publish(ChangeEvent::NodeRemoved {
        node_id: "a".into(),
    });
    assert_eq!(stream.replay().count(), 2);
    assert!(matches!(stream.drain(), Err(StreamError { dropped: 1 })));
    assert!(stream.is_empty());
}

#[test]
fn loss_aware_drain_returns_retained_events_and_drop_count() {
    let mut stream = ChangeStream::bounded(2);
    stream.publish(ChangeEvent::NodeAdded {
        node_id: "a".into(),
    });
    stream.publish(ChangeEvent::NodeChanged {
        node_id: "a".into(),
    });
    stream.publish(ChangeEvent::NodeRemoved {
        node_id: "a".into(),
    });
    let batch = stream.drain_with_loss();
    assert_eq!(batch.dropped, 1);
    assert_eq!(batch.events.len(), 2);
    assert!(stream.drain_with_loss().events.is_empty());
}

#[test]
fn event_trace_round_trips_and_replays_in_order() {
    let mut trace = browsai_event_observer::EventTrace::default();
    trace.record(ChangeEvent::NavigationStarted {
        url: "https://example.test".into(),
    });
    trace.record(ChangeEvent::NavigationCompleted {
        url: "https://example.test".into(),
    });
    let decoded = browsai_event_observer::EventTrace::from_json(&trace.to_json().unwrap()).unwrap();
    assert_eq!(
        decoded
            .entries()
            .iter()
            .map(|entry| entry.sequence)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    let mut count = 0;
    decoded.replay(|_| count += 1).unwrap();
    assert_eq!(count, 2);
}

#[test]
fn event_trace_rejects_non_monotonic_serialized_entries() {
    let value = r#"{"entries":[{"sequence":2,"event":{"NodeAdded":{"node_id":"a"}}},{"sequence":1,"event":{"NodeRemoved":{"node_id":"a"}}}],"next_sequence":3}"#;
    assert_eq!(
        browsai_event_observer::EventTrace::from_json(value),
        Err(browsai_event_observer::TraceError::NonMonotonicSequence)
    );
}

#[test]
fn browser_event_domains_round_trip_as_typed_changes() {
    let mut trace = EventTrace::default();
    trace.record(ChangeEvent::StyleChanged {
        node_id: "dom:1".into(),
    });
    trace.record(ChangeEvent::RuntimeTaskQueued {
        task_id: "task:1".into(),
    });
    trace.record(ChangeEvent::InputDispatched {
        event: "pointerdown".into(),
    });
    trace.record(ChangeEvent::PermissionChanged {
        origin: "https://example.test".into(),
        permission: "Camera".into(),
    });
    let restored = EventTrace::from_json(&trace.to_json().unwrap()).unwrap();
    assert_eq!(restored.entries(), trace.entries());
}
