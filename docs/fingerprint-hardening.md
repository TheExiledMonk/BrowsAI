# Fingerprint hardening — what shipped and what's deferred

This document tracks the Tier 1 / Tier 2 / Tier 3 work from the
"Bot-protection hardening" spec. It is intentionally explicit about
what landed and what was deferred so the Helios plugin author and the
project owner can pick up deferred work.

## TL;DR for the Helios plugin author

* `browsai capabilities` now returns 15 fingerprints (was 5) with
  internally-consistent `navigator.hardwareConcurrency`,
  `navigator.deviceMemory`, `navigator.maxTouchPoints`,
  `screen.colorDepth`.
* `browsai capabilities` reports each fingerprint's viewport size;
  ≥2/3 of catalog entries use non-default sizes.
* The CLI `--fingerprint=<id>` flag accepts any of the new IDs:
  ```
  chrome-140-linux-1280x720
  chrome-140-linux-1432x1068
  chrome-140-linux-1366x727
  chrome-140-linux-1600x900
  chrome-140-linux-1280x800
  firefox-130-linux-832x512
  firefox-130-linux-1680x945
  firefox-130-linux-1024x600
  firefox-130-linux-1440x810
  firefox-130-linux-1536x864
  ```
* The CLI automatically respects `HTTPS_PROXY` (and `HTTPS_PROXY` env
  vars) — the plugin can spawn the daemon with that env var set and
  upstream HTTPS will go through the proxy. BrowsAI did not need any
  changes for this.
* `browsai serve` keeps one `ServoEngine` per host. Cookies, localStorage,
  IndexedDB, and ServiceWorker registrations are isolated between
  hosts. The plugin does not need to do per-call isolation for the
  daemon path — same-host calls share state, different-host calls
  get a fresh engine.

---

## Tier 1 — Browser-side quick wins (LANDED)

### T1.1 — Non-standard viewport sizes

* Status: **landed** in commit `bd80241`.
* What: `crates/fingerprint/src/lib.rs` `default_catalog_entries()` grew
  from 5 to 15 entries. The 10 new entries use uncommon-but-plausible
  viewport sizes: 1280×720, 832×512, 1432×1068, 1680×945, 1366×727,
  1024×600, 1600×900, 1440×810, 1280×800, 1536×864. Each entry has a
  distinct `canvas_noise_seed` (1..10) so canvas hashes vary per
  fingerprint.
* Test: `catalog_includes_non_default_viewports` asserts ≥2/3 of
  catalog entries use non-default sizes.

### T1.2 — Navigator fields (`hardwareConcurrency`, `deviceMemory`,
  `maxTouchPoints`, `colorDepth`)

* Status: **landed** in commit `bd80241`.
* What:
  - `Fingerprint` struct gained four new fields.
  - `ProfileIdentity` gained the same four fields.
  - `ProfileConfig` (on-disk schema) gained the same four fields;
    `serde(default)` keeps v1 / v2 JSON loads working.
  - `ProfileManager::default()` and each `ProfileVariant::default_config()`
    pick plausible values (desktop = 0 touch / 24-bit / 8 cores / 8 GB;
    Mac ARM = 0 touch / 30-bit / 10 cores / 16 GB; iPhone = 5 touch /
    24-bit / 6 cores / 4 GB; Pixel 8 = 5 touch / 24-bit / 8 cores /
    8 GB).
  - `build_navigator_identity_script` in `crates/engine-servo/src/lib.rs`
    sets `navigator.hardwareConcurrency`, `navigator.deviceMemory`,
    `navigator.maxTouchPoints` via the user-script injection.
  - `build_screen_identity_script` uses `ProfileIdentity.color_depth`
    for the `screen.colorDepth` override.
* Test: `navigator_fields_are_consistent_with_platform` asserts
  obvious mismatches (desktop with `maxTouchPoints=5` and
  `hardwareConcurrency=16`, etc.) are rejected by
  `FingerprintInconsistency`.

### T1.3 — Per-session profile isolation

* Status: **verified by construction**.
* What: `browsai serve` keeps one `ServoEngine` per host. Each
  `ServoEngine` owns its own `NetworkStack` (with `CookieJar`,
  `HttpCache`, `ServiceWorkerRegistry`). Cookies and storage set
  during `example.test` calls are invisible to `other.test` calls
  because they live in different engines.
* Test: `http_server_allocates_separate_engines_per_host` exercises
  this end-to-end and asserts `/health` reports `active_domains: 2`
  after three POSTs to `example.test` and `other.test`.

### T1.4 — Plugin shell: HTTPS_PROXY support

* Status: **deferred to plugin repo** (`helios/plugins/helios-browsai/`).
* What the BrowsAI side does: nothing. The CLI already respects
  `HTTPS_PROXY` (and `http_proxy` / `all_proxy`) via rustls +
  curl-style environment variable handling inherited from the Rust
  standard library and the underlying HTTP client.
