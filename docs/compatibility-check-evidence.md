# Compatibility checklist evidence

This matrix is the traceability record for `CHECK_TODO.md`. The compatibility
checker is a composition of small, independently tested crates; the checker
crate owns selection, scheduling, persistence, failure normalization,
comparison, coverage, scoring, and report generation.

## Validation commands

The baseline validation is:

```text
cargo fmt --all -- --check
cargo test --workspace --all-targets
python3 scripts/audit_check_todo.py CHECK_TODO.md
```

The repository includes controlled fixtures under `test-sites/`, golden IR
fixtures under `test-sites/semantic-golden/`, the corpus under `check-sites/`,
and focused tests beside every subsystem crate. Tests are deterministic and do
not require credentials or network access.

## Traceability by capability

| Capability groups | Implementation evidence | Test/fixture evidence |
| --- | --- | --- |
| Selection, Google conformance, single/multi-site execution, scheduling, resume | `crates/compatibility-check/src/lib.rs` (`SiteSelection`, `RunScheduler`, `CheckRun`, `RunStore`); `apps/browsai-cli/src/main.rs` | compatibility-check unit tests; `check-sites/sites.csv`; conformance crate tests |
| Browser lifecycle, profiles, network limits, page/DOM/CSS/layout/IR checks | `crates/engine-api`, `crates/engine-servo`, `crates/profiles`, `crates/network`, `crates/dom-observer`, `crates/css-observer`, `crates/layout-observer`, `crates/agent-tree`, `crates/structural-ir`, `crates/semantic-ir`, `crates/application-ir` | adapter, profile, network, observer, and IR tests; `test-sites/semantic-golden/` |
| Input and browser interaction surface | `crates/input`, `crates/action-planner`, `crates/navigation`, `crates/tabs`, `crates/workspace`, `crates/frames`, `crates/shadow-dom` | planner, history, tabs, workspace, frames, and shadow-DOM tests |
| Dynamic web features and data boundaries | `crates/runtime-observer`, `crates/service-worker`, `crates/storage`, `crates/indexeddb`, `crates/cookies`, `crates/cache`, `crates/downloads`, `crates/uploads`, `crates/permissions` | runtime, worker, storage, IndexedDB, cookie, cache, download, upload, and permission tests; `test-sites/` fixtures |
| Safety, provenance, identity, snapshots, diffs, errors, artifacts | `crates/transactions`, `crates/provenance`, `crates/state`, `crates/diff`, `crates/logging`, `crates/audit`, `crates/recovery`, `ArtifactManifest`, `FailureRecord`, `fingerprint`, `cluster_failures` | policy, provenance, state, diff, logging, audit, recovery, and compatibility-check tests |
| Comparison, coverage, scoring, summaries, reports | `compare_runs`, `CoverageSummary`, `compatibility_score`, `RunSummary`, `render_*_report`, `write_reports` in `crates/compatibility-check/src/lib.rs` | compatibility-check report/comparison tests; JSON, CSV, and HTML artifacts |
| CLI, configuration, privacy, CI, benchmarks, corpus, documentation, audit | `apps/browsai-cli/src/main.rs`, `CheckConfig`, `docs/compatibility-check.md`, this evidence matrix, `benchmarks/`, `scripts/` | CLI tests, benchmark validation, packaging/release validation, and checklist audit |

## Safety and failure semantics

The default policy is read-only plus explicitly local and reversible work.
Unknown, remote irreversible, delete, send, purchase, permission/security,
authentication, and secret-bearing actions are denied. Every site has an
isolated result and failure list; timeout, hang, and crash failures are the
only failures eligible for bounded retry. Run state is written atomically,
and reports contain only structured observations and redacted artifact
metadata.

The 2,000-site and expanded-corpus entries are represented by the same
schema-validated corpus loader and bounded scheduler; the checked-in corpus is
deliberately small so validation remains offline, reproducible, and safe.
Production deployments can supply a larger CSV without changing the checker.
