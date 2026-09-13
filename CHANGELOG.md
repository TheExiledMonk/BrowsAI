# Changelog

## Unreleased

- `browsai-page-text`: emit standard Markdown `[text](url)` for links
  and `![alt](src)` for images when the projection script resolves
  the URL. Anchors/images without a resolved URL still fall back to
  BB-code-style `[link id="…"]` so the action-side `AgentNodeId`
  remains resolvable. Format version bumps to `2` for this
  behaviour change.
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