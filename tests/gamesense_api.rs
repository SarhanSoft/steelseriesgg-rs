//! End-to-end tests of the GameSense HTTP API over a real TCP socket.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};
use steelseries_gg::devices::key_mapping::KeyId;
use steelseries_gg::gamesense::{
    DeviceType, GameSenseOutput, GameSenseServer, LightTarget, Rate, ScreenContent, ScreenLine, ScreenSize,
    TactileStep, discover_prefix_core_props, keyboard_zone_keys, remove_core_props, write_core_props_to,
};
use steelseries_gg::rgb::Color;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast::{self, error::TryRecvError};
use tokio::sync::oneshot;

struct TestServer {
    addr: SocketAddr,
    rx: broadcast::Receiver<GameSenseOutput>,
    stop: Option<oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<steelseries_gg::Result<()>>>,
}

impl TestServer {
    async fn start() -> Self {
        Self::start_with(GameSenseServer::new("127.0.0.1", 0).unwrap()).await
    }

    async fn start_with(server: GameSenseServer) -> Self {
        let server = server.with_core_props(false);
        let rx = server.subscribe();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (stop, stopped) = oneshot::channel::<()>();
        let task = tokio::spawn(async move {
            server
                .serve(listener, async {
                    let _ = stopped.await;
                })
                .await
        });
        Self {
            addr,
            rx,
            stop: Some(stop),
            task: Some(task),
        }
    }

    async fn post(&self, path: &str, body: Value) -> Response {
        self.raw("POST", path, &[("Content-Type", "application/json")], &body.to_string())
            .await
    }

    async fn raw(&self, method: &str, path: &str, headers: &[(&str, &str)], body: &str) -> Response {
        let mut stream = TcpStream::connect(self.addr).await.unwrap();
        let mut request = format!(
            "{method} {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nContent-Length: {}\r\n",
            self.addr,
            body.len()
        );
        for (name, value) in headers {
            request.push_str(&format!("{name}: {value}\r\n"));
        }
        request.push_str("\r\n");
        request.push_str(body);
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).await.unwrap();
        let text = String::from_utf8_lossy(&raw).to_string();
        let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
        let status = head.split_whitespace().nth(1).unwrap().parse().unwrap();
        Response {
            status,
            headers: head.to_ascii_lowercase(),
            body: serde_json::from_str(body).unwrap_or(Value::Null),
        }
    }

    async fn next(&mut self) -> GameSenseOutput {
        tokio::time::timeout(Duration::from_secs(3), self.rx.recv())
            .await
            .expect("timed out waiting for an output")
            .expect("output channel closed")
    }

    fn assert_quiet(&mut self) {
        match self.rx.try_recv() {
            Err(TryRecvError::Empty) => {}
            other => panic!("expected no output, got {other:?}"),
        }
    }

    async fn shutdown(mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(task) = self.task.take() {
            task.await.unwrap().unwrap();
        }
    }
}

struct Response {
    status: u16,
    headers: String,
    body: Value,
}

impl Response {
    fn assert_sdk_error(&self, code: u64) {
        assert_eq!(self.status, 400, "body: {}", self.body);
        assert_eq!(self.body["code"], json!(code), "body: {}", self.body);
        assert!(self.body["error"].is_string());
    }
}

fn keyboard_bind(game: &str, event: &str, zone: &str, color: Value) -> Value {
    json!({
        "game": game,
        "event": event,
        "handlers": [{"device-type": "keyboard", "zone": zone, "mode": "color", "color": color}]
    })
}

fn lighting(output: &GameSenseOutput) -> (&str, &LightTarget, Color, Option<Rate>) {
    match output {
        GameSenseOutput::Lighting {
            event,
            target,
            color,
            flash,
            ..
        } => (event.as_str(), target, *color, *flash),
        other => panic!("expected lighting, got {other:?}"),
    }
}

fn red() -> Value {
    json!({"red": 255, "green": 0, "blue": 0})
}

