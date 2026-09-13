//! Long-running HTTP/1.1 server with per-domain clean sessions.
//!
//! `browsai serve [--bind <host>] [--port <port>] [--idle-shutdown-seconds <N>] [--fingerprint <id>]`
//! opens a localhost HTTP endpoint that drives the BrowsAI engine directly.
//! Unlike the subprocess-per-request mode that the early prototype used,
//! this server keeps one shared `ServoEngine` for the lifetime of the
//! process and gives each host its own `ContextId` + `PageId`. Cookies,
//! localStorage, IndexedDB, and ServiceWorker registrations are still
//! isolated between domains because Servo's `HttpState.cookie_jar`
//! partitions by host (`vendor/servo-net/cookie_storage.rs:54`) and
//! IndexedDB / ServiceWorker scope are keyed by origin URL.
//!
//! `--idle-shutdown-seconds <N>` (default: off) makes the server
//! exit cleanly after no request has been served for `N` seconds.
//! Idle host sessions are evicted at the same interval so the process
//! does not grow without bound.
//!
//! Why one engine (and not one per host): Servo 0.5 keeps its
//! `servo::Opts` in a process-wide global. A second `ServoRuntime::new()`
//! after the first one panics with "Already initialized" at
//! `servo-config/opts.rs:279`. The CLI hides this by being a fresh
//! process per invocation; a long-running daemon cannot. Sharing one
//! engine keeps `ServoRuntime::new()` to one call.

use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use browsai_agent_runtime::{AgentRuntime, AgentSessionId, SolveAuditEvent, TakeoverManager};
use browsai_agent_tree::StructuralRole;
use browsai_engine_api::{
    BrowserEngine, ContextId, ContextOptions, PageId, ProfileIdentity, VirtualViewport,
};
use browsai_engine_servo::ServoEngine;
use browsai_fingerprint::{FingerprintCatalog, FingerprintId};
use browsai_sandbox::{Capability, SandboxPolicy};
use browsai_state::PageSnapshot;
use url::Url;

const MAX_IDLE_PER_DOMAIN_SECONDS: u64 = 60;

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub bind: String,
    pub port: u16,
    pub idle_shutdown_seconds: Option<u64>,
    pub fingerprint: Option<ProfileIdentity>,
    pub live_browser: bool,
}

/// Per-host `(ContextId, PageId)` pair plus an idle-eviction marker
/// and the `ProfileIdentity` the session was constructed under. The
/// host's webview state lives inside the shared `ServoEngine`'s
/// `real_pages: HashMap<PageId, ServoRuntimePage>` map; this struct
/// just remembers which PageId is ours, when we last used it, and
/// which fingerprint pinned the WebView so a per-request override
/// can evict and rebuild on mismatch.
struct DomainSession {
    context_id: ContextId,
    page_id: PageId,
    last_used: Instant,
    fingerprint: Option<ProfileIdentity>,
}

/// Send-safe control surface. Held by the accept loop thread and the
/// idle sweeper thread. The engine + sessions map are not Send (because
/// `ServoEngine` contains `Rc<RefCell<...>>`); only the accept loop
/// thread touches them.
pub struct ServerControl {
    pub config: ServerConfig,
    pub last_activity: Mutex<Instant>,
    pub should_exit: Arc<AtomicBool>,
    pub active_domains: AtomicUsize,
}

impl Clone for ServerControl {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            last_activity: Mutex::new(*self.last_activity.lock().expect("lock")),
            should_exit: Arc::clone(&self.should_exit),
            active_domains: AtomicUsize::new(self.active_domains.load(Ordering::Relaxed)),
        }
    }
}

/// Single shared engine + per-host page table. Held by the accept-loop
/// thread only; the sweeper thread only touches `ServerControl`.
pub struct ServerState {
    control: ServerControl,
    engine: Mutex<ServoEngine>,
    sessions: Mutex<HashMap<String, DomainSession>>,
}

impl ServerState {
    fn new(config: ServerConfig) -> Self {
        Self {
            control: ServerControl {
                config,
                last_activity: Mutex::new(Instant::now()),
                should_exit: Arc::new(AtomicBool::new(false)),
                active_domains: AtomicUsize::new(0),
            },
            engine: Mutex::new(ServoEngine::new()),
            sessions: Mutex::new(HashMap::new()),
        }
    }

    fn record_activity(&self) {
        *self
            .control
            .last_activity
            .lock()
            .expect("last_activity lock") = Instant::now();
    }

