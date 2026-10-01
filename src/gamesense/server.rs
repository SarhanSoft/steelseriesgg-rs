//! GameSense HTTP server implementation.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{
    Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use parking_lot::Mutex;
use serde_json::{Map, Value, json};
use tokio::sync::broadcast;
use tokio::task::AbortHandle;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing::{debug, info, trace, warn};

use super::core_props::{CorePropsGuard, write_core_props};
use super::eval::{self, EventInput, ResolvedFrame, ScreenPlan};
use super::handlers::{HandlerKind, HandlerSpec, Repeats, json_to_i64, parse_handler};
use super::output::{DeviceType, GameSenseOutput, LightTarget};
#[cfg(test)]
use super::*;
use crate::rgb::Color;
use crate::{Error, Result};

/// Idle time after which a game is deactivated, unless its metadata sets
/// `deinitialize_timer_length_ms`.
pub const DEFAULT_HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(15);

/// `deinitialize_timer_length_ms` bounds from the SDK.
const MIN_GAME_TIMEOUT_MS: i64 = 1_000;
const MAX_GAME_TIMEOUT_MS: i64 = 60_000;

/// Outputs buffered per subscriber before a slow one starts losing the oldest.
const OUTPUT_CHANNEL_CAPACITY: usize = 1024;
/// Largest request body accepted.
const MAX_BODY_BYTES: usize = 1024 * 1024;
/// Anti-spam limits (SDK error 5).
const MAX_GAMES: usize = 64;
const MAX_EVENTS_PER_GAME: usize = 256;
const MAX_HANDLERS_PER_EVENT: usize = 64;
const MAX_EVENTS_PER_BATCH: usize = 256;
/// Longest game or event name accepted.
const MAX_NAME_LEN: usize = 128;
/// Longest display name or developer kept.
const MAX_TEXT_FIELD_CHARS: usize = 256;
/// Shortest sleep between screen frames, so a 0-1 ms frame list cannot spin.
const MIN_SCREEN_FRAME: Duration = Duration::from_millis(10);

/// Error answered to a client, with the SDK's numeric code where one applies.
#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    code: Option<u8>,
    message: String,
}

impl ApiError {
    /// One of the errors listed in "Writing Handlers in JSON", answered with HTTP 400.
    fn sdk(code: u8) -> Self {
        let message = match code {
            0 => "Game or event string not specified",
            1 => "Game string not specified",
            2 => {
                "Game or event string contains disallowed characters.  Allowed are upper-case A-Z, 0-9, hyphen, and underscore"
            }
            3 => "Game string contains disallowed characters.  Allowed are upper-case A-Z, 0-9, hyphen, and underscore",
            4 => "GameEvent data member is empty",
            5 => "Events for too many games have been registered recently, please try again later",
            6 => "One or more handlers must be specified for binding",
            9 => "That event is not registered",
            10 => "That game is not registered",
            _ => "Bad request",
        };
        Self {
            status: StatusCode::BAD_REQUEST,
            code: Some(code),
            message: message.to_string(),
        }
    }

    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            code: None,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = match self.code {
            Some(code) => json!({ "error": self.message, "code": code }),
            None => json!({ "error": self.message }),
        };
        (self.status, Json(body)).into_response()
    }
}

type ApiResult = std::result::Result<Value, ApiError>;

/// Per-event registration and handlers.
#[derive(Debug)]
struct EventState {
    min_value: i64,
    max_value: i64,
    icon_id: u32,
    value_optional: bool,
    handlers: Vec<HandlerSpec>,
    /// Last value processed, for the SDK's "only on change" rule. Reset on deactivation.
    last_value: Option<i64>,
}

impl EventState {
    fn new() -> Self {
        Self {
            min_value: 0,
            max_value: 100,
            icon_id: 0,
            value_optional: false,
            handlers: Vec::new(),
            last_value: None,
        }
    }
}

/// Event metadata shared by `register_game_event` and `bind_game_event`.
struct EventMeta {
    min_value: i64,
    max_value: i64,
    icon_id: u32,
    value_optional: bool,
}

impl EventMeta {
    fn from_request(map: &Map<String, Value>) -> Self {
        Self {
            min_value: map.get("min_value").and_then(json_to_i64).unwrap_or(0),
            max_value: map.get("max_value").and_then(json_to_i64).unwrap_or(100),
            icon_id: map
                .get("icon_id")
                .and_then(json_to_i64)
                .map_or(0, |id| id.clamp(0, i64::from(u32::MAX)) as u32),
            value_optional: map.get("value_optional").and_then(Value::as_bool).unwrap_or(false),
        }
    }

    fn apply(&self, event: &mut EventState) {
        event.min_value = self.min_value;
        event.max_value = self.max_value;
        event.icon_id = self.icon_id;
        event.value_optional = self.value_optional;
        event.last_value = None;
    }

    fn to_json(&self, game: &str, event: &str) -> Map<String, Value> {
        let mut out = Map::new();
        out.insert("game".into(), json!(game));
        out.insert("event".into(), json!(event));
        out.insert("min_value".into(), json!(self.min_value));
        out.insert("max_value".into(), json!(self.max_value));
        out.insert("icon_id".into(), json!(self.icon_id));
        out.insert("value_optional".into(), json!(self.value_optional));
        out
    }
}

#[derive(Debug)]
struct GameState {
    display_name: Option<String>,
    developer: Option<String>,
    deinit_timeout: Option<Duration>,
    events: HashMap<String, EventState>,
    /// True between the first event and `stop_game` or the idle timeout.
    active: bool,
    last_activity: Instant,
}

impl GameState {
    fn new(now: Instant) -> Self {
        Self {
            display_name: None,
            developer: None,
            deinit_timeout: None,
            events: HashMap::new(),
            active: false,
            last_activity: now,
        }
    }

    fn event_entry(&mut self, name: &str) -> std::result::Result<&mut EventState, ApiError> {
        let count = self.events.len();
        match self.events.entry(name.to_string()) {
            Entry::Occupied(e) => Ok(e.into_mut()),
            Entry::Vacant(e) => {
                if count >= MAX_EVENTS_PER_GAME {
                    return Err(ApiError {
                        message: format!("A game may register at most {MAX_EVENTS_PER_GAME} events"),
                        ..ApiError::sdk(5)
                    });
                }
                Ok(e.insert(EventState::new()))
            }
        }
    }

