# Release and packaging runbook

Linux is the first supported production target. Headless and desktop artifacts
must be built from the same workspace revision, with the engine adapter, sandbox
policy, profiles, SDKs, and local fixtures versioned together.

Release gates are formatting, clippy, the full workspace test suite, fixture
validation, SDK checks, dependency/license review, and security review of broker
boundaries. Packages carry a schema/protocol version and migration/rollback
notes. macOS and Windows follow the same contracts and must not weaken isolation.
