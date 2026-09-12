//! Reproduce + verify the CLI live-open vs server engine construction
//! divergence.
//!
//! Run with:
//!   cargo test -p browsai-engine-servo --test divergence -- --nocapture
//!   cargo test -p browsai-engine-servo --test divergence --features servo-runtime -- --nocapture
//!
//! The CLI is a fresh process per invocation and constructs one
//! `ServoRuntime` over its whole lifetime. The pre-fix server
//! constructed a fresh `ServoRuntime` per host, which panics on the
//! second-and-later hosts at `servo-config/opts.rs:279` ("Already
//! initialized"). The fix shares one `ServoEngine` across all hosts
//! (the engine layer already supports multiple `ServoRuntimePage`s
//! inside one `ServoRuntime`); these tests pin both behaviours.

use browsai_engine_api::{BrowserEngine, ContextOptions};
use browsai_engine_servo::ServoEngine;
use url::Url;

fn cli_options(profile_identity: Option<browsai_engine_api::ProfileIdentity>) -> ContextOptions {
    ContextOptions {
        profile: None,
        profile_identity,
        headless: true,
        use_real_browser_runtime: true,
        viewport: Some(browsai_engine_api::VirtualViewport::default()),
        deterministic_clock_millis: None,
        no_raster: true,
        http2_profile: None,
        canvas_noise_seed: None,
    }
}

/// Mirrors `apps/browsai-cli/src/server.rs::get_or_create_domain`.
/// After the refactor this matches the CLI on `no_raster`; the only
/// remaining difference is `deterministic_clock_millis` when running
/// without `--live-browser`.
fn server_options(
    live_browser: bool,
    profile_identity: Option<browsai_engine_api::ProfileIdentity>,
) -> ContextOptions {
    ContextOptions {
        profile: None,
        profile_identity,
        headless: true,
        use_real_browser_runtime: live_browser,
        viewport: Some(browsai_engine_api::VirtualViewport::default()),
        deterministic_clock_millis: if live_browser { None } else { Some(0) },
        no_raster: true,
        http2_profile: None,
        canvas_noise_seed: None,
    }
}

fn diff_options(label_a: &str, a: &ContextOptions, label_b: &str, b: &ContextOptions) {
    println!("\n--- ContextOptions diff ({label_a} vs {label_b}) ---");
    let pairs: [(&str, String, String); 7] = [
        ("headless", a.headless.to_string(), b.headless.to_string()),
        (
            "use_real_browser_runtime",
            a.use_real_browser_runtime.to_string(),
            b.use_real_browser_runtime.to_string(),
        ),
        (
            "no_raster",
            a.no_raster.to_string(),
            b.no_raster.to_string(),
        ),
        (
            "deterministic_clock_millis",
            format!("{:?}", a.deterministic_clock_millis),
            format!("{:?}", b.deterministic_clock_millis),
        ),
        (
            "viewport",
            format!("{:?}", a.viewport),
            format!("{:?}", b.viewport),
        ),
        (
            "profile_identity_present",
            a.profile_identity.is_some().to_string(),
            b.profile_identity.is_some().to_string(),
        ),
        (
            "http2_profile",
            format!("{:?}", a.http2_profile),
            format!("{:?}", b.http2_profile),
        ),
    ];
    let mut diverged = 0;
    for (name, av, bv) in pairs {
        let marker = if av == bv { "==" } else { "!!" };
        if marker == "!!" {
            diverged += 1;
        }
        println!("  [{marker}] {name}: cli={av} server={bv}");
    }
    println!("--- {diverged} diverging field(s) ---");
}

#[test]
fn options_are_now_aligned_when_live() {
    let cli = cli_options(None);
    let server = server_options(true, None);
    diff_options(
        "CLI live-open",
        &cli,
        "server /browse --live-browser",
        &server,
    );
    // After the refactor, both paths set no_raster=true and both run
    // the live runtime when --live-browser is passed. The only
    // remaining field-level difference (deterministic_clock_millis)
    // only matters in the non-live path.
    assert_eq!(cli.no_raster, server.no_raster);
    assert_eq!(
        cli.use_real_browser_runtime,
        server.use_real_browser_runtime
    );
}

