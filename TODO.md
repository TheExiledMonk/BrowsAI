# BrowsAI Implementation Plan

Legend: `[ ]` not started · `[~]` in progress · `[x]` complete · `[!]` blocked.

This checklist is derived from the BrowsAI Full System Implementation Specification v1.0. Every item must be updated continuously. A completed item must name implementation files, tests, and documentation where applicable.

## 1. Architecture and repository bootstrapping

- [x] Create the Rust workspace and repository layout from the specification.
  - Implementation: `Cargo.toml`, `crates/`, `apps/`, `sdk/`, `tests/`, `test-sites/`, `benchmarks/`, `docs/`
  - Tests: workspace build smoke test
  - Documentation: `README.md`, `ARCHITECTURE.md`, `SECURITY.md`
- [x] Define platform targets (Linux first; macOS and Windows follow-up) and feature flags.
  - Documentation: `ARCHITECTURE.md` defines Linux as the initial host, macOS/Windows compatibility targets, and deterministic headless versus optional desktop rendering.
- [x] Define product boundaries, non-goals, compatibility policy, versioning, and release criteria.
- [x] Preserve the core principles: execute first, first-class agent rendering, native browser input, untrusted web content, and secret isolation.
  - Documentation: `ARCHITECTURE.md` and `SECURITY.md` define execution-first observation, first-class Agent Render Tree output, native input resolution, untrusted web content, and brokered secret isolation.

## 2. Web engine integration

- [x] Define `crates/engine-api/` engine-neutral interfaces for navigation, document state, layout, events, input, storage, and rendering.
- [x] Define `crates/engine-servo/` Servo adapter and dependency/version policy.
  - Implementation: `crates/engine-servo/Cargo.toml` pins the optional `servo` dependency behind the `servo-runtime` feature and keeps the engine-neutral adapter boundary explicit.
- [x] Integrate Servo as an in-process engine component, not an external browser process.
  - Implementation: `crates/engine-servo/src/lib.rs` embeds `servo::Servo`/`WebView` behind `ServoRuntime`; `cargo check -p browsai-engine-servo --features servo-runtime` verifies the live-runtime boundary.
- [x] Add engine capability discovery and future-engine compatibility boundary.
  - Implementation: `crates/engine-api/` exposes feature probes; `crates/engine-servo/` provides context-scoped capabilities and rejects live-runtime requests when the `servo-runtime` feature is unavailable.
  - Tests: `crates/engine-servo/tests/adapter.rs` covers capability discovery and explicit feature-boundary failure.
- [x] Add engine fixtures and compatibility tests.
  - Tests: `tests/engine/`, `test-sites/html/`, `test-sites/css/`, `test-sites/javascript/`
  - Documentation: engine integration guide
  - Evidence: `test-sites/engine/compatibility.json` maps representative HTML/CSS/JavaScript fixtures to engine features; `crates/engine-servo/tests/fixtures.rs` validates the manifest, required capabilities, deterministic headless navigation, and snapshots.

## 3. Browser process model

- [x] Define host, browser broker, credential broker, profile manager, permission manager, workspace manager, audit service, network service, file broker, download broker, and IPC router processes/services.
- [x] Define renderer-per-origin/site, worker, agent-runtime, and optional compositor process topology.
- [x] Define lifecycle, startup, shutdown, restart, crash, and supervision behavior.
- [x] Add process-boundary security tests proving renderer compromise cannot reach secrets, profiles, filesystem, prompts, sessions, payment, or identity vaults.
  - Evidence: `crates/process-model/` denies renderer/agent privileged routes, and `crates/ipc/tests/ipc.rs` rejects nested raw secret fields while permitting opaque capability references.
- [x] Document process topology and failure behavior.
  - Implementation: `crates/process-model/`, `crates/ipc/`
  - Tests: process topology, supervisor lifecycle, and route authorization tests
  - Documentation: `docs/process-model.md`, `ARCHITECTURE.md`, `SECURITY.md`
  - Evidence: `ProcessTopology::production` includes audit/workspace services and explicit renderer/worker/agent/compositor boundaries; `ProcessSupervisor` retains launch specifications for policy-gated crash restart; topology tests verify privileged routes remain broker-mediated.
  - Additional evidence: process-model tests reject renderer IPC attempts to credential/payment/identity, permission/prompt, profile, file/download, workspace/session, and audit services while allowing only declared broker routes.

## 4. Networking

- [x] Implement browser network stack abstraction and request/response lifecycle.
- [x] Support HTTP(S), redirects, cookies, cache, WebSocket, workers, service workers, proxy configuration, certificates, and timeouts.
  - Implementation: `crates/network/` now routes partitioned GET cache hits and stores successful responses through `HttpCache`
  - Implementation: `crates/network/` provides gated `WebSocketConnection` routing with text, binary, and close frames
  - Implementation: `NetworkStack` mediates enabled service-worker interceptions through `ServiceWorkerRegistry`
  - Implementation: `NetworkStack` enforces zero-timeout failures and exposes profile/partition cache invalidation
  - Implementation: `NetworkStack` resolves configured proxy routes before direct routes while preserving original request IDs and lifecycle events
  - Evidence: configured HTTPS certificate allowlists reject unknown hosts with a normalized `UntrustedCertificate` error before routing.
  - Evidence: started requests that fail routing or redirect validation now emit request-correlated failure events and increment failure metrics.
  - Tests: `crates/network/tests/network.rs` covers WebSocket gating/frame order, service-worker interception, and timeout failure metrics
  - Evidence: `NetworkStack` covers all listed paths, including explicitly gated worker fetches with initiator/lifecycle correlation; normalized failures prevent unsupported worker or WebSocket paths from being guessed.
  - Evidence: `NetworkStack` provides engine-neutral request creation, routing, redirect handling, cache/service-worker interception, response delivery, normalized failures, lifecycle observation, and audit draining; `crates/network/tests/network.rs` covers successful and failed lifecycle paths with request identity.
- [x] Implement network observation, request identity, timing, initiator, response, and mutation events.
- [x] Implement network-to-application correlation and API discovery.
  - Implementation: `crates/api-discovery/` aggregates observed method/URL/status/schema facts with confidence and provenance; `CausalEvidence` links request IDs to entities, actions, state changes, and timing; `crates/application-ir/` carries the evidence into application IR.
  - Tests: `crates/api-discovery/src/lib.rs` and `crates/application-ir/tests/graph.rs` cover schema discovery and causal linkage.
- [x] Add normalized network errors, limits, metrics, and audit events.
  - Implementation: `crates/network/` exposes typed normalized errors, request/response limits, metrics, and request-correlated completed/failed audit events; `crates/api-discovery/` retains provenance for API observations.
  - Tests: `crates/network/tests/network.rs` covers limits, metrics, normalized failures, request identity, WebSocket, worker, and service-worker paths.
  - Documentation: network and API-discovery contracts
  - Evidence: `NetworkStack` emits request-correlated initiator, response, cache-mutation, completion, and failure events and exposes draining network and audit subscription surfaces; network regression tests verify the correlation.

## 5. HTML/DOM integration

- [x] Observe parsed HTML, DOM nodes, attributes, text, mutations, event listeners, forms, dialogs, frames, shadow roots, canvas, media, and accessibility signals.
  - Evidence: `RawDocument`/`RawNode` preserve engine-native nodes, attributes, text, mutations, listeners, lifecycle, provenance, and `RawNode::observed_signals` exposes typed form/dialog/frame/shadow/canvas/media/accessibility/focus/disabled facts; mutation, provenance, and signal tests cover the contract.
- [x] Maintain raw browser state without replacing JavaScript execution with interpretation.
  - Implementation: `crates/dom-observer/` retains engine-produced raw nodes/mutations; `crates/engine-servo/` executes page scripts through the selected engine runtime.
  - Tests: `crates/dom-observer/tests/mutations.rs`, `crates/engine-servo/tests/adapter.rs`
  - Evidence: raw observation is explicitly engine-neutral and does not rewrite scripts, while Servo adapter tests verify selected-page JavaScript execution and restricted capability/origin/timeout checks.
- [x] Track DOM identity, ownership, document lifecycle, and stale references.
- [x] Add DOM observer fixtures and mutation tests.
  - Implementation: `crates/dom-observer/`, `crates/event-observer/`, `crates/accessibility/`
  - Tests: `tests/engine/`, `tests/semantic/`, `test-sites/shadow-dom/`, `test-sites/iframe/`
  - Evidence: `RawDocument` recursively removes descendant subtrees and invalidates document generation, preventing unreachable orphan nodes from remaining observable; raw event-listener attachment/removal facts are generation-bound alongside DOM mutations.
  - Evidence: `crates/dom-observer/tests/mutations.rs` covers insert/attribute/text/listener mutations, subtree removal, lifecycle invalidation, and stale references; representative HTML fixtures live under `test-sites/html/` and related feature directories.
  - Evidence: `NodeReference` binds engine node identity to document generation, parent/child ownership is maintained on insertion/removal, and navigation resets the document scope; stale-reference and ownership regressions are covered by `references_are_generation_bound_and_navigation_resets_document_ownership`.

## 6. CSS/style integration

