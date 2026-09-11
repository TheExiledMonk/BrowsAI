use browsai_provenance::{Confidence, ExplainabilityReport, ProvenanceSource, SourceKind};

#[test]
fn explainability_preserves_confidence_time_and_redaction() {
    let mut report = ExplainabilityReport::new("order:1", "Observed order total");
    report.add(
        ProvenanceSource {
            kind: SourceKind::NetworkResponse,
            reference: "request-1".into(),
            detail: None,
        },
        Confidence::PROBABLE,
        42,
        Some("token=secret".into()),
        true,
    );
    assert!(report.inferred());
    assert_eq!(report.evidence[0].observed_at_millis, 42);
    assert_eq!(report.evidence[0].detail.as_deref(), Some("[REDACTED]"));
    assert_eq!(
        ExplainabilityReport::redact_text("password=secret"),
        "[REDACTED]"
    );
}