#[tokio::test]
async fn game_metadata_endpoint() {
    let mut s = TestServer::start().await;
    let r = s
        .post(
            "/game_metadata",
            json!({"game": "TEST_GAME", "game_display_name": "My testing game", "developer": "My Game Studios",
                   "deinitialize_timer_length_ms": 500}),
        )
        .await;
    assert_eq!(r.status, 200);
    assert_eq!(r.body["game_metadata"]["game"], "TEST_GAME");
    assert_eq!(r.body["game_metadata"]["game_display_name"], "My testing game");
    assert_eq!(r.body["game_metadata"]["developer"], "My Game Studios");
    assert_eq!(
        r.body["game_metadata"]["deinitialize_timer_length_ms"], 1000,
        "clamped to the SDK minimum"
    );

    let r = s.post("/game_metadata", json!({"game": "TEST_GAME"})).await;
    assert_eq!(r.status, 200);
    assert_eq!(r.body["game_metadata"]["game_display_name"], "My testing game", "kept");

    s.post("/game_metadata", json!({"developer": "x"}))
        .await
        .assert_sdk_error(1);
    s.post("/game_metadata", json!({"game": "test game"}))
        .await
        .assert_sdk_error(3);
    s.raw(
        "POST",
        "/game_metadata",
        &[("Content-Type", "application/json")],
        "{not json",
    )
    .await
    .assert_sdk_error(0);
    let r = s
        .raw(
            "POST",
            "/game_metadata",
            &[("Content-Type", "text/plain")],
            r#"{"game":"A"}"#,
        )
        .await;
    assert_eq!(r.status, 415);
    s.assert_quiet();
    s.shutdown().await;
}

#[tokio::test]
async fn register_game_event_endpoint() {
    let mut s = TestServer::start().await;
    let r = s
        .post("/register_game_event", json!({"game": "ADVENTURE", "event": "MANA"}))
        .await;
    assert_eq!(r.status, 200);
    assert_eq!(
        r.body["register_game_event"],
        json!({"game": "ADVENTURE", "event": "MANA", "min_value": 0, "max_value": 100,
               "icon_id": 0, "value_optional": false})
    );
    let r = s
        .post(
            "/register_game_event",
            json!({"game": "ADVENTURE", "event": "HEALTH", "min_value": 0, "max_value": 10,
                   "icon_id": 1, "value_optional": true}),
        )
        .await;
    assert_eq!(r.body["register_game_event"]["max_value"], 10);
    assert_eq!(r.body["register_game_event"]["value_optional"], true);

    s.post("/register_game_event", json!({"game": "ADVENTURE"}))
        .await
        .assert_sdk_error(0);
    s.post("/register_game_event", json!({"game": "ADVENTURE", "event": "mana"}))
        .await
        .assert_sdk_error(2);

    // No handlers: events are accepted and produce nothing.
    let r = s
        .post(
            "/game_event",
            json!({"game": "ADVENTURE", "event": "MANA", "data": {"value": 5}}),
        )
        .await;
    assert_eq!(r.status, 200);
    s.assert_quiet();

    // Registering again keeps handlers bound earlier.
    s.post("/bind_game_event", keyboard_bind("ADVENTURE", "AMMO", "q", red()))
        .await;
    s.post(
        "/register_game_event",
        json!({"game": "ADVENTURE", "event": "AMMO", "max_value": 50}),
    )
    .await;
    s.post(
        "/game_event",
        json!({"game": "ADVENTURE", "event": "AMMO", "data": {"value": 5}}),
    )
    .await;
    let out = s.next().await;
    assert_eq!(lighting(&out).1, &LightTarget::Keys(vec![KeyId::Q]));
    s.shutdown().await;
}

