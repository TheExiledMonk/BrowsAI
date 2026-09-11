#!/usr/bin/env python3
"""Regression tests for corpus result classification and legacy auditing."""

from __future__ import annotations

import importlib.util
import json
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "run_google_corpus", ROOT / "scripts" / "run_google_corpus.py"
)
assert SPEC and SPEC.loader
runner = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(runner)


class CorpusClassificationTests(unittest.TestCase):
    def test_run_json_timeout_terminates_the_probe_process_group(self) -> None:
        code, payload, error = runner.run_json(
            sys.executable,
            ["-c", "import time; print('probe-start', flush=True); time.sleep(2)"],
            timeout=0.1,
        )
        self.assertEqual(code, 124)
        self.assertIsNone(payload)
        self.assertIn("probe-start", error)

    def test_outcome_classes_are_mutually_exclusive(self) -> None:
        expected = {
            "NONE": "PASS",
            "UNKNOWN_TIMEOUT": "TIMEOUT",
            "NAVIGATION_NO_RESULT": "NAVIGATION_OR_TRANSPORT",
            "NAVIGATION_NO_DOCUMENT": "NAVIGATION_OR_TRANSPORT",
            "ENGINE_CRASH": "ENGINE_CRASH",
            "WEBGL_CAPABILITY": "BROWSER_CAPABILITY",
            "MEDIA_CAPABILITY": "BROWSER_CAPABILITY",
            "GENERAL_JS_API": "RUNTIME_OR_SCRIPT",
            "SITE_SECURITY_POLICY": "EXTERNAL_OR_SITE_POLICY",
            "HTTP_409_EXTERNAL": "EXTERNAL_OR_SITE_POLICY",
            "SITE_CONFIGURATION": "EXTERNAL_OR_SITE_POLICY",
            "AUTH_SESSION_REQUIRED": "EXTERNAL_OR_SITE_POLICY",
            "WEBSOCKET_EXTERNAL": "RUNTIME_OR_SCRIPT",
            "SITE_MALFORMED_SCRIPT": "RUNTIME_OR_SCRIPT",
            "SITE_RECURSION": "RUNTIME_OR_SCRIPT",
            "SITE_DOM_ASSUMPTION": "RUNTIME_OR_SCRIPT",
            "SITE_DEPENDENCY_OR_ORDER": "RUNTIME_OR_SCRIPT",
            "SITE_TELEMETRY_USAGE": "RUNTIME_OR_SCRIPT",
        }
        for root_cause, outcome in expected.items():
            status = "pass" if root_cause == "NONE" else "fail"
            self.assertEqual(runner.outcome_class(root_cause, status=status), outcome)

    def test_empty_result_is_navigation_not_capability(self) -> None:
        self.assertEqual(
            runner.classify_result(
                code=0,
                payload=None,
                error="empty output",
                diagnostics_error=None,
                diagnostics_text="",
                runtime_messages=[],
                external_block=True,
                runtime_errors=[],
            ),
            ("NAVIGATION_NO_RESULT", "BROWSAI_NETWORK"),
        )

    def test_renderer_crash_is_not_misclassified_as_navigation(self) -> None:
        self.assertEqual(
            runner.classify_result(
                code=-11,
                payload=None,
                error="thread caused non-unwinding panic; Redirecting call to abort() to mozalloc_abort",
                diagnostics_error=None,
                diagnostics_text="",
                runtime_messages=[],
                external_block=False,
                runtime_errors=[],
            ),
            ("ENGINE_CRASH", "BROWSAI_ENGINE"),
        )

    def test_structured_http_policy_status_precedes_page_text(self) -> None:
        common = {
            "code": 0,
            "error": "",
            "diagnostics_error": None,
            "diagnostics_text": "ordinary response body",
            "runtime_messages": [],
            "external_block": False,
            "runtime_errors": [],
        }
        for status, expected in ((403, "HTTP_403_EXTERNAL"), (429, "HTTP_429_RATE_LIMIT")):
            root_cause, source = runner.classify_result(
                payload={
                    "url": "https://example.test/",
                    "navigation": {"http_status": status},
                },
                **common,
            )
            self.assertEqual((root_cause, source), (expected, "EXTERNAL_SERVICE"))

    def test_navigation_provenance_labels_observed_policy_results(self) -> None:
        def provenance(status: int | None, root_cause: str = "") -> dict:
            payload = {"url": "https://example.test/", "navigation": {}}
            if status is not None:
                payload["navigation"]["http_status"] = status
            return runner.navigation_provenance(
                requested_url="https://example.test/",
                payload=payload,
                code=0,
                error="",
                root_cause=root_cause,
            )

        self.assertEqual(
            provenance(403, "HTTP_403_EXTERNAL")["network_policy_classification"],
            "SITE_POLICY_EXPECTED",
        )
        self.assertEqual(
            provenance(429, "HTTP_429_RATE_LIMIT")["network_policy_classification"],
            "EXTERNAL_RATE_LIMIT",
        )
        self.assertEqual(
            provenance(None, "SECURITY_OR_NETWORK_POLICY")["network_policy_classification"],
            "UNKNOWN",
        )
        self.assertIsNone(provenance(200)["network_policy_classification"])

    def test_optimizely_missing_feature_key_is_site_configuration(self) -> None:
        self.assertEqual(
            runner.classify_result(
                code=0,
                payload={"url": "https://example.test/"},
                error="",
                diagnostics_error=None,
                diagnostics_text="",
                runtime_messages=[],
                external_block=False,
                runtime_errors=[
                    "Error: [OPTIMIZELY] Feature key undefined is not in datafile."
                ],
            ),
            ("SITE_CONFIGURATION", "WEBSITE_SCRIPT"),
        )

    def test_typekit_domain_mismatch_is_site_configuration(self) -> None:
        self.assertEqual(
            runner.classify_result(
                code=0,
                payload={"url": "https://canvas.ocps.net/"},
                error="",
                diagnostics_error=None,
                diagnostics_text="",
                runtime_messages=[
                    'Error: Typekit: the domain "canvas.ocps.net" is not in the list of published domains for kit "example".'
                ],
                external_block=False,
                runtime_errors=[
                    'Error: Typekit: the domain "canvas.ocps.net" is not in the list of published domains for kit "example".'
                ],
            ),
            ("SITE_CONFIGURATION", "WEBSITE_SCRIPT"),
        )

    def test_servo_panics_inside_timeout_wrappers_are_engine_failures(self) -> None:
        cases = (
            (
                "internal error: entered unreachable code: Removed @font-face that was not previously present",
                "FONT_FACE_SET_PANIC",
            ),
            (
                "assertion failed: self.can_run_script() at globalscope.rs",
                "GLOBAL_SCRIPT_READINESS_PANIC",
            ),
        )
        for error, expected in cases:
            self.assertEqual(
                runner.classify_result(
                    code=124,
                    payload=None,
                    error=error,
                    diagnostics_error=None,
                    diagnostics_text="",
                    runtime_messages=[],
                    external_block=False,
                    runtime_errors=[],
                )[0],
                expected,
            )

    def test_vendor_startup_errors_are_external_third_party_failures(self) -> None:
        for message in (
            'Error: window.zitag is undefined',
            'Error: hbspt is not defined; Error: [marquee] No marquee instances found',
        ):
            self.assertEqual(
                runner.classify_result(
                    code=0,
                    payload={"url": "https://example.test/"},
                    error="",
                    diagnostics_error=None,
                    diagnostics_text="",
                    runtime_messages=[message],
                    external_block=True,
                    runtime_errors=[message],
                ),
                ("VENDOR_DEPENDENCY", "THIRD_PARTY_SCRIPT"),
            )

    def test_supported_timeout_subtypes_remain_distinct(self) -> None:
        common = {
            "payload": {"url": "https://example.test/", "diagnostics": {}},
            "diagnostics_error": None,
            "diagnostics_text": "",
            "runtime_messages": [],
            "external_block": False,
            "runtime_errors": [],
        }
        cases = [
            (124, "", None, "UNKNOWN_TIMEOUT"),
            (0, "", "page evaluation timeout exceeds policy", "EVALUATION_TIMEOUT"),
            (0, "probe timeout while extracting links", None, "PROBE_TIMEOUT"),
            (0, "navigation timeout waiting for commit", None, "NAVIGATION_TIMEOUT"),
            (0, "event loop hang exceeded deadline", None, "EVENT_LOOP_TIMEOUT"),
        ]
        for code, error, diagnostic, expected in cases:
            arguments = dict(common)
            arguments.update(
                code=code,
                error=error,
                diagnostics_error=diagnostic,
            )
            root_cause, _ = runner.classify_result(**arguments)
            self.assertEqual(root_cause, expected, (error, diagnostic))

    def test_stage_specific_timeout_subtypes_are_preserved(self) -> None:
        cases = [
            ("DNS timeout resolving host", "DNS_TIMEOUT"),
            ("TLS handshake timed out", "TLS_TIMEOUT"),
            ("HTTP response timeout waiting for response", "HTTP_RESPONSE_TIMEOUT"),
            ("resource load timeout for subresource", "RESOURCE_LOAD_TIMEOUT"),
            ("script execution timeout", "SCRIPT_EXECUTION_TIMEOUT"),
            ("event loop hang exceeded deadline", "EVENT_LOOP_TIMEOUT"),
            ("semantic stability timeout", "SEMANTIC_STABILITY_TIMEOUT"),
            ("DOM projection timeout", "DOM_PROJECTION_TIMEOUT"),
            ("layout observation timeout", "LAYOUT_TIMEOUT"),
            ("Agent Tree timeout", "AGENT_TREE_TIMEOUT"),
            ("click action timeout", "ACTION_TIMEOUT"),
        ]
        for error, expected in cases:
            root_cause, _ = runner.classify_result(
                code=0,
                payload={"url": "https://example.test/"},
                error=error,
                diagnostics_error=None,
                diagnostics_text="",
                runtime_messages=[],
                external_block=False,
                runtime_errors=[],
            )
            self.assertEqual(root_cause, expected, error)

    def test_generic_runtime_signatures_are_decomposed(self) -> None:
        cases = (
            ("InvalidTokenError: Invalid token specified", "AUTH_SESSION_REQUIRED"),
            ("Missing config element # application_config", "SITE_CONFIGURATION"),
            ("Unable to retrieve the configuration value for marketplace-store-onetrust-settings", "SITE_CONFIGURATION"),
            ("Multiple scripts found, cannot determine custom domain", "SITE_CONFIGURATION"),
            ("PMWall Error: Script onLoading Error. URL: https://elpais.com/arc/subs/p.min.js", "VENDOR_DEPENDENCY"),
            ("WebSocket failed to connect to the server", "WEBSOCKET_EXTERNAL"),
            ("too much recursion", "SITE_RECURSION"),
            ("missing ] after element list", "SITE_MALFORMED_SCRIPT"),
            ("JSON.parse: unexpected character at line 1 column 1 of the JSON data", "SITE_DEPENDENCY_OR_ORDER"),
            ('BROWSAI_RUNTIME_EVENT {"message":"can\'t access property "length" of null","script_url":"https://k.twitchcdn.net/ips.js"}', "VENDOR_DEPENDENCY"),
            ('[analytics.js] Failed to load Analytics.js (new ChunkLoadError("Loading chunk 50 failed"))', "VENDOR_DEPENDENCY"),
            ('https://client.aps.amazon-adsystem.com/publisher.js:3 can\'t access property "get"', "VENDOR_DEPENDENCY"),
            ("https://www.nike.com/149e9513/ips.js?KP_UID=challenge", "VENDOR_DEPENDENCY"),
            ("Could not clear consent from root domain.", "VENDOR_DEPENDENCY"),
            ("[CMP Monitor] Osano failure: timeout Osano did not initialize within 2000ms", "VENDOR_DEPENDENCY"),
            ("https://static.foxnews.com/static/isa/core-app.js f.cookie is not a function", "VENDOR_DEPENDENCY"),
            ("google_tag_manager.rm[1438083] split is undefined", "VENDOR_DEPENDENCY"),
            ("Clip component failed to load #data-cp-clip-id", "VENDOR_DEPENDENCY"),
            ("https://sleeknotestaticcontent.sleeknote.com/production/package-anchored.js e.style is undefined", "VENDOR_DEPENDENCY"),
            ("https://www.googletagmanager.com/gtm.js $ is not a function", "VENDOR_DEPENDENCY"),
            ("https://transcend-cdn.com/cm/airgap.js H is undefined", "VENDOR_DEPENDENCY"),
            ("https://js.datadome.co/K89D9F.js", "VENDOR_DEPENDENCY"),
            ("https://static.tacdn.com/assets/ consentPayload is null", "VENDOR_DEPENDENCY"),
            ("https://assets.targetimg1.com/ssx/ssx.mod.js W is null", "VENDOR_DEPENDENCY"),
            ("can't access property querySelector, NAV is null", "SITE_DOM_ASSUMPTION"),
            ("ScrollTrigger is not defined", "SITE_DEPENDENCY_OR_ORDER"),
            ("wp.i18n.setLocaleData is undefined", "SITE_DEPENDENCY_OR_ORDER"),
            ("window.jsc is not a function", "SITE_DEPENDENCY_OR_ORDER"),
            ("href.split is not a function", "SITE_DEPENDENCY_OR_ORDER"),
            ("sinaSSOController is not defined", "SITE_DEPENDENCY_OR_ORDER"),
            ("$.datepicker is undefined", "SITE_DEPENDENCY_OR_ORDER"),
            ("PostHog.js Unique user id has not been set in posthog.identify", "SITE_TELEMETRY_USAGE"),
            ("The pagetype parameter is required when you include the prodid parameter.", "SITE_TELEMETRY_USAGE"),
            ("_paq is not defined", "SITE_TELEMETRY_USAGE"),
            ("Empty events for tab all and bucket primary_bucket", "SITE_TELEMETRY_USAGE"),
            ("HLS not supported in this browser", "MEDIA_CAPABILITY"),
            ("https://mfe.fantascope.uol.com.br/player/news/fantascope-player.js Browser not supported!", "MEDIA_CAPABILITY"),
            ("Firebase SDK messaging/unsupported-browser", "PUSH_MESSAGING_CAPABILITY"),
            ("can't access property __tcfapiCall, a is null; 3000ms timeout exceeded", "VENDOR_DEPENDENCY"),
            ("JSONP request to //i.alibaba.com/ajax/user_portrait.htm timed out", "EXTERNAL_SERVICE_TIMEOUT"),
            ("Fetching /coordination/flags from coordinator timed out", "EXTERNAL_SERVICE_TIMEOUT"),
            ("[api-cache] idle GC failed; Loading timeout", "EXTERNAL_SERVICE_TIMEOUT"),
            ("https://www.aliexpress.com/_____tmd_____/punish?x5secdata=challenge", "AUTH_OR_HUMAN_VERIFICATION"),
            ('Invalid type on $input.time_zone, expect to be string & Pattern<"^[A-Za-z]+/[A-Za-z]">, UTC given', "SITE_CONFIGURATION"),
            ("https://3976941.fls.doubleclick.net/activityi ttd_dom_ready is not defined", "VENDOR_DEPENDENCY"),
            ("https://www.samsung.com/us/smartthings/ result.identity is undefined", "VENDOR_DEPENDENCY"),
            ("https://www.deloitte.com/etc.clientlibs/modern/components/modal-popup Granite is not defined", "VENDOR_DEPENDENCY"),
            ("https://www.state.gov/wp-content/themes/state/foresee_assets/code/fs.utils.js getVendorConfig(...).storage is undefined", "VENDOR_DEPENDENCY"),
            ("https://apps.rokt.com/wsdk/session-transfer/ could not parse URL: relative URL without a base", "VENDOR_DEPENDENCY"),
            ("https://js-cdn.dynatrace.com/jstag/ruxitagent.js overloadPrevention is undefined", "VENDOR_DEPENDENCY"),
        )
        for message, expected in cases:
            root_cause, _ = runner.classify_result(
                code=0,
                payload={"url": "https://example.test/"},
                error="",
                diagnostics_error=None,
                diagnostics_text="",
                runtime_messages=[message],
                external_block=False,
                runtime_errors=[message],
            )
            self.assertEqual(root_cause, expected, message)

    def test_timeout_detection_does_not_match_hang_substrings(self) -> None:
        root_cause, source = runner.classify_result(
            code=0,
            payload={"url": "https://example.test/"},
            error="",
            diagnostics_error=None,
            diagnostics_text="",
            runtime_messages=["[TikTok Pixel] Missing value parameter for the highest value customers"],
            external_block=False,
            runtime_errors=[],
        )
        self.assertEqual((root_cause, source), ("NONE", "UNKNOWN"))

    def test_subresource_http_403_is_external_service_policy(self) -> None:
        message = "Error: BROWSAI_RUNTIME_EVENT {\"message\":\"Response not successful: Received status code 403\"}"
        self.assertEqual(
            runner.classify_result(
                code=0,
                payload={"url": "https://example.test/", "navigation": {}},
                error="",
                diagnostics_error=None,
                diagnostics_text="",
                runtime_messages=[message],
                external_block=True,
                runtime_errors=[message],
            ),
            ("HTTP_403_EXTERNAL", "EXTERNAL_SERVICE"),
        )

    def test_explicit_subresource_status_precedes_action_timeout(self) -> None:
        for status, expected in ((403, "HTTP_403_EXTERNAL"), (409, "HTTP_409_EXTERNAL")):
            message = f'Request failed with status code {status}; action timeout followed'
            self.assertEqual(
                runner.classify_result(
                    code=0,
                    payload={"url": "https://example.test/", "navigation": {}},
                    error="action timeout",
                    diagnostics_error=None,
                    diagnostics_text="",
                    runtime_messages=[message],
                    external_block=False,
                    runtime_errors=[message],
                )[0],
                expected,
            )

    def test_subresource_http_409_is_external_service_policy(self) -> None:
        message = "Error: BROWSAI_RUNTIME_EVENT {\"message\":\"Response not successful: Received status code 409\"}"
        self.assertEqual(
            runner.classify_result(
                code=0,
                payload={"url": "https://example.test/", "navigation": {}},
                error="",
                diagnostics_error=None,
                diagnostics_text="",
                runtime_messages=[message],
                external_block=True,
                runtime_errors=[message],
            ),
            ("HTTP_409_EXTERNAL", "EXTERNAL_SERVICE"),
        )

    def test_new_capability_and_http_409_causes_have_stable_root_cause_ids(self) -> None:
        self.assertEqual(runner.ROOT_CAUSE_IDS["MEDIA_CAPABILITY"], "RC-115")
        self.assertEqual(runner.ROOT_CAUSE_IDS["HTTP_409_EXTERNAL"], "RC-014")
        self.assertEqual(runner.ROOT_CAUSE_IDS["EXTERNAL_SERVICE_TIMEOUT"], "RC-014")

    def test_every_reportable_root_cause_has_catalog_id(self) -> None:
        reportable = {
            "OFFSCREENCANVAS", "WEBGL_CAPABILITY", "EVALUATION_CAPABILITY_DENIED",
            "EVALUATION_TIMEOUT", "UNKNOWN_TIMEOUT", "PROBE_TIMEOUT",
            "NAVIGATION_TIMEOUT", "DNS_TIMEOUT", "TLS_TIMEOUT",
            "HTTP_RESPONSE_TIMEOUT", "RESOURCE_LOAD_TIMEOUT", "SCRIPT_EXECUTION_TIMEOUT",
            "EVENT_LOOP_TIMEOUT", "SEMANTIC_STABILITY_TIMEOUT", "DOM_PROJECTION_TIMEOUT",
            "LAYOUT_TIMEOUT", "AGENT_TREE_TIMEOUT", "ACTION_TIMEOUT",
            "NAVIGATION_NO_DOCUMENT", "NAVIGATION_NO_RESULT", "NAVIGATION_TRANSPORT_ERROR",
            "FONT_API", "GENERAL_JS_API", "SECURITY_OR_NETWORK_POLICY",
            "SITE_SECURITY_POLICY", "HTTP_403_EXTERNAL", "HTTP_409_EXTERNAL",
            "HTTP_429_RATE_LIMIT", "EXTERNAL_SERVICE_TIMEOUT", "VENDOR_DEPENDENCY", "AUTH_OR_HUMAN_VERIFICATION",
            "AUTH_SESSION_REQUIRED", "SITE_CONFIGURATION", "WEBSOCKET_EXTERNAL",
            "SITE_MALFORMED_SCRIPT", "SITE_RECURSION", "SITE_DOM_ASSUMPTION",
            "SITE_DEPENDENCY_OR_ORDER", "SITE_TELEMETRY_USAGE", "MEDIA_CAPABILITY",
            "PUSH_MESSAGING_CAPABILITY",
            "EXTERNAL_BLOCK_UNCLASSIFIED", "ENGINE_CRASH", "FONT_FACE_SET_PANIC",
            "GLOBAL_SCRIPT_READINESS_PANIC",
        }
        self.assertEqual(reportable - set(runner.ROOT_CAUSE_IDS), set())

    def test_benign_ad_probe_message_is_not_counted_as_runtime_failure(self) -> None:
        messages = ["No ad placements found."]
        self.assertEqual(
            [message for message in messages if "no ad placements found" not in message.lower()],
            [],
        )

    def test_deprecation_console_errors_are_not_runtime_failures(self) -> None:
        messages = ["Error: .reset in derived store is deprecated, use .reset in store created via createStore instead"]
        self.assertEqual(
            [message for message in messages if " is deprecated" not in message.lower()],
            [],
        )

    def test_youtube_player_telemetry_is_not_runtime_failure(self) -> None:
        messages = ["Error: Provided data is inadequate."]
        self.assertEqual(
            [message for message in messages if "provided data is inadequate" not in message.lower()],
            [],
        )

    def test_empty_runtime_diagnostics_are_not_failures(self) -> None:
        messages = ["Error: ", "Crash:"]
        self.assertEqual(
            [message for message in messages if message.strip().lower() not in {"error:", "crash:"}],
            [],
        )

    def test_hcaptcha_and_zscaler_are_classified(self) -> None:
        cases = (
            (
                "Error at https://www.hcaptcha.com/js/translations.js:204:12 window.setLang is not a function",
                "AUTH_OR_HUMAN_VERIFICATION",
                "EXTERNAL_SERVICE",
            ),
            (
                "InvalidTokenError: Invalid token specified: can't access property replace, a is undefined at https://mobileadmin.zscaler.net/jwt.js",
                "AUTH_SESSION_REQUIRED",
                "EXTERNAL_SERVICE",
            ),
        )
        for message, expected_root_cause, expected_source in cases:
            self.assertEqual(
                runner.classify_result(
                    code=0,
                    payload={"url": "https://example.test/"},
                    error="",
                    diagnostics_error=None,
                    diagnostics_text="",
                    runtime_messages=[message],
                    external_block=False,
                    runtime_errors=[message],
                ),
                (expected_root_cause, expected_source),
            )

    def test_command_timeout_uses_last_observed_live_open_stage(self) -> None:
        for stage, expected in (
            ("navigation", "NAVIGATION_TIMEOUT"),
            ("stability", "SEMANTIC_STABILITY_TIMEOUT"),
            ("dom_projection", "DOM_PROJECTION_TIMEOUT"),
            ("interaction", "ACTION_TIMEOUT"),
            ("evaluation", "EVALUATION_TIMEOUT"),
        ):
            root_cause, source = runner.classify_result(
                code=124,
                payload=None,
                error=(
                    "timeout after 75s: command timed out\n"
                    f"partial_stderr=BROWSAI_STAGE:{stage}"
                ),
                diagnostics_error=None,
                diagnostics_text="",
                runtime_messages=[],
                external_block=False,
                runtime_errors=[],
            )
            self.assertEqual((root_cause, source), (expected, "BROWSAI_ENGINE" if expected in {"ACTION_TIMEOUT", "EVALUATION_TIMEOUT"} else ("BROWSAI_NETWORK" if expected == "NAVIGATION_TIMEOUT" else "BROWSAI_PROJECTION")))

    def test_execution_provenance_records_last_observed_stage(self) -> None:
        timed_out = runner.execution_provenance(
                payload=None,
                diagnostics_error=None,
                elapsed_seconds=30.0,
                timeout_seconds=30,
                attempts=1,
                runtime_error_count=0,
                root_cause="UNKNOWN_TIMEOUT",
        )
        self.assertEqual(timed_out["last_successful_stage"], "process_start")
        self.assertTrue(timed_out["timed_out"])
        self.assertEqual(timed_out["timeout_subtype"], "UNKNOWN_TIMEOUT")
        self.assertEqual(timed_out["last_observed_stage"], None)

        completed = runner.execution_provenance(
                payload={
                    "url": "https://example.test/",
                    "load_status": "Complete",
                    "diagnostics": {"readyState": "complete", "bodyLength": 12},
                },
                diagnostics_error=None,
                elapsed_seconds=1.0,
                timeout_seconds=30,
                attempts=1,
                runtime_error_count=0,
                root_cause="NONE",
        )
        self.assertEqual(completed["last_successful_stage"], "dom_projection")
        self.assertFalse(completed["timed_out"])
        self.assertIsNone(completed["timeout_subtype"])

    def test_process_stage_marker_is_persisted_separately_from_success_state(self) -> None:
        self.assertEqual(
            runner.observed_process_stage(
                "timeout after 30s\npartial_stderr=BROWSAI_STAGE:dom_projection"
            ),
            "dom_projection",
        )
        provenance = runner.execution_provenance(
            payload=None,
            diagnostics_error=None,
            elapsed_seconds=30.0,
            timeout_seconds=30,
            attempts=1,
            runtime_error_count=0,
            root_cause="DOM_PROJECTION_TIMEOUT",
            observed_stage="dom_projection",
            observed_stages=["startup", "navigation", "dom_projection"],
        )
        self.assertEqual(provenance["last_successful_stage"], "process_start")
        self.assertEqual(provenance["last_observed_stage"], "dom_projection")
        self.assertEqual(
            provenance["stage_history"],
            ["startup", "navigation", "dom_projection"],
        )
        self.assertEqual(
            provenance["process_heartbeat"],
            {"observed": True, "marker_count": 3, "last_stage": "dom_projection"},
        )

    def test_runtime_events_preserve_raw_errors_and_extract_triage_fields(self) -> None:
        events = runner.runtime_event_provenance(
            [
                "Error: Error at blob:https://example.test/worker:1:105 OffscreenCanvas is not defined",
                'Error: (new TypeError("document.fonts.add is not a function", "https://example.test/app.js", 17))',
            ]
        )
        self.assertEqual(len(events), 2)
        self.assertEqual(events[0]["realm"], "blob-worker")
        property_event = runner.runtime_event_provenance(
            [
                'Error: Error at https://example.test/app.js:12:4 '
                'can\'t access property "height", globalThis.visualViewport is undefined'
            ]
        )
        self.assertEqual(
            property_event[0]["missing_symbol"], "globalThis.visualViewport.height"
        )
        self.assertEqual(events[0]["missing_symbol"], "OffscreenCanvas")
        self.assertEqual(events[0]["line"], 1)
        self.assertEqual(events[0]["column"], 105)
        self.assertEqual(events[1]["exception_type"], "TypeError")
        self.assertEqual(events[1]["script_url"], "https://example.test/app.js")
        self.assertEqual(events[1]["raw"].startswith("Error:"), True)

    def test_security_and_network_events_expose_mechanism_signals(self) -> None:
        events = runner.runtime_event_provenance(
            [
                'Error: uncaught exception: SecurityError: The operation is insecure.',
                'Error: (new SecurityError("The operation is insecure", "https://example.test/app.js", 4))',
                'Error: Network error: CORS check failed',
                'Error: Content Security Policy blocked inline script',
            ]
        )
        self.assertEqual(events[0]["security_mechanism"], "restricted_api")
        self.assertEqual(events[0]["network_policy_signal"], "security_error")
        self.assertEqual(events[1]["security_mechanism"], "restricted_api")
        self.assertEqual(events[2]["network_policy_signal"], "cors")
        self.assertEqual(events[3]["network_policy_signal"], "csp")

    def test_full_content_security_policy_wording_is_network_policy(self) -> None:
        message = "Unable to fetch assignment: Network error: Blocked by Content-Security-Policy"
        self.assertEqual(
            runner.classify_result(
                code=0,
                payload={"url": "https://example.test/"},
                error="",
                diagnostics_error=None,
                diagnostics_text="",
                runtime_messages=[message],
                external_block=False,
                runtime_errors=[message],
            ),
            ("SECURITY_OR_NETWORK_POLICY", "UNKNOWN"),
        )

    def test_structured_runtime_event_preserves_stack_and_initialization_context(self) -> None:
        events = runner.runtime_event_provenance([
            'Error: BROWSAI_RUNTIME_EVENT {"kind":"error","message":"boom",'
            '"stack":"Error: boom\\n at https://example.test/app.js:7:9",'
            '"script_url":"https://example.test/app.js","line":7,"column":9,'
            '"realm":"window-or-document","api":"fetch",'
            '"initialization_stage":"bootstrap"}'
        ])
        self.assertEqual(events[0]["message"], "boom")
        self.assertEqual(events[0]["stack"], "Error: boom\n at https://example.test/app.js:7:9")
        self.assertEqual(events[0]["script_url"], "https://example.test/app.js")
        self.assertEqual(events[0]["api"], "fetch")
        self.assertEqual(events[0]["initialization_stage"], "bootstrap")

    def test_network_policy_provenance_preserves_known_and_missing_fields(self) -> None:
        navigation = {
            "origin": "https://example.test",
            "redirect_chain": ["https://example.test/"],
            "network_policy_classification": None,
        }
        events = runner.runtime_event_provenance(
            ["Error: Network error: CORS check failed", "Error: Content Security Policy blocked inline script"]
        )
        policy = runner.network_policy_provenance(
            navigation=navigation,
            runtime_events=events,
            root_cause="SECURITY_OR_NETWORK_POLICY",
        )
        self.assertEqual(policy["page_origin"], "https://example.test")
        self.assertEqual(policy["redirect_chain"], ["https://example.test/"])
        self.assertEqual(policy["signals"], ["cors", "csp"])
        self.assertEqual(policy["cors_decision"], "blocked")
        self.assertEqual(policy["csp_decision"], "blocked")
        self.assertIsNone(policy["request_mode"])
        self.assertFalse(policy["telemetry_complete"])

    def test_runtime_event_projection_is_bounded_and_reports_truncation(self) -> None:
        messages, truncated, used_bytes = runner.bound_runtime_event_messages(
            ["a" * 8, "b" * 8, "c" * 8], limit=2, max_bytes=100
        )
        self.assertEqual(messages, ["a" * 8, "b" * 8])
        self.assertTrue(truncated)
        self.assertEqual(used_bytes, 16)

        messages, truncated, used_bytes = runner.bound_runtime_event_messages(
            ["é" * 20], limit=10, max_bytes=5
        )
        self.assertEqual(used_bytes, len(messages[0].encode("utf-8")))
        self.assertLessEqual(used_bytes, 5)
        self.assertTrue(truncated)

    def test_navigation_provenance_rejects_blank_and_transport_error_pages(self) -> None:
        initial_blank = runner.navigation_provenance(
            requested_url="https://example.test/",
            payload=None,
            code=0,
            error="empty output",
        )
        self.assertEqual(initial_blank["stage"], "initial_blank")
        self.assertFalse(initial_blank["document_created"])
        self.assertFalse(initial_blank["navigation_committed"])

        blank_document = runner.navigation_provenance(
            requested_url="https://example.test/",
            payload={"url": "about:blank", "load_status": "Complete"},
            code=0,
            error="",
        )
        self.assertEqual(blank_document["stage"], "no_document")
        self.assertFalse(blank_document["navigation_committed"])

        transport_error = runner.navigation_provenance(
            requested_url="https://example.test/",
            payload={
                "url": "https://example.test/",
                "load_status": "Complete",
                "diagnostics": {
                    "title": "Error loading page",
                    "bodyText": "Could not load the requested page",
                    "bodyLength": 31,
                },
            },
            code=0,
            error="",
        )
        self.assertEqual(transport_error["stage"], "transport_error_page")
        self.assertFalse(transport_error["document_created"])
        self.assertFalse(transport_error["navigation_committed"])
        self.assertEqual(transport_error["navigation_cause"], "TRANSPORT_ERROR_PAGE")

        committed = runner.navigation_provenance(
            requested_url="https://example.test/",
            payload={
                "url": "https://example.test/",
                "load_status": "Complete",
                "diagnostics": {"title": "Example", "bodyLength": 42},
            },
            code=0,
            error="",
        )
        self.assertEqual(committed["stage"], "committed")
        self.assertTrue(committed["document_created"])
        self.assertTrue(committed["navigation_committed"])
        self.assertEqual(committed["navigation_cause"], "COMMITTED_DOCUMENT")

    def test_navigation_provenance_classifies_http_and_unsupported_mime(self) -> None:
        http_error = runner.navigation_provenance(
            requested_url="https://example.test/missing",
            payload={
                "url": "https://example.test/missing",
                "load_status": "Complete",
                "navigation": {"http_status": 404, "mime_type": "text/plain"},
                "diagnostics": {"title": "Not found", "bodyLength": 9},
            },
            code=0,
            error="",
        )
        self.assertEqual(http_error["navigation_cause"], "HTTP_ERROR")

        download = runner.navigation_provenance(
            requested_url="https://example.test/file",
            payload={
                "url": "https://example.test/file",
                "load_status": "Complete",
                "navigation": {
                    "http_status": 200,
                    "mime_type": "application/octet-stream",
                },
                "diagnostics": {"title": "", "bodyLength": 0},
            },
            code=0,
            error="",
        )
        self.assertEqual(download["navigation_cause"], "UNSUPPORTED_MIME_OR_DOWNLOAD")

    def test_navigation_provenance_preserves_redirect_history_and_response_metadata(self) -> None:
        record = runner.navigation_provenance(
            requested_url="http://example.test/start",
            payload={
                "url": "https://example.test/final",
                "load_status": "Complete",
                "navigation": {
                    "history": [
                        "http://example.test/start",
                        "https://example.test/final",
                    ],
                    "redirect_observed": True,
                    "http_status": 200,
                    "mime_type": "text/html",
                    "origin": "https://example.test",
                    "initiator": None,
                    "resource_type": "main_frame",
                },
                "diagnostics": {"title": "Example", "bodyLength": 42},
            },
            code=0,
            error="",
        )
        self.assertTrue(record["navigation_committed"])
        self.assertTrue(record["redirect_observed"])
        self.assertEqual(record["redirect_chain"][-1], "https://example.test/final")
        self.assertEqual(record["http_status"], 200)
        self.assertEqual(record["mime_type"], "text/html")
        self.assertEqual(record["origin"], "https://example.test")
        self.assertIsNone(record["initiator"])
        self.assertEqual(record["resource_type"], "main_frame")

    def test_navigation_provenance_identifies_repeated_redirect_chain(self) -> None:
        record = runner.navigation_provenance(
            requested_url="http://example.test/a",
            payload={
                "url": "http://example.test/a",
                "load_status": "Complete",
                "navigation": {
                    "history": [
                        "http://example.test/a",
                        "http://example.test/b",
                        "http://example.test/a",
                    ],
                    "redirect_observed": True,
                },
                "diagnostics": {"title": "", "bodyLength": 0},
            },
            code=0,
            error="redirect limit exceeded",
        )
        self.assertEqual(record["stage"], "redirect_loop")
        self.assertTrue(record["redirect_loop_observed"])
        self.assertFalse(record["navigation_committed"])

    def test_legacy_state_partitions_to_two_thousand_records(self) -> None:
        state = json.loads((ROOT / "check-sites" / "new-2000-state.json").read_text())
        counts: dict[str, int] = {}
        for record in state["sites"].values():
            bucket = runner.legacy_audit_bucket(record)
            counts[bucket] = counts.get(bucket, 0) + 1
        self.assertEqual(
            counts,
            {
                "BROWSER_CAPABILITY": 29,
                "EXTERNAL_OR_SITE_POLICY": 57,
                "NAVIGATION_OR_TRANSPORT": 72,
                "PASS": 1771,
                "REDIRECT_POLICY": 4,
                "RUNTIME_OR_SCRIPT": 35,
                "TIMEOUT": 32,
            },
        )
        self.assertEqual(sum(counts.values()), 2000)

    def test_legacy_audit_does_not_mistake_set_timeout_for_a_timeout(self) -> None:
        record = {
            "status": "fail",
            "error": "Error: site script failed while calling setTimeout",
        }
        self.assertEqual(runner.legacy_audit_bucket(record), "RUNTIME_OR_SCRIPT")


if __name__ == "__main__":
    unittest.main()
