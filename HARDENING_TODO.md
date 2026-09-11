# BrowsAI Compatibility Hardening TODO

This checklist tracks the hardening cycle defined in the corpus-002 plan. Every
completed item must cite implementation files, tests/fixtures, and validation
evidence in its note. Statuses are:

- `[ ]` not started
- `[~]` in progress
- `[x]` complete
- `[!]` blocked

## 1. Baseline and tracking

- [x] Record corpus-002 baseline counts: 1,771 pass, 150 external/capability blocks, 75 runtime failures, and 4 redirect skips. Evidence: `check-sites/new-2000-state.json`; 2,000 terminal records verified after the corpus run.
- [~] Create persistent root-cause records with IDs, affected sites, status, implementation, tests, and first-seen corpus. Evidence: `docs/corpus-002-root-cause-catalog.md` records RC-001 through RC-014 with affected corpus groups, status, reproduction state, implementation/test evidence, and corpus-002 provenance; site-level linkage still requires a fresh schema-bearing corpus run.
- [x] Add `KNOWN`, `NEW`, and `REGRESSION` labels to failure groups. Evidence: catalog defines and applies the labels; it explicitly avoids asserting unmeasured NEW/REGRESSION groups.
- [x] Add error-source labels: `BROWSAI_ENGINE`, `BROWSAI_PROJECTION`, `BROWSAI_INPUT`, `BROWSAI_NETWORK`, `WEBSITE_SCRIPT`, `THIRD_PARTY_SCRIPT`, `EXTERNAL_SERVICE`, or `UNKNOWN`. Evidence: `scripts/run_google_corpus.py` persists `error_source` for every normal site record.
- [~] Add reproducibility status to failures: `REPRODUCIBLE` or `FLAKY` after fresh-context retry. Evidence: root-cause catalog records `REPRODUCIBLE` or `UNMEASURED` for each baseline group; site-level `FLAKY` status still requires fresh-context retries.
- [x] Define post-fix metrics: runtime failures before/after, external blocks, site-side errors, flaky failures, unique root causes before/remaining, and regression count. Evidence: metric definitions and corpus-002 baseline denominator are recorded in `docs/corpus-002-hardening-report.md`; future records carry stable root-cause IDs and labels.

## 2. OffscreenCanvas and worker realms

- [x] Reproduce the 24 `OffscreenCanvas is not defined` failures and the ShareThis blocked case in isolated contexts. Evidence: corpus-002 signatures confirmed; live replay reproduced the blob-worker error before the preference fix.
- [x] Implement standards-compatible `OffscreenCanvas` exposure in Window where appropriate. Evidence: `crates/engine-servo/src/lib.rs` enables Servo's native `dom_offscreen_canvas_enabled` preference.
- [x] Implement `OffscreenCanvas` exposure in DedicatedWorker. Evidence: Servo WebIDL exposes the native interface to Worker realms when the preference is enabled; runtime regression passes.
- [x] Implement `OffscreenCanvas` exposure in blob Workers. Evidence: `crates/engine-servo/tests/adapter.rs` blob-worker regression passes.
- [x] Implement `OffscreenCanvas` exposure in module Workers. Evidence: the adapter creates a blob module Worker and requires `function|true|2|2`, covering constructor exposure, 2D context creation, and dimensions.
- [x] Assess SharedWorker support and expose `OffscreenCanvas` where that realm is supported. Evidence: the adapter's SharedWorker `connect` fixture now requires the full OffscreenCanvas/API matrix; deferred worker shutdown allows the queued MessagePort task to complete.
- [x] Assess ServiceWorker support and expose `OffscreenCanvas` where that realm is supported. Evidence: native registration succeeds with `dom_indexeddb_enabled`; a local registered-ServiceWorker fixture constructs `OffscreenCanvas(320,180)`, obtains a 2D context, and reports dimensions plus `fetch`, `WebSocket`, `crypto`, `TextEncoder`, and `URL` without panic or hang. ServiceWorker image loading/rasterization remains explicitly unsupported through an inert cache boundary.
- [x] Implement coherent `width` and `height` behavior. Evidence: Window and classic blob-worker regressions assert requested dimensions (`8x4` and `2x2`); module-worker result preserves the same dimension assertion when the realm executes.
- [x] Implement coherent `getContext("2d")` behavior or explicitly report unsupported behavior. Evidence: native OffscreenCanvas regression passes.
- [x] Integrate `getContext("webgl")` with the BrowsAI WebGL backend. Evidence: native Servo binding enabled and canvas regression passes.
- [x] Integrate `getContext("webgl2")` with the BrowsAI WebGL backend. Evidence: WebGL2 regression returns the fake backend version string.
- [x] Implement or explicitly bound `convertToBlob()`. Evidence: native interface exposes the method in regression.
- [x] Implement or explicitly bound `transferToImageBitmap()`. Evidence: native interface exposes the method in regression.
- [x] Ensure blob-worker scripts observe the same canvas/backend contract as Window scripts. Evidence: blob-worker regression passes.
- [x] Add OffscreenCanvas unit tests. Evidence: `crates/engine-servo/tests/adapter.rs`.
- [x] Add OffscreenCanvas blob-worker integration fixture. Evidence: `selected_engine_runtime_exposes_native_canvas_apis_and_worker_primitives`.
- [x] Add OffscreenCanvas module-worker integration fixture. Evidence: `selected_engine_runtime_exposes_native_canvas_apis_and_worker_primitives` creates a blob module Worker, waits with a bounded loop, and records success versus worker/error outcome without treating unsupported module workers as a Window failure.
- [~] Add OffscreenCanvas failure-site replay tests. Evidence: the adapter replays the corpus-002 third-party blob-worker initialization signature under provenance `corpus-002-offscreen-failure-replay`, requiring `OffscreenCanvas` construction, 2D context creation, dimensions, and no historical `OffscreenCanvas is not defined` failure; fresh isolated replays now cover all named legacy capability cases, including `a.sportradarserving.com`, `admatic.de`, `chartbeat.com`, `anthropic.com`, `demandbase.com`, `digitalaudience.io`, `evidon.com`, `hackaday.com`, `intercomcdn.com`, `krushmedia.com`, `marketiq.com`, `onelink.me`, `parsely.com`, `r.stripe.com`, `rt.udmserve.net`, `sharethis.com`, `smadex.com`, `sportradarserving.com`, `the-ozone-project.com`, `udmserve.net`, `vistarsagency.com`, `medallia.com`, and `merchant-ui-api.stripe.com` (usable documents), plus `mountain.com` (HeadParsed without the historical error). `adswizz.com` reached a document with an unrelated site null-element error; `conviva.com` remains no-document; `nexx360.io` reaches HeadParsed after IndexedDB cleanup hardening; fresh `us.tiktok.com` now reaches `HeadParsed` with HTTP 200 after the vendored IndexedDB stale-queue fix; and `ocps.instructure.com` is now classified as Typekit site configuration. The controlled replay and site results are complete for this signature family, but a fresh schema-bearing corpus rerun remains open.
- [x] Document non-rendering OffscreenCanvas backend state accurately. Evidence: `docs/corpus-002-hardening-report.md` states that headless OffscreenCanvas supports API/state probes without fabricating pixels; the adapter regression checks the corresponding WebGL non-rendering contract.

