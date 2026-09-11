# Agent Render Tree and IR

The browser executes first. Observed DOM, style, layout, runtime, and
accessibility facts are compiled through structural IR, semantic IR, and
application IR into an `AgentRenderTree`.

Agent nodes carry stable conceptual identity, role, name/value, state,
geometry, relationships, actions, provenance, confidence, and generation.
Nodes may also carry an observed origin for frame/site scoping; origin is
treated as data and query filters match it exactly rather than inferring it
from a node identifier.
Sensitive values are omitted or redacted at the engine projection boundary.
Snapshots and queries consume this tree; pixels are optional and never the
only agent representation.

References are generation-bound. A query or action must reject stale or
ambiguous targets rather than guessing. Semantic intent is resolved to native
browser input events, so page behavior remains the source of truth.
