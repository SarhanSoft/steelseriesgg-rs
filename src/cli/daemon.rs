//! `ssgg daemon`: the long-running service (systemd user unit `ssgg.service`).

use std::sync::Arc;

use tokio::sync::{broadcast, watch};
use tracing::{debug, info, warn};

use steelseries_gg::Result;
use steelseries_gg::config::Config;
use steelseries_gg::engine::control::ControlServer;
use steelseries_gg::engine::{Command, Engine};
use steelseries_gg::gamesense::GameSenseServer;

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

    let (stop_tx, stop_rx) = watch::channel(false);
    let gamesense = if config.gamesense.enabled {
        Some(tokio::spawn(run_gamesense(config.clone(), engine.clone(), stop_rx)))
    } else {
        None
    };

    info!("Daemon running. Stop with Ctrl+C or `systemctl --user stop ssgg`.");
    crate::wait_for_shutdown().await?;

    // Let GameSense remove its coreProps.json files before exiting.
    let _ = stop_tx.send(true);
    if let Some(task) = gamesense
        && tokio::time::timeout(std::time::Duration::from_secs(3), task)
            .await
            .is_err()
    {
        warn!("GameSense server did not stop in time");
    }
    tasks.abort();
    if let Some(control) = control {
        control.stop();
    }
    engine.save().await;
    info!("Daemon stopped.");
    Ok(())
}

async fn run_gamesense(config: Config, engine: Arc<Engine>, mut stop: watch::Receiver<bool>) {
    let bind = config.gamesense.bind_address.clone();
    let port = config.gamesense.port;
    let server = match GameSenseServer::new(&bind, port) {
        Ok(server) => server,
        Err(e) => {
            warn!("GameSense server not started: {e}");
            return;
        }
    };

    let mut outputs = server.subscribe();
    let bridge = tokio::spawn(async move {
        loop {
            match outputs.recv().await {
                Ok(output) => engine.apply_gamesense(output).await,
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    debug!("GameSense bridge skipped {skipped} outputs")
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    let listener = match tokio::net::TcpListener::bind((bind.as_str(), port)).await {
        Ok(listener) => listener,
        Err(e) => {
            warn!("GameSense could not listen on {bind}:{port}: {e}");
            bridge.abort();
            return;
        }
    };
    let shutdown = async move {
        while !*stop.borrow() {
            if stop.changed().await.is_err() {
                break;
            }
        }
    };
    if let Err(e) = server.serve(listener, shutdown).await {
        warn!("GameSense server stopped: {e}");
    }
    bridge.abort();
}