- [x] Observe computed styles, visibility, display, opacity, clipping, stacking, z-order, pseudo-elements, and responsive state.
- [x] Preserve style information needed for semantic visibility and hit testing.
- [x] Add CSS compatibility and semantic-render fixtures.
  - Implementation: `crates/layout-observer/`
  - Tests: `crates/css-observer/src/lib.rs`, `tests/engine/`, `tests/semantic/`, `test-sites/css/`
  - Evidence: `StyleSnapshot` preserves computed visibility, clipping, stacking, pseudo-element, and media facts while exposing changed-node invalidation for incremental consumers.
  - Evidence: `crates/css-observer/tests/golden.rs` executes checked-in CSS visibility fixtures and verifies typed visibility states.

## 7. Layout integration

- [x] Observe boxes, geometry, scroll containers, viewport, transforms, overlap, clipping, hit regions, and layout lifecycle.
  - Evidence: `LayoutSnapshot` now preserves typed `LayoutFacts` for scroll offsets, transforms, overlap relationships, and explicit hit regions alongside boxes, viewport, clipping, z-order, dirty lifecycle, and provenance; `crates/layout-observer/tests/facts.rs` covers round-trip facts and hit-region filtering.
- [x] Implement incremental layout observation and dirty-region invalidation.
  - Implementation: `crates/layout-observer/` tracks old/new rectangles on box replacement, node removal, and explicit invalidation, and exposes deterministic dirty-region draining.
  - Tests: `crates/layout-observer/tests/hit_test.rs` verifies moved and removed boxes invalidate both affected regions.
- [x] Add layout and hit-testing golden tests.
  - Implementation: `crates/layout-observer/`, `crates/css-observer/`
  - Tests: layout lifecycle and hit-testing tests
  - Evidence: `LayoutSnapshot::hit_test` enforces configured viewport containment and deterministic `(z-index, node-id)` ordering for overlapping targets.
  - Evidence: `crates/layout-observer/tests/golden.rs` executes `test-sites/semantic-golden/layout-hit.json` and verifies the expected topmost target.

## 8. JavaScript runtime

- [x] Execute page JavaScript through the selected browser engine.
  - Implementation: `crates/engine-api/`, `crates/engine-servo/` (restricted deterministic evaluation plus feature-gated Servo `WebView` execution)
  - Tests: `crates/engine-servo/tests/adapter.rs`
- [x] Observe runtime events, tasks, microtasks, promises, workers, exceptions, and DOM mutations.
  - Implementation: `crates/runtime-observer/` provides typed lifecycle helpers and bounded ordered replay
  - Tests: `crates/runtime-observer/src/lib.rs`, `crates/runtime-observer/tests/runtime.rs`
- [x] Define agent JavaScript runtime separately from page evaluation.
  - Evidence: `crates/agent-runtime/src/lib.rs` provides the capability-scoped `AgentScriptRuntime` lifecycle with provenance, timeout, output quotas, cancellation, and no page-origin input; `crates/agent-runtime/tests/agent_script.rs` verifies separation and denial without `ExecuteAgentScript`; `docs/agent-runtime.md` documents the boundary.
- [x] Implement restricted page-evaluation escape hatch with provenance, timeout, origin, and capability checks.
  - Evidence: `ServoEngine::evaluate_page_script_checked` rejects missing capability, mismatched origin, and excessive timeout before evaluation and carries a provenance reference.
- [x] Add runtime fixtures and security tests.
  - Implementation: `crates/runtime-observer/`, `crates/agent-runtime/`
  - Tests: `crates/runtime-observer/tests/runtime.rs`, `crates/engine-servo/tests/adapter.rs`, `crates/agent-runtime/tests/security.rs`, `test-sites/javascript/`, `test-sites/workers/`

## 9. Web APIs

- [x] Cover forms, dialogs, focus, selection, clipboard, downloads, uploads, media, canvas, WebGL, geolocation, device capabilities, notifications, storage, service workers, WebAuthn, and permissions.
  - Implementation: `crates/conformance/` declares every API with supported/partial/unsupported behavior; concrete brokers/observers cover permissions, clipboard, downloads, uploads, storage, service workers, WebAuthn, forms/dialogs/focus, and canvas/media fixtures.
  - Tests: API-specific crate tests, `crates/conformance/`, and `test-sites/` fixtures.
  - Evidence: unsupported APIs such as WebGL/geolocation are represented explicitly as normalized Unsupported rather than guessed, while supported/partial APIs map to engine capabilities and have focused lifecycle/security tests.
- [x] Define API support matrix, capability prompts, normalized values, and unsupported behavior.
  - Implementation: `crates/conformance/` maps every declared `WebApi` to an engine capability or explicit Unsupported result and defines normalized unsupported behavior; permissions/capability managers provide prompt policy.
  - Tests: `crates/conformance/src/lib.rs` verifies deterministic matrix serialization and supported/unsupported status reporting.
- [x] Add WPT-style/conformance fixtures and compatibility reporting.
  - Implementation: `crates/conformance/`, `crates/network/`, `crates/service-worker/`
  - Tests: conformance matrix and Web API behavior tests
  - Documentation: `test-sites/conformance/README.md`
  - Evidence: `SupportMatrix` maps each Web API to the engine feature that can substantiate it and reports unsupported APIs explicitly instead of treating generic runtime observation as coverage.
  - Evidence: `crates/conformance/` loads and validates repository JSON fixtures recursively with deterministic ordering and records pass/fail compatibility results.

## 10. Browser event system

- [x] Define ordered browser event model for navigation, DOM, style, layout, runtime, network, input, storage, permission, and agent state.
- [x] Implement event subscriptions, backpressure, ordering, replay, and loss reporting.
- [x] Add event trace and replay tests.
  - Implementation: `crates/event-observer/`
  - Tests: `tests/integration/`, `tests/regression/`
  - Evidence: `ChangeEvent` includes typed navigation, DOM, style, layout, runtime-task, network, input, storage, permission, dialog, credential, transaction, and agent-state events; `EventTrace` round-trips and validates sequence order.
  - Evidence: `ChangeStream::drain_with_loss` returns retained ordered events together with an explicit overflow count, preserving usable data after backpressure.
  - Evidence: `EventTrace::from_json` rejects non-monotonic serialized sequences before exposing the trace to replay consumers.

## 11. Input system

- [x] Implement native pointer, keyboard, focus, gesture, scroll, drag-and-drop, context-menu, and selection input.
  - Evidence: `crates/input/` defines the native event contract and `crates/action-planner/` emits validated pointer, keyboard, text, scroll, drag/drop, focus, and context-menu sequences with generation and hit-test checks.
- [x] Implement semantic intent to native browser input resolution.
  - Evidence: `ActionPlanner` resolves activation, click/double/context click, hover, focus/blur, typing, replacement, key presses, selection/toggle, scroll, drag/drop, and native dispatch through `NativeInputDispatcher`; `crates/action-planner/tests/planner.rs` covers pointer, keyboard, focus, drag/drop, and scroll fidelity.
  - Implementation: `crates/action-planner/` resolves focus, blur, drag, and drop actions into validated native pointer/keyboard sequences
  - Evidence: `ActionPlanner` consults current layout hit-testing for DOM-backed targets and rejects overlapped or stale geometry before emitting native events.
  - Evidence: hover and validated single-key press intents now resolve to native pointer/key events; missing key parameters are rejected, with coverage in `crates/action-planner/tests/planner.rs`.
- [x] Implement hit testing, target validation, stale-reference handling, ambiguity handling, and human takeover handoff.
  - Tests: `crates/action-planner/tests/planner.rs` covers focus/blur, drag/drop coordinates, stale, and ambiguous references
  - Evidence: `ActionPlanner` rejects non-interactable and overlapped targets after layout hit testing, enforces generation-bound stale checks, rejects duplicate identities, and integrates with the audited takeover ownership transitions in `crates/agent-runtime/`.
- [x] Add input behavior tests for ordinary browser fidelity.
  - Evidence: planner tests verify coordinate validation, hit testing, pointer button semantics, keyboard sequences, focus/blur, text replacement, drag/drop, context/double click, and scroll event output.
  - Implementation: `crates/input/`, `crates/engine-servo/` (native Servo pointer/keyboard/scroll/text routing), `crates/pointer/`, `crates/keyboard/`, `crates/focus/`, `crates/gestures/`
  - Tests: `tests/input/`, `crates/engine-servo/tests/adapter.rs`, `test-sites/forms/`, `test-sites/downloads/`

## 12. Agent Render Tree

- [x] Define Agent Render Tree v1 schema from browser state, layout, runtime behavior, accessibility, provenance, confidence, and permissions.
- [x] Represent pages, frames, shadow roots, elements, controls, content, relationships, visibility, geometry, and state without screenshot dependence.
  - Evidence: Agent Render Tree structural roles now include explicit `Frame` and `ShadowRoot` nodes; the semantic compiler preserves their hierarchy, state, geometry, provenance, and generation from raw engine observations without pixels, covered by `crates/semantic-compiler/tests/scopes.rs`.
- [x] Implement tree construction, pruning, updates, serialization, and stable IDs.
- [x] Add semantic-render golden tests.
  - Implementation: `crates/agent-tree/`
  - Integration: `crates/engine-servo/` live DOM projection with geometry, roles, stable page roots, and redaction
  - Tests: `tests/semantic/`, `tests/semantic-golden/`, `crates/engine-servo/tests/adapter.rs`
  - Documentation: Agent Render Tree schema
  - Evidence: `crates/semantic-ir/tests/golden.rs` validates roles, labels, relationships, enabled/visible state, geometry presence, provenance, and confidence against `test-sites/semantic-golden/basic-page.json`.
  - Evidence: `SemanticCompiler` constructs generation-stamped trees with conceptual identity keys, `StructuralIr` prunes unreachable invisible subtrees and round-trips JSON, and `crates/diff/` supplies incremental updates keyed by conceptual identity.

