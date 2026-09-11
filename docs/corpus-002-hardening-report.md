# Corpus-002 hardening report

Status: in progress. This report records verified changes; it is not a completion
claim for the full hardening plan.

## Baseline

The corpus-002 state is `check-sites/new-2000-state.json`:

- 1,771 records were labeled `pass`.
- 150 records were labeled `blocked_external`.
- 75 records were labeled `fail`.
- 4 records were labeled `redirect_skipped`.

The original runner grouped several different mechanisms under
`blocked_external`. The baseline also contains records whose body is an engine
error page (`Error loading page` / `Could not load the requested page`) while
the status is `pass`. Those are now recognized by the runner as
`NAVIGATION_TRANSPORT_ERROR` and are not treated as successful target-document
loads in new runs.

### Precise baseline interpretation

The terminal `status` values are campaign compatibility labels, not a
mechanism taxonomy. In particular, the historical `blocked_external` bucket
contains navigation failures, command timeouts, empty results, and a small
number of capability/script cases. The old state file predates the detailed
schema, so its records cannot be retroactively assigned stable root-cause
metadata without rerunning them.

For triage, new records also carry one mutually exclusive `outcome_class`:

| `outcome_class` | Meaning | Fix owner to investigate |
| --- | --- | --- |
| `PASS` | Target document loaded and the configured probe completed | Regression monitoring |
| `TIMEOUT` | A bounded command, navigation, evaluation, or probe deadline expired | Runner/engine stage telemetry |
| `NAVIGATION_OR_TRANSPORT` | No document, no result, or an engine/network load error page | Navigation/network path |
| `BROWSER_CAPABILITY` | A browser API or capability probe failed | Servo projection or compatibility layer |
| `EXTERNAL_OR_SITE_POLICY` | HTTP policy, authentication, CORS/CSP, vendor, or other site-side block | External service/site policy unless reproduced in a fixture |
| `RUNTIME_OR_SCRIPT` | A JavaScript/API exception was observed after a result was available | Missing API, website script, or third-party script |
| `REDIRECT_POLICY` | Campaign deliberately skipped an alias or authentication handoff | Corpus policy |
| `UNKNOWN` | The available evidence was insufficient | Instrumentation first |

The runner still writes the historical `status` field, but `outcome_class`
must be used for fix prioritization and post-fix comparisons. `NAVIGATION_NO_RESULT`
is now distinct from a generic engine failure, so successful process exit with
an empty payload is no longer silently treated as an unspecified external
capability problem.

An evidence-only audit of the legacy records is available with:

```text
python3 scripts/run_google_corpus.py audit --state check-sites/new-2000-state.json
```

Its mutually exclusive result is:

| `outcome_class` | Count | Status split | Immediate action |
| --- | ---: | --- | --- |
| `PASS` | 1,771 | 1,771 pass | Regression monitoring |
| `NAVIGATION_OR_TRANSPORT` | 72 | 72 blocked | Navigation and transport instrumentation |
| `TIMEOUT` | 32 | 32 blocked | Reproduce by timeout stage |
| `BROWSER_CAPABILITY` | 29 | 1 blocked, 28 fail | Compatibility-layer regression fixtures |
| `EXTERNAL_OR_SITE_POLICY` | 57 | 45 blocked, 12 fail | Preserve policy; compare only with controlled fixtures |
| `RUNTIME_OR_SCRIPT` | 35 | 35 fail | Split missing API, site script, and third-party script |
| `REDIRECT_POLICY` | 4 | 4 skipped | Corpus policy |

The 57 external/policy records include 41 completed Microsoft 403 pages; they
are not empty payloads. The legacy state stores the completed document under
`result` and leaves its top-level `error` empty. These counts are a partition
of available legacy evidence, not proof of engine ownership; the schema-bearing
rerun remains authoritative for fresh root-cause and reproducibility data.

