# Changelog

## Unreleased

- `wait_for_network_idle` now waits for three signals simultaneously
  before snapshotting: `window.fetch` and `XMLHttpRequest` in-flight
  counts must be 0, every `<img>` must have finished loading
  (existing images swept on install + `MutationObserver` for
  dynamically-added ones), and `document.readyState` must be
  `complete`. All three must hold for `network_idle_ms` continuously
  (default 500) or `network_idle_max_ms` total (default 10000). The
  page is not returned until the DOM is finished and everything has
  been loaded — returning data mid-load is asking for trouble.
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