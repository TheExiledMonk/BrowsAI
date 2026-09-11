# Engine integration

`crates/engine-api` is the stable browser boundary. Hosts create a context,
create pages, navigate, request snapshots, dispatch `NativeInputEvent` values,
and use the checked page-evaluation escape hatch. Engine-specific types do not
cross this boundary.

`crates/engine-servo` provides the deterministic adapter used by contract tests
and an optional in-process Servo backend behind the `servo-runtime` feature.
Requesting the live backend without that feature returns an explicit
unsupported error; it never silently falls back to deterministic execution.

Use `EngineCapabilities` and `ServoEngine::context_capabilities` before relying
on optional behavior. Page evaluation requires a non-empty origin,
capability, provenance reference, and a bounded non-zero timeout.

Linux is the initial host target. macOS and Windows use the same engine-neutral
contracts as compatibility targets, while headless mode remains no-raster and
deterministic.