The original 75 `fail` records decompose as 35 `RUNTIME_OR_SCRIPT`, 28
`BROWSER_CAPABILITY`, and 12 `EXTERNAL_OR_SITE_POLICY`. The original 150
`blocked_external` records decompose as 72 `NAVIGATION_OR_TRANSPORT`, 32
`TIMEOUT`, 1 `BROWSER_CAPABILITY`, and 45 `EXTERNAL_OR_SITE_POLICY`. The 57
external/policy records are therefore 41 Microsoft 403 responses plus these 16
non-Microsoft records: `admixer.net`, `adscale.de`, `adsmoloco.com`,
`api.hcaptcha.com`, `cdn.quantummetric.com`, `files-asr.acrobat.com`,
`files.acrobat.com`, `iqm.com`, `liveintent.com`, `mobile.zscaler.net`,
`mobile.zscalerone.net`, `mobileadmin.zscalerone.net`, `quantummetric.com`,
`travelaudience.com`, `wayfair.com`, and `xiaomi.com`.

### Fresh replay of all legacy failures

Every one of the 75 legacy `fail` records was rerun in a fresh isolated
runtime on 2026-09-10. The separate result file is
`check-sites/corpus-002-failing-replay.json`; it does not overwrite the legacy
baseline. Its current partition is 26 `PASS`, 17 `RUNTIME_OR_SCRIPT`, 18
`EXTERNAL_OR_SITE_POLICY`, 9 `NAVIGATION_OR_TRANSPORT`, 4 `TIMEOUT`, and 1
`ENGINE_CRASH`. The replay confirms that several historical capability issues
are fixed in the current runtime, but it does not justify treating the
remaining external or site-side outcomes as browser defects.

The generic `InvalidAccessError` records for Ryanair and IndiaTimes are
also removed from the browser-fix queue. Their persisted events contain no
operation, API, or stack identity, so they are treated as session/upstream
startup behavior rather than evidence for a BrowsAI compatibility shim.

That replay also exposed a new engine-owned regression: `us.tiktok.com` hit a
`Disconnected` unwrap panic in `vendor/servo-script/fetch.rs` while a
ServiceWorker resource thread was being torn down. The fetch path now converts
disconnected channels and EOF-without-metadata into `NetworkError::ConnectionFailure`.
A post-fix isolated `live-open https://us.tiktok.com/ --probe-controls` completed
in 13.3 seconds with no panic and a structured `about:blank`/no-document
result.

Corpus-003 has been constructed independently from the daily Tranco top-1M
ranking; the source URL and SHA-256 are recorded in
`check-sites/corpus-003-state.json`. It contains exactly 2,000 unique domains,
with zero normalized overlap against `google-2000.csv` and `new-2000.csv`, and
uses the unchanged runner and safety policy. The full resumable exercise is in
progress; its current results are persisted in that state file.

### What the percentages actually mean

The corpus has 2,000 terminal records. The 1,771 passes leave **229 records
that are not passes (11.45%)**. Of those, 4 are deliberately skipped by
redirect policy and 225 are substantive non-pass outcomes (11.25%). The claim
that only about 2% remains is therefore not supported by this baseline.

For implementation planning, the actionable buckets are:

| Priority | Bucket | Count | Share of corpus | First investigation |
| --- | --- | ---: | ---: | --- |
| 1 | `NAVIGATION_OR_TRANSPORT` + `TIMEOUT` | 104 | 5.20% | Navigation stages, DNS/TLS/HTTP, deadlines, and hang telemetry |
| 2 | `RUNTIME_OR_SCRIPT` | 35 | 1.75% | Missing browser API versus site/third-party JavaScript |
| 3 | `BROWSER_CAPABILITY` | 29 | 1.45% | Compatibility shims and capability-specific fixtures |
| 4 | `EXTERNAL_OR_SITE_POLICY` | 57 | 2.85% | Preserve as policy unless a controlled replay proves an engine defect |
| — | `REDIRECT_POLICY` | 4 | 0.20% | Corpus policy; not an engine failure |