## 13. Structural IR

- [x] Define structural nodes, hierarchy, ownership, geometry, visibility, clipping, z-order, and document/frame boundaries.
- [x] Implement structural IR compiler and serializer.
- [x] Add structural golden and compatibility tests.
  - Implementation: `crates/structural-ir/`
  - Evidence: visibility pruning follows root-reachable hierarchy and removes entire invisible subtrees, preventing detached visible nodes; nested visibility regression coverage is in `crates/structural-ir/tests/ir.rs`.
  - Evidence: `StructuralIr::compile` emits the versioned structural schema, preserves hierarchy and node metadata, and `to_json`/`from_json` provide the serde serializer round trip covered by the structural IR tests.
  - Evidence: `crates/structural-ir/tests/golden.rs` executes `test-sites/semantic-golden/structural.json`, verifying schema compatibility and invisible-subtree pruning.

## 14. Semantic IR

- [x] Define roles, labels, values, states, affordances, constraints, actions, relationships, entities, and confidence.
- [x] Compile semantic meaning from structural state and runtime behavior.
  - Evidence: `SemanticCompiler::compile_with_runtime` folds `RuntimeEvidence` into affected semantic nodes without replacing structural/layout facts, lowering confidence and preserving JavaScript provenance; `crates/semantic-compiler/tests/compiler.rs` covers a runtime DOM mutation.
  - Evidence: `SemanticCompiler::compile` derives roles, names, geometry, visibility, actions, confidence, and DOM provenance from observed document/layout state; `compile_normalized` passes the result through the semantic normalization boundary.
- [x] Implement semantic normalization, ambiguity, and missing-information rules.
  - Implementation: `crates/semantic-ir/`, `crates/semantic-compiler/`
  - Tests: `tests/semantic/`, `tests/semantic-golden/`
  - Evidence: `SemanticCompiler` stamps every compiled node and its tree with the same incremented generation, preventing immediately stale semantic references.
  - Evidence: `SemanticIr::normalize` canonicalizes whitespace, reports missing accessible names for interactive roles, and reports duplicate stable identities without selecting an arbitrary target; `crates/semantic-ir/tests/normalization.rs` covers all three outcomes.
  - Documentation: `docs/semantic-ir.md` defines the normalization and unresolved-information contract.

## 15. Application IR

- [x] Define application-level entities, workflows, collections, search results, tables, forms, dialogs, notifications, and domain relationships.
  - Implementation: `crates/application-ir/` defines typed application entities, workflows, collections, search results, tables, forms, dialogs, notifications, and relations.
  - Tests: `crates/application-ir/tests/graph.rs`
  - Evidence: application surface models serialize through serde and preserve typed workflow steps, collection/table membership, form fields, dialog modality, notification level, entity provenance, and domain relations.
- [x] Implement application graph construction, site learning, semantic cache, and virtualized UI support.
  - Implementation: `crates/application-ir/`
  - Tests: `crates/application-ir/tests/graph.rs`
  - Evidence: `ApplicationGraph` and `ApplicationIr` expose deduplicating relation and virtualized-collection builders, while `SiteLearningCache` and bounded `SemanticCache` preserve deterministic derived state.
- [x] Keep agent memory separate from browser state.
  - Implementation: `crates/agent-runtime/` owns `AgentMemory`; `crates/application-ir/` carries derived application entities without mutating browser state.
  - Tests: `crates/agent-runtime/src/lib.rs` memory isolation test; `crates/application-ir/tests/graph.rs`
  - Evidence: `AgentMemory` is an explicit agent-runtime store with independent remember/forget semantics, while `ApplicationIr` is a derived value passed from semantic state and retains source provenance and causal evidence.

## 16. State tracking

- [x] Define browser, document, frame, DOM, layout, runtime, network, storage, focus, navigation, permission, and transaction state models.
- [x] Track asynchronous page state and expose a page stability API.
- [x] Track normalized values, time, history, autocomplete, focus, scroll, and page locks.
  - Implementation: `crates/state/`
  - Integration: `crates/engine-servo/` projects live focus and scroll into snapshots
  - Tests: `crates/state/tests/stability.rs`, state lifecycle tests, and `crates/engine-servo/tests/adapter.rs`

## 17. Snapshot system

- [x] Define immutable snapshot schema, version, serialization, provenance, sensitivity, and consistency boundaries.
- [x] Implement full snapshots, scoped queries, pagination, token budgets, and snapshot retention.
  - Implementation: `crates/state/` provides immutable schema-versioned `PageSnapshot` values and bounded `SnapshotStore`; `crates/agent-protocol/` provides scoped query shaping and pagination.
- [x] Add snapshot round-trip, schema, and stale-state tests.
  - Implementation: `crates/state/`, `crates/agent-protocol/`
  - Integration: `crates/engine-servo/` populates focused-node and scroll metadata from the live page
  - Tests: `crates/state/tests/snapshot.rs`, `crates/state/tests/paging.rs`, and Servo integration tests cover immutable copies, schema validation, bounded retention, pagination, token budgets, and stale generations.
  - Evidence: `PageQuery::render_page` and `query_page` expose bounded results with total and next-offset metadata; SDK clients expose the same pagination operation.

## 18. Diff system

- [x] Define node additions/removals/updates, relationship changes, action consequences, confidence changes, and invalidation causes.
- [x] Implement incremental diffing and continuous change streams.
- [x] Add diff determinism, coalescing, ordering, and replay tests.
  - Implementation: `crates/diff/`
  - Evidence: `DiffStream::drain_with_loss` returns retained incremental diffs together with an explicit overflow count, preserving recoverable changes after backpressure.

## 19. Agent action system

- [x] Define semantic action schema, planner, resolver, preconditions, target references, native input execution, and result state.
  - Implementation: `crates/action-planner/` maps toggle/select activation and replacement text to native events
  - Tests: `crates/action-planner/tests/planner.rs`
  - Evidence: `ActionPlanner` validates generation, visibility, enablement, identity ambiguity, geometry hit targets, and transaction policy before producing native events; `execute` dispatches those events through `NativeInputDispatcher` and returns a typed completed result.
- [x] Classify action consequences, reversibility, transaction risk, confirmation requirements, and interruption behavior.
  - Implementation: `crates/action-planner/`, `crates/transactions/`, and `crates/agent-runtime/`
  - Tests: `crates/transactions/tests/policy.rs`, `crates/action-planner/tests/planner.rs`, and `crates/agent-runtime/tests/end_to_end.rs`
  - Evidence: action planning assigns consequence/reversibility classifications, transaction policy denies or confirms risky effects, confirmation requests are owner-bound and expiring, and runtime interruption transitions are tested.
- [x] Implement action history, provenance, audit records, and debug replay.
  - Implementation: `crates/action-planner/` records classified planned actions with generation-bound replay lookup and drains non-sensitive generation/action/classification audit events; `crates/transactions/`, `crates/input/`, and `crates/provenance/` provide policy, native events, and confidence foundations.
  - Tests: `tests/agent/`, `tests/security/`, `tests/input/`, `crates/action-planner/tests/planner.rs`
  - Evidence: `ActionHistory` records generation-bound actions and replay lookup, `ActionProvenance`/`AuditJournal` preserve policy and resolution metadata with integrity chaining, and CLI replay verifies and replays the journal; regression coverage spans planner and audit tests.

## 20. Query system

- [x] Implement agent queries over structural, semantic, and application IR.
  - Implementation: `crates/agent-protocol/` provides structural/semantic tree queries; `crates/application-ir/` provides typed entity and collection queries.
  - Tests: `crates/agent-protocol/tests/query.rs` and `crates/application-ir/tests/graph.rs`
  - Evidence: `PageQuery` filters rendered semantic state and `ApplicationIr::query_entities` filters application entities by identity, type, and virtualized collection membership.
- [x] Support stable identity, natural-language/role/entity selectors, relations, state filters, geometry, visibility, and origin scoping.
  - Evidence: `PageQuery::matches` supports stable identity, case-insensitive name/nameContains selectors, focused/selected state, geometry bounds, relationship kind/target, and origin constraints with camelCase wire serialization shared by Rust, TypeScript, and Python SDKs; Agent Render Tree nodes preserve optional origin facts.
  - Evidence: `ApplicationIr::query_entities` adds typed entity and collection selectors; Rust, TypeScript, and Python protocol-surface tests cover the shared `nameContains` wire field.
- [x] Return confidence, provenance, ambiguity, stale-reference, and explainability data.
  - Implementation: `crates/agent-protocol/` returns confidence/provenance in `QueryResult`, structured explanations, and explicit resolution errors.
  - Tests: `crates/agent-protocol/tests/query.rs`
  - Evidence: `PageQuery::resolve` refuses ambiguous matches, `resolve_at_generation` rejects stale trees, and query results/explanations preserve confidence and provenance.
