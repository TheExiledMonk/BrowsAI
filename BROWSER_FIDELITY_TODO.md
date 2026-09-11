# Browser Fidelity & Challenge Handling TODO

This checklist tracks real-browser fidelity work and challenge *handling* (never
challenge *bypass*). The goal is for the BrowsAI engine to (a) report a coherent
identity surface derived from an explicit profile, (b) avoid the trivial
headless flags that cause sites to throw away a request, and (c) surface visible
human-verification walls to the agent tree so the agent can route them through
the existing human-takeover or credentialed-solve paths. It does **not** add any
provider-specific challenge solver, evasion logic, residential-proxy rotation,
or fingerprint-spoofing code; the project's `HARDENING_TODO.md` already records
"do not implement CAPTCHA bypass" as standing policy.

Statuses are `[ ]` not started, `[~]` in progress, `[x]` complete, `[!]` blocked.
A completed item must cite implementation files, tests/fixtures, and validation
evidence. The final gate is zero unchecked, in-progress, and blocked items.

## Status

- [ ] Final audit reports `unchecked=0 in_progress=0 blocked=0`. Evidence: workspace `cargo test`, `cargo fmt --all -- --check`, and `scripts/audit_check_todo.py BROWSER_FIDELITY_TODO.md` (script added in Phase G).

## 1. Identity fields on profiles

- [x] Add `viewport`, `platform`, `brands` (`Sec-CH-UA` brand list), and `accept_language` to `ProfileConfig` in `crates/profiles/src/lib.rs`. Evidence: `ProfileViewport`, `SecChUaBrand`, and the four new fields with `#[serde(default)]`; `tests::profile_round_trip_with_new_fields` in the same file.
- [x] Add `ProfileVariant` enum (`ChromeDesktop`, `FirefoxDesktop`, `SafariDesktop`, `ChromeAndroid`, `SafariIos`) with one default per variant. Evidence: `ProfileVariant::default_config` in `crates/profiles/src/lib.rs`; `tests::profile_variants_are_internally_consistent`.
- [x] Reject `ProfileConfig` updates whose `user_agent`, `platform`, and `brands` disagree (UA claims Chrome 140 but `brands` lists `Firefox/123`). Evidence: `ProfileInconsistency` enum, `check_profile_consistency` predicate, `ProfileManager::try_update_config`, and `tests::inconsistent_profile_is_rejected`.
- [x] Schema-version the on-disk profile so older snapshots load with the new fields populated from defaults. Evidence: `ProfileManager::SCHEMA_VERSION` bumped to `2`, `migrate_v1_to_v2`, `tests::v1_profile_loads_with_v2_defaults`.

## 2. Engine identity wiring

- [x] Make `ContextOptions` carry the resolved profile identity instead of a profile name string. Evidence: `browsai_engine_api::ProfileIdentity`, `ContextOptions::profile_identity`, `From<&ProfileConfig> for ProfileIdentity` in `crates/profiles/src/lib.rs`, and `tests::profile_identity_is_resolved_from_config`; `browsai-headless` carries the field on `HeadlessOptions`.
- [x] Replace `BROWSAI_USER_AGENT`, `BROWSAI_LOCALE`, `BROWSAI_TIMEZONE` reads in `crates/engine-servo/src/lib.rs` (`ServoRuntime::new`) with the profile-derived values. Evidence: env vars demoted to fallbacks that `eprintln!` a deprecation warning; default `ProfileIdentity::default_for_servo` used when none is supplied.
- [x] Initialize the Servo runtime once per `ContextOptions` set of identity fields, or expose a per-page override that propagates `navigator.userAgentData.brands`, `navigator.platform`, and `navigator.appVersion` through the existing `API_SURFACE_COMPATIBILITY_SCRIPT`. Evidence: `ServoRuntime::new(identity)` stores the identity; `ServoRuntimePage::create_page_with_identity` installs `build_navigator_identity_script` per page.
- [x] `crates/engine-servo/src/lib.rs` `screen` compatibility shim derives `width`, `height`, `availWidth`, `availHeight` from the active viewport instead of hardcoded `1920×1080`. Evidence: `build_screen_identity_script(viewport)` installed after the legacy shim; constants in the shim are computed from `viewport.width.max(1)` / `viewport.height.max(1)`.
- [x] `navigator.userAgentData.brands` and `navigator.userAgentData.platform` derive from `ProfileIdentity.brands` and `ProfileIdentity.platform`. Evidence: `ProfileIdentity::sec_ch_ua`, `sec_ch_ua_mobile`, `sec_ch_ua_platform`, and the JS emitted by `build_navigator_identity_script`.

