# BrowsAI Security Boundaries

Web content is untrusted data. It cannot grant capabilities, alter the agent system prompt, approve a transaction, or access privileged broker state.

Agents do not receive plaintext passwords, TOTP seeds, passkey private keys, OAuth refresh tokens, session secrets, payment card numbers, or private identity fields. Credential and vault services perform operations inside a broker, keep encrypted records behind a process-local zeroized key, and return opaque references or policy-approved results.

Renderer/site processes, the agent runtime, and privileged services are separate trust domains. Communication crosses typed, authenticated IPC and carries source, destination, request, workspace, profile, origin, and capability context.

Unknown effects remain unknown. Inferences are marked with confidence and provenance. Every security boundary must ship with automated regression tests before being marked complete in `TODO.md`.