The 64 records in the runtime-plus-capability buckets (3.20%) are the clearest
browser-implementation targets. Navigation and timeout records are also
potentially engine-owned, but cannot be counted as browser defects until their
stage is reproduced. External/policy records must not be added to the browser
defect total without comparative or controlled-fixture evidence.

Timeout records now carry `execution.timed_out` and the exact
`execution.timeout_subtype`; generic command and hang fallbacks are normalized
to `UNKNOWN_TIMEOUT`. The runner still records explicit nulls for pending
requests/tasks, semantic generation, and process heartbeat because the current
engine/embedder does not expose those counters. This keeps missing telemetry
visible instead of implying that no work was pending.
Hard command diagnostics also persist `execution.last_observed_stage` from the
bounded `BROWSAI_STAGE:<stage>` marker independently of
`last_successful_stage`, so a stage at which progress stopped is available
even when no result payload was produced.

Fresh `nexx360.io` replay initially exposed an engine-crash class that the
legacy schema could not represent: three isolated runs exited with signal
`-11` and SpiderMonkey `GetPropMaybeCached`/`CallGetter` frames. The root cause
was non-idempotent IndexedDB upgrade cleanup; stale callbacks now return safely.
After the guard, Nexx360 reaches `HeadParsed` without an abort. The same guard
also removes TikTok's IndexedDB panic and 75-second hang; TikTok now exits
cleanly through evaluation but remains `about:blank`, so it is now tracked as
no-document navigation rather than an engine crash or timeout.

Navigation provenance now also emits `network_policy_classification`: 403 is
`SITE_POLICY_EXPECTED`, 429 is `EXTERNAL_RATE_LIMIT`, and security-policy
signals remain `UNKNOWN` until a controlled or comparative replay establishes
whether BrowsAI diverges from Chrome. No `BROWSAI_POLICY_MISMATCH` is inferred
from a site failure alone.

It also emits `navigation_cause` for `NO_RESULT`, `NO_DOCUMENT`,
`TRANSPORT_ERROR_PAGE`, `REDIRECT_LOOP`, `HTTP_ERROR`,
`UNSUPPORTED_MIME_OR_DOWNLOAD`, `COMMITTED_DOCUMENT`, and
`LOAD_INCOMPLETE`. These are evidence-based classifications; native download
event handling still requires an embedder sink before it can be distinguished
from unsupported MIME.

The real-runtime navigation fixture now also exercises a two-endpoint redirect
loop. The server observes both loop targets, the adapter request trace remains
bounded, and the test completes without waiting for the fixture deadline.
The same fixture now serves a `200 application/octet-stream` response with an
attachment disposition and verifies the observed response status/MIME. This
establishes unsupported-MIME evidence, but the adapter still has no download
event/sink assertion, so it is not yet a complete download-behavior test.

The Servo runtime now installs a bounded `window.onerror`/
`unhandledrejection` bridge. Structured `BROWSAI_RUNTIME_EVENT` records retain
the exception message, stack, realm, script URL, line/column, API, and
initialization stage when the page exposes them; raw console text remains
available and unstructured vendor messages remain nullable.

## Verified hardening changes

### OffscreenCanvas and locale

Servo's native OffscreenCanvas preference is enabled in the real runtime. The
adapter regression covers Window, blob Worker, 2D context creation,
`convertToBlob`, and `transferToImageBitmap`. The runtime locale defaults to
`en-US` and is configurable through `BROWSAI_LOCALE`.
The runtime timezone defaults to `UTC` and is configurable through
`BROWSAI_TIMEZONE`; it is pinned before Servo startup because the current
embedder API has no direct timezone preference.
The OffscreenCanvas backend is intentionally non-rendering in headless mode:
API construction and state probes are supported, while pixel-producing output
is not silently fabricated.
The same regression replays the corpus-002 third-party blob-worker failure
signature under `corpus-002-offscreen-failure-replay`; the worker now completes
the constructor, 2D-context, and dimension probe without the historical
`OffscreenCanvas is not defined` error.
Isolated fresh replays of former `a.sportradarserving.com`, `admatic.de`, and
`chartbeat.com` capability failures now reach usable documents with no
historical OffscreenCanvas error and with the expected Chrome-style UA and
WebGL probes. This is representative evidence, not completion of all 24
original site replays.

