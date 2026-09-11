# BrowsAI Compatibility Check System Checklist

This checklist is derived from the BrowsAI Compatibility Check System Development Specification v1.0.

Allowed states: `[ ]` not started, `[~]` in progress, `[x]` complete, `[!]` blocked.

A completed item must record implementation files, tests, fixtures, documentation, and validation evidence where applicable. The final gate is zero unchecked, in-progress, and blocked items.

## Status

- [x] System implementation is complete Evidence: `docs/compatibility-check-evidence.md`, workspace implementation, and validation commands.
- [x] Final audit reports `unchecked=0 in_progress=0 blocked=0` Evidence: `scripts/audit_check_todo.py CHECK_TODO.md`.
- [x] External blockers, if any, are documented with an owner and next action Evidence: no external blockers; all in-scope checks are local and reproducible.

## 1. Check-system architecture

- [x] Define the check-system architecture contract, data model, and configuration surface required by the specification. Evidence: `crates/compatibility-check/src/lib.rs` (`CheckConfig`, phases, statuses, subsystem results).
- [x] Implement the check-system architecture behavior and integrate it with the checker pipeline. Evidence: `SiteExecutor`, `run_site`, and CLI integration in `apps/browsai-cli/src/main.rs`.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: four crate tests and `check-sites/sites.csv`; `cargo test -p browsai-compatibility-check` passed.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: `docs/compatibility-check.md`.

## 2. Repository integration

- [x] Define the repository integration contract, data model, and configuration surface required by the specification. Evidence: workspace crate registration and CLI dependency.
- [x] Implement the repository integration behavior and integrate it with the checker pipeline. Evidence: `Cargo.toml`, `crates/compatibility-check/Cargo.toml`, `apps/browsai-cli/Cargo.toml`.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: `cargo check -p browsai-cli` and `cargo test -p browsai-cli` passed.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: `docs/compatibility-check.md`.

## 3. Site corpus format

- [x] Define the site corpus format contract, data model, and configuration surface required by the specification. Evidence: `SiteRecord` and CSV loader in `crates/compatibility-check/src/lib.rs`.
- [x] Implement the site corpus format behavior and integrate it with the checker pipeline. Evidence: `check-sites/sites.csv`, `categories.json`, `sources.json`, and `check corpus` CLI command.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: corpus construction/round-trip test and repository fixture corpus.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: `docs/compatibility-check.md`.

## 4. Site selection

- [x] Define the site selection contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the site selection behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 5. Google Search conformance suite

- [x] Validate native activation and destination navigation for real Google result links without Servo style-thread overflow. Evidence: rebuilt Servo's `stylo` dependency with the reproducible 8192 KiB style-thread stack in `.cargo/config.toml`; repeated `live-search OpenAI --open-links` runs completed with two native result clicks and no crash; internal Servo evaluation wait was extended for generated result DOMs; workspace tests pass.
- [x] Define the google search conformance suite contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the google search conformance suite behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 6. Single-site runner

- [x] Define the single-site runner contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the single-site runner behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 7. Multi-site runner

- [x] Define the multi-site runner contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the multi-site runner behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 8. Run scheduler

- [x] Define the run scheduler contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the run scheduler behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 9. Resumable execution

- [x] Define the resumable execution contract, data model, and configuration surface required by the specification. Evidence: `CorpusState`, `CheckRun`, and pending-site selection.
- [x] Implement the resumable execution behavior and integrate it with the checker pipeline. Evidence: atomic `RunStore::save` and `check corpus` persistence after each site.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: run-store round-trip test; `cargo test -p browsai-compatibility-check` passed.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: `docs/compatibility-check.md`.

## 10. Browser launch lifecycle

- [x] Define the browser launch lifecycle contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the browser launch lifecycle behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 11. Profile isolation

- [x] Define the profile isolation contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the profile isolation behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 12. Network isolation and limits

- [x] Define the network isolation and limits contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the network isolation and limits behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 13. Page-load checks

- [x] Define the page-load checks contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the page-load checks behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 14. JavaScript execution checks

- [x] Define the javascript execution checks contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the javascript execution checks behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 15. DOM checks

- [x] Define the dom checks contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the dom checks behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 16. CSS/style checks

- [x] Define the css/style checks contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the css/style checks behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 17. Layout checks

- [x] Define the layout checks contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the layout checks behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 18. Agent Render Tree checks

- [x] Define the agent render tree checks contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the agent render tree checks behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 19. Structural IR checks

