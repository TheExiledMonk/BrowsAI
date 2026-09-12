//! Minimal HTTP/1.1 server for browser control over a localhost TCP socket.
//!
//! The server is a thin wrapper around the `browsai` CLI. Each request
//! spawns a subprocess running the CLI with the matching arguments and
//! streams the subprocess stdout back to the HTTP client over chunked
//! transfer encoding. There is no async runtime, no tokio, and no
//! hyper — only `std::net`, `std::io::BufRead`, and `std::process`.
//!
//! Routes:
//!
//! | Method | Path               | Body                                | Behaviour                  |
//! | ------ | ------------------ | ----------------------------------- | -------------------------- |
//! | GET    | /health            | (none)                              | `browsai browser-health` JSON |
//! | GET    | /capabilities      | (none)                              | `browsai capabilities` JSON |
//! | GET    | /schema            | (none)                              | static schema-version JSON |
//! | GET    | /version           | (none)                              | `browsai version` string  |
//! | POST   | /browse            | `{url, fingerprint?, query?, filter?, snapshot_only?, headless?, auto_solve?, cursor?, limit?, stream?}` | `browsai navigate <url> ...` (streams NDJSON when `stream=true`) |
//! | POST   | /render            | `{url, query?, filter?, cursor?, limit?, stream?, fingerprint?}`           | `browsai render <url> ...` |
//! | POST   | /query             | `{url, query?, filter?, cursor?, limit?, stream?, fingerprint?}`           | `browsai query  <url> ...` |
//! | POST   | /follow-link       | `{page, node_id, stream?, fingerprint?}`                                    | `browsai follow-link <page> <node_id>` |
//! | POST   | /auto-solve        | `{url, fingerprint?}`                                                       | `browsai live-open <url> --auto-solve` |
//!
//! Streaming: when the request body or path includes `?stream=true`,
//! the response is `Transfer-Encoding: chunked` and each NDJSON event
//! from the CLI is forwarded as one chunk. The connection stays open
//! until the CLI emits a `snapshot-complete` event (or its terminator).
//!
//! Auth: the server binds to `127.0.0.1` by default. Pass `--bind
//! 0.0.0.0` to expose it to other hosts; the listener does not
//! authenticate, so the operator is responsible for running the
//! service behind a trusted network or tunnel.

use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

pub fn run(bind: &str, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind((bind, port))
        .map_err(|error| format!("bind {bind}:{port} failed: {error}"))?;
    eprintln!("BROWSAI_SERVER: listening on http://{bind}:{port}");
    for incoming in listener.incoming() {
        match incoming {
            Ok(stream) => {
                let bind = bind.to_string();
                thread::spawn(move || {
                    let _ = handle_request(stream, &bind);
                });
            }
            Err(error) => {
                eprintln!("BROWSAI_SERVER: accept error: {error}");
            }
        }
    }
    Ok(())
}

#[derive(Debug)]
struct HttpRequest {
    method: String,
    path: String,
    query: String,
    body: Vec<u8>,
}

