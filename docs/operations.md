# Operations and recovery

Linux is the initial host target. Headless operation uses deterministic
options and can disable raster output; desktop rendering is optional. Process
startup and IPC routing are broker-mediated, and the process model records
restart/crash transitions.

Recovery checkpoints contain schema version, profile/window/tab references,
typed generation-bound resources for cookies, storage, downloads, workspaces,
and agent sessions, plus locks and pending confirmations. Secret material is
represented only by opaque references and must be reacquired from brokers.
`RecoveryStore` verifies a checksum before returning state. `FileRecoveryStore` uses a lock file, a temporary file with
`sync_all`, atomic rename, and bounded backup rotation. Restore a backup only
after the primary checkpoint has been identified as corrupt or unavailable.

Operational diagnostics should use structured logs and metrics. Never include
secret bytes, plaintext credentials, capability payloads, or unredacted page
content in diagnostics or crash reports.
