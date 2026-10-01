//! Local control API: how the CLI and the web control panel reach the running daemon.
//!
//! The daemon serves HTTP on `127.0.0.1:<port>` (default 27311):
//! - `GET /` — the control panel page (static, contains no secrets);
//! - `POST /api/command` — a JSON [`Command`], answered with `{"ok":true,"data":...}` or
//!   `{"ok":false,"error":"..."}`.
//!
//! Every API call must carry `Authorization: Bearer <token>`. The token is random per daemon
//! start and stored with the port in `$XDG_RUNTIME_DIR/ssgg/control.json` (mode 0600), which
//! only the user can read. The `Host` header must name the loopback address, which defeats DNS
//! rebinding; no CORS headers are sent, so other web pages cannot read responses.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

use super::{Command, Engine};
use crate::{Error, Result};

const CONTROL_FILE: &str = "control.json";
#[cfg(unix)]
const LOCK_FILE: &str = "daemon.lock";
/// How long a one-time panel ticket stays valid.
const TICKET_LIFETIME: Duration = Duration::from_secs(60);
const PANEL_HTML: &str = include_str!("panel.html");
/// Largest request body accepted (a full profile with bindings fits easily).
const MAX_BODY: usize = 1024 * 1024;

/// Where to reach the daemon, written by the daemon and read by clients.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ControlInfo {
    pub port: u16,
    pub token: String,
    pub pid: u32,
}

impl ControlInfo {
    fn dir() -> Result<PathBuf> {
        if let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()) {
            return Ok(PathBuf::from(runtime).join("ssgg"));
        }
        crate::config::Config::config_dir()
            .map(|d| d.join("run"))
            .ok_or_else(|| Error::InvalidConfig("could not determine a runtime directory".to_string()))
    }

    pub fn path() -> Result<PathBuf> {
        Ok(Self::dir()?.join(CONTROL_FILE))
    }

    /// The control file of a running daemon, if any.
    pub fn read() -> Option<Self> {
        let path = Self::path().ok()?;
        let text = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&text).ok()
    }

    fn write(&self) -> Result<()> {
        let dir = Self::dir()?;
        std::fs::create_dir_all(&dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
        }
        crate::fs_utils::secure_write(dir.join(CONTROL_FILE), serde_json::to_vec(self)?)
    }

    /// Remove the control file if it still belongs to this process.
    pub fn remove_if_ours(&self) {
        if let Some(current) = Self::read()
            && current.pid == self.pid
            && current.token == self.token
            && let Ok(path) = Self::path()
        {
            let _ = std::fs::remove_file(path);
        }
    }

    /// URL that opens the control panel already authorised. The token travels in the URL
    /// fragment, which browsers never send to the server or put in `Referer`.
    pub fn panel_url(&self) -> String {
        format!("http://127.0.0.1:{}/#token={}", self.port, self.token)
    }

    /// URL carrying a one-time ticket instead of the token. Program arguments are visible to
    /// other local users, so `ssgg ui` passes this; the page trades it for the token once.
    pub fn ticket_url(&self, ticket: &str) -> String {
        format!("http://127.0.0.1:{}/#ticket={ticket}", self.port)
    }
}

/// Held by the running daemon for its whole life, so a second daemon cannot start and a CLI
/// can tell "no daemon" from "daemon running but unreachable".
pub struct DaemonLock {
    #[cfg(unix)]
    _file: std::fs::File,
}

impl DaemonLock {
    /// Take the lock; `Ok(None)` when another daemon holds it.
    pub fn acquire() -> Result<Option<Self>> {
        #[cfg(unix)]
        {
            let dir = ControlInfo::dir()?;
            std::fs::create_dir_all(&dir)?;
            let file = std::fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .write(true)
                .open(dir.join(LOCK_FILE))?;
            match rustix::fs::flock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive) {
                Ok(()) => Ok(Some(Self { _file: file })),
                Err(rustix::io::Errno::WOULDBLOCK) => Ok(None),
                Err(e) => Err(Error::Io(e.into())),
            }
        }
        #[cfg(not(unix))]
        {
            Ok(Some(Self {}))
        }
    }

    /// Whether some daemon currently holds the lock.
    pub fn is_held() -> bool {
        #[cfg(unix)]
        {
            matches!(Self::acquire(), Ok(None))
        }
        #[cfg(not(unix))]
        {
            false
        }
    }
}