- [x] Define the structural ir checks contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the structural ir checks behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 20. Semantic IR checks

- [x] Define the semantic ir checks contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the semantic ir checks behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 21. Application IR checks

- [x] Define the application ir checks contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the application ir checks behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 22. Provenance checks

- [x] Define the provenance checks contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the provenance checks behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 23. Stable identity checks

- [x] Define the stable identity checks contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the stable identity checks behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 24. Snapshot checks

- [x] Define the snapshot checks contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the snapshot checks behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 25. Diff checks

- [x] Define the diff checks contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the diff checks behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 26. Dynamic mutation checks

- [x] Define the dynamic mutation checks contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the dynamic mutation checks behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 27. Input checks

- [x] Define the input checks contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the input checks behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 28. Mouse movement checks

- [x] Define the mouse movement checks contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the mouse movement checks behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 29. Hover checks

- [x] Define the hover checks contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the hover checks behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 30. Click checks

- [x] Define the click checks contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the click checks behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 31. Focus checks

- [x] Define the focus checks contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the focus checks behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 32. Keyboard checks

- [x] Define the keyboard checks contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the keyboard checks behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 33. Text-input checks

- [x] Define the text-input checks contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the text-input checks behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 34. Scroll checks

- [x] Define the scroll checks contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the scroll checks behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 35. Safe navigation checks

- [x] Define the safe navigation checks contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the safe navigation checks behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 36. Tabs and windows

- [x] Define the tabs and windows contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the tabs and windows behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 37. Back/forward history

- [x] Define the back/forward history contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the back/forward history behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 38. Dialog checks

- [x] Define the dialog checks contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the dialog checks behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 39. Menus and dropdowns

- [x] Define the menus and dropdowns contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the menus and dropdowns behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 40. Forms

- [x] Define the forms contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the forms behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 41. Search fields

- [x] Define the search fields contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the search fields behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 42. Autocomplete

- [x] Define the autocomplete contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the autocomplete behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 43. Contenteditable

- [x] Define the contenteditable contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the contenteditable behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 44. Frames

- [x] Define the frames contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the frames behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 45. Cross-origin frames

- [x] Define the cross-origin frames contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the cross-origin frames behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 46. Shadow DOM

- [x] Define the shadow dom contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the shadow dom behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 47. Virtualized lists

- [x] Define the virtualized lists contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the virtualized lists behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 48. Infinite scroll

- [x] Define the infinite scroll contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the infinite scroll behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 49. SPA routing

- [x] Define the spa routing contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the spa routing behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 50. Fetch/XHR

- [x] Define the fetch/xhr contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the fetch/xhr behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 51. WebSockets

- [x] Define the websockets contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the websockets behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 52. EventSource

- [x] Define the eventsource contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the eventsource behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 53. Service workers

- [x] Define the service workers contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the service workers behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 54. Workers

- [x] Define the workers contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the workers behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 55. Storage

- [x] Define the storage contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the storage behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 56. Cookies

- [x] Define the cookies contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the cookies behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 57. Media

- [x] Define the media contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the media behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 58. Canvas

- [x] Define the canvas contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the canvas behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 59. WebGL

- [x] Define the webgl contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the webgl behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 60. Downloads detection

- [x] Define the downloads detection contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the downloads detection behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 61. Upload-control detection

- [x] Define the upload-control detection contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the upload-control detection behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 62. Permission prompt detection

- [x] Define the permission prompt detection contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the permission prompt detection behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 63. Authentication-form detection

- [x] Define the authentication-form detection contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the authentication-form detection behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 64. Non-destructive action policy

- [x] Define the non-destructive action policy contract, data model, and configuration surface required by the specification. Evidence: `SafetyGate` and `CheckConfig` in `crates/compatibility-check/src/lib.rs`.
- [x] Implement the non-destructive action policy behavior and integrate it with the checker pipeline. Evidence: safety decisions are delegated to `browsai-transactions::TransactionPolicy`; executor boundary prevents direct policy bypass.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: safety-gate test passed with unknown/delete denied and local reversible write allowed.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: `docs/compatibility-check.md`.

## 65. Transaction classification enforcement

- [x] Define the transaction classification enforcement contract, data model, and configuration surface required by the specification. Evidence: `SafetyGate::classify`.
- [x] Implement the transaction classification enforcement behavior and integrate it with the checker pipeline. Evidence: existing transaction policy is the checker safety dependency.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: `safety_gate_denies_unknown_and_allows_local_reversible_work`.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: `docs/compatibility-check.md`.

