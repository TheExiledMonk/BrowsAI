# Compatibility checker

The compatibility checker is a non-destructive validation layer over the
BrowsAI runtime. Its core types live in
`crates/compatibility-check/src/lib.rs`; browser adapters implement
`SiteExecutor`, while `RunStore` persists resumable run state under
`check-runs/<run-id>/run.json`.

The public crawl defaults are deliberately conservative. Read, navigation,
and clearly local reversible writes are eligible for execution. Remote writes,
sends, deletion, purchases, permission/security changes, authentication, and
unknown consequences are not eligible for automatic execution.

## Commands

```text
cargo run -p browsai-cli -- check site https://example.test
cargo run -p browsai-cli -- check corpus check-sites/sites.csv
cargo run -p browsai-cli -- check report <run-id>
```

The corpus CSV requires `id`, `url`, `domain`, `category`, `priority`, and
`enabled` columns. Each site gets an isolated result record and a distinct
state (`pending`, `running`, `complete`, or `failed`) so an interrupted run
can be resumed from the persisted JSON record.

Failure records include phase, category, severity, normalized fingerprint, and
artifact names. Fingerprints normalize whitespace and case and are intended to
cluster the same compatibility defect across different sites; they must not be
based only on hostname.

The current executor validates browser startup, navigation, snapshot creation,
and the Agent Render Tree / Structural IR / Semantic IR projection. Additional
phase adapters should implement `SiteExecutor` and retain the same safety and
artifact boundaries.

Checklist audits use the checker-specific validator, because the repository's
older TODO validator intentionally targets the earlier 80-section roadmap:

```text
python3 scripts/audit_check_todo.py CHECK_TODO.md
```