    /// Mark inactive and forget cached values, so the next event shows again. Returns whether
    /// the game was active.
    fn deactivate(&mut self) -> bool {
        let was_active = self.active;
        self.active = false;
        for event in self.events.values_mut() {
            event.last_value = None;
        }
        was_active
    }

    fn timeout(&self, default: Duration) -> Duration {
        self.deinit_timeout.unwrap_or(default)
    }
}

#[derive(Debug)]
struct ServerState {
    games: HashMap<String, GameState>,
    default_timeout: Duration,
    write_core_props: bool,
}

impl ServerState {
    fn game_entry(&mut self, name: &str, now: Instant) -> std::result::Result<&mut GameState, ApiError> {
        let count = self.games.len();
        match self.games.entry(name.to_string()) {
            Entry::Occupied(e) => Ok(e.into_mut()),
            Entry::Vacant(e) => {
                if count >= MAX_GAMES {
                    return Err(ApiError::sdk(5));
                }
                info!("GameSense: registered game {}", name);
                Ok(e.insert(GameState::new(now)))
            }
        }
    }
}

/// Key of a running screen sequence: game, device type, zone.
type ScreenKey = (String, String, String);

/// A screen plan waiting to be shown.
struct ScreenJob {
    key: ScreenKey,
    game: String,
    event: String,
    device_type: DeviceType,
    zone: String,
    plan: ScreenPlan,
}

/// Outputs produced while the state lock was held, dispatched after it is released.
#[derive(Default)]
struct Batch {
    outputs: Vec<GameSenseOutput>,
    screens: Vec<ScreenJob>,
}

struct Inner {
    state: Mutex<ServerState>,
    tx: broadcast::Sender<GameSenseOutput>,
    screen_tasks: Mutex<HashMap<ScreenKey, AbortHandle>>,
    legacy_task: Mutex<Option<AbortHandle>>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        for (_, task) in self.screen_tasks.get_mut().drain() {
            task.abort();
        }
        if let Some(task) = self.legacy_task.get_mut().take() {
            task.abort();
        }
    }
}

fn send_output(tx: &broadcast::Sender<GameSenseOutput>, output: GameSenseOutput) {
    trace!("GameSense output: {:?}", output);
    if tx.send(output).is_err() {
        trace!("GameSense: no subscribers for output");
    }
}

impl Inner {
    fn emit(&self, outputs: Vec<GameSenseOutput>) {
        for output in outputs {
            send_output(&self.tx, output);
        }
    }

    fn dispatch(&self, batch: Batch) {
        self.emit(batch.outputs);
        for job in batch.screens {
            self.schedule_screen(job);
        }
    }

    /// Show a screen plan, replacing any sequence still playing on the same screen.
    fn schedule_screen(&self, job: ScreenJob) {
        let plan = &job.plan;
        let needs_task = plan.frames.len() > 1
            || (plan.repeats != Repeats::Once && plan.frames.iter().all(|f| f.duration.is_some()));
        let mut tasks = self.screen_tasks.lock();
        if let Some(previous) = tasks.remove(&job.key) {
            previous.abort();
        }
        if needs_task {
            let key = job.key.clone();
            let task = tokio::spawn(play_screen(self.tx.clone(), job));
            tasks.insert(key, task.abort_handle());
        } else {
            drop(tasks);
            if let Some(frame) = job.plan.frames.first() {
                self.emit(screen_outputs(&job, frame));
            }
        }
    }

    fn abort_screens(&self, game: &str) {
        self.screen_tasks.lock().retain(|key, task| {
            if key.0 == game {
                task.abort();
                false
            } else {
                true
            }
        });
    }

    /// Clear outputs of games that were stopped, timed out or removed.
    fn clear_games(&self, games: Vec<String>) {
        for game in games {
            self.abort_screens(&game);
            send_output(&self.tx, GameSenseOutput::Clear { game });
        }
    }

    /// Deactivate games idle longer than their timeout.
    fn expire_idle_games(&self, now: Instant) {
        let expired: Vec<String> = {
            let mut state = self.state.lock();
            let default = state.default_timeout;
            state
                .games
                .iter_mut()
                .filter(|(_, g)| g.active && now.saturating_duration_since(g.last_activity) >= g.timeout(default))
                .map(|(name, g)| {
                    g.deactivate();
                    info!("GameSense: {} timed out; restoring default lighting", name);
                    name.clone()
                })
                .collect()
        };
        self.clear_games(expired);
    }

    fn watchdog_tick(&self) -> Duration {
        let timeout = self.state.lock().default_timeout;
        (timeout / 4).clamp(Duration::from_millis(10), Duration::from_millis(250))
    }

    /// Deactivate every game (server shutdown).
    fn shutdown_games(&self) {
        let active: Vec<String> = {
            let mut state = self.state.lock();
            state
                .games
                .iter_mut()
                .filter_map(|(name, g)| g.deactivate().then(|| name.clone()))
                .collect()
        };
        self.clear_games(active);
        for (_, task) in self.screen_tasks.lock().drain() {
            task.abort();
        }
    }
}

fn screen_outputs(job: &ScreenJob, frame: &ResolvedFrame) -> Vec<GameSenseOutput> {
    frame
        .variants
        .iter()
        .map(|(size, content)| GameSenseOutput::Screen {
            game: job.game.clone(),
            event: job.event.clone(),
            device_type: job.device_type,
            zone: job.zone.clone(),
            size: *size,
            content: content.clone(),
            duration: frame.duration,
            priority: frame.priority,
        })
        .collect()
}

/// Play a multi-frame or repeating screen plan until it ends or a newer event replaces it.
async fn play_screen(tx: broadcast::Sender<GameSenseOutput>, job: ScreenJob) {
    let mut passes: u32 = 0;
    loop {
        for frame in &job.plan.frames {
            for output in screen_outputs(&job, frame) {
                send_output(&tx, output);
            }
            match frame.duration {
                Some(duration) => tokio::time::sleep(duration.max(MIN_SCREEN_FRAME)).await,
                // A frame without length stays until the next screen event.
                None => return,
            }
        }
        passes = passes.saturating_add(1);
        match job.plan.repeats {
            Repeats::Forever => {}
            Repeats::Times(total) if passes < total => {}
            _ => return,
        }
    }
}

