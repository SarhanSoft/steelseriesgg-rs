//! Persisted mixer settings (`mixer.toml` in the app config directory).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::Channel;
use super::chatmix::{chatmix_factors, scale_volume};
use super::eq::Eq;
use super::routing::RoutingConfig;
use crate::config::Config;
use crate::devices::settings::ChatMix;
use crate::{Error, Result};

/// A device choice: a PipeWire/Pulse node name, or `"auto"`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum DeviceSelection {
    /// Prefer a SteelSeries / Arctis device, else the system default.
    #[default]
    Auto,
    /// A specific sink or source name, e.g. `alsa_output.usb-SteelSeries_Arctis_Nova_7-00.analog-stereo`.
    Named(String),
}

impl DeviceSelection {
    pub fn named(&self) -> Option<&str> {
        match self {
            DeviceSelection::Auto => None,
            DeviceSelection::Named(name) => Some(name),
        }
    }

    /// `"auto"` (any case) or an empty string selects [`DeviceSelection::Auto`].
    pub fn parse(value: &str) -> Self {
        let trimmed = value.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("auto") {
            DeviceSelection::Auto
        } else {
            DeviceSelection::Named(trimmed.to_string())
        }
    }

    fn validate(&self, what: &str) -> Result<()> {
        if let DeviceSelection::Named(name) = self {
            if name.chars().any(char::is_control) {
                return Err(Error::InvalidConfig(format!(
                    "{what} device name contains control characters"
                )));
            }
        }
        Ok(())
    }
}

impl From<String> for DeviceSelection {
    fn from(value: String) -> Self {
        Self::parse(&value)
    }
}

impl From<DeviceSelection> for String {
    fn from(value: DeviceSelection) -> Self {
        match value {
            DeviceSelection::Auto => "auto".to_string(),
            DeviceSelection::Named(name) => name,
        }
    }
}

/// Volume and mute of one mix slot.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct Level {
    /// Percent, 0-100.
    pub volume: u8,
    pub muted: bool,
}

impl Default for Level {
    fn default() -> Self {
        Self {
            volume: 100,
            muted: false,
        }
    }
}

/// One virtual output channel.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct ChannelConfig {
    #[serde(flatten)]
    pub level: Level,
    pub eq: Eq,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct OutputChannels {
    pub game: ChannelConfig,
    pub chat: ChannelConfig,
    pub media: ChannelConfig,
    pub aux: ChannelConfig,
}

/// Noise suppression implementation for the microphone chain.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NoiseSuppressionBackend {
    /// RNNoise when `librnnoise_ladspa.so` is installed, else WebRTC.
    #[default]
    Auto,
    /// `libpipewire-module-echo-cancel` with the WebRTC audio processing library.
    Webrtc,
    /// The `noise_suppressor_mono` LADSPA plugin from noise-suppression-for-voice.
    Rnnoise,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct NoiseSuppression {
    pub enabled: bool,
    pub backend: NoiseSuppressionBackend,
    /// RNNoise voice-activity threshold, percent. Higher gates more aggressively.
    pub vad_threshold: u8,
}

impl Default for NoiseSuppression {
    fn default() -> Self {
        Self {
            enabled: false,
            backend: NoiseSuppressionBackend::Auto,
            vad_threshold: 50,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct MicConfig {
    /// Create the "SteelSeries Microphone" chain at all.
    pub enabled: bool,
    pub input: DeviceSelection,
    #[serde(flatten)]
    pub level: Level,
    pub noise_suppression: NoiseSuppression,
    pub eq: Eq,
}

impl Default for MicConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            input: DeviceSelection::Auto,
            level: Level::default(),
            noise_suppression: NoiseSuppression::default(),
            eq: Eq::default(),
        }
    }
}

/// Per-channel levels of the streaming mix (independent of what you hear).
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct StreamMix {
    pub game: Level,
    pub chat: Level,
    pub media: Level,
    pub aux: Level,
    pub mic: Level,
}

impl StreamMix {
    pub fn level(&self, channel: Channel) -> Level {
        match channel {
            Channel::Game => self.game,
            Channel::Chat => self.chat,
            Channel::Media => self.media,
            Channel::Aux => self.aux,
            Channel::Mic => self.mic,
        }
    }

