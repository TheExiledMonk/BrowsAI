# Helios plugin integration

The Helios plugin (Node.js, lives in `helios/plugins/helios-browsai/`)
talks to BrowsAI either by:

1. Spawning the CLI per call (`browsai navigate …`), or
2. Spawning the long-running daemon once (`browsai serve --port
   8765`) and POSTing HTTP requests to it.

Both paths now expose the new fingerprinting knobs. The plugin
author needs to:

- Pass `--fingerprint=<id>` so the request presents a coherent
  identity (UA / locale / viewport / brands / navigator fields /
  color_depth) from BrowsAI's vetted catalog.
- Pass `--http2-profile=firefox-130 | chrome-140 | edge` to align
  the HTTP/2 SETTINGS frame with a real browser (T2.1).
- Optionally pass `--canvas-noise-seed=<u32>` to thread a noise seed
  to the vendored Servo. (The actual noise injection is upstream
  Servo's `stylo` work; BrowsAI exposes the seam.)
- Optionally set `HTTPS_PROXY=http://127.0.0.1:8888` on the daemon
  spawn to route TLS through `curl-impersonate-httpd` for the JA3 /
  cipher-suite impersonation (T3.1).

The full fingerprint catalog lives in BrowsAI's repo at
`crates/fingerprint/src/lib.rs`. BrowsAI exposes it over HTTP via
`GET /capabilities` (see `docs/server.md`). Each entry has the
fields the plugin needs:

| Field | Purpose |
| --- | --- |
| `id` | Pass to `--fingerprint=<id>` |
| `user_agent` | For plugin-side logging / audit |
| `platform` | For platform-specific quirks (e.g. touch input) |
| `viewport` width × height | For layout / window-sizing checks |
| `brands` | For `Sec-CH-UA` validation in client code |
| `accept_language` | For `Accept-Language` headers |

## Spawning the CLI per call

The simplest path: spawn `browsai` per tool invocation. The CLI is
stateless, so each call is independent (no per-call isolation needed).

```javascript
// helios/plugins/helios-browsai/index.js
const { spawn } = require('child_process');
const path = require('path');

const BROWSER_BIN = '/usr/local/bin/browsai';

function browseWithFingerprint({ url, fingerprintId, query }) {
  const args = ['navigate', url];
  if (fingerprintId) {
    args.push(`--fingerprint=${fingerprintId}`);
  }
  if (query) {
    args.push(`--query=${query}`);
  }
  const proc = spawn(BROWSER_BIN, args, { stdio: ['ignore', 'pipe', 'pipe'] });
  return new Promise((resolve, reject) => {
    let stdout = '';
    proc.stdout.on('data', (chunk) => (stdout += chunk));
    proc.on('close', (code) =>
      code === 0 ? resolve(JSON.parse(stdout)) : reject(new Error(`exit ${code}`))
    );
  });
}
```

The fingerprint id is selected from BrowsAI's catalog. The plugin
should query `browsai capabilities` once at startup and cache:

```javascript
let FINGERPRINTS;
async function loadFingerprints() {
  if (FINGERPRINTS) return FINGERPRINTS;
  const out = require('child_process')
    .execFileSync(BROWSER_BIN, ['capabilities'], { encoding: 'utf-8' });
  FINGERPRINTS = JSON.parse(out).commands ? JSON.parse(out).commands : null;
  // capabilities also carries the catalog indirectly via the
  // /schema endpoint; fetch that for the real catalog.
  const schema = JSON.parse(require('child_process')
    .execFileSync(BROWSER_BIN, ['serve', '--port', '0'], { encoding: 'utf-8' }));
  return schema;
}
```

## Spawning the daemon

For tools that need repeated navigation, prefer the daemon so
cookies and storage persist between calls on the same domain:

```javascript
const { spawn } = require('child_process');
const BROWSER_BIN = '/usr/local/bin/browsai';
const DAEMON_PORT = 18765;

let daemonProc;

function ensureDaemon() {
  if (daemonProc && !daemonProc.killed) return;
  daemonProc = spawn(BROWSER_BIN,
    ['serve', '--port', String(DAEMON_PORT), '--idle-shutdown-seconds', '300'],
    {
      env: {
        ...process.env,
        // JA3 impersonation via sidecar (T3.1)
        HTTPS_PROXY: process.env.HTTPS_PROXY || '',
      },
      stdio: ['ignore', 'pipe', 'pipe'],
    });
}

async function browseViaDaemon({ url, fingerprintId, query, stream }) {
  ensureDaemon();
  const body = { url };
  if (fingerprintId) body.fingerprint = fingerprintId;
  if (query) body.query = query;
  if (stream) body.stream = true;
  const res = await fetch(`http://127.0.0.1:${DAEMON_PORT}/browse`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  return res.json();
}
```

## TLS impersonation via `curl-impersonate-httpd`

For Tier 3.1 (JA3 / cipher-suite impersonation), BrowsAI itself does
not patch its vendored Servo TLS stack — the upstream
`rustls-impersonate` repo is no longer available as a standalone
crate. The recommended path is a sidecar that handles the TLS-level
signal.

The plugin spawns `curl-impersonate-httpd` with the target browser's
JA3 template, and BrowsAI's daemon routes all upstream HTTPS through
it via the `HTTPS_PROXY` env var.

```sh
# Stand up the sidecar
docker run -d --name curl-impersonate-httpd \
    -p 8888:8888 \
    ghcr.io/imroc/curl-impersonate-httpd:latest \
    --ja3 firefox-130