fn new_token() -> Result<String> {
    let mut bytes = [0u8; 24];
    getrandom::fill(&mut bytes).map_err(|e| Error::Other(format!("no OS randomness: {e}")))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

#[derive(Clone)]
struct ServerState {
    engine: Arc<Engine>,
    token: Arc<String>,
    port: u16,
    tickets: Arc<parking_lot::Mutex<Vec<(String, Instant)>>>,
}

/// A running control server.
pub struct ControlServer {
    pub info: ControlInfo,
    task: JoinHandle<()>,
}

impl ControlServer {
    /// Bind `127.0.0.1:port` (trying the next few ports if it is taken), write the control
    /// file and start serving.
    pub async fn start(engine: Arc<Engine>, port: u16) -> Result<Self> {
        let mut last_error = None;
        let mut bound = None;
        for candidate in port..port.saturating_add(10) {
            match TcpListener::bind(("127.0.0.1", candidate)).await {
                Ok(listener) => {
                    bound = Some((listener, candidate));
                    break;
                }
                Err(e) => last_error = Some(e),
            }
        }
        let Some((listener, port)) = bound else {
            return Err(Error::Other(format!(
                "control API could not bind 127.0.0.1:{port}..{}: {}",
                port.saturating_add(9),
                last_error.map(|e| e.to_string()).unwrap_or_default()
            )));
        };

        let info = ControlInfo {
            port,
            token: new_token()?,
            pid: std::process::id(),
        };
        info.write()?;

        let state = ServerState {
            engine,
            token: Arc::new(info.token.clone()),
            port,
            tickets: Arc::new(parking_lot::Mutex::new(Vec::new())),
        };
        let app = Router::new()
            .route("/", get(panel))
            .route("/api/command", post(command))
            .route("/api/ticket", post(issue_ticket_route))
            .route("/api/redeem", post(redeem_ticket_route))
            .fallback(not_found)
            .with_state(state);

        let task = tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                tracing::error!("Control API stopped: {e}");
            }
        });
        tracing::info!("Control panel: http://127.0.0.1:{port}/ (open it with `ssgg ui`)");
        Ok(Self { info, task })
    }

    pub fn stop(self) {
        self.task.abort();
        self.info.remove_if_ours();
    }
}

fn host_is_loopback(headers: &HeaderMap, port: u16) -> bool {
    let Some(host) = headers.get(header::HOST).and_then(|h| h.to_str().ok()) else {
        return false;
    };
    [
        format!("127.0.0.1:{port}"),
        format!("localhost:{port}"),
        format!("[::1]:{port}"),
    ]
    .iter()
    .any(|allowed| host.eq_ignore_ascii_case(allowed))
}

fn token_matches(headers: &HeaderMap, token: &str) -> bool {
    let Some(given) = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
    else {
        return false;
    };
    // Constant-time comparison: do not leak how many leading characters matched.
    given.len() == token.len() && given.bytes().zip(token.bytes()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
}

fn reply(status: StatusCode, body: Value) -> Response {
    let mut response = (status, axum::Json(body)).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, header::HeaderValue::from_static("no-store"));
    response
}

async fn panel(State(state): State<ServerState>, headers: HeaderMap) -> Response {
    if !host_is_loopback(&headers, state.port) {
        return reply(StatusCode::FORBIDDEN, json!({"ok": false, "error": "bad host"}));
    }
    let mut response = Html(PANEL_HTML).into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        header::HeaderValue::from_static(
            "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; \
             connect-src 'self'; img-src 'self' data:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
        ),
    );
    headers.insert(header::REFERRER_POLICY, header::HeaderValue::from_static("no-referrer"));
    headers.insert(header::X_FRAME_OPTIONS, header::HeaderValue::from_static("DENY"));
    response
}

async fn command(State(state): State<ServerState>, headers: HeaderMap, body: Bytes) -> Response {
    if !host_is_loopback(&headers, state.port) {
        return reply(StatusCode::FORBIDDEN, json!({"ok": false, "error": "bad host"}));
    }
    if !token_matches(&headers, &state.token) {
        return reply(
            StatusCode::UNAUTHORIZED,
            json!({"ok": false, "error": "missing or wrong token"}),
        );
    }
    if body.len() > MAX_BODY {
        return reply(
            StatusCode::PAYLOAD_TOO_LARGE,
            json!({"ok": false, "error": "request too large"}),
        );
    }
    let command: Command = match serde_json::from_slice(&body) {
        Ok(c) => c,
        Err(e) => {
            return reply(
                StatusCode::BAD_REQUEST,
                json!({"ok": false, "error": format!("invalid command: {e}")}),
            );
        }
    };
    match state.engine.execute(command).await {
        Ok(data) => reply(StatusCode::OK, json!({"ok": true, "data": data})),
        Err(e) => reply(StatusCode::OK, json!({"ok": false, "error": e.to_string()})),
    }
}