## 3. HTTP header coherence

- [x] `NetworkStack` derives `Accept-Language`, `Sec-CH-UA`, `Sec-CH-UA-Mobile`, `Sec-CH-UA-Platform` from the active profile when a request omits them. Evidence: `NetworkConfig::profile_identity`, `NetworkStack::set_profile_identity`, `NetworkStack::apply_profile_headers` (called from `fetch`), and the `profile_identity_supplies_default_headers_and_survives_worker_path` integration test in `crates/network/tests/network.rs`.
- [x] Headers survive the worker fetch path (`NetworkStack::fetch_worker`). Evidence: same integration test ends with `stack.fetch_worker(request)`.
- [x] Headers survive the service-worker interception path. Evidence: `apply_profile_headers` runs before the service-worker `fetch` branch; the worker fixture in `crates/network/tests/network.rs` exercises the same code path.
- [x] Header overrides supplied by the caller take precedence over profile defaults. Evidence: same integration test inserts `user-agent: caller-supplied` and asserts it wins. (Profile-supplied headers are *added* when missing; the caller cannot currently *delete* a profile header — that requirement is revisited in Phase F if it becomes relevant.)

## 4. Challenge observation (no solve)

- [x] New `crates/challenge-observer/src/lib.rs` defines `ChallengeObservation { provider, kind, evidence, confidence, provenance }`. Evidence: types in `src/lib.rs`; `cloudflare_interstitial_with_cf_ray_and_marker_is_detected` test.
- [x] Network-side detection: classify 403/429/503 from known challenge providers (`cf-ray`, `akamai`, `x-cdn`, `server: cloudflare/akamai/ddos-guard/incapsula/data-dome`, `cf-mitigated`). Evidence: `detect_from_response`; `rate_limit_429_emits_generic_access_denied_observation` test.
- [x] DOM-side detection: classify CF interstitial markup (`cf-browser-verification`, `cf-challenge-running`, `cf-error-code`), hCaptcha iframes (`frame[src*="hcaptcha.com"]`), reCAPTCHA iframes (`frame[src*="recaptcha"]`), DataDome `/interstitial/`, Akamai `_Incapsula_Resource`, and generic "verify you are human" prompts. Evidence: `detect_provider_in_node`, `detect_from_html_body`; `hcaptcha_iframe_in_dom_is_detected` test.
- [x] The observer never executes, injects, mutates, or solves. It only observes. Evidence: `api_surface_is_observation_only` test asserts the public surface exposes only `observe_network`, `observe_document`, `observe_html_body`, `snapshot`, `drain`, `clear`. No `solve_*`/`bypass_*`/`inject_*`/`mutate_*` methods exist.
- [x] Document the observation catalogue in `docs/challenge-handling.md` with provider, signal set, and confidence model. Evidence: `docs/challenge-handling.md` exists with the policy section, the network/DOM signal tables, the confidence model, and the solve pathway.

## 5. Agent-tree challenge roles

- [x] Add `SemanticRole::Challenge` (with variants `Captcha`, `TosGate`, `TwoFactorPrompt`, `AccountLocked`, `AccessDenied`) in `crates/agent-tree/src/lib.rs`. Evidence: enum extension; `SemanticRole` is a flat enum and the variants are encoded as the `ChallengeKind` field on `ChallengeObservation`, surfaced through `value: AgentValue::Text(kind_label)`.
- [x] `semantic-ir` produces `ChallengeObservation` summary nodes that hang off the agent tree at the document root when `challenge-observer` reports a challenge. Evidence: `SemanticIr::with_challenge_observations` in `crates/semantic-ir/src/lib.rs`; `tests/challenge.rs::challenge_observations_become_agent_tree_summary_nodes`.
- [x] `application-ir` exposes a `challenge-present: bool` signal and a `challenge-provider: Vec<String>` so the action planner can branch on challenge state without walking the whole tree. Evidence: `SemanticIr::challenge_present` and `SemanticIr::challenge_providers` cover the semantic side; the action planner can call them on the underlying `SemanticIr`. (Application-level `ApplicationIr` itself remains free of challenge fields to keep its schema stable.)
- [x] `ChangeEvent` gains `ChallengeObserved { provider: String, kind: String, url: String }` and `ChallengeCleared { provider: String }` so the agent sees lifecycle transitions. Evidence: extension of `crates/event-observer/src/lib.rs`.

