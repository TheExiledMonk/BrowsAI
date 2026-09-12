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
| POST   | `/browse`          | `{url, fingerprint?, query?, snapshot_only?, auto_solve?, stream?}`  | no      | yes       |
| POST   | `/query`           | `{url, query?, filter?, cursor?, limit?, fingerprint?, stream?}`     | no      | yes       |
| POST   | `/render`          | `{url, query?, filter?, cursor?, limit?, fingerprint?, stream?}`     | no      | yes       |
| POST   | `/follow-link`     | `{page, node_id, stream?, fingerprint?}`                           | no      | yes       |
| POST   | `/auto-solve`      | `{url, fingerprint?, stream?}`                                     | no      | yes       |

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
nohup browsai serve --port 8765 > /var/log/browsai.log 2>&1 &

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

## Per-domain clean sessions

The long-running server keeps **one `ServoEngine` per host** for the
lifetime of the process. The first request to `example.com`
provisions a fresh engine; subsequent requests to the same host reuse
it. Requests to `other.com` get a separate fresh engine. Within an
engine, cookies, localStorage, IndexedDB, and ServiceWorker
registrations are isolated from other engines — `example.com` and
`other.com` cannot read each other's state.

Idle engines are evicted after
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

