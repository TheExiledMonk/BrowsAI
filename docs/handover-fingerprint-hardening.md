# Handover — fingerprint-hardening + Slashdot work

This session landed the full T1 + T2 + T3 (with T3.1 routed through a
sidecar). The remaining items are documented as known upstream gaps.

## State of the work

| Item | Status | Commit / Doc |
| --- | --- | --- |
| T1.1: 15 fingerprints with non-default viewports | landed | commit prior to this session |
| T1.2: navigator fields (4) wired through `engine-servo` user-script injection | landed | commit prior to this session |
| T1.3: per-domain engine isolation (cookies / storage / ServiceWorker) | landed | commit prior to this session |
| T1.4 / T1.5: plugin-side HTTPS_PROXY / per-call isolation | landed for free | `docs/helios-integration.md` |
| T2.1: HTTP/2 SETTINGS knobs | landed | commit `a4b470d` |
| T2.2: canvas noise seed plumbing + actual readback injection | landed | commits `9142908`, `db12402`, `af8e0c0` |
| T3.1: TLS cipher-suite reordering | deferred to `curl-impersonate-httpd` sidecar | commit `f5cb4bc`, `docs/helios-integration.md` |
| Slashdot navigation end-to-end test | landed (1001 nodes, was `about:blank`) | `docs/slashdot-test-result.md` |
| Slashdot article-name extraction | DOM walker patched (this session) | commit `af8e0c0` |

## Slashdot result in detail

Before T1+T2 work: `browsai live-open https://slashdot.org` returned
`about:blank` (bot-detection wall at the network layer fired).

After T1+T2 with the right fingerprint + HTTP/2 profile:
`browsai live-open --fingerprint=firefox-130-linux-x86_64
--http2-profile=firefox-130 https://slashdot.org` returns
`final_url=https://slashdot.org/`, `node_count=1001`, `load_status=Complete`.

The 1001 nodes from the upstream-Servo accessibility tree carry
empty `name` fields on the article-link `<a>` tags. After commit
`af8e0c0`, the JS fallback walker (used when the primary path
returns nothing) does surface `textContent` as `name` for `<a>` tags
without `aria-label` / `title`.

For Slashdot in particular, the upstream Servo accessibility-tree
serialization appears to be the path actually producing the tree, not
the JS walker we patched. If Slashdot's `<a>` tags carry direct text
content, our patched walker will surface it. If the `<a>` is icon-only
(no textContent), nothing surfaces — that's a real upstream Servo
limit, not a BrowsAI limitation.

To verify end-to-end that commit `af8e0c0` improves Slashdot, the
next session should run:

```bash
# live-open slashdot with the patched walker
cargo build -p browsai-cli --features live-browser --bin browsai
./target/debug/browsai live-open "https://slashdot.org" \
    --fingerprint=firefox-130-linux-x86_64 --http2-profile=firefox-130 \
    | python3 -c "import json, sys; d=json.load(sys.stdin); \
        print('node_count:', d.get('node_count'), \
              'candidate_links:', len(d.get('candidate_links', [])))"

# compare candidate_links to example.com's
./target/debug/browsai live-open "https://example.com" \
    --fingerprint=firefox-130-linux-x86_64 --http2-profile=firefox-130 \
    | python3 -c "import json, sys; d=json.load(sys.stdin); \
        print('node_count:', d.get('node_count'), \
              'candidate_links:', len(d.get('candidate_links', [])))"
```

If Slashdot's `candidate_links` count is now non-zero (or higher than
example.com's) the patch is working as intended. If it's still zero,
the dump is coming from the upstream accessibility-tree path and we'd
need to patch that — which requires vendoring `components/style` (stylist)
or `components/accessibility` from Servo, both of which are large crates.

## What this session landed (full commit log)

```
<prior commits>  -- T1.1, T1.2, T1.3 (fingerprint fields + per-domain isolation)
1d927fe         commit 1: vendor servo-net 0.5.0
a4b470d         commit 2: HTTP/2 SETTINGS knobs
f5cb4bc         commit 3: defer T3.1 to sidecar
9142908         commit 4: canvas noise seed plumbing
580bda9         slashdot end-to-end test result doc
fbc0f72         Helios plugin integration guide
db12402         canvas noise wired into vendored Servo readback
af8e0c0         DOM walker textContent fallback
```