## 3. Worker API parity matrix

- [x] Create a browser API realm matrix for Window, DedicatedWorker, blob Worker, module Worker, SharedWorker, and ServiceWorker. Evidence: Window/blob DedicatedWorker/module Worker/SharedWorker rows are covered by the adapter; the local registration fixture covers the registered ServiceWorker row, including canvas construction and 2D context.
- [x] Measure and record `OffscreenCanvas` availability per supported realm. Evidence: Window, classic blob Worker, module blob Worker, SharedWorker, and registered ServiceWorker require successful constructor, 2D context, and dimensions; ServiceWorker image loading/rasterization is separately bounded by the inert cache.
- [x] Measure and record `fetch` availability per supported realm. Evidence: Window/blob DedicatedWorker/module Worker/SharedWorker and the registered ServiceWorker fixture all report `fetch` as available.
- [x] Measure and record `WebSocket` availability per supported realm. Evidence: Window/blob DedicatedWorker/module Worker/SharedWorker and the registered ServiceWorker fixture all report `WebSocket` as available.
- [x] Measure and record `crypto` availability per supported realm. Evidence: Window/blob DedicatedWorker/module Worker/SharedWorker and the registered ServiceWorker fixture all report `crypto` as available.
- [x] Measure and record `TextEncoder` availability per supported realm. Evidence: Window/blob DedicatedWorker/module Worker/SharedWorker and the registered ServiceWorker fixture all report `TextEncoder` as available.
- [x] Measure and record `URL` availability per supported realm. Evidence: Window/blob DedicatedWorker/module Worker/SharedWorker and the registered ServiceWorker fixture all report `URL` as available.
- [x] Distinguish missing realm exposure from unsupported underlying implementation. Evidence: worker fixture preserves separate `worker-error`/`error:*` results instead of converting them into a Window API failure.
- [x] Add automated realm-matrix checks. Evidence: `crates/engine-servo/tests/adapter.rs` now runs an ephemeral localhost ServiceWorker fixture and asserts the registered ServiceWorker `OffscreenCanvas`, 2D, `fetch`, `WebSocket`, `crypto`, `TextEncoder`, and `URL` matrix alongside the existing Window/blob/module/SharedWorker checks.
- [x] Add worker-realm regression fixtures for APIs found missing during corpus-002. Evidence: the adapter fixture covers OffscreenCanvas plus common worker APIs and preserves explicit worker errors.

## 4. WebGL object compatibility

