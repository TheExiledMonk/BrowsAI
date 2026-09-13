# Changelog

## Unreleased

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
- Known limitation: pages that load content via XHR after the initial
  snapshot (turbo-frame, fetch-on-mount) may not have the XHR-loaded
  nodes in the projection. Pass `wait_ms: 5000` for now; a
  selector-poll primitive is a follow-up.
- Initial public source: profile-driven browser identity, challenge
  observer, humanized mouse trajectory generator, unattended solve
  pathway, randomized initial cursor, dual-licensing structure
  (custom public-use licence + commercial licence).
- 295+ workspace tests pass on `cargo test --workspace`.
- 20-site smoke run via `browsai check corpus` (deterministic backend).
- 20-site smoke run via `browsai live-open --auto-solve` (real Servo
  runtime).

Future releases are recorded here as they land.