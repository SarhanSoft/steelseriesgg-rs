//! What the engine remembers between runs: the last value applied to every device setting,
//! per-device and global lighting, and the active profile.
//!
//! Most SteelSeries devices cannot report their current settings, so this file is the source
//! of truth for "current value" in the CLI and UI, and what gets re-applied when a device that
//! forgets its settings on power loss is plugged back in.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::devices::settings::SettingValue;
use crate::rgb::{Color, Effect};
use crate::{Error, Result};

const STATE_FILE: &str = "engine-state.json";
const STATE_VERSION: u32 = 1;

/// Lighting for one device, or the global (synced) lighting.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Lighting {
    pub effect: Effect,
    /// 0-100, applied in software on top of the effect.
    pub brightness: u8,
}

impl Default for Lighting {
    fn default() -> Self {
        Self {
            effect: Effect::Static {
                color: Color::new(255, 96, 0),
            },
            brightness: 100,
        }
    }
}

/// Remembered state for one physical device.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct DeviceRecord {
    /// Display name when last seen, so the UI can list offline devices.
    pub name: String,
    /// Last value applied per setting id.
    #[serde(default)]
    pub settings: BTreeMap<String, SettingValue>,
    /// `None` = follow the global lighting.
    #[serde(default)]
    pub lighting: Option<Lighting>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EngineState {
    pub version: u32,
    #[serde(default)]
    pub devices: BTreeMap<String, DeviceRecord>,
    #[serde(default)]
    pub lighting: Lighting,
    #[serde(default)]
    pub active_profile: Option<String>,
}

impl Default for EngineState {
    fn default() -> Self {
        Self {
            version: STATE_VERSION,
            devices: BTreeMap::new(),
            lighting: Lighting::default(),
            active_profile: None,
        }
    }
}

impl EngineState {
    pub fn path() -> Result<PathBuf> {
        Config::config_dir()
            .map(|dir| dir.join(STATE_FILE))
            .ok_or_else(|| Error::InvalidConfig("could not determine config directory".to_string()))
    }

    /// Load the state file; a missing or unreadable file yields the defaults (logged).
    pub fn load() -> Self {
        let Ok(path) = Self::path() else {
            return Self::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<EngineState>(&text) {
                Ok(state) => state,
                Err(e) => {
                    tracing::warn!("Ignoring unreadable {}: {e}", path.display());
                    Self::default()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(e) => {
                tracing::warn!("Could not read {}: {e}", path.display());
                Self::default()
            }
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self)?;
        crate::fs_utils::secure_write(&path, text)
    }

    pub fn record_mut(&mut self, key: &str, name: &str) -> &mut DeviceRecord {
        let record = self.devices.entry(key.to_string()).or_default();
        if record.name != name {
            record.name = name.to_string();
        }
        record
    }

    /// Lighting that applies to `key`: its own override, else the global lighting.
    pub fn effective_lighting(&self, key: &str) -> &Lighting {
        self.devices
            .get(key)
            .and_then(|r| r.lighting.as_ref())
            .unwrap_or(&self.lighting)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_lighting_prefers_device_override() {
        let mut state = EngineState::default();
        let global = state.lighting.clone();
        assert_eq!(state.effective_lighting("a"), &global);

        let own = Lighting {
            effect: Effect::Off,
            brightness: 10,
        };
        state.record_mut("a", "Apex").lighting = Some(own.clone());
        assert_eq!(state.effective_lighting("a"), &own);
        assert_eq!(state.effective_lighting("b"), &global);
    }

    #[test]
    fn state_round_trips_through_json() {
        let mut state = EngineState::default();
        state
            .record_mut("1038:1628:0", "Apex Pro TKL")
            .settings
            .insert("actuation".into(), SettingValue::Int(15));
        state.active_profile = Some("fps".into());
        let json = serde_json::to_string(&state).unwrap();
        let back: EngineState = serde_json::from_str(&json).unwrap();
        assert_eq!(back.active_profile.as_deref(), Some("fps"));
        assert_eq!(back.devices["1038:1628:0"].settings["actuation"], SettingValue::Int(15));
    }
}
