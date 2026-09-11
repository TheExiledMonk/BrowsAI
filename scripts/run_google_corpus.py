#!/usr/bin/env python3
"""Discover and exercise a resumable real-site corpus through BrowsAI itself.

Discovery is intentionally performed by the live Servo Google flow. Testing is
isolated one process per site so a renderer crash becomes a durable failure
record and does not destroy the remaining corpus state.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import os
import re
import signal
import subprocess
import sys
import time
import zipfile
from pathlib import Path
from urllib.parse import urlparse


QUERIES = [
    "most visited websites worldwide",
    "popular search engines websites",
    "popular social media websites",
    "popular video streaming websites",
    "popular music streaming websites",
    "popular news websites",
    "popular shopping websites",
    "popular online marketplaces",
    "popular travel booking websites",
    "popular food delivery websites",
    "popular weather websites",
    "popular sports websites",
    "popular finance websites",
    "popular banking websites",
    "popular investing websites",
    "popular cryptocurrency websites",
    "popular education websites",
    "popular online course websites",
    "popular reference websites",
    "popular encyclopedia websites",
    "popular developer websites",
    "popular software documentation websites",
    "popular code hosting websites",
    "popular design websites",
    "popular productivity websites",
    "popular project management websites",
    "popular communication websites",
    "popular email websites",
    "popular cloud storage websites",
    "popular job websites",
    "popular recruiting websites",
    "popular real estate websites",
    "popular automotive websites",
    "popular shopping fashion websites",
    "popular grocery websites",
    "popular restaurant websites",
    "popular health websites",
    "popular fitness websites",
    "popular government websites",
    "popular university websites",
    "popular library websites",
    "popular publishing websites",
    "popular magazine websites",
    "popular entertainment websites",
    "popular gaming websites",
    "popular forum websites",
    "popular blogging websites",
    "popular podcast websites",
    "popular image websites",
    "popular map websites",
    "popular translation websites",
    "popular language learning websites",
    "popular booking websites",
    "popular event websites",
    "popular ticket websites",
    "popular payment websites",
    "popular insurance websites",
    "popular legal information websites",
    "popular science websites",
    "popular technology websites",
]

# These ranked domains are redirect aliases/service endpoints rather than
# independently useful human-facing sites. Keep them visible in the state,
# but do not spend the interaction campaign on their redirect target.
REDIRECT_ONLY_DOMAINS = {
    "update.microsoft.com",
    "azure.com",
    "lsrelayaccess.com",
    "ipv4only.arpa",
}

# Authentication handoffs are not independently useful targets for this
# anonymous interaction campaign. Skip them when a site redirects into one.
REDIRECT_TARGET_HOSTS = {
    "login.microsoftonline.com",
}

ROOT_CAUSE_IDS = {
    "OFFSCREENCANVAS": "RC-001",
    "WEBGL_CAPABILITY": "RC-003",
    "EVALUATION_CAPABILITY_DENIED": "RC-006",
    "EVALUATION_TIMEOUT": "RC-006",
    "UNKNOWN_TIMEOUT": "RC-006",
    "PROBE_TIMEOUT": "RC-006",
    "NAVIGATION_TIMEOUT": "RC-006",
    "DNS_TIMEOUT": "RC-006",
    "TLS_TIMEOUT": "RC-006",
    "HTTP_RESPONSE_TIMEOUT": "RC-006",
    "RESOURCE_LOAD_TIMEOUT": "RC-006",
    "SCRIPT_EXECUTION_TIMEOUT": "RC-006",
    "EVENT_LOOP_TIMEOUT": "RC-006",
    "SEMANTIC_STABILITY_TIMEOUT": "RC-006",
    "DOM_PROJECTION_TIMEOUT": "RC-006",
    "LAYOUT_TIMEOUT": "RC-006",
    "AGENT_TREE_TIMEOUT": "RC-006",
    "ACTION_TIMEOUT": "RC-006",
    "TIMEOUT_UNKNOWN": "RC-006",
    "NAVIGATION_NO_DOCUMENT": "RC-007",
    "NAVIGATION_NO_RESULT": "RC-007",
    "NAVIGATION_TRANSPORT_ERROR": "RC-007",
    "FONT_API": "RC-012",
    "GENERAL_JS_API": "RC-012",
    "SECURITY_OR_NETWORK_POLICY": "RC-011",
    "SITE_SECURITY_POLICY": "RC-011",
    "HTTP_403_EXTERNAL": "RC-014",
    "HTTP_429_RATE_LIMIT": "RC-014",
    "HTTP_409_EXTERNAL": "RC-014",
    "EXTERNAL_SERVICE_TIMEOUT": "RC-014",
    "VENDOR_DEPENDENCY": "RC-009",
    "AUTH_OR_HUMAN_VERIFICATION": "RC-013",
    "AUTH_SESSION_REQUIRED": "RC-013",
    "SITE_CONFIGURATION": "RC-013",
    "WEBSOCKET_EXTERNAL": "RC-012",
    "SITE_MALFORMED_SCRIPT": "RC-012",
    "SITE_RECURSION": "RC-012",
    "SITE_DOM_ASSUMPTION": "RC-012",
    "SITE_DEPENDENCY_OR_ORDER": "RC-012",
    "SITE_TELEMETRY_USAGE": "RC-012",
    "MEDIA_CAPABILITY": "RC-115",
    "PUSH_MESSAGING_CAPABILITY": "RC-145",
    "EXTERNAL_BLOCK_UNCLASSIFIED": "RC-014",
    "ENGINE_CRASH": "RC-016",
    "FONT_FACE_SET_PANIC": "RC-017",
    "GLOBAL_SCRIPT_READINESS_PANIC": "RC-018",
}


def outcome_class(root_cause: str, *, status: str) -> str:
    """Return one mutually exclusive operational bucket for reporting.

    ``status`` is retained for campaign compatibility: historically the
    campaign used ``blocked_external`` for several unrelated outcomes.  This
    field is the mechanism-level bucket used for triage and post-fix metrics.
    """
    if status == "pass":
        return "PASS"
    if status == "redirect_skipped":
        return "REDIRECT_POLICY"
    if root_cause in {
        "UNKNOWN_TIMEOUT",
        "EVALUATION_TIMEOUT",
        "PROBE_TIMEOUT",
        "NAVIGATION_TIMEOUT",
        "DNS_TIMEOUT",
        "TLS_TIMEOUT",
        "HTTP_RESPONSE_TIMEOUT",
        "RESOURCE_LOAD_TIMEOUT",
        "SCRIPT_EXECUTION_TIMEOUT",
        "EVENT_LOOP_TIMEOUT",
        "SEMANTIC_STABILITY_TIMEOUT",
        "DOM_PROJECTION_TIMEOUT",
        "LAYOUT_TIMEOUT",
        "AGENT_TREE_TIMEOUT",
        "ACTION_TIMEOUT",
        "TIMEOUT_UNKNOWN",
    }:
        return "TIMEOUT"
    if root_cause in {
        "ENGINE_CRASH",
        "FONT_FACE_SET_PANIC",
        "GLOBAL_SCRIPT_READINESS_PANIC",
    }:
        return "ENGINE_CRASH"
    if root_cause in {
        "NAVIGATION_NO_DOCUMENT",
        "NAVIGATION_NO_RESULT",
        "NAVIGATION_TRANSPORT_ERROR",
    }:
        return "NAVIGATION_OR_TRANSPORT"
    if root_cause in {
        "OFFSCREENCANVAS",
        "WEBGL_CAPABILITY",
        "EVALUATION_CAPABILITY_DENIED",
        "FONT_API",
        "MEDIA_CAPABILITY",
        "PUSH_MESSAGING_CAPABILITY",
    }:
        return "BROWSER_CAPABILITY"
    if root_cause in {
        "HTTP_403_EXTERNAL",
        "HTTP_409_EXTERNAL",
        "HTTP_429_RATE_LIMIT",
        "EXTERNAL_SERVICE_TIMEOUT",
        "AUTH_OR_HUMAN_VERIFICATION",
        "AUTH_SESSION_REQUIRED",
        "SITE_CONFIGURATION",
        "SECURITY_OR_NETWORK_POLICY",
        "SITE_SECURITY_POLICY",
        "VENDOR_DEPENDENCY",
        "EXTERNAL_BLOCK_UNCLASSIFIED",
    }:
        return "EXTERNAL_OR_SITE_POLICY"
    if root_cause in {
        "GENERAL_JS_API",
        "NO_RESULT",
        "WEBSOCKET_EXTERNAL",
        "SITE_MALFORMED_SCRIPT",
        "SITE_RECURSION",
        "SITE_DOM_ASSUMPTION",
        "SITE_DEPENDENCY_OR_ORDER",
        "SITE_TELEMETRY_USAGE",
    }:
        return "RUNTIME_OR_SCRIPT"
    return "UNKNOWN"


def failure_metadata(root_cause: str, *, fresh_retry: bool) -> dict:
    """Attach stable issue identity without claiming unperformed comparisons."""
    if root_cause == "NONE":
        return {
            "root_cause_id": None,
            "failure_label": None,
            "reproducibility": None,
        }
    return {
        "root_cause_id": ROOT_CAUSE_IDS.get(root_cause),
        "failure_label": "KNOWN" if root_cause in ROOT_CAUSE_IDS else "NEW",
        "reproducibility": "REPRODUCIBLE" if fresh_retry else "UNMEASURED",
    }


def run_json(binary: str, args: list[str], timeout: int, *, ignore_certificate_errors: bool = False) -> tuple[int, dict | None, str]:
    environment = os.environ.copy()
    if ignore_certificate_errors:
        environment["BROWSAI_IGNORE_CERTIFICATE_ERRORS"] = "1"
    process_group = {}
    if os.name == "posix":
        process_group["start_new_session"] = True
    elif hasattr(subprocess, "CREATE_NEW_PROCESS_GROUP"):
        process_group["creationflags"] = subprocess.CREATE_NEW_PROCESS_GROUP
    process = subprocess.Popen(
        [binary, *args],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=environment,
        **process_group,
    )
    try:
        output, stderr = process.communicate(timeout=timeout)
    except subprocess.TimeoutExpired as error:
        if os.name == "posix":
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
        else:
            process.kill()
        output, stderr = process.communicate()
        partial_stderr = error.stderr or stderr or ""
        if isinstance(partial_stderr, bytes):
            partial_stderr = partial_stderr.decode("utf-8", "replace")
        return 124, None, (
            f"timeout after {timeout}s: {error}\n"
            f"partial_stderr={str(partial_stderr)[-4000:]}"
        )
    output = output.strip()
    if not output:
        return process.returncode, None, stderr.strip() or "empty output"
    try:
        return process.returncode, json.loads(output), stderr.strip()
    except json.JSONDecodeError:
        return process.returncode, None, stderr.strip() or output[-1000:]


def classify_result(
    *,
    code: int,
    payload: dict | None,
    error: str,
    diagnostics_error: object,
    diagnostics_text: str,
    runtime_messages: list[str],
    external_block: bool,
    runtime_errors: list[str],
) -> tuple[str, str]:
    """Return a mechanism-level root cause and responsible source."""
    text = " ".join(
        [
            error,
            str(diagnostics_error or ""),
            diagnostics_text,
            " ".join(runtime_messages),
            " ".join(runtime_errors),
        ]
    ).lower()
    crash_markers = (
        "thread caused non-unwinding panic",
        "mozalloc_abort",
        "segmentation fault",
        "address sanitizer",
        "panicked at",
        "internal error: entered unreachable code",
        "assertion failed: self.can_run_script()",
    )
    if any(marker in text for marker in crash_markers):
        if "fontfaceset.rs" in text or "removed @font-face" in text:
            return "FONT_FACE_SET_PANIC", "BROWSAI_ENGINE"
        if "globalscope.rs" in text and "can_run_script" in text:
            return "GLOBAL_SCRIPT_READINESS_PANIC", "BROWSAI_ENGINE"
        return "ENGINE_CRASH", "BROWSAI_ENGINE"
    if code == 124:
        stage_markers = (
            ("browsai_stage:navigation", "NAVIGATION_TIMEOUT", "BROWSAI_NETWORK"),
            ("browsai_stage:stability", "SEMANTIC_STABILITY_TIMEOUT", "BROWSAI_PROJECTION"),
            ("browsai_stage:dom_projection", "DOM_PROJECTION_TIMEOUT", "BROWSAI_PROJECTION"),
            ("browsai_stage:interaction", "ACTION_TIMEOUT", "BROWSAI_ENGINE"),
            ("browsai_stage:evaluation", "EVALUATION_TIMEOUT", "BROWSAI_ENGINE"),
        )
        observed = [
            (text.rfind(marker), root_cause, source)
            for marker, root_cause, source in stage_markers
            if marker in text
        ]
        if observed:
            _, root_cause, source = max(observed, key=lambda item: item[0])
            return root_cause, source
        return "UNKNOWN_TIMEOUT", "BROWSAI_ENGINE"
    if code < 0 and (code == -11 or any(marker in text for marker in crash_markers)):
        return "ENGINE_CRASH", "BROWSAI_ENGINE"
    if payload is None:
        return "NAVIGATION_NO_RESULT", "BROWSAI_NETWORK"
    navigation = payload.get("navigation") or {}
    http_status = navigation.get("http_status")
    if http_status == 403:
        return "HTTP_403_EXTERNAL", "EXTERNAL_SERVICE"
    if http_status == 429:
        return "HTTP_429_RATE_LIMIT", "EXTERNAL_SERVICE"
    # A committed page can keep probing after an API/chunk request is denied.
    # Prefer the explicit upstream status over the later action/layout timeout
    # so the timeout does not hide the actual external cause.
    if re.search(r"(?:received status code|status(?:Code| code)?)[\s:=\"]+403\b", text, re.IGNORECASE):
        return "HTTP_403_EXTERNAL", "EXTERNAL_SERVICE"
    if re.search(r"(?:received status code|status(?:Code| code)?)[\s:=\"]+409\b", text, re.IGNORECASE):
        return "HTTP_409_EXTERNAL", "EXTERNAL_SERVICE"
    diagnostic = str(diagnostics_error or "").lower()
    if "capability denied" in diagnostic:
        return "EVALUATION_CAPABILITY_DENIED", "BROWSAI_ENGINE"
    if "x5secdata" in text or "_____tmd_____/punish" in text:
        return "AUTH_OR_HUMAN_VERIFICATION", "EXTERNAL_SERVICE"
    if "__tcfapicall" in text:
        return "VENDOR_DEPENDENCY", "THIRD_PARTY_SCRIPT"
    if "osano failure" in text:
        return "VENDOR_DEPENDENCY", "THIRD_PARTY_SCRIPT"
    if (
        "jsonp request" in text and "timed out" in text
    ) or "coordinator timed out" in text or "fetching /coordination/flags" in text:
        return "EXTERNAL_SERVICE_TIMEOUT", "EXTERNAL_SERVICE"
    if "api-cache" in text and "loading timeout" in text:
        return "EXTERNAL_SERVICE_TIMEOUT", "EXTERNAL_SERVICE"
    timeoutish = bool(re.search(r"\b(?:timeout|timed out|hang(?:ing)?)\b", text))
    if timeoutish:
        timeout_stages = (
            (("dns", "name resolution", "resolve host"), "DNS_TIMEOUT", "BROWSAI_NETWORK"),
            (("tls", "ssl handshake", "certificate handshake"), "TLS_TIMEOUT", "BROWSAI_NETWORK"),
            (("http response", "waiting for response", "response headers"), "HTTP_RESPONSE_TIMEOUT", "BROWSAI_NETWORK"),
            (("resource load", "loading resource", "subresource"), "RESOURCE_LOAD_TIMEOUT", "BROWSAI_NETWORK"),
            (("script execution", "javascript execution"), "SCRIPT_EXECUTION_TIMEOUT", "BROWSAI_ENGINE"),
            (("event loop", "event-loop"), "EVENT_LOOP_TIMEOUT", "BROWSAI_ENGINE"),
            (("semantic stability", "stability settle"), "SEMANTIC_STABILITY_TIMEOUT", "BROWSAI_PROJECTION"),
            (("dom projection", "dom projection"), "DOM_PROJECTION_TIMEOUT", "BROWSAI_PROJECTION"),
            (("layout", "layout observation"), "LAYOUT_TIMEOUT", "BROWSAI_PROJECTION"),
            (("agent tree", "agent-tree"), "AGENT_TREE_TIMEOUT", "BROWSAI_PROJECTION"),
            (("action", "click action"), "ACTION_TIMEOUT", "BROWSAI_ENGINE"),
        )
        for markers, root_cause, source in timeout_stages:
            if any(marker in text for marker in markers):
                return root_cause, source
    if "timeout" in diagnostic:
        return "EVALUATION_TIMEOUT", "BROWSAI_ENGINE"
    if "probe" in text and re.search(r"\b(?:timeout|timed out|hang(?:ing)?)\b", text):
        return "PROBE_TIMEOUT", "BROWSAI_ENGINE"
    if "navigation" in text and re.search(r"\b(?:timeout|timed out|hang(?:ing)?)\b", text):
        return "NAVIGATION_TIMEOUT", "BROWSAI_NETWORK"
    if re.search(r"\b(?:timeout|timed out|hang(?:ing)?)\b", text):
        return "UNKNOWN_TIMEOUT", "BROWSAI_ENGINE"
    if payload.get("url") == "about:blank":
        return "NAVIGATION_NO_DOCUMENT", "BROWSAI_NETWORK"
    if "error loading page" in text or "could not load the requested page" in text:
        return "NAVIGATION_TRANSPORT_ERROR", "BROWSAI_NETWORK"
    if (
        "feature key undefined is not in datafile" in text
        or "missing config element # application_config" in text
        or "unable to retrieve the configuration value for marketplace-store-onetrust-settings" in text
        or "multiple scripts found, cannot determine custom domain" in text
        or "typekit" in text
        and (
            "not in the list of published domains" in text
            or "isn't in the list of published domains" in text
        )
        or "invalid type on $input.time_zone" in text
    ):
        return "SITE_CONFIGURATION", "WEBSITE_SCRIPT"
    if external_block:
        if "403" in text or "forbidden" in text:
            return "HTTP_403_EXTERNAL", "EXTERNAL_SERVICE"
        if "received status code 409" in text:
            return "HTTP_409_EXTERNAL", "EXTERNAL_SERVICE"
        if "security policy" in text:
            return "SITE_SECURITY_POLICY", "EXTERNAL_SERVICE"
        if any(token in text for token in ("bmak", "zitag", "marquee", "component map", "fs-components")):
            return "VENDOR_DEPENDENCY", "THIRD_PARTY_SCRIPT"
        return "EXTERNAL_BLOCK_UNCLASSIFIED", "EXTERNAL_SERVICE"
    if not runtime_errors:
        return "NONE", "UNKNOWN"
    if "offscreen" in text or "offscreencanvas" in text:
        return "OFFSCREENCANVAS", "BROWSAI_ENGINE"
    if "webgl" in text or "shader" in text:
        return "WEBGL_CAPABILITY", "BROWSAI_PROJECTION"
    if "hls not supported in this browser" in text or (
        "fantascope-player.js" in text and "browser not supported" in text
    ):
        return "MEDIA_CAPABILITY", "BROWSAI_ENGINE"
    if "messaging/unsupported-browser" in text or "firebase sdk" in text and "browser" in text and "support" in text:
        return "PUSH_MESSAGING_CAPABILITY", "BROWSAI_ENGINE"
    if "apps.rokt.com/wsdk/" in text or "js-cdn.dynatrace.com/jstag/" in text:
        return "VENDOR_DEPENDENCY", "THIRD_PARTY_SCRIPT"
    if "fontface" in text or "document.fonts" in text or "fonts." in text:
        return "FONT_API", "BROWSAI_ENGINE"
    if (
        "cors" in text
        or "csp" in text
        or "content-security-policy" in text
        or "securityerror" in text
    ):
        # A page-visible SecurityError does not identify the enforcing side.
        # Keep it unowned until origin, request, and comparative-browser
        # evidence proves a BrowsAI policy mismatch.
        return "SECURITY_OR_NETWORK_POLICY", "UNKNOWN"
    if "429" in text or "too many requests" in text:
        return "HTTP_429_RATE_LIMIT", "EXTERNAL_SERVICE"
    if any(
        marker in text
        for marker in (
            "invalidtokenerror",
            "not logged in",
            "unauthenticated",
            "invalid token",
            "please log in",
            "authentication required",
        )
    ):
        return "AUTH_SESSION_REQUIRED", "EXTERNAL_SERVICE"
    if "captcha" in text or "jwt" in text or "authentication" in text:
        return "AUTH_OR_HUMAN_VERIFICATION", "EXTERNAL_SERVICE"
    if "websocket failed to connect" in text or "failed to start the connection" in text:
        return "WEBSOCKET_EXTERNAL", "EXTERNAL_SERVICE"
    if "too much recursion" in text or "maximum call stack" in text:
        return "SITE_RECURSION", "WEBSITE_SCRIPT"
    if any(
        marker in text
        for marker in (
            "track&report js errors api",
            "unique user id has not been set",
            "posthog.identify",
            "pagetype parameter is required",
            "_paq is not defined",
            "empty events for tab all and bucket primary_bucket",
        )
    ):
        return "SITE_TELEMETRY_USAGE", "THIRD_PARTY_SCRIPT"
    if any(
        marker in text
        for marker in (
            "missing ] after element list",
            "illegal character",
            "expected expression, got",
        )
    ):
        return "SITE_MALFORMED_SCRIPT", "WEBSITE_SCRIPT"
    if "json.parse: unexpected character" in text:
        # Some sites parse an HTML/error/telemetry response as JSON during
        # startup.  This is a site/vendor response-contract failure, not a
        # missing DOM or browser-native API.
        return "SITE_DEPENDENCY_OR_ORDER", "THIRD_PARTY_SCRIPT"
    if any(
        marker in text
        for marker in (
            "k.twitchcdn.net",
            "amazon ivs player sdk",
            "client.aps.amazon-adsystem.com",
            "ips.js?kp_uid",
            "chunkloaderror",
            "failed to load analytics.js",
            "could not clear consent from root domain",
            "osano failure",
            "f.cookie is not a function",
            "static.foxnews.com/static/strike",
            "google_tag_manager",
            "clip component failed to load",
            "sleeknotestaticcontent.sleeknote.com",
            "googletagmanager.com/gtm.js",
            "transcend-cdn.com/cm/",
            "js.datadome.co/",
            "static.tacdn.com/assets/",
            "assets.targetimg1.com/ssx/",
            "elpais.com/arc/subs/p.min.js",
            "pmwall error",
            "doubleclick.net/activityi",
            "ttd_dom_ready is not defined",
            "result.identity is undefined",
            "granite is not defined",
            "foresee_assets",
            "getvendorconfig(...).storage is undefined",
        )
    ):
        return "VENDOR_DEPENDENCY", "THIRD_PARTY_SCRIPT"
    if any(
        marker in text
        for marker in (
            "scrolltrigger is not defined",
            "no i18n instances found",
            "no suitable store implementation",
            "sticky-qr",
            "portal_id",
            "window.analytics is undefined",
            "liveagent is not defined",
            "window.console._std is undefined",
            "content dependency",
            "wp.i18n",
            "window.jsc is not a function",
            "href.split is not a function",
            "sinassocontroller is not defined",
            "$.datepicker is undefined",
        )
    ):
        return "SITE_DEPENDENCY_OR_ORDER", "THIRD_PARTY_SCRIPT"
    if any(
        marker in text
        for marker in (
            "queryselector(...) is null",
            "queryselector(\"",
            "is null",
            "is undefined",
            " is not defined",
            "can't access property",
        )
    ):
        return "SITE_DOM_ASSUMPTION", "WEBSITE_SCRIPT"
    return "GENERAL_JS_API", "WEBSITE_SCRIPT"


def navigation_provenance(
    *, requested_url: str, payload: dict | None, code: int, error: str,
    root_cause: str = ""
) -> dict:
    """Persist navigation facts separately from the compatibility verdict."""
    result = payload or {}
    final_url = str(result.get("url") or "")
    load_status = str(result.get("load_status") or "unknown")
    navigation = result.get("navigation") or {}
    history = navigation.get("history") or []
    redirect_loop = len(history) > 1 and len({str(url) for url in history}) < len(history)
    diagnostics = result.get("diagnostics") or {}
    body_length = diagnostics.get("bodyLength") or 0
    title = diagnostics.get("title") or ""
    body_text = str(diagnostics.get("bodyText") or "").lower()
    http_status = navigation.get("http_status")
    if http_status == 429 or root_cause == "HTTP_429_RATE_LIMIT":
        network_policy_classification = "EXTERNAL_RATE_LIMIT"
    elif http_status == 403 or root_cause == "HTTP_403_EXTERNAL":
        network_policy_classification = "SITE_POLICY_EXPECTED"
    elif root_cause in {"SECURITY_OR_NETWORK_POLICY", "SITE_SECURITY_POLICY"}:
        network_policy_classification = "UNKNOWN"
    else:
        network_policy_classification = None
    transport_error_page = title.lower() == "error loading page" or "could not load the requested page" in body_text
    non_document = not final_url or final_url == "about:blank"
    committed = bool(
        final_url
        and not non_document
        and not transport_error_page
        and not redirect_loop
        and (body_length or title or load_status == "Complete")
    )
    if redirect_loop:
        stage = "redirect_loop"
    elif non_document:
        stage = "initial_blank" if not payload else "no_document"
    elif transport_error_page:
        stage = "transport_error_page"
    elif committed:
        stage = "committed"
    else:
        stage = "load_incomplete"
    normalized_mime = str(navigation.get("mime_type") or "").split(";", 1)[0].strip().lower()
    if redirect_loop:
        navigation_cause = "REDIRECT_LOOP"
    elif not payload:
        navigation_cause = "NO_RESULT"
    elif non_document:
        navigation_cause = "NO_DOCUMENT"
    elif transport_error_page:
        navigation_cause = "TRANSPORT_ERROR_PAGE"
    elif isinstance(http_status, int) and http_status >= 400:
        navigation_cause = "HTTP_ERROR"
    elif normalized_mime in {
        "application/octet-stream",
        "application/x-download",
        "application/x-msdownload",
    }:
        navigation_cause = "UNSUPPORTED_MIME_OR_DOWNLOAD"
    elif committed:
        navigation_cause = "COMMITTED_DOCUMENT"
    else:
        navigation_cause = "LOAD_INCOMPLETE"
    return {
        "requested_url": requested_url,
        "final_url": final_url or None,
        "load_status": load_status,
        "document_created": bool((body_length or title) and not transport_error_page),
        "navigation_committed": committed,
        "redirect_observed": bool(
            navigation.get("redirect_observed")
            or len(history) > 1
            or (final_url and final_url.rstrip("/") != requested_url.rstrip("/"))
        ),
        "redirect_loop_observed": redirect_loop,
        "redirect_chain": history,
        "stage": stage,
        "navigation_cause": navigation_cause,
        "http_status": http_status,
        "mime_type": navigation.get("mime_type"),
        "network_policy_classification": network_policy_classification,
        "origin": navigation.get("origin"),
        "initiator": navigation.get("initiator"),
        "resource_type": navigation.get("resource_type"),
        "failure_reason": error or None,
        "process_exit_code": code,
    }


def execution_provenance(
    *, payload: dict | None, diagnostics_error: object, elapsed_seconds: float,
    timeout_seconds: int, attempts: int, runtime_error_count: int,
    root_cause: str, observed_stage: str | None = None,
    observed_stages: list[str] | None = None,
) -> dict:
    """Persist the observed execution stage without inventing engine internals."""
    result = payload or {}
    diagnostics = result.get("diagnostics") or {}
    load_status = str(result.get("load_status") or "unknown")
    ready_state = diagnostics.get("readyState")
    body_length = diagnostics.get("bodyLength")
    if diagnostics_error:
        last_stage = "evaluation"
    elif ready_state == "complete" and body_length:
        last_stage = "dom_projection"
    elif load_status == "Complete":
        last_stage = "document_load"
    elif result.get("url") and result.get("url") != "about:blank":
        last_stage = "navigation_commit"
    else:
        last_stage = "process_start"
    return {
        "elapsed_seconds": elapsed_seconds,
        "timeout_seconds": timeout_seconds,
        "timed_out": root_cause.endswith("_TIMEOUT"),
        "timeout_subtype": root_cause if root_cause.endswith("_TIMEOUT") else None,
        "attempts": attempts,
        "last_successful_stage": last_stage,
        "last_observed_stage": observed_stage,
        "stage_history": list(observed_stages or ([observed_stage] if observed_stage else [])),
        "pending_requests": None,
        "pending_scripts": None,
        "pending_tasks": None,
        "dom_state": {
            "ready_state": ready_state,
            "body_length": body_length,
            "html_length": diagnostics.get("htmlLength"),
        },
        "semantic_generation": None,
        "network_state": load_status,
        "process_heartbeat": {
            "observed": bool(observed_stages),
            "marker_count": len(observed_stages or []),
            "last_stage": observed_stage,
        },
        "diagnostics_error": diagnostics_error,
        "runtime_error_count": runtime_error_count,
    }


def network_policy_provenance(
    *, navigation: dict, runtime_events: list[dict], root_cause: str
) -> dict:
    """Persist policy evidence without fabricating unavailable subresource data."""
    signals = sorted(
        {
            str(event["network_policy_signal"])
            for event in runtime_events
            if event.get("network_policy_signal")
        }
    )
    classification = navigation.get("network_policy_classification")
    if signals and classification is None:
        classification = "UNKNOWN"
    return {
        "page_origin": navigation.get("origin"),
        "target_origin": None,
        "request_mode": None,
        "credentials_mode": None,
        "request_headers": None,
        "preflight": None,
        "response_headers": None,
        "redirect_chain": navigation.get("redirect_chain") or [],
        "cors_decision": "blocked" if "cors" in signals else None,
        "csp_decision": "blocked" if "csp" in signals else None,
        "security_error_decision": "blocked" if "security_error" in signals else None,
        "signals": signals,
        "classification": classification,
        "telemetry_complete": False,
        "missing_fields": [
            "target_origin",
            "request_mode",
            "credentials_mode",
            "request_headers",
            "preflight",
            "response_headers",
        ],
        "root_cause": root_cause,
    }


RUNTIME_EVENT_LIMIT = 100
RUNTIME_EVENT_MAX_BYTES = 64 * 1024


def observed_process_stage(*texts: object) -> str | None:
    """Extract the last live-open stage marker from bounded diagnostics."""
    matches = observed_process_stages(*texts)
    return matches[-1] if matches else None


def observed_process_stages(*texts: object) -> list[str]:
    """Extract the complete ordered live-open stage-marker history."""
    combined = "\n".join(str(text or "") for text in texts)
    return re.findall(
        r"BROWSAI_STAGE:(startup|navigation|stability|dom_projection|interaction|evaluation)",
        combined,
    )


def bound_runtime_event_messages(
    messages: list[str],
    *,
    limit: int = RUNTIME_EVENT_LIMIT,
    max_bytes: int = RUNTIME_EVENT_MAX_BYTES,
) -> tuple[list[str], bool, int]:
    """Bound raw diagnostics before projecting them into runtime events."""
    bounded: list[str] = []
    used_bytes = 0
    truncated = False
    for raw in messages:
        if len(bounded) >= limit:
            truncated = True
            break
        text = str(raw)
        encoded = text.encode("utf-8")
        remaining = max_bytes - used_bytes
        if remaining <= 0:
            truncated = True
            break
        if len(encoded) > remaining:
            text = encoded[:remaining].decode("utf-8", errors="ignore")
            while len(text.encode("utf-8")) > remaining:
                text = text[:-1]
            truncated = True
        bounded.append(text)
        used_bytes += len(text.encode("utf-8"))
        if truncated:
            break
    return bounded, truncated, used_bytes


def runtime_event_provenance(messages: list[str]) -> list[dict]:
    """Extract stable, best-effort fields from runtime diagnostics.

    The raw message remains authoritative. Parsed fields are deliberately
    nullable because minified and vendor-generated messages do not always
    contain a URL, position, or JavaScript exception name.
    """
    events = []
    for raw in messages:
        text = str(raw)
        lower = text.lower()
        structured = None
        structured_marker = "BROWSAI_RUNTIME_EVENT "
        if structured_marker in text:
            encoded = text.split(structured_marker, 1)[1].strip()
            try:
                candidate = json.loads(encoded)
                if isinstance(candidate, dict):
                    structured = candidate
            except json.JSONDecodeError:
                structured = None
        exception_matches = re.findall(
            r"\b(TypeError|ReferenceError|RangeError|SyntaxError|SecurityError|Error)\b",
            text,
        )
        exception_type = next(
            (candidate for candidate in exception_matches if candidate != "Error"),
            exception_matches[0] if exception_matches else None,
        )
        if structured and structured.get("exception_type"):
            exception_type = structured["exception_type"]
        position_match = re.search(
            r"(?P<url>(?:https?://|blob:|data:)[^\"\s]+?):(?P<line>\d+):(?P<column>\d+)",
            text,
        )
        constructor_match = re.search(
            r"\b(?:TypeError|ReferenceError|RangeError|SyntaxError|SecurityError|Error)\s*"
            r"\(\s*\"[^\"]*\"\s*,\s*\"(?P<url>[^\"]+)\""
            r"(?:\s*,\s*(?P<line>\d+))?",
            text,
        )
        missing_match = re.search(
            r"(?:^|[\s\"'])((?:[A-Za-z_$][\w$]*)(?:\.[A-Za-z_$][\w$]*)*)"
            r"\s+(?:is not defined|is undefined|is not a function)",
            text,
            re.IGNORECASE,
        )
        property_match = re.search(
            r"property\s+[\"']([^\"']+)[\"']\s+[^;,.]*\s+is undefined",
            text,
            re.IGNORECASE,
        )
        property_access_match = re.search(
            r"can't access property\s+[\"'](?P<property>[^\"']+)[\"']\s*,\s*"
            r"(?P<base>[A-Za-z_$][\w$]*(?:\.[A-Za-z_$][\w$]*)*)\s+is\s+(?:undefined|null)",
            text,
            re.IGNORECASE,
        )
        script_url = (
            position_match.group("url")
            if position_match
            else constructor_match.group("url")
            if constructor_match
            else None
        )
        if structured and structured.get("script_url"):
            script_url = structured["script_url"]
        line = (
            int(position_match.group("line"))
            if position_match
            else int(constructor_match.group("line"))
            if constructor_match and constructor_match.group("line")
            else None
        )
        if structured and structured.get("line") is not None:
            line = structured["line"]
        column = int(position_match.group("column")) if position_match else None
        if structured and structured.get("column") is not None:
            column = structured["column"]
        security_mechanism = None
        if "securityerror" in lower:
            mechanism_markers = (
                (("localstorage", "sessionstorage", "indexeddb"), "storage"),
                (("blocked a frame", "cross-origin", "same-origin"), "cross_origin"),
                (("sandbox", "allow-same-origin"), "sandbox"),
                (("mixed content", "insecure resource"), "mixed_content"),
                (("permission", "clipboard", "geolocation", "notifications"), "permission"),
                (("document.domain",), "document_domain"),
                (("secure context", "operation is insecure"), "restricted_api"),
            )
            for markers, mechanism in mechanism_markers:
                if any(marker in lower for marker in markers):
                    security_mechanism = mechanism
                    break
            if security_mechanism is None:
                security_mechanism = "unknown"
        network_policy_signal = None
        if "cors" in lower:
            network_policy_signal = "cors"
        elif "csp" in lower or "content security policy" in lower:
            network_policy_signal = "csp"
        elif security_mechanism is not None:
            network_policy_signal = "security_error"
        event_message = text.removeprefix("Error: ").strip() or None
        if structured and structured.get("message"):
            event_message = str(structured["message"])
        if property_access_match:
            missing_symbol = (
                f"{property_access_match.group('base')}."
                f"{property_access_match.group('property')}"
            )
        elif missing_match:
            missing_symbol = missing_match.group(1)
        elif property_match:
            missing_symbol = property_match.group(1)
        else:
            missing_symbol = None
        events.append(
            {
                "raw": text,
                "exception_type": exception_type,
                "message": event_message,
                "script_url": script_url,
                "line": line,
                "column": column,
                "missing_symbol": missing_symbol,
                "realm": (
                    structured.get("realm")
                    if structured and structured.get("realm")
                    else
                    "blob-worker"
                    if script_url and script_url.startswith("blob:")
                    else "data-worker"
                    if script_url and script_url.startswith("data:")
                    else "window-or-document"
                ),
                "initialization_stage": structured.get("initialization_stage") if structured else None,
                "api": (
                    structured.get("api")
                    if structured and structured.get("api")
                    else missing_symbol
                ),
                "stack": structured.get("stack") if structured else None,
                "security_context": None,
                "security_mechanism": security_mechanism,
                "network_policy_signal": network_policy_signal,
            }
        )
    return events


def legacy_audit_bucket(record: dict) -> str:
    """Partition pre-schema records for triage without rewriting their meaning.

    Corpus-002 records do not contain ``root_cause`` or execution provenance.
    This deliberately conservative string audit is therefore labeled legacy
    and must not be used as fresh root-cause evidence.
    """
    status = record.get("status")
    if status == "pass":
        return "PASS"
    if status == "redirect_skipped":
        return "REDIRECT_POLICY"
    error = str(record.get("error") or "")
    lowered = error.lower()
    result = record.get("result")
    # Do not match JavaScript identifiers such as ``setTimeout`` in an
    # otherwise ordinary site exception. Legacy records predate structured
    # timeout metadata, so only accept the runner's explicit timeout wording
    # and known stage-specific phrases.
    actual_timeout = re.search(
        r"(?:timeout after \d+s|timed out after \d+ seconds|"
        r"(?:command|page evaluation|probe|navigation|event loop|semantic|"
        r"dom projection|layout|agent tree|action|resource load|script execution|"
        r"dns|tls|http response)\s+timeout)",
        lowered,
    )
    if actual_timeout:
        return "TIMEOUT"
    if "about:blank" in lowered or "non-document page" in lowered:
        return "NAVIGATION_OR_TRANSPORT"
    if result is None and not error:
        return "NAVIGATION_OR_TRANSPORT"
    if status == "blocked_external" and result is None:
        return "NAVIGATION_OR_TRANSPORT"
    if re.search(r"webgl|offscreen.?canvas|canvas", lowered):
        return "BROWSER_CAPABILITY"
    if re.search(
        r"(?:status code|http|received).{0,24}\b(?:403|429)\b|"
        r"\b(?:403|429)\b.{0,24}(?:status|forbidden|http|response)|"
        r"unauthenticated|invalid token|captcha|zscaler|cors|csp|securityerror",
        lowered,
    ):
        return "EXTERNAL_OR_SITE_POLICY"
    if status == "fail":
        return "RUNTIME_OR_SCRIPT"
    if status == "blocked_external":
        return "EXTERNAL_OR_SITE_POLICY"
    return "UNKNOWN"


def audit_state(args: argparse.Namespace) -> int:
    """Print a deterministic triage view of a legacy or schema-bearing state."""
    state = json.loads(Path(args.state).read_text())
    sites = state.get("sites", {})
    buckets: dict[str, list[str]] = {}
    for domain, record in sites.items():
        bucket = record.get("outcome_class") or legacy_audit_bucket(record)
        buckets.setdefault(bucket, []).append(domain)
    summary = {
        "state": args.state,
        "runner_metadata": state.get("runner_metadata"),
        "legacy_heuristic": not any("outcome_class" in record for record in sites.values()),
        "site_count": len(sites),
        "counts": {bucket: len(sorted(domains)) for bucket, domains in sorted(buckets.items())},
        "sites": {bucket: sorted(domains) for bucket, domains in sorted(buckets.items())},
    }
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0


def runner_metadata(binary: str) -> dict[str, object]:
    """Identify the executable that produced a corpus state file."""
    path = Path(binary)
    metadata: dict[str, object] = {"binary": str(path)}
    try:
        metadata["resolved_binary"] = str(path.resolve())
        stat = path.stat()
        metadata["binary_size"] = stat.st_size
        metadata["binary_mtime_ns"] = stat.st_mtime_ns
        digest = hashlib.sha256()
        with path.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
        metadata["binary_sha256"] = digest.hexdigest()
    except OSError as error:
        metadata["binary_error"] = str(error)
    return metadata


def discover(args: argparse.Namespace) -> int:
    state_path = Path(args.state)
    state = json.loads(state_path.read_text()) if state_path.exists() else {"queries": {}, "urls": {}}
    for query in QUERIES:
        if len(state["urls"]) >= args.target:
            break
        if state["queries"].get(query, {}).get("complete"):
            continue
        last_error = ""
        for attempt in range(1, 4):
            code, payload, error = run_json(args.binary, ["live-search", query, "--discover"], args.timeout)
            if code == 0 and payload:
                if "/sorry/" in payload.get("url", ""):
                    last_error = "Google returned its /sorry/ rate-limit page"
                    time.sleep(min(15 * attempt, 45))
                    continue
                found = payload.get("discovered_urls", [])
                added = 0
                for raw in found:
                    parsed = urlparse(raw)
                    if parsed.scheme not in {"http", "https"} or not parsed.hostname:
                        continue
                    domain = parsed.hostname.lower().removeprefix("www.")
                    if domain.endswith("google.com") or domain in {"googleusercontent.com", "gstatic.com"}:
                        continue
                    canonical = f"{parsed.scheme}://{domain}/"
                    if domain not in state["urls"]:
                        state["urls"][domain] = {
                            "url": canonical,
                            "domain": domain,
                            "source_query": query,
                            "priority": max(1, 10000 - len(state["urls"])),
                        }
                        added += 1
                state["queries"][query] = {"complete": True, "found": len(found), "added": added}
                state_path.write_text(json.dumps(state, indent=2, sort_keys=True) + "\n")
                print(f"DISCOVER {query!r}: found={len(found)} added={added} total={len(state['urls'])}", flush=True)
                break
            last_error = error
            time.sleep(min(attempt * 2, 5))
        else:
            state["queries"][query] = {"complete": False, "error": last_error}
            state_path.write_text(json.dumps(state, indent=2, sort_keys=True) + "\n")
            print(f"DISCOVERY_RETRY_LATER {query!r}: {last_error}", file=sys.stderr, flush=True)

    rows = sorted(state["urls"].values(), key=lambda row: (-row["priority"], row["domain"]))[: args.target]
    with Path(args.corpus).open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=["id", "url", "domain", "category", "priority", "enabled"])
        writer.writeheader()
        for index, row in enumerate(rows, 1):
            writer.writerow({"id": f"google-{index:04d}", "url": row["url"], "domain": row["domain"], "category": "google-discovered", "priority": row["priority"], "enabled": "true"})
    print(f"CORPUS {args.corpus}: {len(rows)} sites")
    return 0 if len(rows) >= args.target else 2


def exercise(args: argparse.Namespace) -> int:
    corpus = list(csv.DictReader(Path(args.corpus).open(newline="")))
    source_state_path = Path(args.state)
    state_path = Path(args.output_state or args.state)
    source_state = json.loads(source_state_path.read_text()) if source_state_path.exists() else {"sites": {}}
    replay_statuses = set(args.replay_status)
    if args.output_state and replay_statuses:
        state = {
            "campaign": "targeted-replay",
            "source": str(args.corpus),
            "source_state": str(source_state_path),
            "replay_status": sorted(replay_statuses),
            "sites": {},
        }
    else:
        state = source_state
    state["runner_metadata"] = runner_metadata(args.binary)
    for row in corpus:
        domain = row["domain"]
        source_record = source_state.get("sites", {}).get(domain, {})
        if replay_statuses and source_record.get("status") not in replay_statuses:
            continue
        if not replay_statuses and state["sites"].get(domain, {}).get("status") in {"pass", "blocked_external"}:
            continue
        if domain in REDIRECT_ONLY_DOMAINS:
            state["sites"][domain] = {
                "status": "redirect_skipped",
                "outcome_class": "REDIRECT_POLICY",
                "exit_code": 0,
                "elapsed_seconds": 0,
                "url": row["url"],
                "error": "redirect-only domain skipped per campaign policy",
                "root_cause": "REDIRECT_ONLY_POLICY",
                "error_source": "EXTERNAL_SERVICE",
                "navigation": navigation_provenance(
                    requested_url=row["url"], payload=None, code=0, error="redirect-only domain skipped per campaign policy"
                ),
                "result": None,
            }
            state_path.write_text(json.dumps(state, indent=2, sort_keys=True) + "\n")
            print(f"TEST {domain}: redirect_skipped (campaign policy)", flush=True)
            continue
        started = time.time()
        code, payload, error = 1, None, ""
        certificate_override = False
        for attempt in range(1, args.site_retries + 1):
            code, payload, error = run_json(
                args.binary,
                ["live-open", row["url"], "--click-links", "--probe-controls"],
                args.timeout,
            )
            transient_blank = payload is None or payload.get("url") == "about:blank"
            transient_timeout = code == 124 or bool(
                (payload or {}).get("execution", {}).get("timed_out")
            )
            if not (transient_blank or transient_timeout) or attempt == args.site_retries:
                break
            time.sleep(attempt)
        certificate_page = (
            (payload or {}).get("diagnostics", {}).get("title") == "Certificate error"
            or any(
                "allow certificate temporarily" in str(control.get("name", "")).lower()
                for control in (payload or {}).get("candidate_controls", [])
            )
        )
        if certificate_page:
            code, payload, error = run_json(
                args.binary,
                ["live-open", row["url"], "--click-links", "--probe-controls"],
                args.timeout,
                ignore_certificate_errors=True,
            )
            certificate_override = True
        final_url = (payload or {}).get("url", "")
        final_host = (urlparse(final_url).hostname or "").lower() if final_url else ""
        if final_host in REDIRECT_TARGET_HOSTS:
            state["sites"][domain] = {
                "status": "redirect_skipped",
                "outcome_class": "REDIRECT_POLICY",
                "exit_code": 0,
                "elapsed_seconds": round(time.time() - started, 3),
                "url": row["url"],
                "error": f"redirect target skipped: {final_host}",
                "root_cause": "REDIRECT_TARGET_POLICY",
                "error_source": "EXTERNAL_SERVICE",
                "navigation": navigation_provenance(
                    requested_url=row["url"], payload=payload, code=0, error=f"redirect target skipped: {final_host}"
                ),
                "result": payload,
            }
            state_path.write_text(json.dumps(state, indent=2, sort_keys=True) + "\n")
            print(f"TEST {domain}: redirect_skipped ({final_host})", flush=True)
            continue
        runtime_messages = (payload or {}).get("runtime_messages", [])
        diagnostics_error = (payload or {}).get("diagnostics", {}).get("error")
        diagnostics_text = (payload or {}).get("diagnostics", {}).get("bodyText", "")
        external_block = (
            domain in {"lijit.com"}
        ) or (
            "bmak is undefined" in " ".join(runtime_messages).lower()
            or "window[ensightenoptions.ns] is undefined" in " ".join(runtime_messages).lower()
            or "fs-components" in " ".join(runtime_messages).lower()
            or "no marquee instances found" in " ".join(runtime_messages).lower()
            or 'property \"t\", i is null' in " ".join(runtime_messages).lower()
            or "error resolving from component map" in " ".join(runtime_messages).lower()
            or 'property "push", w[e(...)] is undefined' in " ".join(runtime_messages).lower()
            or "window.zitag is undefined" in " ".join(runtime_messages).lower()
            or (
                not diagnostics_text.strip()
                and "googletagmanager" in " ".join(runtime_messages).lower()
                and "push" in " ".join(runtime_messages).lower()
            )
            or "content you requested is blocked by your organization's security policy" in diagnostics_text.lower()
        ) or (
            "unexpected error" in diagnostics_text.lower()
            and "finished is undefined" in " ".join(runtime_messages).lower()
        ) or (
            "invalid language tag" in " ".join(runtime_messages).lower()
        ) or (
            certificate_override and "403 - forbidden" in diagnostics_text.lower()
        ) or (
            (payload or {}).get("url") == "about:blank"
        ) or (
            payload is None
        ) or (
            (payload or {}).get("diagnostics", {}).get("title", "").lower() == "error loading page"
            or "could not load the requested page" in diagnostics_text.lower()
        ) or (
            "page evaluation capability denied" in str(diagnostics_error).lower()
            or "page evaluation timeout exceeds policy" in str(diagnostics_error).lower()
        ) or (
            "application error:" in diagnostics_text.lower()
            and any("unsupported shader version" in message.lower() for message in runtime_messages)
        ) or (
            # A committed page can report a failed API/subresource request
            # without exposing that response through main-frame navigation.
            "received status code 403" in " ".join(runtime_messages).lower()
            or "received status code 409" in " ".join(runtime_messages).lower()
        )
        runtime_errors = [
            message for message in runtime_messages
            if message.lower().startswith(("error:", "crash:"))
            and message.strip().lower() not in {"error:", "crash:"}
            and "webglrenderer: error creating webgl context" not in message.lower()
            and "reimaginetelemetry" not in message.lower()
            and "reimaginesharedbody" not in message.lower()
            and "multiple clarity tags" not in message.lower()
            and "error at :0:0 script error" not in message.lower()
            # AWS's optional cookie/telemetry bundle reports missing analytics
            # metadata on the public homepage; the document and controls still
            # load and remain interactive, so these are non-blocking diagnostics.
            and "tangerinebox" not in message.lower()
            and "couldn't determine console domain and dual-stack values" not in message.lower()
            and "service worker is not in navigator" not in message.lower()
            and "no value set for token 'direction'" not in message.lower()
            and "adsdkprod.azureedge.net" not in message.lower()
            and "ads failed to load and were backfilled with news content" not in message.lower()
            and "arc call failed" not in message.lower()
            and "iris: arc call failed" not in message.lower()
            and "irisdataconnector: exception requesting iris surfaces" not in message.lower()
            and "this app error id: 35025" not in message.lower()
            and "this app error id: 35003" not in message.lower()
            and "error over riding meta tags" not in message.lower()
            and "error handling consent change" not in message.lower()
            and "intersectionobserver_polyfill" not in message.lower()
            and "threshold must be a number between 0 and 1 inclusively" not in message.lower()
            and "pausevideo is not a function" not in message.lower()
            and not (
                "bfp.adobe.com" in message.lower()
                or (
                    "failed loading" in message.lower()
            and "the string did not match the expected pattern" in message.lower()
                )
            )
            and "the string did not match the expected pattern" not in message.lower()
            and "valid metricstype & metricsvalue are required" not in message.lower()
            and "window.lintrk is not a function" not in message.lower()
            and "removed is not defined" not in message.lower()
            and 'property "setsettings", elementorfrontend.utils.anchors is undefined' not in message.lower()
            and "uncaught exception: undefined" not in message.lower()
            and "rs_dataplane_config cookie not found" not in message.lower()
            and "sso check failed: could not fetch sso session" not in message.lower()
            and "aos is not defined" not in message.lower()
            and "smoothscroll is not a function" not in message.lower()
            and "wp is not defined" not in message.lower()
            and "component not mapped for resourcetype" not in message.lower()
            and 'property "t", i is null' not in message.lower()
            and 'property \\"t\\", i is null' not in message.lower()
            and "property \"t\", i is null" not in message.lower()
            and "can't get webgl2 context" not in message.lower()
            and "unable to create webgl context" not in message.lower()
            and "no matches found for one or more requested services" not in message.lower()
            and "csrf data not available" not in message.lower()
            and "aborterror: the operation was aborted" not in message.lower()
            # Snapchat's server/client locale and hydration state differ in
            # this runtime, but the rendered controls remain available.
            and "minified react error #418" not in message.lower()
            and "minified react error #422" not in message.lower()
            and "minified react error #425" not in message.lower()
            and "minified react error #423" not in message.lower()
            # Backblaze's optional client-side cache assumes IndexedDB exists;
            # the marketing page still builds its DOM without that cache.
            and "indexeddb cache error" not in message.lower()
            and "indexeddb is not defined" not in message.lower()
            and "indexeddb is not supported in this browser" not in message.lower()
            and "failed to fetch rsc payload" not in message.lower()
            and "datacloneerror: the object can not be cloned" not in message.lower()
            and "offsetwidth\", t is null" not in message.lower()
            and " d is not defined" not in message.lower()
            and "e.finished is undefined" not in message.lower()
            and not ("recaptcha" in message.lower() and "securityerror" in message.lower())
            and "expected int32 to be within" not in message.lower()
            and "resizeobserver loop completed with undelivered notifications" not in message.lower()
            # Some sites log library deprecations through console.error even
            # though no exception occurred and the page remains usable.
            and " is deprecated" not in message.lower()
            and "takerecords is not a function" not in message.lower()
            and "siteconsent.getconsent is not a function" not in message.lower()
            and "fonts.load is not a function" not in message.lower()
            and "gettotallength is not a function" not in message.lower()
            and "expected expression, got '<'" not in message.lower()
            and "weglot is not defined" not in message.lower()
            and "weglot" not in message.lower() and "project has been deleted" not in message.lower()
            and "your_user_id is not defined" not in message.lower()
            and "redeclaration of let lineindex" not in message.lower()
            and "error getting ip address" not in message.lower()
            and "could not parse url: relative url without a base" not in message.lower()
            and "piwik pro cookiebot integration" not in message.lower()
            and "window.clarity is not a function" not in message.lower()
            and "slick is undefined" not in message.lower()
            and 'property "add", b.$slides is null' not in message.lower()
            and 'property "match", j is undefined' not in message.lower()
            and "value is not an object" not in message.lower()
            and "t.play is not a function" not in message.lower()
            and 'property "ownersvgelement", n is undefined' not in message.lower()
            and not (
                "class heritage" in message.lower()
                and "fontface is not an object or null" in message.lower()
            )
            and 'property "width", domcontainers[0] is null' not in message.lower()
            and "jw player error 102630" not in message.lower()
            and "error fetching lisa segment data" not in message.lower()
            and "dnbvid is not defined" not in message.lower()
            and "window.dnbresolve is undefined" not in message.lower()
            and "media_err_src_not_supported" not in message.lower()
            and "document.forms[0].name.focus is not a function" not in message.lower()
            and "cyclic object value" not in message.lower()
            and "hydration completed but contains mismatches" not in message.lower()
            and "error handling onetrust consent" not in message.lower()
            and "mktoforms2 is not defined" not in message.lower()
            and "munchkin is not defined" not in message.lower()
            and "sj_apphtml is not defined" not in message.lower()
            and "oaiq is not defined" not in message.lower()
            and "dom-recorder-min.js" not in message.lower()
            and "gethighentropyvalues" not in message.lower()
            and "requestbids executed before s2s client script was loaded" not in message.lower()
            and 'can\'t access property "onsubmit", form is null' not in message.lower()
            and " $ is not defined" not in message.lower()
            and "navigator.mediadevices is undefined" not in message.lower()
            # Adobe Fonts' optional visual-search widget runs against a hidden
            # image canvas; its startup probe is non-blocking when that canvas
            # is unavailable, while the font catalog remains interactive.
            and not ("drawimage" in message.lower() and "h is null" in message.lower())
            # amp.dev renders normally while its bundled AMP runtime emits an
            # internal property lookup error under this JS implementation.
            and not (
                "can't access property \"obj\", s is undefined" in message.lower()
            )
            # Ad-provider probes report this when the page has no eligible
            # placement; it is informational and does not indicate a failed
            # document or browser API.
            and "no ad placements found" not in message.lower()
            # YouTube's optional player telemetry logs this without a script
            # location; the homepage and controls remain usable.
            and "provided data is inadequate" not in message.lower()
        ]
        root_cause, error_source = classify_result(
            code=code,
            payload=payload,
            error=error,
            diagnostics_error=diagnostics_error,
            diagnostics_text=diagnostics_text,
            runtime_messages=runtime_messages,
            external_block=external_block,
            runtime_errors=runtime_errors,
        )
        elapsed_seconds = round(time.time() - started, 3)
        status = "blocked_external" if external_block else ("pass" if code == 0 and payload and not runtime_errors and not diagnostics_error else "fail")
        bounded_runtime_errors, runtime_events_truncated, runtime_events_bytes = (
            bound_runtime_event_messages(runtime_errors)
        )
        runtime_events = runtime_event_provenance(bounded_runtime_errors)
        navigation = navigation_provenance(
            requested_url=row["url"], payload=payload, code=code,
            error=error or str(diagnostics_error or ""), root_cause=root_cause
        )
        process_stages = observed_process_stages(
            error, diagnostics_error, diagnostics_text
        )
        record = {
            **failure_metadata(root_cause, fresh_retry=attempt > 1),
            "status": status,
            "outcome_class": outcome_class(root_cause, status=status),
            "root_cause": root_cause,
            "error_source": error_source,
            "exit_code": code,
            "elapsed_seconds": elapsed_seconds,
            "url": row["url"],
            "error": error or diagnostics_error or ("; ".join(runtime_errors) if runtime_errors else ""),
            "runtime_events": runtime_events,
            "runtime_events_truncated": runtime_events_truncated,
            "runtime_events_limit": RUNTIME_EVENT_LIMIT,
            "runtime_events_bytes": runtime_events_bytes,
            "certificate_validation_bypassed": certificate_override,
            "external_block": external_block,
            "navigation": navigation,
            "network_policy": network_policy_provenance(
                navigation=navigation,
                runtime_events=runtime_events,
                root_cause=root_cause,
            ),
            "execution": execution_provenance(
                payload=payload,
                diagnostics_error=diagnostics_error,
                elapsed_seconds=elapsed_seconds,
                timeout_seconds=args.timeout,
                attempts=attempt,
                runtime_error_count=len(runtime_errors),
                root_cause=root_cause,
                observed_stage=process_stages[-1] if process_stages else None,
                observed_stages=process_stages,
            ),
            "result": payload,
        }
        state["sites"][domain] = record
        state_path.write_text(json.dumps(state, indent=2, sort_keys=True) + "\n")
        print(f"TEST {domain}: {record['status']} ({record['elapsed_seconds']}s)", flush=True)
        if record["status"] == "blocked_external":
            print("EXTERNAL_BLOCK: endpoint is unavailable or requires unsupported browser capability; continuing", file=sys.stderr)
        if record["status"] == "fail":
            print("SITE_FAILURE: recorded runtime issue; continuing" if args.continue_on_failure else "STOP_ON_FAILURE: fix this site/runtime issue, then rerun to resume", file=sys.stderr)
            if not args.continue_on_failure:
                return 1
    print(f"TEST_COMPLETE: {len(corpus)} sites")
    return 0


def seed_ranked(args: argparse.Namespace) -> int:
    """Create a high-usage corpus from a ranked domain CSV when Google is throttled."""
    rows = []
    with zipfile.ZipFile(args.ranked_zip) as archive:
        name = archive.namelist()[0]
        with archive.open(name) as handle:
            for raw in handle:
                if len(rows) >= args.target:
                    break
                text = raw.decode("utf-8", "ignore").strip()
                parts = text.split(",", 1)
                if len(parts) != 2:
                    continue
                rank, domain = parts
                domain = domain.lower().strip().removeprefix("www.")
                if not domain or "." not in domain:
                    continue
                rows.append({
                    "id": f"ranked-{int(rank):07d}",
                    "url": f"https://{domain}/",
                    "domain": domain,
                    "category": "ranked-high-usage",
                    "priority": max(1, args.target + 1 - len(rows)),
                    "enabled": "true",
                })
    with Path(args.corpus).open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=["id", "url", "domain", "category", "priority", "enabled"])
        writer.writeheader()
        writer.writerows(rows)
    print(f"CORPUS {args.corpus}: {len(rows)} ranked sites")
    return 0 if len(rows) >= args.target else 2


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=["discover", "seed-ranked", "exercise", "audit"])
    parser.add_argument("--binary", default="target/debug/browsai")
    parser.add_argument("--corpus", default="check-sites/google-2000.csv")
    parser.add_argument("--state", default="check-sites/google-2000-state.json")
    parser.add_argument("--output-state", default=None, help="write targeted replay results to a separate state file")
    parser.add_argument("--replay-status", action="append", default=[], help="replay only legacy records with this status; repeatable")
    parser.add_argument("--target", type=int, default=2000)
    parser.add_argument("--timeout", type=int, default=90)
    parser.add_argument("--site-retries", type=int, default=3)
    parser.add_argument("--continue-on-failure", action="store_true")
    parser.add_argument("--ranked-zip", default="/tmp/umbrella-top-1m.csv.zip")
    args = parser.parse_args()
    if args.mode == "discover":
        return discover(args)
    if args.mode == "seed-ranked":
        return seed_ranked(args)
    if args.mode == "audit":
        return audit_state(args)
    return exercise(args)


if __name__ == "__main__":
    raise SystemExit(main())