/// Authenticated: create a one-time ticket the browser can trade for the token.
async fn issue_ticket_route(State(state): State<ServerState>, headers: HeaderMap) -> Response {
    if !host_is_loopback(&headers, state.port) {
        return reply(StatusCode::FORBIDDEN, json!({"ok": false, "error": "bad host"}));
    }
    if !token_matches(&headers, &state.token) {
        return reply(
            StatusCode::UNAUTHORIZED,
            json!({"ok": false, "error": "missing or wrong token"}),
        );
    }
    let ticket = match new_token() {
        Ok(t) => t,
        Err(e) => {
            return reply(
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({"ok": false, "error": e.to_string()}),
            );
        }
    };
    let now = Instant::now();
    let mut tickets = state.tickets.lock();
    tickets.retain(|(_, expires)| *expires > now);
    tickets.push((ticket.clone(), now + TICKET_LIFETIME));
    reply(StatusCode::OK, json!({"ok": true, "data": {"ticket": ticket}}))
}

/// Trade a valid, unused ticket for the token (single use, 60 s).
async fn redeem_ticket_route(State(state): State<ServerState>, headers: HeaderMap, body: Bytes) -> Response {
    if !host_is_loopback(&headers, state.port) {
        return reply(StatusCode::FORBIDDEN, json!({"ok": false, "error": "bad host"}));
    }
    let given = serde_json::from_slice::<Value>(&body)
        .ok()
        .and_then(|v| v["ticket"].as_str().map(str::to_string))
        .unwrap_or_default();
    let now = Instant::now();
    let mut tickets = state.tickets.lock();
    tickets.retain(|(_, expires)| *expires > now);
    let found = tickets.iter().position(|(t, _)| {
        t.len() == given.len() && t.bytes().zip(given.bytes()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
    });
    match found {
        Some(index) => {
            tickets.remove(index);
            reply(
                StatusCode::OK,
                json!({"ok": true, "data": {"token": state.token.as_str()}}),
            )
        }
        None => reply(
            StatusCode::FORBIDDEN,
            json!({"ok": false, "error": "ticket expired or already used"}),
        ),
    }
}

async fn not_found() -> Response {
    reply(StatusCode::NOT_FOUND, json!({"ok": false, "error": "not found"}))
}

/// Send `command` to the running daemon.
///
/// Returns `Ok(None)` when no daemon is reachable (no control file, stale file, connection
/// refused), so the caller can fall back to an in-process engine.
pub async fn send(command: &Command) -> Result<Option<Value>> {
    let Some(info) = ControlInfo::read() else {
        return Ok(None);
    };
    let stream =
        match tokio::time::timeout(Duration::from_millis(800), TcpStream::connect(("127.0.0.1", info.port))).await {
            Ok(Ok(stream)) => stream,
            _ => return Ok(None),
        };
    let body = serde_json::to_vec(command)?;
    let (status, body) =
        match tokio::time::timeout(Duration::from_secs(60), exchange(stream, &info, "/api/command", &body)).await {
            Ok(result) => result?,
            Err(_) => return Err(Error::Other("the daemon did not answer within 60 s".to_string())),
        };
    if status == 401 {
        // Another process holds the port, or the file belongs to a dead daemon.
        tracing::debug!("Control file is stale (401); falling back to direct device access");
        return Ok(None);
    }
    let parsed: Value = serde_json::from_slice(&body)
        .map_err(|e| Error::Other(format!("unreadable reply from daemon (HTTP {status}): {e}")))?;
    if parsed["ok"].as_bool() == Some(true) {
        Ok(Some(parsed["data"].clone()))
    } else {
        Err(Error::Other(
            parsed["error"]
                .as_str()
                .unwrap_or("daemon reported an error")
                .to_string(),
        ))
    }
}

/// Ask the running daemon for a one-time panel ticket. `Ok(None)` when no daemon answers.
pub async fn issue_ticket() -> Result<Option<(ControlInfo, String)>> {
    let Some(info) = ControlInfo::read() else {
        return Ok(None);
    };
    let Ok(Ok(stream)) =
        tokio::time::timeout(Duration::from_millis(800), TcpStream::connect(("127.0.0.1", info.port))).await
    else {
        return Ok(None);
    };
    let (status, body) = tokio::time::timeout(Duration::from_secs(10), exchange(stream, &info, "/api/ticket", b"{}"))
        .await
        .map_err(|_| Error::Other("the daemon did not answer".to_string()))??;
    if status != 200 {
        return Ok(None);
    }
    let parsed: Value = serde_json::from_slice(&body)?;
    Ok(parsed["data"]["ticket"].as_str().map(|t| (info.clone(), t.to_string())))
}

async fn exchange(mut stream: TcpStream, info: &ControlInfo, path: &str, body: &[u8]) -> Result<(u16, Vec<u8>)> {
    let head = format!(
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: Bearer {}\r\n\
         Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        info.port,
        info.token,
        body.len()
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(body).await?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).await?;
    parse_http_response(&raw)
}

/// Split a complete HTTP/1.1 response into status code and body (Content-Length or chunked).
fn parse_http_response(raw: &[u8]) -> Result<(u16, Vec<u8>)> {
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| Error::Other("malformed HTTP response".to_string()))?;
    let head = String::from_utf8_lossy(&raw[..split]);
    let mut body = raw[split + 4..].to_vec();
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse().ok())
        .ok_or_else(|| Error::Other("malformed HTTP status line".to_string()))?;
    let chunked = head.lines().any(|line| {
        let lower = line.to_ascii_lowercase();
        lower.starts_with("transfer-encoding:") && lower.contains("chunked")
    });
    if chunked {
        body = dechunk(&body)?;
    }
    Ok((status, body))
}