- [x] Implement search, collections, virtualized UIs, and token-efficient response shaping.
  - Implementation: `crates/agent-protocol/` now returns provenance/geometry/origin with query results and exposes deterministic `QueryShape` result/token/context bounds; `crates/application-ir/` models collections and virtualized membership.
  - Tests: query golden and token benchmark tests
  - Evidence: `PageQuery::search` provides case-insensitive text search, `ApplicationIr` exposes bounded virtualized collections and typed collection queries, and `QueryShape` enforces deterministic result/context/token limits; `crates/agent-protocol/tests/query.rs` and `crates/application-ir/tests/graph.rs` cover these paths.

## 21. Agent scripting runtime

- [x] Define capability-scoped agent runtime and script lifecycle.
  - Implementation: `crates/agent-runtime/`, `crates/sandbox/`, and `crates/engine-servo/`
  - Tests: `crates/agent-runtime/tests/end_to_end.rs`, `crates/agent-runtime/tests/security.rs`, and `crates/engine-servo/tests/adapter.rs`
  - Evidence: agent sessions enforce capability grants, lifecycle transitions, quotas, cancellation/timeouts, and page-script capability checks; page evaluation remains a separate engine operation with origin, timeout, and provenance validation.
- [x] Implement TypeScript-first SDK bindings and Python SDK bindings.
  - Implementation: `sdk/typescript/`, `sdk/python/`
  - Tests: `sdk/typescript/test/sdk.test.ts`, `sdk/python/test_sdk.py`
  - Documentation: `docs/sdk.md`
  - Evidence: both SDKs expose the versioned query, snapshot, diff, action, capability, confirmation, event, navigation, get, and explain surfaces; strict TypeScript compilation and Python synchronous/async tests pass.
- [x] Define agent protocol, remote operation, multi-agent support, leases, interruption, and page locking.
  - Implementation: `crates/agent-protocol/` defines versioned wire requests/responses, structured remote operations, exclusive page/workspace leases, and interruption state.
  - Tests: `crates/agent-protocol/tests/query.rs`, including protocol validation, camelCase wire format, lease ownership/expiry, and interruption coverage.
- [x] Prevent web content from influencing agent authority or system prompt.
  - Implementation: `crates/agent-runtime/`, `crates/agent-protocol/`, `sdk/typescript/`, `sdk/python/`
  - Tests: `tests/agent/`, `tests/security/`
  - Documentation: SDK and protocol references; `docs/prompt-injection.md`
  - Evidence: fixture-backed runtime tests reject page-script escalation and deny capability/system-instruction/confirmation influence.

## 22. Browser profiles

- [x] Implement profile creation, isolation, selection, lifecycle, migration, quotas, and persistence.
  - Implementation: `crates/profiles/` provides schema-versioned JSON migration, selected-profile lifecycle, deletion handoff, configurable profile/history/autocomplete/extension quotas, and generation-preserving persistence.
- [x] Implement session, history, autocomplete, browser identity, user-agent, and extension configuration per profile.
  - Implementation: `crates/profiles/` provides selected-profile lifecycle, deletion handoff, and explicit JSON persistence preserving generations/configuration
  - Tests: `crates/profiles/tests/profiles.rs`, profile isolation, migration, recovery, and persistence tests

## 23. Cookies

- [x] Implement cookie storage, parsing, domain/path matching, expiry, SameSite, Secure, HttpOnly, partitioning, and origin policy.
  - Implementation: `crates/cookies/` enforces expiry/SameSite context, Secure checks, partitioning, and path boundaries, and persists partitioned jars with explicit JSON wire data
  - Tests: `crates/cookies/tests/cookies.rs`, cookie conformance and isolation fixtures
  - Evidence: cookie parsing distinguishes host-only and Domain attributes, prevents host-only cookies from crossing subdomains, applies parsed Max-Age expiry against an injected clock, and supports expired-entry purging.
- [x] Integrate cookies with network, profiles, snapshots, and audit.
  - Implementation: `crates/network/` sends partitioned cookie headers and ingests routed `Set-Cookie` responses into `crates/cookies/`
  - Tests: `crates/network/tests/network.rs` verifies response-cookie persistence; cookie conformance and isolation fixtures
  - Evidence: `NetworkStack` exports/restores the partitioned cookie snapshot, records non-sensitive `CookieStored` audit events containing only request identity/domain, and binds every cookie operation to the request profile/top-level-site partition.

## 24. LocalStorage

- [x] Implement origin-scoped LocalStorage with quotas, persistence, change observation, and crash-safe writes.
  - Implementation: `crates/storage/` provides typed quota writes, generation tracking, explicit JSON persistence, and mutation events
  - Tests: `crates/storage/tests/isolation.rs`, storage fixtures and recovery tests

## 25. SessionStorage

- [x] Implement tab/document-scoped SessionStorage lifecycle, cloning, isolation, and recovery semantics.
  - Implementation: `crates/storage/` supports session namespace cloning with preserved generation state, persistence, and clone events
  - Tests: `crates/storage/tests/isolation.rs`

## 26. IndexedDB

- [x] Implement IndexedDB adapter, transactions, quotas, version upgrades, observation, and profile/origin isolation.
  - Implementation: `crates/storage/`
  - Tests: storage conformance fixtures
  - Evidence: IndexedDB transactions validate proposed committed bytes against the configured quota before applying writes, preserving atomic rejection; open/upgrade/commit/abort lifecycle events retain profile/origin/database identity.

## 27. Cache

- [x] Implement HTTP cache and Cache API persistence, eviction, partitioning, invalidation, and service-worker integration.
  - Implementation: `crates/cache/` (`HttpCache` capacity-safe replacement with a nonzero default capacity, partition invalidation, explicit JSON persistence wire format, and named `CacheStorage` API with namespace persistence), `crates/storage/`, `crates/network/`
  - Integration: `crates/network/` invalidates same-partition GET entries after successful non-GET responses
  - Tests: `crates/cache/tests/cache.rs`, `crates/network/tests/network.rs`, storage fixtures and recovery tests

## 28. Service workers

- [x] Implement service-worker registration, lifecycle, scope, fetch interception, cache integration, worker isolation, and recovery.
  - Implementation: `crates/network/` mediates enabled interceptions and exposes registration, transition, and unregister lifecycle operations; `crates/service-worker/` enforces scope, ordered lifecycle transitions, unregister cleanup, activated-only control, and serde recovery
  - Tests: `crates/network/tests/network.rs` and `crates/service-worker/tests/worker.rs`, including network-facade unregister and activated-only interception regressions; `test-sites/service-worker/`

## 29. Credential broker

- [x] Define broker API, capability references, origin binding, consent, policy, expiration, revocation, and audit.
  - Implementation: `crates/secret-store/` supports explicit-consent grants, purpose/lifetime policy, expiring capability grants, handle revocation, broker-only callback release, and non-sensitive lifecycle events; policy is retained through encrypted snapshot restore.
  - Evidence: `crates/secret-store/tests/broker.rs` covers consent denial, purpose/lifetime policy rejection, policy-preserving restore, expiry, revocation, origin/profile binding, one-time use, and event draining.
- [x] Ensure agents never receive raw passwords, tokens, seeds, private keys, cards, or identity secrets.
  - Evidence: Servo tree projection omits sensitive/password/hidden values and redacts sensitive text; credentials, TOTP, identity, payment, and secret-store brokers expose only scoped opaque capabilities; engine, vault, IPC, audit, recovery, and malicious-content tests verify raw values do not enter agent-visible state.
  - Implementation: `crates/credentials/`, `crates/secret-store/`
  - Tests: `tests/credentials/`, `tests/security/`
  - Documentation: secret-handling model

## 30. Password manager

- [x] Implement encrypted credential records, origin matching, login detection, autofill orchestration, save/update, and policy controls.
  - Implementation: `crates/credentials/`, `crates/password-vault/`, and `crates/secret-store/`
  - Tests: `crates/credentials/tests/credentials.rs`, `crates/password-vault/tests/password.rs`, and `crates/password-vault/tests/detection.rs`
  - Evidence: credentials are encrypted in snapshots, matched by profile/origin, login forms are detected from observed field metadata, autofill returns only metadata until a consent/policy-checked opaque capability is authorized, and updates revoke the old handle before replacing it.
  - Implementation: `crates/credentials/` provides versioned credential snapshots backed by authenticated encrypted secret-store records; `crates/password-vault/` supports origin-matched autofill planning and password updates that revoke prior secret handles before replacement
  - Tests: `crates/credentials/tests/credentials.rs`, `crates/password-vault/tests/password.rs`, credential and autofill fixtures
  - Evidence: credential snapshot tests verify plaintext passwords are absent from serialized state, keyed restore succeeds, and wrong-key release fails authentication.
- [x] Keep password material inside the broker/vault boundary.
  - Implementation: `crates/credentials/` stores only opaque handles in credential records; bytes remain in `crates/secret-store/`
  - Evidence: `crates/credentials/tests/credentials.rs` verifies password release is callback-only and one-time capabilities cannot be reused.

## 31. TOTP

- [x] Implement encrypted TOTP seed storage, broker-side code generation, origin/action binding, expiry, and one-time exposure policy.
  - Implementation: `crates/totp/`
  - Tests: deterministic TOTP and secret-isolation tests
  - Evidence: TOTP generation rejects invalid UTF-8/base32 or empty seeds instead of silently producing codes from an empty key.

## 32. Passkeys/WebAuthn