#[tokio::test]
async fn bind_game_event_endpoint() {
    let mut s = TestServer::start().await;
    let stringified =
        r#"{"device-type":"rgb-2-zone","zone":"two","mode":"color","color":{"red":0,"green":0,"blue":255}}"#;
    let r = s
        .post(
            "/bind_game_event",
            json!({
                "game": "ADVENTURE",
                "event": "HEALTH",
                "min_value": 0,
                "max_value": 100,
                "icon_id": 1,
                "handlers": [
                    {
                        "device-type": "keyboard",
                        "zone": "function-keys",
                        "color": {"gradient": {"zero": {"red": 255, "green": 0, "blue": 0},
                                               "hundred": {"red": 0, "green": 255, "blue": 0}}},
                        "mode": "percent"
                    },
                    {"device-type": "toaster", "zone": "one", "color": {"red": 1, "green": 1, "blue": 1}},
                    stringified
                ]
            }),
        )
        .await;
    assert_eq!(r.status, 200, "{}", r.body);
    assert_eq!(r.body["bind_game_event"]["icon_id"], 1);
    let warnings = r.body["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].as_str().unwrap().contains("toaster"));

    s.post(
        "/bind_game_event",
        json!({"game": "ADVENTURE", "event": "X", "handlers": []}),
    )
    .await
    .assert_sdk_error(6);
    s.post("/bind_game_event", json!({"game": "ADVENTURE", "event": "X"}))
        .await
        .assert_sdk_error(6);
    s.post(
        "/bind_game_event",
        json!({"game": "adventure", "event": "X", "handlers": [{}]}),
    )
    .await
    .assert_sdk_error(2);

    s.post(
        "/game_event",
        json!({"game": "ADVENTURE", "event": "HEALTH", "data": {"value": 50}}),
    )
    .await;
    let fkeys = keyboard_zone_keys("function-keys").unwrap();
    let out = s.next().await;
    assert_eq!(
        lighting(&out),
        (
            "HEALTH",
            &LightTarget::Keys(fkeys[..6].to_vec()),
            Color::new(128, 128, 0),
            None
        )
    );
    let out = s.next().await;
    assert_eq!(lighting(&out).1, &LightTarget::Keys(fkeys[6..].to_vec()));
    assert_eq!(lighting(&out).2, Color::BLACK);
    let out = s.next().await;
    assert_eq!(lighting(&out).1, &LightTarget::Zone(1));
    assert_eq!(lighting(&out).2, Color::new(0, 0, 255));
    if let GameSenseOutput::Lighting { device_type, .. } = out {
        assert_eq!(device_type, DeviceType::RgbZoned(2));
    }
    s.assert_quiet();
    s.shutdown().await;
}

#[tokio::test]
async fn game_event_endpoint() {
    let mut s = TestServer::start().await;
    s.post("/bind_game_event", keyboard_bind("GAME", "HEALTH", "all", red()))
        .await;

    let r = s
        .post(
            "/game_event",
            json!({"game": "GAME", "event": "HEALTH", "data": {"value": 75}}),
        )
        .await;
    assert_eq!(r.status, 200);
    assert_eq!(r.body["game_event"]["data"]["value"], 75);
    let out = s.next().await;
    assert_eq!(lighting(&out).1, &LightTarget::AllZones);
    assert_eq!(lighting(&out).2, Color::new(255, 0, 0));

    // Same value again: cached, nothing to do.
    s.post(
        "/game_event",
        json!({"game": "GAME", "event": "HEALTH", "data": {"value": 75}}),
    )
    .await;
    s.assert_quiet();
    // No value on a normal event: ignored.
    s.post("/game_event", json!({"game": "GAME", "event": "HEALTH", "data": {}}))
        .await;
    s.assert_quiet();
    // Stringified data is accepted. Value 0 turns a static color off.
    let r = s
        .post(
            "/game_event",
            json!({"game": "GAME", "event": "HEALTH", "data": "{\"value\": 0}"}),
        )
        .await;
    assert_eq!(r.status, 200);
    assert_eq!(lighting(&s.next().await).2, Color::BLACK);

    s.post("/game_event", json!({"game": "GAME", "event": "HEALTH"}))
        .await
        .assert_sdk_error(4);
    s.post(
        "/game_event",
        json!({"game": "GAME", "event": "HEALTH", "data": "nope"}),
    )
    .await
    .assert_sdk_error(4);
    s.post("/game_event", json!({"game": "GAME", "data": {"value": 1}}))
        .await
        .assert_sdk_error(0);

    // value_optional events run their handlers on every update.
    s.post(
        "/bind_game_event",
        json!({"game": "GAME", "event": "CTX", "value_optional": true,
               "handlers": [{"device-type": "rgb-3-zone", "zone": "one", "mode": "context-color",
                             "context-frame-key": "c"}]}),
    )
    .await;
    for _ in 0..2 {
        s.post(
            "/game_event",
            json!({"game": "GAME", "event": "CTX", "data": {"frame": {"c": {"red": 0, "green": 9, "blue": 0}}}}),
        )
        .await;
        let out = s.next().await;
        assert_eq!(lighting(&out).1, &LightTarget::Zone(0));
        assert_eq!(lighting(&out).2, Color::new(0, 9, 0));
    }

    // Unregistered events are registered on the fly.
    let r = s
        .post(
            "/game_event",
            json!({"game": "GAME", "event": "NEW_ONE", "data": {"value": 3}}),
        )
        .await;
    assert_eq!(r.status, 200);
    s.assert_quiet();
    s.shutdown().await;
}

#[tokio::test]
async fn multiple_game_events_endpoint() {
    let mut s = TestServer::start().await;
    s.post("/bind_game_event", keyboard_bind("GAME", "HEALTH", "q", red()))
        .await;
    s.post(
        "/bind_game_event",
        keyboard_bind("GAME", "AMMO", "w", json!({"red": 0, "green": 0, "blue": 255})),
    )
    .await;

    let r = s.raw("GET", "/supports_multiple_game_events", &[], "").await;
    assert_eq!(r.status, 200);

    let r = s
        .post(
            "/multiple_game_events",
            json!({"game": "GAME", "events": [
                {"event": "HEALTH", "data": {"value": 75}},
                {"event": "AMMO", "data": {"value": 36, "frame": {"k": "v"}}}
            ]}),
        )
        .await;
    assert_eq!(r.status, 200, "{}", r.body);
    assert_eq!(r.body["multiple_game_events"]["events"], 2);
    let first = s.next().await;
    assert_eq!(lighting(&first).0, "HEALTH");
    assert_eq!(lighting(&first).1, &LightTarget::Keys(vec![KeyId::Q]));
    let second = s.next().await;
    assert_eq!(lighting(&second).0, "AMMO");
    assert_eq!(lighting(&second).1, &LightTarget::Keys(vec![KeyId::W]));

    s.post(
        "/multiple_game_events",
        json!({"game": "GAME", "events": [{"event": "HEALTH", "data": {"value": 1}}, {"event": "bad name", "data": {}}]}),
    )
    .await
    .assert_sdk_error(2);
    s.assert_quiet();
    s.post("/multiple_game_events", json!({"game": "GAME"}))
        .await
        .assert_sdk_error(0);
    s.post("/multiple_game_events", json!({"events": []}))
        .await
        .assert_sdk_error(1);
    s.shutdown().await;
}

#[tokio::test]
async fn heartbeat_keeps_game_alive_and_timeout_clears_it() {
    let server = GameSenseServer::new("127.0.0.1", 0)
        .unwrap()
        .with_heartbeat_timeout(Duration::from_millis(500));
    let mut s = TestServer::start_with(server).await;
    s.post("/bind_game_event", keyboard_bind("GAME", "HEALTH", "q", red()))
        .await;
    s.post(
        "/game_event",
        json!({"game": "GAME", "event": "HEALTH", "data": {"value": 10}}),
    )
    .await;
    lighting(&s.next().await);

    for _ in 0..8 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let r = s.post("/game_heartbeat", json!({"game": "GAME"})).await;
        assert_eq!(r.status, 200);
        assert_eq!(r.body["game_heartbeat"]["game"], "GAME");
    }
    s.assert_quiet();

    let cleared = s.next().await;
    assert_eq!(cleared, GameSenseOutput::Clear { game: "GAME".into() });

    // After the timeout the cached value is forgotten: the same value shows again.
    s.post(
        "/game_event",
        json!({"game": "GAME", "event": "HEALTH", "data": {"value": 10}}),
    )
    .await;
    lighting(&s.next().await);

    s.post("/game_heartbeat", json!({"game": "lower"}))
        .await
        .assert_sdk_error(3);
    let r = s.post("/game_heartbeat", json!({"game": "UNKNOWN"})).await;
    assert_eq!(r.status, 200);
    s.shutdown().await;
}

