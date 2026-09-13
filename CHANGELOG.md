# Changelog

## Unreleased

- New `POST /native-input` HTTP endpoint on `browsai serve` forwards
  raw `NativeInputEvent`s (`pointer_move`, `pointer_down`,
  `pointer_up`, `key_down`, `key_up`, `text_input`, `scroll`) to the
  live runtime's `WebView::notify_input_event` for the page that
  owns the supplied `url`'s host. Lets plugins drive gestures that
  the `ActionPlanner` doesn't have a typed verb for — wheel scrolls
  on IntersectionObserver-gated pages, drag-and-drop, raw key chords,
  multi-event click sequences. Body accepts an `events` array
  (tagged JSON with snake_case `type` discriminator) plus the same
  post-dispatch settling fields as `/browse` (`wait_ms`,
  `wait_for_network_idle`, `network_idle_ms`,
  `network_idle_grace_ms`, `network_idle_max_ms`) and an optional
  `snapshot_only` flag. Response carries `dispatched`, `host`,
  `url`, the echoed `events`, and the projected `PageSnapshot` so
  callers don't need a follow-up `/browse`.
- `wait_for_network_idle` scroll-trigger now does **two passes of
  6 steps each** (~3.5s total) and **dispatches a synthetic
  `WheelEvent`** to both `window` and `document` at every step —
  `window.scrollTo()` doesn't fire wheel events, and many
  third-party lazy-loaders gate on `'wheel'` rather than `'scroll'`.
  The second pass catches the chained case where the first pass
  reveals lazy-loaded content whose own `IntersectionObserver`s
  need a second pass to fire; a longer pause at the end of each
  pass gives any chained `setTimeout(0)` / fetch work time to
  land. If the heuristic still misses a particular site, the new
  `POST /native-input` endpoint on `browsai serve` exposes raw
  `NativeInputEvent` dispatch so a plugin can run its own
  scroll-and-snapshot loop with full control over timing (see
  `docs/server.md` § *Plugin-side scroll loop*).
- `wait_for_network_idle` now also waits for **scroll-triggered
  lazy loads** in addition to DOM stability. The installer kicks off
  a programmatic top→bottom→top scroll in 6 steps so any
  `IntersectionObserver`-gated content below the fold actually fires
  before the stability check starts ticking. Sites like GitHub topic
  pages gate the repo-card region behind an `IntersectionObserver`
  that never observes anything below the viewport until the user
  scrolls — without this nudge the DOM reaches a stable "no
  mutations" state almost immediately and the snapshot returns the
  empty shell. The scroll sequence runs in roughly 6×120ms ≈ 720ms
  and is short-circuited when the page fits in the viewport. A new
  `network_idle_grace_ms` (default `1000`) is the additional quiet
  window required after the `network_idle_ms` stability check
  passes, so a lazy-load that schedules work on the *next* tick
  lands its first mutation while we're still waiting instead of
  after we've already snapshotted.
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