The next isolated sample produced the same result for `anthropic.com`,
`demandbase.com`, and `digitalaudience.io`. `adswizz.com` reached a usable
document but emitted an unrelated site-script null-element error; it no longer
emits the historical OffscreenCanvas signature. `conviva.com` currently ends at
`about:blank` with `load_status: Complete`, so it is now tracked as
`NAVIGATION_OR_TRANSPORT`, not as a resolved capability replay.

Five more former capability cases also reached usable documents without the
historical signature: `evidon.com` (redirected to Rezolve), `hackaday.com`,
`intercomcdn.com` (redirected to Intercom), `krushmedia.com`, and
`marketiq.com`. These replays validate the fix across 11 former capability
sites; they do not replace the required full-site replay.

The next five also reached usable documents without the historical signature:
`onelink.me` (redirected to AppsFlyer), `parsely.com`, `r.stripe.com`
(redirected to Stripe), `rt.udmserve.net` (redirected to Underdog Media), and
`sharethis.com`. ShareThis therefore clears the former blocked capability case
in fresh replay; `nexx360.io` remains the separate reproducible engine crash.

The final legacy capability sample reached usable documents for `smadex.com`,
`sportradarserving.com` (redirected to Sportradar), `the-ozone-project.com`
(redirected to Ozone), `udmserve.net` (redirected to Underdog Media), and
`vistarsagency.com`. Additional isolated replays reached usable documents for
`medallia.com` and `merchant-ui-api.stripe.com` (redirected to Stripe), while
`mountain.com` reached `HeadParsed` without the historical capability error.
`us.tiktok.com` now exits through evaluation without a crash or timeout but
remains `about:blank`; this is a no-document navigation result, not a remaining
OffscreenCanvas failure.

The observed OCPS Typekit message is now classified as `SITE_CONFIGURATION`
(`WEBSITE_SCRIPT`), because it explicitly reports that the current domain is
absent from the kit's published-domain list. Whether the site intended a
different origin remains a comparative investigation, not a browser shim.

Across the 29 legacy `BROWSER_CAPABILITY` records, fresh isolated replay now
covers the complete named signature family without reproducing the historical
OffscreenCanvas/WebGL capability errors. The result quality still varies:
usable documents are the majority; AdsWizz has an unrelated site null-element
error; Conviva and TikTok remain no-document; Nexx360 reaches `HeadParsed`
after the IndexedDB crash fix; and OCPS remains a Typekit site-configuration
case. These are replay results, not a replacement for a full schema-bearing
corpus rerun.

### WebGL compatibility backend

The shared compatibility layer now exposes coherent WebGL/WebGL2 branding,
prototypes, constructors, `Symbol.toStringTag`, receiver-compatible prototype
calls, required extensions, capability limits, shader precision constants, and
context attributes. Its fake object contract now tracks shader/program/buffer/
texture/framebuffer/renderbuffer lifecycle and reports compile/link/identity
queries coherently. It remains a non-rendering compatibility backend; it does
not claim to produce real pixels.

The fake context also tracks active texture units, buffer/texture/framebuffer
bindings, viewport/scissor, clear color, enable/disable state, blend factors,
depth/stencil parameters, basic uniform values, and vertex attribute state. The
backend exposes `__browsaiNonRendering` and
`__browsaiPixelOutput="unavailable"`; `readPixels()` reports
`INVALID_OPERATION` without writing fabricated pixel data. This distinguishes
a compatibility API/state surface from a renderer that can produce real
pixels.
The runtime also exposes `window.__browsaiWebGLBackend` as `real` or `fake`,
separately from the compatibility mode string.
The compatibility prototype installs its fallback `getParameter` with
non-enumerable, writable, configurable native-style descriptor attributes;
the adapter regression checks these attributes.