async fn watchdog(inner: Arc<Inner>) {
    loop {
        tokio::time::sleep(inner.watchdog_tick()).await;
        inner.expire_idle_games(Instant::now());
    }
}

/// Aborts a task when dropped.
struct AbortOnDrop(AbortHandle);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// GameSense-compatible HTTP server.
///
/// Effects are published as [`GameSenseOutput`]s; see [`GameSenseServer::subscribe`].
pub struct GameSenseServer {
    inner: Arc<Inner>,
    bind_addr: SocketAddr,
}

impl GameSenseServer {
    /// Create a server that will listen on `host:port`.
    pub fn new(host: &str, port: u16) -> Result<Self> {
        let addr: SocketAddr = format!("{}:{}", host, port)
            .parse()
            .map_err(|e| Error::GameSense(format!("Invalid bind address: {}", e)))?;

        let (tx, _) = broadcast::channel(OUTPUT_CHANNEL_CAPACITY);
        Ok(Self {
            inner: Arc::new(Inner {
                state: Mutex::new(ServerState {
                    games: HashMap::new(),
                    default_timeout: DEFAULT_HEARTBEAT_TIMEOUT,
                    write_core_props: true,
                }),
                tx,
                screen_tasks: Mutex::new(HashMap::new()),
                legacy_task: Mutex::new(None),
            }),
            bind_addr: addr,
        })
    }

    /// Idle time after which a game without `deinitialize_timer_length_ms` is deactivated
    /// and its outputs cleared. Default: 15 s.
    pub fn with_heartbeat_timeout(self, timeout: Duration) -> Self {
        self.inner.state.lock().default_timeout = timeout.max(Duration::from_millis(1));
        self
    }

    /// Whether [`serve`](Self::serve) writes `coreProps.json` (default: true).
    pub fn with_core_props(self, enabled: bool) -> Self {
        self.inner.state.lock().write_core_props = enabled;
        self
    }

    /// Receive every output from now on.
    pub fn subscribe(&self) -> broadcast::Receiver<GameSenseOutput> {
        self.inner.tx.subscribe()
    }

    /// Legacy: call `callback(zone, r, g, b)` for keyboard lighting outputs.
    ///
    /// Kept so existing callers compile; new code should use [`subscribe`](Self::subscribe).
    /// The callback runs on a task fed by the output channel and sees only `keyboard` and
    /// `rgb-per-key-zones` lighting. `zone` is `"all"` for the whole device or any key-level
    /// target, `"<n>"` (1-based) for numbered zones, or the zone name. Unlit keys of
    /// `percent`/`count` (black) are not forwarded; flashing, screens and tactile are dropped.
    /// Calling it again replaces the previous callback.
    pub async fn set_rgb_callback<F>(&self, callback: F)
    where
        F: Fn(&str, u8, u8, u8) + Send + Sync + 'static,
    {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            warn!("GameSense: set_rgb_callback needs a Tokio runtime; callback not installed");
            return;
        };
        let mut rx = self.subscribe();
        let task = runtime.spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(output) => {
                        if let Some((zone, color)) = legacy_rgb_update(&output) {
                            callback(&zone, color.r, color.g, color.b);
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        debug!("GameSense: legacy RGB callback skipped {} outputs", skipped);
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });
        if let Some(previous) = self.inner.legacy_task.lock().replace(task.abort_handle()) {
            previous.abort();
        }
    }

    /// Build the router.
    fn router(&self) -> Router {
        Router::new()
            .route("/game_metadata", post(game_metadata))
            .route("/register_game_event", post(register_game_event))
            .route("/bind_game_event", post(bind_game_event))
            .route("/game_event", post(game_event))
            .route("/multiple_game_events", post(multiple_game_events))
            .route(
                "/supports_multiple_game_events",
                get(supports_multiple_game_events).post(supports_multiple_game_events),
            )
            .route("/game_heartbeat", post(game_heartbeat))
            .route("/stop_game", post(stop_game))
            .route("/remove_game_event", post(remove_game_event))
            .route("/remove_game", post(remove_game))
            .route("/load_golisp_handlers", post(load_golisp_handlers))
            .route("/", get(server_info))
            .fallback(not_found)
            .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
            .layer(
                CorsLayer::new()
                    .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                    .allow_headers([header::CONTENT_TYPE])
                    .allow_origin(AllowOrigin::predicate(|origin: &HeaderValue, _parts: &_| {
                        is_origin_allowed(origin)
                    })),
            )
            .with_state(Arc::clone(&self.inner))
    }

    /// Bind the configured address and serve until SIGINT/SIGTERM (Ctrl+C on Windows).
    pub async fn run(&self) -> Result<()> {
        let listener = tokio::net::TcpListener::bind(self.bind_addr).await?;
        self.serve(listener, shutdown_signal()).await
    }

    /// Serve on `listener` until `shutdown` completes.
    ///
    /// Writes `coreProps.json` (unless disabled with [`with_core_props`](Self::with_core_props))
    /// for the listener's port and removes it when serving stops, runs the heartbeat timeout,
    /// and clears every active game on exit.
    pub async fn serve<F>(&self, listener: tokio::net::TcpListener, shutdown: F) -> Result<()>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let local = listener.local_addr()?;
        info!("GameSense server listening on {}", local);

        let write_props = self.inner.state.lock().write_core_props;
        let _core_props = if write_props {
            let port = local.port();
            match tokio::task::spawn_blocking(move || write_core_props(port)).await {
                Ok(Ok(paths)) => {
                    for path in &paths {
                        info!("GameSense: wrote {}", path.display());
                    }
                    CorePropsGuard::new(paths)
                }
                Ok(Err(e)) => {
                    warn!("GameSense: could not write coreProps.json: {}", e);
                    CorePropsGuard::default()
                }
                Err(e) => {
                    warn!("GameSense: coreProps.json task failed: {}", e);
                    CorePropsGuard::default()
                }
            }
        } else {
            CorePropsGuard::default()
        };

        let _watchdog = AbortOnDrop(tokio::spawn(watchdog(Arc::clone(&self.inner))).abort_handle());
        let result = axum::serve(listener, self.router())
            .with_graceful_shutdown(shutdown)
            .await;
        self.inner.shutdown_games();
        result?;
        Ok(())
    }

    /// Get the server address.
    pub fn address(&self) -> SocketAddr {
        self.bind_addr
    }

    #[cfg(all(test, unix))]
    fn write_secure_json(path: &std::path::Path, content: &serde_json::Value) -> Result<()> {
        super::core_props::write_secure_json(path, content)
    }
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        match (signal(SignalKind::terminate()), signal(SignalKind::interrupt())) {
            (Ok(mut term), Ok(mut int)) => {
                tokio::select! {
                    _ = term.recv() => {}
                    _ = int.recv() => {}
                }
            }
            _ => {
                warn!("GameSense: cannot listen for shutdown signals");
                std::future::pending::<()>().await;
            }
        }
    }
    #[cfg(not(unix))]
    {
        if tokio::signal::ctrl_c().await.is_err() {
            warn!("GameSense: cannot listen for Ctrl+C");
            std::future::pending::<()>().await;
        }
    }
    info!("GameSense server shutting down");
}

