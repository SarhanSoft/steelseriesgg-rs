//! The mixer engine: runs the PipeWire child, applies levels, routes application streams.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::config::{DeviceSelection, MixerConfig, StreamMix, ensure_private_dir_for};
use super::detect::{self, DetectReport};
use super::eq::{Eq, EqBand};
use super::pactl::{self, PACTL, PaNode, PaSinkInput, Pactl, parse_new_sink_input_event};
use super::pwconf::{GraphPlan, NoiseSuppressionPlan, generate_pipewire_config};
use super::routing::{RoutingConfig, RoutingRule};
use super::runner::{ChildHandle, CommandRunner};
use super::{Channel, ChatMix, nodes};
use crate::config::Config;
use crate::{Error, Result};

const PIPEWIRE: &str = "pipewire";
const PW_DUMP: &str = "pw-dump";
const PW_CLI: &str = "pw-cli";

/// Files the mixer writes at runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MixerPaths {
    /// The generated PipeWire config.
    pub pipewire_conf: PathBuf,
    /// Child PID and the default devices to restore; present only while a session runs, so
    /// finding it at start means the previous run crashed.
    pub state_file: PathBuf,
}

impl MixerPaths {
    pub const CONF_NAME: &'static str = "mixer-pipewire.conf";
    pub const STATE_NAME: &'static str = "mixer-state.json";

    pub fn in_dir(dir: &Path) -> Self {
        Self {
            pipewire_conf: dir.join(Self::CONF_NAME),
            state_file: dir.join(Self::STATE_NAME),
        }
    }

    /// `$XDG_RUNTIME_DIR/ssgg/` when available, else `<config dir>/run/`.
    pub fn default_paths() -> Result<Self> {
        let runtime = directories::ProjectDirs::from("com", "steelseries-gg", "ssgg")
            .and_then(|dirs| dirs.runtime_dir().map(Path::to_path_buf));
        let dir = runtime
            .or_else(|| Config::config_dir().map(|dir| dir.join("run")))
            .ok_or_else(|| Error::FileSystemError("could not determine a runtime directory".into()))?;
        Ok(Self::in_dir(&dir))
    }
}

/// Waits and polling intervals. Tests set them to zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MixerTiming {
    /// How long to wait for the child's nodes to appear (and stale nodes to vanish).
    pub node_wait: Duration,
    pub poll_interval: Duration,
    /// Time between SIGTERM and SIGKILL when stopping the child.
    pub child_grace: Duration,
    /// Routing watcher wake-up interval; also the polling period when `pactl subscribe` is
    /// unavailable.
    pub watch_interval: Duration,
}

impl Default for MixerTiming {
    fn default() -> Self {
        Self {
            node_wait: Duration::from_secs(5),
            poll_interval: Duration::from_millis(100),
            child_grace: Duration::from_secs(2),
            watch_interval: Duration::from_secs(1),
        }
    }
}

/// A physical output or input device.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AudioDevice {
    pub name: String,
    pub description: String,
    pub steelseries: bool,
}