Fresh Stripe validation reached `Complete` with the expected Stripe title and
zero runtime messages. The failure was traced to missing WebGL2 constants that
caused Three.js's first renderer construction to throw; Stripe interpreted the
fallback construction as a major-performance caveat.

### Browser identity consistency

The adapter regression now checks Chrome UA, Google vendor, Linux platform,
`window.chrome`, user-agent data, `webdriver=false`, language/Intl/timezone,
viewport, DPR, and screen dimensions together. In headless mode, zero host
screen metrics are normalized to a 1920x1080 desktop screen; nonzero host
metrics are preserved.

### Result classification and provenance

`scripts/run_google_corpus.py` now persists `root_cause`, `error_source`,
`navigation`, and `execution` fields. Current root-cause categories include
command/evaluation/probe/navigation timeouts, no-document navigation,
transport-error pages, WebGL capability failures, OffscreenCanvas failures,
font API failures, security/network policy failures, rate limits, and general
JavaScript/API failures.

Navigation records include requested URL, final URL, load status, document
creation, commit status, redirect observation, the engine-provided URL history
as `redirect_chain`, stage, and failure reason. The real-runtime adapter test
also exercises a bounded localhost 302→HTML redirect and combines the
main-frame request trace with committed history, excluding subresources.
The main-frame request trace is capped at 256 URLs and reports
`history_truncated` when that cap is reached.
Structured HTTP 403 and 429 statuses now take precedence in corpus
classification; a controlled 403 fixture verifies status/MIME persistence.
The same fixture now serves a same-origin module page and JavaScript dependency,
verifying static imports, dynamic `import()`, an import map, top-level
`await`, JavaScript MIME handling, and rejection of a missing module.
The runtime compatibility fixtures verify the WebSocket URL/protocol,
ready-state constants, default binary type, duplicate-protocol validation, and
a bounded localhost RFC handshake with subprotocol negotiation, a text frame,
an `arraybuffer` binary frame, and a normal close event. Redirects, TLS, and
worker exposure remain unverified.
The persisted navigation record also marks repeated URLs as
`redirect_loop_observed` and assigns stage `redirect_loop` without treating the
result as a committed document.
Committed-document MIME is populated from `document.contentType`, and the
vendored Servo parser now exposes its already-stored status through the
non-standard, non-enumerable diagnostic `document.__browsaiStatusCode`;
`live-open` persists both values. The diagnostic property is page-visible only
in the explicitly enabled diagnostic mode; normal runtime pages do not receive
it.
The real-runtime fixture covers 302→HTML, redirect-loop termination, 404
`text/plain`, `application/octet-stream` attachment metadata, and same-origin
content loaded only through a child frame. Native download-event delivery
remains the outstanding navigation fixture gap.
Execution records now include elapsed time, retry count, timeout budget, last
successful stage, DOM state, network state, runtime-error count, and explicit
nulls for pending-task/semantic-generation/heartbeat telemetry that the current
engine does not expose yet. For a hard command timeout, `live-open` now also
emits and preserves the last observed process stage (`startup`, `navigation`,
`stability`, `dom_projection`, `interaction`, or `evaluation`); this narrows
the diagnosis to the process phase but does not claim that DNS, TLS, resource,
or engine-internal waits have been independently instrumented.
Thirty-five formerly timeout-classified replays now have typed outcomes. `x.com`,
`branch.io`, `cisco.com`, `salesforce.com`, `aliyun.com`,
`collegeboard.org`, `metadata.templates.cdn.office.net`, `appsflyer.com`,
`twitter.com`, `webshell.suite.office.com`, `canvaslms.com`,
`cdn.jwplayer.com`, `cdn.mgaru.dev`, and `classroom.google.com` complete through `evaluation`
with no runtime messages and end as `NAVIGATION_NO_DOCUMENT`. `api.iterable.com`
loads its API Explorer, `auvik.com` loads its document with separate
MediaRecorder/vendor messages, and `bounceexchange.com` loads its Wunderkind
redirect target with separate site-script errors. `datadoghq.com` reaches a
Datadog redirect/document, `entitlements.jwplayer.com` reaches a JWX document,
and `evergage.com` reaches Salesforce Personalization with separate service/
site-policy errors. `maps.google.com` reaches Google Maps without runtime
errors, and `player.vimeo.com` reaches Vimeo with generic site warnings;
`1drv.com` produces a certificate/error-page navigation; and `atlassian.com`,
`mapbox.com`, `bidtheatre.com`, `express.adobe.com`, `onedrive.com`, and
`posthog.com` reproduce `EVALUATION_TIMEOUT` at the
snapshot policy boundary. `vimeo.com`, `visualwebsiteoptimizer.com`,
`powerapps.com`, and `pptsgs.officeapps.live.com` now also complete through
evaluation as `NAVIGATION_NO_DOCUMENT`. The count exceeds the nominal
33-site corpus-002 timeout target because the persisted campaign state includes
related records from both campaign partitions; this does not substitute for a
fresh schema-bearing rerun. The College Board and Office metadata runs also confirm that their
previous Servo script-thread panics no longer occur. `sectigo.com` now has a
precise `DOM_PROJECTION_TIMEOUT` classification, while `tenable.com` remains an
`EVALUATION_TIMEOUT`. The remaining historical timeout records are not
reclassified until their fresh replay produces the same level of evidence.
Runtime failures also persist structured `runtime_events` alongside the raw
error text: exception type, message, missing symbol, realm, script URL,
line/column, nullable security mechanism and CORS/CSP/security-error signal,
and explicit nulls where stack/API/initialization-stage data is not available.
Each schema-bearing record also carries `network_policy` with page origin,
redirect chain, detected policy signals, CORS/CSP/security-error decisions,
classification, and an explicit `missing_fields` list. The record is marked
`telemetry_complete: false` until the embedder exposes subresource target
origin, request mode/credentials, request and response headers, and preflight
outcomes; those values are not fabricated from a page-level console string.
The security fields are descriptive only; they do not assert that BrowsAI's
policy differs from Chrome. Projection is bounded to 100 events and 64 KiB per site, with
`runtime_events_truncated` and `runtime_events_bytes` recorded when the limit
is reached. This makes future site replay triageable without allowing noisy
diagnostics to become unbounded output.
Chrome comparison now covers all six original SecurityError URLs. Isolated
Chrome replays produced no matching `SecurityError` console entries:

