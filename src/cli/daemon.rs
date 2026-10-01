//! `ssgg daemon`: the long-running service (systemd user unit `ssgg.service`).

use std::time::{Duration, Instant};

use tracing::{info, warn};

use steelseries_gg::Result;
use steelseries_gg::config::Config;
use steelseries_gg::devices::DeviceType;
use steelseries_gg::engine::control::ControlServer;
use steelseries_gg::engine::{Command, Engine, Overlay, OverlayTarget};
use steelseries_gg::gamesense::GameSenseServer;
use steelseries_gg::rgb::Color;

/// How long a GameSense colour stays without a new event (the SDK's heartbeat window).
const GAMESENSE_HOLD: Duration = Duration::from_secs(15);

pub async fn run() -> Result<()> {
    info!("Starting SteelSeries GG daemon {}", env!("CARGO_PKG_VERSION"));
    let config = match Config::load_async().await {
        Ok(config) => config,
        Err(e) => {
            warn!("config.toml is invalid ({e}); using defaults");
            Config::default()
        }
    };

    let engine = Engine::open(true).await?;
    let tasks = engine.spawn_background();

    let control = if config.control.enabled {
        match ControlServer::start(engine.clone(), config.control.port).await {
            Ok(server) => Some(server),
            Err(e) => {
                warn!("Control API unavailable: {e}");
                None
            }
        }
    } else {
        None
    };

    if let Some(name) = &config.default_profile
        && let Err(e) = engine.execute(Command::ProfileLoad { name: name.clone() }).await
    {
        warn!("Default profile '{name}' not applied: {e}");
    }

    if config.gamesense.enabled {
        start_gamesense(&config, engine.clone());
    }

    info!("Daemon running. Stop with Ctrl+C or `systemctl --user stop ssgg`.");
    crate::wait_for_shutdown().await?;

    tasks.abort();
    if let Some(control) = control {
        control.stop();
    }
    engine.save().await;
    info!("Daemon stopped.");
    Ok(())
}

fn start_gamesense(config: &Config, engine: std::sync::Arc<Engine>) {
    let bind = config.gamesense.bind_address.clone();
    let port = config.gamesense.port;
    tokio::spawn(async move {
        let server = match GameSenseServer::new(&bind, port) {
            Ok(server) => server,
            Err(e) => {
                warn!("GameSense server not started: {e}");
                return;
            }
        };
        let handle = tokio::runtime::Handle::current();
        server
            .set_rgb_callback(move |zone: &str, r: u8, g: u8, b: u8| {
                let engine = engine.clone();
                let zone_index = crate::parse_zone_number(zone);
                handle.spawn(async move {
                    engine
                        .push_overlay(Overlay {
                            target: OverlayTarget::Kind(DeviceType::Keyboard),
                            zone: zone_index,
                            color: Color::new(r, g, b),
                            expires: Instant::now() + GAMESENSE_HOLD,
                            flash_hz: None,
                        })
                        .await;
                });
            })
            .await;
        if let Err(e) = server.run().await {
            warn!("GameSense server stopped: {e}");
        }
    });
}