/// What the legacy RGB callback receives for an output, if anything.
fn legacy_rgb_update(output: &GameSenseOutput) -> Option<(String, Color)> {
    let GameSenseOutput::Lighting {
        device_type,
        target,
        color,
        ..
    } = output
    else {
        return None;
    };
    if !device_type.is_keyboard() {
        return None;
    }
    let zone = match target {
        LightTarget::AllZones => "all".to_string(),
        LightTarget::Zone(index) => (index + 1).to_string(),
        LightTarget::NamedZone(name) => name.clone(),
        LightTarget::Keys(_) | LightTarget::HidCodes(_) => {
            if *color == Color::BLACK {
                return None;
            }
            "all".to_string()
        }
    };
    Some((zone, *color))
}

type AppState = Arc<Inner>;

/// Check if an origin is allowed for GameSense API.
fn is_origin_allowed(origin: &HeaderValue) -> bool {
    // Origin header must be valid UTF-8 and a valid URI
    let Ok(origin_str) = origin.to_str() else {
        return false;
    };

    // Parse as URI to safely extract host
    let Ok(uri) = origin_str.parse::<axum::http::Uri>() else {
        return false;
    };

    // Strictly validate host and ensure it doesn't contain a colon (port bypass)
    // Note: URI parser might sometimes treat 'localhost:evil.com' as host='localhost'
    // with a non-numeric port that it then ignores or misinterprets.
    // We also check for any characters that shouldn't be in a clean hostname.
    match uri.host() {
        Some(host) => {
            // Check for standard local hosts.
            // Note: uri.host() typically strips brackets from IPv6 addresses, but we check both
            // to be safe across different versions/implementations.
            let is_allowed_host = host == "localhost" || host == "127.0.0.1" || host == "::1" || host == "[::1]";

            // Additional check: ensure the full authority doesn't have suspicious characters
            // and if a port is present, it must be numeric.
            let is_clean = if let Some(auth) = uri.authority() {
                let auth_str = auth.as_str();
                // Find the port separator, taking into account IPv6 brackets
                let port_start = if auth_str.starts_with('[') {
                    auth_str.find("]:").map(|i| i + 1)
                } else {
                    auth_str.find(':')
                };

                if let Some(port_idx) = port_start {
                    let port_part = &auth_str[port_idx + 1..];
                    !port_part.is_empty() && port_part.chars().all(|c| c.is_ascii_digit())
                } else {
                    true
                }
            } else {
                true
            };

            is_allowed_host && is_clean
        }
        None => false,
    }
}

/// Validate and parse a request body.
///
/// Requests carrying a non-local `Origin` are refused outright (403), and the body must be
/// declared `application/json`, which keeps cross-site "simple" browser requests out.
fn parse_body(headers: &HeaderMap, body: &[u8]) -> std::result::Result<Map<String, Value>, ApiError> {
    if let Some(origin) = headers.get(header::ORIGIN) {
        if !is_origin_allowed(origin) {
            return Err(ApiError::new(StatusCode::FORBIDDEN, "Origin not allowed"));
        }
    }
    let is_json = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| {
            ct.split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .eq_ignore_ascii_case("application/json")
        });
    if !is_json {
        return Err(ApiError::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "Content-Type must be application/json",
        ));
    }
    match serde_json::from_slice::<Value>(body) {
        Ok(Value::Object(map)) => Ok(map),
        _ => Err(ApiError::sdk(0)),
    }
}

/// SDK rule for game and event names: upper-case A-Z, 0-9, hyphen and underscore.
fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME_LEN
        && name
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

fn game_name(map: &Map<String, Value>) -> std::result::Result<String, ApiError> {
    let game = map
        .get("game")
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::sdk(1))?;
    if !is_valid_name(game) {
        return Err(ApiError::sdk(3));
    }
    Ok(game.to_string())
}

fn game_and_event(map: &Map<String, Value>) -> std::result::Result<(String, String), ApiError> {
    let game = map.get("game").and_then(Value::as_str);
    let event = map.get("event").and_then(Value::as_str);
    let (Some(game), Some(event)) = (game, event) else {
        return Err(ApiError::sdk(0));
    };
    if !is_valid_name(game) || !is_valid_name(event) {
        return Err(ApiError::sdk(2));
    }
    Ok((game.to_string(), event.to_string()))
}

/// `data` of an event: an object or a stringified object (SDK error 4 otherwise).
fn event_data(map: &Map<String, Value>) -> std::result::Result<Map<String, Value>, ApiError> {
    match map.get("data") {
        Some(Value::Object(data)) => Ok(data.clone()),
        Some(Value::String(text)) => match serde_json::from_str::<Value>(text) {
            Ok(Value::Object(data)) => Ok(data),
            _ => Err(ApiError::sdk(4)),
        },
        _ => Err(ApiError::sdk(4)),
    }
}

/// The event `value`: integers, rounded floats, booleans and numeric strings are accepted.
fn parse_event_value(value: &Value) -> Option<i64> {
    match value {
        Value::Number(_) => json_to_i64(value),
        Value::Bool(b) => Some(i64::from(*b)),
        Value::String(s) => {
            let s = s.trim();
            s.parse::<i64>().ok().or_else(|| {
                s.parse::<f64>()
                    .ok()
                    .filter(|f| f.is_finite())
                    .map(|f| f.round() as i64)
            })
        }
        _ => None,
    }
}