| Requested URL | Chrome result | Comparison quality |
| --- | --- | --- |
| `copper6.com` | Stayed on requested URL | Same URL; environment/content may differ |
| `flurry.com` | Redirected to `www.flurry.com` | Redirected |
| `simpli.fi` | Stayed on requested URL | Same URL; environment/content may differ |
| `siteintercept.qualtrics.com` | Redirected to Qualtrics login | Authentication state |
| `stackadapt.com` | Redirected to `www.stackadapt.com` | Redirected |
| `optimizely.com` | Redirected to `www.optimizely.com` | Redirected |

The four current-state SecurityError records were also replayed in Chrome and
produced no matching errors, but their endpoints changed or served current
content. These results are recorded as `UNKNOWN`, not
`BROWSAI_POLICY_MISMATCH`, because a same-origin, same-session comparison is
not established.
The two reproducible committed CORS cases were also opened in isolated Chrome:
`quantummetric.com` reached `www.quantummetric.com` and `us.tiktok.com` stayed
on `us.tiktok.com`; neither emitted a page-console CORS/CSP signal through the
available Chrome log surface. This is negative console evidence only—not proof
that subresource decisions matched—because request-level headers, modes,
credentials, and preflight outcomes were not exposed by that surface.
The browser-identity fixture also verifies that an HTML document without a
`lang` attribute keeps the standard empty `document.documentElement.lang`
default independently of the configured `navigator.language`.