#[tokio::test]
async fn stop_game_endpoint() {
    let mut s = TestServer::start().await;
    s.post("/bind_game_event", keyboard_bind("GAME", "HEALTH", "q", red()))
        .await;
    s.post(
        "/game_event",
        json!({"game": "GAME", "event": "HEALTH", "data": {"value": 10}}),
    )
    .await;
    lighting(&s.next().await);

    let r = s.post("/stop_game", json!({"game": "GAME"})).await;
    assert_eq!(r.status, 200);
    assert_eq!(s.next().await, GameSenseOutput::Clear { game: "GAME".into() });

    // Stopping twice does nothing more.
    s.post("/stop_game", json!({"game": "GAME"})).await;
    s.assert_quiet();

    s.post(
        "/game_event",
        json!({"game": "GAME", "event": "HEALTH", "data": {"value": 10}}),
    )
    .await;
    lighting(&s.next().await);

    assert_eq!(s.post("/stop_game", json!({"game": "NEVER_SEEN"})).await.status, 200);
    s.post("/stop_game", json!({})).await.assert_sdk_error(1);
    s.shutdown().await;
}

#[tokio::test]
async fn remove_endpoints() {
    let mut s = TestServer::start().await;
    s.post("/bind_game_event", keyboard_bind("GAME", "HEALTH", "q", red()))
        .await;

    let r = s
        .post("/remove_game_event", json!({"game": "GAME", "event": "HEALTH"}))
        .await;
    assert_eq!(r.status, 200);
    s.post("/remove_game_event", json!({"game": "GAME", "event": "HEALTH"}))
        .await
        .assert_sdk_error(9);
    s.post(
        "/game_event",
        json!({"game": "GAME", "event": "HEALTH", "data": {"value": 10}}),
    )
    .await;
    s.assert_quiet();

    s.post("/bind_game_event", keyboard_bind("GAME", "AMMO", "w", red()))
        .await;
    s.post(
        "/game_event",
        json!({"game": "GAME", "event": "AMMO", "data": {"value": 10}}),
    )
    .await;
    lighting(&s.next().await);
    let r = s.post("/remove_game", json!({"game": "GAME"})).await;
    assert_eq!(r.status, 200);
    assert_eq!(s.next().await, GameSenseOutput::Clear { game: "GAME".into() });
    s.post("/remove_game", json!({"game": "GAME"}))
        .await
        .assert_sdk_error(10);
    s.post(
        "/game_event",
        json!({"game": "GAME", "event": "AMMO", "data": {"value": 10}}),
    )
    .await;
    s.assert_quiet();
    s.shutdown().await;
}

