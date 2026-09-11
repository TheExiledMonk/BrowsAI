use browsai_provenance::{ProvenanceSource, SourceKind};
use browsai_runtime_observer::{RuntimeEvent, RuntimeEvidence, RuntimeObserver};

#[test]
fn runtime_evidence_preserves_event_source_without_changing_event_replay() {
    let mut observer = RuntimeObserver::default();
    observer.observe_with_provenance(
        RuntimeEvent::DomMutation { node_id: 9 },
        ProvenanceSource {
            kind: SourceKind::JavaScript,
            reference: "script:mutation-handler".into(),
            detail: Some("mutation callback".into()),
        },
    );
    let evidence: Vec<RuntimeEvidence> = observer.drain_evidence().collect();
    assert_eq!(evidence.len(), 1);
    assert_eq!(
        evidence[0].provenance[0].reference,
        "script:mutation-handler"
    );
    assert!(matches!(
        evidence[0].event,
        RuntimeEvent::DomMutation { node_id: 9 }
    ));
}
