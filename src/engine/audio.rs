//! Sonar-style mixer inside the engine: the daemon owns the PipeWire mixer process, its
//! settings (`mixer.toml`) and the link from the headset's ChatMix dial.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::devices::settings::ChatMix;
use crate::mixer::{
    AudioDevice, Channel, DeviceSelection, Eq, EqPreset, Mixer, MixerConfig, MixerStatus, RoutingRule, SystemRunner,
    chatmix_from_balance,
};
use crate::{Error, Result};

/// A partial change to the mixer settings. Every field is optional; only the ones present
/// are changed.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct MixerPatch {
    #[serde(default)]
    pub volume: Option<(Channel, u8)>,
    #[serde(default)]
    pub mute: Option<(Channel, bool)>,
    /// -100 = all game, 0 = both full, 100 = all chat.
    #[serde(default)]
    pub chatmix_balance: Option<i8>,
    /// Built-in preset name (`flat`, `bass_boost`, ...).
    #[serde(default)]
    pub eq_preset: Option<(Channel, String)>,
    /// One gain (dB) per band of the channel's current EQ.
    #[serde(default)]
    pub eq_gains: Option<(Channel, Vec<f32>)>,
    /// Sink name, or `auto`.
    #[serde(default)]
    pub output: Option<String>,
    /// Source name, or `auto`.
    #[serde(default)]
    pub input: Option<String>,
    #[serde(default)]
    pub route: Option<RoutingRule>,
    /// Remove the routing rule with this pattern.
    #[serde(default)]
    pub unroute: Option<String>,
    #[serde(default)]
    pub streamer: Option<bool>,
    /// Volume of a channel in the streaming mix.
    #[serde(default)]
    pub stream_volume: Option<(Channel, u8)>,
    #[serde(default)]
    pub noise_suppression: Option<bool>,
    #[serde(default)]
    pub spatial: Option<bool>,
    #[serde(default)]
    pub mic_enabled: Option<bool>,
}

impl MixerPatch {
    /// Apply to `config`, validating the result.
    pub fn apply_to(&self, config: &mut MixerConfig) -> Result<()> {
        if let Some((channel, volume)) = self.volume {
            config.level_mut(channel).volume = volume.min(100);
        }
        if let Some((channel, muted)) = self.mute {
            config.level_mut(channel).muted = muted;
        }
        if let Some(balance) = self.chatmix_balance {
            config.chatmix = chatmix_from_balance(f32::from(balance.clamp(-100, 100)) / 100.0);
        }
        if let Some((channel, name)) = &self.eq_preset {
            let preset = EqPreset::from_name(name).ok_or_else(|| {
                let names: Vec<&str> = EqPreset::ALL.iter().map(|p| p.name()).collect();
                Error::InvalidConfig(format!("unknown EQ preset '{name}' (one of: {})", names.join(", ")))
            })?;
            *config.eq_mut(*channel) = Eq::from_preset(preset);
        }
        if let Some((channel, gains)) = &self.eq_gains {
            let eq = config.eq_mut(*channel);
            if gains.len() != eq.bands.len() {
                return Err(Error::InvalidConfig(format!(
                    "{} EQ has {} bands, got {} gains",
                    channel,
                    eq.bands.len(),
                    gains.len()
                )));
            }
            for (band, gain) in eq.bands.iter_mut().zip(gains) {
                band.gain = *gain;
            }
            eq.preset = None;
        }
        if let Some(output) = &self.output {
            config.output = DeviceSelection::parse(output);
        }
        if let Some(input) = &self.input {
            config.mic.input = DeviceSelection::parse(input);
        }
        if let Some(rule) = &self.route {
            config
                .routing
                .rules
                .retain(|r| !(r.field == rule.field && r.pattern.eq_ignore_ascii_case(&rule.pattern)));
            config.routing.rules.insert(0, rule.clone());
        }
        if let Some(pattern) = &self.unroute {
            config
                .routing
                .rules
                .retain(|r| !r.pattern.eq_ignore_ascii_case(pattern));
        }
        if let Some(enabled) = self.streamer {
            config.streamer.enabled = enabled;
        }
        if let Some((channel, volume)) = self.stream_volume {
            config.streamer.mix.level_mut(channel).volume = volume.min(100);
        }
        if let Some(enabled) = self.noise_suppression {
            config.mic.noise_suppression.enabled = enabled;
        }
        if let Some(enabled) = self.spatial {
            config.spatial.enabled = enabled;
        }
        if let Some(enabled) = self.mic_enabled {
            config.mic.enabled = enabled;
        }
        config.validate()
    }
}

