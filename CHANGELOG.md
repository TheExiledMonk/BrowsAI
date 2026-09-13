# Changelog

## Unreleased

- `wait_for_network_idle` is now the default on the daemon and CLI.
  Every `/browse`, `/query`, and `/render` request (and every
  `browsai live-open` invocation) installs the JS interceptor that
  tracks `window.fetch` and `XMLHttpRequest` in-flight counts and
  waits until they reach zero for 500ms continuously (or 10s total).
  Opt out per request with `"wait_for_network_idle": false` for
  cached / fully server-rendered pages where the extra 500ms
  minimum is wasteful. The deterministic backend returns
  `EngineError::Unsupported` for the trait method and the server
  silently ignores it, so test fixtures and offline runs are
  unaffected.
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