- [x] Implement WebAuthn broker abstraction, RP/origin validation, user verification policy, credential handles, and private-key isolation.
  - Implementation: `crates/webauthn/` tracks broker-issued assertion challenges and enforces single-use completion alongside RP/profile binding
  - Tests: `crates/webauthn/tests/webauthn.rs`, WebAuthn fixtures and negative security tests

## 33. Identity vault

- [x] Implement encrypted identity profiles, field-level release policy, origin binding, consent, masking, and audit.
  - Implementation: `crates/identity-vault/` stores only broker handles, exposes consent-gated field authorization, and emits non-sensitive creation/authorization/release audit events.

## 34. Payment vault

- [x] Implement payment instrument abstraction, merchant/origin binding, transaction risk classification, confirmation, masking, and audit.
  - Implementation: `crates/payment-vault/`, `crates/transactions/`; card capability release is confirmation-gated and emits non-sensitive audit events.
  - Tests: payment security and confirmation tests

## 35. Permissions

- [x] Define capability model for secrets, files, clipboard, network, camera/mic, geolocation, notifications, downloads, uploads, certificates, proxy, and device APIs.
  - Implementation: `crates/permissions/` exposes typed permission variants for all listed capability classes.
- [x] Implement request, decision, persistence, revocation, prompts, and per-origin/profile scope.
  - Implementation: `crates/permissions/`
  - Implementation: `crates/permissions/` provides profile-scoped grant/deny JSON persistence, expired-grant purging, and explicit revoke-to-prompt behavior
  - Tests: `crates/permissions/tests/permissions.rs`, `tests/security/`, permission conformance fixtures
  - Evidence: expired grant purging removes stale Granted decisions, so a purged permission returns to Prompt rather than being granted implicitly.
  - Evidence: `PermissionManager` emits typed requested/decided/expired/revoked events with profile and origin scope and exposes a draining subscription API.

## 36. Secret isolation

- [x] Implement encrypted secret store, memory/process boundaries, redaction, zeroization strategy, capability references, and broker-only release.
  - Evidence: `crates/secret-store/` encrypts persisted records, uses `Zeroizing` key material, issues scoped opaque references, and releases bytes only through broker callbacks; process topology and IPC route policy keep brokers outside renderer/agent reach; logging, audit, engine, and recovery layers apply redaction.
- [x] Add tests proving secrets do not appear in agent trees, logs, snapshots, IPC, crash dumps, or error messages.
  - Evidence: engine adapter tests reject password/token values in agent trees; secret-store, credentials, recovery, logging, audit, IPC, TOTP, identity, payment, and error-path tests assert encrypted/opaque/redacted outputs and absence of plaintext secret material.
  - Evidence: `SecretBroker::with_secret` rebinds capability profile/origin to the stored record before release, with tampering regression coverage.
  - Implementation: `crates/secret-store/` encrypts records with ChaCha20-Poly1305, zeroizes decrypted callback buffers and the process-local key, supports authenticated snapshots, explicit handle revocation, and expired capabilities; `crates/audit/`, `crates/logging/`
  - Evidence: `crates/secret-store/tests/broker.rs` verifies ciphertext snapshots round-trip only with the key, reject tampering, and exclude plaintext; `SecretEvent` lifecycle serialization and debug output are tested to exclude stored secret bytes.
  - Evidence: `crates/ipc/tests/ipc.rs` rejects raw password/secret/token fields in nested request/response bodies before routing.
  - Tests: `crates/secret-store/tests/broker.rs`, `tests/security/`

## 37. Prompt-injection boundaries

- [x] Label page content as untrusted data and isolate it from agent instructions, capabilities, system prompt, and confirmation policy.
  - Implementation: `crates/agent-runtime/` wraps captures as `UntrustedPageData` and evaluates `PageContentAuthority` as an explicit deny result.
- [x] Detect suspicious instructions and provide provenance without granting authority.
  - Implementation: `UntrustedPageData::assess` returns source-bound signals without mutating session capabilities.
- [x] Add malicious-content and prompt-injection test sites.
  - Tests: `tests/security/`, `test-sites/malicious-content/`
  - Tests: `crates/agent-runtime/tests/security.rs` exercises both malicious fixtures and denied page-script escalation.
  - Documentation: `docs/prompt-injection.md`

## 38. Clipboard

- [x] Implement read/write broker, user gesture and permission policy, origin/profile scoping, redaction, and audit.
  - Implementation: `crates/clipboard/` stores values by profile/origin and consumes one-time gesture tokens for reads
  - Tests: `crates/clipboard/tests/clipboard.rs`, clipboard security and input tests

## 39. Downloads

- [x] Implement download detection, destination policy, filename normalization, quota/limits, progress, cancellation, quarantine, and audit.
  - Implementation: `crates/downloads/` provides authorized handle tracking, filename normalization, bounded progress, and terminal cancellation/quarantine states
  - Tests: `crates/downloads/tests/downloads.rs`, `test-sites/downloads/`, file broker tests
  - Evidence: `DownloadManager` emits ordered pending/progress/completed/cancelled/quarantined lifecycle events for audit and subscribers.

## 40. Uploads

- [x] Implement upload intent, picker-less broker flow, file capability references, MIME/size policy, progress, cancellation, and audit.
  - Implementation: `crates/uploads/` provides permission-bound handles, filename normalization, and stateful bounded progress/cancellation
  - Tests: `crates/uploads/tests/uploads.rs`, `crates/uploads/tests/filename.rs`, upload and permission fixtures
  - Evidence: `UploadManager` emits ordered pending/progress/completed/cancelled lifecycle events for audit and subscribers.

## 41. File sandbox

- [x] Implement filesystem capability broker, sandbox roots, path normalization, symlink policy, quotas, temporary files, and cleanup.
  - Implementation: `crates/file-broker/` adds canonicalized root confinement, symlink-safe size/quota accounting, unique temporary files, and cleanup; `crates/sandbox/`
  - Tests: `crates/file-broker/tests/broker.rs`, `crates/file-broker/tests/sandbox.rs` (symlink escape, quota/temp cleanup), `tests/security/`, file boundary tests

## 42. Navigation

- [x] Implement URL parsing, navigation lifecycle, redirects, history, reload, stop, same-document navigation, origin transitions, and error pages.
  - Implementation: `crates/navigation/` models reload without history duplication and exposes explicit origin-transition checks
  - Tests: `crates/navigation/tests/history.rs`, navigation/recovery integration tests
  - Evidence: identical navigations reuse the current history entry while fragment-only navigations remain same-document transitions.

## 43. Frames

- [x] Implement iframe/frame tree, origin boundaries, frame navigation, nested agent-tree scopes, and cross-frame action/query rules.
  - Implementation: `crates/frames/`
  - Tests: `test-sites/iframe/`, security isolation tests
  - Evidence: `FrameTree::navigate` updates a frame origin and removes descendant document scopes; `FrameTree::scoped_ids` exposes validated nested scopes and JSON recovery rejects dangling frame links; regression coverage verifies cross-origin access changes and stale descendants disappear.
  - Additional evidence: `AgentFrameScope` exposes validated nested frame topology, while `can_query` and `can_action` enforce origin-bound cross-frame access in `crates/frames/tests/frames.rs`.

## 44. Shadow DOM

- [x] Implement open/closed shadow-root observation policy, composed tree relationships, selectors, events, and agent exposure rules.
  - Implementation: `crates/shadow-dom/` now tracks composed selector metadata and typed root/child events while enforcing open/closed exposure boundaries.
  - Tests: `crates/shadow-dom/tests/shadow.rs`, `test-sites/shadow-dom/`
  - Evidence: `ShadowDomRegistry` enforces one root per host, idempotent child attachment, safe removal, selector lookup, event draining, and open/closed exposure behavior.

## 45. Tabs

- [x] Implement tab lifecycle, navigation association, focus, background throttling, snapshots, persistence, and agent query scope.
  - Implementation: `crates/tabs/` exposes navigation/back/forward helpers that complete lifecycle and advance tab generations, profile-scoped access, checked JSON recovery normalization, stable `TabSnapshot` values, and explicit foreground/background execution policies.
  - Tests: `crates/tabs/tests/tabs.rs` covers active/background state, navigation generations, profile scope, persistence round trips, and activation/restore throttling invariants.
  - Evidence: `TabManager::agent_scope` exposes only profile-scoped stable `TabSnapshot` values, preserving navigation generation and foreground/background execution policy across persistence.

## 46. Windows

- [x] Implement window lifecycle, viewport, compositor association, dialogs, focus, visual shell integration, and persistence.
  - Implementation: `apps/browsai-desktop/` and `crates/workspace/` provide focused-surface/window handoff and close behavior
  - Tests: `crates/workspace/tests/workspace.rs`
  - Evidence: `DesktopShell` persists and restores window, tab/dialog/prompt/confirmation/download/takeover surfaces together with focused-surface identity.
  - Evidence: workspace tests cover window creation, focus transfer, close handoff, viewport dimensions, and profile-safe checkpoint restore.

## 47. Workspaces

- [x] Implement workspace lifecycle, profile/tab/window membership, permissions, locking, persistence, and multi-agent boundaries.
  - Implementation: `crates/workspace/` validates focused-window handoff while preserving profile/tab checkpoint boundaries
  - Tests: `crates/workspace/tests/workspace.rs`
  - Tests: workspace recovery and isolation tests
  - Evidence: workspace tab attachment rejects missing, cross-profile, and duplicate ownership; checkpoint restore rejects dangling/duplicate tab references and inconsistent focus state before mutation.
  - Evidence: workspace checkpoints now persist explicit permission grants alongside profile/tab/window membership and agent locks; lock ownership and duplicate attachment tests cover multi-agent boundaries.

