# Changelog

## Unreleased

- New `BrowserEngine::wait_for_network_idle` engine primitive plus a JS
  interceptor that tracks `window.fetch` and `XMLHttpRequest`
  in-flight counts. The daemon polls the counter until it has been
  zero for `network_idle_ms` continuously (default 500) or
  `network_idle_max_ms` total (default 8000), so XHR-driven DOM
  mutations from turbo-frame / fetch-on-mount / SPA hydration are
  visible in the snapshot. Opt in via the new
  `wait_for_network_idle` body field on `/browse`, `/query`,
  `/render`, or the `--wait-for-network-idle` CLI flag on
  `live-open`. The deterministic backend returns `Unsupported` for
  the trait method, so test fixtures and offline runs are unaffected.
- `browsai-page-text`: `link_href` and `image_src` now also fall back
  to `description`, `value` as text, and `name` when the projection
  pipeline hasn't promoted the resolved URL to `AgentValue::Url`. This
  catches builds where the URL surfaces in a non-standard field, and
  lets the standard `[text](url)` / `![alt](src)` emission fire
  instead of dropping back to BB-code without a URL.
- Servo projection script (`crates/engine-servo/src/lib.rs`) now
  captures `href` / `xlink:href` / `src` / `action` from anchors,
  links, forms, images, iframes, sources, and scripts so the
  AgentNode for those elements carries a `value: AgentValue::Url`.
- Initial public source: profile-driven browser identity, challenge
  observer, humanized mouse trajectory generator, unattended solve
  pathway, randomized initial cursor, dual-licensing structure
  (custom public-use licence + commercial licence).
- 295+ workspace tests pass on `cargo test --workspace`.
- 20-site smoke run via `browsai check corpus` (deterministic backend).
- 20-site smoke run via `browsai live-open --auto-solve` (real Servo
  runtime).

Future releases are recorded here as they land.