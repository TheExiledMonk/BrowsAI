# Process topology and failure behavior

The host owns the process supervisor and connects to broker-mediated services.
The production topology includes browser broker, credential broker, profile and
permission managers, network, file and download brokers, audit and workspace
managers, renderer, worker, agent-runtime, and optional compositor processes.
Renderer, worker, agent, and compositor processes are isolated process kinds;
privileged edges are explicitly allowlisted by `ProcessTopology` and checked by
the typed IPC router.

Child launches clear inherited environment variables and detach standard
streams. A child transition to `Crashed` is recorded by the supervisor. Services
with `OnFailure` or `Always` policy may be restarted from their retained launch
specification; `Never` services remain stopped. IPC channels are bounded,
sequence-numbered, cancellable, and reconnectable, so a restart cannot reuse
queued requests or stale sequence state.

The process-model unit tests cover privileged-route denial, version checks,
isolated child launch, crash/restart state transitions, and clean shutdown.
