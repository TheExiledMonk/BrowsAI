# Controlled test sites

These fixtures are local, deterministic inputs for engine, observer, semantic,
security, and agent tests. They must not depend on third-party hosts or remote
scripts. Each fixture should document the browser behavior it exercises.

Additional groups include `malicious-content/`, `conformance/`, and
`semantic-golden/`; all are local, deterministic, and untrusted input.
The `engine/` group contains compatibility manifests that tie representative
fixtures to required engine capabilities.
The fixture tree includes deterministic pages for HTML/DOM, CSS, JavaScript,
forms/dialogs, framework-shaped mounts, frames, shadow DOM, WebSocket, workers,
service workers, canvas/WebGL, media, and accessibility observations. Fixtures
do not load third-party networks; each page is self-contained or uses a local
fixture companion.

Run `python3 scripts/run_fixture_suite.py` for the deterministic fixture,
engine, semantic, action, and agent harness suite; CI runs its check-only
manifest validation in addition to the full Rust workspace gate.