/// Everything the front ends show about the mixer.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MixerView {
    pub enabled: bool,
    pub running: bool,
    pub config: MixerConfig,
    pub status: Option<MixerStatus>,
    pub presets: Vec<String>,
    pub outputs: Vec<AudioDevice>,
    pub inputs: Vec<AudioDevice>,
    pub warnings: Vec<String>,
}

/// The engine's mixer slot.
pub struct AudioState {
    pub config: MixerConfig,
    pub mixer: Option<Mixer>,
    last_chatmix: Option<ChatMix>,
    last_error: Option<String>,
}

impl AudioState {
    pub fn load() -> Self {
        let config = MixerConfig::load().unwrap_or_else(|e| {
            warn!("mixer.toml unreadable ({e}); using defaults");
            MixerConfig::default()
        });
        Self {
            config,
            mixer: None,
            last_chatmix: None,
            last_error: None,
        }
    }

    pub fn is_running(&mut self) -> bool {
        self.mixer.as_mut().is_some_and(Mixer::is_running)
    }

    /// Start the PipeWire mixer with the current settings.
    pub fn start(&mut self) -> Result<()> {
        if self.is_running() {
            return Ok(());
        }
        let mut mixer = match self.mixer.take() {
            Some(m) => m,
            None => Mixer::new(Arc::new(SystemRunner::new()))?,
        };
        let report = mixer.detect();
        info!("Mixer environment: {report:?}");
        match mixer.start(&self.config) {
            Ok(()) => {
                self.last_error = None;
                self.mixer = Some(mixer);
                info!("Audio mixer started (SteelSeries Game/Chat/Media/Aux/Microphone)");
                Ok(())
            }
            Err(e) => {
                self.last_error = Some(e.to_string());
                self.mixer = Some(mixer);
                Err(e)
            }
        }
    }

    /// Stop the mixer and restore the previous default devices.
    pub fn stop(&mut self) {
        if let Some(mixer) = self.mixer.as_mut()
            && let Err(e) = mixer.stop()
        {
            warn!("Mixer stop: {e}");
        }
    }

    /// Change settings: save them, and apply them live when the mixer runs.
    pub fn update(&mut self, patch: &MixerPatch) -> Result<()> {
        let mut config = self.config.clone();
        patch.apply_to(&mut config)?;
        if let Some(mixer) = self.mixer.as_mut()
            && mixer.is_running()
        {
            mixer.apply(&config)?;
        }
        config.save()?;
        self.config = config;
        Ok(())
    }

    /// Switch to a whole new configuration (profile load): apply it live and save it.
    pub fn replace_config(&mut self, config: MixerConfig) -> Result<()> {
        config.validate()?;
        if let Some(mixer) = self.mixer.as_mut()
            && mixer.is_running()
        {
            mixer.apply(&config)?;
        }
        config.save()?;
        self.config = config;
        Ok(())
    }

    /// Follow the headset's ChatMix dial.
    pub fn follow_dial(&mut self, chatmix: ChatMix) {
        if self.last_chatmix == Some(chatmix) {
            return;
        }
        self.last_chatmix = Some(chatmix);
        self.config.chatmix = chatmix;
        if let Some(mixer) = self.mixer.as_mut()
            && mixer.is_running()
            && let Err(e) = mixer.set_chatmix(chatmix)
        {
            warn!("ChatMix update failed: {e}");
        }
    }