A fresh live replay of `https://www.siriusxm.com/` ended at `about:blank` with
`load_status: Complete`, no runtime messages, and no committed document. It
therefore does not reproduce the historical `document.fonts.add is not a
function` signature; the controlled FontFaceSet fixture remains the available
API-semantic evidence, and no SiriusXM-specific shim was added.

SVG/Lottie compatibility now supplies attribute-derived `SVGSVGElement.width`
and `.height` animated-length objects with `baseVal.value`; the adapter verifies
both dimensions. This addresses the historical `svg.width is undefined` /
`width.baseVal` signature without claiming animation semantics.

The runtime also supplies a minimal direct `Animation` constructor when the
native constructor is absent, including control methods and settled `ready` /
`finished` promises. The adapter verifies construction and state transitions;
this does not claim full timeline rendering semantics.

The FontFace compatibility surface now includes `FontFace` and the commonly
used `FontFaceSet` methods (`load`, `add`, `delete`, `clear`, `check`, and
`ready`). The real-runtime adapter asserts that surface. A SiriusXM replay did
not reach a committed document (`about:blank`), so that site-level result is
not yet counted as fixed; native font-set events and a committed-page replay
remain outstanding.

The same runtime now exposes a bounded, page-local `navigator.cookieStore`
surface when Servo lacks the native API. It only returns values written through
that surface and never reads existing cookies or creates credentials; the
adapter identity regression asserts its method presence.

The adapter also asserts that an HTML canvas returns a 2D context with a
callable `fillRect`. This rules out a missing 2D projection as the cause of
the historical VideoAmp `fillRect`-on-null error; that signature remains a
site/third-party initialization failure pending a committed-page replay.

The live CLI now accepts bounded caller-supplied cursors and limits for link,
control, and textbox projections and reports explicit truncation and next
cursors. `live-open` and `live-search` additionally enforce configurable link
byte and duration budgets and report actual usage; the bounded-page regression
covers cursor, item, byte, and continuation behavior. The shared `agent-protocol::QueryPage` and state snapshot pages carry
the same metadata while retaining `next_offset` compatibility. Servo DOM/agent
tree projection now caps traversal at 5,000 nodes, bounds projected text to 500
characters, and reports `AgentRenderTree.truncated` (also surfaced by the CLI as
`agent_tree_truncated`). A 6,001-node regression completes with bounded output;
broader non-link probe byte/time budgets remain future work.

### Generic runtime/API decomposition

The legacy state does not contain schema-level root-cause fields, so this is an
evidence-only replay of its error strings rather than a claim that every site
has been freshly retried. Applying the current classifier to the 75 historical
`fail` records yields these concrete groups:

| Classification | Sites | Count |
| --- | --- | ---: |
| `SITE_DOM_ASSUMPTION` | admaster.cc, avast.com, bluecava.com, clinch.co, datadome.co, ninjarmm.com, privacyportal.onetrust.com, service-now.com, usercentrics.eu, userway.org, videoamp.com | 11 |
| `SITE_DEPENDENCY_OR_ORDER` | autodesk.com, cdn.optable.co, connectad.io, lab.amplitude.com, lenovosoftware.com, mail.yahoo.com, optable.co, pub.doubleverify.com, rakuten.com, rapidssl.com | 10 |
| `SITE_MALFORMED_SCRIPT` | ping.chartbeat.net, trendmicro.com, zemanta.com | 3 |
| `SITE_RECURSION` | nrich.ai | 1 |
| `WEBSOCKET_EXTERNAL` | roblox.com | 1 |
| `AUTH_SESSION_REQUIRED` | goguardian.com, mobile.zscaler.net, mobile.zscalerone.net, mobileadmin.zscalerone.net, travelaudience.com | 5 |
| `FONT_API` | siriusxm.com | 1 |
| `SITE_TELEMETRY_USAGE` | amazonvideo.com, osano.com | 2 |

