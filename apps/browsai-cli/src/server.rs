//! Long-running HTTP/1.1 server with per-domain clean sessions.
//!
//! `browsai serve [--bind <host>] [--port <port>] [--idle-shutdown-seconds <N>] [--fingerprint <id>]`
//! opens a localhost HTTP endpoint that drives the BrowsAI engine directly.
//! Unlike the subprocess-per-request mode that the early prototype used,
//! this server keeps one `ServoEngine` per host so cookies, localStorage,
//! IndexedDB, and ServiceWorker registrations are isolated between
//! domains. The first request to `example.com` allocates a fresh
//! engine; subsequent requests to the same host reuse it; requests to
//! `other.com` get a separate fresh engine.
//!
//! `--idle-shutdown-seconds <N>` (default: off) makes the server
//! exit cleanly after no request has been served for `N` seconds.
//! Idle domains are evicted at the same interval so the process does
//! not grow without bound.

use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use browsai_engine_api::{
    BrowserEngine, ContextId, ContextOptions, PageId, ProfileIdentity, VirtualViewport,
};
use browsai_engine_servo::ServoEngine;
use browsai_fingerprint::{FingerprintCatalog, FingerprintId};

const MAX_IDLE_PER_DOMAIN_SECONDS: u64 = 60;

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub bind: String,
    pub port: u16,
    pub idle_shutdown_seconds: Option<u64>,
    pub fingerprint: Option<ProfileIdentity>,
}

struct DomainEngine {
    engine: ServoEngine,
    context_id: ContextId,
    page_id: PageId,
    last_used: Instant,
}

/// Send-safe control surface. Held by the accept loop thread and the
/// idle sweeper thread. The engines map itself is not Send (because
/// `ServoEngine` contains `Rc<RefCell<...>>`); only the accept loop
/// thread touches it.
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

/// Per-domain engines. Held by the accept-loop thread only. The
/// `!Send` bound on `ServoEngine` means we cannot share this across
/// threads; the sweeper thread must talk to the engine map via a
/// channel or through the accept loop, never directly.
pub struct ServerState {
    control: ServerControl,
    engines: Mutex<HashMap<String, DomainEngine>>,
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
            engines: Mutex::new(HashMap::new()),
        }
    }

    fn record_activity(&self) {
        *self
            .control
            .last_activity
            .lock()
            .expect("last_activity lock") = Instant::now();
    }

    fn get_or_create_domain(&self, host: &str) -> Result<(ContextId, PageId), String> {
        let mut engines = self.engines.lock().expect("engines lock");
        if let Some(entry) = engines.get_mut(host) {
            entry.last_used = Instant::now();
            return Ok((entry.context_id, entry.page_id));
        }
        let options = ContextOptions {
            headless: true,
            viewport: Some(VirtualViewport::default()),
            deterministic_clock_millis: Some(0),
            profile_identity: self.control.config.fingerprint.clone(),
            ..Default::default()
        };
        drop(engines);
        let mut engine = ServoEngine::new();
        let context = engine
            .create_context(options)
            .map_err(|error| format!("create_context failed for {host}: {error:?}"))?;
        let page = engine
            .create_page(context)
            .map_err(|error| format!("create_page failed for {host}: {error:?}"))?;
        let mut engines = self.engines.lock().expect("engines lock");
        engines.insert(
            host.to_string(),
            DomainEngine {
                engine,
                context_id: context,
                page_id: page,
                last_used: Instant::now(),
            },
        );
        self.control.active_domains.fetch_add(1, Ordering::Relaxed);
        let entry = engines.get(host).expect("domain engine just inserted");
        Ok((entry.context_id, entry.page_id))
    }

    fn idle_sweep(&self) {
        let mut engines = self.engines.lock().expect("engines lock");
        let now = Instant::now();
        let threshold = Duration::from_secs(MAX_IDLE_PER_DOMAIN_SECONDS);
        let before = engines.len();
        engines.retain(|_, entry| now.duration_since(entry.last_used) <= threshold);
        self.control
            .active_domains
            .store(engines.len(), Ordering::Relaxed);
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
    // `state.engines` (the `ServoEngine` it contains is !Send).
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
        return handle_query_or_render(&body, "query", state);
    }
    if path == "/render" && request.method == "POST" {
        return handle_query_or_render(&body, "render", state);
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
    let _ = query; // streaming is a future extension; for v1 always single-blob

    let (_context_id, page_id) = match state.get_or_create_domain(&host) {
        Ok(ids) => ids,
        Err(error) => return error_response(error),
    };
    let navigation = {
        let mut engines = state.engines.lock().expect("engines lock");
        let entry = engines.get_mut(&host).expect("domain engine present");
        match entry
            .engine
            .navigate(entry.page_id, url::Url::parse(&url).expect("checked url"))
        {
            Ok(nav) => nav,
            Err(error) => return error_response(format!("navigate failed: {error:?}")),
        }
    };
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
        let mut engines = state.engines.lock().expect("engines lock");
        let entry = engines.get_mut(&host).expect("domain engine present");
        match entry.engine.snapshot(entry.page_id) {
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

fn handle_query_or_render(_body: &Value, command: &str, _state: &ServerState) -> Response {
    let payload = serde_json::json!({
        "error": format!(
            "{command} requires a snapshot, but the long-running server \
             always navigates first; use POST /browse instead"
        ),
    });
    Response::json(400, serde_json::to_vec(&payload).unwrap_or_default())
}

fn handle_follow_link(_body: &Value, _state: &ServerState) -> Response {
    let payload = serde_json::json!({
        "error": "follow-link requires a per-page plan; not yet wired through the long-running server"
    });
    Response::json(501, serde_json::to_vec(&payload).unwrap_or_default())
}

fn handle_auto_solve(_body: &Value, _state: &ServerState) -> Response {
    let payload = serde_json::json!({
        "error": "auto-solve requires runtime + takeover plumbing; not yet wired through the long-running server"
    });
    Response::json(501, serde_json::to_vec(&payload).unwrap_or_default())
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