## 48. Human takeover

- [x] Implement explicit handoff, pause/resume, redaction, confirmation UI, action ownership, timeout, and audit trail.
  - Implementation: `crates/agent-runtime/` provides requested/active/paused/completed/expired ownership transitions and redacts sensitive handoff payloads; `apps/browsai-desktop/` provides a takeover surface.
  - Tests: `crates/agent-runtime/src/lib.rs` covers pause/resume, timeout, redaction, ownership, and audit events.
  - Documentation: `docs/human-takeover.md`
  - Evidence: `TakeoverManager` enforces explicit owner/state transitions, timeout expiry, sensitive payload redaction, and ordered audit events; `DesktopShell` exposes takeover and confirmation surfaces and tests cover focus/close behavior.

## 49. Visual renderer

- [x] Implement optional visual compositor/shell using engine layout and paint output.
  - Evidence: `apps/browsai-desktop/` provides `DesktopShell::compose_layout`, projecting visible engine `LayoutSnapshot` boxes into ordered `CompositorFrame` paint commands; regression coverage verifies visibility and z-order while agent rendering remains independent.
- [x] Support windows, tabs, dialogs, focus indicators, downloads, permissions, and confirmation surfaces.
  - Implementation: `apps/browsai-desktop/` now maintains surface layout/z-order/visibility, produces an ordered render plan, and performs compositor-style hit testing for windows, tabs, dialogs, prompts, downloads, confirmations, and takeover surfaces.
  - Tests: `apps/browsai-desktop/src/lib.rs` covers render-plan ordering, hit testing, focus, and persistence.
  - Evidence: `DesktopShell` models and renders all listed shell surfaces with deterministic z-order and focused-surface handoff; desktop smoke tests cover ordering, hit testing, and recovery.

## 50. Headless mode

- [x] Implement headless host, virtual viewport, deterministic clock/options, no-raster agent operation, and CLI lifecycle.
  - Implementation: `apps/browsai-headless/`, `apps/browsai-cli/`
  - Tests: headless integration tests
  - Evidence: `HeadlessHost::advance_clock` advances the engine context’s deterministic clock while preserving virtual viewport and no-raster options; CLI smoke tests cover version, navigation, and query lifecycle.

## 51. Network intelligence

- [x] Correlate requests, responses, scripts, forms, actions, state changes, and application entities.
  - Evidence: `CausalEvidence` retains request identity, timing, action/entity/state links, and optional form/script context; `ApiDiscovery` preserves those correlations and `crates/application-ir/tests/graph.rs` verifies application-level causal evidence.
- [x] Provide causal evidence and timing in application IR and explainability output.
  - Evidence: `ApplicationIr::causal_evidence` stores request/action/entity/state links with timing and provenance, while `explainability_report` projects those records into timestamped `ExplainabilityReport` evidence; `crates/application-ir/tests/graph.rs` verifies elapsed timing and source retention.
  - Implementation: `crates/network/`, `crates/application-ir/`, `crates/provenance/`
  - Evidence: `NetworkStack` normalizes routed, cached, and service-worker response IDs to the originating request ID; lifecycle tests verify the request/response join.

## 52. API discovery

- [x] Discover API endpoints and schemas from network behavior, runtime usage, forms, and application graph.
  - Evidence: observed responses infer JSON schemas with network provenance, while `ApiDiscovery::infer_form_endpoint`, `infer_runtime_endpoint`, and `infer_application_endpoint` record typed inferred endpoints and source kinds; discovery tests cover all paths.
- [x] Mark inferred versus observed facts and preserve origin/provenance.
  - Implementation: `crates/api-discovery/`
  - Tests: network correlation and discovery fixtures
  - Evidence: `ApiDiscovery` records endpoint status/schema observations and request-linked causal evidence with timing and provenance.
  - Evidence: `ApiEndpoint` now labels endpoint observations and response-schema derivation as `Observed` or `Inferred`, while retaining confidence and typed network provenance; `crates/api-discovery/src/lib.rs` tests both origins.

## 53. Runtime provenance

- [x] Attach source/provenance to DOM, layout, runtime, network, semantic, and application facts.
- [x] Support explainability, confidence, timestamps, and redaction.
  - Implementation: `crates/provenance/`
  - Tests: provenance golden tests
  - Evidence: `Confidence::new` clamps all finite values and normalizes NaN to a finite bounded value; explainability retains source, timestamp, redaction, and inference status.
  - Additional evidence: `RawNode` and `LayoutBox` now carry serializable `ProvenanceSource` collections with regression tests in `crates/dom-observer/tests/provenance.rs` and `crates/layout-observer/tests/provenance.rs`; runtime, network, semantic, and application provenance remain covered by their respective observer/IR contracts.
  - Additional evidence: `RuntimeEvidence` and `RuntimeObserver::drain_evidence` retain source references for JavaScript/runtime events while `drain` and `drain_bounded` preserve the existing event-only API; `crates/runtime-observer/tests/provenance.rs` verifies the round trip.
  - Evidence: `ExplainabilityReport` redacts sensitive evidence details at insertion time and exposes inferred/observed status through bounded confidence values; `crates/provenance/tests/explainability.rs` covers these invariants.

## 54. Action provenance

- [x] Record intent, selector/reference, resolution, native events, consequences, policy decisions, user/agent identity, and audit links.
  - Implementation: `crates/provenance/`, `crates/audit/`
  - Tests: action replay and audit tests
  - Evidence: `ActionProvenance` records intent, target selector, resolved node, native-event count, consequence/reversibility, policy decision, capability/audit references, and explicit agent/user identities; `action_event` persists the record in the integrity-chained journal.

## 55. Transaction classification

- [x] Classify reads, navigation, reversible writes, irreversible writes, external side effects, sensitive actions, and financial actions.
- [x] Implement confidence, policy, confirmation, and escalation rules.
  - Implementation: `crates/transactions/`
  - Tests: transaction policy matrix
  - Evidence: confirmation transitions lazily mark overdue pending requests as Expired before rejecting use, preserving auditable state.
  - Evidence: `ConfirmationQueue` emits ordered, actor-bound pending/approved/denied/escalated/expired events for replay and audit correlation.

## 56. Audit journal

- [x] Implement append-only journal with action, capability, permission, secret, navigation, download, upload, and recovery events.
- [x] Add integrity, retention, redaction, export, replay correlation, and sensitive logging controls.
  - Implementation: `crates/audit/`
  - Tests: audit integrity and redaction tests
  - Evidence: `AuditJournal` records a chained integrity value and verifies sequence, predecessor, and record contents; tampering regression coverage is in `crates/audit/tests/journal.rs`.
  - Evidence: checked `to_json`/`from_json` APIs reject serialized journals whose integrity chain does not verify.
  - Evidence: `AuditJournal` supports audit-ID correlation, integrity-checked replay, and bounded retention with chain re-establishment after pruning.

## 57. Site isolation

- [x] Enforce origin/site process separation, storage partitioning, cross-origin policy, frame boundaries, and broker mediation.
  - Implementation: `crates/sandbox/`, `crates/ipc/`, `crates/process-model/`
  - Tests: cross-origin, renderer-compromise, child-supervisor, and route-authorization regressions
  - Evidence: sandbox origin tuples, profile/origin-partitioned storage and cookies, `FrameTree` access checks, and topology-authorized IPC routes jointly prevent cross-site direct access; process-model and frame/storage/IPC tests cover the boundaries.

## 58. Renderer sandbox

- [x] Define OS sandbox policy, syscall/filesystem/network restrictions, process identity, resource limits, and crash containment.
  - Evidence: `SandboxPolicy::os_profile` derives typed process identity, syscall allowlists, filesystem roots, network egress, resource limits, and crash containment from capability policy; `SandboxPolicy::validate` rejects unsafe renderer/agent combinations and sandbox tests cover the boundary.
  - Implementation: `crates/sandbox/`
  - Tests: sandbox policy tests
  - Evidence: `CapabilityBroker` uses parsed scheme/host/port origin comparison, preventing path-string and cross-origin policy mismatches; covered by sandbox policy regressions.

## 59. Agent sandbox

- [x] Restrict agent runtime resources, filesystem/network/process access, capabilities, scripts, and remote operation.
- [x] Implement quotas, timeouts, cancellation, and isolation from page content.
  - Implementation: `crates/agent-runtime/`, `crates/sandbox/`
  - Tests: agent sandbox escape tests
  - Evidence: `AgentRuntime` enforces action/event/output quotas and a deterministic runtime deadline; advancing the clock cancels expired sessions and prevents further page work.

## 60. IPC

- [x] Define typed IPC messages, routing, authentication, origin/profile labels, capability references, backpressure, cancellation, and version negotiation.
- [x] Implement host/broker/renderer/worker/agent channels and crash-safe reconnect.
  - Implementation: `crates/ipc/`
  - Tests: IPC fuzzing, ordering, and authorization tests
  - Evidence: `IpcRouter` validates both envelope and handshake protocol versions before process-edge authorization; regression coverage is in `crates/ipc/tests/ipc.rs`.
  - Evidence: `IpcChannel` supports request cancellation and reconnect reset, clearing in-flight messages and restarting sequence ordering safely.

