# BrowsAI Architecture

## Boundary

The browser engine executes HTML, CSS, JavaScript, network activity, storage, events, input, and layout. BrowsAI observes resulting state through `engine-api`, compiles it into structural, semantic, and application IR, then emits an Agent Render Tree. The tree is the source for agent queries, explanations, snapshots, and diffs.

Semantic intent is resolved into native input events. Page JavaScript and agent JavaScript are separate runtimes. Privileged services are brokered through typed boundaries; agents receive capability references rather than raw secrets.

## Initial crate responsibilities

- `engine-api`: engine-neutral browser context, page, snapshot, input, and page-evaluation contracts.
- `engine-servo`: the Servo-facing adapter boundary. Its default backend is deterministic and in-memory for contract tests; the real Servo runtime is an explicit follow-up feature so core crates stay buildable without a graphics/runtime installation.
- `agent-tree`: stable agent nodes, roles, values, actions, relationships, confidence, and provenance.
- `structural-ir`, `semantic-ir`, `application-ir`: progressively more meaningful representations without overwriting lower-level facts.
- `provenance`: observed/inferred evidence and confidence.
- `state`: immutable page snapshots and lifecycle state.
- `diff`: semantic snapshot changes.
- `input`: semantic actions and native input events.

Servo will be implemented as an adapter to `engine-api`. No agent-facing contract depends on Servo types.

## Platform policy

Linux is the initial host target. macOS and Windows are compatibility targets
using the same engine-neutral contracts, process topology, sandbox policy, and
broker boundaries. Headless execution is deterministic and no-raster; desktop
rendering is optional.

## Data flow

```text
engine state -> structural IR -> semantic IR -> application IR -> Agent Render Tree
       |              |               |                |
       +--------------+---------------+----------------+-> snapshot/diff/query
agent intent -> action planner -> native input -> browser runtime
```