    fn get_or_create_domain(
        &self,
        host: &str,
        fingerprint_override: Option<ProfileIdentity>,
    ) -> Result<(ContextId, PageId), String> {
        // Per-request fingerprint overrides the daemon default. The
        // override is intentionally an `Option<Option<…>>` semantically:
        // `None` means "no body field supplied, fall back to daemon
        // default", `Some(daemon_default)` is the same intent, and
        // `Some(other)` means "evict any stale session pinned to a
        // different fingerprint and create a new one".
        let effective_fingerprint =
            fingerprint_override.or_else(|| self.control.config.fingerprint.clone());
        let mut sessions = self.sessions.lock().expect("sessions lock");
        if let Some(session) = sessions.get_mut(host) {
            if session.fingerprint == effective_fingerprint {
                session.last_used = Instant::now();
                return Ok((session.context_id, session.page_id));
            }
            // Fingerprint mismatch — drop the stale session. The
            // ServoEngine will drop its WebView when the corresponding
            // PageRecord is dropped; we don't tear down the context by
            // hand because the engine owns the WebView lifetime.
            sessions.remove(host);
            self.control.active_domains.fetch_sub(1, Ordering::Relaxed);
        }
        let options = ContextOptions {
            profile: None,
            profile_identity: effective_fingerprint.clone(),
            headless: true,
            use_real_browser_runtime: self.control.config.live_browser,
            viewport: Some(VirtualViewport::default()),
            deterministic_clock_millis: if self.control.config.live_browser {
                None
            } else {
                Some(0)
            },
            no_raster: true,
            http2_profile: None,
            canvas_noise_seed: None,
        };
        // Release the sessions lock before taking the engine lock so the
        // accept loop is never blocked while we construct contexts.
        drop(sessions);
        let mut engine = self.engine.lock().expect("engine lock");
        let context = engine
            .create_context(options)
            .map_err(|error| format!("create_context failed for {host}: {error:?}"))?;
        let page = engine
            .create_page(context)
            .map_err(|error| format!("create_page failed for {host}: {error:?}"))?;
        let mut sessions = self.sessions.lock().expect("sessions lock");
        sessions.insert(
            host.to_string(),
            DomainSession {
                context_id: context,
                page_id: page,
                last_used: Instant::now(),
                fingerprint: effective_fingerprint,
            },
        );
        self.control.active_domains.fetch_add(1, Ordering::Relaxed);
        Ok((context, page))
    }

    fn idle_sweep(&self) {
        let mut sessions = self.sessions.lock().expect("sessions lock");
        let now = Instant::now();
        let threshold = Duration::from_secs(MAX_IDLE_PER_DOMAIN_SECONDS);
        let before = sessions.len();
        sessions.retain(|_, session| now.duration_since(session.last_used) <= threshold);
        self.control
            .active_domains
            .store(sessions.len(), Ordering::Relaxed);
        let _ = before;
    }
}

pub fn run(config: ServerConfig) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind((config.bind.as_str(), config.port))
        .map_err(|error| format!("bind {}:{} failed: {error}", config.bind, config.port))?;
    eprintln!(
        "BROWSAI_SERVER: listening on http://{}:{} (idle_shutdown_seconds={:?})",
        config.bind, config.port, config.idle_shutdown_seconds
    );
    let state = ServerState::new(config);
    let sweeper_control = state.control.clone();
    thread::spawn(move || idle_loop(sweeper_control));
    // The accept-loop thread is the only thread that touches
    // `state.engine` (the `ServoEngine` it contains is !Send).
    // Per-domain idle eviction happens lazily on each accept.
    for incoming in listener.incoming() {
        if state.control.should_exit.load(Ordering::Acquire) {
            eprintln!("BROWSAI_SERVER: idle timeout reached; shutting down");
            break;
        }
        state.idle_sweep();
        match incoming {
            Ok(stream) => match handle_request(stream, &state) {
                Ok(()) => {}
                Err(error) => eprintln!("BROWSAI_SERVER: request error: {error}"),
            },
            Err(error) => eprintln!("BROWSAI_SERVER: accept error: {error}"),
        }
    }
    Ok(())
}

fn idle_loop(control: ServerControl) {
    loop {
        std::thread::sleep(Duration::from_secs(1));
        let Some(idle_secs) = control.config.idle_shutdown_seconds else {
            continue;
        };
        let last = *control.last_activity.lock().expect("last_activity lock");
        if last.elapsed() >= Duration::from_secs(idle_secs) {
            control.should_exit.store(true, Ordering::Release);
            eprintln!("BROWSAI_SERVER: idle for {idle_secs} seconds; exiting");
            std::process::exit(0);
        }
    }
}

pub fn resolve_fingerprint_for_server(id: &str) -> Result<Option<ProfileIdentity>, String> {
    if id.trim().is_empty() {
        return Ok(None);
    }
    let catalog = FingerprintCatalog::default_catalog();
    match catalog.get(&FingerprintId::new(id.to_string())) {
        Some(entry) => Ok(Some(entry.clone().into_profile_identity())),
        None => Err(format!(
            "unknown fingerprint id {id:?}; available: {}",
            catalog
                .ids()
                .map(|f| f.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// Map a fingerprint id (e.g. `firefox-130-linux-x86_64`) to the
/// matching HTTP/2 SETTINGS profile id (`firefox-130`). Returns `None`
/// when the family isn't recognised so the caller can fall back to a
/// generic default. Used so the daemon's `--http2-profile` tracks the
/// chosen `--fingerprint` without an extra flag.
pub fn http2_profile_for_fingerprint(id: &str) -> Option<&'static str> {
    let lower = id.trim().to_ascii_lowercase();
    if lower.starts_with("firefox") {
        Some("firefox-130")
    } else if lower.starts_with("chrome") || lower.starts_with("chromium") {
        Some("chrome-140")
    } else if lower.starts_with("edge") {
        Some("edge")
    } else {
        None
    }
}

#[derive(Debug)]
struct HttpRequest {
    method: String,
    path: String,
    query: String,
    body: Vec<u8>,
}

fn handle_request(mut stream: TcpStream, state: &ServerState) -> std::io::Result<()> {
    state.record_activity();
    let mut reader = BufReader::new(stream.try_clone()?);
    let request_line = match read_request_line(&mut reader) {
        Ok(line) => line,
        Err(error) => {
            let _ = write_http_error(&mut stream, 400, format!("bad request: {error}"));
            return Ok(());
        }
    };
    let (method, path_with_query) = split_request_line(&request_line);
    let (path, query) = split_path_query(path_with_query);
    let mut content_length: usize = 0;
    loop {
        let mut header_line = String::new();
        if reader.read_line(&mut header_line)? == 0 {
            break;
        }
        let trimmed = header_line.trim_end();
        if trimmed.is_empty() {
            break;
        }
        if let Some((name, value)) = trimmed.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse().unwrap_or(0);
            }
        }
    }
    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body)?;
    }
    let request = HttpRequest {
        method,
        path: path.to_string(),
        query: query.to_string(),
        body,
    };
    let response = route(request, state);
    write_response(&mut stream, response)
}