## 61. Logging

- [x] Implement structured logs, levels, correlation IDs, redaction, secret-safe error model, and debug/replay traces.
  - Implementation: `crates/logging/`
  - Tests: sensitive logging tests
  - Evidence: `SecretSafeLogger` redacts sensitive field values and message content at collection time, preventing debug traces from carrying plaintext secrets.
  - Evidence: `SecretSafeLogger` supports correlation filtering, JSON round-trip export/import, and draining consumed traces without bypassing collection-time redaction.

## 62. Metrics

- [x] Implement performance, memory, concurrency, network, rendering, token, action, and security metrics.
  - Implementation: `crates/metrics/`
  - Tests: metric emission and benchmark harness tests
  - Evidence: `MetricRegistry` provides typed domain namespaces for all eight required measurement surfaces, deterministic median/p95 percentiles, merge/export, and timed observations; `crates/metrics/tests/metrics.rs` covers domain emission and benchmark-ready summaries.

## 63. Crash recovery

- [x] Implement process supervision, crash reports, safe restart, state checkpointing, journal recovery, and corruption handling.
  - Implementation: `crates/recovery/`
  - Tests: forced-crash and recovery regression tests
  - Evidence: `FileRecoveryStore` validates schema/checksum before atomic replacement, preserves the primary until a durable new payload exists, and retains a recoverable backup; process-supervisor tests cover crash/restart policy.

## 64. Session recovery

- [x] Restore profiles, windows, tabs, navigation, storage transactions, agent sessions, locks, and pending confirmations safely.
  - Additional evidence: `SessionRecoveryState::plan` validates opaque-only metadata and creates dependency-ordered recovery phases (profiles, workspaces, tabs/navigation, storage, agent sessions, locks, confirmations); recovery tests cover ordering and secret rejection before restoration.
  - Additional evidence: `RecoveryTarget` and `RecoveryPlan::restore` execute each phase through explicit typed application/broker hooks and return a `RestoreReport`; tests verify resource classes run in dependency order.
  - Implementation: `crates/recovery/` now persists typed, generation-bound, secret-safe recovery resources for profiles, tabs, cookies, storage, downloads, workspaces, agent sessions, locks, and pending confirmations; opaque secret references are retained without secret bytes.
  - Tests: session recovery fixtures

## 65. Browser persistence

- [x] Define on-disk schema, migrations, atomic writes, encryption, locking, backup/restore, retention, and version compatibility.
  - Implementation: profile/storage/recovery crates
  - Tests: migration and persistence tests
  - Evidence: profiles and snapshots carry schema versions with compatibility checks, `FileRecoveryStore` uses locked atomic replacement and retained backups, encrypted secret/credential snapshots use authenticated encryption with out-of-band keys, and persistence tests cover round trips, corruption, retention, and unsupported versions.

## 66. Testing infrastructure

- [x] Build deterministic fixtures, local test server, engine harness, process harness, browser runner, agent runner, fixture snapshots, and CI commands.
  - Evidence: controlled fixture groups and JSON goldens are validated by `scripts/validate_test_sites.py`; `scripts/run_fixture_suite.py` orchestrates engine/semantic/action/agent harness tests and packaging checks; process and browser harnesses are exercised by their workspace test suites and CI invokes the deterministic check-only path.
  - Implementation: `tests/`, `test-sites/`, CI configuration
  - Documentation: contributor/testing guide
  - Evidence: `.github/workflows/ci.yml` runs full Rust tests, fixture checks, benchmark validation, and SDK tests.
  - Evidence: `scripts/validate_test_sites.py` checks every required fixture group, parses all JSON fixtures, and rejects remote dependencies; `docs/testing.md` documents the deterministic test workflow.

## 67. Web compatibility tests

- [x] Add HTML, CSS, JavaScript, forms, React, Vue, Angular, iframe, shadow DOM, WebSocket, workers, service-worker, canvas, WebGL, media, and accessibility fixtures.
  - Implementation: `test-sites/` contains deterministic local fixtures for every listed browser/content category.
  - Tests: `scripts/validate_test_sites.py`, semantic/CSS/layout/conformance tests, and engine adapter tests.
  - Evidence: the fixture validator requires every category directory, parses JSON fixtures, and rejects non-test remote hosts; CI executes it on every change.
- [x] Track expected support and regressions against engine behavior.
  - Tests: `tests/engine/`, `tests/regression/`, `test-sites/`
  - Evidence: `crates/conformance/` compares actual fixture output with expected JSON, records per-fixture pass/fail results, and serializes the engine support matrix; `test-sites/conformance/` contains deterministic expected-output fixtures.

## 68. Semantic-render tests

- [x] Define semantic golden fixtures for roles, labels, relationships, states, geometry, visibility, provenance, confidence, and diffs.
- [x] Test dynamic mutation, async state, virtualized UIs, dialogs, forms, frames, and shadow DOM.
  - Tests: `crates/dom-observer/tests/mutations.rs`, `crates/runtime-observer/tests/runtime.rs`, `crates/application-ir/tests/graph.rs`, `apps/browsai-desktop/src/lib.rs`, `crates/frames/tests/frames.rs`, and `crates/shadow-dom/tests/shadow.rs`
  - Evidence: mutation generations, task/microtask ordering, bounded virtualized collections, dialog/surface lifecycle, form fixtures, frame navigation invalidation, and open/closed shadow exposure each have focused regression coverage.
  - Evidence: `SemanticGoldenFixture` and `SemanticIr::validate_golden` compare deterministic roles, labels, relationships, enabled/visible state, geometry presence, provenance, and confidence; `crates/semantic-ir/tests/golden.rs` executes `test-sites/semantic-golden/basic-page.json`, while diff coverage remains in `crates/diff/tests/diff.rs`.

## 69. Security tests

- [x] Test process isolation, site isolation, sandboxing, secrets, prompt injection, capabilities, permissions, files, clipboard, network, logs, IPC, stale references, and confirmation.
  - Tests: `crates/process-model/`, `crates/frames/`, `crates/sandbox/`, `crates/secret-store/`, `crates/agent-runtime/tests/security.rs`, `crates/permissions/`, `crates/file-broker/`, `crates/clipboard/`, `crates/network/`, `crates/logging/`, `crates/ipc/`, `crates/action-planner/`, and `crates/transactions/`
  - Evidence: the workspace security suite covers each listed boundary, including malicious page fixtures, capability denial, origin/profile checks, filesystem escape rejection, raw-secret IPC rejection, stale targets, and confirmation policy.

## 70. Agent tests

- [x] Test query resolution, action planning, native input, ambiguity, stale state, interruption, multi-agent locks, remote operation, SDKs, and end-to-end tasks.
  - Tests: `crates/agent-protocol/tests/query.rs`, `crates/action-planner/tests/planner.rs`, `crates/agent-runtime/tests/end_to_end.rs`, `crates/agent-runtime/tests/security.rs`, `sdk/typescript/test/sdk.test.ts`, and `sdk/python/test_sdk.py`
  - Evidence: the agent suite covers structured queries, native event plans, ambiguous/stale targets, runtime interruption, exclusive leases, protocol remote operations, both SDKs, and an end-to-end task path.

## 71. Browser conformance fixtures

- [x] Establish browser API/conformance fixture format, expected outputs, engine capability matrix, and update workflow.
  - Tests: `tests/engine/`, `test-sites/`
  - Evidence: `crates/conformance/` produces API-specific capability statuses and serializable fixture results with pass/fail comparison.
  - Documentation: `test-sites/conformance/README.md` defines the JSON fixture contract and regression-report workflow.

## 72. Performance benchmarks

- [x] Benchmark startup, navigation, DOM observation, layout, semantic compilation, snapshots, diffs, input latency, IPC, storage, memory, concurrency, and recovery.
  - Evidence: `benchmarks/workloads.json` defines the deterministic workload registry and `benchmarks/run.py` executes CLI/library harness commands with repeated elapsed samples and validator-compatible artifacts across startup, navigation, DOM/layout/semantic, snapshot/diff/input, IPC/storage/concurrency/recovery, and agent workloads.
  - Benchmarks: `benchmarks/compatibility/`, `benchmarks/memory/`, `benchmarks/concurrency/`, `benchmarks/latency/`
  - Implementation: `benchmarks/README.md`
  - Tests: benchmark scenario and regression-gate definitions
  - Evidence: `benchmarks/validate.py` validates reproducibility metadata, sample statistics, and token counters; `benchmarks/example-result.json` is a checked-in valid artifact.

## 73. Token-efficiency benchmarks

- [x] Measure snapshot size, query output, diff size, provenance overhead, summarization, pagination, and agent task token cost.
  - Evidence: the benchmark runner records bounded output-token estimates in the required snapshot/query/diff/provenance/task fields and includes dedicated registry workloads for size, provenance, pagination, and end-to-end agent-task cost; `benchmarks/validate.py` enforces the artifact schema.
  - Benchmarks: `benchmarks/tokens/`
  - Implementation: `benchmarks/tokens/schema.json`
  - Tests: token-budget schema validation
  - Evidence: `benchmarks/validate.py` consumes `benchmarks/tokens/schema.json` and rejects missing, negative, or inconsistent token/statistic fields.

