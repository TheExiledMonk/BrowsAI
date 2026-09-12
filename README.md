# BrowsAI

BrowsAI is an AI-native browser architecture. Web content executes in a real browser runtime (Servo); agents consume a first-class **Agent Render Tree** and issue semantic intent that resolves through native browser input.

The repository ships:

- an engine-neutral browser contract (`engine-api`)
- a Servo-backed deterministic engine (`engine-servo`)
- a profile-driven identity layer (`profiles`, `fingerprint`)
- a challenge observer + consent-gated solve pathway (`challenge-observer`, `agent-runtime`)
- a humanized mouse trajectory generator (`input`)
- the `browsai` CLI and a `browsai-headless` host

## Build

```sh
cargo build --workspace
cargo test --workspace
```

Requires Rust 1.78+ and the Servo build dependencies listed in `crates/engine-servo/Cargo.toml`. The `servo-runtime` feature is off by default; enable it with `cargo build --workspace --features browsai-engine-servo/servo-runtime` to embed the real Servo embedder.

## CLI

The `browsai` binary is the primary user surface. Run `browsai` with no arguments to see the full command list.

### Live-open a URL

```sh
browsai live-open https://example.com
```

Boots a Servo-backed context, navigates, projects the Agent Render Tree, and emits a JSON snapshot. The `--click-links`, `--probe-controls`, and `--auto-solve` flags add interaction. See `docs/challenge-handling.md` for the full auto-solve model.

### Live-search

```sh
browsai live-search "rust browser engine" --open-links
```

Runs a search query through the search corpus and optionally clicks the result links.

### Run the compatibility corpus

```sh
browsai check corpus check-sites/sites.csv
browsai check report <run-id>
```

Loads a site corpus, runs each site through the agent tree, and emits a run report.

### Other commands

| Command          | Purpose                                                  |
| ---------------- | -------------------------------------------------------- |
| `version`        | print package + protocol versions                        |
| `capabilities`   | dump `EngineCapabilities`                                |
| `profile <name>` | create a `ProfileManager` entry and print its JSON       |
| `headless <url>` | deterministic no-raster navigation                       |
| `open <url>`     | navigation + projection (alias for `headless`)           |
| `navigate <url>` | same, with `use_real_browser_runtime = true`              |
| `render <url>`   | full Agent Render Tree dump                              |
| `query <url>`    | paged query (`--cursor`, `--limit`)                      |
| `action`         | dispatch a single `NativeInputEvent`                     |
| `audit <path>`   | validate a persisted audit journal                       |
| `logs <path>`    | validate a secret-safe log export                        |
| `replay <path>`  | replay an audit journal                                   |
| `benchmark <path>` | validate a benchmark result document                   |
| `recovery <path>` | validate a recovery checkpoint                          |

## Profiles and identity

BrowsAI exposes a coherent browser identity sourced from a `ProfileConfig`. Five vetted variants ship in `browsai_fingerprint::FingerprintCatalog`:

- `chrome-140-linux-x86_64`
- `firefox-130-linux-x86_64`
- `safari-17-macos-arm64`
- `chrome-android-140-pixel8`
- `safari-ios-17-iphone`

Resolve one and apply it to a profile:

```rust
use browsai_fingerprint::{FingerprintCatalog, FingerprintId};
use browsai_profiles::ProfileManager;

let mut profiles = ProfileManager::default();
let id = profiles.create("work");
let catalog = FingerprintCatalog::default_catalog();
profiles
    .apply_fingerprint(&id, &catalog, &FingerprintId::new("chrome-140-linux-x86_64"))
    .expect("consistent fingerprint");
```

`ProfileManager::try_update_config` rejects `ProfileConfig`s that disagree with their declared `User-Agent` family or platform. The full schema and v1→v2 migration live in [docs/profile-schema.md](docs/profile-schema.md). The end-to-end identity model — including the Servo runtime, the navigator identity script, and the network header derivation — is in [docs/browser-fidelity.md](docs/browser-fidelity.md).

## Challenge handling

`browsai-challenge-observer` watches the network and DOM for human-verification walls (Cloudflare, hCaptcha, reCAPTCHA, Akamai, DataDome, generic "verify you are human"). It is observation-only — it has no `solve_*`, `bypass_*`, or `inject_*` methods. Detected challenges appear in the Agent Render Tree as `SemanticRole::Challenge` nodes and as `ChangeEvent::ChallengeObserved` lifecycle events.

The runtime exposes three authorization sources for solving:

1. **HumanTakeover** — an active `Human`-owned `TakeoverSession`. Interactive mode.
2. **CredentialSolve** — the session also holds the `CredentialSolve` capability. For stored credentials.
3. **Unattended** — `SandboxPolicy::allow_unattended_solve = true`. For autonomous VPS deployments.

The runtime's `solve_observed_challenges` walks a `PageSnapshot`, gates each `Challenge` widget through `attempt_challenge_click_with_trajectory`, and dispatches the resulting events through a closure. Cursor within 16 px of the widget triggers a discrete `PointerDown`/`PointerUp`; further away triggers a non-linear bezier mouse trajectory with overshoot and per-step jitter. The full operational model is in [docs/challenge-handling.md](docs/challenge-handling.md).

### Wire the auto-solve path into a host application

