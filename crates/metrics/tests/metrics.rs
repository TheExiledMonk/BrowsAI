use browsai_metrics::{MetricDomain, MetricRegistry, MetricValue};

#[test]
fn metrics_capture_counters_gauges_and_samples() {
    let mut metrics = MetricRegistry::default();
    metrics.increment("actions.total", 2);
    metrics.gauge("memory.bytes", 42);
    metrics.observe("navigation.latency_ms", 12.5);
    assert_eq!(metrics.get("actions.total"), Some(&MetricValue::Counter(2)));
    assert!(metrics.snapshot_json().get("values").is_some());
    assert_eq!(metrics.median("navigation.latency_ms"), Some(12.5));
}

#[test]
fn typed_domains_cover_runtime_measurement_surfaces() {
    let mut metrics = MetricRegistry::default();
    for domain in [
        MetricDomain::Performance,
        MetricDomain::Memory,
        MetricDomain::Concurrency,
        MetricDomain::Network,
        MetricDomain::Rendering,
        MetricDomain::Token,
        MetricDomain::Action,
        MetricDomain::Security,
    ] {
        metrics.increment_domain(domain, "events", 1);
        metrics.gauge_domain(domain, "current", 2);
        metrics.observe_domain(domain, "latency_ms", 3.0);
        assert!(!domain.prefix().is_empty());
    }
    assert_eq!(
        metrics.get("security.events"),
        Some(&MetricValue::Counter(1))
    );
    assert_eq!(metrics.median("token.latency_ms"), Some(3.0));
}