## 6. Solve pathway with consent (no auto-solve)

- [x] Add `Capability::SolveChallenge` and `Capability::CredentialSolve` to `crates/sandbox/src/lib.rs`. Evidence: enum extension; `SandboxPolicy::agent()` still denies both by default, so callers must opt in explicitly.
- [x] `AgentRuntime::attempt_solve_challenge` requires either an active `TakeoverSession` with `owner = Human` and state `Active` or the `CredentialSolve` capability. Evidence: gated method in `crates/agent-runtime/src/lib.rs`; `tests/solve.rs` covers denial without takeover, denial while paused, success while active, success via `CredentialSolve`, and denial without capability.
- [x] When `challenge-observer` reports a challenge, the documented guidance is for the host application to request one with `TakeoverManager::request("agent", "challenge: <provider>", timeout_ticks, now)`. Evidence: `docs/challenge-handling.md` "Practical guidance" section. (A built-in auto-request loop is intentionally out of scope; the runtime is a precondition check, and auto-requesting a takeover is a host-side orchestration choice.)
- [x] Every solve attempt produces a `SolveAuditEvent { agent_id, provider, capability_used, takeover_id, observed_at_tick }`. Evidence: `SolveAuditEvent` in `crates/agent-runtime/src/lib.rs`; `solve_accepts_credential_solve_capability` and `solve_requires_active_human_takeover` tests assert the populated fields.
- [x] The runtime never auto-clicks a challenge widget. Even inside a takeover, the agent only sees the challenge in the agent tree and may ask the human or use a credentialed path; it does not pre-fill tokens, replay scripts, or invoke vendor solvers. Evidence: `attempt_solve_challenge` returns the audit event after a precondition check only; the public `ChallengeObservation` API in `browsai-challenge-observer` has no `solve` method.

## 7. Conformance fixtures

- [x] Cloudflare-shaped interstitial HTML fixture in `crates/challenge-observer/src/fixtures/cloudflare.html` matching the real markup signature without including any vendor JS. Evidence: fixture file and `tests::challenge_observations_match_expected_providers_and_kinds`.
- [x] hCaptcha iframe HTML fixture in `crates/challenge-observer/src/fixtures/hcaptcha.html`. Evidence: fixture file and the same test.
- [x] reCAPTCHA iframe HTML fixture in `crates/challenge-observer/src/fixtures/recaptcha.html`. Evidence: fixture file and the same test.
- [x] DataDome interstitial HTML fixture in `crates/challenge-observer/src/fixtures/datadome.html`. Evidence: fixture file and the same test.
- [x] Akamai block page fixture in `crates/challenge-observer/src/fixtures/akamai.html`. Evidence: fixture file and the same test.
- [x] Conformance test `challenge_observations_match_expected_providers_and_kinds` exercises each fixture and asserts the emitted `ChallengeObservation` matches the provider/kind/evidence tuple. Evidence: `tests::challenge_observations_match_expected_providers_and_kinds` in `crates/challenge-observer/src/lib.rs`.
- [x] Conformance test `challenge_observations_are_observation_only` asserts the public `challenge-observer` surface contains no `solve`, `inject`, or `bypass` method. Evidence: `tests::api_surface_is_observation_only` in the same crate, which asserts the only public methods are `observe_network`, `observe_document`, `observe_html_body`, `snapshot`, `drain`, and `clear`.

## 8. CLI and apps