    pub fn level_mut(&mut self, channel: Channel) -> &mut Level {
        match channel {
            Channel::Game => &mut self.game,
            Channel::Chat => &mut self.chat,
            Channel::Media => &mut self.media,
            Channel::Aux => &mut self.aux,
            Channel::Mic => &mut self.mic,
        }
    }
}

/// Streamer mode: the regular channel levels become the Monitoring mix, and a separate
/// "SteelSeries Stream" sink carries the Streaming mix for OBS.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct StreamerConfig {
    pub enabled: bool,
    pub mix: StreamMix,
}

/// Virtual surround on the Game channel via the PipeWire `sofa` spatializer.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct SpatialConfig {
    pub enabled: bool,
    /// HRTF in SOFA format, e.g. from the SADIE II or MIT KEMAR databases.
    pub sofa_path: Option<PathBuf>,
}

/// Everything the mixer needs to build and run its graph.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct MixerConfig {
    /// Physical sink every channel plays into.
    pub output: DeviceSelection,
    pub chatmix: ChatMix,
    pub channels: OutputChannels,
    pub mic: MicConfig,
    pub streamer: StreamerConfig,
    pub spatial: SpatialConfig,
    pub routing: RoutingConfig,
}

impl Default for MixerConfig {
    fn default() -> Self {
        Self {
            output: DeviceSelection::Auto,
            chatmix: ChatMix { game: 100, chat: 100 },
            channels: OutputChannels::default(),
            mic: MicConfig::default(),
            streamer: StreamerConfig::default(),
            spatial: SpatialConfig::default(),
            routing: RoutingConfig::default(),
        }
    }
}

impl MixerConfig {
    pub const FILE_NAME: &'static str = "mixer.toml";

    /// `<config dir>/mixer.toml`, next to `config.toml`.
    pub fn default_path() -> Option<PathBuf> {
        Config::config_dir().map(|dir| dir.join(Self::FILE_NAME))
    }

    /// Load from [`MixerConfig::default_path`]; defaults when the file does not exist.
    pub fn load() -> Result<Self> {
        match Self::default_path() {
            Some(path) => Self::load_from(&path),
            None => Ok(Self::default()),
        }
    }