fn read_request_line(reader: &mut BufReader<TcpStream>) -> std::io::Result<String> {
    let mut line = String::new();
    reader.read_line(&mut line)?;
    if line.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "empty request line",
        ));
    }
    Ok(line)
}

fn split_request_line(line: &str) -> (String, &str) {
    let mut parts = line.trim_end().split(' ');
    let method = parts.next().unwrap_or("GET").to_string();
    let target = parts.next().unwrap_or("/");
    (method, target)
}

fn split_path_query(path_with_query: &str) -> (&str, &str) {
    match path_with_query.find('?') {
        Some(index) => (&path_with_query[..index], &path_with_query[index + 1..]),
        None => (path_with_query, ""),
    }
}

struct Response {
    status: u16,
    status_text: &'static str,
    content_type: &'static str,
    body: ResponseBody,
}

enum ResponseBody {
    Static(Vec<u8>),
}

impl Response {
    fn json(status: u16, body: Vec<u8>) -> Self {
        Self {
            status,
            status_text: status_text(status),
            content_type: "application/json",
            body: ResponseBody::Static(body),
        }
    }
    fn text(status: u16, content_type: &'static str, body: Vec<u8>) -> Self {
        Self {
            status,
            status_text: status_text(status),
            content_type,
            body: ResponseBody::Static(body),
        }
    }
}

fn status_text(code: u16) -> &'static str {
    match code {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        500 => "Internal Server Error",
        _ => "Unknown",
    }
}

fn parse_query(query: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (k, v) = match pair.split_once('=') {
            Some((k, v)) => (k, v),
            None => (pair, ""),
        };
        map.insert(percent_decode(k), percent_decode(v));
    }
    map
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
            {
                out.push((high << 4) | low);
                index += 3;
                continue;
            }
        }
        if bytes[index] == b'+' {
            out.push(b' ');
        } else {
            out.push(bytes[index]);
        }
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn route(request: HttpRequest, state: &ServerState) -> Response {
    let path = request.path.as_str();
    let query = parse_query(&request.query);
    let body: Value = if request.body.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&request.body).unwrap_or(Value::Null)
    };

    if request.method == "GET" && path == "/health" {
        return Response::json(200, health_response(state));
    }
    if request.method == "GET" && path == "/capabilities" {
        return Response::json(200, capabilities_response(state));
    }
    if request.method == "GET" && path == "/version" {
        return Response::text(200, "text/plain; charset=utf-8", b"browsai".to_vec());
    }
    if request.method == "GET" && path == "/schema" {
        return Response::json(200, schema_response());
    }
    if path == "/browse" && request.method == "POST" {
        return handle_browse(&body, &query, state);
    }
    if path == "/query" && request.method == "POST" {
        return handle_query_or_render(&body, &query, "query", state);
    }
    if path == "/render" && request.method == "POST" {
        return handle_query_or_render(&body, &query, "render", state);
    }
    if path == "/follow-link" && request.method == "POST" {
        return handle_follow_link(&body, state);
    }
    if path == "/auto-solve" && request.method == "POST" {
        return handle_auto_solve(&body, state);
    }

    let payload = serde_json::json!({
        "error": "not found",
        "method": request.method,
        "path": request.path,
    });
    Response::json(404, serde_json::to_vec_pretty(&payload).unwrap_or_default())
}

fn health_response(state: &ServerState) -> Vec<u8> {
    let last = *state
        .control
        .last_activity
        .lock()
        .expect("last_activity lock");
    let active = state.control.active_domains.load(Ordering::Relaxed);
    let payload = serde_json::json!({
        "live_browser_compiled": cfg!(feature = "live-browser"),
        "servo_loaded": false,
        "egl_available": false,
        "uptime_seconds": last.elapsed().as_secs(),
        "active_domains": active,
        "idle_shutdown_seconds": state.control.config.idle_shutdown_seconds,
        "max_idle_per_domain_seconds": MAX_IDLE_PER_DOMAIN_SECONDS,
        "fingerprint": state.control.config.fingerprint.as_ref().map(|id| id.user_agent.clone()),
    });
    serde_json::to_vec_pretty(&payload).unwrap_or_default()
}

fn capabilities_response(_state: &ServerState) -> Vec<u8> {
    let payload = serde_json::json!({
        "engine_name": "servo-adapter",
        "engine_version": env!("CARGO_PKG_VERSION"),
        "features": ["Navigation", "Snapshots", "NativeInput", "PageEvaluation"],
        "live_browser_compiled": cfg!(feature = "live-browser"),
        "commands": ["version", "capabilities", "browser-health", "navigate", "query",
                     "render", "follow-link", "live-open", "live-search"],
    });
    serde_json::to_vec_pretty(&payload).unwrap_or_default()
}