fn text_field(map: &Map<String, Value>, key: &str) -> Option<String> {
    map.get(key)
        .and_then(Value::as_str)
        .map(|s| s.chars().take(MAX_TEXT_FIELD_CHARS).collect())
}

/// Process one event update while holding the state lock.
fn process_event(
    state: &mut ServerState,
    game: &str,
    event: &str,
    data: &Map<String, Value>,
    now: Instant,
    batch: &mut Batch,
) -> std::result::Result<(), ApiError> {
    let game_state = state.game_entry(game, now)?;
    game_state.last_activity = now;
    if !game_state.active {
        game_state.active = true;
        info!("GameSense: game {} is active", game);
    }

    let value = data.get("value").and_then(parse_event_value);
    let event_state = game_state.event_entry(event)?;
    let value = if event_state.value_optional {
        if value.is_some() {
            event_state.last_value = value;
        }
        value.or(event_state.last_value).unwrap_or(0)
    } else {
        match value {
            None => return Ok(()),
            Some(v) if event_state.last_value == Some(v) => return Ok(()),
            Some(v) => {
                event_state.last_value = Some(v);
                v
            }
        }
    };

    let game_state: &GameState = game_state;
    let Some(event_state) = game_state.events.get(event) else {
        return Ok(());
    };
    let input = EventInput {
        value,
        min_value: event_state.min_value,
        max_value: event_state.max_value,
        frame: data.get("frame").and_then(Value::as_object),
    };
    for handler in &event_state.handlers {
        evaluate_handler(game, event, handler, &input, game_state, batch);
    }
    Ok(())
}

fn evaluate_handler(
    game: &str,
    event: &str,
    handler: &HandlerSpec,
    input: &EventInput<'_>,
    game_state: &GameState,
    batch: &mut Batch,
) {
    match &handler.kind {
        HandlerKind::Color(spec) => {
            let Some(target) = &handler.target else {
                return;
            };
            let Some(result) = eval::evaluate_lighting(target, spec, input) else {
                return;
            };
            for segment in result.segments {
                batch.outputs.push(GameSenseOutput::Lighting {
                    game: game.to_string(),
                    event: event.to_string(),
                    device_type: handler.device_type,
                    target: segment.target,
                    color: segment.color,
                    flash: if segment.lit { result.flash } else { None },
                    duration: None,
                });
            }
        }
        HandlerKind::Bitmap {
            partial,
            excluded_events,
        } => {
            let Some(colors) = eval::parse_bitmap(input.frame) else {
                return;
            };
            let excluded = if *partial {
                let from_frame: Option<Vec<String>> = input
                    .frame
                    .and_then(|f| f.get("excluded-events"))
                    .and_then(Value::as_array)
                    .map(|list| list.iter().filter_map(|v| v.as_str().map(str::to_string)).collect());
                excluded_targets(game_state, from_frame.as_deref().unwrap_or(excluded_events))
            } else {
                Vec::new()
            };
            batch.outputs.push(GameSenseOutput::Bitmap {
                game: game.to_string(),
                event: event.to_string(),
                device_type: handler.device_type,
                colors,
                excluded,
            });
        }
        HandlerKind::Screen(spec) => {
            if let Some(plan) = eval::resolve_screen(&handler.device_type, spec, input) {
                let zone = handler.zone_name();
                batch.screens.push(ScreenJob {
                    key: (game.to_string(), handler.device_type.to_string(), zone.clone()),
                    game: game.to_string(),
                    event: event.to_string(),
                    device_type: handler.device_type,
                    zone,
                    plan,
                });
            }
        }
        HandlerKind::Tactile(spec) => {
            if let Some((pattern, rate)) = eval::resolve_tactile(spec, input) {
                batch.outputs.push(GameSenseOutput::Tactile {
                    game: game.to_string(),
                    event: event.to_string(),
                    device_type: handler.device_type,
                    zone: handler.zone_name(),
                    pattern,
                    rate,
                });
            }
        }
    }
}

/// Keys used by the named events' keyboard handlers (`partial-bitmap` exclusions).
fn excluded_targets(game_state: &GameState, events: &[String]) -> Vec<LightTarget> {
    events
        .iter()
        .filter_map(|name| game_state.events.get(name))
        .flat_map(|ev| ev.handlers.iter())
        .filter(|h| h.device_type.is_keyboard() && matches!(h.kind, HandlerKind::Color(_)))
        .filter_map(|h| h.target.clone())
        .collect()
}

fn respond(result: ApiResult) -> Response {
    match result {
        Ok(body) => (StatusCode::OK, Json(body)).into_response(),
        Err(err) => {
            debug!("GameSense request rejected: {} {}", err.status, err.message);
            err.into_response()
        }
    }
}

/// Server info endpoint.
async fn server_info() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "address": "127.0.0.1",
        "encrypted_address": null,
        "sse_address": null,
        "version": "4.0.0"
    }))
}

async fn not_found() -> Response {
    ApiError::new(StatusCode::NOT_FOUND, "Unknown GameSense endpoint").into_response()
}

async fn supports_multiple_game_events() -> Json<Value> {
    Json(json!({}))
}

async fn game_metadata(State(inner): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    respond(handle_game_metadata(&inner, &headers, &body))
}

fn handle_game_metadata(inner: &Inner, headers: &HeaderMap, body: &[u8]) -> ApiResult {
    let map = parse_body(headers, body)?;
    let game = game_name(&map)?;
    let display_name = text_field(&map, "game_display_name");
    let developer = text_field(&map, "developer");
    let timeout_ms = map
        .get("deinitialize_timer_length_ms")
        .and_then(json_to_i64)
        .map(|ms| ms.clamp(MIN_GAME_TIMEOUT_MS, MAX_GAME_TIMEOUT_MS));

    let mut state = inner.state.lock();
    let game_state = state.game_entry(&game, Instant::now())?;
    if display_name.is_some() {
        game_state.display_name = display_name;
    }
    if developer.is_some() {
        game_state.developer = developer;
    }
    if let Some(ms) = timeout_ms {
        game_state.deinit_timeout = Some(Duration::from_millis(ms.unsigned_abs()));
    }
    info!(
        "GameSense: metadata for {} ({})",
        game,
        game_state.display_name.as_deref().unwrap_or("no display name")
    );
    Ok(json!({
        "game_metadata": {
            "game": game,
            "game_display_name": game_state.display_name,
            "developer": game_state.developer,
            "deinitialize_timer_length_ms": game_state.deinit_timeout.map(|d| d.as_millis() as u64),
        }
    }))
}