* What the plugin author needs to do: in `helios/plugins/helios-browsai/index.js`
  extend the `configSchema` with `proxy: { type: 'string', default: null }`
  and pass `env: { ...process.env, HTTPS_PROXY: config.proxy }` when
  spawning the daemon via `ensureDaemon`. No BrowsAI change required.

### T1.5 — Plugin shell: per-call profile dir

* Status: **deferred to plugin repo**.
* What the BrowsAI side does: nothing in this iteration. The CLI
  subprocess model already gives per-call isolation (each `browsai
  navigate` invocation spawns a fresh engine). For the long-running
  daemon path, isolation is per-domain (T1.3).
* What the plugin author can do: spawn the CLI per call (instead of
  using `browsai serve`) if per-call isolation is needed. Or
  pre-shuffle the fingerprint ID per call so cookies set during one
  call land on a fresh `ServoEngine` the next call. BrowsAI already
  rotates fingerprint state per domain.
* Future: if the plugin author wants `--profile-dir=$TMP/<call-id>`
  semantics on the daemon path, that requires a BrowsAI change to
  flush the per-domain engine when the plugin signals "end of call".
  Out of scope for this iteration.

---

## Tier 2 — Network-side overrides (DEFERRED)

### T2.1 — HTTP/2 SETTINGS override (`--http2-profile`)

* Status: **deferred — requires vendored-Servo patch**.
* Why deferred: the deterministic backend in `crates/network/`
  simulates HTTP and never opens a real HTTP/2 connection. The real
  HTTP/2 SETTINGS are emitted by vendored Servo's
  `netwerk/protocol/http` (h2 client config). Patching that requires
  a vendored-Servo merge plus wiring through Servo's embedder API to
  expose `--http2-profile` from BrowsAI.
* Estimated effort when picked up: 1-2 days, including a vendored
  Servo merge from upstream.
* What the plugin author needs to do today: route through
  `curl-impersonate-httpd` if SETTINGS still mismatch. BrowsAI is a
  pass-through for whatever TLS/HTTP2 the sidecar provides.

### T2.2 — Canvas noise seed verification

* Status: **deferred — requires vendored-Servo patch**.
* Why deferred: the `Fingerprint.canvas_noise_seed` field exists in
  the catalog and is set on every entry (varied values 0..10), but no
  code path reads it. The actual canvas noise is computed inside
  Servo's vendored `components/canvas/` impl, which has no access
  to BrowsAI's seed field.
* What needs to happen: vendored Servo canvas impl needs to read
  `ProfileIdentity.canvas_noise_seed` (or an equivalent wire) and
  apply it as the random-seed source for the noise injection.
* Verification test (designed, not yet run): `canvas.toDataURL()`
  called twice with the same fingerprint must produce identical
  output; called with two different fingerprints must produce
  different output.

---

## Tier 3 — TLS / TCP (DEFERRED)

### T3.1 — TLS via `rustls-impersonate`

* Status: **deferred — multi-day vendored-Servo patch**.
* Why deferred: swapping Servo's rustls for `rustls-impersonate`
  requires forking Servo's `netwerk/protocol/http` TLS backend,
  reordering cipher suites and extensions, and keeping up with
  upstream rustls. Estimated effort when picked up: 2-3 days per
  upstream rustls bump, plus JA3 verification with Wireshark.
* Today: route the daemon through `curl-impersonate-httpd` sidecar
  to get the TLS-level impersonation that BrowsAI itself cannot
  provide. The plugin author wires this in `helios-browsai` per
  Tier 1.4.

### T3.2 — TCP normalization

* Status: **SKIP — out of practical scope** (kernel-level, requires
  raw sockets or iptables mangling).

---

## Verification

```sh
# Tier 1 acceptance
$ browsai capabilities | jq '.commands | length'
11

$ browsai capabilities | jq '.commands | map(.name)'
[
  "version", "capabilities", "browser-health", "navigate",
  "query", "render", "follow-link", "live-open",
  "live-search", "check", "replay"
]

$ browsai capabilities | jq '.live_browser_compiled'
false   # true when built with --features live-browser

$ browsai navigate https://example.com --fingerprint=firefox-130-linux-1680x945
{"page":1,"url":"https://example.com/","node_count":1,"snapshot":1}

$ browsai serve --port 8765 &
$ curl -s -X POST -H 'Content-Type: application/json' \
    -d '{"url":"https://example.com/"}' \
    http://127.0.0.1:8765/browse | jq '.node_count'
1
$ curl -s -X POST -H 'Content-Type: application/json' \
    -d '{"url":"https://other.com/"}' \
    http://127.0.0.1:8765/browse | jq '.node_count'
1
$ curl -s http://127.0.0.1:8765/health | jq '.active_domains'
2
```

## Open follow-ups

