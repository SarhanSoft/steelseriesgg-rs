//! CLI for the audio mixer (`ssgg mixer`) and key bindings (`ssgg bind`, `ssgg macro`).

use clap::Subcommand;
use colored::Colorize;
use serde_json::{Value, json};

use steelseries_gg::devices::settings::parse_bool;
use steelseries_gg::engine::Command;
use steelseries_gg::engine::audio::{MixerPatch, MixerView};
use steelseries_gg::input::{Binding, BindingSet, InputKey};
use steelseries_gg::mixer::{Channel, MatchField, RoutingRule};
use steelseries_gg::{Error, Result};

use super::call;

#[derive(Subcommand)]
pub enum MixerAction {
    /// Show channels, devices, routed applications and problems
    Status,
    /// Create the SteelSeries Game/Chat/Media/Aux/Microphone devices (runs in the daemon)
    Enable,
    /// Remove them and restore the previous default devices
    Disable,
    /// Channel volume 0-100 (game, chat, media, aux, mic)
    Volume {
        channel: Channel,
        level: u8,
    },
    Mute {
        channel: Channel,
    },
    Unmute {
        channel: Channel,
    },
    /// Game/chat balance: -100 = all game, 0 = both full, 100 = all chat
    Chatmix {
        #[arg(allow_hyphen_values = true)]
        balance: i8,
    },
    /// Equalizer: a preset name, or one gain in dB per band (comma-separated)
    Eq {
        channel: Channel,
        #[arg(allow_hyphen_values = true)]
        preset_or_gains: String,
    },
    /// Headset (or speakers) every channel plays into; `auto` prefers a SteelSeries device
    Output {
        device: String,
    },
    /// Physical microphone for the SteelSeries Microphone; `auto` prefers a SteelSeries device
    Input {
        device: String,
    },
    /// Send an application to a channel, e.g. `ssgg mixer route discord chat`
    Route {
        app: String,
        channel: Channel,
    },
    /// Forget the routing rule for an application
    Unroute {
        app: String,
    },
    /// Streamer mode: a separate "SteelSeries Stream" mix for OBS
    Streamer {
        state: String,
    },
    /// Volume of a channel in the streaming mix
    StreamVolume {
        channel: Channel,
        level: u8,
    },
    /// Microphone noise suppression on/off
    Noise {
        state: String,
    },
    /// Virtual surround on the Game channel on/off (needs a SOFA file in mixer.toml)
    Spatial {
        state: String,
    },
}

fn toggle(state: &str) -> Result<bool> {
    parse_bool(state).ok_or_else(|| Error::InvalidConfig(format!("expected on or off, got '{state}'")))
}

pub async fn mixer(action: MixerAction) -> Result<()> {
    let mut patch = MixerPatch::default();
    let mut enabled = None;
    match action {
        MixerAction::Status => {}
        MixerAction::Enable => enabled = Some(true),
        MixerAction::Disable => enabled = Some(false),
        MixerAction::Volume { channel, level } => patch.volume = Some((channel, level.min(100))),
        MixerAction::Mute { channel } => patch.mute = Some((channel, true)),
        MixerAction::Unmute { channel } => patch.mute = Some((channel, false)),
        MixerAction::Chatmix { balance } => patch.chatmix_balance = Some(balance.clamp(-100, 100)),
        MixerAction::Eq {
            channel,
            preset_or_gains,
        } => {
            if preset_or_gains.contains(',') || preset_or_gains.parse::<f32>().is_ok() {
                let gains = preset_or_gains
                    .split(',')
                    .map(|g| g.trim().parse::<f32>())
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|_| Error::InvalidConfig(format!("bad gain list '{preset_or_gains}'")))?;
                patch.eq_gains = Some((channel, gains));
            } else {
                patch.eq_preset = Some((channel, preset_or_gains));
            }
        }
        MixerAction::Output { device } => patch.output = Some(device),
        MixerAction::Input { device } => patch.input = Some(device),
        MixerAction::Route { app, channel } => {
            patch.route = Some(RoutingRule::new(MatchField::default(), app, channel));
        }
        MixerAction::Unroute { app } => patch.unroute = Some(app),
        MixerAction::Streamer { state } => patch.streamer = Some(toggle(&state)?),
        MixerAction::StreamVolume { channel, level } => patch.stream_volume = Some((channel, level.min(100))),
        MixerAction::Noise { state } => patch.noise_suppression = Some(toggle(&state)?),
        MixerAction::Spatial { state } => patch.spatial = Some(toggle(&state)?),
    }
    let has_patch = serde_json::to_value(&patch)?
        .as_object()
        .is_some_and(|o| o.values().any(|v| !v.is_null()));
    let (value, _) = call(Command::Mixer {
        patch: has_patch.then_some(patch),
        enabled,
    })
    .await?;
    print_mixer(&serde_json::from_value(value)?);
    Ok(())
}