async fn register_game_event(State(inner): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    respond(handle_register_game_event(&inner, &headers, &body))
}

fn handle_register_game_event(inner: &Inner, headers: &HeaderMap, body: &[u8]) -> ApiResult {
    let map = parse_body(headers, body)?;
    let (game, event) = game_and_event(&map)?;
    let meta = EventMeta::from_request(&map);

    let mut state = inner.state.lock();
    let event_state = state.game_entry(&game, Instant::now())?.event_entry(&event)?;
    meta.apply(event_state);
    debug!("GameSense: registered event {}/{}", game, event);
    Ok(json!({ "register_game_event": meta.to_json(&game, &event) }))
}

async fn bind_game_event(State(inner): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    respond(handle_bind_game_event(&inner, &headers, &body))
}

fn handle_bind_game_event(inner: &Inner, headers: &HeaderMap, body: &[u8]) -> ApiResult {
    let map = parse_body(headers, body)?;
    let (game, event) = game_and_event(&map)?;
    let raw_handlers = match map.get("handlers") {
        Some(Value::Array(list)) if !list.is_empty() => list,
        _ => return Err(ApiError::sdk(6)),
    };
    let meta = EventMeta::from_request(&map);

    let mut handlers = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    for (index, raw) in raw_handlers.iter().enumerate() {
        if handlers.len() >= MAX_HANDLERS_PER_EVENT {
            warnings.push(format!(
                "handler {index}: more than {MAX_HANDLERS_PER_EVENT} handlers; ignored"
            ));
            continue;
        }
        match parse_handler(raw) {
            Ok(spec) => {
                for arg in spec.unsupported_args() {
                    warnings.push(format!(
                        "handler {index}: arg '{arg}' is a GoLisp expression this server cannot evaluate; the event value is shown instead"
                    ));
                }
                handlers.push(spec);
            }
            Err(e) => warnings.push(format!("handler {index}: {e}")),
        }
    }
    for warning in &warnings {
        warn!("GameSense: {}/{}: {}", game, event, warning);
    }

    let mut state = inner.state.lock();
    let event_state = state.game_entry(&game, Instant::now())?.event_entry(&event)?;
    meta.apply(event_state);
    debug!("GameSense: bound {} handler(s) to {}/{}", handlers.len(), game, event);
    event_state.handlers = handlers;

    let mut echo = meta.to_json(&game, &event);
    echo.insert("handlers".into(), Value::Array(raw_handlers.clone()));
    let mut response = Map::new();
    response.insert("bind_game_event".into(), Value::Object(echo));
    if !warnings.is_empty() {
        response.insert("warnings".into(), json!(warnings));
    }
    Ok(Value::Object(response))
}

async fn game_event(State(inner): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    respond(handle_game_event(&inner, &headers, &body))
}

fn handle_game_event(inner: &Inner, headers: &HeaderMap, body: &[u8]) -> ApiResult {
    let map = parse_body(headers, body)?;
    let (game, event) = game_and_event(&map)?;
    let data = event_data(&map)?;

    let mut batch = Batch::default();
    let result = {
        let mut state = inner.state.lock();
        process_event(&mut state, &game, &event, &data, Instant::now(), &mut batch)
    };
    inner.dispatch(batch);
    result?;
    Ok(json!({ "game_event": { "game": game, "event": event, "data": data } }))
}

async fn multiple_game_events(State(inner): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    respond(handle_multiple_game_events(&inner, &headers, &body))
}

fn handle_multiple_game_events(inner: &Inner, headers: &HeaderMap, body: &[u8]) -> ApiResult {
    let map = parse_body(headers, body)?;
    let game = game_name(&map)?;
    let list = map
        .get("events")
        .and_then(Value::as_array)
        .ok_or_else(|| ApiError::sdk(0))?;
    if list.len() > MAX_EVENTS_PER_BATCH {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            format!("At most {MAX_EVENTS_PER_BATCH} events per request"),
        ));
    }
    let mut events = Vec::with_capacity(list.len());
    for entry in list {
        let obj = entry.as_object().ok_or_else(|| ApiError::sdk(0))?;
        let name = obj
            .get("event")
            .and_then(Value::as_str)
            .ok_or_else(|| ApiError::sdk(0))?;
        if !is_valid_name(name) {
            return Err(ApiError::sdk(2));
        }
        events.push((name.to_string(), event_data(obj)?));
    }

    let mut batch = Batch::default();
    let result = {
        let mut state = inner.state.lock();
        let now = Instant::now();
        events
            .iter()
            .try_for_each(|(event, data)| process_event(&mut state, &game, event, data, now, &mut batch))
    };
    inner.dispatch(batch);
    result?;
    Ok(json!({ "multiple_game_events": { "game": game, "events": events.len() } }))
}

async fn game_heartbeat(State(inner): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    respond(handle_game_heartbeat(&inner, &headers, &body))
}

fn handle_game_heartbeat(inner: &Inner, headers: &HeaderMap, body: &[u8]) -> ApiResult {
    let map = parse_body(headers, body)?;
    let game = game_name(&map)?;
    if let Some(game_state) = inner.state.lock().games.get_mut(&game) {
        game_state.last_activity = Instant::now();
    }
    trace!("GameSense: heartbeat from {}", game);
    Ok(json!({ "game_heartbeat": { "game": game } }))
}

async fn stop_game(State(inner): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    respond(handle_stop_game(&inner, &headers, &body))
}