fn handle_request(mut stream: TcpStream, bind: &str) -> std::io::Result<()> {
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
    let response = route(request, bind);
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
    Streaming {
        lines: mpsc::Receiver<String>,
        binary: std::sync::Arc<std::sync::Mutex<bool>>,
    },
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

    fn ndjson_streaming(
        status: u16,
        lines: mpsc::Receiver<String>,
        binary: std::sync::Arc<std::sync::Mutex<bool>>,
    ) -> Self {
        Self {
            status,
            status_text: status_text(status),
            content_type: "application/x-ndjson",
            body: ResponseBody::Streaming { lines, binary },
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

fn route(request: HttpRequest, _bind: &str) -> Response {
    let path = request.path.as_str();
    let query = parse_query(&request.query);
    let body: Value = if request.body.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&request.body).unwrap_or(Value::Null)
    };

    if request.method == "GET" && path == "/health" {
        return invoke_cli_blocking(vec!["browser-health".into()]);
    }
    if request.method == "GET" && path == "/capabilities" {
        return invoke_cli_blocking(vec!["capabilities".into()]);
    }
    if request.method == "GET" && path == "/version" {
        return invoke_cli_blocking(vec!["version".into()]);
    }
    if request.method == "GET" && path == "/schema" {
        return schema_response();
    }
    if path == "/browse" && request.method == "POST" {
        return handle_browse(&body, &query);
    }
    if path == "/render" && request.method == "POST" {
        return handle_render(&body, &query);
    }
    if path == "/query" && request.method == "POST" {
        return handle_query(&body, &query);
    }
    if path == "/follow-link" && request.method == "POST" {
        return handle_follow_link(&body, &query);
    }
    if path == "/auto-solve" && request.method == "POST" {
        return handle_auto_solve(&body, &query);
    }

    let payload = serde_json::json!({
        "error": "not found",
        "method": request.method,
        "path": request.path,
    });
    Response::json(404, serde_json::to_vec_pretty(&payload).unwrap_or_default())
}

fn schema_response() -> Response {
    let payload = serde_json::json!({
        "schema_version": "browsai-agent-tree/1.0",
        "transports": ["http+json", "http+chunked-ndjson"],
        "endpoints": [
            {"method": "GET", "path": "/health", "returns": "BrowserHealth"},
            {"method": "GET", "path": "/capabilities", "returns": "EngineCapabilities"},
            {"method": "GET", "path": "/version", "returns": "string"},
            {"method": "GET", "path": "/schema", "returns": "Schema"},
            {"method": "POST", "path": "/browse", "returns": "NavigationResult|stream"},
            {"method": "POST", "path": "/query", "returns": "QueryResult|stream"},
            {"method": "POST", "path": "/render", "returns": "RenderResult|stream"},
            {"method": "POST", "path": "/follow-link", "returns": "FollowLinkResult"},
            {"method": "POST", "path": "/auto-solve", "returns": "LiveResult|stream"}
        ],
        "streaming_query_param": "stream=true",
        "doc": "docs/agent-tree-schema.md"
    });
    Response::json(200, serde_json::to_vec_pretty(&payload).unwrap_or_default())
}

fn invoke_cli_blocking(args: Vec<String>) -> Response {
    let binary = browse_binary();
    let output = match Command::new(&binary).args(&args).output() {
        Ok(output) => output,
        Err(error) => {
            let payload = serde_json::json!({
                "error": format!("failed to spawn {binary:?}: {error}")
            });
            return Response::json(500, serde_json::to_vec(&payload).unwrap_or_default());
        }
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let payload = serde_json::json!({
            "error": format!("browsai exited {:?}", output.status.code()),
            "stderr": stderr.trim()
        });
        return Response::json(500, serde_json::to_vec(&payload).unwrap_or_default());
    }
    let stdout = output.stdout;
    if looks_like_json(&stdout) {
        Response::json(200, stdout)
    } else {
        Response::text(200, "text/plain; charset=utf-8", stdout)
    }
}

fn looks_like_json(bytes: &[u8]) -> bool {
    for byte in bytes.iter().take(64) {
        if *byte == b'{' || *byte == b'[' {
            return true;
        }
        if !byte.is_ascii_whitespace() {
            return false;
        }
    }
    false
}

fn handle_browse(body: &Value, query: &std::collections::HashMap<String, String>) -> Response {
    let url = match body.get("url").and_then(Value::as_str) {
        Some(s) => s.to_string(),
        None => {
            return Response::json(400, b"{\"error\":\"missing url\"}".to_vec());
        }
    };
    let wants_stream = query.get("stream").map(String::as_str) == Some("true")
        || body.get("stream").and_then(Value::as_bool) == Some(true);
    let mut args = vec!["navigate".to_string(), url];
    if wants_stream {
        args.push("--stream".to_string());
    }
    push_optional_flag(&mut args, "--fingerprint", body.get("fingerprint"));
    if body.get("snapshot_only").and_then(Value::as_bool) == Some(true) {
        args.push("--snapshot-only".into());
    }
    if body.get("auto_solve").and_then(Value::as_bool) == Some(true) {
        args.push("--auto-solve".into());
    }
    if let Some(query_text) = body.get("query").and_then(Value::as_str) {
        push_optional_flag(
            &mut args,
            "--query",
            Some(&Value::String(query_text.to_string())),
        );
    }
    if wants_stream {
        invoke_cli_streaming(args)
    } else {
        invoke_cli_blocking(args)
    }
}

fn handle_query(body: &Value, query: &std::collections::HashMap<String, String>) -> Response {
    handle_query_or_render(body, query, "query")
}

fn handle_render(body: &Value, query: &std::collections::HashMap<String, String>) -> Response {
    handle_query_or_render(body, query, "render")
}

fn handle_query_or_render(
    body: &Value,
    query: &std::collections::HashMap<String, String>,
    command: &str,
) -> Response {
    let url = match body.get("url").and_then(Value::as_str) {
        Some(s) => s.to_string(),
        None => {
            return Response::json(400, b"{\"error\":\"missing url\"}".to_vec());
        }
    };
    let wants_stream = query.get("stream").map(String::as_str) == Some("true")
        || body.get("stream").and_then(Value::as_bool) == Some(true);
    let mut args = vec![command.to_string(), url];
    if wants_stream {
        args.push("--stream".to_string());
    }
    push_optional_flag(&mut args, "--query", body.get("query"));
    push_optional_flag(&mut args, "--filter", body.get("filter"));
    if let Some(cursor) = body.get("cursor").and_then(Value::as_u64) {
        args.push(format!("--cursor={cursor}"));
    }
    if let Some(limit) = body.get("limit").and_then(Value::as_u64) {
        args.push(format!("--limit={limit}"));
    }
    if wants_stream {
        invoke_cli_streaming(args)
    } else {
        invoke_cli_blocking(args)
    }
}

fn handle_follow_link(body: &Value, query: &std::collections::HashMap<String, String>) -> Response {
    let page = match body.get("page").and_then(Value::as_u64) {
        Some(value) => value,
        None => {
            return Response::json(400, b"{\"error\":\"missing page\"}".to_vec());
        }
    };
    let node_id = match body.get("node_id").and_then(Value::as_str) {
        Some(value) => value.to_string(),
        None => {
            return Response::json(400, b"{\"error\":\"missing node_id\"}".to_vec());
        }
    };
    let mut args = vec!["follow-link".to_string(), page.to_string(), node_id];
    if query.get("stream").map(String::as_str) == Some("true")
        || body.get("stream").and_then(Value::as_bool) == Some(true)
    {
        args.push("--stream".to_string());
    }
    push_optional_flag(&mut args, "--fingerprint", body.get("fingerprint"));
    if query.get("stream").map(String::as_str) == Some("true")
        || body.get("stream").and_then(Value::as_bool) == Some(true)
    {
        invoke_cli_streaming(args)
    } else {
        invoke_cli_blocking(args)
    }
}

fn handle_auto_solve(body: &Value, query: &std::collections::HashMap<String, String>) -> Response {
    let url = match body.get("url").and_then(Value::as_str) {
        Some(s) => s.to_string(),
        None => {
            return Response::json(400, b"{\"error\":\"missing url\"}".to_vec());
        }
    };
    let wants_stream = query.get("stream").map(String::as_str) == Some("true")
        || body.get("stream").and_then(Value::as_bool) == Some(true);
    let mut args = vec!["live-open".to_string(), url, "--auto-solve".to_string()];
    if wants_stream {
        args.push("--stream".to_string());
    }
    push_optional_flag(&mut args, "--fingerprint", body.get("fingerprint"));
    if wants_stream {
        invoke_cli_streaming(args)
    } else {
        invoke_cli_blocking(args)
    }
}

fn push_optional_flag(args: &mut Vec<String>, flag: &str, value: Option<&Value>) {
    let Some(value) = value else { return };
    let Some(s) = value.as_str() else { return };
    if s.is_empty() {
        return;
    }
    args.push(format!("{flag}={s}"));
}

fn invoke_cli_streaming(args: Vec<String>) -> Response {
    let binary = browse_binary();
    let mut command = Command::new(&binary);
    command
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            let payload = serde_json::json!({
                "error": format!("failed to spawn {binary:?}: {error}")
            });
            return Response::json(500, serde_json::to_vec(&payload).unwrap_or_default());
        }
    };
    let stdout = child.stdout.take().expect("child stdout piped");
    let stderr = child.stderr.take().expect("child stderr piped");
    let (line_tx, line_rx) = mpsc::channel::<String>();
    let binary_label = binary.to_string_lossy().into_owned();
    let line_sender = line_tx.clone();
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut buffer = String::new();
        loop {
            buffer.clear();
            match reader.read_line(&mut buffer) {
                Ok(0) => break,
                Ok(_) => {}
                Err(error) => {
                    eprintln!("BROWSAI_SERVER: stdout read error: {error}");
                    break;
                }
            }
            let trimmed = buffer.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                continue;
            }
            if line_sender.send(trimmed.to_string()).is_err() {
                break;
            }
        }
    });
    drop(line_tx);
    thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut buffer = vec![0u8; 4096];
        let mut accumulated = String::new();
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    accumulated.push_str(&String::from_utf8_lossy(&buffer[..n]));
                }
                Err(_) => break,
            }
            if accumulated.len() > 16384 {
                break;
            }
        }
        if !accumulated.trim().is_empty() {
            eprintln!("BROWSAI_SERVER: {binary_label} stderr: {accumulated}");
        }
    });
    // Spawn a waiter thread to keep the child off the zombie pile.
    thread::spawn(move || {
        let _ = child.wait();
    });
    Response::ndjson_streaming(
        200,
        line_rx,
        std::sync::Arc::new(std::sync::Mutex::new(false)),
    )
}