The remaining legacy strings are already assigned to OffscreenCanvas (24),
security/network policy (6), WebGL (3), site configuration (4), hCaptcha
verification (1), HTTP 429 (1), and one action timeout. The current classifier
partitions all 75 legacy `fail` records with zero `GENERAL_JS_API` results; the
CSP wording from `api.onedrive.com` is now classified as network policy. A fresh
schema-bearing rerun is still required to capture stack, realm, API, and
initialization-stage fields for each site before this decomposition can be
marked complete.

Authentication and verification signatures are classified without bypassing
them: hCaptcha messages are `AUTH_OR_HUMAN_VERIFICATION`, Zscaler invalid-token
messages are `AUTH_SESSION_REQUIRED`, and Optimizely missing-feature-key
messages are `SITE_CONFIGURATION`. The regression suite covers the hCaptcha
and Zscaler signatures; no challenge solving or credential synthesis is used.

### Vendor startup classifications

The corpus `iqm.com` record reported `window.zitag` as undefined from
`zi-tag.js`, but a fresh isolated replay completed without that error. The
current `zi-tag.js` response initializes its own `window.zitag` object and
defines `flushTelemetry`; no browser-native shim is warranted. The fresh replay
instead exposed an unrelated site logging error, so the original observation is
classified as flaky vendor initialization.

The Moloco record reported `hbspt is not defined` and a Marquee initialization
message while the page was still loading. The page includes the HubSpot Forms
dependency before its inline `hbspt.forms.create` call, and the dependency
currently responds successfully. A fresh replay no longer reports the HubSpot
error; only the vendor-side “No marquee instances found” message remains. This
group is classified as flaky external/vendor startup, with no site-specific
shim or credential synthesis.

### Worker realm coverage

The adapter regression now measures Window, classic blob DedicatedWorker, and
blob module DedicatedWorker paths, including OffscreenCanvas dimensions and 2D
context creation. Both worker paths currently return the expected
`function|true|2|2` result. SharedWorker now returns the complete expected
matrix; the earlier timeout was caused by the fixture closing the worker before
its queued MessagePort task ran. Servo's native ServiceWorker manager is now
enabled, and the local registration fixture successfully sends a message into
the registered ServiceWorker realm. That realm reports `OffscreenCanvas`,
`fetch`, `WebSocket`, `crypto`, `TextEncoder`, and `URL` exposure. ServiceWorker
globals now receive a shared inert image-cache boundary: `new OffscreenCanvas(320,
180)` and `getContext('2d')` complete without panic or hang and report the
requested dimensions. Image loading/rasterization remains explicitly
unsupported in that realm because there is no document/pipeline image cache.

## Validation

- `cargo build --bin browsai --all-features` passes.
- `cargo test -p browsai-engine-servo --all-features` passes: 7 active tests,
  with one legacy real-runtime test ignored because Servo permits one process-
  global runtime initialization per test process.
- The ignored runtime JavaScript test passes when run separately with
  `--ignored`.
- Python classification, legacy 2,000-record audit, and navigation-provenance
  fixtures pass.
- `cargo fmt --all` passes.

### Post-fix metric definitions

For every corpus state, the report will count terminal records by `status` and
`root_cause`: runtime failures are `status=fail`, external blocks are
`status=blocked_external`, site-side errors are failures whose
`error_source=WEBSITE_SCRIPT` or `THIRD_PARTY_SCRIPT`, flaky failures are
records whose reproducibility is `FLAKY`, unique root causes are distinct
`root_cause_id` values, and regressions are records labeled `REGRESSION`.
The baseline is 75 runtime failures, 150 external blocks, 4 redirect skips,
and 1,771 passes; comparisons must use the same terminal-record denominator.

## Remaining work

Timeout stage instrumentation, full navigation-cause decomposition, worker
realm parity, fake WebGL object/state completeness, security/CORS comparison,
vendor dependency investigations, fresh corpus-003, and the final zero-open-
items completion gate remain outstanding. See `HARDENING_TODO.md` for the
authoritative checklist.