```rust
use browsai_agent_runtime::{
    solve_observed_challenges, AgentRuntime, SandboxPolicy, TakeoverManager,
};
use browsai_sandbox::Capability;

let mut runtime = AgentRuntime::new(SandboxPolicy::autonomous_agent());
let session = runtime.open([Capability::SolveChallenge], Default::default());
runtime.start(session).unwrap();
let takeovers = TakeoverManager::default();

let cursor_origin = random_cursor_origin(viewport, &url, invocation_seed);
let audit_events = solve_observed_challenges(
    page_id,
    &snapshot,
    &mut runtime,
    session,
    &takeovers,
    runtime.now_millis(),
    cursor_origin,
    |event| engine.dispatch_input(page_id, event.clone()),
    |error| log::warn!("auto-solve dispatch failed: {error}"),
)?;
```

Every attempt produces a `SolveAuditEvent` with `capability_used = "SolveChallenge+Unattended"`. Audit events are the only record of solve activity; ship them with your application logs.

## Architecture (short form)

```
URL → engine-servo → AgentRenderTree → semantic-ir → Challenge nodes
                                              ↓
                                  agent-runtime::solve_observed_challenges
                                              ↓
                              BrowserEngine::dispatch_input (per NativeInputEvent)
                                              ↓
                                          audit event
```

The engine-neutral `engine-api` trait is the contract; `engine-servo` is the Servo adapter. `engine-api` carries the resolved `ProfileIdentity`, the page lifecycle, and the snapshot.

## Workspace layout

| Path                       | Purpose                                                       |
| -------------------------- | ------------------------------------------------------------- |
| `crates/engine-api`        | engine-neutral browser contract                              |
| `crates/engine-servo`      | Servo-backed deterministic adapter + `servo-runtime` feature  |
| `crates/profiles`          | profile identity, variants, consistency checks, JSON schema   |
| `crates/fingerprint`       | vetted browser-identity catalog + consistency checks          |
| `crates/challenge-observer`| observation-only detector for human-verification walls        |
| `crates/agent-runtime`     | session/quota gates, takeovers, solve pathway, audit events    |
| `crates/agent-tree`        | agent-side tree, structural + semantic roles, geometry         |
| `crates/semantic-ir`       | semantic-IR passes, challenge summary nodes                    |
| `crates/event-observer`    | bounded change stream                                         |
| `crates/input`             | `NativeInputEvent` + humanized mouse trajectory generator     |
| `crates/state`             | page snapshots, page state                                    |
| `crates/network`           | HTTP lifecycle, header derivation from profile                |
| `apps/browsai-cli`         | `browsai` binary                                              |
| `apps/browsai-headless`   | deterministic no-raster host                                  |

## Development

```sh
cargo test --workspace              # 295+ tests across 49 crates
cargo build -p browsai-cli          # CLI binary
cargo run -p browsai-cli -- version  # sanity check
```

The `agent-runtime` and `challenge-observer` crates host the public solve-pathway API. Host applications that want to auto-solve visible challenges should depend on those two crates plus `browsai-engine-api` for the `PageSnapshot` and `PageId` types.

## License

BrowsAI is source-available.

You may use, modify, evaluate, and deploy BrowsAI for personal,
research, educational, and internal organisational purposes, including
internal use within commercial companies.

A separate commercial licence is required if BrowsAI is embedded in,
distributed with, hosted as part of, or materially powers a commercial
product or paid service offered to third parties.

See [LICENSE](LICENSE) for the full public licence terms and
[LICENSE-COMMERCIAL.md](LICENSE-COMMERCIAL.md) for commercial licensing
information.

This summary is informational. If the wording in this README conflicts
with `LICENSE`, `LICENSE` controls.

Third-party component licences are tracked in
[THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md) and
[docs/licensing.md](docs/licensing.md).

## JSON schema for host integrations

Every CLI response shape and every NDJSON streaming event emitted by
`browsai` is documented in [docs/agent-tree-schema.md](docs/agent-tree-schema.md).
The current schema version is `browsai-agent-tree/1.0`. Every event
emitted in `--stream` mode carries a `schema_version` field so host
integrations (the Helios plugin, IDE integrations, automation daemons)
can pin to a schema version and reject responses that declare a
higher major.

Streaming events include `page-pending`, `page`, `node`, and
`snapshot-complete`; see the schema doc for fields and the example
stream. The deterministic-backend `live-open` and `live-search`
commands honour `--auto-solve`, `--fingerprint=<id>`, and the randomized
initial cursor path; the real-Servo path requires the
`live-browser` Cargo feature.

## HTTP server for LLM control

`browsai serve --port 8765` exposes the same shape over a localhost
HTTP/1.1 socket — bind to `127.0.0.1` by default, `--bind 0.0.0.0`
to expose externally. LLMs and other clients drive the browser with
plain `curl` or any HTTP client:

```sh
curl -s -X POST -H 'Content-Type: application/json' \
    -d '{"url":"https://example.test","query":"release notes"}' \
    http://127.0.0.1:8765/browse | jq
```

Streaming is `?stream=true` (or `"stream": true` in the body) and uses
chunked transfer encoding so the host reads events as they arrive. See
[docs/server.md](docs/server.md) for the route table, systemd unit,
and CLI command list.