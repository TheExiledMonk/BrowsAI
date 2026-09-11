# Data flow and representation boundaries

The selected browser engine owns browser state. Observers read the resulting
DOM, layout, runtime, network, storage, focus, and navigation facts. Those
facts are compiled in order:

```text
browser state -> structural IR -> semantic IR -> application IR
              -> Agent Render Tree -> query/snapshot/diff
agent intent  -> action planner -> native input -> browser state
```

Structural and semantic compilation must preserve source node identity,
confidence, provenance, generation, and visibility. Application-level entities
may aggregate nodes, but they do not replace page state or agent memory.

Sensitive material is broker-owned. Trees, snapshots, logs, IPC envelopes, and
error values carry opaque references or redacted values only. Network and API
discovery records retain request IDs, response status/schema observations, and
causal timing so application evidence can be joined without copying secrets.
