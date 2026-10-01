//! CLI front end for the engine: every user-facing command becomes an engine [`Command`],
//! sent to the running daemon when there is one and executed in-process otherwise.

pub mod daemon;
pub mod extras;

use colored::Colorize;
use serde_json::Value;

use steelseries_gg::devices::DeviceType;
use steelseries_gg::devices::settings::{SettingKind, SettingValue, Verification};
use steelseries_gg::engine::control;
use steelseries_gg::engine::{Command, Engine, EngineSnapshot, ProfileSummary, SettingsView};
use steelseries_gg::rgb::{Color, Effect, WaveDirection};
use steelseries_gg::{Error, Result};

/// Run `command` on the daemon if it is running, otherwise on a one-shot engine.
/// Returns the result and whether the daemon answered.
pub async fn call(command: Command) -> Result<(Value, bool)> {
    if let Some(value) = control::send(&command).await? {
        return Ok((value, true));
    }
    let renders = matches!(
        command,
        Command::Lighting { .. } | Command::ProfileLoad { .. } | Command::Refresh
    );
    let draws_oled = matches!(command, Command::Oled { .. });
    let engine = Engine::open(false).await?;
    let value = engine.execute(command).await?;
    if renders {
        engine.render_once().await;
    }
    if draws_oled {
        engine.render_oled_once().await;
    }
    engine.save().await;
    Ok((value, false))
}

fn decode<T: serde::de::DeserializeOwned>(value: Value) -> Result<T> {
    serde_json::from_value(value).map_err(Error::from)
}

/// `ssgg devices`
pub async fn devices() -> Result<()> {
    let (value, _) = call(Command::Status).await?;
    print_snapshot(&decode(value)?);
    Ok(())
}

pub fn print_snapshot(snapshot: &EngineSnapshot) {
    if snapshot.devices.is_empty() {
        println!("No SteelSeries devices connected.");
    }
    for device in &snapshot.devices {
        let kind = match device.kind {
            DeviceType::Keyboard => "keyboard",
            DeviceType::Mouse => "mouse",
            DeviceType::Headset => "headset",
            DeviceType::Unknown => "device",
        };
        println!("{}  {}  [{}]", device.name.bold(), kind.dimmed(), device.key);
        let mut facts = Vec::new();
        if let Some(percent) = device.status.battery_percent {
            let charging = if device.status.charging == Some(true) {
                " (charging)"
            } else {
                ""
            };
            facts.push(format!("battery {percent}%{charging}"));
        }
        if let Some(mix) = device.status.chatmix {
            facts.push(format!("ChatMix game {} / chat {}", mix.game, mix.chat));
        }
        if device.status.wireless_connected == Some(false) {
            facts.push("headset off / out of range".to_string());
        }
        if let Some(lighting) = &device.lighting {
            let source = if device.follows_global_lighting {
                "synced"
            } else {
                "own"
            };
            facts.push(format!(
                "lighting {} at {}% ({source})",
                describe_effect(&lighting.effect),
                lighting.brightness
            ));
        }
        if !device.capabilities.is_empty() {
            facts.push(format!("can: {}", device.capabilities.join(", ")));
        }
        for fact in facts {
            println!("    {fact}");
        }
    }
    if let Some(profile) = &snapshot.active_profile {
        println!("\nActive profile: {}", profile.green());
    }
    for warning in &snapshot.warnings {
        println!("{} {warning}", "warning:".yellow());
    }
    if !snapshot.daemon {
        println!(
            "\n{}",
            "Daemon not running: animated lighting, hot-plug and battery alerts need `systemctl --user start ssgg`."
                .dimmed()
        );
    }
}

pub fn describe_effect(effect: &Effect) -> String {
    match effect {
        Effect::Static { color } => format!("static {color}"),
        Effect::Breathing { color, speed } => format!("breathing {color} x{speed}"),
        Effect::Spectrum { speed } => format!("spectrum x{speed}"),
        Effect::Wave { speed, .. } => format!("wave x{speed}"),
        Effect::Reactive { color, .. } => format!("reactive {color}"),
        Effect::Gradient { start, end } => format!("gradient {start}→{end}"),
        Effect::Custom { colors } => format!("custom ({} zones)", colors.len()),
        Effect::Off => "off".to_string(),
    }
}