/// An application playback stream.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AppStream {
    /// Pulse sink-input index.
    pub index: u32,
    pub application_name: Option<String>,
    pub process_binary: Option<String>,
    pub media_name: Option<String>,
    /// Sink it currently plays to.
    pub sink: Option<String>,
    /// Mixer channel it currently plays to (`None` when on a physical device).
    pub channel: Option<Channel>,
    /// Channel the routing rules choose for it.
    pub rule_channel: Channel,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ChannelStatus {
    pub channel: Channel,
    pub node_name: &'static str,
    pub description: &'static str,
    /// The user's volume, percent.
    pub volume: u8,
    /// Volume set on the node (ChatMix applied), percent.
    pub effective_volume: u8,
    pub muted: bool,
    pub eq_preset: Option<String>,
    pub eq_bands: usize,
    /// The node exists in the sound server.
    pub present: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MicStatus {
    pub enabled: bool,
    pub node_name: &'static str,
    pub present: bool,
    pub input_device: Option<String>,
    pub volume: u8,
    pub muted: bool,
    /// `"webrtc"` or `"rnnoise"` while active.
    pub noise_suppression: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StreamerStatus {
    pub enabled: bool,
    pub node_name: &'static str,
    pub present: bool,
    pub mix: StreamMix,
}

/// Snapshot returned by [`Mixer::status`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MixerStatus {
    pub running: bool,
    pub output_device: Option<String>,
    pub channels: Vec<ChannelStatus>,
    pub chatmix: ChatMix,
    pub mic: MicStatus,
    pub streamer: StreamerStatus,
    pub spatial_active: bool,
    pub apps: Vec<AppStream>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
struct SavedState {
    child_pid: Option<u32>,
    previous_default_sink: Option<String>,
    previous_default_source: Option<String>,
}

type FileProbe = Arc<dyn Fn(&Path) -> bool + Send + Sync>;

struct Session {
    child: ChildHandle,
    plan: GraphPlan,
    pactl: Pactl,
    saved: SavedState,
    routing: Arc<Mutex<RoutingConfig>>,
    watcher: Option<Watcher>,
    warnings: Vec<String>,
}

/// Sonar-style mixer on PipeWire. See the [module docs](super).
///
/// Setters work whether or not the mixer runs: they always update [`Mixer::config`], and apply
/// the change to the sound server only while running. Persisting the config is the caller's
/// job ([`MixerConfig::save`]). Dropping a running mixer stops it.
pub struct Mixer {
    runner: Arc<dyn CommandRunner>,
    paths: MixerPaths,
    timing: MixerTiming,
    file_exists: FileProbe,
    live_eq_updates: bool,
    config: MixerConfig,
    report: Option<DetectReport>,
    session: Option<Session>,
}

impl Mixer {
    /// Mixer using [`MixerPaths::default_paths`].
    pub fn new(runner: Arc<dyn CommandRunner>) -> Result<Self> {
        Ok(Self::with_paths(runner, MixerPaths::default_paths()?))
    }

    pub fn with_paths(runner: Arc<dyn CommandRunner>, paths: MixerPaths) -> Self {
        Self {
            runner,
            paths,
            timing: MixerTiming::default(),
            file_exists: Arc::new(|path: &Path| path.exists()),
            live_eq_updates: true,
            config: MixerConfig::default(),
            report: None,
            session: None,
        }
    }

    pub fn set_timing(&mut self, timing: MixerTiming) {
        self.timing = timing;
    }

    /// Replace the filesystem check used for plugin and SOFA lookups (for tests).
    pub fn set_file_probe(&mut self, probe: impl Fn(&Path) -> bool + Send + Sync + 'static) {
        self.file_exists = Arc::new(probe);
    }

    /// When enabled (the default), an EQ change that keeps the band shapes is sent to the
    /// running filter with `pw-cli set-param` instead of restarting the PipeWire child.
    pub fn set_live_eq_updates(&mut self, enabled: bool) {
        self.live_eq_updates = enabled;
    }

    /// The desired configuration (last started, applied or set).
    pub fn config(&self) -> &MixerConfig {
        &self.config
    }

    pub fn paths(&self) -> &MixerPaths {
        &self.paths
    }

    /// The PipeWire child is alive.
    pub fn is_running(&mut self) -> bool {
        self.session.as_mut().is_some_and(|s| s.child.is_running())
    }

    /// Check PipeWire, tools and plugins. Never fails; problems are listed in the report.
    pub fn detect(&mut self) -> DetectReport {
        let report = detect::detect(self.runner.as_ref(), self.file_exists.as_ref());
        self.report = Some(report.clone());
        report
    }

    fn report(&mut self) -> DetectReport {
        match &self.report {
            Some(report) => report.clone(),
            None => self.detect(),
        }
    }

    fn pactl(&mut self) -> Pactl {
        if let Some(session) = &self.session {
            return session.pactl.clone();
        }
        let json = self.report().pactl_json;
        Pactl::new(Arc::clone(&self.runner), json)
    }

    fn can_live_update_eq(&self, report: &DetectReport) -> bool {
        self.live_eq_updates && report.tools.pw_dump && report.tools.pw_cli
    }

    /// Create the virtual devices and start routing. On an already running mixer this is
    /// [`Mixer::apply`].
    pub fn start(&mut self, config: &MixerConfig) -> Result<()> {
        config.validate()?;
        if self.session.is_some() {
            if self.is_running() {
                return self.apply(config);
            }
            if let Err(e) = self.stop() {
                tracing::warn!("cleaning up the previous mixer session: {e}");
            }
        }

        let report = self.detect();
        if !report.can_start() {
            return Err(Error::Audio(format!("cannot start the mixer:\n{}", report.summary())));
        }
        let pactl = Pactl::new(Arc::clone(&self.runner), report.pactl_json);
        let mut warnings = Vec::new();
        let crashed = self.clean_stale(&pactl, &report, &mut warnings);

        let info = pactl.info()?;
        let not_ours = |name: Option<String>| name.filter(|n| !nodes::is_ours(n));
        let mut saved = SavedState {
            child_pid: None,
            previous_default_sink: crashed.previous_default_sink.or(not_ours(info.default_sink)),
            previous_default_source: crashed.previous_default_source.or(not_ours(info.default_source)),
        };
        let (plan, plan_warnings) = self.build_plan(
            config,
            &pactl,
            &report,
            saved.previous_default_sink.as_deref(),
            saved.previous_default_source.as_deref(),
        )?;
        warnings.extend(plan_warnings);

        self.write_state(&saved)?;
        let (child, plan) = match self.launch(&pactl, plan, &mut warnings) {
            Ok(launched) => launched,
            Err(e) => {
                if let Err(remove) = self.remove_state() {
                    tracing::warn!("removing mixer state after a failed start: {remove}");
                }
                return Err(e);
            }
        };
        saved.child_pid = Some(child.pid());
        if let Err(e) = self.write_state(&saved) {
            warnings.push(format!("could not record the mixer state: {e}"));
        }

        apply_all_levels(&pactl, config, &plan, &mut warnings);
        set_defaults(&pactl, &plan, &mut warnings);
        // Streams present now are routed here; the watcher only handles streams that appear
        // later, so a stream the user moves by hand is not moved back.
        let seen: HashSet<u32> = match pactl.sink_inputs() {
            Ok(inputs) => {
                if let Err(e) = route_inputs(&pactl, &config.routing, &inputs, |_| true) {
                    warnings.push(format!("routing existing streams: {e}"));
                }
                inputs.iter().map(|i| i.index).collect()
            }
            Err(e) => {
                warnings.push(format!("routing existing streams: {e}"));
                HashSet::new()
            }
        };
        let routing = Arc::new(Mutex::new(config.routing.clone()));
        let watcher = Watcher::start(
            &self.runner,
            pactl.clone(),
            Arc::clone(&routing),
            seen,
            self.timing.watch_interval,
        );

        for warning in &warnings {
            tracing::warn!("mixer: {warning}");
        }
        self.config = config.clone();
        self.session = Some(Session {
            child,
            plan,
            pactl,
            saved,
            routing,
            watcher: Some(watcher),
            warnings,
        });
        Ok(())
    }

    /// Restore the previous default devices and remove every mixer node.
    pub fn stop(&mut self) -> Result<()> {
        let Some(mut session) = self.session.take() else {
            return Ok(());
        };
        drop(session.watcher.take());

        let mut errors = Vec::new();
        // Defaults first: when the virtual sinks vanish, the session manager moves their
        // streams to the default sink, which should be the real device by then.
        restore_defaults(&session, &mut errors);
        if let Err(e) = session.child.terminate(self.timing.child_grace) {
            errors.push(format!("stopping the pipewire child: {e}"));
        }
        if let Err(e) = self.remove_state() {
            errors.push(format!("removing {}: {e}", self.paths.state_file.display()));
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(Error::Audio(errors.join("; ")))
        }
    }

    /// Bring the running mixer to `config` with the fewest changes: restart the PipeWire child
    /// only when the graph shape, devices or modes change; otherwise update EQ controls,
    /// levels and routing in place.
    pub fn apply(&mut self, config: &MixerConfig) -> Result<()> {
        config.validate()?;
        let Some(session) = self.session.as_ref() else {
            self.config = config.clone();
            return Ok(());
        };
        let pactl = session.pactl.clone();
        let default_sink = session.saved.previous_default_sink.clone();
        let default_source = session.saved.previous_default_source.clone();
        let running_topology = session.plan.topology();

        let report = self.report();
        let (plan, warnings) = self.build_plan(
            config,
            &pactl,
            &report,
            default_sink.as_deref(),
            default_source.as_deref(),
        )?;
        if plan.topology() != running_topology {
            return self.restart(config, plan, warnings);
        }

        let eq_changes: Vec<(Channel, Vec<EqBand>)> = self.session.as_ref().map_or_else(Vec::new, |session| {
            Channel::ALL
                .into_iter()
                .filter_map(|c| {
                    let new = plan.bands(c)?;
                    (Some(new) != session.plan.bands(c)).then(|| (c, new.to_vec()))
                })
                .collect()
        });
        if !eq_changes.is_empty() {
            let live = self.can_live_update_eq(&report)
                && self.session.as_ref().is_some_and(|session| {
                    eq_changes.iter().all(|(channel, bands)| {
                        live_update_eq(self.runner.as_ref(), &session.plan, *channel, bands)
                            .map_err(|e| tracing::info!("live EQ update for {channel} failed ({e}); restarting"))
                            .is_ok()
                    })
                });
            if !live {
                return self.restart(config, plan, warnings);
            }
        }

        let old = self.config.clone();
        let mut errors = Vec::new();
        for channel in Channel::ALL {
            if old.level(channel) != config.level(channel)
                || old.effective_volume(channel) != config.effective_volume(channel)
            {
                record(&mut errors, apply_level(&pactl, config, &plan, channel));
            }
        }
        if plan.streamer && old.streamer.mix != config.streamer.mix {
            let changed: Vec<Channel> = Channel::ALL
                .into_iter()
                .filter(|&c| old.streamer.mix.level(c) != config.streamer.mix.level(c))
                .collect();
            record(
                &mut errors,
                apply_stream_mix(&pactl, &plan, &config.streamer.mix, Some(&changed)),
            );
        }
        if old.routing != config.routing {
            if let Some(session) = &self.session {
                *session.routing.lock() = config.routing.clone();
            }
            record(&mut errors, route_existing(&pactl, &config.routing).map(drop));
        }

        if let Some(session) = self.session.as_mut() {
            session.plan = plan;
            session.warnings = warnings;
        }
        self.config = config.clone();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(Error::Audio(errors.join("; ")))
        }
    }

    /// Set a channel's volume (0-100). Game and Chat are additionally scaled by ChatMix.
    pub fn set_volume(&mut self, channel: Channel, percent: u8) -> Result<()> {
        if percent > 100 {
            return Err(Error::InvalidConfig(format!("volume {percent} is above 100")));
        }
        self.config.level_mut(channel).volume = percent;
        self.apply_level_now(channel)
    }

    pub fn set_mute(&mut self, channel: Channel, muted: bool) -> Result<()> {
        self.config.level_mut(channel).muted = muted;
        self.apply_level_now(channel)
    }

    /// Balance Game against Chat (from the headset dial or a slider).
    pub fn set_chatmix(&mut self, mix: ChatMix) -> Result<()> {
        if mix.game > 100 || mix.chat > 100 {
            return Err(Error::InvalidConfig("ChatMix levels must be 0-100".into()));
        }
        self.config.chatmix = mix;
        self.apply_level_now(Channel::Game)?;
        self.apply_level_now(Channel::Chat)
    }

    fn apply_level_now(&mut self, channel: Channel) -> Result<()> {
        match &self.session {
            Some(session) => apply_level(&session.pactl, &self.config, &session.plan, channel),
            None => Ok(()),
        }
    }

    /// Replace a channel's EQ. Same band shapes: live control update; otherwise (or if the
    /// live update fails) the PipeWire child restarts with the new graph.
    pub fn set_eq(&mut self, channel: Channel, eq: &Eq) -> Result<()> {
        eq.validate()?;
        let mut config = self.config.clone();
        *config.eq_mut(channel) = eq.clone();
        let Some(session) = self.session.as_ref() else {
            self.config = config;
            return Ok(());
        };
        let Some(current) = session.plan.bands(channel) else {
            // Channel not in the running graph (microphone disabled or not found).
            self.config = config;
            return Ok(());
        };
        let same_shape = current.iter().map(|b| b.kind).eq(eq.bands.iter().map(|b| b.kind));
        let mut plan = session.plan.clone();
        plan.set_bands(channel, &eq.bands);
        let warnings = session.warnings.clone();

        let report = self.report();
        if same_shape && self.can_live_update_eq(&report) {
            match live_update_eq(self.runner.as_ref(), &plan, channel, &eq.bands) {
                Ok(()) => {
                    if let Some(session) = self.session.as_mut() {
                        session.plan = plan;
                    }
                    self.config = config;
                    return Ok(());
                }
                Err(e) => tracing::info!("live EQ update for {channel} failed ({e}); restarting"),
            }
        }
        self.restart(&config, plan, warnings)
    }

    /// Choose the physical output: a sink name, or `"auto"`.
    pub fn set_output(&mut self, sink: &str) -> Result<()> {
        let mut config = self.config.clone();
        config.output = DeviceSelection::parse(sink);
        self.apply(&config)
    }

    /// Add (or replace) a routing rule ahead of the existing ones, and move the matching
    /// streams that are playing now.
    pub fn route_app(&mut self, rule: RoutingRule) -> Result<()> {
        rule.validate()?;
        self.config.routing.upsert(rule.clone());
        let Some(session) = &self.session else {
            return Ok(());
        };
        *session.routing.lock() = self.config.routing.clone();
        let inputs = session.pactl.sink_inputs()?;
        route_inputs(&session.pactl, &self.config.routing, &inputs, |input| {
            rule.matches(&input.identity())
        })
        .map(drop)
    }

    /// Move every current application stream to the channel its rules choose. Returns how
    /// many streams moved.
    pub fn route_existing_streams(&mut self) -> Result<usize> {
        match &self.session {
            Some(session) => route_existing(&session.pactl, &self.config.routing),
            None => Err(Error::Audio("the mixer is not running".into())),
        }
    }

    pub fn status(&mut self) -> Result<MixerStatus> {
        let running = self.is_running();
        let pactl = self.pactl();
        let mut warnings = Vec::new();
        if let Some(session) = &self.session {
            warnings.extend(session.warnings.iter().cloned());
            if !running {
                warnings.push(format!(
                    "the mixer's pipewire process exited: {}",
                    tail_or_placeholder(&session.child.stderr_tail())
                ));
            }
        }

        let snapshot = pactl
            .sinks()
            .and_then(|sinks| Ok((sinks, pactl.sources()?, pactl.sink_inputs()?)));
        let (sinks, sources, inputs) = match snapshot {
            Ok(snapshot) => snapshot,
            Err(e) if !running => {
                warnings.push(format!("could not query the sound server: {e}"));
                Default::default()
            }
            Err(e) => return Err(e),
        };
        let has_sink = |name: &str| sinks.iter().any(|s| s.name == name);
        let has_source = |name: &str| sources.iter().any(|s| s.name == name);
        let plan = self.session.as_ref().map(|s| &s.plan);
        let config = &self.config;

        let channels = Channel::OUTPUTS
            .into_iter()
            .map(|channel| {
                let level = config.level(channel);
                let eq = config.eq(channel);
                ChannelStatus {
                    channel,
                    node_name: channel.node_name(),
                    description: channel.description(),
                    volume: level.volume,
                    effective_volume: config.effective_volume(channel),
                    muted: level.muted,
                    eq_preset: eq.preset.clone(),
                    eq_bands: eq.bands.len(),
                    present: has_sink(channel.node_name()),
                }
            })
            .collect();
        let mic_plan = plan.and_then(|p| p.mic.as_ref());
        let mic = MicStatus {
            enabled: config.mic.enabled,
            node_name: nodes::MIC,
            present: has_source(nodes::MIC),
            input_device: mic_plan.map(|m| m.input_source.clone()),
            volume: config.mic.level.volume,
            muted: config.mic.level.muted,
            noise_suppression: mic_plan.and_then(|m| m.noise_suppression.as_ref()).map(|ns| match ns {
                NoiseSuppressionPlan::Webrtc => "webrtc".to_string(),
                NoiseSuppressionPlan::Rnnoise { .. } => "rnnoise".to_string(),
            }),
        };

        Ok(MixerStatus {
            running,
            output_device: plan.map(|p| p.output_sink.clone()),
            channels,
            chatmix: config.chatmix,
            mic,
            streamer: StreamerStatus {
                enabled: config.streamer.enabled,
                node_name: nodes::STREAM,
                present: has_sink(nodes::STREAM),
                mix: config.streamer.mix,
            },
            spatial_active: plan.is_some_and(GraphPlan::spatial_active),
            apps: app_streams(&sinks, &inputs, &config.routing),
            warnings,
        })
    }

    /// Physical sinks (the mixer's own nodes excluded).
    pub fn list_outputs(&mut self) -> Result<Vec<AudioDevice>> {
        let sinks = self.pactl().sinks()?;
        Ok(sinks.iter().filter(|s| !s.is_ours()).map(audio_device).collect())
    }

    /// Physical sources (monitors and the mixer's own nodes excluded).
    pub fn list_inputs(&mut self) -> Result<Vec<AudioDevice>> {
        let sources = self.pactl().sources()?;
        Ok(sources
            .iter()
            .filter(|s| !s.is_ours() && !s.is_monitor())
            .map(audio_device)
            .collect())
    }

    /// Application playback streams with their current and rule-chosen channels.
    pub fn list_apps(&mut self) -> Result<Vec<AppStream>> {
        let pactl = self.pactl();
        let sinks = pactl.sinks()?;
        let inputs = pactl.sink_inputs()?;
        Ok(app_streams(&sinks, &inputs, &self.config.routing))
    }

    fn build_plan(
        &self,
        config: &MixerConfig,
        pactl: &Pactl,
        report: &DetectReport,
        default_sink: Option<&str>,
        default_source: Option<&str>,
    ) -> Result<(GraphPlan, Vec<String>)> {
        let mut warnings = Vec::new();
        let output = resolve_output(&config.output, &pactl.sinks()?, default_sink, &mut warnings)?;
        let input = if config.mic.enabled {
            resolve_input(&config.mic.input, &pactl.sources()?, default_source, &mut warnings)
        } else {
            None
        };
        let sofa_exists = config
            .spatial
            .sofa_path
            .as_deref()
            .is_some_and(|path| (self.file_exists)(path));
        let (plan, plan_warnings) =
            GraphPlan::build(config, output, input, report.rnnoise_plugin.as_deref(), sofa_exists);
        warnings.extend(plan_warnings);
        Ok((plan, warnings))
    }

    /// Start the child for `plan`; if that fails and the plan uses noise suppression or
    /// spatial audio (whose modules may be missing), retry once without them.
    fn launch(&self, pactl: &Pactl, plan: GraphPlan, warnings: &mut Vec<String>) -> Result<(ChildHandle, GraphPlan)> {
        match self.spawn_and_wait(pactl, &plan) {
            Ok(child) => Ok((child, plan)),
            Err(first) if plan.has_optional_features() => {
                let reduced = plan.without_optional_features();
                warnings.push(format!(
                    "started without noise suppression and spatial audio because the full graph failed: {first}"
                ));
                let child = self.spawn_and_wait(pactl, &reduced)?;
                Ok((child, reduced))
            }
            Err(e) => Err(e),
        }
    }

    fn spawn_and_wait(&self, pactl: &Pactl, plan: &GraphPlan) -> Result<ChildHandle> {
        ensure_private_dir_for(&self.paths.pipewire_conf)?;
        crate::fs_utils::secure_write(&self.paths.pipewire_conf, generate_pipewire_config(plan))?;
        let conf = self.paths.pipewire_conf.to_string_lossy().into_owned();
        let mut child = self.runner.spawn(PIPEWIRE, &["-c".to_string(), conf])?;
        self.wait_for_nodes(pactl, &mut child, plan)?;
        Ok(child)
    }

    fn wait_for_nodes(&self, pactl: &Pactl, child: &mut ChildHandle, plan: &GraphPlan) -> Result<()> {
        let deadline = Instant::now() + self.timing.node_wait;
        let sinks = plan.expected_sinks();
        let sources = plan.expected_sources();
        loop {
            if let Some(code) = child.try_wait()? {
                return Err(Error::Audio(format!(
                    "pipewire exited with code {code} while starting the mixer: {}",
                    tail_or_placeholder(&child.stderr_tail())
                )));
            }
            let present_sinks: HashSet<String> = pactl.sinks()?.into_iter().map(|s| s.name).collect();
            let present_sources: HashSet<String> = if sources.is_empty() {
                HashSet::new()
            } else {
                pactl.sources()?.into_iter().map(|s| s.name).collect()
            };
            let missing: Vec<&str> = sinks
                .iter()
                .filter(|n| !present_sinks.contains(**n))
                .chain(sources.iter().filter(|n| !present_sources.contains(**n)))
                .copied()
                .collect();
            if missing.is_empty() {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(Error::Audio(format!(
                    "mixer nodes did not appear within {} ms (missing {}); pipewire said: {}",
                    self.timing.node_wait.as_millis(),
                    missing.join(", "),
                    tail_or_placeholder(&child.stderr_tail())
                )));
            }
            thread::sleep(self.timing.poll_interval);
        }
    }

    /// Replace the running child with one for `plan`, then re-apply levels, defaults and
    /// routing (streams fall back to the default device while the nodes are gone).
    fn restart(&mut self, config: &MixerConfig, plan: GraphPlan, mut warnings: Vec<String>) -> Result<()> {
        let Some(mut session) = self.session.take() else {
            return Err(Error::Audio("the mixer is not running".into()));
        };
        if let Err(e) = session.child.terminate(self.timing.child_grace) {
            warnings.push(format!("stopping the previous pipewire child: {e}"));
        }
        let (child, plan) = match self.launch(&session.pactl, plan, &mut warnings) {
            Ok(launched) => launched,
            Err(e) => {
                let mut errors = Vec::new();
                restore_defaults(&session, &mut errors);
                for error in errors {
                    tracing::warn!("{error}");
                }
                if let Err(remove) = self.remove_state() {
                    tracing::warn!("removing mixer state: {remove}");
                }
                self.config = config.clone();
                return Err(e);
            }
        };
        session.saved.child_pid = Some(child.pid());
        if let Err(e) = self.write_state(&session.saved) {
            warnings.push(format!("could not record the mixer state: {e}"));
        }
        session.child = child;
        session.plan = plan;
        apply_all_levels(&session.pactl, config, &session.plan, &mut warnings);
        set_defaults(&session.pactl, &session.plan, &mut warnings);
        *session.routing.lock() = config.routing.clone();
        if let Err(e) = route_existing(&session.pactl, &config.routing) {
            warnings.push(format!("routing existing streams: {e}"));
        }
        session.warnings = warnings;
        self.config = config.clone();
        self.session = Some(session);
        Ok(())
    }

    /// Stop what a crashed run left behind. Returns the crashed run's saved defaults.
    fn clean_stale(&self, pactl: &Pactl, report: &DetectReport, warnings: &mut Vec<String>) -> SavedState {
        let saved = self.read_state(warnings);

        let mut pids: Vec<u32> = Vec::new();
        if report.tools.pgrep {
            let pattern = stale_process_pattern(&self.paths.pipewire_conf);
            match self.runner.run("pgrep", &["-f".to_string(), pattern]) {
                Ok(out) => pids.extend(out.stdout.lines().filter_map(|l| l.trim().parse::<u32>().ok())),
                Err(e) => tracing::debug!("pgrep failed: {e}"),
            }
        }
        let stale = our_nodes(pactl);
        pids.extend(stale.iter().filter_map(|node| {
            let binary = node.properties.get("application.process.binary")?;
            if binary != PIPEWIRE {
                return None;
            }
            node.properties.get("application.process.id")?.parse::<u32>().ok()
        }));
        pids.sort_unstable();
        pids.dedup();
        pids.retain(|&pid| pid != std::process::id());

        for pid in &pids {
            match self.runner.run("kill", &["-TERM".to_string(), pid.to_string()]) {
                Ok(out) if out.success() => tracing::info!("stopped stale mixer process {pid}"),
                Ok(out) => warnings.push(format!(
                    "could not stop stale mixer process {pid}: {}",
                    out.stderr.trim()
                )),
                Err(e) => warnings.push(format!("could not stop stale mixer process {pid}: {e}")),
            }
        }

        if !stale.is_empty() {
            let deadline = Instant::now() + self.timing.node_wait;
            loop {
                let remaining = our_nodes(pactl);
                if remaining.is_empty() {
                    break;
                }
                if pids.is_empty() || Instant::now() >= deadline {
                    let names: Vec<&str> = remaining.iter().map(|n| n.name.as_str()).collect();
                    warnings.push(format!(
                        "nodes from an earlier mixer run are still present: {}",
                        names.join(", ")
                    ));
                    break;
                }
                thread::sleep(self.timing.poll_interval);
            }
        }
        saved
    }

    fn read_state(&self, warnings: &mut Vec<String>) -> SavedState {
        match std::fs::read_to_string(&self.paths.state_file) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
                warnings.push(format!("ignoring unreadable {}: {e}", self.paths.state_file.display()));
                SavedState::default()
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => SavedState::default(),
            Err(e) => {
                warnings.push(format!("cannot read {}: {e}", self.paths.state_file.display()));
                SavedState::default()
            }
        }
    }

    fn write_state(&self, state: &SavedState) -> Result<()> {
        ensure_private_dir_for(&self.paths.state_file)?;
        crate::fs_utils::secure_write(&self.paths.state_file, serde_json::to_string_pretty(state)?)
    }

    fn remove_state(&self) -> Result<()> {
        match std::fs::remove_file(&self.paths.state_file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        }
    }
}

