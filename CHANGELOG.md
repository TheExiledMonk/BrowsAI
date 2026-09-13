# Changelog

## Unreleased

- `browsai-page-text`: new crate that renders an `AgentRenderTree` as
  markdown for LLM consumption. Actionable widgets get BB-code-style
  tags (`[link id="…"]`, `[button id="…"]`, `[textbox id="…"]`, etc.)
  so the LLM can both read the page as prose and resolve the same
  `AgentNodeId` back through the existing `/follow-link` and `/action`
  endpoints. `--format=text` strips the tags for cheaper read-only
  prompts. Body wrapped in a `<page-content trust="untrusted">`
  envelope by default to mark page data vs. instructions.
- Wire surface:
  - CLI: `browsai query <url> --format=markdown|text` and
    `browsai render <url> --format=markdown|text`. With `--stream`,
    emits a `text-frame` NDJSON event before `snapshot-complete`.
  - HTTP: `format` body field on `/browse`, `/query`, `/render`.
- Initial public source: profile-driven browser identity, challenge
  observer, humanized mouse trajectory generator, unattended solve
  pathway, randomized initial cursor, dual-licensing structure
  (custom public-use licence + commercial licence).
- 295+ workspace tests pass on `cargo test --workspace`.
- 20-site smoke run via `browsai check corpus` (deterministic backend).
- 20-site smoke run via `browsai live-open --auto-solve` (real Servo
  runtime).

Future releases are recorded here as they land.