fn write_response(stream: &mut TcpStream, response: Response) -> std::io::Result<()> {
    match response.body {
        ResponseBody::Static(bytes) => write_static_response(
            stream,
            response.status,
            response.status_text,
            response.content_type,
            &bytes,
        ),
        ResponseBody::Streaming { lines, binary } => write_streaming_response(
            stream,
            response.status,
            response.status_text,
            &lines,
            &binary,
        ),
    }
}

fn write_static_response(
    stream: &mut TcpStream,
    status: u16,
    status_text: &str,
    content_type: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let header = format!(
        "HTTP/1.1 {status} {status_text}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()?;
    Ok(())
}

fn write_streaming_response(
    stream: &mut TcpStream,
    status: u16,
    status_text: &str,
    lines: &mpsc::Receiver<String>,
    _binary: &std::sync::Arc<std::sync::Mutex<bool>>,
) -> std::io::Result<()> {
    let header = format!(
        "HTTP/1.1 {status} {status_text}\r\n\
         Content-Type: application/x-ndjson\r\n\
         Transfer-Encoding: chunked\r\n\
         Connection: close\r\n\
         \r\n"
    );
    stream.write_all(header.as_bytes())?;
    stream.flush()?;
    let mut received_terminator = false;
    while let Ok(line) = lines.recv_timeout(Duration::from_millis(2000)) {
        if line.contains("\"type\":\"snapshot-complete\"") {
            write_chunk(stream, line.as_bytes())?;
            received_terminator = true;
            break;
        }
        if line.contains("\"type\":\"process-exit\"") {
            write_chunk(stream, line.as_bytes())?;
            continue;
        }
        write_chunk(stream, line.as_bytes())?;
    }
    if !received_terminator {
        while let Ok(line) = lines.recv_timeout(Duration::from_millis(500)) {
            write_chunk(stream, line.as_bytes())?;
        }
    }
    write_chunk(stream, b"")?;
    stream.flush()?;
    let _ = stream.shutdown(std::net::Shutdown::Both);
    Ok(())
}

fn write_chunk(stream: &mut TcpStream, data: &[u8]) -> std::io::Result<()> {
    let size_line = format!("{:x}\r\n", data.len());
    stream.write_all(size_line.as_bytes())?;
    stream.write_all(data)?;
    stream.write_all(b"\r\n")?;
    Ok(())
}

fn write_http_error(stream: &mut TcpStream, status: u16, message: String) -> std::io::Result<()> {
    let body = serde_json::to_vec(&serde_json::json!({"error": message})).unwrap_or_default();
    write_static_response(
        stream,
        status,
        status_text(status),
        "application/json",
        &body,
    )
}

pub fn browse_binary() -> PathBuf {
    if let Ok(path) = std::env::var("BROWSAI_BIN") {
        return PathBuf::from(path);
    }
    if let Ok(exe) = std::env::current_exe() {
        return exe;
    }
    PathBuf::from("browsai")
}