impl Drop for Mixer {
    fn drop(&mut self) {
        if self.session.is_some() {
            if let Err(e) = self.stop() {
                tracing::warn!("stopping the mixer on drop: {e}");
            }
        }
    }
}

fn record(errors: &mut Vec<String>, result: Result<()>) {
    if let Err(e) = result {
        errors.push(e.to_string());
    }
}

fn tail_or_placeholder(tail: &str) -> &str {
    if tail.trim().is_empty() {
        "(no output)"
    } else {
        tail.trim()
    }
}

fn audio_device(node: &PaNode) -> AudioDevice {
    AudioDevice {
        name: node.name.clone(),
        description: node.description.clone(),
        steelseries: node.is_steelseries(),
    }
}

/// Every sink and source named `ssgg_*`. Query failures count as "none".
fn our_nodes(pactl: &Pactl) -> Vec<PaNode> {
    let mut found = Vec::new();
    for list in [pactl.sinks(), pactl.sources()] {
        match list {
            Ok(nodes) => found.extend(nodes.into_iter().filter(PaNode::is_ours)),
            Err(e) => tracing::debug!("listing nodes for stale cleanup failed: {e}"),
        }
    }
    found
}

/// `pgrep -f` pattern matching only a `pipewire -c <conf>` command line, never e.g. an editor
/// that has the file open.
pub(crate) fn stale_process_pattern(conf: &Path) -> String {
    let mut escaped = String::new();
    for c in conf.to_string_lossy().chars() {
        if "\\.[]()*+?{}|^$".contains(c) {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    format!("^([^ ]*/)?{PIPEWIRE} -c {escaped}$")
}

/// SteelSeries devices first ("game" and "stereo" sinks before "chat" ones), then the
/// system default, then the first device.
fn pick_device(candidates: &[&PaNode], default: Option<&str>, rank: fn(&PaNode) -> i32) -> Option<String> {
    let mut best: Option<(&PaNode, i32)> = None;
    for node in candidates.iter().copied().filter(|n| n.is_steelseries()) {
        let score = rank(node);
        if best.is_none_or(|(_, top)| score > top) {
            best = Some((node, score));
        }
    }
    if let Some((node, _)) = best {
        return Some(node.name.clone());
    }
    if let Some(default) = default {
        if candidates.iter().any(|n| n.name == default) {
            return Some(default.to_string());
        }
    }
    candidates.first().map(|n| n.name.clone())
}

fn sink_rank(node: &PaNode) -> i32 {
    let text = format!("{} {}", node.name, node.description).to_lowercase();
    let mut score = 0;
    if text.contains("game") {
        score += 2;
    }
    if text.contains("stereo") {
        score += 1;
    }
    if text.contains("chat") || text.contains("mono") {
        score -= 2;
    }
    score
}

fn source_rank(_: &PaNode) -> i32 {
    0
}

pub(crate) fn resolve_output(
    selection: &DeviceSelection,
    sinks: &[PaNode],
    default_sink: Option<&str>,
    warnings: &mut Vec<String>,
) -> Result<String> {
    let candidates: Vec<&PaNode> = sinks.iter().filter(|s| !s.is_ours()).collect();
    if let DeviceSelection::Named(name) = selection {
        if candidates.iter().any(|s| &s.name == name) {
            return Ok(name.clone());
        }
        warnings.push(format!("output device {name} not found; choosing one automatically"));
    }
    pick_device(&candidates, default_sink, sink_rank)
        .ok_or_else(|| Error::Audio("no output device (sink) found".into()))
}

pub(crate) fn resolve_input(
    selection: &DeviceSelection,
    sources: &[PaNode],
    default_source: Option<&str>,
    warnings: &mut Vec<String>,
) -> Option<String> {
    let candidates: Vec<&PaNode> = sources.iter().filter(|s| !s.is_ours() && !s.is_monitor()).collect();
    if let DeviceSelection::Named(name) = selection {
        if candidates.iter().any(|s| &s.name == name) {
            return Some(name.clone());
        }
        warnings.push(format!("input device {name} not found; choosing one automatically"));
    }
    pick_device(&candidates, default_source, source_rank)
}

fn apply_level(pactl: &Pactl, config: &MixerConfig, plan: &GraphPlan, channel: Channel) -> Result<()> {
    let level = config.level(channel);
    let volume = config.effective_volume(channel);
    let node = channel.node_name();
    if channel.is_output() {
        if plan.bands(channel).is_none() {
            return Ok(());
        }
        pactl.set_sink_volume(node, volume)?;
        pactl.set_sink_mute(node, level.muted)
    } else {
        if plan.mic.is_none() {
            return Ok(());
        }
        pactl.set_source_volume(node, volume)?;
        pactl.set_source_mute(node, level.muted)
    }
}

/// Set the streaming-mix tap volumes (all, or only `only`).
fn apply_stream_mix(pactl: &Pactl, plan: &GraphPlan, mix: &StreamMix, only: Option<&[Channel]>) -> Result<()> {
    let taps = plan.stream_taps();
    if taps.is_empty() {
        return Ok(());
    }
    let inputs = pactl.sink_inputs()?;
    let by_name: HashMap<&str, u32> = inputs
        .iter()
        .filter_map(|i| Some((i.prop("node.name")?, i.index)))
        .collect();
    let mut missing = Vec::new();
    for (channel, node) in &taps {
        if only.is_some_and(|only| !only.contains(channel)) {
            continue;
        }
        match by_name.get(node.as_str()) {
            Some(&index) => {
                let level = mix.level(*channel);
                pactl.set_sink_input_volume(index, level.volume.min(100))?;
                pactl.set_sink_input_mute(index, level.muted)?;
            }
            None => missing.push(node.as_str()),
        }
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(Error::Audio(format!(
            "streaming-mix streams not found: {}",
            missing.join(", ")
        )))
    }
}

fn apply_all_levels(pactl: &Pactl, config: &MixerConfig, plan: &GraphPlan, warnings: &mut Vec<String>) {
    let mut errors = Vec::new();
    for channel in Channel::ALL {
        record(&mut errors, apply_level(pactl, config, plan, channel));
    }
    if plan.streamer {
        record(&mut errors, apply_stream_mix(pactl, plan, &config.streamer.mix, None));
    }
    warnings.extend(errors.into_iter().map(|e| format!("setting levels: {e}")));
}

fn set_defaults(pactl: &Pactl, plan: &GraphPlan, warnings: &mut Vec<String>) {
    if let Err(e) = pactl.set_default_sink(nodes::GAME) {
        warnings.push(format!("could not make SteelSeries Game the default output: {e}"));
    }
    if plan.mic.is_some() {
        if let Err(e) = pactl.set_default_source(nodes::MIC) {
            warnings.push(format!("could not make SteelSeries Microphone the default input: {e}"));
        }
    }
}

fn restore_defaults(session: &Session, errors: &mut Vec<String>) {
    if let Some(sink) = &session.saved.previous_default_sink {
        if let Err(e) = session.pactl.set_default_sink(sink) {
            errors.push(format!("restoring default output {sink}: {e}"));
        }
    }
    if session.plan.mic.is_some() {
        if let Some(source) = &session.saved.previous_default_source {
            if let Err(e) = session.pactl.set_default_source(source) {
                errors.push(format!("restoring default input {source}: {e}"));
            }
        }
    }
}

/// Move the streams accepted by `filter` to their rule-chosen channel. Streams that fail to
/// move (for example because they just ended) are logged, not fatal.
fn route_inputs(
    pactl: &Pactl,
    routing: &RoutingConfig,
    inputs: &[PaSinkInput],
    filter: impl Fn(&PaSinkInput) -> bool,
) -> Result<usize> {
    let wanted: Vec<&PaSinkInput> = inputs.iter().filter(|i| !i.is_internal() && filter(i)).collect();
    if wanted.is_empty() {
        return Ok(0);
    }
    let sink_names: HashMap<u32, String> = pactl.sinks()?.into_iter().map(|s| (s.index, s.name)).collect();
    let mut moved = 0;
    for input in wanted {
        let target = routing.channel_for(&input.identity()).node_name();
        let current = input.sink.and_then(|index| sink_names.get(&index));
        if current.is_some_and(|name| name == target) {
            continue;
        }
        match pactl.move_sink_input(input.index, target) {
            Ok(()) => moved += 1,
            Err(e) => tracing::warn!("could not move stream {} to {target}: {e}", input.index),
        }
    }
    Ok(moved)
}

fn route_existing(pactl: &Pactl, routing: &RoutingConfig) -> Result<usize> {
    let inputs = pactl.sink_inputs()?;
    route_inputs(pactl, routing, &inputs, |_| true)
}

/// Route streams whose index is not in `seen`, then remember them, so a stream the user moves
/// by hand afterwards is left alone.
pub(crate) fn route_new_streams(pactl: &Pactl, routing: &RoutingConfig, seen: &mut HashSet<u32>) -> Result<usize> {
    let inputs = pactl.sink_inputs()?;
    let current: HashSet<u32> = inputs.iter().map(|i| i.index).collect();
    seen.retain(|index| current.contains(index));
    let fresh: HashSet<u32> = current.difference(seen).copied().collect();
    if fresh.is_empty() {
        return Ok(0);
    }
    seen.extend(&fresh);
    route_inputs(pactl, routing, &inputs, |input| fresh.contains(&input.index))
}

fn app_streams(sinks: &[PaNode], inputs: &[PaSinkInput], routing: &RoutingConfig) -> Vec<AppStream> {
    let names: HashMap<u32, &str> = sinks.iter().map(|s| (s.index, s.name.as_str())).collect();
    inputs
        .iter()
        .filter(|input| !input.is_internal())
        .map(|input| {
            let sink = input
                .sink
                .and_then(|index| names.get(&index))
                .map(|name| (*name).to_string());
            AppStream {
                index: input.index,
                application_name: input.prop("application.name").map(str::to_string),
                process_binary: input.prop("application.process.binary").map(str::to_string),
                media_name: input.prop("media.name").map(str::to_string),
                channel: sink.as_deref().and_then(Channel::from_node_name),
                sink,
                rule_channel: routing.channel_for(&input.identity()),
            }
        })
        .collect()
}

/// `id` of the node called `node_name` in `pw-dump` output.
pub(crate) fn find_node_id(dump: &str, node_name: &str) -> Option<u32> {
    let objects: Value = serde_json::from_str(dump).ok()?;
    objects
        .as_array()?
        .iter()
        .find(|obj| {
            obj.get("type").and_then(Value::as_str) == Some("PipeWire:Interface:Node")
                && obj.pointer("/info/props/node.name").and_then(Value::as_str) == Some(node_name)
        })
        .and_then(|obj| obj.get("id")?.as_u64())
        .and_then(|id| u32::try_from(id).ok())
}

/// Send new EQ control values to the running filter-chain without restarting it.
fn live_update_eq(runner: &dyn CommandRunner, plan: &GraphPlan, channel: Channel, bands: &[EqBand]) -> Result<()> {
    let (node, props) = plan
        .eq_props(channel, bands)
        .ok_or_else(|| Error::Audio(format!("{channel} is not part of the running graph")))?;
    let dump = runner.run(PW_DUMP, &[])?;
    if !dump.success() {
        return Err(Error::Audio(format!("pw-dump failed: {}", dump.stderr.trim())));
    }
    let id = find_node_id(&dump.stdout, node)
        .ok_or_else(|| Error::Audio(format!("node {node} not found in pw-dump output")))?;
    let out = runner.run(
        PW_CLI,
        &["set-param".to_string(), id.to_string(), "Props".to_string(), props],
    )?;
    let text = format!("{}{}", out.stdout, out.stderr);
    if !out.success() || text.to_lowercase().contains("error") {
        return Err(Error::Audio(format!("pw-cli set-param failed: {}", text.trim())));
    }
    Ok(())
}

/// Background thread applying the routing rules to new streams.
struct Watcher {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    subscribe: Option<ChildHandle>,
}

impl Watcher {
    fn start(
        runner: &Arc<dyn CommandRunner>,
        pactl: Pactl,
        routing: Arc<Mutex<RoutingConfig>>,
        seen: HashSet<u32>,
        interval: Duration,
    ) -> Self {
        let (subscribe, events) = match runner.spawn(PACTL, &pactl::args::subscribe()) {
            Ok(mut child) => {
                let events = child.take_stdout();
                (Some(child), events)
            }
            Err(e) => {
                tracing::warn!("`pactl subscribe` unavailable ({e}); polling for new streams instead");
                (None, None)
            }
        };
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread = thread::Builder::new()
            .name("ssgg-mixer-routing".into())
            .spawn(move || watch_loop(&pactl, &routing, &thread_stop, events, seen, interval));
        let thread = match thread {
            Ok(handle) => Some(handle),
            Err(e) => {
                tracing::warn!("could not start the routing thread: {e}; new streams will not be routed");
                None
            }
        };
        Self {
            stop,
            thread,
            subscribe,
        }
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(mut child) = self.subscribe.take() {
            if let Err(e) = child.terminate(Duration::from_millis(500)) {
                tracing::warn!("stopping `pactl subscribe`: {e}");
            }
        }
        if let Some(handle) = self.thread.take() {
            if handle.join().is_err() {
                tracing::warn!("the routing thread panicked");
            }
        }
    }
}

fn watch_loop(
    pactl: &Pactl,
    routing: &Mutex<RoutingConfig>,
    stop: &AtomicBool,
    mut events: Option<Receiver<String>>,
    mut seen: HashSet<u32>,
    interval: Duration,
) {
    while !stop.load(Ordering::SeqCst) {
        let mut subscription_lost = false;
        let scan = if let Some(rx) = events.as_ref() {
            match rx.recv_timeout(interval) {
                Ok(line) => {
                    let mut new_stream = parse_new_sink_input_event(&line).is_some();
                    while let Ok(more) = rx.try_recv() {
                        new_stream |= parse_new_sink_input_event(&more).is_some();
                    }
                    new_stream
                }
                Err(RecvTimeoutError::Timeout) => false,
                Err(RecvTimeoutError::Disconnected) => {
                    subscription_lost = true;
                    true
                }
            }
        } else {
            thread::sleep(interval);
            true
        };
        if subscription_lost {
            events = None;
            if !stop.load(Ordering::SeqCst) {
                tracing::warn!("`pactl subscribe` exited; polling for new streams instead");
            }
        }
        if scan && !stop.load(Ordering::SeqCst) {
            let rules = routing.lock().clone();
            if let Err(e) = route_new_streams(pactl, &rules, &mut seen) {
                tracing::warn!("routing new streams: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mixer::config::{Level, NoiseSuppression, NoiseSuppressionBackend};
    use crate::mixer::eq::EqPreset;
    use crate::mixer::pactl::samples::INFO_TEXT;
    use crate::mixer::routing::MatchField;
    use crate::mixer::runner::{CommandOutput, FakeRunner};

    const HEADSET: &str = "alsa_output.usb-SteelSeries_Arctis_Nova_7-00.analog-stereo";
    const BUILTIN: &str = "alsa_output.pci-0000_00_1f.3.analog-stereo";
    const HEADSET_MIC: &str = "alsa_input.usb-SteelSeries_Arctis_Nova_7-00.mono-fallback";

    fn sink_json(index: u32, name: &str, description: &str) -> String {
        format!(
            r#"{{"index":{index},"name":"{name}","description":"{description}","mute":false,
               "volume":{{"front-left":{{"value_percent":"100%"}}}},"properties":{{"media.class":"Audio/Sink"}}}}"#
        )
    }

    fn input_json(index: u32, sink: u32, props: &str) -> String {
        format!(r#"{{"index":{index},"sink":{sink},"mute":false,"volume":{{}},"properties":{{{props}}}}}"#)
    }

    fn physical_sinks() -> Vec<String> {
        vec![
            sink_json(48, BUILTIN, "Built-in Audio Analog Stereo"),
            sink_json(56, HEADSET, "Arctis Nova 7 Analog Stereo"),
        ]
    }

    fn mixer_sinks() -> Vec<String> {
        let mut sinks = physical_sinks();
        for (i, channel) in Channel::OUTPUTS.iter().enumerate() {
            sinks.push(sink_json(100 + i as u32, channel.node_name(), channel.description()));
        }
        sinks.push(sink_json(110, nodes::STREAM, "SteelSeries Stream"));
        sinks
    }

    fn physical_sources() -> Vec<String> {
        vec![
            sink_json(57, &format!("{HEADSET}.monitor"), "Monitor of Arctis Nova 7"),
            sink_json(58, HEADSET_MIC, "Arctis Nova 7 Mono"),
        ]
    }

    fn mixer_sources() -> Vec<String> {
        let mut sources = physical_sources();
        sources.push(sink_json(120, nodes::MIC, "SteelSeries Microphone"));
        sources.push(sink_json(121, nodes::STREAM_OUT, "SteelSeries Stream (source)"));
        sources
    }

    fn app_inputs() -> Vec<String> {
        vec![
            input_json(
                200,
                56,
                r#""application.name":"WEBRTC VoiceEngine","application.process.binary":"Discord""#,
            ),
            input_json(
                201,
                56,
                r#""application.name":"Firefox","application.process.binary":"firefox""#,
            ),
            input_json(
                202,
                100,
                r#""application.name":"cs2","application.process.binary":"cs2""#,
            ),
        ]
    }

    fn mixer_inputs() -> Vec<String> {
        let mut inputs = app_inputs();
        inputs.push(input_json(
            300,
            56,
            r#""node.name":"ssgg_game_out","node.link-group":"fc-1""#,
        ));
        for (i, channel) in Channel::ALL.iter().enumerate() {
            inputs.push(input_json(
                310 + i as u32,
                110,
                &format!(
                    r#""node.name":"ssgg_stream_{}","node.link-group":"lb-{i}""#,
                    channel.key()
                ),
            ));
        }
        inputs
    }

    fn array(items: &[String]) -> CommandOutput {
        CommandOutput::ok(format!("[{}]", items.join(",")))
    }

    /// A healthy PipeWire system where the mixer's nodes exist only while its child runs.
    fn system() -> FakeRunner {
        let fake = FakeRunner::new();
        fake.on("pactl", &["info"], CommandOutput::ok(INFO_TEXT));
        fake.on(
            "pactl",
            &["-f", "json", "info"],
            CommandOutput::ok(format!(
                r#"{{"server_name":"PulseAudio (on PipeWire 1.0.5)","default_sink_name":"{HEADSET}","default_source_name":"{HEADSET_MIC}"}}"#
            )),
        );
        let list = ["-f", "json", "list"];
        fake.on("pactl", &[list[0], list[1], list[2], "sinks"], array(&physical_sinks()));
        fake.on(
            "pactl",
            &[list[0], list[1], list[2], "sources"],
            array(&physical_sources()),
        );
        fake.on(
            "pactl",
            &[list[0], list[1], list[2], "sink-inputs"],
            array(&app_inputs()),
        );
        fake.on_while_running(
            "pipewire",
            "pactl",
            &[list[0], list[1], list[2], "sinks"],
            array(&mixer_sinks()),
        );
        fake.on_while_running(
            "pipewire",
            "pactl",
            &[list[0], list[1], list[2], "sources"],
            array(&mixer_sources()),
        );
        fake.on_while_running(
            "pipewire",
            "pactl",
            &[list[0], list[1], list[2], "sink-inputs"],
            array(&mixer_inputs()),
        );
        fake.on("pgrep", &["--version"], CommandOutput::ok("pgrep from procps-ng 4.0.4"));
        fake.on_prefix("pgrep", &["-f"], CommandOutput::failed(1, ""));
        fake
    }

    fn quick() -> MixerTiming {
        MixerTiming {
            node_wait: Duration::ZERO,
            poll_interval: Duration::ZERO,
            child_grace: Duration::ZERO,
            watch_interval: Duration::from_millis(5),
        }
    }

    fn mixer(fake: &FakeRunner, dir: &Path) -> Mixer {
        let mut mixer = Mixer::with_paths(Arc::new(fake.clone()), MixerPaths::in_dir(dir));
        mixer.set_timing(quick());
        mixer.set_file_probe(|_: &Path| false);
        mixer
    }

    fn pactl_calls(fake: &FakeRunner, prefix: &str) -> Vec<String> {
        fake.command_lines("pactl")
            .into_iter()
            .filter(|c| c.starts_with(prefix))
            .collect()
    }

    #[test]
    fn mixer_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Mixer>();
    }

    #[test]
    fn start_builds_graph_sets_levels_defaults_and_routes() {
        let dir = tempfile::tempdir().unwrap();
        let fake = system();
        let mut mixer = mixer(&fake, dir.path());
        let mut config = MixerConfig {
            chatmix: ChatMix { game: 100, chat: 50 },
            ..MixerConfig::default()
        };
        config.channels.chat.level.volume = 80;

        mixer.start(&config).unwrap();
        assert!(mixer.is_running());

        let spawns = fake.spawns();
        let pipewire: Vec<_> = spawns.iter().filter(|s| s.program == "pipewire").collect();
        assert_eq!(pipewire.len(), 1);
        let conf = &mixer.paths().pipewire_conf;
        assert_eq!(
            pipewire[0].args,
            ["-c".to_string(), conf.to_string_lossy().into_owned()]
        );
        let text = std::fs::read_to_string(conf).unwrap();
        assert!(
            text.contains(&format!("target.object = \"{HEADSET}\"")),
            "SteelSeries sink chosen"
        );
        assert!(text.contains(&format!("target.object = \"{HEADSET_MIC}\"")));
        assert!(spawns.iter().any(|s| s.command_line() == "pactl subscribe"));

        let volumes = pactl_calls(&fake, "pactl set-sink-volume");
        assert!(volumes.contains(&"pactl set-sink-volume ssgg_game 100%".to_string()));
        assert!(
            volumes.contains(&"pactl set-sink-volume ssgg_chat 40%".to_string()),
            "{volumes:?}"
        );
        assert!(
            pactl_calls(&fake, "pactl set-source-volume").contains(&"pactl set-source-volume ssgg_mic 100%".into())
        );
        assert!(pactl_calls(&fake, "pactl set-default-sink").contains(&"pactl set-default-sink ssgg_game".into()));
        assert!(pactl_calls(&fake, "pactl set-default-source").contains(&"pactl set-default-source ssgg_mic".into()));
        let moves = pactl_calls(&fake, "pactl move-sink-input");
        assert_eq!(
            moves,
            [
                "pactl move-sink-input 200 ssgg_chat",
                "pactl move-sink-input 201 ssgg_media"
            ],
            "cs2 already on ssgg_game; internal streams untouched"
        );

        let state: SavedState =
            serde_json::from_str(&std::fs::read_to_string(&mixer.paths().state_file).unwrap()).unwrap();
        assert_eq!(state.previous_default_sink.as_deref(), Some(HEADSET));
        assert_eq!(state.previous_default_source.as_deref(), Some(HEADSET_MIC));
        assert!(state.child_pid.is_some());

        fake.clear_calls();
        mixer.stop().unwrap();
        assert!(!mixer.is_running());
        assert_eq!(fake.running_children("pipewire"), 0);
        assert_eq!(fake.running_children("pactl"), 0, "subscribe child stopped");
        assert_eq!(
            pactl_calls(&fake, "pactl set-default"),
            [
                format!("pactl set-default-sink {HEADSET}"),
                format!("pactl set-default-source {HEADSET_MIC}")
            ]
        );
        assert!(!mixer.paths().state_file.exists());
    }

    #[test]
    fn start_refuses_without_pipewire() {
        let dir = tempfile::tempdir().unwrap();
        let fake = system();
        fake.set_missing("pipewire");
        let mut mixer = mixer(&fake, dir.path());
        let err = mixer.start(&MixerConfig::default()).unwrap_err().to_string();
        assert!(err.contains("pipewire"), "{err}");
        assert!(fake.spawns().is_empty());
        assert!(!mixer.paths().state_file.exists());
    }

    #[test]
    fn failed_full_graph_retries_without_optional_features() {
        let dir = tempfile::tempdir().unwrap();
        let fake = system();
        fake.exit_on_spawn("pipewire", 1, 255);
        let mut mixer = mixer(&fake, dir.path());
        let mut config = MixerConfig::default();
        config.mic.noise_suppression = NoiseSuppression {
            enabled: true,
            backend: NoiseSuppressionBackend::Webrtc,
            vad_threshold: 50,
        };
        mixer.start(&config).unwrap();
        assert_eq!(fake.spawns().iter().filter(|s| s.program == "pipewire").count(), 2);
        let text = std::fs::read_to_string(&mixer.paths().pipewire_conf).unwrap();
        assert!(!text.contains("libpipewire-module-echo-cancel"));
        let status = mixer.status().unwrap();
        assert!(status.mic.noise_suppression.is_none());
        assert!(
            status.warnings.iter().any(|w| w.contains("without noise suppression")),
            "{:?}",
            status.warnings
        );
    }

    #[test]
    fn crash_without_optional_features_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let fake = system();
        fake.exit_on_spawn("pipewire", 5, 1);
        let mut mixer = mixer(&fake, dir.path());
        let err = mixer.start(&MixerConfig::default()).unwrap_err().to_string();
        assert!(err.contains("exited with code 1"), "{err}");
        assert!(!mixer.is_running());
        assert!(!mixer.paths().state_file.exists());
    }

    #[test]
    fn nodes_that_never_appear_time_out() {
        let dir = tempfile::tempdir().unwrap();
        let fake = system();
        // Lists never include the mixer's nodes.
        fake.on_while_running(
            "pipewire",
            "pactl",
            &["-f", "json", "list", "sinks"],
            array(&physical_sinks()),
        );
        let mut mixer = mixer(&fake, dir.path());
        let err = mixer.start(&MixerConfig::default()).unwrap_err().to_string();
        assert!(err.contains("did not appear") && err.contains("ssgg_game"), "{err}");
        assert_eq!(fake.running_children("pipewire"), 0, "child not left behind");
    }

    #[test]
    fn stale_run_is_cleaned_and_its_defaults_restored_on_stop() {
        let dir = tempfile::tempdir().unwrap();
        let fake = system();
        let paths = MixerPaths::in_dir(dir.path());
        let crashed = SavedState {
            child_pid: Some(4242),
            previous_default_sink: Some(BUILTIN.into()),
            previous_default_source: None,
        };
        std::fs::write(&paths.state_file, serde_json::to_string(&crashed).unwrap()).unwrap();
        let pattern = stale_process_pattern(&paths.pipewire_conf);
        fake.on("pgrep", &["-f", &pattern], CommandOutput::ok("4242\n"));

        let mut mixer = mixer(&fake, dir.path());
        mixer.start(&MixerConfig::default()).unwrap();
        assert!(fake.command_lines("kill").contains(&"kill -TERM 4242".to_string()));

        fake.clear_calls();
        mixer.stop().unwrap();
        assert!(pactl_calls(&fake, "pactl set-default-sink").contains(&format!("pactl set-default-sink {BUILTIN}")));
    }

    #[test]
    fn stale_pattern_is_anchored_and_escaped() {
        let pattern = stale_process_pattern(Path::new("/run/user/1000/ssgg/mixer-pipewire.conf"));
        assert_eq!(
            pattern,
            r"^([^ ]*/)?pipewire -c /run/user/1000/ssgg/mixer-pipewire\.conf$"
        );
        let pattern = stale_process_pattern(Path::new("/tmp/a+b (1)/x.conf"));
        assert!(pattern.ends_with(r"/tmp/a\+b \(1\)/x\.conf$"));
    }

    #[test]
    fn setters_issue_exact_pactl_calls_while_running() {
        let dir = tempfile::tempdir().unwrap();
        let fake = system();
        let mut mixer = mixer(&fake, dir.path());
        mixer.start(&MixerConfig::default()).unwrap();
        fake.clear_calls();

        mixer.set_volume(Channel::Media, 35).unwrap();
        mixer.set_mute(Channel::Mic, true).unwrap();
        mixer.set_chatmix(ChatMix { game: 30, chat: 100 }).unwrap();
        assert_eq!(
            pactl_calls(&fake, "pactl set-"),
            [
                "pactl set-sink-volume ssgg_media 35%",
                "pactl set-sink-mute ssgg_media 0",
                "pactl set-source-volume ssgg_mic 100%",
                "pactl set-source-mute ssgg_mic 1",
                "pactl set-sink-volume ssgg_game 30%",
                "pactl set-sink-mute ssgg_game 0",
                "pactl set-sink-volume ssgg_chat 100%",
                "pactl set-sink-mute ssgg_chat 0",
            ]
        );
        assert!(mixer.set_volume(Channel::Game, 101).is_err());
        assert_eq!(mixer.config().level(Channel::Media).volume, 35);
    }

    #[test]
    fn setters_only_store_config_when_stopped() {
        let dir = tempfile::tempdir().unwrap();
        let fake = system();
        let mut mixer = mixer(&fake, dir.path());
        mixer.set_volume(Channel::Aux, 20).unwrap();
        mixer
            .set_eq(Channel::Game, &Eq::from_preset(EqPreset::BassBoost))
            .unwrap();
        mixer.set_output(HEADSET).unwrap();
        mixer
            .route_app(RoutingRule::new(MatchField::ProcessBinary, "cs2", Channel::Aux))
            .unwrap();
        assert!(fake.calls().is_empty(), "{:?}", fake.calls());
        assert_eq!(mixer.config().level(Channel::Aux).volume, 20);
        assert_eq!(mixer.config().output, DeviceSelection::Named(HEADSET.into()));
        assert_eq!(mixer.config().routing.rules[0].pattern, "cs2");
        assert!(mixer.route_existing_streams().is_err());
    }

    fn pw_dump_with(node: &str, id: u32) -> CommandOutput {
        CommandOutput::ok(format!(
            r#"[{{"id":0,"type":"PipeWire:Interface:Core","info":{{}}}},
               {{"id":{id},"type":"PipeWire:Interface:Node","info":{{"props":{{"node.name":"{node}"}}}}}}]"#
        ))
    }

    fn running_with_eq_tools() -> (tempfile::TempDir, FakeRunner, Mixer) {
        let dir = tempfile::tempdir().unwrap();
        let fake = system();
        fake.on_while_running("pipewire", "pw-dump", &[], pw_dump_with("ssgg_game", 77));
        let mut mixer = mixer(&fake, dir.path());
        mixer.start(&MixerConfig::default()).unwrap();
        fake.clear_calls();
        (dir, fake, mixer)
    }

    #[test]
    fn eq_value_change_is_applied_live() {
        let (_dir, fake, mut mixer) = running_with_eq_tools();
        mixer
            .set_eq(Channel::Game, &Eq::from_preset(EqPreset::FpsFootsteps))
            .unwrap();
        assert!(fake.spawns().is_empty(), "no restart");
        let calls = fake.command_lines("pw-cli");
        assert_eq!(calls.len(), 1);
        assert!(
            calls[0].starts_with("pw-cli set-param 77 Props { params = [ \"eq_1:Freq\" 32.0"),
            "{}",
            calls[0]
        );
        assert!(calls[0].contains("\"eq_8:Gain\" 5.0"));
        assert_eq!(
            mixer.config().eq(Channel::Game).preset.as_deref(),
            Some("FPS Footsteps")
        );
    }

    #[test]
    fn eq_shape_change_restarts_the_child() {
        let (_dir, fake, mut mixer) = running_with_eq_tools();
        let mut eq = Eq::from_preset(EqPreset::Flat);
        eq.bands.truncate(3);
        mixer.set_eq(Channel::Chat, &eq).unwrap();
        assert!(fake.command_lines("pw-cli").is_empty());
        assert_eq!(fake.spawns().iter().filter(|s| s.program == "pipewire").count(), 1);
        assert_eq!(
            fake.running_children("pipewire"),
            1,
            "old child stopped, new one running"
        );
        let text = std::fs::read_to_string(&mixer.paths().pipewire_conf).unwrap();
        assert_eq!(
            text.matches("label = \"bq_").count(),
            10 + 3 + 10 + 10 + 10,
            "Chat now has 3 bands"
        );
        // Levels, defaults and routing are re-applied after a restart.
        assert!(pactl_calls(&fake, "pactl set-default-sink").contains(&"pactl set-default-sink ssgg_game".into()));
        assert!(!pactl_calls(&fake, "pactl move-sink-input").is_empty());
    }

    #[test]
    fn failed_live_update_falls_back_to_restart() {
        let (_dir, fake, mut mixer) = running_with_eq_tools();
        fake.on_prefix(
            "pw-cli",
            &["set-param"],
            CommandOutput::ok("Error: \"set-param\" failed: Invalid argument"),
        );
        mixer.set_eq(Channel::Game, &Eq::from_preset(EqPreset::Music)).unwrap();
        assert_eq!(fake.command_lines("pw-cli").len(), 1);
        assert_eq!(fake.spawns().iter().filter(|s| s.program == "pipewire").count(), 1);
    }

    #[test]
    fn live_updates_can_be_disabled() {
        let (_dir, fake, mut mixer) = running_with_eq_tools();
        mixer.set_live_eq_updates(false);
        mixer.set_eq(Channel::Game, &Eq::from_preset(EqPreset::Music)).unwrap();
        assert!(fake.command_lines("pw-cli").is_empty());
        assert_eq!(fake.spawns().iter().filter(|s| s.program == "pipewire").count(), 1);
    }

    #[test]
    fn apply_changes_only_what_differs() {
        let dir = tempfile::tempdir().unwrap();
        let fake = system();
        let mut mixer = mixer(&fake, dir.path());
        let mut config = MixerConfig::default();
        config.streamer.enabled = true;
        mixer.start(&config).unwrap();
        fake.clear_calls();

        let mut next = config.clone();
        next.channels.media.level.volume = 55;
        next.streamer.mix.chat = Level {
            volume: 20,
            muted: false,
        };
        mixer.apply(&next).unwrap();
        assert!(fake.spawns().is_empty());
        let writes: Vec<String> = fake
            .command_lines("pactl")
            .into_iter()
            .filter(|c| c.starts_with("pactl set-"))
            .collect();
        assert_eq!(
            writes,
            [
                "pactl set-sink-volume ssgg_media 55%",
                "pactl set-sink-mute ssgg_media 0",
                "pactl set-sink-input-volume 311 20%",
                "pactl set-sink-input-mute 311 0",
            ]
        );

        fake.clear_calls();
        let mut off = next.clone();
        off.streamer.enabled = false;
        mixer.apply(&off).unwrap();
        assert_eq!(
            fake.spawns().iter().filter(|s| s.program == "pipewire").count(),
            1,
            "mode change restarts"
        );
        let text = std::fs::read_to_string(&mixer.paths().pipewire_conf).unwrap();
        assert!(!text.contains("ssgg_stream"));
    }

    #[test]
    fn set_output_to_another_device_restarts() {
        let dir = tempfile::tempdir().unwrap();
        let fake = system();
        let mut mixer = mixer(&fake, dir.path());
        mixer.start(&MixerConfig::default()).unwrap();
        fake.clear_calls();
        mixer.set_output(BUILTIN).unwrap();
        assert_eq!(fake.spawns().iter().filter(|s| s.program == "pipewire").count(), 1);
        let text = std::fs::read_to_string(&mixer.paths().pipewire_conf).unwrap();
        assert!(text.contains(&format!("target.object = \"{BUILTIN}\"")));
        assert_eq!(mixer.status().unwrap().output_device.as_deref(), Some(BUILTIN));
    }

    #[test]
    fn route_app_moves_only_matching_streams() {
        let dir = tempfile::tempdir().unwrap();
        let fake = system();
        let mut mixer = mixer(&fake, dir.path());
        mixer.start(&MixerConfig::default()).unwrap();
        fake.clear_calls();
        mixer
            .route_app(RoutingRule::new(MatchField::ProcessBinary, "cs2", Channel::Aux))
            .unwrap();
        assert_eq!(
            pactl_calls(&fake, "pactl move-sink-input"),
            ["pactl move-sink-input 202 ssgg_aux"]
        );
        assert!(
            mixer
                .route_app(RoutingRule::new(MatchField::Any, "x", Channel::Mic))
                .is_err()
        );
    }

    #[test]
    fn watcher_routes_each_new_stream_once() {
        let fake = system();
        let pactl = Pactl::new(Arc::new(fake.clone()), true);
        let routing = RoutingConfig::default();
        let mut seen: HashSet<u32> = HashSet::from([202]);

        assert_eq!(route_new_streams(&pactl, &routing, &mut seen).unwrap(), 2);
        assert_eq!(
            pactl_calls(&fake, "pactl move-sink-input"),
            [
                "pactl move-sink-input 200 ssgg_chat",
                "pactl move-sink-input 201 ssgg_media"
            ]
        );
        fake.clear_calls();
        assert_eq!(route_new_streams(&pactl, &routing, &mut seen).unwrap(), 0);
        assert!(pactl_calls(&fake, "pactl move-sink-input").is_empty());
        assert_eq!(seen, HashSet::from([200, 201, 202]));
    }

    #[test]
    fn watcher_thread_reacts_to_subscribe_events() {
        let dir = tempfile::tempdir().unwrap();
        let fake = system();
        let mut mixer = mixer(&fake, dir.path());
        mixer.start(&MixerConfig::default()).unwrap();
        let mut inputs = mixer_inputs();
        inputs.push(input_json(
            999,
            100,
            r#""application.name":"Spotify","application.process.binary":"spotify""#,
        ));
        fake.on_while_running(
            "pipewire",
            "pactl",
            &["-f", "json", "list", "sink-inputs"],
            array(&inputs),
        );
        fake.push_stdout("pactl", "Event 'change' on sink #100");
        fake.push_stdout("pactl", "Event 'new' on sink-input #999");

        let deadline = Instant::now() + Duration::from_secs(5);
        while !pactl_calls(&fake, "pactl move-sink-input").contains(&"pactl move-sink-input 999 ssgg_media".into()) {
            assert!(Instant::now() < deadline, "watcher never routed the new stream");
            thread::sleep(Duration::from_millis(10));
        }
        mixer.stop().unwrap();
    }

    #[test]
    fn status_reports_channels_apps_and_devices() {
        let dir = tempfile::tempdir().unwrap();
        let fake = system();
        let mut mixer = mixer(&fake, dir.path());
        let stopped = mixer.status().unwrap();
        assert!(!stopped.running);
        assert!(stopped.channels.iter().all(|c| !c.present));

        let config = MixerConfig {
            chatmix: ChatMix { game: 100, chat: 25 },
            ..MixerConfig::default()
        };
        mixer.start(&config).unwrap();
        let status = mixer.status().unwrap();
        assert!(status.running);
        assert_eq!(status.output_device.as_deref(), Some(HEADSET));
        assert!(status.channels.iter().all(|c| c.present));
        let chat = status.channels.iter().find(|c| c.channel == Channel::Chat).unwrap();
        assert_eq!((chat.volume, chat.effective_volume), (100, 25));
        assert!(status.mic.present);
        assert_eq!(status.mic.input_device.as_deref(), Some(HEADSET_MIC));
        assert_eq!(status.apps.len(), 3, "internal streams hidden");
        let cs2 = status.apps.iter().find(|a| a.index == 202).unwrap();
        assert_eq!(cs2.channel, Some(Channel::Game));
        let discord = status.apps.iter().find(|a| a.index == 200).unwrap();
        assert_eq!(discord.channel, None);
        assert_eq!(discord.sink.as_deref(), Some(HEADSET));
        assert_eq!(discord.rule_channel, Channel::Chat);

        let outputs = mixer.list_outputs().unwrap();
        assert_eq!(
            outputs.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(),
            [BUILTIN, HEADSET]
        );
        assert!(outputs[1].steelseries);
        let inputs = mixer.list_inputs().unwrap();
        assert_eq!(
            inputs.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(),
            [HEADSET_MIC]
        );
        assert_eq!(mixer.list_apps().unwrap().len(), 3);
    }

    #[test]
    fn dead_child_shows_in_status() {
        let dir = tempfile::tempdir().unwrap();
        let fake = system();
        let mut mixer = mixer(&fake, dir.path());
        mixer.start(&MixerConfig::default()).unwrap();
        fake.crash_children("pipewire", 134);
        let status = mixer.status().unwrap();
        assert!(!status.running);
        assert!(status.warnings.iter().any(|w| w.contains("exited")));
        // Starting again replaces the dead session.
        mixer.start(&MixerConfig::default()).unwrap();
        assert!(mixer.is_running());
    }

    #[test]
    fn dropping_a_running_mixer_stops_it() {
        let dir = tempfile::tempdir().unwrap();
        let fake = system();
        let mut mixer = mixer(&fake, dir.path());
        mixer.start(&MixerConfig::default()).unwrap();
        drop(mixer);
        assert_eq!(fake.running_children("pipewire"), 0);
        assert!(!dir.path().join(MixerPaths::STATE_NAME).exists());
    }

    fn node(name: &str, description: &str) -> PaNode {
        PaNode {
            name: name.into(),
            description: description.into(),
            ..PaNode::default()
        }
    }

    #[test]
    fn output_resolution() {
        let sinks = vec![
            node(BUILTIN, "Built-in Audio"),
            node("alsa_output.usb-SteelSeries_Arctis_7-00.mono-chat", "Arctis 7 Chat"),
            node("alsa_output.usb-SteelSeries_Arctis_7-00.analog-stereo", "Arctis 7 Game"),
            node(nodes::GAME, "SteelSeries Game"),
        ];
        let mut warnings = Vec::new();
        assert_eq!(
            resolve_output(&DeviceSelection::Auto, &sinks, Some(BUILTIN), &mut warnings).unwrap(),
            "alsa_output.usb-SteelSeries_Arctis_7-00.analog-stereo"
        );
        assert_eq!(
            resolve_output(&DeviceSelection::Named(BUILTIN.into()), &sinks, None, &mut warnings).unwrap(),
            BUILTIN
        );
        assert!(warnings.is_empty());
        let plain = vec![node("a", "A"), node("b", "B"), node(nodes::GAME, "SteelSeries Game")];
        assert_eq!(
            resolve_output(&DeviceSelection::Auto, &plain, Some("b"), &mut warnings).unwrap(),
            "b"
        );
        assert_eq!(
            resolve_output(&DeviceSelection::Auto, &plain, Some(nodes::GAME), &mut warnings).unwrap(),
            "a",
            "never pick one of our own nodes"
        );
        assert_eq!(
            resolve_output(&DeviceSelection::Named("gone".into()), &plain, Some("b"), &mut warnings).unwrap(),
            "b"
        );
        assert_eq!(warnings.len(), 1);
        assert!(resolve_output(&DeviceSelection::Auto, &[node(nodes::GAME, "x")], None, &mut warnings).is_err());
    }

    #[test]
    fn input_resolution_skips_monitors() {
        let sources = vec![
            node(&format!("{HEADSET}.monitor"), "Monitor of Arctis"),
            node("alsa_input.pci.analog-stereo", "Built-in Mic"),
            node(HEADSET_MIC, "Arctis Nova 7 Mono"),
            node(nodes::MIC, "SteelSeries Microphone"),
        ];
        let mut warnings = Vec::new();
        assert_eq!(
            resolve_input(&DeviceSelection::Auto, &sources, None, &mut warnings).as_deref(),
            Some(HEADSET_MIC)
        );
        assert_eq!(
            resolve_input(&DeviceSelection::Auto, &sources[..1], None, &mut warnings),
            None
        );
    }

    #[test]
    fn pw_dump_node_lookup() {
        let dump = pw_dump_with("ssgg_chat", 91).stdout;
        assert_eq!(find_node_id(&dump, "ssgg_chat"), Some(91));
        assert_eq!(find_node_id(&dump, "ssgg_game"), None);
        assert_eq!(find_node_id("not json", "ssgg_chat"), None);
    }
}