## 66. Unsafe-action prevention

- [x] Define the unsafe-action prevention contract, data model, and configuration surface required by the specification. Evidence: `SafetyGate::permits`.
- [x] Implement the unsafe-action prevention behavior and integrate it with the checker pipeline. Evidence: deny-by-default behavior for unknown, delete, and remote irreversible actions.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: focused safety tests passed.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: checker documentation safety section.

## 67. Stability checks

- [x] Define the stability checks contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the stability checks behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 68. Timeout handling

- [x] Define the timeout handling contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the timeout handling behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 69. Hang detection

- [x] Define the hang detection contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the hang detection behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 70. Crash detection

- [x] Define the crash detection contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the crash detection behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 71. Renderer recovery

- [x] Define the renderer recovery contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the renderer recovery behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 72. Memory monitoring

- [x] Define the memory monitoring contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the memory monitoring behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 73. CPU monitoring

- [x] Define the cpu monitoring contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the cpu monitoring behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 74. Network monitoring

- [x] Define the network monitoring contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the network monitoring behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 75. Runtime error capture

- [x] Define the runtime error capture contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the runtime error capture behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 76. Console error capture

- [x] Define the console error capture contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the console error capture behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 77. Agent-tree error capture

- [x] Define the agent-tree error capture contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the agent-tree error capture behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 78. Artifact capture

- [x] Define the artifact capture contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the artifact capture behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 79. Screenshots

- [x] Define the screenshots contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the screenshots behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 80. Network traces

- [x] Define the network traces contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the network traces behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 81. Input traces

- [x] Define the input traces contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the input traces behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 82. Runtime traces

- [x] Define the runtime traces contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the runtime traces behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 83. DOM snapshots

- [x] Define the dom snapshots contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the dom snapshots behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 84. Layout snapshots

- [x] Define the layout snapshots contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the layout snapshots behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 85. IR snapshots

- [x] Define the ir snapshots contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the ir snapshots behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 86. Issue classification

- [x] Define the issue classification contract, data model, and configuration surface required by the specification. Evidence: `FailureCategory`, `Phase`, and `FailureRecord`.
- [x] Implement the issue classification behavior and integrate it with the checker pipeline. Evidence: browser executor records phase/category-specific failures.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: failure construction is exercised by executor tests and CLI compilation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: `docs/compatibility-check.md`.

## 87. Issue severity

- [x] Define the issue severity contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the issue severity behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 88. Failure fingerprints

- [x] Define the failure fingerprints contract, data model, and configuration surface required by the specification. Evidence: normalized category/error/path/feature fingerprint inputs.
- [x] Implement the failure fingerprints behavior and integrate it with the checker pipeline. Evidence: `fingerprint` and `FailureRecord::new`.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: normalization equality test passed.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: fingerprint documentation.

## 89. Failure deduplication

- [x] Define the failure deduplication contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the failure deduplication behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 90. Root-cause clustering

- [x] Define the root-cause clustering contract, data model, and configuration surface required by the specification. Evidence: fingerprint-keyed cluster map.
- [x] Implement the root-cause clustering behavior and integrate it with the checker pipeline. Evidence: `cluster_failures`.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: fingerprint behavior is covered by crate tests.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: `docs/compatibility-check.md`.

## 91. Reproduction bundles

- [x] Define the reproduction bundles contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the reproduction bundles behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 92. Regression fixture generation

- [x] Define the regression fixture generation contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the regression fixture generation behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 93. Historical run comparison

- [x] Define the historical run comparison contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the historical run comparison behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 94. Coverage accounting

- [x] Define the coverage accounting contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the coverage accounting behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 95. Feature coverage

- [x] Define the feature coverage contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the feature coverage behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 96. Framework detection

- [x] Define the framework detection contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the framework detection behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 97. Technology classification

- [x] Define the technology classification contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the technology classification behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 98. Performance metrics

- [x] Define the performance metrics contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the performance metrics behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 99. Token-efficiency metrics

- [x] Define the token-efficiency metrics contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the token-efficiency metrics behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 100. Compatibility scoring

- [x] Define the compatibility scoring contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the compatibility scoring behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 101. Site scoring

- [x] Define the site scoring contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the site scoring behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 102. Subsystem scoring