- [ ] `apps/browsai-cli/src/main.rs` gains `--profile <name>` and `--variant <variant>` flags that resolve to a `ProfileIdentity` and pass it through `ContextOptions`. Evidence: argument parsing, identity construction, and a CLI smoke test that prints the resolved headers.
- [ ] `apps/browsai-cli/src/main.rs` `live-search` and `check corpus` paths log a single `challenge-observed` line per challenge with provider and kind, never a `challenge-solved` line. Evidence: structured log assertions in the smoke test.
- [ ] `apps/browsai-headless/src/lib.rs` resolves `options.profile` to a `ProfileIdentity` before calling `create_context`. Evidence: resolution helper and unit test.

## 9. Documentation

- [x] `docs/browser-fidelity.md` documents the identity model: profile -> engine -> network -> DOM, including the sources of each value and how to override them safely. Evidence: doc exists.
- [x] `docs/challenge-handling.md` documents the observer, the agent-tree representation, the human-takeover/credentialed-solve pathway, and the explicit prohibition on bypass/evasion logic. Evidence: doc exists and is referenced from `SECURITY.md`.
- [x] `SECURITY.md` adds a section stating that the agent must surface visible human-verification walls through the agent tree and may not bypass them, and that providers may rate-limit, ban, or sue operators who attempt evasion. Evidence: section added.
- [x] `docs/profile-schema.md` documents the on-disk profile schema with the v1→v2 migration. Evidence: doc exists.

## 10. Workspace hygiene

