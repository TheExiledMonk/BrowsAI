# SDK reference

The TypeScript and Python SDKs share protocol version `1` and expose page
query, bounded query pagination, node lookup, explainability, navigation,
actions, snapshots, diffs, capabilities, confirmations, and subscriptions.

The Python client also provides `AsyncAgentClient` with the same wire methods;
both clients preserve request IDs supplied by their transport for correlation.
High-level clients expose `BrowsAI.launch(...)` and `open`/navigation through
the same transport; the transport remains injectable so embedding applications
can provide their authenticated IPC or remote connection.

Wire query fields use camelCase. Page content is returned as data and is never
treated as agent instructions. Clients should honor explicit capability and
confirmation responses and preserve request IDs when correlating operations.
Bounded query and snapshot pages expose `cursor`, `limit`, `total`,
`truncated`, and opaque `next_cursor`; `next_offset` remains available for
compatibility with older clients.

Rust protocol changes require synchronized SDK updates, serialization tests,
and compatibility handling for persisted data.
