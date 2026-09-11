# Challenge Handling

BrowsAI ships a consent-gated humanized mouse trajectory generator and an
observation-only detector for anti-bot challenge walls. This document
catalogues what the engine observes, how the agent tree represents a
challenge, and how a solve path can be authorized with explicit human
consent.

## What the observer detects

`browsai_challenge_observer` walks two streams and produces a
`ChallengeObservation { provider, kind, evidence, confidence, provenance }`.

### Network-side signals (`observe_network`)

| Signal                         | Provider     | Kind              |
| ------------------------------ | ------------ | ----------------- |
| `cf-ray` + `cf-browser-verification` body marker | `cloudflare` | `BrowserChallenge` |
| `cf-ray` with 403/503          | `cloudflare` | `AccessDenied`    |
| `server: akamai` + `_Incapsula_Resource` body | `akamai` | `BrowserChallenge` |
| `server: akamai` with 403      | `akamai`     | `AccessDenied`    |
| `server: cloudflare` + `cf-mitigated` | `cloudflare` | `BrowserChallenge` |
| `server: ddos-guard`           | `ddos-guard` | `BrowserChallenge` |
| `server: data-dome` or body match | `datadome` | `BrowserChallenge` |
| HTTP 429                       | `rate-limit` | `AccessDenied`    |

### DOM-side signals (`observe_document`, `observe_html_body`)

| Signal                                              | Provider               | Kind             |
| --------------------------------------------------- | ---------------------- | ---------------- |
| `class*="cf-browser-verification"` / `cf-wrapper`  | `cloudflare`           | `BrowserChallenge` |
| `<iframe src*="hcaptcha.com">`                      | `hcaptcha`             | `Captcha`        |
| `<iframe src*="recaptcha">`                        | `recaptcha`            | `Captcha`        |
| `<iframe src*="challenges.cloudflare.com">`        | `cloudflare-turnstile` | `Captcha`        |
| `class*="_Incapsula_Resource"`                      | `akamai`               | `BrowserChallenge` |
| Body text contains "verify you are human"          | `generic`              | `Captcha`        |
| Body text contains "two-factor" / "verification code" | `totp`              | `TwoFactorPrompt` |
| Body text contains "account locked" / "suspended"  | `site-policy`          | `AccountLocked`  |
| `<form action="/interstitial/">`                    | `datadome`             | `BrowserChallenge` |

### Confidence model

Confidence is set by the detection rule and is bounded to `[0.0, 1.0]`. The
observer rejects out-of-range confidence with `ChallengeObserverError::InvalidConfidence`.
Confidence is a hint for the agent tree, not a permission.

## Agent-tree representation

`semantic-ir` consumes a `ChallengeObservationSet` via
`with_challenge_observations` and emits a `SemanticRole::Challenge` summary
node for each observation. The planner can detect challenges via:

```rust
let ir = SemanticIr::from_structural(tree)
    .with_challenge_observations(&observations);
if ir.challenge_present() {
    for provider in ir.challenge_providers() {
        // route through the consent-gated solve path
    }
}
```

`ChangeEvent` gains `ChallengeObserved { provider, kind, url }` and
`ChallengeCleared { provider }` so the change stream sees the lifecycle.

## Solve pathway

The runtime exposes three entry points. Each is a **policy gate**: it
authorizes the caller to dispatch a specific input sequence through the
engine, and the audit trail records what was authorized.

### Authorization modes

Every entry point first requires the `SolveChallenge` capability on the
session (and the policy must allow it). Beyond that, **one of** the following
authorization sources must be in effect:

| **B**Authorization source | | **B**Requirement |
| ------------------------ | | ---------------- |
| `HumanTakeover`           | | An active `TakeoverSession` for ` Human`-owned `takeover_id` with `state = Active` and `expires_at_tick > now`. |
| `CredentialSolve`        | | The session also holds the `CredentialSolve` capability. |
| `Unattended`             | | `SandboxPolicy::allow_unattended_solve` is true. Intended for autonomous VPS deployments where a human takeover cannot be staged. The capability check still applies. |

When `Unattended` is used, the audit event records
`capability_used = "SolveChallenge+Unattended"` so the trail distinguishes
attended from unattended attempts.

### `attempt_solve_challenge`

Generic precondition check for satisfying a challenge through any of the
three authorization sources. Returns a `SolveAuditEvent`.

### `attempt_challenge_click`

Authorizes a **single native click** on a challenge widget. Returns a
`SolveAuditEvent` carrying `page_id` and `target_node_id`. The caller
dispatches one `PointerDown` followed by one `PointerUp`. Use this when the
challenge widget is already focused or the page does not require a
humanized mouse path.

### `attempt_challenge_click_with_trajectory`

Authorizes a **humanized mouse move + click** sequence. Returns both the
`SolveAuditEvent` and the full `Vec<NativeInputEvent>` to dispatch. The
trajectory is generated by `browsai_input::generate_human_trajectory` (see
`crates/input/src/lib.rs`):

- Cubic bezier path with two control points offset perpendicular to the
  start→end line.
- Per-step timing follows an acceleration/deceleration profile (slow start,
  fast middle, slow end) with deterministic per-tick jitter.
- Optional small overshoot near the end that settles back to the target,
  plus a short pre-click pause.

```rust
use browsai_input::{generate_human_trajectory, MouseTrajectoryOptions};

let opts = MouseTrajectoryOptions::default();
let (audit, events) = runtime.attempt_challenge_click_with_trajectory(
    session_id,
    page_id,
    Some("challenge:hcaptcha:0".into()),
    "hcaptcha",
    start,
    end,
    takeover_id,        // None if the policy allows unattended solve
    &takeover_manager,
    now,
    opts,
)?;

for event in &events {
    engine.dispatch_input(page_id, event.clone())?;
}
```

The trajectory is opt-in per call: callers can request a single discrete
click via `attempt_challenge_click` instead. Every call still goes through
the same `SolveChallenge` capability check plus the three authorization
sources above, and produces one `SolveAuditEvent`.

## Practical guidance

- For a captcha checkbox, prefer `attempt_challenge_click_with_trajectory`
  once the agent has resolved the checkbox `target_node_id` and the
  geometry from the agent tree. Dispatch each event in the returned
  vector.
- For an `I am not a robot` widget where the cursor is already over the
  checkbox, `attempt_challenge_click` is enough.
- For autonomous VPS deployments, build the runtime with
  `SandboxPolicy::autonomous_agent()` and pass `None` for `takeover_id`.
  The audit trail still records every attempt; the unattended flag is
  surfaced via `capability_used = "SolveChallenge+Unattended"`.
- The `browsai-cli` `live-open` command accepts `--auto-solve` and wires
  this end-to-end: it walks the agent tree for `SemanticRole::Challenge`
  nodes with geometry, calls `attempt_challenge_click_with_trajectory` for
  each, dispatches the trajectory events through the engine, and reports
  the audit events under `auto_solve_audit` in the JSON output. The CLI
  uses `SandboxPolicy::autonomous_agent()` when `--auto-solve` is set.
- Treat `CredentialSolve` as a strictly credentialed path (e.g. a stored
  OAuth token that the page accepts).
- Log `SolveAuditEvent` records; never log the secret material that made
  `CredentialSolve` possible.