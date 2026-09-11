# Agent Render Tree JSON Schema

**Schema version**: `browsai-agent-tree/1.0`

This document describes the wire format of every JSON value the `browsai`
CLI emits for Agent Render Trees, snapshots, capability queries, and
streaming events. Host applications (the Helios plugin, IDE
integrations, automation daemons) can use this schema to parse responses
statically instead of hand-rolling walkers.

The schema is versioned. A change that breaks existing parsers bumps
the major version; a change that adds new optional fields bumps the
minor version. Host applications should pin to a specific schema version
and reject responses that declare a higher major.

## Top-level response shapes

The CLI emits one of these top-level shapes depending on the command:

| Command shape           | When                                          |
| ----------------------- | --------------------------------------------- |
| `EngineCapabilities`   | `browsai capabilities`                        |
| `BrowserHealth`         | `browsai browser-health`                       |
| `NavigationResult`      | `browsai headless / navigate / open`           |
| `QueryResult`           | `browsai query`                               |
| `RenderResult`          | `browsai render`                              |
| `ActionPlanResult`      | `browsai action`                              |
| `FollowLinkResult`      | `browsai follow-link`                         |
| `LiveResult`            | `browsai live-open / live-search` (deterministic & live) |
| `CompatibilityRun`      | `browsai check corpus / report`               |
| NDJSON event stream     | any command with `--stream`                   |

## Top-level reference types

### `EngineCapabilities` (capabilities)

```jsonc
{
  "engine_name": "servo-adapter",         // string — adapter family name
  "engine_version": "0.1.0",             // string|null — adapter version
  "features": [                          // array of strings
    "Navigation", "Snapshots", "NativeInput", "PageEvaluation"
  ],
  "live_browser_compiled": true,          // bool — was the `servo-runtime` feature enabled at build?
  "commands": [EngineCommand, ...]        // array — introspectable command surface
}
```

`EngineCommand`:

```jsonc
{
  "name": "navigate",                      // string
  "args": ["<url>", "[--flag=...]"],      // array of strings — argument shapes
  "returns": "NavigationResult",          // string — return-shape tag
  "example": { /* example payload or null */ }
}
```

### `BrowserHealth`

```jsonc
{
  "live_browser_compiled": true,          // bool
  "servo_loaded": false,                  // bool — Servo embedder actually constructed?
  "egl_available": false,                 // bool — libEGL loadable from standard search paths?
  "failures": "libEGL.so.1 not found" | null  // string|null — last construction error
}
```

### `NavigationResult`

```jsonc
{
  "page": 1,                              // u64 — page id
  "url": "https://example.test/",         // string — final URL after redirects
  "snapshot": 1,                          // u64|null — snapshot id; absent if `--snapshot-only`
  "node_count": 530                       // u64|null — Agent Render Tree node count
}
```

`--snapshot-only` returns `{ "page": 1, "url": "..." }` only.

### `QueryResult` (full view)

The default shape returned by `browsai query` without `--cursor`/`--limit`:

```jsonc
[ // array of QueryRow
  {
    "node_id": "node-12",                 // string
    "name": "Click here",                 // string|null
    "role": "Link",                       // string — StructuralRole (e.g. Link, Button)
    "semantic_role": "PrimaryNavigation", // string|null — SemanticRole
    "value": "Click here",                // any|null — AgentValue
    "confidence": 0.95,                   // number
    "provenance": [{                      // array of ProvenanceSource
      { "kind": "Dom", "reference": "...", "detail": null }
    }],
    "geometry": { "x": 12, "y": 34, "width": 200, "height": 22 },  // object|null
    "origin": "https://example.test/"      // string|null
  }
]
```

### `QueryResult` (paged view)

When `--cursor`/`--limit` are present (or `--filter` is applied), the
response is wrapped:

```jsonc
{
  "results": [QueryRow, ...],
  "cursor": 0,                            // u64
  "limit": 100,                           // u64
  "total": 530,                           // u64 — total matched
  "truncated": true,                      // bool
  "next_cursor": "100",                   // string|null — opaque cursor for next page
  "filter": ["Link", "Heading"]           // array|null — role filter that produced this
}
```

### `RenderResult`

`browsai render` always returns a `QueryPage`:

```jsonc
{
  "results": [QueryRow, ...],
  "cursor": 0,
  "limit": 100,
  "total": 530,
  "truncated": true,
  "next_cursor": "100"
}
```

### `ActionPlanResult`

```jsonc
[NativeInputEvent, ...]                  // array of NativeInputEvent
```

Each `NativeInputEvent` is one of:

```jsonc
{ "PointerMove":  { "x": 12.5, "y": 34.0 } }
{ "PointerDown":  { "button": 0 } }
{ "PointerUp":    { "button": 0 } }
{ "KeyDown":      { "key": "Enter" } }
{ "KeyUp":        { "key": "Enter" } }
{ "TextInput":    { "text": "hello" } }
{ "Scroll":       { "delta_x": 0.0, "delta_y": 100.0 } }
```

### `FollowLinkResult`

```jsonc
{
  "page": 1,                              // u64 — page id supplied by caller
  "node_id": "node-12",                  // string — target node id
  "events": [NativeInputEvent, ...]       // planned click sequence
}
```

### `LiveResult`

`live-open` and `live-search` return a single JSON blob that wraps the
snapshot under `snapshot` plus several metadata fields:

```jsonc
{
  "url": "...",
  "navigation": {
    "requested_url": "...",
    "final_url": "...",
    "history": ["..."],
    "history_truncated": false,
    "redirect_observed": false,
    "http_status": 200,
    "mime_type": "text/html",
    "origin": "https://...",
    "initiator": null,
    "resource_type": "main_frame"
  },
  "load_status": "Complete",
  "resource_diagnostics": { /* ... */ },
  "resource_requests": [...],
  "resource_requests_truncated": false,
  "runtime_messages": [...],
  "node_count": 530,
  "agent_tree_truncated": false,
  "candidate_links": [],
  "candidate_textboxes": [],
  "candidate_controls": [],
  "clicked_links": [],
  "probed_controls": [],
  "skipped_controls": [],
  "auto_solve_audit": [SolveAuditEvent, ...],
  "diagnostics": { /* optional evaluated script output or { error: ..., document_available: bool } */ }
}
```

`SolveAuditEvent`:

```jsonc
{
  "agent_id": "Agent(...)",
  "provider": "hcaptcha",                 // string — provider id
  "capability_used": "SolveChallenge+Unattended",  // or "CredentialSolve"
  "takeover_id": null,                    // string|null
  "observed_at_tick": 12,                 // u64
  "page": 7,                              // u64|null
  "target_node_id": "challenge:hcaptcha:0"  // string|null
}
```

## Streaming events (NDJSON, `--stream`)

When `--stream` is set, the CLI emits one JSON object per line on stdout.
Every event has a `type` field. The final line of an event stream is
always `snapshot-complete`. The plugin shell (or any host) reads
line-delimited JSON from stdout.

### Event types

| `type`              | Fields                                                 | Emitted when |
| ------------------- | ------------------------------------------------------ | ------------ |
| `page-pending`     | `command`, `url`                                       | right before navigation |
| `page`              | `command`, `page`, `url`                               | right after navigation completes |
| `node`              | `page`, `node_id`, `name`, `role`, `semantic_role`, `geometry`, `confidence` | per node, after confidence pass |
| `snapshot-complete` | `command`, `page`, `node_count`, plus `result_count` for filtered queries, `snapshot` if available | once, after the final snapshot |

### Example stream

```jsonl
{"command":"navigate","type":"page-pending","url":"https://example.test/"}
{"command":"navigate","page":1,"type":"page","url":"https://example.test/"}
{"confidence":0.95,"geometry":{"x":12.0,"y":0.0,"width":120.0,"height":24.0},"name":"Home","node_id":"link-0","page":1,"role":"Link","semantic_role":"PrimaryNavigation","type":"node"}
{"confidence":1.0,"geometry":{"x":128.0,"y":0.0,"width":96.0,"height":24.0},"name":"About","node_id":"link-1","page":1,"role":"Link","semantic_role":"PrimaryNavigation","type":"node"}
{"command":"navigate","node_count":530,"page":1,"snapshot":1,"type":"snapshot-complete"}
```

Plugins should:
- Stream-parse each line as JSON; never assume a single object spans
  more than one line.
- Use `type` to multiplex event handling.
- Treat unknown `type` values as warnings, not errors.
- Cap internal queue size at the `node_count` carried by
  `snapshot-complete`, since the streaming phase ends there.

## Common types

### `StructuralRole`

A Rust enum serialised via `Debug` (e.g. `"Link"`, `"Button"`,
`"Textbox"`, `"Checkbox"`, `"Heading"`, `"Region"`, `"Page"`,
`"Dialog"`, etc.). See `crates/agent-tree/src/lib.rs` for the full list.

### `SemanticRole`

A Rust enum serialised via `Debug` (e.g. `"PrimaryNavigation"`,
`"SearchField"`, `"SubmitAction"`, `"LoginAction"`, `"ConfirmationDialog"`,
`"Challenge"`, `"ProfileInconsistency"`, etc.).

### `Confidence`

Wrapped `f32` in `[0.0, 1.0]`. The CLI semantic-IR pass bumps confidence
to `1.0` on nodes whose visible text matches `--query` terms and to
`0.9` on nodes whose description matches. Lower values mean the node's
classification is uncertain.

### `Geometry`

```jsonc
{ "x": 12.0, "y": 0.0, "width": 240.0, "height": 32.0 }
```

CSS pixels. `null` means the node's geometry is unknown (CSS display
`none`, detached, or virtual).

### `ProvenanceSource`

```jsonc
{
  "kind": "Dom",                          // SourceKind — "Dom", "JavaScript", "Layout", etc.
  "reference": "renderer-dom",            // string — origin identifier
  "detail": "optional extra"              // string|null
}
```

## Versioning rules

1. Adding a new optional field to a top-level response or event:
   minor bump (`1.0` -> `1.1`).
2. Removing a field, renaming a field, or changing a field's type:
   major bump (`1.0` -> `2.0`); host parsers built against `1.x` will
   reject `2.x` unless explicitly opted in.
3. Adding a new event `type` or a new command surface entry:
   minor bump.
4. Bug fixes that do not change the wire format: no bump; document the
   fix in the release notes.

Plugins should:
- Pin to the major version they were built against.
- Reject responses with `schema_version` greater than the major they
  pin to, or fall back to a permissive walker with a logged warning.

The CLI prints the schema version at the top of `--stream` output
within the first event of every batch. Plugins can grab it without a
round-trip to `capabilities`.