    /// Load from `path`; defaults when the file does not exist.
    pub fn load_from(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::from_toml(&text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::default_path()
            .ok_or_else(|| Error::FileSystemError("could not determine the config directory".into()))?;
        self.save_to(&path)
    }

    /// Validate, then write `path` with owner-only permissions.
    pub fn save_to(&self, path: &Path) -> Result<()> {
        self.validate()?;
        if let Some(dir) = path.parent() {
            ensure_private_dir(dir)?;
        }
        crate::fs_utils::secure_write(path, self.to_toml()?)
    }

    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self).map_err(|e| Error::SerializationMessage(e.to_string()))
    }

    pub fn from_toml(text: &str) -> Result<Self> {
        let config: Self = toml::from_str(text)?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        let check_level = |what: &str, level: &Level| {
            if level.volume > 100 {
                Err(Error::InvalidConfig(format!(
                    "{what} volume {} is above 100",
                    level.volume
                )))
            } else {
                Ok(())
            }
        };
        self.output.validate("output")?;
        self.mic.input.validate("input")?;
        if self.chatmix.game > 100 || self.chatmix.chat > 100 {
            return Err(Error::InvalidConfig("ChatMix levels must be 0-100".into()));
        }
        for channel in Channel::OUTPUTS {
            if let Some(cfg) = self.channel(channel) {
                check_level(channel.key(), &cfg.level)?;
                cfg.eq.validate()?;
            }
        }
        check_level("mic", &self.mic.level)?;
        self.mic.eq.validate()?;
        if self.mic.noise_suppression.vad_threshold > 100 {
            return Err(Error::InvalidConfig(
                "noise suppression VAD threshold must be 0-100".into(),
            ));
        }
        for channel in Channel::ALL {
            check_level(&format!("stream {}", channel.key()), &self.streamer.mix.level(channel))?;
        }
        self.routing.validate()
    }

    /// Settings of an output channel; `None` for [`Channel::Mic`].
    pub fn channel(&self, channel: Channel) -> Option<&ChannelConfig> {
        match channel {
            Channel::Game => Some(&self.channels.game),
            Channel::Chat => Some(&self.channels.chat),
            Channel::Media => Some(&self.channels.media),
            Channel::Aux => Some(&self.channels.aux),
            Channel::Mic => None,
        }
    }

    pub fn channel_mut(&mut self, channel: Channel) -> Option<&mut ChannelConfig> {
        match channel {
            Channel::Game => Some(&mut self.channels.game),
            Channel::Chat => Some(&mut self.channels.chat),
            Channel::Media => Some(&mut self.channels.media),
            Channel::Aux => Some(&mut self.channels.aux),
            Channel::Mic => None,
        }
    }

    /// The user's volume and mute for any channel, including the mic.
    pub fn level(&self, channel: Channel) -> Level {
        match self.channel(channel) {
            Some(cfg) => cfg.level,
            None => self.mic.level,
        }
    }

    pub fn level_mut(&mut self, channel: Channel) -> &mut Level {
        match channel {
            Channel::Game => &mut self.channels.game.level,
            Channel::Chat => &mut self.channels.chat.level,
            Channel::Media => &mut self.channels.media.level,
            Channel::Aux => &mut self.channels.aux.level,
            Channel::Mic => &mut self.mic.level,
        }
    }

    pub fn eq(&self, channel: Channel) -> &Eq {
        match self.channel(channel) {
            Some(cfg) => &cfg.eq,
            None => &self.mic.eq,
        }
    }

    pub fn eq_mut(&mut self, channel: Channel) -> &mut Eq {
        match channel {
            Channel::Game => &mut self.channels.game.eq,
            Channel::Chat => &mut self.channels.chat.eq,
            Channel::Media => &mut self.channels.media.eq,
            Channel::Aux => &mut self.channels.aux.eq,
            Channel::Mic => &mut self.mic.eq,
        }
    }

    /// Volume actually set on the node: the user's volume with ChatMix applied to Game and Chat.
    pub fn effective_volume(&self, channel: Channel) -> u8 {
        let volume = self.level(channel).volume;
        let (game, chat) = chatmix_factors(self.chatmix);
        match channel {
            Channel::Game => scale_volume(volume, game),
            Channel::Chat => scale_volume(volume, chat),
            _ => volume.min(100),
        }
    }
}

/// Create `dir` (and parents) if missing; on Unix restrict it to the owner.
fn ensure_private_dir(dir: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

        match std::fs::symlink_metadata(dir) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
            }
            Ok(metadata) => {
                if !metadata.file_type().is_dir() {
                    return Err(Error::FileSystemError(format!("{} is not a directory", dir.display())));
                }
                let mut perms = metadata.permissions();
                if perms.mode() & 0o077 != 0 {
                    perms.set_mode(0o700);
                    std::fs::set_permissions(dir, perms)?;
                }
            }
            Err(e) => return Err(e.into()),
        }
    }
    #[cfg(not(unix))]
    {
        std::fs::create_dir_all(dir)?;
    }
    Ok(())
}