#[tokio::test]
async fn golisp_unknown_paths_and_methods() {
    let s = TestServer::start().await;
    let r = s
        .post(
            "/load_golisp_handlers",
            json!({"game": "GAME", "golisp": "(handler \"X\" (lambda (data) nil))"}),
        )
        .await;
    assert_eq!(r.status, 200);
    assert!(r.body["warnings"][0].as_str().unwrap().contains("GoLisp"));

    let r = s.post("/no_such_endpoint", json!({})).await;
    assert_eq!(r.status, 404);
    assert!(r.body["error"].is_string());

    let r = s.raw("GET", "/game_event", &[], "").await;
    assert_eq!(r.status, 405);

    let r = s.raw("GET", "/", &[], "").await;
    assert_eq!(r.status, 200);
    s.shutdown().await;
}

#[tokio::test]
async fn origin_policy_is_localhost_only() {
    let mut s = TestServer::start().await;
    let body = r#"{"game":"GAME"}"#;
    let evil = s
        .raw(
            "POST",
            "/game_metadata",
            &[("Content-Type", "application/json"), ("Origin", "http://evil.com")],
            body,
        )
        .await;
    assert_eq!(evil.status, 403);
    assert!(!evil.headers.contains("access-control-allow-origin"));

    let local = s
        .raw(
            "POST",
            "/game_metadata",
            &[
                ("Content-Type", "application/json"),
                ("Origin", "http://localhost:3000"),
            ],
            body,
        )
        .await;
    assert_eq!(local.status, 200);
    assert!(
        local
            .headers
            .contains("access-control-allow-origin: http://localhost:3000")
    );
    s.assert_quiet();
    s.shutdown().await;
}

