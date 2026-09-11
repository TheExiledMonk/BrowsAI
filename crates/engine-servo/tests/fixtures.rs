use browsai_engine_api::{BrowserEngine, ContextOptions, EngineFeature};
use browsai_engine_servo::ServoEngine;
use serde::Deserialize;
use std::path::Path;
use url::Url;

#[derive(Deserialize)]
struct CompatibilityManifest {
    schema_version: u16,
    fixtures: Vec<Fixture>,
}

#[derive(Deserialize)]
struct Fixture {
    path: String,
    feature: String,
}

#[test]
fn engine_compatibility_manifest_is_present_and_executable() {
    let manifest: CompatibilityManifest = serde_json::from_str(include_str!(
        "../../../test-sites/engine/compatibility.json"
    ))
    .unwrap();
    assert_eq!(manifest.schema_version, 1);
    assert!(!manifest.fixtures.is_empty());

    let mut engine = ServoEngine::new();
    let capabilities = engine.capabilities();
    let context = engine
        .create_context(ContextOptions {
            headless: true,
            no_raster: true,
            deterministic_clock_millis: Some(0),
            ..Default::default()
        })
        .unwrap();
    let page = engine.create_page(context).unwrap();
    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test-sites");
    for fixture in manifest.fixtures {
        assert!(fixture_root.join(&fixture.path).is_file());
        let required = match fixture.feature.as_str() {
            "navigation" => EngineFeature::Navigation,
            "snapshots" => EngineFeature::Snapshots,
            "page_evaluation" => EngineFeature::PageEvaluation,
            other => panic!("unknown engine feature in compatibility manifest: {other}"),
        };
        assert!(capabilities.supports(required));
        let route = format!("https://example.test/fixtures/{}", fixture.path);
        let navigation = engine.navigate(page, Url::parse(&route).unwrap()).unwrap();
        assert_eq!(navigation.url.as_str(), route);
        assert_eq!(
            engine.snapshot(page).unwrap().url,
            Url::parse(&route).unwrap()
        );
    }
}