fn schema_response() -> Vec<u8> {
    let payload = serde_json::json!({
        "schema_version": "browsai-agent-tree/1.0",
        "transports": ["http+json"],
        "endpoints": [
            {"method": "GET", "path": "/health", "returns": "BrowserHealth"},
            {"method": "GET", "path": "/capabilities", "returns": "EngineCapabilities"},
            {"method": "GET", "path": "/version", "returns": "string"},
            {"method": "GET", "path": "/schema", "returns": "Schema"},
            {"method": "POST", "path": "/browse", "returns": "NavigationResult"},
            {"method": "POST", "path": "/query", "returns": "QueryResult"},
            {"method": "POST", "path": "/render", "returns": "RenderResult"},
            {"method": "POST", "path": "/follow-link", "returns": "FollowLinkResult"},
            {"method": "POST", "path": "/auto-solve", "returns": "LiveResult"}
        ],
        "doc": "docs/server.md",
        "tree_doc": "docs/agent-tree-schema.md"
    });
    serde_json::to_vec_pretty(&payload).unwrap_or_default()
}

fn url_host(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    parsed.host_str().map(|host| host.to_ascii_lowercase())
}

/// Resolve an optional `fingerprint` JSON field into a `ProfileIdentity`.
/// Empty / missing fields fall through to `Ok(None)` so the caller can
/// keep using the daemon default. Unknown ids surface a 400 instead of
/// silently degrading to the default — silent fallback was the easiest
/// way to misroute traffic to the wrong fingerprint during testing.
fn resolve_body_fingerprint(body: &Value) -> Result<Option<ProfileIdentity>, String> {
    let raw = match body.get("fingerprint").and_then(Value::as_str) {
        Some(s) if !s.trim().is_empty() => s.trim(),
        _ => return Ok(None),
    };
    let catalog = FingerprintCatalog::default_catalog();
    match catalog.get(&FingerprintId::new(raw.to_string())) {
        Some(entry) => Ok(Some(entry.clone().into_profile_identity())),
        None => Err(format!(
            "unknown fingerprint id {raw:?}; available: {}",
            catalog
                .ids()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// Pick a starting mouse position inside the viewport. Deterministic per
/// (url, invocation_seed) so replays of the same challenge look the
/// same; distinct per request so concurrent calls don't bias toward
/// the same corner. Mirrors `apps/browsai-cli/src/main.rs:36`.
fn auto_solve_cursor_origin(
    viewport: VirtualViewport,
    url: &Url,
    invocation_seed: u64,
) -> (f64, f64) {
    use browsai_input::SplitMix64;
    let url_seed = url.as_str().bytes().fold(0u64, |acc, byte| {
        acc.wrapping_mul(0x100000001B3).wrapping_add(byte as u64)
    });
    let mut rng = SplitMix64::new(url_seed.wrapping_add(invocation_seed));
    let margin = 32.0_f64;
    let max_x = (viewport.width as f64 - margin).max(margin + 1.0);
    let max_y = (viewport.height as f64 - margin).max(margin + 1.0);
    let x = margin + rng.next_unit() * (max_x - margin);
    let y = margin + rng.next_unit() * (max_y - margin);
    (x, y)
}

/// Window over `values` starting at `cursor`, capped by `limit`,
/// `max_bytes`, and `max_duration_ms`. Mirrors
/// `apps/browsai-cli/src/main.rs:1685`.
struct BoundedStringPage {
    items: Vec<String>,
    truncated: bool,
    next_cursor: Option<String>,
    bytes_used: usize,
    duration_ms: u128,
}

fn bounded_string_page(
    values: &[String],
    cursor: usize,
    limit: usize,
    max_bytes: usize,
    max_duration_ms: usize,
) -> BoundedStringPage {
    let started = Instant::now();
    let start = cursor.min(values.len());
    let mut items = Vec::new();
    let mut bytes_used = 0usize;
    let mut next_index = start;
    for (index, value) in values.iter().enumerate().skip(start).take(limit.max(1)) {
        let elapsed = started.elapsed().as_millis();
        if elapsed >= max_duration_ms as u128 && !items.is_empty() {
            break;
        }
        let item_bytes = value.len().saturating_add(3);
        if bytes_used.saturating_add(item_bytes) > max_bytes && !items.is_empty() {
            break;
        }
        if item_bytes > max_bytes && items.is_empty() {
            next_index = index.saturating_add(1);
            break;
        }
        bytes_used = bytes_used.saturating_add(item_bytes);
        items.push(value.clone());
        next_index = index.saturating_add(1);
    }
    let truncated = next_index < values.len();
    BoundedStringPage {
        items,
        truncated,
        next_cursor: truncated.then_some(next_index.to_string()),
        bytes_used,
        duration_ms: started.elapsed().as_millis(),
    }
}

/// Run the challenge-observer loop using a freshly-created AgentRuntime
/// + session. Returns the audit trail. Mirrors
///   `apps/browsai-cli/src/main.rs::auto_solve_challenges`.
#[allow(clippy::too_many_arguments)]
fn run_auto_solve(
    engine: &mut ServoEngine,
    page: PageId,
    snapshot: &PageSnapshot,
    runtime: &mut AgentRuntime,
    session: AgentSessionId,
    takeovers: &TakeoverManager,
    now: u64,
    cursor_origin: (f64, f64),
) -> Result<Vec<SolveAuditEvent>, String> {
    browsai_agent_runtime::solve_observed_challenges(
        page,
        snapshot,
        runtime,
        session,
        takeovers,
        now,
        cursor_origin,
        |event| {
            engine
                .dispatch_input(page, event.clone())
                .map_err(|error| format!("auto-solve dispatch failed: {error:?}"))
        },
        |error| eprintln!("BROWSAI_AUTO_SOLVE_ERROR: {error}"),
    )
}

fn handle_live_auto_solve(body: &Value, state: &ServerState) -> Response {
    // Auto-solve only makes sense against the live runtime — the
    // deterministic backend returns a single Page root node with no
    // challenge observers. Refuse early so the operator knows to add
    // --live-browser rather than getting back a confusing 200 with no
    // candidate_links.
    if !state.control.config.live_browser {
        let payload = serde_json::json!({
            "error": "auto-solve requires the daemon to be started with --live-browser (or BROWSAI_SERVER_LIVE_BROWSER=1); the deterministic backend has no JS-driven challenges to solve"
        });
        return Response::json(503, serde_json::to_vec(&payload).unwrap_or_default());
    }
    let url = match body.get("url").and_then(Value::as_str) {
        Some(s) => s.to_string(),
        None => {
            let payload = serde_json::json!({"error": "missing url"});
            return Response::json(400, serde_json::to_vec(&payload).unwrap_or_default());
        }
    };
    let parsed_url = match Url::parse(&url) {
        Ok(u) => u,
        Err(error) => {
            let payload = serde_json::json!({"error": format!("invalid url: {error}")});
            return Response::json(400, serde_json::to_vec(&payload).unwrap_or_default());
        }
    };
    let host = match parsed_url.host_str() {
        Some(host) => host.to_ascii_lowercase(),
        None => {
            let payload = serde_json::json!({"error": "url has no host"});
            return Response::json(400, serde_json::to_vec(&payload).unwrap_or_default());
        }
    };
    let fingerprint_override = match resolve_body_fingerprint(body) {
        Ok(f) => f,
        Err(error) => {
            let payload = serde_json::json!({"error": error});
            return Response::json(400, serde_json::to_vec(&payload).unwrap_or_default());
        }
    };
    let wait_ms = body.get("wait_ms").and_then(Value::as_u64).unwrap_or(1000);
    // HTTP defaults are larger than the CLI's because the only HTTP
    // callers (Helios plugin + curl) want to see the full candidate
    // set on the first call, not page through with cursor=2/limit=2.
    let link_cursor = body.get("link_cursor").and_then(Value::as_u64).unwrap_or(0) as usize;
    let link_limit = body
        .get("link_limit")
        .and_then(Value::as_u64)
        .unwrap_or(50)
        .max(1) as usize;
    let link_max_bytes = body
        .get("link_max_bytes")
        .and_then(Value::as_u64)
        .unwrap_or(64 * 1024) as usize;
    let link_max_duration_ms = body
        .get("link_max_duration_ms")
        .and_then(Value::as_u64)
        .unwrap_or(2_000) as usize;
    let textbox_limit = body
        .get("textbox_limit")
        .and_then(Value::as_u64)
        .unwrap_or(100)
        .max(1) as usize;
    let control_limit = body
        .get("control_limit")
        .and_then(Value::as_u64)
        .unwrap_or(100)
        .max(1) as usize;

    eprintln!("BROWSAI_STAGE:auto_solve startup host={host}");
    let (_ctx, page_id) = match state.get_or_create_domain(&host, fingerprint_override) {
        Ok(ids) => ids,
        Err(error) => return error_response(error),
    };
    let _navigation = {
        let mut engine = state.engine.lock().expect("engine lock");
        match engine.navigate(page_id, parsed_url.clone()) {
            Ok(nav) => nav,
            Err(error) => return error_response(format!("navigate failed: {error:?}")),
        }
    };
    if wait_ms > 0 {
        let engine = state.engine.lock().expect("engine lock");
        if let Err(error) = engine.pump_runtime(page_id, wait_ms) {
            return error_response(format!("post-navigation pump failed: {error:?}"));
        }
    }
    eprintln!("BROWSAI_STAGE:auto_solve dom_projection");
    let initial_snapshot = {
        let engine = state.engine.lock().expect("engine lock");
        match engine.snapshot(page_id) {
            Ok(snap) => snap,
            Err(error) => return error_response(format!("snapshot failed: {error:?}")),
        }
    };
    // Auto-solve challenge loop. The AgentRuntime + TakeoverManager are
    // short-lived for this single request — they carry no state that
    // needs to survive across requests (cookies/storage live inside
    // Servo).
    eprintln!("BROWSAI_STAGE:auto_solve challenge_loop");
    let mut runtime = AgentRuntime::new(SandboxPolicy::autonomous_agent());
    let session = runtime.open([Capability::SolveChallenge], Default::default());
    if let Err(error) = runtime.start(session) {
        return error_response(format!("agent runtime start failed: {error}"));
    }
    let takeovers = TakeoverManager::default();
    let now = runtime.now_millis();
    let cursor_origin = auto_solve_cursor_origin(VirtualViewport::default(), &parsed_url, 1);
    eprintln!(
        "BROWSAI_AUTO_SOLVE: initial cursor = ({:.1}, {:.1})",
        cursor_origin.0, cursor_origin.1
    );
    let auto_solve_audit = {
        let mut engine = state.engine.lock().expect("engine lock");
        match run_auto_solve(
            &mut engine,
            page_id,
            &initial_snapshot,
            &mut runtime,
            session,
            &takeovers,
            now,
            cursor_origin,
        ) {
            Ok(events) => {
                for event in &events {
                    eprintln!(
                        "BROWSAI_AUTO_SOLVE: provider={} capability={} target={:?}",
                        event.provider, event.capability_used, event.target_node_id
                    );
                }
                events
            }
            Err(error) => {
                eprintln!("BROWSAI_AUTO_SOLVE_ERROR: {error}");
                Vec::new()
            }
        }
    };
    let snapshot = if auto_solve_audit.is_empty() {
        initial_snapshot
    } else {
        let engine = state.engine.lock().expect("engine lock");
        match engine.snapshot(page_id) {
            Ok(snap) => snap,
            Err(error) => {
                return error_response(format!("post-auto-solve snapshot failed: {error:?}"))
            }
        }
    };
    let visible_link_ids = snapshot
        .tree
        .nodes
        .iter()
        .filter(|node| node.structural_role == StructuralRole::Link && node.state.visible)
        .map(|node| node.id.clone())
        .collect::<Vec<_>>();
    let link_page = bounded_string_page(
        &visible_link_ids,
        link_cursor,
        link_limit,
        link_max_bytes,
        link_max_duration_ms,
    );
    let candidate_links = link_page.items.clone();
    let textbox_count = snapshot
        .tree
        .nodes
        .iter()
        .filter(|node| node.structural_role == StructuralRole::Textbox)
        .count();
    let candidate_textboxes: Vec<serde_json::Value> = snapshot
        .tree
        .nodes
        .iter()
        .filter(|node| node.structural_role == StructuralRole::Textbox)
        .map(|node| {
            serde_json::json!({
                "id": node.id,
                "name": node.name,
                "visible": node.state.visible,
                "geometry": node.geometry,
            })
        })
        .take(textbox_limit)
        .collect();
    let candidate_control_count = snapshot
        .tree
        .nodes
        .iter()
        .filter(|node| {
            node.state.visible
                && matches!(
                    node.structural_role,
                    StructuralRole::Button
                        | StructuralRole::Checkbox
                        | StructuralRole::Radio
                        | StructuralRole::Link
                )
        })
        .count();
    let candidate_controls: Vec<serde_json::Value> = snapshot
        .tree
        .nodes
        .iter()
        .filter(|node| {
            node.state.visible
                && matches!(
                    node.structural_role,
                    StructuralRole::Button
                        | StructuralRole::Checkbox
                        | StructuralRole::Radio
                        | StructuralRole::Link
                )
        })
        .take(control_limit)
        .map(|node| {
            serde_json::json!({
                "id": node.id,
                "role": format!("{:?}", node.structural_role),
                "name": node.name,
                "geometry": node.geometry,
            })
        })
        .collect();

    let diagnostics_url = match state
        .engine
        .lock()
        .expect("engine lock")
        .runtime_current_url(page_id)
    {
        Ok(u) => u,
        Err(error) => return error_response(format!("runtime_current_url failed: {error:?}")),
    };
    let (navigation_history, navigation_request_history, navigation_request_history_truncated) = {
        let engine = state.engine.lock().expect("engine lock");
        let h = match engine.runtime_navigation_history(page_id) {
            Ok(h) => h,
            Err(error) => return error_response(format!("navigation history failed: {error:?}")),
        };
        let r = match engine.runtime_navigation_request_history(page_id) {
            Ok(r) => r,
            Err(error) => {
                return error_response(format!("navigation request history failed: {error:?}"))
            }
        };
        let t = match engine.runtime_navigation_request_history_truncated(page_id) {
            Ok(t) => t,
            Err(error) => {
                return error_response(format!(
                    "navigation request history truncated failed: {error:?}"
                ))
            }
        };
        (h, r, t)
    };
    let mut redirect_chain: Vec<String> = navigation_request_history
        .iter()
        .map(|url| url.as_str().to_owned())
        .collect();
    for url in navigation_history.iter().map(|url| url.as_str()) {
        if redirect_chain.last().map(String::as_str) != Some(url) {
            redirect_chain.push(url.to_owned());
        }
    }
    let (
        load_status,
        resource_request_history,
        resource_request_history_truncated,
        runtime_messages,
    ) = {
        let engine = state.engine.lock().expect("engine lock");
        let ls = match engine.runtime_load_status(page_id) {
            Ok(s) => s,
            Err(error) => return error_response(format!("runtime_load_status failed: {error:?}")),
        };
        let rrh = match engine.runtime_resource_request_history(page_id) {
            Ok(r) => r,
            Err(error) => {
                return error_response(format!("resource_request_history failed: {error:?}"))
            }
        };
        let rrt = match engine.runtime_resource_request_history_truncated(page_id) {
            Ok(t) => t,
            Err(error) => {
                return error_response(format!(
                    "resource_request_history_truncated failed: {error:?}"
                ))
            }
        };
        let rm = match engine.runtime_messages(page_id) {
            Ok(m) => m,
            Err(error) => return error_response(format!("runtime_messages failed: {error:?}")),
        };
        (ls, rrh, rrt, rm)
    };
    eprintln!("BROWSAI_STAGE:auto_solve evaluation");
    let payload = serde_json::json!({
        "page": page_id,
        "url": snapshot.url,
        "navigation": {
            "requested_url": url,
            "final_url": diagnostics_url,
            "history": redirect_chain,
            "history_truncated": navigation_request_history_truncated,
            "redirect_observed": redirect_chain.len() > 1,
            "http_status": Value::Null,
            "mime_type": Value::Null,
            "origin": diagnostics_url.origin().ascii_serialization(),
            "initiator": Value::Null,
            "resource_type": "main_frame",
        },
        "load_status": load_status,
        "resource_diagnostics": Value::Null,
        "resource_requests": resource_request_history
            .into_iter()
            .map(|u| u.to_string())
            .collect::<Vec<_>>(),
        "resource_requests_truncated": resource_request_history_truncated,
        "runtime_messages": runtime_messages,
        "node_count": snapshot.tree.nodes.len(),
        "agent_tree_truncated": snapshot.tree.truncated,
        "candidate_links": candidate_links,
        "candidate_links_cursor": link_cursor,
        "candidate_links_limit": link_limit,
        "candidate_links_truncated": link_page.truncated,
        "candidate_links_next_cursor": link_page.next_cursor,
        "candidate_links_bytes": link_page.bytes_used,
        "candidate_links_max_bytes": link_max_bytes,
        "candidate_links_duration_ms": link_page.duration_ms,
        "candidate_links_max_duration_ms": link_max_duration_ms,
        "candidate_textboxes": candidate_textboxes,
        "candidate_textboxes_truncated": textbox_count > candidate_textboxes.len(),
        "candidate_controls": candidate_controls,
        "candidate_controls_truncated": candidate_control_count > candidate_controls.len(),
        "clicked_links": Vec::<String>::new(),
        "probed_controls": Vec::<String>::new(),
        "skipped_controls": Vec::<String>::new(),
        "auto_solve_audit": auto_solve_audit,
        "diagnostics": serde_json::json!({
            "document_available": diagnostics_url.as_str() != "about:blank",
            "diagnostics_mode": "snapshot-only",
        }),
    });
    let body_bytes = serde_json::to_vec_pretty(&payload).unwrap_or_default();
    Response::json(200, body_bytes)
}

fn handle_browse(
    body: &Value,
    query: &std::collections::HashMap<String, String>,
    state: &ServerState,
) -> Response {
    let url = match body.get("url").and_then(Value::as_str) {
        Some(s) => s.to_string(),
        None => {
            return Response::json(400, b"{\"error\":\"missing url\"}".to_vec());
        }
    };
    let host = match url_host(&url) {
        Some(h) => h,
        None => {
            return Response::json(
                400,
                format!("{{\"error\":\"invalid url {url:?}\"}}").into_bytes(),
            );
        }
    };
    let snapshot_only = body.get("snapshot_only").and_then(Value::as_bool) == Some(true);
    let query_text = body
        .get("query")
        .and_then(Value::as_str)
        .map(|q| q.to_string());
    let cursor = body.get("cursor").and_then(Value::as_u64).unwrap_or(0) as usize;
    let limit = body
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(100)
        .max(1) as usize;
    let format = body.get("format").and_then(Value::as_str).unwrap_or("tree");
    let wait_for_network_idle = body
        .get("wait_for_network_idle")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let network_idle_ms = body
        .get("network_idle_ms")
        .and_then(Value::as_u64)
        .unwrap_or(500);
    let network_idle_max_ms = body
        .get("network_idle_max_ms")
        .and_then(Value::as_u64)
        .unwrap_or(8000);
    let roles_filter: Vec<String> = body
        .get("filter")
        .and_then(Value::as_str)
        .map(|value| {
            value
                .split(',')
                .map(|role| role.trim().to_string())
                .filter(|role| !role.is_empty())
                .collect()
        })
        .unwrap_or_default();
    // Default to a 1s settle after navigation. The CLI's `live-open`
    // unconditionally pumps the runtime for 2s after navigate() returns;
    // `load_status::Complete` alone doesn't guarantee that JS-driven DOM
    // (DuckDuckGo's React bundle, X, login walls) has settled. The
    // server caller can still override with `wait_ms: 0` for
    // server-rendered pages where the snapshot is already complete.
    let wait_ms = body.get("wait_ms").and_then(Value::as_u64).unwrap_or(1000);
    let _ = query; // streaming is a future extension; for v1 always single-blob

    let (_context_id, page_id) = match state.get_or_create_domain(&host, None) {
        Ok(ids) => ids,
        Err(error) => return error_response(error),
    };
    let navigation = {
        let mut engine = state.engine.lock().expect("engine lock");
        match engine.navigate(page_id, url::Url::parse(&url).expect("checked url")) {
            Ok(nav) => nav,
            Err(error) => return error_response(format!("navigate failed: {error:?}")),
        }
    };
    if wait_ms > 0 {
        let engine = state.engine.lock().expect("engine lock");
        if let Err(error) = engine.pump_runtime(page_id, wait_ms) {
            return error_response(format!("wait_ms pump failed: {error:?}"));
        }
    }
    if wait_for_network_idle {
        let mut engine = state.engine.lock().expect("engine lock");
        if let Err(error) =
            engine.wait_for_network_idle(page_id, network_idle_ms, network_idle_max_ms)
        {
            // Network-idle wait is best-effort. Log and continue with
            // the snapshot rather than failing the whole request.
            eprintln!(
                "BROWSAI_NETWORK_IDLE_ERROR: {error:?} (idle_ms={network_idle_ms}, max_ms={network_idle_max_ms})"
            );
        }
    }
    if snapshot_only {
        return Response::json(
            200,
            serde_json::to_vec(&serde_json::json!({
                "page": navigation.page,
                "url": navigation.url,
            }))
            .unwrap_or_default(),
        );
    }
    let snapshot = {
        let engine = state.engine.lock().expect("engine lock");
        match engine.snapshot(page_id) {
            Ok(snap) => snap,
            Err(error) => return error_response(format!("snapshot failed: {error:?}")),
        }
    };
    let mut snapshot = snapshot;
    if let Some(terms) = query_text.as_deref() {
        bias_confidence_by_query(&mut snapshot, terms);
    }
    if !roles_filter.is_empty() {
        let view =
            browsai_agent_protocol::PageQuery::new(&snapshot.tree).render_page(cursor, limit);
        let mut results = view.results;
        results.retain(|row| {
            snapshot
                .tree
                .nodes
                .iter()
                .any(|node| node.id == row.node_id && node_matches_role_static(node, &roles_filter))
        });
        let payload = serde_json::json!({
            "results": results,
            "cursor": view.offset,
            "limit": limit,
            "total": view.total,
            "truncated": view.next_offset.is_some(),
            "next_cursor": view.next_offset.map(|offset| offset.to_string()),
            "filter": roles_filter,
        });
        return Response::json(200, serde_json::to_vec_pretty(&payload).unwrap_or_default());
    }
    if format == "markdown" || format == "text" {
        let page_text_format = if format == "text" {
            browsai_page_text::PageTextFormat::Plain
        } else {
            browsai_page_text::PageTextFormat::Markdown
        };
        let opts = browsai_page_text::MarkdownOptions {
            format: page_text_format,
            ..Default::default()
        };
        let view = browsai_page_text::MarkdownEmitter::new(&snapshot.tree).emit(&opts);
        let payload = serde_json::json!({
            "format": format,
            "format_version": view.format_version,
            "page": page_id,
            "url": navigation.url,
            "content": view.content,
            "node_index": view.node_index,
            "skipped_nodes": view.skipped_nodes,
        });
        return Response::json(200, serde_json::to_vec_pretty(&payload).unwrap_or_default());
    }
    if cursor > 0 || limit != 100 {
        let view =
            browsai_agent_protocol::PageQuery::new(&snapshot.tree).render_page(cursor, limit);
        return Response::json(200, serde_json::to_vec(&view).unwrap_or_default());
    }
    let payload = serde_json::json!({
        "page": page_id,
        "url": navigation.url,
        "node_count": snapshot.tree.nodes.len(),
        "results": browsai_agent_protocol::PageQuery::new(&snapshot.tree).render(None),
    });
    Response::json(200, serde_json::to_vec_pretty(&payload).unwrap_or_default())
}

fn handle_query_or_render(
    body: &Value,
    query: &std::collections::HashMap<String, String>,
    command: &str,
    state: &ServerState,
) -> Response {
    if body.get("url").and_then(Value::as_str).is_none() {
        let payload = serde_json::json!({
            "error": format!("{command} requires a `url` field; it navigates and filters in one call"),
        });
        return Response::json(400, serde_json::to_vec(&payload).unwrap_or_default());
    }
    let _ = command;
    handle_browse(body, query, state)
}

fn handle_follow_link(_body: &Value, _state: &ServerState) -> Response {
    let payload = serde_json::json!({
        "error": "follow-link requires a per-page plan; not yet wired through the long-running server"
    });
    Response::json(501, serde_json::to_vec(&payload).unwrap_or_default())
}

fn handle_auto_solve(body: &Value, state: &ServerState) -> Response {
    handle_live_auto_solve(body, state)
}

fn error_response(message: String) -> Response {
    let payload = serde_json::json!({ "error": message });
    Response::json(500, serde_json::to_vec(&payload).unwrap_or_default())
}

fn bias_confidence_by_query(snapshot: &mut browsai_state::PageSnapshot, query: &str) {
    if query.trim().is_empty() {
        return;
    }
    let terms: Vec<String> = query
        .split_whitespace()
        .map(|term| term.to_ascii_lowercase())
        .filter(|term| term.len() >= 2)
        .collect();
    if terms.is_empty() {
        return;
    }
    let needle = terms.join(" ");
    for node in snapshot.tree.nodes.iter_mut() {
        if let Some(name) = node.name.as_deref() {
            let lower = name.to_ascii_lowercase();
            if lower.contains(&needle) || terms.iter().any(|t| lower.contains(t)) {
                node.confidence = browsai_provenance::Confidence(1.0);
                continue;
            }
        }
        let description = node.description.as_deref().unwrap_or("");
        let combined = description.to_ascii_lowercase();
        if combined.contains(&needle) || terms.iter().any(|t| combined.contains(t)) {
            node.confidence = browsai_provenance::Confidence(0.9);
        }
    }
}

fn node_matches_role_static(node: &browsai_agent_tree::AgentNode, roles: &[String]) -> bool {
    if roles.is_empty() {
        return true;
    }
    let role = format!("{:?}", node.structural_role);
    roles.iter().any(|r| r == &role)
}

fn write_response(stream: &mut TcpStream, response: Response) -> std::io::Result<()> {
    let ResponseBody::Static(bytes) = response.body;
    let header = format!(
        "HTTP/1.1 {} {}\r\n\
         Content-Type: {}\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n",
        response.status,
        response.status_text,
        response.content_type,
        bytes.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(&bytes)?;
    stream.flush()?;
    let _ = stream.shutdown(std::net::Shutdown::Both);
    Ok(())
}

fn write_http_error(stream: &mut TcpStream, status: u16, message: String) -> std::io::Result<()> {
    let body = serde_json::to_vec(&serde_json::json!({"error": message})).unwrap_or_default();
    let header = format!(
        "HTTP/1.1 {} {}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n",
        status,
        status_text(status),
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(&body)?;
    stream.flush()?;
    let _ = stream.shutdown(std::net::Shutdown::Both);
    Ok(())
}