#[tokio::test]
async fn screen_handler_end_to_end() {
    let mut s = TestServer::start().await;
    let r = s
        .post(
            "/bind_game_event",
            json!({
                "game": "GAME", "event": "KILLS",
                "handlers": [{
                    "device-type": "screened-128x40", "mode": "screen", "zone": "one",
                    "datas": [
                        {"has-text": true, "suffix": "Headshot!", "length-millis": 100, "arg": "", "icon-id": 7},
                        {"lines": [
                            {"has-text": true, "suffix": " kills"},
                            {"has-text": true, "arg": "(weapon: (context-frame: self))"},
                            {"has-progress-bar": true, "context-frame-key": "xp"}
                        ]}
                    ]
                }]
            }),
        )
        .await;
    assert_eq!(r.status, 200);
    s.post(
        "/game_event",
        json!({"game": "GAME", "event": "KILLS", "data": {"value": 15, "frame": {"weapon": "AWP", "xp": 40}}}),
    )
    .await;

    match s.next().await {
        GameSenseOutput::Screen {
            device_type,
            zone,
            size,
            content,
            duration,
            ..
        } => {
            assert_eq!(device_type, DeviceType::Screened(Some(ScreenSize::new(128, 40))));
            assert_eq!(zone, "one");
            assert_eq!(size, Some(ScreenSize::new(128, 40)));
            assert_eq!(duration, Some(Duration::from_millis(100)));
            assert_eq!(
                content,
                ScreenContent::Lines {
                    lines: vec![ScreenLine::Text {
                        text: "Headshot!".into(),
                        bold: false,
                        wrap: 0
                    }],
                    icon_id: Some(7)
                }
            );
        }
        other => panic!("expected screen, got {other:?}"),
    }
    match s.next().await {
        GameSenseOutput::Screen { content, duration, .. } => {
            assert_eq!(duration, None);
            assert_eq!(
                content,
                ScreenContent::Lines {
                    lines: vec![
                        ScreenLine::Text {
                            text: "15 kills".into(),
                            bold: false,
                            wrap: 0
                        },
                        ScreenLine::Text {
                            text: "AWP".into(),
                            bold: false,
                            wrap: 0
                        },
                        ScreenLine::ProgressBar { percent: 40 },
                    ],
                    icon_id: None
                }
            );
        }
        other => panic!("expected screen, got {other:?}"),
    }
    s.shutdown().await;
}

#[tokio::test]
async fn tactile_and_bitmap_end_to_end() {
    let mut s = TestServer::start().await;
    s.post(
        "/bind_game_event",
        json!({"game": "GAME", "event": "HIT", "handlers": [{
            "device-type": "tactile", "zone": "one", "mode": "vibrate",
            "pattern": [{"type": "ti_predefined_strongclick_100", "delay-ms": 150}, {"type": "custom", "length-ms": 250}],
            "rate": {"frequency": 2, "repeat_limit": 3}
        }]}),
    )
    .await;
    s.post(
        "/game_event",
        json!({"game": "GAME", "event": "HIT", "data": {"value": 1}}),
    )
    .await;
    match s.next().await {
        GameSenseOutput::Tactile {
            pattern, rate, zone, ..
        } => {
            assert_eq!(zone, "one");
            assert_eq!(
                pattern,
                vec![
                    TactileStep::Predefined {
                        name: "ti_predefined_strongclick_100".into(),
                        delay_ms: Some(150)
                    },
                    TactileStep::Custom {
                        length_ms: 250,
                        delay_ms: None
                    }
                ]
            );
            assert_eq!(
                rate,
                Some(Rate {
                    frequency_hz: 2.0,
                    repeat_limit: Some(3)
                })
            );
        }
        other => panic!("expected tactile, got {other:?}"),
    }

    s.post("/bind_game_event", keyboard_bind("GAME", "AMMO", "q", red()))
        .await;
    s.post(
        "/bind_game_event",
        json!({"game": "GAME", "event": "BG", "value_optional": true, "handlers": [{
            "device-type": "rgb-per-key-zones", "mode": "partial-bitmap", "excluded-events": ["AMMO"]
        }]}),
    )
    .await;
    let bitmap: Vec<Value> = (0..132).map(|i| json!([i, 0, 0])).collect();
    s.post(
        "/game_event",
        json!({"game": "GAME", "event": "BG", "data": {"frame": {"bitmap": bitmap}}}),
    )
    .await;
    match s.next().await {
        GameSenseOutput::Bitmap { colors, excluded, .. } => {
            assert_eq!(colors.len(), 132);
            assert_eq!(colors[131], Color::new(131, 0, 0));
            assert_eq!(excluded, vec![LightTarget::Keys(vec![KeyId::Q])]);
        }
        other => panic!("expected bitmap, got {other:?}"),
    }
    s.shutdown().await;
}