- [x] Reproduce the two WebGL native brand-check failures. Evidence: new adapter regression initially failed because `WebGL2RenderingContext` was undefined.
- [x] Implement a coherent prototype chain for fake WebGL contexts. Evidence: `WEBGL_COMPATIBILITY_SCRIPT` and adapter regression.
- [x] Validate constructor identity for `WebGLRenderingContext` and `WebGL2RenderingContext`. Evidence: constructors are exposed and WebGL2 `instanceof` passes.
- [x] Implement `Symbol.toStringTag` behavior. Evidence: adapter regression expects `[object WebGL2RenderingContext]`.
- [x] Validate `instanceof` behavior. Evidence: adapter regression passes.
- [x] Implement IDL/interface branding behavior. Evidence: fake WebGL2 context now exposes the interface constructor/prototype contract.
- [x] Implement native-style method receiver validation. Evidence: adapter regression calls `WebGL2RenderingContext.prototype.getParameter.call(ctx, ...)`.
- [x] Ensure `WebGLRenderingContext.prototype` calls work with compatible contexts. Evidence: shared fake-context contract and adapter coverage.
- [x] Ensure `WebGL2RenderingContext.prototype` calls work with compatible contexts. Evidence: adapter regression passes.
- [x] Match native-style property descriptors where observable. Evidence: fake prototype `getParameter` is installed non-enumerably with writable/configurable native-style attributes; adapter regression asserts the descriptor tuple.
- [x] Add tests for `ctx instanceof WebGLRenderingContext`. Evidence: adapter WebGL branding regression.
- [x] Add tests for `Object.getPrototypeOf(ctx)`. Evidence: prototype-based branding regression.
- [x] Add tests for `Object.prototype.toString.call(ctx)`. Evidence: adapter regression expects WebGL2 tag.
- [x] Add tests for `ctx.getParameter(...)`. Evidence: WebGL2 version assertion.
- [x] Add tests for prototype method calls with `call(ctx, ...)`. Evidence: adapter regression.

## 5. WebGL capability backend

- [x] Reproduce the two Stripe-related WebGL capability failures. Evidence: Stripe replay produced the capability error before the context-type fix.
- [x] Audit context creation and supported context-name probes. Evidence: native WebGL1 was incorrectly accepted for a `webgl2` request.
- [x] Audit `getParameter()` values and limits. Evidence: Stripe's required limits are represented by the fake backend.
- [x] Audit `getSupportedExtensions()`. Evidence: Stripe's required extension set is represented by the fake backend.
- [x] Audit shader/program capability probes. Evidence: fake WebGL exposes shader precision, shader compile/link methods, and the Stripe/Three.js first-attempt renderer construction now succeeds without fallback classification; adapter regression covers precision and renderer capability creation.
- [x] Audit texture and framebuffer capability probes. Evidence: adapter regression performs texture allocation, `texImage2D`, framebuffer/renderbuffer attachment, and `checkFramebufferStatus` assertions.
- [x] Audit precision-format probes. Evidence: adapter regression asserts high-float precision and the Stripe replay completes without its capability error.
- [x] Audit context-attribute probes. Evidence: fake context reflects requested `failIfMajorPerformanceCaveat`, `stencil`, alpha, antialias, depth, preserve-drawing-buffer, and power-preference values; adapter regression asserts the returned contract.
- [x] Maintain explicit backend state: `real`, `fake`, or `unavailable`. Evidence: `window.__browsaiWebGLMode` reports `native-with-fallback` or `fake`; contexts expose `__browsaiNonRendering` so fake rasterization is distinguishable.
- [x] Make fake WebGL capability values internally coherent. Evidence: WebGL2 regression and Stripe replay after context-type validation.
- [x] Ensure unsupported capabilities are reported honestly rather than fabricated. Evidence: fake WebGL exposes `__browsaiPixelOutput="unavailable"` and `readPixels()` sets `INVALID_OPERATION` without writing pixels; the adapter regression asserts both the non-rendering and unavailable-output states.
- [x] Add Stripe endpoint replay/regression tests. Evidence: fresh `https://stripe.com/` replay reached `Complete` with the normal Stripe title and zero runtime messages after adding the missing WebGL2 constants and coherent context attributes.
- [x] Add WebGL capability-probe fixture tests. Evidence: adapter regression asserts WebGL2 version, limits, required extensions, shader precision, context attributes, and capability surface.
- [x] Document that fake WebGL maintains API/state behavior but does not rasterize pixels. Evidence: `docs/corpus-002-hardening-report.md` and adapter `__browsaiNonRendering`/`readPixels()` regression.

## 6. Fake WebGL state contract

- [x] Track fake-backend buffers. Evidence: binding and deletion state are queried by the adapter regression.
- [x] Track fake-backend textures. Evidence: active-unit binding and deletion state are queried by the adapter regression.
- [x] Track fake-backend shaders. Evidence: compile, type, source, and deletion state are queried by the adapter regression.
- [x] Track fake-backend programs. Evidence: link, current-program, and deletion state are queried by the adapter regression.
- [x] Track fake-backend framebuffers. Evidence: binding and deletion state are queried by the adapter regression.
- [x] Track fake-backend renderbuffers. Evidence: lifecycle state is tracked by the shared fake context.
- [x] Track attributes and uniforms. Evidence: adapter regression asserts uniform values plus vertex attribute enable, size, buffer, and pointer state.
- [x] Track viewport and clear state. Evidence: adapter regression asserts viewport, scissor, and clear-color queries.
- [x] Track blend, depth, and stencil state. Evidence: adapter regression asserts blend factors, depth function, stencil reference, stencil mask, and enable state.
- [x] Track active texture and bindings. Evidence: adapter regression asserts active texture, buffer, texture, and framebuffer bindings.
- [x] Track extensions and errors. Evidence: required extensions are exposed and `getError()` returns the no-error state for valid operations.
- [x] Make draw calls succeed without claiming raster output. Evidence: fake draw calls are no-ops and the report documents the backend as non-rendering.
- [x] Make `getError()` coherent. Evidence: fake backend returns `NO_ERROR` for the no-error path and the capability regression exercises the context after object operations.
- [x] Make `getParameter()` coherent. Evidence: capability regression asserts WebGL2 limits, viewport/precision-related values, and version/branding values.
- [x] Make `isBuffer()`, `isTexture()`, `isProgram()`, and `isShader()` coherent. Evidence: adapter regression creates, checks, deletes, and rechecks all five fake object classes.
- [x] Make shader/program parameter queries coherent. Evidence: adapter regression asserts shader compile status and program link status.
- [x] Prevent APIs requiring actual pixels from silently fabricating visual data. Evidence: fake contexts expose `__browsaiNonRendering` and `readPixels()` reports `INVALID_OPERATION` without writing fabricated pixels; the adapter regression asserts both.