/// Build an effect from CLI words.
pub fn parse_effect(name: &str, color: Option<&str>, color2: Option<&str>, speed: f32) -> Result<Effect> {
    let color_or = |text: Option<&str>, fallback: Color| -> Result<Color> {
        match text {
            Some(t) => Color::parse(t).ok_or_else(|| Error::InvalidConfig(format!("invalid color '{t}'"))),
            None => Ok(fallback),
        }
    };
    let speed = speed.clamp(0.05, 10.0);
    Ok(match name.to_ascii_lowercase().as_str() {
        "static" | "solid" => Effect::Static {
            color: color_or(color, Color::WHITE)?,
        },
        "breathing" | "breathe" => Effect::Breathing {
            color: color_or(color, Color::PURPLE)?,
            speed,
        },
        "spectrum" | "rainbow" | "cycle" => Effect::Spectrum { speed },
        "wave" => Effect::Wave {
            colors: match color {
                Some(_) => vec![color_or(color, Color::RED)?, color_or(color2, Color::BLUE)?],
                None => Color::DEFAULT_COLORS.to_vec(),
            },
            speed,
            direction: WaveDirection::LeftToRight,
        },
        "reactive" => Effect::Reactive {
            color: color_or(color, Color::WHITE)?,
            duration: 1.0 / speed,
        },
        "gradient" => Effect::Gradient {
            start: color_or(color, Color::RED)?,
            end: color_or(color2, Color::BLUE)?,
        },
        "off" | "none" => Effect::Off,
        other => {
            return Err(Error::InvalidConfig(format!(
                "unknown effect '{other}' (static, breathing, spectrum, wave, reactive, gradient, off)"
            )));
        }
    })
}

/// `ssgg rgb color|brightness|effect|off` and `ssgg lighting ...`
pub async fn lighting(
    device: Option<String>,
    effect: Option<Effect>,
    brightness: Option<u8>,
    follow_global: bool,
) -> Result<()> {
    let animated = matches!(
        effect,
        Some(Effect::Breathing { .. } | Effect::Spectrum { .. } | Effect::Wave { .. } | Effect::Reactive { .. })
    );
    let target = device.clone().unwrap_or_else(|| "all devices".to_string());
    let (_, daemon) = call(Command::Lighting {
        device,
        effect: effect.clone(),
        brightness,
        follow_global,
    })
    .await?;
    if let Some(effect) = &effect {
        println!("Lighting on {target}: {}", describe_effect(effect));
    }
    if let Some(b) = brightness {
        println!("Brightness on {target}: {b}%");
    }
    if follow_global {
        println!("{target} now follows the synced lighting");
    }
    if animated && !daemon {
        println!(
            "{}",
            "Saved. Animated effects play while the daemon runs: systemctl --user start ssgg".dimmed()
        );
    }
    Ok(())
}

/// `ssgg settings <device>`
pub async fn settings(device: String) -> Result<()> {
    let (value, _) = call(Command::Settings { device }).await?;
    let view: SettingsView = decode(value)?;
    println!("{}  [{}]", view.name.bold(), view.device);
    if view.settings.is_empty() {
        println!("  This device exposes no settings yet.");
        return Ok(());
    }
    for entry in &view.settings {
        let d = &entry.descriptor;
        let marker = match d.verification {
            Verification::Hardware => "".normal(),
            Verification::Reference => " [untested]".yellow(),
            Verification::Guess => " [guess]".red(),
        };
        let current = entry
            .value
            .as_ref()
            .map(format_value)
            .unwrap_or_else(|| "-".to_string());
        println!(
            "  {:<22} {:<24} {}{}",
            d.id.cyan(),
            current,
            describe_kind(&d.kind).dimmed(),
            marker
        );
        if !d.description.is_empty() {
            println!("  {:<22} {}", "", d.description.dimmed());
        }
    }
    println!(
        "\n{}",
        "[untested] = taken from a published open-source driver; [guess] = extrapolated. Neither was tried on hardware."
            .dimmed()
    );
    Ok(())
}

pub fn format_value(value: &SettingValue) -> String {
    match value {
        SettingValue::Int(v) => v.to_string(),
        SettingValue::Choice(c) => c.clone(),
        SettingValue::Bool(b) => if *b { "on" } else { "off" }.to_string(),
        SettingValue::Color(c) => c.to_string(),
        SettingValue::Colors(cs) => cs.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(","),
        SettingValue::Dpi(d) => d.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(","),
        SettingValue::Gains(g) => g.iter().map(|v| format!("{v:+}")).collect::<Vec<_>>().join(","),
        SettingValue::Buttons(map) => map
            .iter()
            .map(|(b, a)| format!("{b}={a}"))
            .collect::<Vec<_>>()
            .join(","),
        SettingValue::Trigger => "(action)".to_string(),
    }
}

