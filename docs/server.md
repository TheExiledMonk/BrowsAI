# BrowsAI HTTP server

`browsai serve [--bind <host>] [--port <port>]` opens a localhost
HTTP/1.1 endpoint that drives the same CLI logic over a socket, so an
LLM (or any automation) can command the browser with `curl`, Python
`requests`, or any HTTP client.

The server has **no async runtime, no tokio, no hyper**. It is a thin
wrapper around `std::net::TcpListener`. Each request spawns a CLI
subprocess and streams the subprocess stdout back over chunked transfer
encoding for streaming responses.

## Routes

| Method | Path               | Body                                                                 | Default | Streaming |
| ------ | ------------------ | -------------------------------------------------------------------- | ------- | --------- |
| GET    | `/health`          | (none)                                                               | yes     | no        |
| GET    | `/capabilities`    | (none)                                                               | yes     | no        |
| GET    | `/version`         | (none)                                                               | yes     | no        |
| GET    | `/schema`          | (none)                                                               | yes     | no        |
| POST   | `/browse`          | `{url, fingerprint?, query?, filter?, cursor?, limit?, wait_ms?, wait_for_network_idle?, network_idle_ms?, network_idle_grace_ms?, network_idle_max_ms?, snapshot_only?, format?, auto_solve?, stream?}` | no      | yes       |
| POST   | `/query`           | `{url, fingerprint?, query?, filter?, cursor?, limit?, wait_ms?, wait_for_network_idle?, network_idle_ms?, network_idle_grace_ms?, network_idle_max_ms?, format?, stream?}`                            | no      | yes       |
| POST   | `/render`          | `{url, fingerprint?, query?, filter?, cursor?, limit?, wait_ms?, wait_for_network_idle?, network_idle_ms?, network_idle_grace_ms?, network_idle_max_ms?, format?, stream?}`                            | no      | yes       |
| POST   | `/follow-link`     | `{page, node_id, stream?, fingerprint?}`                           | no      | yes       |
| POST   | `/auto-solve`      | `{url, fingerprint?, stream?}`                                     | no      | yes       |

