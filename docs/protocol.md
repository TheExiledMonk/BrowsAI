# Protocol and SDK contracts

The engine-neutral contracts live in `crates/engine-api`. Agent-facing query,
snapshot, diff, action, capability, confirmation, and event types live in
`crates/agent-protocol`. IPC envelopes include protocol version, sequence,
request ID, process endpoints, workspace/profile labels, origin, capability,
and a typed payload.

Large page queries use `QueryPage`: callers provide an offset and limit and
receive bounded results plus total and next-offset metadata. The TypeScript
and Python clients expose the same operation as `queryPage` and `query_page`.
Limits are normalized to at least one item so a caller cannot receive a
non-progressing cursor.

Protocol changes must be intentional:

1. update the Rust contract and serialization tests;
2. update the TypeScript and Python SDK surfaces;
3. add compatibility or migration behavior for persisted data;
4. run the workspace test, Clippy, and formatting gates.

Unsupported browser capabilities are represented explicitly. Callers must not
infer support from an absent field or silently substitute automation semantics.
