//! Regression guard for the HTTP/2 keepalive timer wiring in
//! `vendor/servo-net/connector.rs::create_http_client_with_settings`.
//!
//! Run in its own process so the live-runtime singleton Servo
//! constructs on first `create_context` doesn't poison sibling tests
//! in in `legacy_engine_panic.rs` (which depends on the second
//! `ServoRuntime::new()` actually panicking):
//!   cargo test -p browsai-engine-servo --test keepalive_timer \
//!     --features servo-runtime
//!
//! Before `hyper_util::rt::TokioTimer` was wired into the
//! `hyper_util::client::legacy::Client` builder, h2's `KeepAlive::new`
//! panicked the moment an HTTP/2 connection was established because
//! `Time::Empty.sleep(...)` was called with no timer configured
//! (`hyper-1.11.1/src/common/time.rs:36-37`). The server ships with
//! `BROWSAI_HTTP2_PROFILE=firefox-130` by default, which sets
//! `keep_alive_interval: Some(Duration::from_secs(45))` in
//! `FIREFOX_130`, so every navigation that opened an HTTP/2 connection
//! panicked before the keepalive wiring landed.
//!
//! This test deliberately uses `about:blank` so it runs offline; the
//! assertion is that `ServoRuntime::new` (and the resulting
//! hyper-util client construction) survives the keepalive wiring
//! rather than that an actual HTTP/2 connection completes. The
//! `browsai serve --live-browser` integration is what exercises the
//! full keepalive-PING path (see the 70-second probe in the
//! regression-timeline commit message).

#![cfg(feature = "servo-runtime")]

use browsai_engine_api::{BrowserEngine, ContextOptions};
use browsai_engine_servo::ServoEngine;
use url::Url;

fn cli_options() -> ContextOptions {
    ContextOptions {
        profile: None,
        profile_identity: None,
        headless: true,
        use_real_browser_runtime: true,
        viewport: Some(browsai_engine_api::VirtualViewport::default()),
        deterministic_clock_millis: None,
        no_raster: true,
        http2_profile: None,
        canvas_noise_seed: None,
    }
}

#[test]
fn live_runtime_construction_does_not_panic_when_http2_keepalive_is_set() {
    std::env::set_var("BROWSAI_HTTP2_PROFILE", "firefox-130");

    let mut engine = ServoEngine::new();
    let ctx = engine
        .create_context(cli_options())
        .expect("create_context under firefox-130 keepalive config");
    let page = engine.create_page(ctx).expect("create_page");
    engine
        .navigate(page, Url::parse("about:blank").unwrap())
        .expect("navigate should not panic on keepalive wiring");
    let _ = engine.snapshot(page).expect("snapshot");
}