#[tokio::test]
async fn legacy_rgb_callback_still_works() {
    let server = GameSenseServer::new("127.0.0.1", 0).unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(String, u8, u8, u8)>();
    server
        .set_rgb_callback(move |zone: &str, r, g, b| {
            let _ = tx.send((zone.to_string(), r, g, b));
        })
        .await;
    let s = TestServer::start_with(server).await;
    s.post("/bind_game_event", keyboard_bind("GAME", "HEALTH", "all", red()))
        .await;
    s.post(
        "/game_event",
        json!({"game": "GAME", "event": "HEALTH", "data": {"value": 10}}),
    )
    .await;
    let got = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got, ("all".to_string(), 255, 0, 0));
    s.shutdown().await;
}

fn mkdirs(path: &Path) -> PathBuf {
    std::fs::create_dir_all(path).unwrap();
    path.to_path_buf()
}

fn engine_file(program_data: &Path) -> PathBuf {
    program_data
        .join("SteelSeries")
        .join("SteelSeries Engine 3")
        .join("coreProps.json")
}

#[test]
fn core_props_prefix_discovery_write_and_remove() {
    let home_dir = tempfile::tempdir().unwrap();
    let home = home_dir.path();

    let steamapps = home.join(".steam").join("steam").join("steamapps");
    let proton = mkdirs(&steamapps.join("compatdata/1091500/pfx/drive_c/ProgramData"));
    // A compatdata entry whose prefix was never initialised is skipped.
    mkdirs(&steamapps.join("compatdata/228980/pfx/drive_c"));

    let library = home.join("SteamLibrary");
    let library_prefix = mkdirs(&library.join("steamapps/compatdata/570/pfx/drive_c/ProgramData"));
    let escaped = library.display().to_string().replace('\\', "\\\\");
    std::fs::write(
        steamapps.join("libraryfolders.vdf"),
        format!("\"libraryfolders\"\n{{\n\t\"0\"\n\t{{\n\t\t\"path\"\t\t\"{escaped}\"\n\t}}\n}}\n"),
    )
    .unwrap();

    let wine = mkdirs(&home.join(".wine/drive_c/ProgramData"));
    let lutris = mkdirs(&home.join("Games/some-game/drive_c/ProgramData"));
    let bottles = mkdirs(&home.join(".local/share/bottles/bottles/MyBottle/drive_c/ProgramData"));
    // A folder that is not a prefix.
    mkdirs(&home.join("Games/not-a-prefix/data"));

    let mut found = discover_prefix_core_props(home);
    found.sort();
    let mut expected = vec![
        engine_file(&proton),
        engine_file(&library_prefix),
        engine_file(&wine),
        engine_file(&lutris),
        engine_file(&bottles),
    ];
    expected.sort();
    assert_eq!(found, expected);

    let written = write_core_props_to(27301, &found).unwrap();
    assert_eq!(written.len(), 5);
    for path in &written {
        let content: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(content["address"], "127.0.0.1:27301");
        assert_eq!(content["encrypted_address"], "");
        assert_eq!(content["ggEncryptedAddress"], "");
    }

    remove_core_props(&written);
    for path in &written {
        assert!(!path.exists());
    }
    assert!(
        !proton.join("SteelSeries").exists(),
        "empty SteelSeries folders are removed"
    );
    assert!(proton.exists(), "the prefix itself is untouched");

    // No prefixes at all: nothing found.
    let empty = tempfile::tempdir().unwrap();
    assert!(discover_prefix_core_props(empty.path()).is_empty());
}
