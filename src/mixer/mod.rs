//! Sonar-style audio mixer for PipeWire.
//!
//! Recreates SteelSeries Sonar on Linux: virtual Game / Chat / Media / Aux sinks and a
//! Microphone source, each with volume, mute and a 10-band parametric EQ; all outputs mixed
//! into one physical device; ChatMix; microphone noise suppression; a separate streaming mix;
//! per-application routing; and optional virtual surround.
//!
//! The mixer drives external tools at runtime instead of linking to PipeWire. It writes one
//! standalone PipeWire config (see [`pwconf`]) and runs it as a child process
//! (`pipewire -c <file>`); volumes, mutes, defaults and stream moves go through `pactl`. Every
//! process call goes through [`CommandRunner`], so the logic is tested without a live audio
//! system. See `docs/development/mixer.md`.
//!
//! **Not yet run against a real PipeWire system.** The generated config and command lines
//! follow the PipeWire documentation and shipped example configs, but have only been verified
//! by unit tests.

pub mod chatmix;
pub mod config;
pub mod detect;
pub mod engine;
pub mod eq;
pub mod pactl;
pub mod pwconf;
pub mod routing;
pub mod runner;

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

pub use crate::devices::settings::ChatMix;
pub use chatmix::{chatmix_factors, chatmix_from_balance};
pub use config::{
    ChannelConfig, DeviceSelection, Level, MicConfig, MixerConfig, NoiseSuppression, NoiseSuppressionBackend,
    OutputChannels, SpatialConfig, StreamMix, StreamerConfig,
};
pub use detect::{DetectReport, ToolAvailability};
pub use engine::{
    AppStream, AudioDevice, ChannelStatus, MicStatus, Mixer, MixerPaths, MixerStatus, MixerTiming, StreamerStatus,
};
pub use eq::{BandType, Eq, EqBand, EqPreset};
pub use pwconf::{GraphPlan, generate_pipewire_config};
pub use routing::{AppIdentity, MatchField, RoutingConfig, RoutingRule};
pub use runner::{ChildHandle, CommandOutput, CommandRunner, FakeOutcome, FakeRunner, Invocation, SystemRunner};

/// PipeWire node names created by the mixer. Every name starts with [`nodes::PREFIX`], which is
/// how stale nodes and the mixer's own streams are recognised.
pub mod nodes {
    pub const PREFIX: &str = "ssgg_";

    pub const GAME: &str = "ssgg_game";
    pub const CHAT: &str = "ssgg_chat";
    pub const MEDIA: &str = "ssgg_media";
    pub const AUX: &str = "ssgg_aux";
    /// Virtual microphone source applications record from.
    pub const MIC: &str = "ssgg_mic";
    /// Filter-chain capture stream of the microphone chain (reads the physical mic or the
    /// noise-suppression stage).
    pub const MIC_IN: &str = "ssgg_mic_in";
    /// WebRTC echo-cancel stage (noise suppression backend `webrtc`).
    pub const MIC_NS_IN: &str = "ssgg_mic_ns_in";
    pub const MIC_NS: &str = "ssgg_mic_ns";
    pub const MIC_NS_REF: &str = "ssgg_mic_ns_ref";
    pub const MIC_NS_REF_OUT: &str = "ssgg_mic_ns_ref_out";
    /// Streaming mix sink and its source side (streamer mode).
    pub const STREAM: &str = "ssgg_stream";
    pub const STREAM_OUT: &str = "ssgg_stream_out";

    /// True for any node or stream this mixer owns.
    pub fn is_ours(name: &str) -> bool {
        name.starts_with(PREFIX)
    }
}

/// A mixer channel: four virtual outputs and the microphone.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    Game,
    Chat,
    Media,
    Aux,
    Mic,
}

impl Channel {
    pub const ALL: [Channel; 5] = [Channel::Game, Channel::Chat, Channel::Media, Channel::Aux, Channel::Mic];
    pub const OUTPUTS: [Channel; 4] = [Channel::Game, Channel::Chat, Channel::Media, Channel::Aux];

    pub fn is_output(self) -> bool {
        self != Channel::Mic
    }

    /// Lowercase identifier used in config files and node names.
    pub fn key(self) -> &'static str {
        match self {
            Channel::Game => "game",
            Channel::Chat => "chat",
            Channel::Media => "media",
            Channel::Aux => "aux",
            Channel::Mic => "mic",
        }
    }

    /// PipeWire node name of the virtual sink (outputs) or source (mic).
    pub fn node_name(self) -> &'static str {
        match self {
            Channel::Game => nodes::GAME,
            Channel::Chat => nodes::CHAT,
            Channel::Media => nodes::MEDIA,
            Channel::Aux => nodes::AUX,
            Channel::Mic => nodes::MIC,
        }
    }

    /// Human-readable node description shown in desktop sound settings.
    pub fn description(self) -> &'static str {
        match self {
            Channel::Game => "SteelSeries Game",
            Channel::Chat => "SteelSeries Chat",
            Channel::Media => "SteelSeries Media",
            Channel::Aux => "SteelSeries Aux",
            Channel::Mic => "SteelSeries Microphone",
        }
    }

    /// The channel whose virtual sink/source is called `name`.
    pub fn from_node_name(name: &str) -> Option<Channel> {
        Self::ALL.into_iter().find(|c| c.node_name() == name)
    }
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.key())
    }
}

impl FromStr for Channel {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let lower = s.trim().to_ascii_lowercase();
        let lower = match lower.as_str() {
            "microphone" => "mic",
            "auxiliary" => "aux",
            other => other,
        };
        Self::ALL
            .into_iter()
            .find(|c| c.key() == lower)
            .ok_or_else(|| Error::InvalidConfig(format!("unknown mixer channel '{s}'")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_names_are_prefixed_and_unique() {
        let mut names: Vec<&str> = Channel::ALL.iter().map(|c| c.node_name()).collect();
        assert!(names.iter().all(|n| nodes::is_ours(n)));
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), Channel::ALL.len());
        for c in Channel::ALL {
            assert_eq!(Channel::from_node_name(c.node_name()), Some(c));
            assert_eq!(c.key().parse::<Channel>().unwrap(), c);
            assert!(c.description().starts_with("SteelSeries "));
        }
        assert_eq!("Microphone".parse::<Channel>().unwrap(), Channel::Mic);
        assert!("master".parse::<Channel>().is_err());
        assert!(!nodes::is_ours("alsa_output.usb-SteelSeries_Arctis"));
    }
}