pub(crate) fn ensure_private_dir_for(path: &Path) -> Result<()> {
    match path.parent() {
        Some(dir) => ensure_private_dir(dir),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mixer::eq::EqPreset;
    use crate::mixer::routing::{MatchField, RoutingRule};

    #[test]
    fn defaults_are_valid_and_sensible() {
        let cfg = MixerConfig::default();
        cfg.validate().unwrap();
        assert_eq!(cfg.output, DeviceSelection::Auto);
        assert_eq!(cfg.level(Channel::Game).volume, 100);
        assert!(cfg.mic.enabled);
        assert!(!cfg.streamer.enabled);
        assert!(!cfg.spatial.enabled);
        assert_eq!(cfg.eq(Channel::Chat).bands.len(), 10);
    }

    #[test]
    fn toml_round_trip_preserves_everything() {
        let mut cfg = MixerConfig {
            output: DeviceSelection::Named("alsa_output.usb-SteelSeries_Arctis_Nova_7-00.analog-stereo".into()),
            chatmix: ChatMix { game: 100, chat: 35 },
            ..MixerConfig::default()
        };
        cfg.channels.game.eq = Eq::from_preset(EqPreset::FpsFootsteps);
        cfg.channels.media.level = Level {
            volume: 42,
            muted: true,
        };
        cfg.mic.noise_suppression = NoiseSuppression {
            enabled: true,
            backend: NoiseSuppressionBackend::Rnnoise,
            vad_threshold: 70,
        };
        cfg.mic.input = DeviceSelection::Named("alsa_input.usb-mic".into());
        cfg.streamer.enabled = true;
        cfg.streamer.mix.chat = Level { volume: 0, muted: true };
        cfg.spatial = SpatialConfig {
            enabled: true,
            sofa_path: Some(PathBuf::from("/usr/share/sofa/hrtf.sofa")),
        };
        cfg.routing
            .upsert(RoutingRule::new(MatchField::ProcessBinary, "cs2", Channel::Aux));

        let text = cfg.to_toml().unwrap();
        let back = MixerConfig::from_toml(&text).unwrap();
        assert_eq!(back, cfg, "TOML:\n{text}");
        assert!(text.contains("output = \"alsa_output.usb-SteelSeries_Arctis_Nova_7-00.analog-stereo\""));

        let json = serde_json::to_string(&cfg).unwrap();
        let back: MixerConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cfg);
    }

    #[test]
    fn partial_files_fill_in_defaults() {
        let cfg = MixerConfig::from_toml("output = \"auto\"\n[channels.chat]\nvolume = 30\n").unwrap();
        assert_eq!(cfg.level(Channel::Chat).volume, 30);
        assert!(!cfg.level(Channel::Chat).muted);
        assert_eq!(cfg.level(Channel::Game).volume, 100);
        assert_eq!(cfg.routing, RoutingConfig::default());
        assert_eq!(cfg.eq(Channel::Chat).bands.len(), 10);
    }

    #[test]
    fn validation_rejects_bad_values() {
        let mut cfg = MixerConfig::default();
        cfg.channels.aux.level.volume = 101;
        assert!(cfg.validate().is_err());
        let mut cfg = MixerConfig::default();
        cfg.streamer.mix.mic.volume = 200;
        assert!(cfg.validate().is_err());
        let cfg = MixerConfig {
            output: DeviceSelection::Named("bad\nname".into()),
            ..MixerConfig::default()
        };
        assert!(cfg.validate().is_err());
        assert!(MixerConfig::from_toml("[channels.game]\nvolume = 150\n").is_err());
    }

    #[test]
    fn effective_volume_applies_chatmix_to_game_and_chat_only() {
        let mut cfg = MixerConfig::default();
        cfg.channels.game.level.volume = 80;
        cfg.channels.chat.level.volume = 60;
        cfg.channels.media.level.volume = 50;
        cfg.chatmix = ChatMix { game: 100, chat: 50 };
        assert_eq!(cfg.effective_volume(Channel::Game), 80);
        assert_eq!(cfg.effective_volume(Channel::Chat), 30);
        assert_eq!(cfg.effective_volume(Channel::Media), 50);
        cfg.chatmix = ChatMix { game: 25, chat: 100 };
        assert_eq!(cfg.effective_volume(Channel::Game), 20);
        assert_eq!(cfg.effective_volume(Channel::Chat), 60);
    }

    #[test]
    fn device_selection_parsing() {
        assert_eq!(DeviceSelection::parse("AUTO"), DeviceSelection::Auto);
        assert_eq!(DeviceSelection::parse(""), DeviceSelection::Auto);
        assert_eq!(DeviceSelection::parse(" sink "), DeviceSelection::Named("sink".into()));
    }

    #[test]
    fn save_and_load_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join(MixerConfig::FILE_NAME);
        assert_eq!(MixerConfig::load_from(&path).unwrap(), MixerConfig::default());
        let mut cfg = MixerConfig::default();
        cfg.channels.chat.level.volume = 12;
        cfg.save_to(&path).unwrap();
        assert_eq!(MixerConfig::load_from(&path).unwrap(), cfg);
    }
}