#[test]
fn deterministic_construction_uses_independent_state() {
    // Two engines, one mimicking the CLI shape and one the post-fix
    // server shape (both deterministic). The CLI engine runs the live
    // runtime; for this deterministic check we disable it on the CLI
    // copy so we don't need the servo-runtime feature.
    let mut cli_engine = ServoEngine::new();
    let mut cli_opts = cli_options(None);
    cli_opts.use_real_browser_runtime = false;
    let cli_ctx = cli_engine
        .create_context(cli_opts)
        .expect("cli create_context");
    let cli_page = cli_engine.create_page(cli_ctx).expect("cli create_page");

    let mut server_engine = ServoEngine::new();
    let server_ctx = server_engine
        .create_context(server_options(false, None))
        .expect("server create_context");
    let server_page = server_engine
        .create_page(server_ctx)
        .expect("server create_page");

    let stored_cli = cli_engine.context_options(cli_ctx).unwrap().clone();
    let stored_server = server_engine.context_options(server_ctx).unwrap().clone();

    println!("\n=== stored CLI options ===\n{stored_cli:?}");
    println!("\n=== stored server options ===\n{stored_server:?}");

    // Remaining deterministic-mode divergence: server pins the
    // simulated clock to 0 (the legacy JS-renderer replay mode);
    // CLI relies on the wall clock.
    assert_eq!(stored_cli.deterministic_clock_millis, None);
    assert_eq!(stored_server.deterministic_clock_millis, Some(0));

    let nav = cli_engine
        .navigate(cli_page, Url::parse("about:blank").unwrap())
        .unwrap();
    assert_eq!(nav.url.as_str(), "about:blank");
    let nav = server_engine
        .navigate(server_page, Url::parse("about:blank").unwrap())
        .unwrap();
    assert_eq!(nav.url.as_str(), "about:blank");

    let cli_snap = cli_engine.snapshot(cli_page).unwrap();
    let server_snap = server_engine.snapshot(server_page).unwrap();
    println!(
        "\n=== CLI snapshot ===\n  url={}\n  nodes={}",
        cli_snap.url,
        cli_snap.tree.nodes.len()
    );
    println!(
        "\n=== server snapshot ===\n  url={}\n  nodes={}",
        server_snap.url,
        server_snap.tree.nodes.len()
    );

    assert_eq!(cli_snap.tree.nodes.len(), server_snap.tree.nodes.len());
    assert_eq!(cli_snap.url, server_snap.url);
}

#[test]
fn shared_engine_hosts_multiple_independent_pages() {
    // Mirror the post-fix server: one ServoEngine, two hosts, two
    // (ContextId, PageId) pairs, each navigating independently.
    // This is the structural invariant the refactor relies on; if the
    // engine layer ever stops supporting multiple WebViews inside one
    // ServoRuntime, this test fails loudly.
    let mut engine = ServoEngine::new();

    // Deterministic options so this test runs without the live feature.
    let mut opts = server_options(false, None);
    opts.profile_identity = None;

    let ctx_a = engine.create_context(opts.clone()).expect("ctx a");
    let page_a = engine.create_page(ctx_a).expect("page a");
    let ctx_b = engine.create_context(opts.clone()).expect("ctx b");
    let page_b = engine.create_page(ctx_b).expect("page b");

    // PageIds are independent inside one engine (the engine increments
    // next_page for every create_page).
    assert_ne!(page_a, page_b);
    assert_ne!(ctx_a, ctx_b);

    engine
        .navigate(page_a, Url::parse("about:blank").unwrap())
        .unwrap();
    engine
        .navigate(page_b, Url::parse("about:blank").unwrap())
        .unwrap();

    let snap_a = engine.snapshot(page_a).unwrap();
    let snap_b = engine.snapshot(page_b).unwrap();
    assert_eq!(snap_a.url.as_str(), "about:blank");
    assert_eq!(snap_b.url.as_str(), "about:blank");

    // The contexts are independently readable from the same engine.
    let opts_a = engine.context_options(ctx_a).unwrap().clone();
    let opts_b = engine.context_options(ctx_b).unwrap().clone();
    assert_eq!(
        opts_a.use_real_browser_runtime,
        opts_b.use_real_browser_runtime
    );
}

#[cfg(feature = "servo-runtime")]
#[test]
fn shared_engine_hosts_multiple_hosts_in_one_process() {
    // The post-fix invariant under the live runtime: one ServoEngine
    // constructs ServoRuntime exactly once, then creates a separate
    // WebView per host. Cookie isolation between hosts is automatic
    // because Servo's HttpState.cookie_jar is keyed by domain
    // (vendor/servo-net/cookie_storage.rs:54); the test just verifies
    // that two pages can coexist and be navigated without panicking.
    //
    // This must run in a process that has never called
    // ServoRuntime::new() before (i.e. not in the same binary as
    // `legacy_engine_panic.rs`).
    let mut engine = ServoEngine::new();

    let ctx_a = engine.create_context(cli_options(None)).expect("ctx a");
    let page_a = engine.create_page(ctx_a).expect("page a");
    let ctx_b = engine.create_context(cli_options(None)).expect("ctx b");
    let page_b = engine.create_page(ctx_b).expect("page b");

    // Both pages must navigate without triggering the singleton panic.
    engine
        .navigate(page_a, Url::parse("about:blank").unwrap())
        .expect("navigate a");
    engine
        .navigate(page_b, Url::parse("about:blank").unwrap())
        .expect("navigate b");

    let _ = engine.snapshot(page_a).expect("snap a");
    let _ = engine.snapshot(page_b).expect("snap b");
}