fn describe_kind(kind: &SettingKind) -> String {
    match kind {
        SettingKind::Range { min, max, step, unit } => {
            let unit = unit.as_deref().unwrap_or("");
            if *step > 1 {
                format!("{min}..{max}{unit} step {step}")
            } else {
                format!("{min}..{max}{unit}")
            }
        }
        SettingKind::Choice { options } => options.iter().map(|o| o.id.as_str()).collect::<Vec<_>>().join("|"),
        SettingKind::Toggle => "on|off".to_string(),
        SettingKind::Color => "color".to_string(),
        SettingKind::ColorZones { zones } => format!("colors: {}", zones.join(",")),
        SettingKind::DpiStages {
            min,
            max,
            step,
            max_stages,
        } => {
            format!("up to {max_stages} DPI values {min}..{max} step {step}")
        }
        SettingKind::Equalizer {
            bands_hz,
            min_db,
            max_db,
            ..
        } => format!("{} gains {min_db}..{max_db} dB", bands_hz.len()),
        SettingKind::ButtonMap { buttons, .. } => format!("button=action for {}", buttons.join(",")),
        SettingKind::Action => "run with value 'now'".to_string(),
    }
}

/// `ssgg set <device> <setting> <value>`
pub async fn set(device: String, setting: String, value: String) -> Result<()> {
    let (result, daemon) = call(Command::SetText {
        device,
        setting: setting.clone(),
        value: value.clone(),
    })
    .await?;
    let key = result["device"].as_str().unwrap_or("device");
    println!("{setting} = {value} on {key}");
    if !daemon {
        println!(
            "{}",
            "Remembered; the daemon re-applies it whenever the device reconnects.".dimmed()
        );
    }
    Ok(())
}

/// `ssgg profile list`
pub async fn profile_list() -> Result<()> {
    let (value, _) = call(Command::ProfileList).await?;
    let profiles: Vec<ProfileSummary> = decode(value)?;
    if profiles.is_empty() {
        println!("No profiles yet. Save one with: ssgg profile save <name>");
    }
    for p in profiles {
        let marker = if p.active { "*".green() } else { " ".normal() };
        let apps = if p.apps.is_empty() {
            String::new()
        } else {
            format!("  (auto: {})", p.apps.join(", "))
        };
        let description = p.description.map(|d| format!(" — {d}")).unwrap_or_default();
        println!("{marker} {}{description}{}", p.name.bold(), apps.dimmed());
    }
    Ok(())
}

pub async fn profile_command(command: Command, done: &str) -> Result<()> {
    call(command).await?;
    println!("{done}");
    Ok(())
}

/// `ssgg oled ...`
pub async fn oled(command: Command) -> Result<()> {
    let (_, daemon) = call(command).await?;
    println!("OLED updated.");
    if !daemon {
        println!(
            "{}",
            "Drawn once. Clocks, stats, animations and timed messages need the daemon: systemctl --user start ssgg"
                .dimmed()
        );
    }
    Ok(())
}

/// `ssgg moments ...`
pub async fn moments(command: Command) -> Result<()> {
    let saving = matches!(command, Command::MomentsSave);
    let (value, daemon) = call(command).await?;
    if saving {
        println!(
            "Clip saved to {}",
            value["saved_to"].as_str().unwrap_or("the Moments folder")
        );
        return Ok(());
    }
    let status: steelseries_gg::moments::MomentsStatus = decode(value)?;
    println!(
        "Replay buffer: {} ({} s), clips go to {}",
        if status.recording {
            "recording".green()
        } else if status.enabled {
            "enabled, not running".yellow()
        } else {
            "off".normal()
        },
        status.replay_seconds,
        status.output_dir
    );
    if let Some(error) = status.last_error {
        println!("{} {error}", "problem:".yellow());
    }
    if status.enabled && !daemon {
        println!(
            "{}",
            "The buffer runs inside the daemon: systemctl --user restart ssgg".dimmed()
        );
    }
    Ok(())
}

/// `ssgg ui` — open the control panel of the running daemon in a browser.
pub async fn open_ui() -> Result<()> {
    if control::send(&Command::Ping).await?.is_none() {
        return Err(Error::Other(
            "the daemon is not running; start it with `systemctl --user start ssgg` (or `ssgg daemon`)".to_string(),
        ));
    }
    let info = control::ControlInfo::read().ok_or_else(|| Error::Other("control file disappeared".to_string()))?;
    let url = info.panel_url();
    let opener = if cfg!(target_os = "windows") {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    match std::process::Command::new(opener).arg(&url).spawn() {
        Ok(_) => println!("Opened the control panel in your browser."),
        Err(_) => println!("Open this address in a browser:\n  {url}"),
    }
    Ok(())
}