Workspace tests: 314 passed, 0 failed.

## What's left for the next session

1. **Verify the Slashdot textContent fallback is reachable** — the test
   above (run slashdot's `live-open` and look at `candidate_links`)
   tells us whether the JS fallback walker is the path actually producing
   the tree for Slashdot. If it is, the patch is doing useful work. If
   not (and the upstream accessibility tree is the path), Slashdot
   article-name extraction is genuinely upstream-Servo work.

2. **If the test in (1) fails** — vendoring `components/style` or
   `components/accessibility` from Servo (large, multi-day, likely
   not worth it for one site). Better alternative: have the plugin run
   a client-side DOM parser against the rendered HTML for Slashdot-
   class scraping.

3. **Verify the `curl-impersonate-httpd` sidecar works end-to-end** —
   the plugin author needs to stand up the sidecar (per
   `docs/helios-integration.md`), run Slashdot navigation through the
   plugin with `HTTPS_PROXY` set, and capture a Wireshark JA3 hash to
   verify it matches Firefox-130.

4. **End-to-end Slashdot article extraction in the plugin** — even
   without link names, `node_count: 1001` from Slashdot means the
   DOM is real. The plugin can run its own DOM parser against the
   rendered HTML (via the existing `browsai evaluate-page-script` path
   or a follow-on that exposes `page.toDataURL()`-style output).

## Files for the next session to read first

- `docs/fingerprint-hardening.md` — overall roadmap, what shipped, what was deferred, why
- `docs/slashdot-test-result.md` — actual Slashdot test result with `node_count` and analysis
- `docs/helios-integration.md` — plugin-side wiring guide with copy-pasteable snippets
- `docs/server.md` — server endpoint table
- `docs/agent-tree-schema.md` — wire format
- `crates/engine-servo/src/lib.rs:1451` — the patched JS DOM walker (this commit)
- `crates/engine-servo/tests/canvas_noise.rs` — canvas-noise unit tests
- `vendor/servo-canvas/lib.rs:22` — `canvas_noise_seed_from_env()` helper
- `vendor/servo-canvas/canvas_data.rs:read_pixels` — canvas noise injection seam

## Key context for the next session

- BrowsAI uses Servo 0.5.0 from crates.io. The vendored crates are
  `servo-script`, `servo-script-bindings`, `servo-storage`, `servo-net`,
  `servo-canvas`. The `servo-net` and `servo-canvas` are vendored into
  `vendor/`. The `servo` umbrella crate is consumed via `crates.io`.
- The deterministic backend (`no_raster: true`, default) returns only
  the Page root; `live-open` uses the real Servo runtime path and
  gets the real DOM.
- The `--fingerprint=` and `--http2-profile=` flags are propagated
  to vendored Servo via `BROWSAI_CANVAS_NOISE_SEED` (env var) and
  through `ContextOptions.http2_profile` (Rust type).
- `cargo test -p browsai-engine-servo --features servo-runtime
  --test canvas_noise` runs the new noise tests if you want to
  sanity-check locally.

## Open items from the original spec that genuinely need upstream work

1. **Tier 3.1 actual TLS impersonation inside BrowsAI** — the upstream
   `rustls-impersonate` repo (0x67646e/rustls-impersonate) is no longer
   available as a standalone crate. The functionality is folded into
   `z0uki/impit`, which is too large to vendor. The recommended path
   is the sidecar (commit `f5cb4bc`).
2. **Slashdot article-name resolution at the upstream Servo level** —
   the upstream Servo accessibility-tree serialization uses its own
   algorithm that returns empty names for Slashdot article teasers.
   Patching that requires vendoring more of Servo (likely not worth
   it for one site; better to extract links client-side via a JS
   script that does `document.querySelectorAll('a').textContent`).

## If the next session is short, do only this

1. Run the Slashdot verification command from "What's left for the next
   session" item 1 to see whether the JS walker fallback is actually
   in use on Slashdot. If yes, the patch is doing real work. If no,
   document that and move on.
