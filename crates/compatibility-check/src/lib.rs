//! Non-destructive compatibility checking primitives.
//!
//! The checker deliberately separates observation and reporting from browser
//! execution.  Browser adapters can implement [`SiteExecutor`] without being
//! able to bypass [`SafetyGate`].

use browsai_metrics::MetricRegistry;
use browsai_transactions::{Consequence, PolicyDecision, Reversibility, TransactionPolicy};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use url::Url;
use uuid::Uuid;

pub const SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum SiteStatus {
    Pass,
    PassWithWarnings,
    Partial,
    Fail,
    Blocked,
    Timeout,
    Crash,
    Skipped,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum Subsystem {
    Engine,
    Javascript,
    Dom,
    Css,
    Layout,
    AgentTree,
    StructuralIr,
    SemanticIr,
    ApplicationIr,
    Provenance,
    Input,
    State,
    Network,
    Storage,
    Stability,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum FailureCategory {
    Network,
    Dns,
    Tls,
    Http,
    Redirect,
    Engine,
    Html,
    Css,
    Layout,
    Javascript,
    Dom,
    Event,
    Input,
    HitTest,
    Focus,
    Keyboard,
    Pointer,
    Scroll,
    Navigation,
    History,
    Frame,
    ShadowDom,
    ServiceWorker,
    Worker,
    WebSocket,
    Storage,
    AgentTree,
    StructuralIr,
    SemanticIr,
    ApplicationIr,
    Provenance,
    StableId,
    Snapshot,
    Diff,
    Security,
    Timeout,
    Hang,
    Crash,
    Memory,
    Performance,
    SiteBlock,
    Unsupported,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum Phase {
    Startup,
    Navigation,
    RuntimeStabilization,
    BrowserObservation,
    AgentTree,
    InteractionDiscovery,
    SafeInteraction,
    DynamicState,
    HistoryAndTabs,
    Finalization,
    Cleanup,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CorpusState {
    Pending,
    Running,
    Complete,
    Failed,
    RetryPending,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SiteRecord {
    pub id: String,
    pub url: Url,
    pub domain: String,
    pub category: String,
    pub priority: u32,
    pub region: Option<String>,
    pub expected_technology: Option<String>,
    pub notes: Option<String>,
    pub enabled: bool,
}

/// Deterministic corpus selection applied before a run is created.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SiteSelection {
    pub categories: BTreeSet<String>,
    pub domains: BTreeSet<String>,
    pub min_priority: Option<u32>,
    pub max_sites: Option<usize>,
    pub enabled_only: bool,
}

impl SiteSelection {
    pub fn select(&self, sites: &[SiteRecord]) -> Vec<SiteRecord> {
        let mut selected = sites
            .iter()
            .filter(|site| !self.enabled_only || site.enabled)
            .filter(|site| self.categories.is_empty() || self.categories.contains(&site.category))
            .filter(|site| self.domains.is_empty() || self.domains.contains(&site.domain))
            .filter(|site| self.min_priority.map_or(true, |min| site.priority >= min))
            .cloned()
            .collect::<Vec<_>>();
        selected.sort_by(|left, right| {
            right
                .priority
                .cmp(&left.priority)
                .then_with(|| left.id.cmp(&right.id))
        });
        if let Some(max_sites) = self.max_sites {
            selected.truncate(max_sites);
        }
        selected
    }
}

impl SiteRecord {
    pub fn new(id: impl Into<String>, url: Url) -> Self {
        let domain = url.domain().unwrap_or_default().to_owned();
        Self {
            id: id.into(),
            url,
            domain,
            category: "uncategorized".into(),
            priority: 0,
            region: None,
            expected_technology: None,
            notes: None,
            enabled: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CheckConfig {
    pub workers: usize,
    pub resume: bool,
    pub timeout_seconds: u64,
    pub max_scrolls: u32,
    pub max_safe_actions: u32,
    pub max_tabs: u32,
    pub max_network_requests: u32,
    pub max_download_bytes: u64,
    pub screenshot_on_failure: bool,
    pub network_trace_on_failure: bool,
    pub runtime_trace_on_failure: bool,
    pub agent_tree_on_failure: bool,
}

impl Default for CheckConfig {
    fn default() -> Self {
        Self {
            workers: 1,
            resume: true,
            timeout_seconds: 60,
            max_scrolls: 5,
            max_safe_actions: 20,
            max_tabs: 3,
            max_network_requests: 500,
            max_download_bytes: 10 * 1024 * 1024,
            screenshot_on_failure: true,
            network_trace_on_failure: true,
            runtime_trace_on_failure: true,
            agent_tree_on_failure: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubsystemResult {
    pub status: SiteStatus,
    pub details: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FailureRecord {
    pub id: String,
    pub run_id: String,
    pub site_id: String,
    pub url: Url,
    pub phase: Phase,
    pub category: FailureCategory,
    pub severity: Severity,
    pub summary: String,
    pub expected: Option<String>,
    pub observed: Option<String>,
    pub reproducible: bool,
    pub fingerprint: String,
    pub artifacts: Vec<String>,
}

impl FailureRecord {
    pub fn new(
        run_id: &str,
        site: &SiteRecord,
        phase: Phase,
        category: FailureCategory,
        severity: Severity,
        summary: impl Into<String>,
    ) -> Self {
        let summary = summary.into();
        let fingerprint = fingerprint(category, None, None, &summary);
        Self {
            id: format!("failure-{}", Uuid::new_v4()),
            run_id: run_id.into(),
            site_id: site.id.clone(),
            url: site.url.clone(),
            phase,
            category,
            severity,
            summary,
            expected: None,
            observed: None,
            reproducible: false,
            fingerprint,
            artifacts: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SiteResult {
    pub site: Option<SiteRecord>,
    pub status: Option<SiteStatus>,
    pub state: Option<CorpusState>,
    pub subsystems: BTreeMap<Subsystem, SubsystemResult>,
    pub failures: Vec<FailureRecord>,
    pub metrics: MetricRegistry,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckRun {
    pub schema_version: u16,
    pub id: String,
    pub config: CheckConfig,
    pub state: CorpusState,
    pub sites: Vec<SiteResult>,
    pub created_at_unix_seconds: u64,
}

impl CheckRun {
    pub fn new(config: CheckConfig, sites: Vec<SiteRecord>, now: u64) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            id: format!("run-{}", Uuid::new_v4()),
            config,
            state: CorpusState::Pending,
            sites: sites
                .into_iter()
                .map(|site| SiteResult {
                    site: Some(site),
                    state: Some(CorpusState::Pending),
                    ..Default::default()
                })
                .collect(),
            created_at_unix_seconds: now,
        }
    }

    pub fn pending_site_indices(&self) -> impl Iterator<Item = usize> + '_ {
        self.sites.iter().enumerate().filter_map(|(index, result)| {
            matches!(
                result.state,
                Some(CorpusState::Pending | CorpusState::RetryPending)
            )
            .then_some(index)
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CheckStoreError {
    Io(String),
    Serialization(String),
    Invalid(String),
}

impl std::fmt::Display for CheckStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for CheckStoreError {}

#[derive(Clone, Debug)]
pub struct RunStore {
    root: PathBuf,
}

impl RunStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn run_path(&self, run_id: &str) -> PathBuf {
        self.root.join(run_id).join("run.json")
    }

    pub fn save(&self, run: &CheckRun) -> Result<(), CheckStoreError> {
        let path = self.run_path(&run.id);
        let directory = path
            .parent()
            .ok_or_else(|| CheckStoreError::Invalid("run path has no parent".into()))?;
        fs::create_dir_all(directory).map_err(|error| CheckStoreError::Io(error.to_string()))?;
        let bytes = serde_json::to_vec_pretty(run)
            .map_err(|error| CheckStoreError::Serialization(error.to_string()))?;
        let temporary = directory.join("run.json.tmp");
        fs::write(&temporary, bytes).map_err(|error| CheckStoreError::Io(error.to_string()))?;
        fs::rename(temporary, path).map_err(|error| CheckStoreError::Io(error.to_string()))
    }

    pub fn load(&self, run_id: &str) -> Result<CheckRun, CheckStoreError> {
        let path = self.run_path(run_id);
        let contents =
            fs::read_to_string(&path).map_err(|error| CheckStoreError::Io(error.to_string()))?;
        let run: CheckRun = serde_json::from_str(&contents)
            .map_err(|error| CheckStoreError::Serialization(error.to_string()))?;
        if run.schema_version > SCHEMA_VERSION {
            return Err(CheckStoreError::Invalid(
                "unsupported run schema version".into(),
            ));
        }
        Ok(run)
    }
}

pub fn load_sites_csv(path: impl AsRef<Path>) -> Result<Vec<SiteRecord>, CheckStoreError> {
    let contents =
        fs::read_to_string(path).map_err(|error| CheckStoreError::Io(error.to_string()))?;
    let mut lines = contents.lines();
    let header = lines.next().unwrap_or_default();
    let columns: Vec<_> = header.split(',').map(str::trim).collect();
    let required = ["id", "url", "domain", "category", "priority", "enabled"];
    if !required.iter().all(|field| columns.contains(field)) {
        return Err(CheckStoreError::Invalid(
            "sites.csv is missing required columns".into(),
        ));
    }
    let index = |field: &str| columns.iter().position(|column| *column == field).unwrap();
    let mut sites = Vec::new();
    for (line_number, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let values: Vec<_> = line.split(',').map(str::trim).collect();
        let url =
            Url::parse(values.get(index("url")).copied().unwrap_or_default()).map_err(|error| {
                CheckStoreError::Invalid(format!("line {}: {error}", line_number + 2))
            })?;
        let mut site = SiteRecord::new(values[index("id")], url);
        site.domain = values[index("domain")].to_owned();
        site.category = values[index("category")].to_owned();
        site.priority = values[index("priority")].parse().map_err(|_| {
            CheckStoreError::Invalid(format!("line {}: invalid priority", line_number + 2))
        })?;
        site.enabled = matches!(
            values[index("enabled")].to_ascii_lowercase().as_str(),
            "1" | "true" | "yes"
        );
        sites.push(site);
    }
    Ok(sites)
}

#[derive(Clone, Debug, Default)]
pub struct SafetyGate {
    policy: TransactionPolicy,
}

impl SafetyGate {
    pub fn classify(
        &self,
        consequence: Consequence,
        reversibility: Reversibility,
    ) -> PolicyDecision {
        self.policy.classify(consequence, reversibility).decision
    }

    pub fn permits(&self, consequence: Consequence, reversibility: Reversibility) -> bool {
        matches!(
            self.classify(consequence, reversibility),
            PolicyDecision::Allow
        )
    }
}

pub trait SiteExecutor {
    fn execute(&mut self, site: &SiteRecord, config: &CheckConfig) -> SiteResult;
}

pub fn run_site<E: SiteExecutor>(
    executor: &mut E,
    site: SiteRecord,
    run_id: &str,
    config: &CheckConfig,
) -> SiteResult {
    let mut result = executor.execute(&site, config);
    result.site = Some(site);
    for failure in &mut result.failures {
        if failure.run_id.is_empty() {
            failure.run_id = run_id.into();
        }
    }
    result
}

pub fn fingerprint(
    category: FailureCategory,
    engine_error: Option<&str>,
    semantic_path: Option<&str>,
    feature: &str,
) -> String {
    let normalized = [
        format!("{category:?}"),
        normalize(engine_error.unwrap_or("")),
        normalize(semantic_path.unwrap_or("")),
        normalize(feature),
    ]
    .join("|");
    format!("fp-{:016x}", fnv1a(normalized.as_bytes()))
}

pub fn cluster_failures(failures: &[FailureRecord]) -> BTreeMap<String, Vec<String>> {
    let mut clusters: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for failure in failures {
        clusters
            .entry(failure.fingerprint.clone())
            .or_default()
            .push(failure.id.clone());
    }
    clusters
}

/// A bounded retry decision. Timeout, hang, and crash failures are retryable;
/// policy, safety, and deterministic compatibility failures are not.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_attempts: u32,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self { max_attempts: 2 }
    }
}

impl RetryPolicy {
    pub fn should_retry(&self, attempt: u32, result: &SiteResult) -> bool {
        attempt < self.max_attempts
            && result.failures.iter().any(|failure| {
                matches!(
                    failure.category,
                    FailureCategory::Timeout | FailureCategory::Hang | FailureCategory::Crash
                )
            })
    }
}

/// Scheduler state is intentionally serializable so a caller can persist it
/// alongside `CheckRun` and resume without reordering the corpus.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchedulerConfig {
    pub workers: usize,
    pub max_attempts: u32,
    pub requests_per_site: u32,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            workers: 1,
            max_attempts: 2,
            requests_per_site: 500,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduledSite {
    pub index: usize,
    pub attempt: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunScheduler {
    pub config: SchedulerConfig,
    pub queue: Vec<ScheduledSite>,
}

impl RunScheduler {
    pub fn from_run(run: &CheckRun, config: SchedulerConfig) -> Self {
        let queue = run
            .pending_site_indices()
            .map(|index| ScheduledSite { index, attempt: 0 })
            .collect();
        Self { config, queue }
    }

    pub fn next_batch(&mut self) -> Vec<ScheduledSite> {
        let count = self.config.workers.max(1).min(self.queue.len());
        self.queue.drain(..count).collect()
    }

    pub fn retry(&mut self, site: ScheduledSite, result: &SiteResult) {
        let policy = RetryPolicy {
            max_attempts: self.config.max_attempts,
        };
        if policy.should_retry(site.attempt, result) {
            self.queue.push(ScheduledSite {
                index: site.index,
                attempt: site.attempt + 1,
            });
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RunComparison {
    pub added_sites: Vec<String>,
    pub removed_sites: Vec<String>,
    pub status_changes: Vec<StatusChange>,
    pub new_fingerprints: Vec<String>,
    pub resolved_fingerprints: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StatusChange {
    pub site_id: String,
    pub before: Option<SiteStatus>,
    pub after: Option<SiteStatus>,
}

pub fn compare_runs(before: &CheckRun, after: &CheckRun) -> RunComparison {
    let left = before
        .sites
        .iter()
        .filter_map(|result| result.site.as_ref().map(|site| (site.id.clone(), result)))
        .collect::<BTreeMap<_, _>>();
    let right = after
        .sites
        .iter()
        .filter_map(|result| result.site.as_ref().map(|site| (site.id.clone(), result)))
        .collect::<BTreeMap<_, _>>();
    let added_sites = right
        .keys()
        .filter(|id| !left.contains_key(*id))
        .cloned()
        .collect();
    let removed_sites = left
        .keys()
        .filter(|id| !right.contains_key(*id))
        .cloned()
        .collect();
    let mut comparison = RunComparison {
        added_sites,
        removed_sites,
        ..Default::default()
    };
    for id in left.keys().filter(|id| right.contains_key(*id)) {
        let before_status = left[id].status;
        let after_status = right[id].status;
        if before_status != after_status {
            comparison.status_changes.push(StatusChange {
                site_id: id.clone(),
                before: before_status,
                after: after_status,
            });
        }
    }
    let fingerprints = |run: &CheckRun| {
        run.sites
            .iter()
            .flat_map(|site| {
                site.failures
                    .iter()
                    .map(|failure| failure.fingerprint.clone())
            })
            .collect::<BTreeSet<_>>()
    };
    let old = fingerprints(before);
    let new = fingerprints(after);
    comparison.new_fingerprints = new.difference(&old).cloned().collect();
    comparison.resolved_fingerprints = old.difference(&new).cloned().collect();
    comparison
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CoverageSummary {
    pub scheduled_sites: usize,
    pub tested_sites: usize,
    pub passed_sites: usize,
    pub subsystem_observations: BTreeMap<Subsystem, usize>,
    pub feature_observations: BTreeMap<String, usize>,
}

impl CoverageSummary {
    pub fn from_run(run: &CheckRun) -> Self {
        let mut coverage = Self {
            scheduled_sites: run.sites.len(),
            ..Self::default()
        };
        for site in &run.sites {
            if site.status.is_some() {
                coverage.tested_sites += 1;
            }
            if matches!(
                site.status,
                Some(SiteStatus::Pass | SiteStatus::PassWithWarnings)
            ) {
                coverage.passed_sites += 1;
            }
            for (subsystem, result) in &site.subsystems {
                *coverage
                    .subsystem_observations
                    .entry(*subsystem)
                    .or_default() += 1;
                if let Some(details) = &result.details {
                    *coverage
                        .feature_observations
                        .entry(details.clone())
                        .or_default() += 1;
                }
            }
        }
        coverage
    }
}

pub fn compatibility_score(run: &CheckRun) -> f32 {
    if run.sites.is_empty() {
        return 0.0;
    }
    let total = run.sites.len() as f32;
    let weighted = run
        .sites
        .iter()
        .map(|site| match site.status {
            Some(SiteStatus::Pass) => 1.0,
            Some(SiteStatus::PassWithWarnings) => 0.85,
            Some(SiteStatus::Partial) => 0.5,
            Some(SiteStatus::Skipped) => 0.0,
            _ => 0.0,
        })
        .sum::<f32>();
    weighted / total
}

fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ArtifactManifest {
    pub files: BTreeSet<String>,
    pub redacted_fields: BTreeSet<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RunSummary {
    pub scheduled: usize,
    pub tested: usize,
    pub statuses: BTreeMap<SiteStatus, usize>,
    pub subsystem_passes: BTreeMap<Subsystem, usize>,
    pub failures: usize,
    pub fingerprints: usize,
}

impl RunSummary {
    pub fn from_run(run: &CheckRun) -> Self {
        let mut summary = Self {
            scheduled: run.sites.len(),
            ..Self::default()
        };
        let mut fingerprints = BTreeSet::new();
        for site in &run.sites {
            if site.status.is_some() {
                summary.tested += 1;
            }
            if let Some(status) = site.status {
                *summary.statuses.entry(status).or_default() += 1;
            }
            summary.failures += site.failures.len();
            for failure in &site.failures {
                fingerprints.insert(failure.fingerprint.clone());
            }
            for (subsystem, result) in &site.subsystems {
                if matches!(
                    result.status,
                    SiteStatus::Pass | SiteStatus::PassWithWarnings
                ) {
                    *summary.subsystem_passes.entry(*subsystem).or_default() += 1;
                }
            }
        }
        summary.fingerprints = fingerprints.len();
        summary
    }
}

pub fn render_json_report(run: &CheckRun) -> Result<String, CheckStoreError> {
    let value = serde_json::json!({
        "schemaVersion": SCHEMA_VERSION,
        "run": run,
        "summary": RunSummary::from_run(run),
    });
    serde_json::to_string_pretty(&value)
        .map_err(|error| CheckStoreError::Serialization(error.to_string()))
}

pub fn render_csv_report(run: &CheckRun) -> String {
    let mut csv = String::from("site_id,url,status,failures\n");
    for result in &run.sites {
        let Some(site) = &result.site else { continue };
        let status = result
            .status
            .map(|value| format!("{value:?}"))
            .unwrap_or_else(|| "PENDING".into());
        csv.push_str(&format!(
            "{},{},{},{}\n",
            site.id,
            site.url,
            status,
            result.failures.len()
        ));
    }
    csv
}

pub fn render_html_report(run: &CheckRun) -> String {
    let summary = RunSummary::from_run(run);
    let mut rows = String::new();
    for result in &run.sites {
        if let Some(site) = &result.site {
            let status = result
                .status
                .map(|value| format!("{value:?}"))
                .unwrap_or_else(|| "PENDING".into());
            rows.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape_html(&site.id),
                escape_html(site.url.as_str()),
                status,
                result.failures.len()
            ));
        }
    }
    format!("<!doctype html><meta charset=\"utf-8\"><title>BrowsAI Compatibility Run</title><h1>BrowsAI Compatibility Run</h1><p>Sites scheduled: {} · tested: {} · failures: {} · fingerprints: {}</p><table><thead><tr><th>Site</th><th>URL</th><th>Status</th><th>Failures</th></tr></thead><tbody>{rows}</tbody></table>", summary.scheduled, summary.tested, summary.failures, summary.fingerprints)
}

pub fn write_reports(run: &CheckRun, directory: impl AsRef<Path>) -> Result<(), CheckStoreError> {
    let directory = directory.as_ref();
    fs::create_dir_all(directory).map_err(|error| CheckStoreError::Io(error.to_string()))?;
    fs::write(directory.join("summary.json"), render_json_report(run)?)
        .map_err(|error| CheckStoreError::Io(error.to_string()))?;
    fs::write(directory.join("results.csv"), render_csv_report(run))
        .map_err(|error| CheckStoreError::Io(error.to_string()))?;
    fs::write(directory.join("report.html"), render_html_report(run))
        .map_err(|error| CheckStoreError::Io(error.to_string()))?;
    Ok(())
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

impl ArtifactManifest {
    pub fn add(&mut self, name: impl Into<String>) {
        self.files.insert(name.into());
    }
    pub fn redact(&mut self, field: impl Into<String>) {
        self.redacted_fields.insert(field.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    struct PassingExecutor;
    impl SiteExecutor for PassingExecutor {
        fn execute(&mut self, _site: &SiteRecord, _config: &CheckConfig) -> SiteResult {
            SiteResult {
                status: Some(SiteStatus::Pass),
                state: Some(CorpusState::Complete),
                ..Default::default()
            }
        }
    }

    #[test]
    fn run_store_round_trips_and_exposes_pending_sites() {
        let root = tempdir().unwrap();
        let site = SiteRecord::new("one", Url::parse("https://example.test").unwrap());
        let run = CheckRun::new(CheckConfig::default(), vec![site], 42);
        let store = RunStore::new(root.path());
        store.save(&run).unwrap();
        let loaded = store.load(&run.id).unwrap();
        assert_eq!(loaded.pending_site_indices().collect::<Vec<_>>(), vec![0]);
    }

    #[test]
    fn safety_gate_denies_unknown_and_allows_local_reversible_work() {
        let gate = SafetyGate::default();
        assert!(!gate.permits(Consequence::Unknown, Reversibility::Unknown));
        assert!(gate.permits(Consequence::LocalWrite, Reversibility::LocalReversible));
        assert!(!gate.permits(Consequence::Delete, Reversibility::RemoteIrreversible));
    }

    #[test]
    fn fingerprints_group_normalized_failures() {
        let first = fingerprint(
            FailureCategory::StableId,
            Some("React  node"),
            Some("/main"),
            "ID churn",
        );
        let second = fingerprint(
            FailureCategory::StableId,
            Some("React node"),
            Some("/main"),
            "id churn",
        );
        assert_eq!(first, second);
    }

    #[test]
    fn executor_result_is_bound_to_site_and_run() {
        let site = SiteRecord::new("one", Url::parse("https://example.test").unwrap());
        let result = run_site(
            &mut PassingExecutor,
            site.clone(),
            "run-1",
            &CheckConfig::default(),
        );
        assert_eq!(result.site, Some(site));
        assert_eq!(result.status, Some(SiteStatus::Pass));
    }

    #[test]
    fn reports_include_summary_and_escape_site_values() {
        let mut site = SiteRecord::new("one", Url::parse("https://example.test/").unwrap());
        site.id = "<one>".into();
        let mut run = CheckRun::new(CheckConfig::default(), vec![site], 42);
        run.sites[0].status = Some(SiteStatus::Pass);
        let json = render_json_report(&run).unwrap();
        assert!(json.contains("scheduled"));
        let html = render_html_report(&run);
        assert!(html.contains("&lt;one&gt;"));
        assert!(render_csv_report(&run).contains("<one>,"));
    }

    #[test]
    fn selection_is_enabled_priority_and_id_deterministic() {
        let mut high = SiteRecord::new("b", Url::parse("https://b.test").unwrap());
        high.priority = 10;
        let mut low = SiteRecord::new("a", Url::parse("https://a.test").unwrap());
        low.priority = 1;
        low.enabled = false;
        let selected = SiteSelection {
            enabled_only: true,
            max_sites: Some(1),
            ..Default::default()
        }
        .select(&[low, high.clone()]);
        assert_eq!(selected, vec![high]);
    }

    #[test]
    fn scheduler_retries_only_transient_failures() {
        let site = SiteRecord::new("one", Url::parse("https://example.test").unwrap());
        let run = CheckRun::new(CheckConfig::default(), vec![site.clone()], 0);
        let mut scheduler = RunScheduler::from_run(&run, SchedulerConfig::default());
        let batch = scheduler.next_batch();
        assert_eq!(batch[0].index, 0);
        let mut timeout = SiteResult {
            status: Some(SiteStatus::Timeout),
            ..Default::default()
        };
        timeout.failures.push(FailureRecord::new(
            "run",
            &site,
            Phase::Finalization,
            FailureCategory::Timeout,
            Severity::High,
            "timeout",
        ));
        scheduler.retry(batch[0].clone(), &timeout);
        assert_eq!(scheduler.queue[0].attempt, 1);
    }

    #[test]
    fn comparison_and_score_are_stable() {
        let site = SiteRecord::new("one", Url::parse("https://example.test").unwrap());
        let mut before = CheckRun::new(CheckConfig::default(), vec![site.clone()], 0);
        before.sites[0].status = Some(SiteStatus::Partial);
        let mut after = before.clone();
        after.sites[0].status = Some(SiteStatus::Pass);
        assert_eq!(compare_runs(&before, &after).status_changes.len(), 1);
        assert_eq!(compatibility_score(&after), 1.0);
    }
}
