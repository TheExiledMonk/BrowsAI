# Changelog

## Unreleased

- `wait_for_network_idle` now also waits for **DOM stability** on top
  of the fetch / images / readyState signals. A `MutationObserver`
  on `documentElement` watching `childList`, `subtree`,
  `attributes`, and `characterData` bumps a timestamp on every
  mutation; the polling loop also requires no mutation for
  `idle_ms` continuously. This catches lazy-loaded content that the
  counter-based signals miss: `setTimeout(fn, 0)` callbacks,
  `requestAnimationFrame` callbacks, `IntersectionObserver`
  triggers, and promise microtasks that mutate the DOM after all
  explicit resources have settled. Without it, GitHub topic pages
  with turbo-frame cards, infinite-scroll lists, fetch-on-mount
  SPAs, and intersection-observer lazy-loaded images would still
  return mid-load snapshots — the page is not finished until the
  DOM has been quiet, and we wait for that.
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