```

BrowsAI's daemon picks up `HTTPS_PROXY` automatically; no change to
the daemon itself. The plugin spawns the daemon with
`HTTPS_PROXY=http://127.0.0.1:8888`:

```javascript
function startDaemonWithProxy() {
  ensureDaemon();
  daemonProc = spawn(BROWSER_BIN,
    ['serve', '--port', String(DAEMON_PORT), '--idle-shutdown-seconds', '300'],
    {
      env: {
        ...process.env,
        HTTPS_PROXY: 'http://127.0.0.1:8888',
      },
      stdio: ['ignore', 'pipe', 'pipe'],
    });
}
```

## Tool configuration schema

The Helios plugin's `openclaw.plugin.json` should expose these
options so deployments can tune the fingerprint policy per
environment:

```json
{
  "configSchema": {
    "properties": {
      "browsaiBin": {
        "type": "string",
        "default": "/usr/local/bin/browsai",
        "description": "Path to the browsai binary"
      },
      "daemonPort": {
        "type": "integer",
        "default": 18765,
        "description": "TCP port the long-running daemon listens on"
      },
      "defaultFingerprint": {
        "type": "string",
        "enum": [
          "firefox-130-linux-x86_64",
          "firefox-130-linux-1680x945",
          "chrome-140-linux-1280x720",
          "chrome-140-linux-x86_64",
          "safari-17-macos-arm64"
        ],
        "default": "firefox-130-linux-x86_64",
        "description": "Default fingerprint id passed to --fingerprint"
      },
      "http2Profile": {
        "type": "string",
        "enum": ["firefox-130", "chrome-140", "edge"],
        "default": "firefox-130",
        "description": "HTTP/2 SETTINGS profile (T2.1)"
      },
      "httpsProxy": {
        "type": "string",
        "default": null,
        "description": "HTTPS proxy for TLS-level impersonation (T3.1, e.g. http://127.0.0.1:8888)"
      },
      "idleShutdownSeconds": {
        "type": "integer",
        "default": 300,
        "description": "Daemon self-shutdown after this many seconds idle"
      },
      "canvasNoiseSeed": {
        "type": "integer",
        "default": null,
        "description": "Canvas / 2D-rendering noise seed (T2.2 plumbing)"
      }
    }
  }
}
```

The plugin author can read these from `configSchema.properties` and
forward them to the CLI / daemon spawn as flags + env vars.

## Per-call isolation (T1.5)

For high-stakes workflows (financial, account creation, etc.) the
plugin should prefer spawning the CLI per call rather than using the
daemon. Each CLI invocation starts fresh — no cookies, no storage,
no shared state. Use the daemon only when the user explicitly wants
session continuity (shopping cart, login flows, etc.).

```javascript
function browseIsolated({ url, fingerprintId, query }) {
  // Each call gets a fresh engine — no cross-call state.
  return browseWithFingerprint({ url, fingerprintId, query });
}
```

## What this doc does NOT cover

- The vendored Servo `stylo` canvas noise injection — that's
  upstream Servo work; BrowsAI exposes the seam
  (`BROWSAI_CANVAS_NOISE_SEED`) but the actual noise code is
  elsewhere.
- Slashdot article-name extraction — that's upstream Servo
  accessibility-tree work. The plugin can work around it by
  running a client-side DOM parser against the rendered HTML if
  extractable article titles are required.
