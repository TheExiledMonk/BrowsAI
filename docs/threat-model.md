# Threat model

Web content is hostile input. Text, DOM attributes, scripts, network payloads,
and rendered labels cannot grant capabilities, alter system instructions,
satisfy confirmations, or access broker state.

Renderer/site and agent processes are untrusted relative to privileged brokers.
IPC routes are typed, version-checked, and topology-authorized. Secrets remain
behind zeroizing broker handles and are excluded from agent trees, snapshots,
logs, and page content. File paths are canonicalized against capability roots.

Unknown effects remain unknown and require policy handling. Irreversible,
financial, authentication, security, and external side effects require explicit
confirmation unless policy denies them. Every boundary is covered by a
regression test before its TODO item is eligible for completion.