fn print_mixer(view: &MixerView) {
    let state = if view.running {
        "running".green()
    } else if view.enabled {
        "enabled, not running".yellow()
    } else {
        "off".normal()
    };
    println!("Audio mixer: {state}");
    let cfg = &view.config;
    let output: String = cfg.output.clone().into();
    println!("  Output: {output}");
    for channel in Channel::ALL {
        let level = cfg.level(channel);
        let eq = cfg.eq(channel).preset.clone().unwrap_or_else(|| "custom".to_string());
        println!(
            "  {:<6} {:>3}%{}  EQ: {eq}",
            channel.key(),
            level.volume,
            if level.muted { " (muted)" } else { "" }
        );
    }
    println!("  ChatMix: game {} / chat {}", cfg.chatmix.game, cfg.chatmix.chat);
    println!(
        "  Mic noise suppression: {}   Streamer mode: {}   Spatial: {}",
        if cfg.mic.noise_suppression.enabled { "on" } else { "off" },
        if cfg.streamer.enabled { "on" } else { "off" },
        if cfg.spatial.enabled { "on" } else { "off" },
    );
    if let Some(status) = &view.status
        && !status.apps.is_empty()
    {
        println!("  Applications:");
        for app in &status.apps {
            println!("    {app:?}");
        }
    }
    if !cfg.routing.rules.is_empty() {
        let rules: Vec<String> = cfg
            .routing
            .rules
            .iter()
            .map(|r| format!("{}→{}", r.pattern, r.channel))
            .collect();
        println!("  Routing: {}", rules.join(", "));
    }
    println!("  EQ presets: {}", view.presets.join(", "));
    for warning in &view.warnings {
        println!("{} {warning}", "note:".yellow());
    }
}

/// Build a binding action from CLI words, e.g. `key ctrl`, `text "Hello"`, `launch firefox`.
pub fn parse_action(words: &[String]) -> Result<Value> {
    let (kind, rest) = words.split_first().ok_or_else(|| {
        Error::InvalidConfig(
            "missing action (key, combo, text, launch, media, mouse, profile, macro, disabled)".to_string(),
        )
    })?;
    let joined = rest.join(" ");
    let need = |what: &str| -> Result<()> {
        if rest.is_empty() {
            Err(Error::InvalidConfig(format!("`{kind}` needs {what}")))
        } else {
            Ok(())
        }
    };
    Ok(match kind.to_ascii_lowercase().as_str() {
        "disabled" | "disable" | "off" | "none" => json!("disabled"),
        "passthrough" | "default" => json!("passthrough"),
        "key" => {
            need("a key name")?;
            json!({ "key": joined })
        }
        "combo" => {
            need("keys like ctrl+c")?;
            let keys: Vec<&str> = joined.split('+').map(str::trim).filter(|k| !k.is_empty()).collect();
            json!({ "combo": keys })
        }
        "text" | "type" => {
            need("the text to type")?;
            json!({ "text": joined.replace("\\n", "\n") })
        }
        "launch" | "run" => {
            need("a program")?;
            json!({ "launch": { "command": rest[0], "args": rest[1..].to_vec() } })
        }
        "media" => {
            need("play_pause, next, previous, volume_up, volume_down or mute")?;
            json!({ "media": joined })
        }
        "mouse" => {
            need("left, right, middle, side or extra")?;
            json!({ "mouse_button": joined })
        }
        "profile" => {
            need("a profile name")?;
            json!({ "switch_profile": joined })
        }
        "macro" => {
            need("steps like tap:KEY_H delay:50 tap:KEY_I")?;
            let mut steps = Vec::new();
            for step in rest {
                let (op, arg) = step.split_once(':').ok_or_else(|| {
                    Error::InvalidConfig(format!("bad macro step '{step}' (use tap:, press:, release:, delay:)"))
                })?;
                steps.push(match op {
                    "delay" => json!({ "delay": arg.parse::<u64>().map_err(|_| Error::InvalidConfig(format!("bad delay '{arg}'")))? }),
                    "tap" | "press" | "release" => json!({ op: arg }),
                    other => return Err(Error::InvalidConfig(format!("unknown macro step '{other}'"))),
                });
            }
            json!({ "macro": { "steps": steps, "repeat": 1, "mode": "once" } })
        }
        other => return Err(Error::InvalidConfig(format!("unknown action '{other}'"))),
    })
}

