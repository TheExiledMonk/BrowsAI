use browsai_logging::{LogLevel, SecretSafeLogger};
use std::collections::BTreeMap;
use uuid::Uuid;

#[test]
fn sensitive_fields_are_redacted_at_collection_time() {
    let mut logger = SecretSafeLogger::default();
    let mut fields = BTreeMap::new();
    fields.insert("password".into(), "plaintext".into());
    fields.insert("status".into(), "ok".into());
    logger.log(
        LogLevel::Info,
        Uuid::new_v4(),
        "password=plaintext message",
        fields,
    );
    let trace = logger.debug_trace();
    assert!(!trace.contains("plaintext"));
    assert!(trace.contains("[REDACTED]"));
    assert!(trace.contains("ok"));
    assert!(!trace.contains("password=plaintext message"));
}

#[test]
fn logger_filters_exports_and_drains_correlated_events() {
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    let mut logger = SecretSafeLogger::default();
    logger.log(LogLevel::Info, first, "loaded", BTreeMap::new());
    logger.log(LogLevel::Debug, second, "debug", BTreeMap::new());
    assert_eq!(logger.events_for(first).count(), 1);
    let restored = SecretSafeLogger::from_json(&logger.debug_trace()).unwrap();
    assert_eq!(restored.events_for(second).count(), 1);
    assert_eq!(logger.drain().count(), 2);
    assert!(logger.events().is_empty());
}