fn handle_stop_game(inner: &Inner, headers: &HeaderMap, body: &[u8]) -> ApiResult {
    let map = parse_body(headers, body)?;
    let game = game_name(&map)?;
    let was_active = inner
        .state
        .lock()
        .games
        .get_mut(&game)
        .is_some_and(GameState::deactivate);
    if was_active {
        info!("GameSense: {} stopped; restoring default lighting", game);
        inner.clear_games(vec![game.clone()]);
    } else {
        inner.abort_screens(&game);
    }
    Ok(json!({ "stop_game": { "game": game } }))
}

async fn remove_game_event(State(inner): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    respond(handle_remove_game_event(&inner, &headers, &body))
}

fn handle_remove_game_event(inner: &Inner, headers: &HeaderMap, body: &[u8]) -> ApiResult {
    let map = parse_body(headers, body)?;
    let (game, event) = game_and_event(&map)?;
    let removed = inner
        .state
        .lock()
        .games
        .get_mut(&game)
        .and_then(|g| g.events.remove(&event))
        .is_some();
    if !removed {
        return Err(ApiError::sdk(9));
    }
    debug!("GameSense: removed event {}/{}", game, event);
    Ok(json!({ "remove_game_event": { "game": game, "event": event } }))
}

async fn remove_game(State(inner): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    respond(handle_remove_game(&inner, &headers, &body))
}

fn handle_remove_game(inner: &Inner, headers: &HeaderMap, body: &[u8]) -> ApiResult {
    let map = parse_body(headers, body)?;
    let game = game_name(&map)?;
    let removed = inner.state.lock().games.remove(&game);
    let Some(game_state) = removed else {
        return Err(ApiError::sdk(10));
    };
    info!("GameSense: removed game {}", game);
    if game_state.active {
        inner.clear_games(vec![game.clone()]);
    } else {
        inner.abort_screens(&game);
    }
    Ok(json!({ "remove_game": { "game": game } }))
}

async fn load_golisp_handlers(State(_inner): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    respond(handle_load_golisp_handlers(&headers, &body))
}

fn handle_load_golisp_handlers(headers: &HeaderMap, body: &[u8]) -> ApiResult {
    let map = parse_body(headers, body)?;
    let game = game_name(&map)?;
    warn!(
        "GameSense: {} sent GoLisp handlers; GoLisp is not supported, use JSON handlers (bind_game_event)",
        game
    );
    Ok(json!({
        "load_golisp_handlers": { "game": game },
        "warnings": ["GoLisp handlers are not supported by this server and were ignored; use bind_game_event"],
    }))
}

