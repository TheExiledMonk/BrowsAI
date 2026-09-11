//! Machine-readable browser API support and fixture compatibility reporting.

use browsai_engine_api::{EngineCapabilities, EngineFeature};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SupportStatus {
    Supported,
    Partial,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum WebApi {
    Forms,
    Dialogs,
    Focus,
    Selection,
    Clipboard,
    Downloads,
    Uploads,
    Media,
    Canvas,
    WebGl,
    Geolocation,
    DeviceCapabilities,
    Notifications,
    Storage,
    ServiceWorkers,
    WebAuthn,
    Permissions,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApiSupport {
    pub status: SupportStatus,
    pub capability: Option<String>,
    pub unsupported_behavior: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SupportMatrix {
    pub engine_name: String,
    pub engine_version: Option<String>,
    pub apis: BTreeMap<WebApi, ApiSupport>,
}

impl SupportMatrix {
    pub fn from_capabilities(capabilities: &EngineCapabilities) -> Self {
        let support = |api: WebApi| {
            let feature = match api {
                WebApi::Forms | WebApi::Focus | WebApi::Selection | WebApi::Uploads => {
                    EngineFeature::NativeInput
                }
                WebApi::Dialogs | WebApi::Notifications | WebApi::Permissions => {
                    EngineFeature::RuntimeObservation
                }
                WebApi::Downloads | WebApi::ServiceWorkers => EngineFeature::NetworkObservation,
                WebApi::Canvas => EngineFeature::LayoutObservation,
                WebApi::Storage => EngineFeature::Snapshots,
                WebApi::Clipboard
                | WebApi::Media
                | WebApi::WebGl
                | WebApi::Geolocation
                | WebApi::DeviceCapabilities
                | WebApi::WebAuthn => return None,
            };
            capabilities.features.contains(&feature).then_some(feature)
        };
        let mut apis = BTreeMap::new();
        for api in [
            WebApi::Forms,
            WebApi::Dialogs,
            WebApi::Focus,
            WebApi::Selection,
            WebApi::Clipboard,
            WebApi::Downloads,
            WebApi::Uploads,
            WebApi::Media,
            WebApi::Canvas,
            WebApi::WebGl,
            WebApi::Geolocation,
            WebApi::DeviceCapabilities,
            WebApi::Notifications,
            WebApi::Storage,
            WebApi::ServiceWorkers,
            WebApi::WebAuthn,
            WebApi::Permissions,
        ] {
            apis.insert(
                api,
                ApiSupport {
                    status: if support(api).is_some() {
                        SupportStatus::Partial
                    } else {
                        SupportStatus::Unsupported
                    },
                    capability: support(api).map(|feature| format!("{feature:?}")),
                    unsupported_behavior: "return normalized Unsupported instead of guessing"
                        .into(),
                },
            );
        }
        Self {
            engine_name: capabilities.engine_name.clone(),
            engine_version: capabilities.engine_version.clone(),
            apis,
        }
    }
    pub fn status(&self, api: WebApi) -> SupportStatus {
        self.apis
            .get(&api)
            .map(|support| support.status)
            .unwrap_or(SupportStatus::Unsupported)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConformanceFixture {
    pub id: String,
    pub api: WebApi,
    pub input: serde_json::Value,
    pub expected: serde_json::Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FixtureLoadError {
    Io(String),
    Serialization(String),
    InvalidFixture(String),
}

pub fn load_fixture(path: impl AsRef<Path>) -> Result<ConformanceFixture, FixtureLoadError> {
    let path = path.as_ref();
    let bytes = std::fs::read(path).map_err(|error| FixtureLoadError::Io(error.to_string()))?;
    let fixture: ConformanceFixture = serde_json::from_slice(&bytes)
        .map_err(|error| FixtureLoadError::Serialization(format!("{}: {error}", path.display())))?;
    if fixture.id.trim().is_empty() {
        return Err(FixtureLoadError::InvalidFixture(format!(
            "{}: fixture id is empty",
            path.display()
        )));
    }
    Ok(fixture)
}

pub fn load_fixtures(
    directory: impl AsRef<Path>,
) -> Result<Vec<ConformanceFixture>, FixtureLoadError> {
    let mut paths = json_paths(directory.as_ref())?;
    paths.sort();
    paths.into_iter().map(load_fixture).collect()
}

fn json_paths(directory: &Path) -> Result<Vec<PathBuf>, FixtureLoadError> {
    let mut paths = Vec::new();
    for entry in
        std::fs::read_dir(directory).map_err(|error| FixtureLoadError::Io(error.to_string()))?
    {
        let path = entry
            .map_err(|error| FixtureLoadError::Io(error.to_string()))?
            .path();
        if path.is_dir() {
            paths.extend(json_paths(&path)?);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            paths.push(path);
        }
    }
    Ok(paths)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FixtureResult {
    pub fixture_id: String,
    pub passed: bool,
    pub actual: serde_json::Value,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CompatibilityReport {
    pub engine: Option<SupportMatrix>,
    pub results: Vec<FixtureResult>,
}

impl CompatibilityReport {
    pub fn record(&mut self, fixture: &ConformanceFixture, actual: serde_json::Value) {
        self.results.push(FixtureResult {
            fixture_id: fixture.id.clone(),
            passed: actual == fixture.expected,
            actual,
            detail: None,
        });
    }

    pub fn run<F>(
        engine: Option<SupportMatrix>,
        fixtures: &[ConformanceFixture],
        mut evaluate: F,
    ) -> Self
    where
        F: FnMut(&ConformanceFixture) -> Result<serde_json::Value, String>,
    {
        let mut report = Self {
            engine,
            results: Vec::with_capacity(fixtures.len()),
        };
        for fixture in fixtures {
            match evaluate(fixture) {
                Ok(actual) => report.record(fixture, actual),
                Err(error) => report.results.push(FixtureResult {
                    fixture_id: fixture.id.clone(),
                    passed: false,
                    actual: serde_json::Value::Null,
                    detail: Some(error),
                }),
            }
        }
        report
    }

    pub fn passed(&self) -> bool {
        self.results.iter().all(|result| result.passed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_and_fixture_report_are_serializable_and_deterministic() {
        let matrix = SupportMatrix::from_capabilities(&EngineCapabilities {
            engine_name: "test".into(),
            engine_version: Some("1".into()),
            features: [EngineFeature::NativeInput].into_iter().collect(),
            live_browser_compiled: false,
            commands: Vec::new(),
        });
        assert_eq!(matrix.status(WebApi::Forms), SupportStatus::Partial);
        assert_eq!(matrix.status(WebApi::Clipboard), SupportStatus::Unsupported);
        assert_eq!(
            matrix.apis[&WebApi::Forms].capability.as_deref(),
            Some("NativeInput")
        );
        let fixture = ConformanceFixture {
            id: "forms-basic".into(),
            api: WebApi::Forms,
            input: serde_json::json!({"value":"x"}),
            expected: serde_json::json!({"normalized":"x"}),
        };
        let mut report = CompatibilityReport {
            engine: Some(matrix),
            results: vec![],
        };
        report.record(&fixture, serde_json::json!({"normalized":"x"}));
        assert!(report.passed());
    }

    #[test]
    fn fixture_runner_preserves_expected_failures_and_evaluation_errors() {
        let fixtures = vec![
            ConformanceFixture {
                id: "pass".into(),
                api: WebApi::Forms,
                input: serde_json::json!({}),
                expected: serde_json::json!({"ok":true}),
            },
            ConformanceFixture {
                id: "error".into(),
                api: WebApi::Storage,
                input: serde_json::json!({}),
                expected: serde_json::json!({"ok":true}),
            },
        ];
        let report = CompatibilityReport::run(None, &fixtures, |fixture| {
            if fixture.id == "error" {
                Err("unsupported in test engine".into())
            } else {
                Ok(serde_json::json!({"ok":true}))
            }
        });
        assert!(!report.passed());
        assert!(report.results[0].passed);
        assert_eq!(
            report.results[1].detail.as_deref(),
            Some("unsupported in test engine")
        );
    }

    #[test]
    fn fixture_loader_reads_repository_format_in_stable_order() {
        let directory =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test-sites/conformance");
        let fixtures = load_fixtures(directory).unwrap();
        assert_eq!(
            fixtures
                .iter()
                .map(|fixture| fixture.id.as_str())
                .collect::<Vec<_>>(),
            vec!["forms-basic", "storage-origin-profile-partitioning"]
        );
    }
}