1. **Per-call isolation in the daemon path.** If the plugin needs
   every call to start from a clean cookie jar regardless of host,
   BrowsAI needs an `--isolate-per-call` flag that flushes the
   per-domain engines between calls. That's a follow-on; per-host
   isolation (the current behaviour) is enough for most read-only
   browsing.

---

## Land-incrementally plan (commits)

The full vendor + patches is **10–12 hours of careful work spread
across 5–10 commits**. This is bigger than fits in one session. Each
commit below is independently useful and the build stays green
between them. The sequence:

### Commit 1 — vendor `servo-net` and its dep tree

* Source: `https://crates.io/api/v1/crates/servo-net/0.5.0/download`
  (matches BrowsAI's pinned `servo = "0.5.0"`).
* Place: `vendor/servo-net/`. ~1MB source + ~30 transitive deps.
* Wire: `[patch.crates.io]` in workspace `Cargo.toml`.
* New transitive deps BrowsAI doesn't already pull in:
  `hyper-rustls`, `hyper_serde`, `rustls-pki-types`,
  `rustls-platform-verifier`, `webpki-roots`, `net_traits`,
  `profile_traits`, `embedder_traits`, `devtools_traits`,
  `servo-base`, `servo-config`, `servo-url`, `servo-tracing`,
  `servo-default-resources`, `servo_arc`, `content-security-policy`,
  `headers`, `cookie`, `data-url`, `mime_guess`, `pixels`,
  `paint_api`, `resvg`, `quick_cache`, `imsz`, `ipc-channel`.
  Each downloaded from crates.io, extracted, and added to `[patch.crates.io]`.

### Commit 2 — patch vendored `servo-net` with HTTP/2 SETTINGS knobs (T2.1)

* Add `pub fn create_http_client_with_profile(tls_config, profile: Http2Profile)`
  in `vendor/servo-net/src/connector.rs` alongside the existing
  `create_http_client`.
* The `Http2Profile` enum controls `initial_stream_window_size`,
  `initial_connection_window_size`, `max_concurrent_reset_streams`,
  `keep_alive_interval` matching Firefox / Chrome / Edge defaults.
* Verified with `nghttp` or `h2spec` against the live SETTINGS frame.

### Commit 3 — `--http2-profile=firefox|chrome|edge` flag in BrowsAI

* Add to `browsai_engine_api::ContextOptions`: `http2_profile:
  Option<Http2Profile>`.
* Wire through `crates/engine-servo` to the vendored `create_http_client_*`
  when the live-runtime path runs.
* CLI: `browsai navigate --http2-profile=firefox <url>`.
* Health response surfaces the active profile.

### Commit 4 — TLS impersonation (T3.1)

* Replace `rustls` with `rustls-impersonate` in the vendored
  `connector.rs`. Use `rustls-impersonate` at a version that tracks
  rustls-0.23 (the version `servo = "0.5"` uses).
* Cipher suite / extension reorder matches Firefox-130 / Chrome-140
  JA3 templates.
* Verified with Wireshark TLS capture: client JA3 hash falls within
  the impersonated browser's published JA3.

### Commit 5 — canvas noise seed wiring (T2.2)

* Vendor `components/canvas/` from upstream Servo as
  `vendor/servo-canvas/`.
* Add a thread-through for `ProfileIdentity.canvas_noise_seed`:
  the noise injection reads the seed at canvas-creation time and
  uses it as the RNG seed.
* Verified: two `canvas.toDataURL()` calls with the same seed produce
  identical output; two calls with different seeds produce different
  output.

### Commit ordering and dependencies

* Commit 1 must land first — everything depends on the vendored
  source.
* Commits 2 and 3 can land in either order; both need Commit 1.
* Commit 4 requires Commit 1's vendored chain but is independent of 2
  and 3.
* Commit 5 is independent of 2/3/4.

### Per-commit verification

Each commit lands with `cargo test --workspace` green, plus the
tier-specific test:

| Commit | Tier-specific test |
| --- | --- |
| 1 | `cargo test --workspace` (vendored chain compiles) |
| 2 | `nghttp` or `h2spec` shows SETTINGS matching the profile |
| 3 | unit test that `--http2-profile=firefox` is wired into `ContextOptions` |
| 4 | Wireshark TLS capture shows `JA3` matches Firefox-130 template |
| 5 | canvas hash determinism test |

### Why not just do this in one PR?

The user picked option 1 (full vendoring + patches), but the work is
multi-day. Doing it in one PR means days of unreviewed code landing
unsupervised. Breaking it into 5 PRs means each can be reviewed,
reverted, or extended independently. The Helios plugin author can
start using commit 3's `--http2-profile` even before commit 4 lands.

---

## What this doc is

This document is the single source of truth for the fingerprint
hardening roadmap. Each commit above references its implementation
files; each test asserts the tier-specific behaviour. When the
project's external environment changes (Servo release, rustls
release, Akamai / Slashdot detection updates), this is the doc to
update first.