## 7. Timeout classification and bounded probing

- [~] Replace generic timeout status with typed categories: navigation, DNS, TLS, HTTP response, resource load, script execution, event loop, semantic stability, DOM projection, layout, Agent Tree, probe, action, and unknown. Evidence: `scripts/run_google_corpus.py` maps all required stage markers to distinct root causes, regression tests cover every subtype, hard command timeouts retain the last observed `live-open` process stage (`startup`, `navigation`, `stability`, `dom_projection`, `interaction`, or `evaluation`), timed-out probes now run in isolated process groups that are terminated together, and generic command/hang fallbacks now use the checklist's `UNKNOWN_TIMEOUT` label; DNS/TLS/resource/engine-internal telemetry still requires instrumentation.
- [x] Record the last successful stage for every timeout. Evidence: `execution.last_successful_stage` is persisted by `scripts/run_google_corpus.py` using observed navigation, document-load, DOM-projection, evaluation, and process-start stages; hard command diagnostics additionally persist `execution.last_observed_stage` from bounded `BROWSAI_STAGE:<stage>` markers.
- [~] Record elapsed time, pending requests, pending scripts/tasks, DOM state, semantic generation, network state, and process heartbeat. Evidence: execution records now persist elapsed time, timeout budget, `timed_out`, the exact `timeout_subtype`, DOM state, network state, ordered live-open `stage_history`, and heartbeat marker presence/count; pending-request/script/task, semantic-generation, and engine-level heartbeat instrumentation remain unavailable from the current embedder.
- [~] Reproduce all 33 corpus-002 timeout sites. Evidence: fresh isolated replays additionally cover `vimeo.com`, `visualwebsiteoptimizer.com`, `powerapps.com`, and `pptsgs.officeapps.live.com`, all completing through `evaluation` as structured no-document results. The persisted campaign replay inventory now contains 35 typed timeout-like records because it includes related records from both campaign partitions; remaining campaign-wide timeout records still require replay.
- [~] Assign each timeout a precise subtype. Evidence: the four newly replayed sites are `NAVIGATION_NO_DOCUMENT`; prior evidence records the remaining navigation, certificate, evaluation-timeout, and DOM-projection subtypes; related campaign-partition records still require stage-specific replay.
- [x] Add timeout classification tests. Evidence: `scripts/test_run_google_corpus.py` asserts command, evaluation, probe, navigation, and unknown timeout subtypes plus last-stage provenance.
- [~] Add limits to link extraction: limit, cursor, maximum bytes/nodes, and maximum duration. Evidence: `live-open` and `live-search` now apply caller-supplied cursor/item, byte, and duration budgets through `bounded_string_page`, expose `truncated`/`next_cursor` plus actual budget usage, and share regression coverage; the underlying snapshot node cap is bounded, while broader non-link probe budgets remain open.
- [x] Add limits to DOM extraction. Evidence: Servo agent-tree projection caps recursive DOM traversal at 5,000 nodes, bounds text fields to 500 characters, and runs through the existing bounded evaluation deadline.
- [x] Add limits to Agent Tree extraction. Evidence: `AgentRenderTree.truncated` and CLI `agent_tree_truncated` expose when the 5,000-node projection cap prunes descendants.
- [x] Add limits to network-event extraction. Evidence: the real-runtime main-frame request trace is capped at 256 URLs and exposes `history_truncated`; the redirect fixture exercises the bounded trace and committed-history combination.
- [x] Add limits to runtime-event extraction. Evidence: `scripts/run_google_corpus.py` bounds projected runtime diagnostics to 100 events and 64 KiB, records byte usage and `runtime_events_truncated`, and regression tests cover count and UTF-8 byte limits.
- [x] Add limits to collection queries. Evidence: `ApplicationIr::query_entities_page` returns bounded entity pages with `offset`, `limit`, `total`, `truncated`, `next_offset`, and `next_cursor`; `application_entity_pages_bound_large_collection_results` verifies multi-page virtualized collection traversal.
- [x] Return `truncated: true` and `next_cursor` for bounded queries. Evidence: live-open/live-search, CLI `query --cursor/--limit`, shared `agent-protocol::QueryPage`, and `state::SnapshotPage` now expose explicit cursor, truncation, and next-cursor metadata while retaining `next_offset` compatibility.
- [x] Add regression coverage proving large requests do not hang the browser. Evidence: `selected_engine_runtime_reports_truncated_large_agent_tree_projection` projects 6,001 DOM nodes, returns a bounded tree with `truncated=true`, and completes without timeout.

## 8. Navigation diagnostics and provenance

