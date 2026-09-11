use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::Instant;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MetricValue {
    Counter(u64),
    Gauge(i64),
    Samples(Vec<f64>),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum MetricDomain {
    Performance,
    Memory,
    Concurrency,
    Network,
    Rendering,
    Token,
    Action,
    Security,
}

impl MetricDomain {
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::Performance => "performance",
            Self::Memory => "memory",
            Self::Concurrency => "concurrency",
            Self::Network => "network",
            Self::Rendering => "rendering",
            Self::Token => "token",
            Self::Action => "action",
            Self::Security => "security",
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MetricRegistry {
    values: BTreeMap<String, MetricValue>,
}

impl MetricRegistry {
    pub fn increment_domain(&mut self, domain: MetricDomain, name: &str, by: u64) {
        self.increment(format!("{}.{}", domain.prefix(), name), by);
    }

    pub fn gauge_domain(&mut self, domain: MetricDomain, name: &str, value: i64) {
        self.gauge(format!("{}.{}", domain.prefix(), name), value);
    }

    pub fn observe_domain(&mut self, domain: MetricDomain, name: &str, value: f64) {
        self.observe(format!("{}.{}", domain.prefix(), name), value);
    }

    pub fn increment(&mut self, name: impl Into<String>, by: u64) {
        if let MetricValue::Counter(value) = self
            .values
            .entry(name.into())
            .or_insert(MetricValue::Counter(0))
        {
            *value += by;
        }
    }
    pub fn gauge(&mut self, name: impl Into<String>, value: i64) {
        self.values.insert(name.into(), MetricValue::Gauge(value));
    }
    pub fn observe(&mut self, name: impl Into<String>, value: f64) {
        if let MetricValue::Samples(values) = self
            .values
            .entry(name.into())
            .or_insert_with(|| MetricValue::Samples(vec![]))
        {
            values.push(value);
        }
    }
    pub fn get(&self, name: &str) -> Option<&MetricValue> {
        self.values.get(name)
    }

    pub fn percentile(&self, name: &str, percentile: f64) -> Option<f64> {
        if !(0.0..=1.0).contains(&percentile) {
            return None;
        }
        let MetricValue::Samples(samples) = self.values.get(name)? else {
            return None;
        };
        if samples.is_empty() {
            return None;
        }
        let mut sorted = samples.clone();
        sorted.sort_by(f64::total_cmp);
        let index = percentile * (sorted.len() - 1) as f64;
        let lower = index.floor() as usize;
        let upper = index.ceil() as usize;
        if lower == upper {
            Some(sorted[lower])
        } else {
            let weight = index - lower as f64;
            Some(sorted[lower] + (sorted[upper] - sorted[lower]) * weight)
        }
    }

    pub fn median(&self, name: &str) -> Option<f64> {
        self.percentile(name, 0.5)
    }

    pub fn p95(&self, name: &str) -> Option<f64> {
        self.percentile(name, 0.95)
    }
    pub fn snapshot_json(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("metrics serialize")
    }

    pub fn timed<T>(&mut self, name: impl Into<String>, operation: impl FnOnce() -> T) -> T {
        let started = Instant::now();
        let result = operation();
        self.observe(name, started.elapsed().as_secs_f64() * 1000.0);
        result
    }

    pub fn merge(&mut self, other: &MetricRegistry) {
        for (name, value) in &other.values {
            match value {
                MetricValue::Counter(count) => self.increment(name.clone(), *count),
                MetricValue::Gauge(gauge) => self.gauge(name.clone(), *gauge),
                MetricValue::Samples(samples) => {
                    for sample in samples {
                        self.observe(name.clone(), *sample);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timed_measurements_and_merges_are_exportable() {
        let mut first = MetricRegistry::default();
        assert_eq!(first.timed("startup_ms", || 7), 7);
        let mut second = MetricRegistry::default();
        second.increment("requests", 2);
        first.merge(&second);
        assert!(matches!(
            first.get("requests"),
            Some(MetricValue::Counter(2))
        ));
        assert!(first.snapshot_json().get("values").is_some());
    }

    #[test]
    fn percentiles_are_deterministic_and_reject_non_sample_metrics() {
        let mut metrics = MetricRegistry::default();
        for sample in [4.0, 1.0, 3.0, 2.0] {
            metrics.observe("latency", sample);
        }
        assert_eq!(metrics.median("latency"), Some(2.5));
        assert!((metrics.p95("latency").unwrap() - 3.85).abs() < f64::EPSILON * 4.0);
        metrics.increment("requests", 1);
        assert_eq!(metrics.median("requests"), None);
        assert_eq!(metrics.percentile("latency", 1.1), None);
    }
}