pub async fn bind(key: String, action: Vec<String>, profile: Option<String>, on_release: bool) -> Result<()> {
    let source = InputKey::parse(&key)?;
    let mut binding = json!({ "source": source, "action": parse_action(&action)? });
    if on_release {
        binding["mode"] = json!("release");
    }
    let binding: Binding =
        serde_json::from_value(binding).map_err(|e| Error::InvalidConfig(format!("invalid binding: {e}")))?;
    let (value, daemon) = call(Command::Bind { profile, binding }).await?;
    println!(
        "{} bound in profile {}",
        source,
        value["profile"].as_str().unwrap_or("?")
    );
    if !daemon {
        println!(
            "{}",
            "Bindings take effect while the daemon runs: systemctl --user start ssgg".dimmed()
        );
    }
    Ok(())
}

pub async fn unbind(key: String, profile: Option<String>) -> Result<()> {
    let source = InputKey::parse(&key)?;
    let (value, _) = call(Command::Unbind { profile, source }).await?;
    println!(
        "{} restored in profile {}",
        source,
        value["profile"].as_str().unwrap_or("?")
    );
    Ok(())
}

pub async fn bindings(profile: Option<String>) -> Result<()> {
    let (value, _) = call(Command::Bindings { profile }).await?;
    let name = value["profile"].as_str().unwrap_or("(no active profile)").to_string();
    let set: BindingSet = serde_json::from_value(value["bindings"].clone())?;
    println!("Key bindings — profile {}", name.bold());
    if set.bindings.is_empty() {
        println!("  none. Example: ssgg bind capslock key ctrl");
    }
    for binding in &set.bindings {
        let action = serde_json::to_string(&binding.action)?;
        println!("  {:<16} {action}", binding.source.to_string().cyan());
    }
    if let Some(error) = value["error"].as_str() {
        println!("{} {error}", "problem:".yellow());
    } else if value["running"].as_bool() == Some(false) && !set.bindings.is_empty() {
        println!(
            "{}",
            "Not active right now (daemon not running, or profile not active).".dimmed()
        );
    }
    Ok(())
}

pub async fn record_macro(key: String, stop_key: Option<String>, timeout: u32, profile: Option<String>) -> Result<()> {
    let source = InputKey::parse(&key)?;
    let stop_key = stop_key.map(|k| InputKey::parse(&k)).transpose()?;
    println!(
        "Recording from SteelSeries devices: press {} to stop (or wait {timeout} s)...",
        stop_key.map(|k| k.to_string()).unwrap_or_else(|| "Esc".to_string())
    );
    let (value, _) = call(Command::MacroRecord {
        stop_key,
        timeout_secs: Some(timeout),
    })
    .await?;
    let steps = value["steps"].clone();
    let count = steps.as_array().map_or(0, Vec::len);
    if count == 0 {
        return Err(Error::Other("nothing was recorded".to_string()));
    }
    let binding: Binding = serde_json::from_value(json!({
        "source": source,
        "action": { "macro": { "steps": steps, "repeat": 1, "mode": "once" } }
    }))?;
    let (value, _) = call(Command::Bind { profile, binding }).await?;
    println!(
        "Recorded {count} step(s); {} plays them (profile {}).",
        source,
        value["profile"].as_str().unwrap_or("?")
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_string).collect()
    }

    #[test]
    fn actions_deserialize_into_bindings() {
        for spec in [
            "key ctrl",
            "combo ctrl+shift+t",
            "text hello world",
            "launch firefox --new-window",
            "media play_pause",
            "mouse middle",
            "profile gaming",
            "disabled",
            "macro tap:KEY_H delay:50 tap:KEY_I",
        ] {
            let action = parse_action(&words(spec)).unwrap();
            let binding = json!({ "source": "KEY_F13", "action": action });
            let parsed: std::result::Result<Binding, _> = serde_json::from_value(binding);
            assert!(parsed.is_ok(), "{spec}: {:?}", parsed.err());
        }
    }

    #[test]
    fn bad_actions_are_rejected() {
        assert!(parse_action(&words("fly away")).is_err());
        assert!(parse_action(&words("key")).is_err());
        assert!(parse_action(&words("macro jump:1")).is_err());
    }
}