`format` accepts `"tree"` (default, current JSON shape), `"markdown"`,
or `"text"`. When set to `"markdown"`, the response body is a
[`MarkdownView`](agent-tree-schema.md#markdownview) — a prose rendering
of the page with BB-code-style actionable tags (`[link id="…"]`,
`[button id="…"]`, `[textbox id="…"]`) so the LLM can both read the
page and resolve actionable elements back through the existing
`/follow-link` and `/action` endpoints using the `id` attribute. When
set to `"text"`, the BB-code tags are stripped and only the prose
remains — useful for cheaper Q&A/summarisation prompts where the LLM
doesn't need to act on the page.

`wait_for_network_idle` (default **`true`**) installs a JS interceptor
that tracks four signals and polls until **all** of them have been
quiet for `network_idle_ms + network_idle_grace_ms` continuously
(defaults `500` + `1000` = `1500`) or `network_idle_max_ms` total
elapsed (default `10000`):

1. `window.fetch` and `XMLHttpRequest` in-flight counts (`__browsaiInFlight`)
2. `<img>` elements still loading (`__browsaiImagesLoading`) — the
   interceptor sweeps existing images and watches
   `MutationObserver` for dynamically-added ones
3. `document.readyState` (`__browsaiReadyState`) — must be `complete`
4. **DOM stability** (`__browsaiLastMutation`) — a `MutationObserver`
   on `documentElement` watching `childList`, `subtree`,
   `attributes`, and `characterData` bumps a timestamp on every
   mutation. The polling loop also requires that no mutation has
   happened for `idle_ms`. This catches the cases where simple
   counter-based waits miss: `setTimeout(fn, 0)` callbacks,
   `requestAnimationFrame` callbacks, `IntersectionObserver`
   triggers, and promise microtasks that mutate the DOM after the
   explicit resources have settled. Without this, lazy-loaded
   content (turbo-frame cards, infinite scroll, fetch-on-mount
   SPAs, intersection-observer image lazy loading) doesn't make it
   into the snapshot.
5. **Scroll trigger complete** (`__browsaiScrollComplete`) — the
   installer kicks off a programmatic top→bottom→top scroll in
   6 steps so any `IntersectionObserver`-gated content below the
   fold actually fires before the stability check starts ticking.
   Sites like GitHub topic pages gate the entire repo-card region
   behind an `IntersectionObserver` that never observes anything
   below the viewport until the user scrolls — without this nudge
   the DOM reaches a stable "no mutations" state almost immediately
   and the snapshot returns the empty shell. The scroll sequence
   runs in roughly 6×120ms ≈ 720ms and is short-circuited when the
   page fits in the viewport (no scrollable distance). Pages that
   fit in the viewport are unaffected.

All five must hold simultaneously before the snapshot proceeds.
Pass `wait_for_network_idle: false` to skip the wait for cached /
fully server-rendered pages where the extra 500ms minimum is
wasteful. The deterministic backend returns `EngineError::Unsupported`
for this method and the server silently ignores it — tests and
offline runs are unaffected.

`network_idle_grace_ms` (default `1000`) is the additional quiet
window required *after* the `network_idle_ms` stability check passes.
Without the grace, a scroll-triggered lazy-load that schedules work
on the *next* tick (e.g. a chained promise microtask) can land its
first mutation just after we declare stability, producing a
half-populated snapshot. The grace is on top of the scroll trigger
and the regular idle window — set it to `0` to recover the original
behaviour.

### Response shapes

Non-streaming responses are `application/json` and follow the same
shapes documented in [`agent-tree-schema.md`](agent-tree-schema.md).
The CLI's existing `browse`/`query`/`render`/`follow-link`/`live-open`
commands produce the same JSON — the server just swaps the CLI argv
boundary for an HTTP boundary.

### Streaming

Add `?stream=true` to any POST endpoint (or `"stream": true` in the
JSON body). The response is `Transfer-Encoding: chunked` with
`Content-Type: application/x-ndjson`. Each NDJSON event from the CLI
becomes one HTTP chunk:

```http
POST /browse?stream=true HTTP/1.1
Content-Type: application/json

{"url":"https://example.test"}
```

```http
HTTP/1.1 200 OK
Content-Type: application/x-ndjson
Transfer-Encoding: chunked
Connection: close

aa
{"command":"navigate","schema_version":"browsai-agent-tree/1.0","type":"page-pending","url":"https://example.test/"}

89
{"command":"navigate","page":1,"schema_version":"browsai-agent-tree/1.0","type":"page","url":"https://example.test/"}

ab
{"confidence":1.0,...,"node_id":"page:...","role":"Page","schema_version":"browsai-agent-tree/1.0","type":"node"}

55
{"command":"navigate","node_count":1,"snapshot":1,"type":"snapshot-complete"}

0

```

## Examples

### Discovery

```sh
curl -s http://127.0.0.1:8765/health | jq
curl -s http://127.0.0.1:8765/capabilities | jq '.commands[].name'
curl -s http://127.0.0.1:8765/schema | jq '.endpoints'
```

### One-shot browse

```sh
curl -s -X POST -H 'Content-Type: application/json' \
    -d '{"url":"https://example.test","query":"release notes","snapshot_only":true}' \
    http://127.0.0.1:8765/browse | jq
```

### Stream an agent interaction

```sh
curl -s -N -X POST -H 'Content-Type: application/json' \
    -d '{"url":"https://example.test","stream":true,"query":"login"}' \
    http://127.0.0.1:8765/browse
```

Each line of output is one NDJSON event. The connection closes when
the `snapshot-complete` event is emitted.

### Query with role filter

```sh
curl -s -X POST -H 'Content-Type: application/json' \
    -d '{"url":"https://example.test","filter":"Link,Heading","cursor":0,"limit":50}' \
    http://127.0.0.1:8765/query | jq '.results | length'
```

`/query` and `/render` navigate to the supplied `url` first and then
return the same shapes as `/browse` would (with the role `filter`,
`cursor`, `limit`, and confidence-biasing `query` applied). They share
the daemon's per-domain `ServoEngine`, so a follow-up `/browse` or
`/query` to the same host reuses the same engine instance.

### Waiting for JS-rendered content

Sites whose initial HTML is a shell populated by client-side JS (e.g.
DuckDuckGo search results) can return a snapshot that only contains
the loading skeleton. Pass `wait_ms` on `/browse`, `/query`, or `/render`
to give the embedder extra event-loop time after `LoadStatus::Complete`
fires before the snapshot is projected:

```sh
# DuckDuckGo: results render after the React bundle runs
curl -s -X POST -H 'Content-Type: application/json' \
    -d '{"url":"https://duckduckgo.com/?q=rust+async","wait_ms":3000}' \
    http://127.0.0.1:8765/browse | jq '.node_count'
```

`wait_ms` defaults to `1000` so SPA shells (DuckDuckGo, X, login
walls) settle their JS-driven DOM on the first call without the
caller having to opt in. Pass `0` for server-rendered pages where the
snapshot is already complete (skips the post-navigation pump), or
`3000-5000` for sites that fire an XHR after `LoadStatus::Complete`.
The CLI's `live-open` unconditionally pumps for `2000` ms
(`apps/browsai-cli/src/main.rs:574-576`); the server default of `1000`
covers the common case without doubling the response latency on
plain HTML.

### Follow a search result

```sh
curl -s -X POST -H 'Content-Type: application/json' \
    -d '{"page":1,"node_id":"link-12"}' \
    http://127.0.0.1:8765/follow-link | jq '.events'
```

## Auth and binding

By default the server binds to `127.0.0.1`. To expose it on another
host, pass `--bind 0.0.0.0`. The listener does **not** authenticate —
the operator is responsible for firewalling, tunnels, or running
behind a reverse proxy.

The server does not keep any state between requests. Each request is
isolated; the subprocess it spawned is reaped when it exits.

## Resource limits and errors

If the CLI subprocess exits non-zero, the server responds with HTTP
`500` and a JSON body containing the stderr trailer:

```json
{
  "error": "browsai exited Some(1)",
  "stderr": "BROWSAI_RUNTIME_ERROR: ..."
}
```

The streaming endpoint handles process death naturally — the HTTP
connection is closed with the last chunked write.

## Running it as a daemon

```sh
# Foreground, blocking
browsai serve --port 8765

# Background, log to file
nohup browsai serve --port 8765 >/var/log/browsai.log 2>&1 &

# Self-shut-down after 5 minutes of inactivity
browsai serve --port 8765 --idle-shutdown-seconds 300
```

There is no built-in `browsai stop`; send `SIGTERM` to the process.
For long-running deployments, point a systemd unit at the binary:

```ini
[Unit]
Description=BrowsAI HTTP server
After=network-online.target

[Service]
ExecStart=/usr/local/bin/browsai serve --port 8765 --idle-shutdown-seconds 3600
Restart=always
RestartSec=2

[Install]
WantedBy=multi-user.target
```

## Live-browser runtime

The daemon runs the live Servo embedder by default. The deterministic
backend (1 node per page, no JS execution, no network) is now opt-in via
`--no-live-browser` and is only useful for unit-test fixtures that
need the wire protocol without the heavy Servo dep:

```sh
# Production: live runtime, real navigation, full agent tree.
browsai serve --port 8765 --idle-shutdown-seconds 3600 \
    --fingerprint=firefox-130-linux-x86_64 \
    --http2-profile=firefox-130

# Test fixture: deterministic stub, returns 1 Page root node.
browsai serve --port 8765 --no-live-browser
```

The default flipped from `--live-browser` opt-in to live by default
because shipping a daemon that silently serves a 1-node stub is a worse
failure mode than asking the operator to opt out of the live embedder.
If the binary was built without the `live-browser` Cargo feature, the
daemon logs a loud warning at startup and every navigation returns
`EngineError::Unsupported` until you either pass `--no-live-browser`
or rebuild with `--features browsai-cli/live-browser`.

Equivalent env-var form (useful for process supervisors that can't
pass flags):

| Env var | Effect |
| --- | --- |
| `BROWSAI_SERVER_LIVE_BROWSER=0` / `=no` / `=off` | Same as `--no-live-browser` |
| `BROWSAI_SERVER_LIVE_BROWSER=1` | Same as `--live-browser` (explicit; default is already live) |
| `BROWSAI_DEFAULT_FINGERPRINT` | Default `--fingerprint` if no flag |
| `BROWSAI_DEFAULT_HTTP2_PROFILE` | Default `--http2-profile` if no flag |
| `BROWSAI_CANVAS_NOISE_SEED` | Forwarded to vendored canvas readback |
| `BROWSAI_HTTP2_PROFILE` | Set automatically by `browsai serve`; vendored `servo-net` reads it for HTTP/2 SETTINGS |

`--http2-profile` is auto-derived from `--fingerprint` when not
explicitly set: `firefox-*` → `firefox-130`, `chrome-*` / `chromium-*`
→ `chrome-140`, `edge-*` → `edge`. If neither fingerprint nor
`--http2-profile` is set, the daemon defaults to `firefox-130`.

## Per-domain clean sessions

The long-running server keeps **one `ServoEngine`** (one
`ServoRuntime`) for the lifetime of the process and gives each host
its own `(ContextId, PageId)`. The first request to `example.com`
provisions a fresh context + page; subsequent requests to the same
host reuse the same page; requests to `other.com` get a separate
fresh context + page in the same engine.

Because Servo's `HttpState.cookie_jar` is keyed by host
(`vendor/servo-net/cookie_storage.rs:54`) and IndexedDB /
ServiceWorker scope are keyed by origin URL, cookies, localStorage,
IndexedDB, and ServiceWorker registrations are isolated between
domains even though they share a single Servo instance:
`example.com` cannot read `other.com`'s state.

A previous design instantiated a fresh `ServoEngine` per host. That
tripped Servo 0.5's process-wide `Opts` singleton at
`servo-config/opts.rs:279` ("Already initialized") on every
second-and-later domain and was replaced by the shared-engine model.
The CLI never hit that panic because each `browsai live-open`
invocation is a fresh process.

Idle host sessions are evicted after
`--max-idle-per-domain-seconds` (default 60) of no activity, so the
process does not grow without bound across many distinct hosts.

The current `GET /health` response carries `active_domains` so you can
monitor how many hosts are alive:

```sh
$ curl -s http://127.0.0.1:8765/health | jq
{
  "live_browser_compiled": false,
  "servo_loaded": false,
  "egl_available": false,
  "uptime_seconds": 42,
  "active_domains": 2,
  "idle_shutdown_seconds": 3600,
  "max_idle_per_domain_seconds": 60,
  "fingerprint": "Mozilla/5.0 ..."
}
```

