# Release runbook

Before publishing a build:

1. verify the workspace and SDK versions are aligned;
2. run formatting, strict Clippy, all-feature workspace tests, conformance
   fixtures, benchmark-artifact validation, and the TODO audit;
3. review `SECURITY.md` and the process/sandbox regression results;
4. record engine version, feature matrix, fixture revision, and benchmark
   metadata;
5. produce platform-specific artifacts only for supported host integrations;
6. preserve the previous artifact and checkpoint format for rollback.

Packaging metadata is validated with `scripts/validate_packaging.py`; the
reproducible Linux headless build is `packaging/build.sh headless <directory>`.
The manifest records the desktop-shell library and the macOS/Windows roadmap
without claiming unsupported executable integrations.

Generate a deterministic CycloneDX workspace SBOM with
`scripts/generate_sbom.py --output <path>`. Release notes are maintained in
`CHANGELOG.md`; signing and platform-specific artifact publication remain
host-release responsibilities described by the target roadmap.

The final TODO audit must report zero unchecked and zero in-progress required
items before the project is declared complete. The audit script intentionally
fails `--require-complete` while either class remains.