#[cfg(test)]
fn compute_color(color: &ColorHandler, value: i32) -> Option<(u8, u8, u8)> {
    let def = super::handlers::Ranged::from(color);
    eval::evaluate_color(&def, i64::from(value), f64::from(value.clamp(0, 100))).map(|c| (c.r, c.g, c.b))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    #[test]
    fn test_compute_color_static() {
        let handler = ColorHandler::Static {
            red: 255,
            green: 0,
            blue: 0,
        };

        let (r, g, b) = compute_color(&handler, 50).unwrap();
        assert_eq!(r, 255);
        assert_eq!(g, 0);
        assert_eq!(b, 0);
    }

    #[test]
    fn test_server_new() {
        // Valid address
        let server = GameSenseServer::new("127.0.0.1", 0);
        assert!(server.is_ok());
        assert_eq!(server.unwrap().address().ip().to_string(), "127.0.0.1");

        // Valid IPv6 address
        let server_v6 = GameSenseServer::new("[::1]", 0);
        assert!(server_v6.is_ok());
        assert_eq!(server_v6.unwrap().address().ip().to_string(), "::1");
    }

    #[test]
    fn test_server_new_invalid_address() {
        // Invalid host
        let result = GameSenseServer::new("invalid_host", 8080);
        assert!(matches!(result, Err(Error::GameSense(ref m)) if m.contains("Invalid bind address")));

        // Empty host
        let result = GameSenseServer::new("", 8080);
        assert!(matches!(result, Err(Error::GameSense(ref m)) if m.contains("Invalid bind address")));

        // Invalid IP format
        let result = GameSenseServer::new("127.0.0.1.1", 8080);
        assert!(matches!(result, Err(Error::GameSense(ref m)) if m.contains("Invalid bind address")));
    }

    #[test]
    fn test_compute_color_gradient() {
        let handler = ColorHandler::Gradient {
            gradient: GradientSpec {
                zero: ColorSpec {
                    red: 255,
                    green: 0,
                    blue: 0,
                },
                hundred: ColorSpec {
                    red: 0,
                    green: 255,
                    blue: 0,
                },
            },
        };

        // At value 0, should be red
        let (r, g, b) = compute_color(&handler, 0).unwrap();
        assert_eq!(r, 255);
        assert_eq!(g, 0);
        assert_eq!(b, 0);

        // At value 100, should be green
        let (r, g, b) = compute_color(&handler, 100).unwrap();
        assert_eq!(r, 0);
        assert_eq!(g, 255);
        assert_eq!(b, 0);

        // At value 50, should be blend
        let (r, g, b) = compute_color(&handler, 50).unwrap();
        assert!(r > 100 && r < 150);
        assert!(g > 100 && g < 150);
        assert_eq!(b, 0);
    }

    #[test]
    fn test_compute_color_range() {
        let handler = ColorHandler::Range {
            color: vec![
                RangeColor {
                    low: 0,
                    high: 25,
                    color: ColorSpec {
                        red: 255,
                        green: 0,
                        blue: 0,
                    },
                },
                RangeColor {
                    low: 26,
                    high: 75,
                    color: ColorSpec {
                        red: 255,
                        green: 255,
                        blue: 0,
                    },
                },
                RangeColor {
                    low: 76,
                    high: 100,
                    color: ColorSpec {
                        red: 0,
                        green: 255,
                        blue: 0,
                    },
                },
            ],
        };

        // Test first range
        let (r, g, b) = compute_color(&handler, 10).unwrap();
        assert_eq!(r, 255);
        assert_eq!(g, 0);
        assert_eq!(b, 0);

        // Test second range
        let (r, g, b) = compute_color(&handler, 50).unwrap();
        assert_eq!(r, 255);
        assert_eq!(g, 255);
        assert_eq!(b, 0);

        // Test third range
        let (r, g, b) = compute_color(&handler, 90).unwrap();
        assert_eq!(r, 0);
        assert_eq!(g, 255);
        assert_eq!(b, 0);

        // Test value outside ranges
        assert!(compute_color(&handler, 150).is_none());
    }

    #[test]
    fn test_cors_origin_predicate() {
        use axum::http::HeaderValue;

        // Allowed origins
        assert!(is_origin_allowed(&HeaderValue::from_static("http://localhost")));
        assert!(is_origin_allowed(&HeaderValue::from_static("https://localhost")));
        assert!(is_origin_allowed(&HeaderValue::from_static("http://localhost:8080")));
        assert!(is_origin_allowed(&HeaderValue::from_static("http://127.0.0.1")));
        assert!(is_origin_allowed(&HeaderValue::from_static("http://127.0.0.1:3000")));
        assert!(is_origin_allowed(&HeaderValue::from_static("http://[::1]")));
        assert!(is_origin_allowed(&HeaderValue::from_static("http://[::1]:1234")));

        // Malicious or invalid origins
        assert!(!is_origin_allowed(&HeaderValue::from_static(
            "http://localhost.evil.com"
        )));
        assert!(!is_origin_allowed(&HeaderValue::from_static(
            "http://127.0.0.1.attacker.com"
        )));
        assert!(!is_origin_allowed(&HeaderValue::from_static(
            "http://localhost:evil.com"
        )));
        assert!(!is_origin_allowed(&HeaderValue::from_static("http://not-localhost")));
        assert!(!is_origin_allowed(&HeaderValue::from_static("http://192.168.1.1")));
        assert!(!is_origin_allowed(&HeaderValue::from_static("null")));
        assert!(!is_origin_allowed(&HeaderValue::from_static("")));
    }

    #[test]
    #[cfg(unix)]
    fn test_secure_json_write() -> Result<()> {
        let temp_dir = std::env::temp_dir().join("test_gamesense_secure");
        let target_path = temp_dir.join("coreProps.json");

        // Cleanup
        if temp_dir.exists() {
            std::fs::remove_dir_all(&temp_dir)?;
        }

        // Write content
        let props = serde_json::json!({ "test": true });

        // This should create the directory securely and write the file
        GameSenseServer::write_secure_json(&target_path, &props)?;

        // Verify directory
        let metadata = std::fs::symlink_metadata(&temp_dir)?;
        assert!(metadata.is_dir());
        assert_eq!(metadata.uid(), rustix::process::getuid().as_raw());
        // Verify secure permissions (0o700)
        assert_eq!(metadata.permissions().mode() & 0o777, 0o700);

        // Verify file
        let file_metadata = std::fs::symlink_metadata(&target_path)?;
        // Verify secure permissions (0o600)
        assert_eq!(file_metadata.permissions().mode() & 0o777, 0o600);
        assert!(!file_metadata.file_type().is_symlink());

        // Cleanup
        std::fs::remove_dir_all(&temp_dir)?;

        Ok(())
    }

    #[test]
    fn names_follow_sdk_rule() {
        assert!(is_valid_name("MY_GAME-2"));
        assert!(!is_valid_name("my_game"));
        assert!(!is_valid_name("MY GAME"));
        assert!(!is_valid_name(""));
        assert!(!is_valid_name(&"A".repeat(MAX_NAME_LEN + 1)));
    }

    #[test]
    fn event_values_are_parsed_leniently() {
        assert_eq!(parse_event_value(&json!(75)), Some(75));
        assert_eq!(parse_event_value(&json!(75.6)), Some(76));
        assert_eq!(parse_event_value(&json!(true)), Some(1));
        assert_eq!(parse_event_value(&json!(" 42 ")), Some(42));
        assert_eq!(parse_event_value(&json!("abc")), None);
        assert_eq!(parse_event_value(&Value::Null), None);
    }

    fn lighting(device_type: DeviceType, target: LightTarget, color: Color) -> GameSenseOutput {
        GameSenseOutput::Lighting {
            game: "G".into(),
            event: "E".into(),
            device_type,
            target,
            color,
            flash: None,
            duration: None,
        }
    }

    #[test]
    fn legacy_callback_mapping() {
        let red = Color::new(255, 0, 0);
        assert_eq!(
            legacy_rgb_update(&lighting(DeviceType::Keyboard, LightTarget::AllZones, red)),
            Some(("all".to_string(), red))
        );
        assert_eq!(
            legacy_rgb_update(&lighting(DeviceType::RgbPerKeyZones, LightTarget::Zone(2), red)),
            Some(("3".to_string(), red))
        );
        assert_eq!(
            legacy_rgb_update(&lighting(
                DeviceType::Keyboard,
                LightTarget::NamedZone("logo".into()),
                red
            )),
            Some(("logo".to_string(), red))
        );
        assert_eq!(
            legacy_rgb_update(&lighting(
                DeviceType::Keyboard,
                LightTarget::Keys(vec![crate::devices::key_mapping::KeyId::Q]),
                red
            )),
            Some(("all".to_string(), red))
        );
        assert_eq!(
            legacy_rgb_update(&lighting(
                DeviceType::Keyboard,
                LightTarget::HidCodes(vec![70]),
                Color::BLACK
            )),
            None
        );
        assert_eq!(
            legacy_rgb_update(&lighting(DeviceType::Mouse, LightTarget::AllZones, red)),
            None
        );
        assert_eq!(legacy_rgb_update(&GameSenseOutput::Clear { game: "G".into() }), None);
    }

    #[test]
    fn idle_games_expire() {
        let server = GameSenseServer::new("127.0.0.1", 0)
            .unwrap()
            .with_heartbeat_timeout(Duration::from_millis(100));
        let mut rx = server.subscribe();
        let start = Instant::now();
        {
            let mut state = server.inner.state.lock();
            let mut batch = Batch::default();
            let data = json!({"value": 1});
            process_event(&mut state, "G", "E", data.as_object().unwrap(), start, &mut batch).unwrap();
            assert!(state.games["G"].active);
        }
        server.inner.expire_idle_games(start + Duration::from_millis(50));
        assert!(rx.try_recv().is_err(), "not idle long enough");
        server.inner.expire_idle_games(start + Duration::from_millis(150));
        assert_eq!(rx.try_recv().unwrap(), GameSenseOutput::Clear { game: "G".into() });
        assert!(!server.inner.state.lock().games["G"].active);
    }
}
