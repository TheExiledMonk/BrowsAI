# Configuration

Context options include profile identity, headless mode, live-runtime
selection, virtual viewport, deterministic clock, and no-raster operation.
Network configuration separately controls proxy/certificate inputs, WebSocket,
worker, and service-worker availability; limits bound requests, responses,
timeouts, and redirects.

Profiles own user-agent, locale, timezone, homepage, extension, history, and
autocomplete configuration. Storage, cookies, cache, permissions, and secrets
are partitioned by the relevant profile and origin/top-level site context.
Persisted state uses explicit JSON wire formats and recovery uses validated
atomic replacement.