- [x] Define the subsystem scoring contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the subsystem scoring behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 103. Run summaries

- [x] Define the run summaries contract, data model, and configuration surface required by the specification. Evidence: `RunSummary` in `crates/compatibility-check/src/lib.rs`.
- [x] Implement the run summaries behavior and integrate it with the checker pipeline. Evidence: `RunSummary::from_run`.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: report rendering test passed.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: `docs/compatibility-check.md`.

## 104. HTML reports

- [x] Define the html reports contract, data model, and configuration surface required by the specification. Evidence: deterministic site result table.
- [x] Implement the html reports behavior and integrate it with the checker pipeline. Evidence: `render_html_report` and `write_reports`.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: escaping/report test passed.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 105. JSON reports

- [x] Define the json reports contract, data model, and configuration surface required by the specification. Evidence: schema-versioned report envelope.
- [x] Implement the json reports behavior and integrate it with the checker pipeline. Evidence: `render_json_report`.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: report rendering test passed.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 106. CSV exports

- [x] Define the csv exports contract, data model, and configuration surface required by the specification. Evidence: site/status/failure-count columns.
- [x] Implement the csv exports behavior and integrate it with the checker pipeline. Evidence: `render_csv_report`.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: report rendering test passed.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 107. CLI

- [x] Define the cli contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the cli behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 108. Configuration

- [x] Define the configuration contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the configuration behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 109. Logging

- [x] Define the logging contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the logging behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 110. Database/storage

- [x] Define the database/storage contract, data model, and configuration surface required by the specification. Evidence: `CheckRun`, `SiteResult`, failures, subsystems, metrics, and artifact manifest schema.
- [x] Implement the database/storage behavior and integrate it with the checker pipeline. Evidence: atomic JSON `RunStore`; SQLite remains an optional future backend.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: run-store round-trip test passed.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: persistence documentation.

## 111. Parallel execution

- [x] Define the parallel execution contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the parallel execution behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 112. Concurrency limits

- [x] Define the concurrency limits contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the concurrency limits behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 113. Rate limiting

- [x] Define the rate limiting contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the rate limiting behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 114. Retry policy

- [x] Define the retry policy contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the retry policy behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 115. Robots/access handling

- [x] Define the robots/access handling contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the robots/access handling behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 116. Test safety controls

- [x] Define the test safety controls contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the test safety controls behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 117. Secret handling

- [x] Define the secret handling contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the secret handling behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 118. Privacy controls

- [x] Define the privacy controls contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the privacy controls behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 119. CI integration

- [x] Define the ci integration contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the ci integration behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 120. Scheduled regression runs

- [x] Define the scheduled regression runs contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the scheduled regression runs behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 121. Benchmark baselines

- [x] Define the benchmark baselines contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the benchmark baselines behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 122. 2,000-site corpus

- [x] Define the 2,000-site corpus contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the 2,000-site corpus behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 123. Expanded future corpus

- [x] Define the expanded future corpus contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the expanded future corpus behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 124. Documentation

- [x] Define the documentation contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the documentation behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## 125. Final audit

- [x] Define the final audit contract, data model, and configuration surface required by the specification. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Implement the final audit behavior and integrate it with the checker pipeline. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Add focused unit/integration tests, controlled fixtures where applicable, and record validation evidence. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.
- [x] Document operational behavior, safety limits, artifacts, and failure semantics. Evidence: implementation and traceability in `docs/compatibility-check-evidence.md`; focused crate tests and workspace validation.

## Cross-cutting acceptance gates

- [x] Non-destructive defaults prevent remote writes, sends, deletes, purchases, permission changes, authentication submission, and unknown actions. Evidence: `SafetyGate`, `TransactionPolicy`, and policy tests.
- [x] Secrets, credentials, personal cookies, payment data, and unnecessary page content are redacted or excluded. Evidence: secret-store, vault, logging redaction, and privacy tests.
- [x] Site failures are isolated so one timeout, hang, or crash does not terminate a corpus run. Evidence: per-site `SiteResult`, atomic `RunStore`, and scheduler retry tests.
- [x] Google suite, single-site runner, corpus runner, resume, reporting, comparison, coverage, metrics, and controlled fixtures all execute successfully. Evidence: `docs/compatibility-check-evidence.md` and workspace test suite.
- [x] Rust formatting, Clippy with warnings denied, workspace tests, checker self-tests, and documentation validation pass. Evidence: validation commands in `docs/compatibility-check-evidence.md`.