fn dechunk(mut data: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let line_end = data
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or_else(|| Error::Other("malformed chunked body".to_string()))?;
        let size_text = String::from_utf8_lossy(&data[..line_end]);
        let size = usize::from_str_radix(size_text.split(';').next().unwrap_or("").trim(), 16)
            .map_err(|_| Error::Other("malformed chunk size".to_string()))?;
        data = &data[line_end + 2..];
        if size == 0 {
            return Ok(out);
        }
        let needed = size
            .checked_add(2)
            .ok_or_else(|| Error::Other("malformed chunk size".to_string()))?;
        if data.len() < needed {
            return Err(Error::Other("truncated chunked body".to_string()));
        }
        out.extend_from_slice(&data[..size]);
        data = &data[size + 2..];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn host_check_accepts_only_loopback_names_with_the_right_port() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("127.0.0.1:27311"));
        assert!(host_is_loopback(&headers, 27311));
        assert!(!host_is_loopback(&headers, 27312));
        headers.insert(header::HOST, HeaderValue::from_static("localhost:27311"));
        assert!(host_is_loopback(&headers, 27311));
        headers.insert(header::HOST, HeaderValue::from_static("evil.example:27311"));
        assert!(!host_is_loopback(&headers, 27311));
        assert!(!host_is_loopback(&HeaderMap::new(), 27311));
    }

    #[test]
    fn token_check_requires_exact_bearer_token() {
        let mut headers = HeaderMap::new();
        assert!(!token_matches(&headers, "abc"));
        headers.insert(header::AUTHORIZATION, HeaderValue::from_static("Bearer abc"));
        assert!(token_matches(&headers, "abc"));
        assert!(!token_matches(&headers, "abd"));
        assert!(!token_matches(&headers, "abcd"));
        headers.insert(header::AUTHORIZATION, HeaderValue::from_static("abc"));
        assert!(!token_matches(&headers, "abc"));
    }

    #[test]
    fn tokens_are_long_and_distinct() {
        let a = new_token().unwrap();
        let b = new_token().unwrap();
        assert_eq!(a.len(), 48);
        assert_ne!(a, b);
    }

    #[test]
    fn parses_content_length_and_chunked_responses() {
        let raw = b"HTTP/1.1 200 OK\r\ncontent-length: 11\r\n\r\n{\"ok\":true}";
        let (status, body) = parse_http_response(raw).unwrap();
        assert_eq!(status, 200);
        assert_eq!(body, b"{\"ok\":true}");

        let raw =
            b"HTTP/1.1 401 Unauthorized\r\ntransfer-encoding: chunked\r\n\r\n4\r\n{\"ok\r\n7\r\n\":true}\r\n0\r\n\r\n";
        let (status, body) = parse_http_response(raw).unwrap();
        assert_eq!(status, 401);
        assert_eq!(body, b"{\"ok\":true}");
    }

    #[test]
    fn panel_url_keeps_the_token_out_of_the_request() {
        let info = ControlInfo {
            port: 27311,
            token: "t0k".into(),
            pid: 1,
        };
        assert_eq!(info.panel_url(), "http://127.0.0.1:27311/#token=t0k");
    }
}