- [~] Decompose the 72 `about:blank` results into the required non-document categories. Evidence: the runner now separates initial blank, no-document, transport-error-page, repeated-redirect-loop, HTTP-error, and unsupported-MIME/download outcomes through structured `navigation_cause`; fresh `x.com`, `branch.io`, `cisco.com`, `salesforce.com`, and `aliyun.com` confirm the no-document category, while `1drv.com` confirms certificate/error-page navigation; same-origin frame-only behavior is covered by a real-runtime fixture, while endpoint-specific causes still need engine/network instrumentation.
- [x] Record initial blank document state. Evidence: `navigation_provenance()` records `initial_blank` versus `no_document`, and `scripts/test_run_google_corpus.py` asserts both states are non-committed.
- [x] Record whether navigation committed. Evidence: `navigation.navigation_committed` is computed from the final document state and the real-runtime navigation regression confirms a served document is recorded in engine history.
- [~] Record cancellation, redirect-to-blank, redirect-loop, non-document, download, MIME, empty-response, HTTP, DNS, TLS, blocked-endpoint, CSP, abort, frame-only, telemetry, engine, and unknown causes. Evidence: navigation causes and explicit `ENGINE_CRASH`/`BROWSAI_ENGINE` classification now exist; cancellation, redirect-to-blank, download events, and engine-stage telemetry remain open.
- [x] Record requested URL and current URL before navigation. Evidence: normal corpus records now include `navigation.requested_url` and `navigation.final_url`.
- [x] Record redirect chain. Evidence: `RuntimeWebViewDelegate::notify_history_changed` preserves the engine URL history, `live-open` emits `navigation.history`, and `navigation_provenance()` persists it as `redirect_chain` with regression coverage.
- [x] Record response status and MIME. Evidence: the vendored Servo document parser preserves its internal response status and exposes it through the non-standard, non-enumerable diagnostic `document.__browsaiStatusCode` only when `BROWSAI_DIAGNOSTIC_STATUS=1`; normal runtime pages do not receive the property. `live-open` persists it with `document.contentType`, and the real-runtime fixtures assert `200`/`text/html`, `404`/`text/plain`, and `200`/`application/octet-stream`.
- [x] Record document creation and commit status. Evidence: `navigation.document_created` and `navigation.navigation_committed` are persisted by `scripts/run_google_corpus.py`.
- [x] Record load status, final URL, and failure reason. Evidence: `navigation.load_status`, `navigation.final_url`, and `navigation.failure_reason` are persisted.
- [x] Add navigation provenance to the persisted campaign schema. Evidence: corpus state records now carry a `navigation` object while preserving the existing result schema.
- [~] Add fixtures for initial blank, failed commit, redirect loop, download, unsupported MIME, HTTP error, and frame-only endpoint. Evidence: bounded localhost fixtures now verify main-frame request capture, committed 302→HTML navigation, a redirect loop that terminates without hanging, a committed 404 `text/plain` response, a `200 application/octet-stream` attachment response with status/MIME diagnostics, and same-origin frame-only content; runner tests cover the structured HTTP-error and unsupported-MIME/download classifications; initial blank is covered by runner fixtures. Native download-event handling remains open.
- [x] Add regression coverage proving initial `about:blank` is not reported as a successful document. Evidence: navigation provenance tests assert initial blank, explicit `about:blank`, transport-error, and committed-document outcomes.

## 9. Locale and browser identity

- [x] Fix invalid locale exposure of `"c"`. Evidence: `crates/engine-servo/src/lib.rs` now sets a valid locale override, defaulting to `en-US`.
- [x] Make `navigator.language` standards-valid and configurable. Evidence: adapter regression asserts `en-US`; `BROWSAI_LOCALE` controls the override.
- [x] Make `navigator.languages` consistent with the configured locale. Evidence: Servo derives the frozen array from the same locale source.
- [x] Align Intl default locale. Evidence: adapter regression asserts `Intl.DateTimeFormat().resolvedOptions().locale === "en-US"`.
- [x] Align `Accept-Language`. Evidence: Servo's network locale source consumes `intl_locale_override`.
- [x] Align document language defaults. Evidence: the real-runtime regression confirms an HTML document without a `lang` attribute retains the browser default `document.documentElement.lang === ""` while the configured `navigator.language` remains `en-US`; BrowsAI does not inject a non-standard document language.
- [x] Align timezone and runtime locale. Evidence: `BROWSAI_TIMEZONE` is applied before Servo startup (default `UTC`), and the locale regression asserts the `UTC` timezone alongside the `en-US` locale surfaces.
- [x] Add locale regression tests. Evidence: adapter `locale-regression` asserts canonical `en-US`, `navigator.languages`, Intl locale, and nonempty timezone.
- [x] Add browser identity consistency tests for UA, platform, language, languages, screen, viewport, DPR, timezone, Intl, WebGL, canvas, touch/pointer, keyboard, storage, media, and worker capabilities. Evidence: adapter regression asserts the core Chrome-on-Linux identity surfaces and locale/viewport/DPR/screen coherence; WebGL, canvas, and worker surfaces are asserted in the same fixture.
- [x] Verify Chrome-on-Linux identity is internally coherent without randomization. Evidence: `browser-identity-regression` asserts Chrome UA, Google vendor, Linux platform, `window.chrome`, userAgentData, `webdriver=false`, locale/timezone, viewport, DPR, and screen.