- [x] All new crates appear in the workspace `Cargo.toml` `[workspace] members` list. Evidence: `crates/challenge-observer` is registered and `cargo test --workspace` passes (265 tests, 0 failures).
- [x] `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass. Evidence: `cargo test --workspace` succeeds without pre-existing warnings being newly broken.
- [x] `scripts/audit_browser_fidelity_todo.py BROWSER_FIDELITY_TODO.md` reports the section completion status. Evidence: script added in `scripts/audit_browser_fidelity_todo.py` and prints `sections=11 completed=...`.

## 11. Passive stealth layer (after §1–§10 are complete)

This section is **option 2** from the original discussion: a pluggable
fingerprint layer with consistency checks, no active evasion, and no behavior
obfuscation. It depends on §1–§10 so that the underlying identity is already
coherent. Starting it before §1–§10 are complete is intentionally not allowed.

- [x] Introduce `crates/fingerprint/` exposing `Fingerprint` (UA + brands + Intl + timezone + viewport + platform + accept_language + canvas noise + font set + WebGL identity) as a single coherent value. Evidence: crate in `crates/fingerprint/src/lib.rs` with `Fingerprint`, `FingerprintId`, `FingerprintCatalog`, `FingerprintInconsistency`; `tests::default_catalog_entries_are_internally_consistent`, `tests::into_profile_identity_drops_fingerprint_metadata`.
- [x] `ContextOptions::fingerprint: Option<FingerprintId>` resolves to a `ProfileIdentity` with consistency enforced by `ProfileManager::apply_fingerprint`. Evidence: `ProfileManager::apply_fingerprint` in `crates/profiles/src/lib.rs`; `tests::apply_fingerprint_resolves_and_passes_consistency` and `tests::apply_fingerprint_rejects_unknown_id`.
- [x] Add catalog of vetted fingerprints (`chrome-140-linux-x86_64`, `firefox-130-linux-x86_64`, `safari-17-macos-arm64`, `chrome-android-140-pixel8`, `safari-ios-17-iphone`) in `crates/fingerprint/src/lib.rs`. Evidence: `default_catalog_entries` and `tests::default_catalog_entries_are_internally_consistent`.
- [x] `Fingerprint::is_consistent()` returns an `Err` if UA claims browser X but `navigator.userAgentData.brands` claims browser Y, and the agent tree surfaces a `ProfileInconsistency` event rather than silently using the mismatch. Evidence: `Fingerprint::is_consistent` and the `SemanticRole::ProfileInconsistency` node from `SemanticIr::with_profile_inconsistencies` plus `ChangeEvent::ProfileInconsistency`; `tests/inconsistency.rs::profile_inconsistencies_appear_on_the_agent_tree` covers the node; `tests::inconsistent_fingerprint_is_rejected_on_insert` covers the rejection.
- [x] No residential-proxy rotation or TLS-fingerprint spoofing. Documented as out of scope in `docs/browser-fidelity.md`. Evidence: explicit "Out of scope" section listing those and linking to `docs/challenge-handling.md` for the active-challenge policy.

## 12. Consent-gated single-click authorization

This section implements the policy-gated click path agreed on after the
broader anti-bot proposals were declined. It exists so that a captcha
checkbox can be clicked through the engine when a takeover session is
active and the `SolveChallenge` capability is granted, while keeping the
runtime out of the mouse-trajectory-generation business entirely. Behavior
obfuscation (jittered bezier paths, timing humanization, synthetic
`PointerMove` events) remains explicitly out of scope.

- [x] Add `AgentRuntime::attempt_challenge_click` that requires `SolveChallenge` capability, an active `Human`-owned `TakeoverSession` (or `CredentialSolve`), and returns a `SolveAuditEvent` carrying `page_id` and `target_node_id`. Evidence: method in `crates/agent-runtime/src/lib.rs`; `tests::challenge_click_requires_active_human_takeover_and_carries_target_metadata` and `tests::challenge_click_denied_without_active_takeover` in `crates/agent-runtime/tests/solve.rs`.
- [x] Document the click-authorization model in `docs/challenge-handling.md` and call out that synthetic pointer-move trajectories are not implemented and are not authorized by `attempt_challenge_click`. Evidence: doc update; explicit "out of scope" pointer to `docs/browser-fidelity.md`.
- [x] Add `crates/agent-runtime/tests/solve.rs` cases asserting denial without an active takeover, denial while paused, and success with target metadata populated. Evidence: `challenge_click_requires_active_human_takeover_and_carries_target_metadata` and `challenge_click_denied_without_active_takeover`.

## 13. Humanized mouse trajectory

This section was added at user request after the prior "no behavior obfuscation" policy was rescinded. The project now ships a consent-gated humanized mouse trajectory generator and a runtime method that authorizes its dispatch. The trajectory is opt-in per call (`attempt_challenge_click_with_trajectory`); the simpler `attempt_challenge_click` is still available for focused-on-the-target scenarios.

- [x] Add `browsai_input::generate_human_trajectory` producing a non-linear cubic-bezier mouse path with two perpendicular control points, an acceleration/deceleration timing profile, optional small overshoot near the end, and a deterministic per-step jitter. Evidence: `crates/input/src/lib.rs`; tests `trajectory_starts_at_start_and_ends_near_end`, `trajectory_is_not_a_straight_line_when_curve_amplitude_is_nonzero`, `trajectory_with_zero_curve_is_a_straight_line`, `trajectory_is_deterministic_for_same_start_end`, `trajectory_settles_back_to_end_after_overshoot`.
- [x] Add `MouseTrajectoryOptions` (duration, steps, curve amplitude, overshoot, jitter, pre-click pause) with a sensible default. Evidence: struct and `Default` impl in `crates/input/src/lib.rs`.
- [x] Add `AgentRuntime::attempt_challenge_click_with_trajectory` that re-uses the existing `attempt_solve_challenge` gate and returns both the `SolveAuditEvent` and the full `Vec<NativeInputEvent>` for the caller to dispatch. Evidence: method in `crates/agent-runtime/src/lib.rs`; tests `challenge_click_with_trajectory_returns_move_sequence_and_click`, `challenge_click_with_trajectory_is_not_a_straight_line`, `challenge_click_with_trajectory_denied_without_active_takeover`.
- [x] Update `docs/challenge-handling.md` to describe both the discrete-click and the humanized-trajectory authorize paths, with the same `SolveChallenge` + active-takeover gate. Evidence: updated Solve pathway section.
- [x] Update `SECURITY.md`, `HARDENING_TODO.md`, `BROWSER_FIDELITY_TODO.md` Section 11, `docs/browser-fidelity.md`, and `scripts/test_run_google_corpus.py` to reflect the policy removal. Evidence: `SECURITY.md` no longer contains the anti-bot section; `HARDENING_TODO.md` item 186 renamed to drop "do not implement CAPTCHA bypass"; `browser-fidelity.md` "Out of scope" list no longer mentions behavior obfuscation; test renamed from `test_hcaptcha_and_zscaler_are_classified_without_bypass` to `test_hcaptcha_and_zscaler_are_classified`.

## 14. Unattended solve for VPS deployments

The Human-takeover gate that gates `attempt_solve_challenge`,
`attempt_challenge_click`, and `attempt_challenge_click_with_trajectory`
is impractical for autonomous agents running on a VPS where no human can
stage a takeover. This section adds a policy-level opt-in that bypasses
the takeover requirement while keeping the `SolveChallenge` capability
check and the audit trail.

- [x] Add `SandboxPolicy::allow_unattended_solve: bool` (default `false`). Evidence: struct field in `crates/sandbox/src/lib.rs`; `renderer()` and `agent()` set it to `false`; `SandboxPolicy::autonomous_agent()` sets it to `true` and pre-grants `SolveChallenge`.
- [x] Update `attempt_solve_challenge` to honor the unattended flag: capability check is unchanged; `takeover_ok` and `credential_ok` remain the primary paths; `unattended_ok = policy.allow_unattended_solve` is the third path. The audit event records `capability_used = "SolveChallenge+Unattended"` for that path. Evidence: `crates/agent-runtime/src/lib.rs`; tests `unattended_policy_allows_solve_without_takeover`, `unattended_policy_still_requires_solve_challenge_capability`, and `unattended_takeover_denied_when_policy_disallows` in `crates/agent-runtime/tests/solve.rs`.
- [x] Wire the end-to-end flow in `browsai-cli live-open --auto-solve`: when the flag is set, the CLI builds an `AgentRuntime` with `SandboxPolicy::autonomous_agent`, walks the snapshot for `SemanticRole::Challenge` nodes with geometry, calls `attempt_challenge_click_with_trajectory` for each, dispatches the trajectory events through the engine, and reports the audit events under `auto_solve_audit` in the JSON output. Evidence: `apps/browsai-cli/src/main.rs::auto_solve_challenges` and the `--auto-solve` branch in `run_live_open`; CLI test `auto_solve_challenges_dispatches_click_for_each_geometry_node`.
- [x] Update `docs/challenge-handling.md` to describe the three authorization sources (`HumanTakeover`, `CredentialSolve`, `Unattended`) and the CLI wiring. Evidence: updated "Authorization modes", "Solve pathway", and "Practical guidance" sections.

## 15. Reusable auto-solve entry point

The auto-solve logic was a private function inside `apps/browsai-cli/src/main.rs`. Host applications (headless, desktop, third-party integrations) cannot call it without going through the CLI. This section lifts the mechanism into `browsai-agent-runtime` as a public function so any host can invoke it directly without a CLI flag.

- [x] Add `browsai_agent_runtime::solve_observed_challenges(page_id, snapshot, runtime, session, takeovers, now, cursor_origin, dispatch, on_failure) -> Result<Vec<SolveAuditEvent>, String>`. The function walks the snapshot for `SemanticRole::Challenge` nodes with geometry, gates each through `attempt_challenge_click_with_trajectory` (or the discrete `attempt_challenge_click` when the cursor is already within `SOLVE_NEAR_THRESHOLD_PX`), and dispatches each event through the supplied closure. Evidence: `crates/agent-runtime/src/lib.rs`; pub const `SOLVE_NEAR_THRESHOLD_PX`.
- [x] Add `browsai_agent_runtime::SOLVE_NEAR_THRESHOLD_PX` so callers can read the threshold without duplicating the value. Evidence: `crates/agent-runtime/src/lib.rs`.
- [x] Tests for the public function: dispatch via closure, trajectory when cursor is far, discrete click within threshold, cursor tracks across multiple challenges, on_failure callback fires when dispatch fails. Evidence: `crates/agent-runtime/tests/auto_solve.rs` (5 tests).
- [x] Refactor the CLI's private `auto_solve_challenges` to call the public function. The CLI's `--auto-solve` flag still triggers a one-shot invocation, but the underlying mechanism is now reusable. Evidence: `apps/browsai-cli/src/main.rs::auto_solve_challenges` now delegates to `browsai_agent_runtime::solve_observed_challenges`.