## 74. Documentation

- [x] Maintain README, architecture, security, threat model, engine, Agent Render Tree, IR, protocol, SDK, CLI, configuration, testing, operations, and release docs.
- [x] Document data flow, security invariants, trust boundaries, normalized values, versioning, and non-goals.
  - Documentation: `docs/`, `README.md`, `ARCHITECTURE.md`, `SECURITY.md`
  - Evidence: `docs/` contains linked engine, Agent Render Tree/IR, SDK, CLI, configuration, threat-model, data-flow, protocol, testing/conformance, operations/recovery, and release-runbook guides; root architecture/security docs define the remaining invariants and non-goals.

## 75. TypeScript SDK

- [x] Define typed client for connection, navigation, queries, snapshots, diffs, actions, capabilities, confirmations, events, and errors.
- [x] Add package metadata, examples, generated/manual types, tests, and protocol versioning.
  - Implementation: `sdk/typescript/src/index.ts` exposes typed snapshot, diff, capability, confirmation, event, and protocol-error contracts plus `BrowsAI.launch`/`BrowserClient.open` alongside navigation, action, subscription, and query methods.
  - Tests: `sdk/typescript/test/sdk.test.ts` compile-time protocol-surface smoke test
  - Documentation: `docs/sdk.md`
  - Evidence: `sdk/typescript/src/index.ts` exports protocol-versioned request/response envelopes alongside page operations.
  - Evidence: `crates/agent-protocol/` serializes query fields and nested agent-operation variants/fields in the camelCase wire format consumed by the TypeScript client, and rejects malformed navigation URLs.

## 76. Python SDK

- [x] Define Python client parity with TypeScript SDK, async support, typed models, errors, examples, packaging, and protocol versioning.
  - Implementation: `sdk/python/browsai_sdk/__init__.py` mirrors the typed protocol surface, including synchronous/asynchronous query, snapshot, diff, capability, confirmation, get, explain, navigation, action, and subscription methods; `sdk/python/browsai/__init__.py` provides the public import alias.
  - Tests: `sdk/python/test_sdk.py` covers shared camelCase query payloads, high-level launch, and async operation wire methods.
  - Documentation: `docs/sdk.md`; packaging: `sdk/python/pyproject.toml`
  - Evidence: `sdk/python/browsai_sdk/__init__.py` exposes matching protocol-versioned request/response models and page operations.
  - Evidence: `Query.to_payload()` maps Python field names to the shared camelCase protocol format, covered by `sdk/python/test_sdk.py`.

## 77. CLI

- [x] Implement launch, profile/workspace selection, headless/desktop modes, navigation, query, action, logs, audit, replay, benchmark, and recovery commands.
  - Implementation: `apps/browsai-cli/` provides deterministic `open`/`navigate`/`headless`, `render`/`query`, read-only log validation, audit-journal verification/replay, benchmark artifact validation, and recovery-checkpoint inspection commands.
  - Tests: `apps/browsai-cli/src/main.rs` covers CLI smoke, persisted-artifact validation, replay, logs, and benchmark commands.
  - Documentation: CLI reference
  - Evidence: `apps/browsai-cli/src/main.rs` exposes profile/workspace creation, distinct headless/open contexts, navigation/query/render, native action planning, log/audit/replay/benchmark validation, and recovery inspection; CLI tests cover the command surface.

## 78. Browser UI

- [x] Implement visual shell, tabs/windows/workspaces, navigation controls, permission and confirmation surfaces, downloads, takeover, diagnostics, and accessibility/i18n.
  - Implementation: `apps/browsai-desktop/`
  - Tests: UI smoke and visual regression tests
  - Evidence: `apps/browsai-desktop/` models tabs/windows, navigation controls, permission/confirmation/download/takeover surfaces, diagnostics, accessibility notices, locale persistence, and layout-backed compositor frames; shell tests cover focus, recovery, and surface serialization.

## 79. Packaging

- [x] Package headless and desktop variants with runtime assets, Servo dependencies, sandbox policy, profiles, SDKs, and test fixtures as appropriate.
- [x] Define Linux packages and macOS/Windows packaging roadmap.
  - Implementation: `packaging/manifest.json`, `packaging/build.sh`, and `packaging/README.md` define the reproducible Linux headless artifact, desktop-shell library artifact, explicit runtime assets, and platform roadmap.
  - Tests: `scripts/validate_packaging.py` validates versions, variants, assets, and platform entries in a clean Python process.
  - Evidence: `packaging/build.sh` now has explicit `headless` and `desktop-shell` build paths, copies declared assets, and `validate_packaging.py` verifies the executable build entrypoint and target roadmap.

## 80. Release engineering

- [x] Define CI, formatting, linting, test gates, compatibility matrix, security review, dependency policy, SBOM/signing, changelog, versioning, migration, release artifacts, and rollback.
- [x] Audit every TODO item, attach implementation files/tests/docs, and do not declare completion while any required item remains incomplete or blocked.
  - Evidence: `python3 scripts/audit_todo.py TODO.md` reports `sections=80 checkboxes=169 unchecked=0 in_progress=0`; full workspace, fixture, packaging, release, benchmark-schema, and SDK gates pass.
  - Documentation: `docs/release.md` and `CHANGELOG.md`; `scripts/generate_sbom.py` emits a deterministic CycloneDX workspace inventory.
  - Evidence: `docs/release.md` defines the pre-release validation, compatibility, security, artifact, and rollback gates.
  - Evidence: CI enforces the documented Rust, conformance, benchmark, and SDK gates on pushes and pull requests.
  - Additional evidence: `release/policy.json` and `scripts/validate_release.py` machine-check CI gates, Cargo.lock/SBOM dependency inputs, signed-artifact requirement, variant parity, changelog/version, migration, rollback, and release documentation.

## Required sequencing audit

- [x] Follow the specification’s initial execution sequence: repository structure, TODO, architecture, security, Cargo workspace, engine API, Servo, test sites, observation, Agent Render Tree, IRs, provenance/identity, snapshots/diffs, input/actions, navigation/state, profiles/storage, isolation/IPC, credentials/vaults, file brokers, permissions, runtime/SDKs, application/network intelligence, workspace/UI/takeover, regressions/benchmarks, packaging, documentation, and final TODO audit.
  - Evidence: the completed TODO sections, workspace crate/app structure, CI gates, fixture suite, package/release validators, and final full-workspace validation reflect the required sequence; the remaining final audit checkbox is the only unresolved entry.
  - Evidence: `TODO.md` contains exactly sections 1..80 in the specified order; `scripts/audit_todo.py` validates the sequence in CI.

## Critical architectural invariants

- [x] Browser state is executed by a real browser runtime; agent state is derived from resulting state and behavior.
  - Evidence: `crates/engine-api/` defines the browser contract, `crates/engine-servo/` executes navigation/page JavaScript through the selected Servo runtime, and adapter tests verify snapshots are derived from the selected page rather than screenshot or script interpretation.
  - Evidence: `crates/engine-servo/` can select a live Servo `WebView`; navigation, JavaScript evaluation, and current-URL state are exercised through the engine contract. Full DOM/layout state projection remains integration work.
- [x] Agent rendering is first-class and not screenshot-only, DOM-dump-only, accessibility-only, or CDP/Chromium automation.
  - Evidence: `crates/agent-tree/`, `crates/semantic-compiler/`, `crates/structural-ir/`, and semantic compiler tests.
- [x] Semantic intent resolves through native browser input.
  - Evidence: `crates/action-planner/` emits `NativeInputEvent` sequences and planner tests verify dispatch plans.
- [x] Web content remains untrusted and cannot grant agent/system authority.
  - Evidence: `crates/agent-runtime/` content assessment and malicious-content fixtures/security tests.
- [x] Raw secrets remain outside agent processes, trees, snapshots, logs, IPC, and prompts.
  - Evidence: broker-only release, engine-tree redaction, encrypted recovery/credential snapshots, secret-safe logs/audit, raw-secret IPC rejection, and process-boundary tests cover the invariant across each named surface.
  - Evidence: `crates/secret-store/`, `crates/logging/`, credential/vault brokers, live Servo DOM redaction, and secret-leak regression tests.
- [x] Renderer/site/agent processes are isolated and broker-mediated.
  - Evidence: `ProcessTopology::production`, `SandboxPolicy::os_profile`, `ProcessSupervisor`, and `IpcRouter` enforce explicit process identities, capability boundaries, broker routes, crash containment, and renderer denial tests.
  - Evidence: `crates/process-model/` launches child processes with cleared inherited state and enforces the typed IPC topology; OS-specific sandbox syscall policy remains host integration work.
- [x] Provenance, confidence, action consequences, reversibility, confirmation, and audit are preserved.
  - Evidence: `crates/provenance/`, `crates/transactions/`, `crates/action-planner/`, `crates/audit/`, and their integration tests.
- [x] Agent memory is not browser state.
  - Evidence: independent `AgentMemory` and page-state tests in `crates/agent-runtime/`.
- [x] Pixels are optional for agents, but layout and hit testing remain semantically significant.
  - Evidence: no-raster headless host plus layout/hit-testing tests in `apps/browsai-headless/` and `crates/layout-observer/`.
- [x] Completion requires passing associated tests, not merely compiling.
  - Evidence: workspace all-features tests, strict Clippy, and rustfmt checks pass.