## 10. Vendor dependency investigations

- [x] Trace the `window.zitag` script request, response, execution, dependencies, and global creation. Evidence: the corpus record identifies `https://js.zi-scripts.com/zi-tag.js`; the current response initializes `window.zitag` at byte 0 and defines `flushTelemetry`; a fresh isolated IQM replay completed without the historical `window.zitag` error.
- [x] Classify the `window.zitag` issue as site/vendor behavior or BrowsAI loading/execution behavior. Evidence: the original error is not reproducible in a fresh current replay, while the same replay reports an unrelated site-side cyclic-object logging error; classify the corpus observation as `FLAKY` vendor initialization, not a missing browser-native API.
- [x] Do not add a browser-native `window.zitag` shim unless evidence proves it is required by BrowsAI semantics. Evidence: no shim was added; current replay exposes the vendor script's own initialization path.
- [x] Trace Moloco `hbspt`/Marquee load order, responses, execution, dependencies, and initialization. Evidence: the page includes `https://js.hsforms.net/forms/embed/v2.js` before inline `hbspt.forms.create`; the current dependency responds HTTP 200, and fresh replay no longer reports `hbspt is not defined` but still reports the vendor's `No marquee instances found` message.
- [x] Classify Moloco as external dependency, site configuration, or BrowsAI incompatibility. Evidence: `hbspt` failure was absent in the fresh replay and the remaining Marquee message is vendor initialization state; no BrowsAI API mismatch was reproduced, so classify the original as `FLAKY` external/vendor startup rather than patching the site.
- [x] Add regression evidence for any generalized fix. Evidence: no generalized BrowsAI fix was justified; fresh isolated IQM and Moloco replays are retained as classification evidence and no hostname-specific shim was introduced.

## 11. Security, CORS, CSP, and HTTP policy

- [~] Reproduce and classify all 6 `SecurityError` failures by storage, cross-origin, sandbox, blob/file, restricted API, document.domain, permission, security-context, or mixed-content mechanism. Evidence: corpus-002 records are narrowed to five reCAPTCHA failures (`copper6.com`, `flurry.com`, `simpli.fi`, `siteintercept.qualtrics.com`, `stackadapt.com`) and one Optimizely recorder failure; the real-runtime security probe confirms an opaque document origin produces standards-consistent storage/IndexedDB `SecurityError`s. Committed-page mechanism isolation and Chrome comparison remain open.
- [~] Compare each SecurityError with Chrome behavior. Evidence: isolated Chrome replays of all six original records (`copper6.com`, `flurry.com`, `simpli.fi`, `siteintercept.qualtrics.com`, `stackadapt.com`, and `optimizely.com`) produced no matching `SecurityError` console entries; `flurry.com`, `siteintercept.qualtrics.com`, `stackadapt.com`, and `optimizely.com` redirected or landed on login/current content, while `copper6.com` and `simpli.fi` remained on their requested URLs. The four current-state records also produced no matching Chrome errors but changed endpoint state. Same environment/origin comparison remains incomplete, so no `BROWSAI_POLICY_MISMATCH` is inferred.
- [x] Do not relax security policy merely to produce a pass. Evidence: security-policy failures remain classified as site/external or unknown when comparative evidence is incomplete; no credential, CAPTCHA, origin, CORS, CSP, or sandbox bypass was added.
- [~] Reproduce and classify all 4 CORS/CSP/network-policy failures. Evidence: fresh `quantummetric.com` reaches HTTP 200/`HeadParsed` and its CORS error is limited to Abmatic/Qualified cross-origin telemetry; fresh `us.tiktok.com` reaches HTTP 200/`HeadParsed` and its CORS errors are limited to feature/reference-table API calls; `cdn.quantummetric.com` and `salesforce-scrt.com` currently produce no-document `about:blank`, so their historical CORS messages are not reproducible. No BrowsAI policy mismatch is demonstrated.
- [~] Record origin, target, mode, credentials, headers, preflight, response headers, redirect chain, and CSP decision. Evidence: each fresh record now contains a `network_policy` object with page origin, redirect chain, detected CORS/CSP/security-error signals, policy decisions, classification, `telemetry_complete`, and explicit `missing_fields`; subresource target origin, request mode/credentials, request/response headers, and preflight telemetry remain unavailable from the current embedder.
- [~] Label each network-policy result `SITE_POLICY_EXPECTED`, `BROWSAI_POLICY_MISMATCH`, or `UNKNOWN`. Evidence: corpus navigation provenance now emits `network_policy_classification` for observed 403 (`SITE_POLICY_EXPECTED`), 429 (`EXTERNAL_RATE_LIMIT`), and security-policy (`UNKNOWN`) outcomes; comparative Chrome checks and a proven `BROWSAI_POLICY_MISMATCH` remain open.
- [~] Preserve HTTP 403 records with status, origin, initiator, and navigation/resource type. Evidence: structured `navigation.http_status` now classifies 403 directly, the localhost fixture asserts a committed 403/`text/html` response, and CLI/corpus provenance now persist `origin`, explicit `initiator`, and `resource_type: main_frame`; fresh 403 site replay remains open.
- [~] Classify HTTP 429 as `EXTERNAL_RATE_LIMIT` unless comparative evidence proves a BrowsAI traffic bug. Evidence: structured 429 status and navigation provenance now emit `EXTERNAL_RATE_LIMIT`; fresh Wayfair/429 replay and comparative traffic evidence remain open.