    pub fn view(&mut self, enabled: bool) -> MixerView {
        let running = self.is_running();
        let mut warnings: Vec<String> = self.last_error.iter().cloned().collect();
        let (status, outputs, inputs) = match self.mixer.as_mut() {
            Some(mixer) if running => {
                let status = mixer.status().map_err(|e| warnings.push(e.to_string())).ok();
                let outputs = mixer.list_outputs().unwrap_or_default();
                let inputs = mixer.list_inputs().unwrap_or_default();
                (status, outputs, inputs)
            }
            _ => (None, Vec::new(), Vec::new()),
        };
        if let Some(status) = &status {
            warnings.extend(status.warnings.iter().cloned());
        }
        if enabled && !running && self.last_error.is_none() {
            warnings.push("The mixer runs inside the daemon: systemctl --user start ssgg".to_string());
        }
        MixerView {
            enabled,
            running,
            config: self.config.clone(),
            status,
            presets: EqPreset::ALL.iter().map(|p| p.name().to_string()).collect(),
            outputs,
            inputs,
            warnings,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_changes_only_what_it_names() {
        let mut config = MixerConfig::default();
        let before = config.clone();
        MixerPatch {
            volume: Some((Channel::Chat, 40)),
            ..MixerPatch::default()
        }
        .apply_to(&mut config)
        .unwrap();
        assert_eq!(config.level(Channel::Chat).volume, 40);
        assert_eq!(config.level(Channel::Game), before.level(Channel::Game));
    }

    #[test]
    fn eq_presets_and_gains() {
        let mut config = MixerConfig::default();
        MixerPatch {
            eq_preset: Some((Channel::Game, "bass_boost".into())),
            ..MixerPatch::default()
        }
        .apply_to(&mut config)
        .unwrap();
        let bands = config.eq(Channel::Game).bands.len();
        assert!(bands > 0);
        let gains = vec![1.0; bands];
        MixerPatch {
            eq_gains: Some((Channel::Game, gains)),
            ..MixerPatch::default()
        }
        .apply_to(&mut config)
        .unwrap();
        assert!(
            config
                .eq(Channel::Game)
                .bands
                .iter()
                .all(|b| (b.gain - 1.0).abs() < 1e-6)
        );
        assert!(config.eq(Channel::Game).preset.is_none());

        let wrong = MixerPatch {
            eq_gains: Some((Channel::Game, vec![1.0])),
            ..MixerPatch::default()
        };
        assert!(wrong.apply_to(&mut config).is_err());
        let unknown = MixerPatch {
            eq_preset: Some((Channel::Game, "nope".into())),
            ..MixerPatch::default()
        };
        assert!(unknown.apply_to(&mut config).is_err());
    }

    #[test]
    fn routing_rules_replace_by_pattern() {
        let mut config = MixerConfig::default();
        let rule = RoutingRule::new(crate::mixer::MatchField::default(), "Discord", Channel::Media);
        MixerPatch {
            route: Some(rule.clone()),
            ..MixerPatch::default()
        }
        .apply_to(&mut config)
        .unwrap();
        assert_eq!(config.routing.rules[0], rule);
        MixerPatch {
            unroute: Some("discord".into()),
            ..MixerPatch::default()
        }
        .apply_to(&mut config)
        .unwrap();
        assert!(!config.routing.rules.iter().any(|r| r.pattern == "Discord"));
    }

    #[test]
    fn chatmix_balance_maps_to_volumes() {
        let mut config = MixerConfig::default();
        MixerPatch {
            chatmix_balance: Some(-100),
            ..MixerPatch::default()
        }
        .apply_to(&mut config)
        .unwrap();
        assert!(config.chatmix.game > config.chatmix.chat);
    }
}
