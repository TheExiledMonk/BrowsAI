//! Reproduces the legacy multi-engine panic that the shared-engine
//! refactor in `apps/browsai-cli/src/server.rs` fixes.
//!
//! Run with the live feature in its own process so the `servo::Opts`
//! global it poisons does not affect other integration tests:
//!   cargo test -p browsai-engine-servo --test legacy_engine_panic \
//!     --features servo-runtime -- --nocapture
//!
//! The pre-fix server constructed a fresh `ServoEngine` (and therefore
//! a fresh `ServoRuntime`) for every host. The first host gets the
//! live runtime; the second host's `ServoRuntime::new()` panics at
//! `servo-config/opts.rs:279` ("Already initialized"). The engine
//! layer catches that panic and converts it to an
//! `EngineError::Other("...Already initialized...")`, which is what the
//! server's `get_or_create_domain` propagates back as HTTP 500. The CLI
//! never hit this because each invocation is a fresh process.
//!
//! Note: the `catch_unwind` is inside `engine-servo`, not around
//! `ServoRuntime::new()` itself. By the time the test sees anything,
//! the panic has already been turned into an `Err`, so we assert on
//! the error message rather than expecting a fresh panic to escape.

#![cfg(feature = "servo-runtime")]

use browsai_engine_api::{BrowserEngine, ContextOptions};
use browsai_engine_servo::ServoEngine;

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
fn second_engine_create_context_returns_already_initialized_error() {
    let mut first = ServoEngine::new();
    let ctx_one = first
        .create_context(cli_options())
        .expect("first create_context");
    let _ = first.create_page(ctx_one).expect("first page");

    let mut second = ServoEngine::new();
    let err = second
        .create_context(cli_options())
        .expect_err("second create_context must surface the singleton panic as Err");

    let detail = format!("{err:?}");
    println!(
        "\nsecond ServoEngine::create_context returned error as expected: {detail}\n\
         → this is the HTTP 500 the pre-fix server returned for every new host \
         (docs/server.md:226-231)"
    );

    assert!(
        detail.to_lowercase().contains("already initialized")
            || detail.to_lowercase().contains("already")
                && detail.to_lowercase().contains("initialized"),
        "error message did not look like the documented Servo singleton failure: {detail}"
    );
}