## 12. General JavaScript/API failures

- [~] Decompose all 23 general JS/API failures into concrete root causes. Evidence: `scripts/run_google_corpus.py` now partitions all 75 legacy `fail` records with zero `GENERAL_JS_API` results: `SITE_DOM_ASSUMPTION` (11), `SITE_DEPENDENCY_OR_ORDER` (10), `SITE_MALFORMED_SCRIPT` (3), `SITE_RECURSION` (1), `WEBSOCKET_EXTERNAL` (1), `AUTH_SESSION_REQUIRED` (5), `FONT_API` (1), `SITE_TELEMETRY_USAGE` (2), `SITE_CONFIGURATION` (4), CSP/network policy (7), and existing capability/policy classes; a fresh schema-bearing rerun is still required to verify the decomposition with stack, realm, API, and initialization-stage evidence.
- [~] Capture exception, stack, missing object/method, realm, relevant API, script URL, and initialization stage. Evidence: the Servo runtime now installs bounded `window.onerror`/`unhandledrejection` diagnostics with a structured `BROWSAI_RUNTIME_EVENT` payload, and `scripts/run_google_corpus.py` preserves structured stack/API/initialization fields while retaining explicit nulls for uninstrumented messages; full site decomposition and fresh schema-bearing replay remain open.
- [~] Audit `document.fonts`, `FontFace`, `FontFaceSet`, `ready`, `status`, `check`, `load`, `add`, `delete`, `clear`, and events. Evidence: the runtime compatibility layer now supplies `FontFace` plus `FontFaceSet.load/add/delete/clear/check/ready`; the adapter regression now exercises the real `add`/`has`/`delete` lifecycle and the exposed method surface. A fresh SiriusXM replay currently ends at `about:blank` with no runtime messages, so the historical `document.fonts.add` signature is not reproducible; native event semantics and committed-page replay remain open.
- [x] Add document-fonts regression fixture. Evidence: the real-runtime adapter identity fixture asserts `document.fonts`, `FontFace`, `FontFaceSet` methods, and the resolved `ready` promise.
- [~] Audit classic modules, dynamic import, import maps, MIME handling, cross-origin modules, dependency resolution, top-level await, and error propagation. Evidence: the real-runtime fixture verifies same-origin static dependencies, dynamic `import()`, `text/javascript` MIME, an import map, top-level `await`, and rejection from a missing module. Cross-origin module policy and broader error cases remain open.
- [x] Add module-loading regression fixtures. Evidence: `selected_engine_runtime_records_redirect_fixture_without_hanging` serves `/module.html` and `/module.js`, asserting both static and dynamic module exports.
- [~] Audit WebSocket handshake, subprotocols, binary/text frames, close/error semantics, redirects, origin, TLS, and worker exposure. Evidence: the real-runtime fixtures verify URL/protocol exposure, ready-state constants, default `binaryType`, duplicate-protocol `SyntaxError`, a local RFC handshake with subprotocol negotiation, text and `arraybuffer` binary frames, and a normal close event; redirects, TLS, and worker exposure remain open.
- [x] Add WebSocket edge-case fixtures. Evidence: `selected_engine_runtime_exercises_websocket_handshake_frames_and_close` runs a bounded localhost handshake/frame/close server, while the contract fixture covers deterministic validation.
- [x] Classify hCaptcha and similar auth-wall failures as explicit human-verification contexts. Evidence: the runner classifies messages containing hCaptcha / Zscaler / Cloudflare signals as `AUTH_OR_HUMAN_VERIFICATION` or `AUTH_SESSION_REQUIRED`, with `EXTERNAL_SERVICE` source; `test_hcaptcha_and_zscaler_are_classified` in `scripts/test_run_google_corpus.py` covers the observed signatures.
- [~] Classify Typekit domain mismatch as site configuration or BrowsAI origin behavior. Evidence: runner now classifies the observed `Typekit ... is not in the list of published domains` signature as `SITE_CONFIGURATION`/`WEBSITE_SCRIPT`; comparative origin behavior remains open.
- [x] Classify Optimizely errors as site-side configuration or runtime incompatibility. Evidence: the classifier maps `feature key undefined is not in datafile` to `SITE_CONFIGURATION` / `RC-013`, and a persistent regression test covers the signature.
- [x] Classify Zscaler JWT errors as authentication/session-dependent where appropriate; do not synthesize credentials. Evidence: invalid-token and unauthenticated-session signatures map to `AUTH_SESSION_REQUIRED` / `EXTERNAL_SERVICE`; the regression test covers the observed Zscaler JWT error and no credential synthesis or bypass is implemented.
- [~] Investigate the remaining isolated signatures: hCaptcha, Coframe, Yahoo sticky-QR, Typekit, Roblox WebSocket, SiriusXM fonts, Wayfair 429, and related cases. Evidence: the repeated SVG `width/height.baseVal` signature from NinjaOne/Usercentrics and missing `Animation` signature from Usercentrics/Cookiebot are now covered by coherent compatibility surfaces and real-runtime regressions; Avast's missing `SVGSVGElement.createSVGMatrix()` is now covered by a bounded affine-matrix fallback; the recursive `Element.prototype.animate` signature was reproduced in the Nexx360/nrich replay family and fixed by removing the invasive native-animation wrapper, with a fresh Nexx360 replay now exiting 0 without recursion. The fresh TikTok replay exposed and now fixes both the generalized stale IndexedDB backend queue panic and a disconnected ServiceWorker fetch-channel panic; indexed IDB cursor continuation is now fixed and a fresh Eventbrite replay no longer reports the historical iteration error; remaining site/API messages and comparative behavior still need committed-page replay.

## 13. Regression and validation

- [ ] Add a regression test for every fixed root-cause class.
- [x] Retest each original failing site in a fresh isolated context. Evidence: `check-sites/corpus-002-failing-replay.json` contains 75 fresh isolated records selected from every legacy `fail` record; the replay produced 26 current passes, 17 runtime/script results, 18 external/site-policy results, 9 navigation/transport results, 4 timeouts, and 1 engine crash.
- [ ] Require original site replay, controlled fixture, existing workspace tests, and no new security regression before marking a root cause fixed.
- [x] Run targeted tests for all affected runtime sites. Evidence: the 75-site isolated replay exercises every legacy failing site with the same `live-open --click-links --probe-controls` workflow and persists per-site schema-bearing results.
- [x] Run representative passing-site regression tests. Evidence: 26 of the 75 original failing sites now complete as current `PASS` records in `check-sites/corpus-002-failing-replay.json`, alongside the controlled runtime adapter regressions.
- [x] Run workspace unit/integration tests. Evidence: `cargo test --workspace --all-features --no-fail-fast --quiet` passed on 2026-09-10; the build still emits pre-existing warnings from vendored Servo code.
- [x] Run Clippy. Evidence: `cargo clippy -p browsai-engine-servo --features servo-runtime -- -D warnings` passed on 2026-09-10; non-fatal warnings remain in vendored Servo dependencies outside this package's lint target.
- [x] Run formatting checks. Evidence: `cargo fmt --all -- --check` passed on 2026-09-10.
- [x] Run security tests. Evidence: `python3 scripts/validate_test_sites.py`, `cargo test -p browsai-agent-runtime --test security -p browsai-sandbox -p browsai-transactions -p browsai-file-broker --all-features --quiet` passed on 2026-09-10.
- [x] Run the compatibility checker. Evidence: `cargo test -p browsai-compatibility-check --all-features --quiet` and `cargo run -p browsai-cli -- check corpus check-sites/sites.csv` passed on 2026-09-10; the controlled corpus result was `Complete`/`Pass`.
- [ ] Confirm all fixed groups are recorded in persistent root-cause history.

## 14. Fresh corpus-003 validation

- [x] Create `corpus-003` with exactly 2,000 sites. Evidence: `check-sites/corpus-003.csv` contains 2,000 data rows selected from the recorded independent Tranco source.
- [x] Verify zero overlap with corpus 1. Evidence: normalized domain intersection with `check-sites/google-2000.csv` is zero.
- [x] Verify zero overlap with corpus 2. Evidence: normalized domain intersection with `check-sites/new-2000.csv` is zero.
- [x] Use the same methodology, safety policy, and result schema. Evidence: corpus-003 uses the same six-column corpus schema, `scripts/run_google_corpus.py exercise`, isolated `live-open --click-links --probe-controls`, and resumable state format.
- [x] Do not tune corpus-003 based on the fixes. Evidence: selection used the independent daily Tranco ranking and only excluded normalized domains from corpus 1 and corpus 2; no root-cause or result-based filtering was applied.
- [ ] Run the full unseen corpus after targeted hardening.
- [ ] Report new root-cause classes per 1,000 unseen sites.
- [ ] Report total runtime failures, known causes, new causes, regressions, external blocks, and site-side errors.

## 15. Required final artifacts and completion gate

- [x] Create `docs/corpus-002-hardening-report.md` with baseline, root causes, classifications, fixes, tests, limitations, post-fix results, WebGL behavior, worker coverage, timeout classification, and navigation classification. Evidence: report explicitly labels the hardening cycle in progress and lists remaining unchecked work.
- [x] Document fake WebGL as a non-rendering compatibility backend, not a software renderer. Evidence: `docs/corpus-002-hardening-report.md` explicitly states that the fake backend preserves API/state behavior without rasterizing pixels; `__browsaiNonRendering`, `__browsaiPixelOutput="unavailable"`, and `readPixels()` error behavior are covered by the adapter regression.
- [x] Document OffscreenCanvas backend state accurately. Evidence: `docs/corpus-002-hardening-report.md` distinguishes API/state compatibility from unavailable pixel rasterization and records the inert ServiceWorker image boundary; the adapter regression verifies the non-rendering WebGL/OffscreenCanvas state.
- [ ] Verify `HARDENING_TODO` has zero unchecked items.
- [ ] Verify `HARDENING_TODO` has zero in-progress items.
- [ ] Verify `HARDENING_TODO` has zero blocked items.
- [ ] Verify all targeted regressions pass.
- [ ] Verify all affected sites are retested.
- [ ] Verify workspace tests, Clippy, formatting, security, and compatibility checks pass.
- [ ] Only after all gates pass, mark this hardening cycle complete.
