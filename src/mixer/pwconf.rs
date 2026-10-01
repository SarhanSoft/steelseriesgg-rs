//! The standalone PipeWire config the mixer runs as `pipewire -c <file>`.
//!
//! One client-side PipeWire instance hosts every mixer node, so stopping (or killing) that one
//! process removes all of them. Contents:
//!
//! - one `libpipewire-module-filter-chain` per output channel: capture side is the virtual
//!   `Audio/Sink` (`ssgg_game`, ...), the graph is the channel's EQ (plus the `sofa`
//!   spatializer on Game when spatial audio is on), playback side targets the physical sink;
//! - the microphone chain: optional `libpipewire-module-echo-cancel` (WebRTC noise
//!   suppression), then a filter-chain with optional RNNoise LADSPA plugin and the mic EQ,
//!   whose playback side is the virtual `Audio/Source` `ssgg_mic`;
//! - in streamer mode, a `libpipewire-module-loopback` providing the "SteelSeries Stream" sink
//!   and one loopback per channel copying that channel into it at its own volume.
//!
//! The structure follows PipeWire's shipped examples (`filter-chain/sink-eq6.conf`,
//! `source-rnnoise.conf`, `spatializer-7.1.conf`, the virtual-devices wiki page).

use std::path::{Path, PathBuf};

use super::config::{MixerConfig, NoiseSuppressionBackend};
use super::eq::{BandType, EqBand};
use super::{Channel, nodes};

/// Plugin name PipeWire searches for when the RNNoise library was not found on disk.
const RNNOISE_FALLBACK_PLUGIN: &str = "librnnoise_ladspa";

/// Noise suppression stage of the microphone chain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoiseSuppressionPlan {
    /// `libpipewire-module-echo-cancel` with `aec/libspa-aec-webrtc`.
    Webrtc,
    /// `noise_suppressor_mono` from `librnnoise_ladspa.so` inside the mic filter-chain.
    Rnnoise { plugin: String, vad_threshold: u8 },
}

/// Graph of one output channel.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelGraph {
    pub channel: Channel,
    pub bands: Vec<EqBand>,
    /// SOFA file for virtual 7.1 surround (Game only).
    pub spatial_sofa: Option<PathBuf>,
}

/// Graph of the microphone chain.
#[derive(Clone, Debug, PartialEq)]
pub struct MicGraph {
    /// Physical source the chain records from.
    pub input_source: String,
    pub noise_suppression: Option<NoiseSuppressionPlan>,
    pub bands: Vec<EqBand>,
}

/// Everything that goes into the generated config, with devices already resolved to names.
#[derive(Clone, Debug, PartialEq)]
pub struct GraphPlan {
    pub output_sink: String,
    pub channels: Vec<ChannelGraph>,
    pub mic: Option<MicGraph>,
    pub streamer: bool,
}

/// What forces a restart of the PipeWire child when it changes: everything except EQ
/// control values.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Topology {
    output_sink: String,
    channels: Vec<(Channel, Vec<BandType>, Option<PathBuf>)>,
    mic: Option<(String, Option<NoiseSuppressionPlan>, Vec<BandType>)>,
    streamer: bool,
}

fn shape(bands: &[EqBand]) -> Vec<BandType> {
    bands.iter().map(|b| b.kind).collect()
}

impl GraphPlan {
    /// Build a plan from `config` and resolved devices. Returns warnings for features that
    /// had to be left out.
    pub fn build(
        config: &MixerConfig,
        output_sink: String,
        input_source: Option<String>,
        rnnoise_plugin: Option<&Path>,
        sofa_file_exists: bool,
    ) -> (Self, Vec<String>) {
        let mut warnings = Vec::new();

        let channels = Channel::OUTPUTS
            .into_iter()
            .map(|channel| {
                let spatial_sofa = if channel == Channel::Game && config.spatial.enabled {
                    match &config.spatial.sofa_path {
                        Some(path) if sofa_file_exists => Some(path.clone()),
                        Some(path) => {
                            warnings.push(format!(
                                "spatial audio disabled: SOFA file {} does not exist",
                                path.display()
                            ));
                            None
                        }
                        None => {
                            warnings.push("spatial audio disabled: no SOFA file configured".to_string());
                            None
                        }
                    }
                } else {
                    None
                };
                ChannelGraph {
                    channel,
                    bands: config.eq(channel).bands.clone(),
                    spatial_sofa,
                }
            })
            .collect();

        let mic = if !config.mic.enabled {
            None
        } else if let Some(input_source) = input_source {
            let ns = &config.mic.noise_suppression;
            let rnnoise = |warnings: &mut Vec<String>| {
                let plugin = match rnnoise_plugin {
                    Some(path) => path.to_string_lossy().into_owned(),
                    None => {
                        warnings.push(
                            "librnnoise_ladspa.so was not found in the standard LADSPA paths; \
                             relying on PipeWire's own plugin search"
                                .to_string(),
                        );
                        RNNOISE_FALLBACK_PLUGIN.to_string()
                    }
                };
                NoiseSuppressionPlan::Rnnoise {
                    plugin,
                    vad_threshold: ns.vad_threshold.min(100),
                }
            };
            let noise_suppression = ns.enabled.then(|| match ns.backend {
                NoiseSuppressionBackend::Webrtc => NoiseSuppressionPlan::Webrtc,
                NoiseSuppressionBackend::Rnnoise => rnnoise(&mut warnings),
                NoiseSuppressionBackend::Auto if rnnoise_plugin.is_some() => rnnoise(&mut warnings),
                NoiseSuppressionBackend::Auto => NoiseSuppressionPlan::Webrtc,
            });
            Some(MicGraph {
                input_source,
                noise_suppression,
                bands: config.mic.eq.bands.clone(),
            })
        } else {
            warnings.push("no microphone found; the SteelSeries Microphone source was not created".to_string());
            None
        };

        let plan = Self {
            output_sink,
            channels,
            mic,
            streamer: config.streamer.enabled,
        };
        (plan, warnings)
    }

    pub(crate) fn topology(&self) -> Topology {
        Topology {
            output_sink: self.output_sink.clone(),
            channels: self
                .channels
                .iter()
                .map(|c| (c.channel, shape(&c.bands), c.spatial_sofa.clone()))
                .collect(),
            mic: self
                .mic
                .as_ref()
                .map(|m| (m.input_source.clone(), m.noise_suppression.clone(), shape(&m.bands))),
            streamer: self.streamer,
        }
    }

    /// Noise suppression or spatial audio is in the plan (features whose PipeWire modules or
    /// plugins may be missing).
    pub fn has_optional_features(&self) -> bool {
        self.channels.iter().any(|c| c.spatial_sofa.is_some())
            || self.mic.as_ref().is_some_and(|m| m.noise_suppression.is_some())
    }

    pub fn without_optional_features(&self) -> Self {
        let mut plan = self.clone();
        for channel in &mut plan.channels {
            channel.spatial_sofa = None;
        }
        if let Some(mic) = &mut plan.mic {
            mic.noise_suppression = None;
        }
        plan
    }

    pub fn spatial_active(&self) -> bool {
        self.channels.iter().any(|c| c.spatial_sofa.is_some())
    }

    /// Sinks that exist once the child is up.
    pub fn expected_sinks(&self) -> Vec<&'static str> {
        let mut sinks: Vec<&'static str> = self.channels.iter().map(|c| c.channel.node_name()).collect();
        if self.streamer {
            sinks.push(nodes::STREAM);
        }
        sinks
    }

    /// Sources that exist once the child is up.
    pub fn expected_sources(&self) -> Vec<&'static str> {
        let mut sources = Vec::new();
        if self.mic.is_some() {
            sources.push(nodes::MIC);
        }
        if self.streamer {
            sources.push(nodes::STREAM_OUT);
        }
        sources
    }

    /// Streaming-mix taps (playback node names) present in this plan.
    pub fn stream_taps(&self) -> Vec<(Channel, String)> {
        if !self.streamer {
            return Vec::new();
        }
        let mut taps: Vec<Channel> = self.channels.iter().map(|c| c.channel).collect();
        if self.mic.is_some() {
            taps.push(Channel::Mic);
        }
        taps.into_iter().map(|c| (c, stream_tap_node(c))).collect()
    }

    pub fn bands(&self, channel: Channel) -> Option<&[EqBand]> {
        if channel == Channel::Mic {
            return self.mic.as_ref().map(|m| m.bands.as_slice());
        }
        self.channels
            .iter()
            .find(|c| c.channel == channel)
            .map(|c| c.bands.as_slice())
    }

    pub(crate) fn set_bands(&mut self, channel: Channel, bands: &[EqBand]) {
        if channel == Channel::Mic {
            if let Some(mic) = &mut self.mic {
                mic.bands = bands.to_vec();
            }
        } else if let Some(graph) = self.channels.iter_mut().find(|c| c.channel == channel) {
            graph.bands = bands.to_vec();
        }
    }

    /// Filter-chain node holding the EQ controls of `channel`, and the `Props` value that sets
    /// every band to `bands` (for `pw-cli set-param <id> Props <value>`).
    pub(crate) fn eq_props(&self, channel: Channel, bands: &[EqBand]) -> Option<(&'static str, String)> {
        let (node, prefixes): (&'static str, Vec<&str>) = if channel == Channel::Mic {
            self.mic.as_ref()?;
            (nodes::MIC_IN, vec!["eq"])
        } else {
            let graph = self.channels.iter().find(|c| c.channel == channel)?;
            let prefixes = if graph.spatial_sofa.is_some() {
                vec!["eql", "eqr"]
            } else {
                vec!["eq"]
            };
            (channel.node_name(), prefixes)
        };
        let mut params = Vec::new();
        for prefix in prefixes {
            for (i, band) in bands.iter().enumerate() {
                let name = band_node_name(prefix, i);
                for (control, value) in [("Freq", band.freq), ("Q", band.q), ("Gain", band.gain)] {
                    params.push(quote(&format!("{name}:{control}")));
                    params.push(float(value));
                }
            }
        }
        Some((node, format!("{{ params = [ {} ] }}", params.join(" "))))
    }
}

/// Playback node of the streaming-mix tap for `channel`.
pub fn stream_tap_node(channel: Channel) -> String {
    format!("{}stream_{}", nodes::PREFIX, channel.key())
}

fn band_node_name(prefix: &str, index: usize) -> String {
    format!("{prefix}_{}", index + 1)
}

// ---------------------------------------------------------------------------------------------
// SPA-JSON writer
// ---------------------------------------------------------------------------------------------

/// A SPA-JSON value. Every string is written quoted, so device names with spaces or symbols
/// stay intact.
#[derive(Clone, Debug)]
enum Spa {
    Str(String),
    Int(i64),
    Float(f32),
    Bool(bool),
    Arr(Vec<Spa>),
    Obj(Vec<(String, Spa)>),
}

impl Spa {
    fn s(value: impl Into<String>) -> Self {
        Spa::Str(value.into())
    }

    fn obj(pairs: Vec<(&str, Spa)>) -> Self {
        Spa::Obj(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }

    fn strs<S: AsRef<str>>(items: &[S]) -> Self {
        Spa::Arr(items.iter().map(|s| Spa::s(s.as_ref())).collect())
    }

    fn is_scalar(&self) -> bool {
        !matches!(self, Spa::Arr(_) | Spa::Obj(_))
    }

    fn write(&self, out: &mut String, indent: usize) {
        match self {
            Spa::Str(s) => out.push_str(&quote(s)),
            Spa::Int(i) => out.push_str(&i.to_string()),
            Spa::Float(f) => out.push_str(&float(*f)),
            Spa::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Spa::Arr(items) if items.is_empty() => out.push_str("[ ]"),
            Spa::Arr(items) if items.iter().all(Spa::is_scalar) => {
                out.push('[');
                for item in items {
                    out.push(' ');
                    item.write(out, indent);
                }
                out.push_str(" ]");
            }
            Spa::Arr(items) => {
                out.push_str("[\n");
                for item in items {
                    pad(out, indent + 1);
                    item.write(out, indent + 1);
                    out.push('\n');
                }
                pad(out, indent);
                out.push(']');
            }
            Spa::Obj(pairs) if pairs.is_empty() => out.push_str("{ }"),
            Spa::Obj(pairs) if pairs.len() <= 3 && pairs.iter().all(|(_, v)| v.is_scalar()) => {
                out.push('{');
                for (key, value) in pairs {
                    out.push(' ');
                    out.push_str(&spa_key(key));
                    out.push_str(" = ");
                    value.write(out, indent);
                }
                out.push_str(" }");
            }
            Spa::Obj(pairs) => {
                out.push_str("{\n");
                for (key, value) in pairs {
                    pad(out, indent + 1);
                    out.push_str(&spa_key(key));
                    out.push_str(" = ");
                    value.write(out, indent + 1);
                    out.push('\n');
                }
                pad(out, indent);
                out.push('}');
            }
        }
    }
}

fn pad(out: &mut String, indent: usize) {
    for _ in 0..indent {
        out.push_str("    ");
    }
}

/// SPA-JSON string literal.
fn quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Keys made only of letters, digits, `.`, `-` and `_` are written bare, like PipeWire's own
/// configs; anything else is quoted.
fn spa_key(key: &str) -> String {
    let bare = !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
    if bare { key.to_string() } else { quote(key) }
}

/// Decimal float that SPA-JSON parses as a float (`60` → `60.0`).
fn float(value: f32) -> String {
    if !value.is_finite() {
        return "0.0".to_string();
    }
    let text = value.to_string();
    if text.contains('.') { text } else { format!("{text}.0") }
}

// ---------------------------------------------------------------------------------------------
// Config generation
// ---------------------------------------------------------------------------------------------

const STEREO: [&str; 2] = ["FL", "FR"];
const MONO: [&str; 1] = ["MONO"];
/// Virtual speaker layout for spatial audio: channel and azimuth in degrees, as in PipeWire's
/// `spatializer-7.1.conf` example.
const SURROUND: [(&str, f32); 8] = [
    ("FL", 30.0),
    ("FR", 330.0),
    ("FC", 0.0),
    ("LFE", 0.0),
    ("RL", 150.0),
    ("RR", 210.0),
    ("SL", 90.0),
    ("SR", 270.0),
];

fn module(name: &str, args: Option<Spa>, flags: &[&str]) -> Spa {
    let mut pairs = vec![("name", Spa::s(name))];
    if let Some(args) = args {
        pairs.push(("args", args));
    }
    if !flags.is_empty() {
        pairs.push(("flags", Spa::strs(flags)));
    }
    Spa::obj(pairs)
}

fn link(output: &str, input: &str) -> Spa {
    Spa::obj(vec![("output", Spa::s(output)), ("input", Spa::s(input))])
}

fn audio_layout(pairs: &mut Vec<(&str, Spa)>, positions: &[&str]) {
    pairs.push(("audio.channels", Spa::Int(positions.len() as i64)));
    pairs.push(("audio.position", Spa::strs(positions)));
}

/// Properties that keep a stream on its target: if the target disappears, the stream stays
/// unlinked instead of falling back to the default device. The default sink is `ssgg_game`
/// while the mixer runs, so a fallback could feed one mixer node into another.
fn pin_to_target(pairs: &mut Vec<(&str, Spa)>, target: &str) {
    pairs.push(("target.object", Spa::s(target)));
    pairs.push(("node.dont-reconnect", Spa::Bool(true)));
    pairs.push(("node.dont-fallback", Spa::Bool(true)));
}

/// Linear chain of biquads named `{prefix}_1..n`.
struct Chain {
    nodes: Vec<Spa>,
    links: Vec<Spa>,
    first_input: String,
    last_output: String,
}

fn eq_chain(prefix: &str, bands: &[EqBand]) -> Option<Chain> {
    if bands.is_empty() {
        return None;
    }
    let names: Vec<String> = (0..bands.len()).map(|i| band_node_name(prefix, i)).collect();
    let nodes = bands
        .iter()
        .zip(&names)
        .map(|(band, name)| {
            Spa::obj(vec![
                ("type", Spa::s("builtin")),
                ("name", Spa::s(name.as_str())),
                ("label", Spa::s(band.kind.builtin_label())),
                (
                    "control",
                    Spa::obj(vec![
                        ("Freq", Spa::Float(band.freq)),
                        ("Q", Spa::Float(band.q)),
                        ("Gain", Spa::Float(band.gain)),
                    ]),
                ),
            ])
        })
        .collect();
    let links = names
        .windows(2)
        .map(|pair| link(&format!("{}:Out", pair[0]), &format!("{}:In", pair[1])))
        .collect();
    Some(Chain {
        nodes,
        links,
        first_input: format!("{}:In", names.first()?),
        last_output: format!("{}:Out", names.last()?),
    })
}

fn copy_node(name: &str) -> Spa {
    Spa::obj(vec![
        ("type", Spa::s("builtin")),
        ("name", Spa::s(name)),
        ("label", Spa::s("copy")),
    ])
}

fn graph(nodes: Vec<Spa>, links: Vec<Spa>, inputs: Vec<String>, outputs: Vec<String>) -> Spa {
    Spa::obj(vec![
        ("nodes", Spa::Arr(nodes)),
        ("links", Spa::Arr(links)),
        ("inputs", Spa::strs(&inputs)),
        ("outputs", Spa::strs(&outputs)),
    ])
}

/// Mono EQ graph; filter-chain runs one copy per audio channel.
fn eq_graph(bands: &[EqBand]) -> Spa {
    match eq_chain("eq", bands) {
        Some(chain) => graph(
            chain.nodes,
            chain.links,
            vec![chain.first_input],
            vec![chain.last_output],
        ),
        None => graph(
            vec![copy_node("copy")],
            Vec::new(),
            vec!["copy:In".into()],
            vec!["copy:Out".into()],
        ),
    }
}

/// 7.1 in, binaural stereo out: one spatializer per speaker, summed per ear, then the EQ on
/// each ear.
fn spatial_graph(sofa: &Path, bands: &[EqBand]) -> Spa {
    let mut nodes = Vec::new();
    let mut links = Vec::new();
    let mut inputs = Vec::new();
    for (i, (position, azimuth)) in SURROUND.iter().enumerate() {
        let name = format!("sp_{position}");
        nodes.push(Spa::obj(vec![
            ("type", Spa::s("sofa")),
            ("label", Spa::s("spatializer")),
            ("name", Spa::s(name.as_str())),
            (
                "config",
                Spa::obj(vec![("filename", Spa::s(sofa.to_string_lossy().into_owned()))]),
            ),
            (
                "control",
                Spa::obj(vec![
                    ("Azimuth", Spa::Float(*azimuth)),
                    ("Elevation", Spa::Float(0.0)),
                    ("Radius", Spa::Float(3.0)),
                ]),
            ),
        ]));
        links.push(link(&format!("{name}:Out L"), &format!("mix_l:In {}", i + 1)));
        links.push(link(&format!("{name}:Out R"), &format!("mix_r:In {}", i + 1)));
        inputs.push(format!("{name}:In"));
    }
    for mixer in ["mix_l", "mix_r"] {
        nodes.push(Spa::obj(vec![
            ("type", Spa::s("builtin")),
            ("label", Spa::s("mixer")),
            ("name", Spa::s(mixer)),
        ]));
    }
    let mut outputs = Vec::new();
    for (mixer, prefix) in [("mix_l", "eql"), ("mix_r", "eqr")] {
        match eq_chain(prefix, bands) {
            Some(chain) => {
                links.push(link(&format!("{mixer}:Out"), &chain.first_input));
                nodes.extend(chain.nodes);
                links.extend(chain.links);
                outputs.push(chain.last_output);
            }
            None => outputs.push(format!("{mixer}:Out")),
        }
    }
    graph(nodes, links, inputs, outputs)
}

fn output_channel_module(channel: &ChannelGraph, output_sink: &str) -> Spa {
    let description = channel.channel.description();
    let (filter_graph, capture_positions): (Spa, Vec<&str>) = match &channel.spatial_sofa {
        Some(sofa) => (
            spatial_graph(sofa, &channel.bands),
            SURROUND.iter().map(|(p, _)| *p).collect(),
        ),
        None => (eq_graph(&channel.bands), STEREO.to_vec()),
    };

    let mut capture = vec![
        ("node.name", Spa::s(channel.channel.node_name())),
        ("node.description", Spa::s(description)),
        ("media.class", Spa::s("Audio/Sink")),
    ];
    audio_layout(&mut capture, &capture_positions);

    let out_name = format!("{}_out", channel.channel.node_name());
    let mut playback = vec![
        ("node.name", Spa::s(out_name)),
        ("node.description", Spa::s(format!("{description} (output)"))),
        ("node.passive", Spa::Bool(true)),
    ];
    pin_to_target(&mut playback, output_sink);
    audio_layout(&mut playback, &STEREO);

    module(
        "libpipewire-module-filter-chain",
        Some(Spa::obj(vec![
            ("node.description", Spa::s(description)),
            ("media.name", Spa::s(description)),
            ("filter.graph", filter_graph),
            ("capture.props", Spa::obj(capture)),
            ("playback.props", Spa::obj(playback)),
        ])),
        &[],
    )
}

fn echo_cancel_module(mic: &MicGraph, output_sink: &str) -> Spa {
    let mut capture = vec![
        ("node.name", Spa::s(nodes::MIC_NS_IN)),
        (
            "node.description",
            Spa::s("SteelSeries Microphone (noise suppression input)"),
        ),
        ("node.passive", Spa::Bool(true)),
    ];
    pin_to_target(&mut capture, &mic.input_source);

    let mut playback = vec![
        ("node.name", Spa::s(nodes::MIC_NS_REF_OUT)),
        ("node.description", Spa::s("SteelSeries Mic Echo Reference (output)")),
        ("node.passive", Spa::Bool(true)),
    ];
    pin_to_target(&mut playback, output_sink);

    let mut args = vec![("library.name", Spa::s("aec/libspa-aec-webrtc"))];
    audio_layout(&mut args, &MONO);
    args.extend([
        (
            "aec.args",
            Spa::obj(vec![
                ("webrtc.noise_suppression", Spa::Bool(true)),
                ("webrtc.high_pass_filter", Spa::Bool(true)),
                ("webrtc.gain_control", Spa::Bool(false)),
                ("webrtc.extended_filter", Spa::Bool(true)),
                ("webrtc.delay_agnostic", Spa::Bool(true)),
            ]),
        ),
        ("capture.props", Spa::obj(capture)),
        (
            "source.props",
            Spa::obj(vec![
                ("node.name", Spa::s(nodes::MIC_NS)),
                (
                    "node.description",
                    Spa::s("SteelSeries Microphone (noise suppressed, internal)"),
                ),
            ]),
        ),
        (
            "sink.props",
            Spa::obj(vec![
                ("node.name", Spa::s(nodes::MIC_NS_REF)),
                ("node.description", Spa::s("SteelSeries Mic Echo Reference")),
            ]),
        ),
        ("playback.props", Spa::obj(playback)),
    ]);
    module("libpipewire-module-echo-cancel", Some(Spa::obj(args)), &[])
}

fn mic_module(mic: &MicGraph) -> Spa {
    let mut nodes_list = Vec::new();
    let mut links = Vec::new();
    let mut first_input: Option<String> = None;
    let mut last_output: Option<String> = None;
    let rnnoise = matches!(mic.noise_suppression, Some(NoiseSuppressionPlan::Rnnoise { .. }));

    if let Some(NoiseSuppressionPlan::Rnnoise { plugin, vad_threshold }) = &mic.noise_suppression {
        nodes_list.push(Spa::obj(vec![
            ("type", Spa::s("ladspa")),
            ("name", Spa::s("rnnoise")),
            ("plugin", Spa::s(plugin.as_str())),
            ("label", Spa::s("noise_suppressor_mono")),
            (
                "control",
                Spa::obj(vec![
                    ("VAD Threshold (%)", Spa::Float(f32::from(*vad_threshold))),
                    ("VAD Grace Period (ms)", Spa::Float(200.0)),
                    ("Retroactive VAD Grace (ms)", Spa::Float(0.0)),
                ]),
            ),
        ]));
        first_input = Some("rnnoise:Input".to_string());
        last_output = Some("rnnoise:Output".to_string());
    }
    if let Some(chain) = eq_chain("eq", &mic.bands) {
        if let Some(previous) = &last_output {
            links.push(link(previous, &chain.first_input));
        }
        first_input.get_or_insert(chain.first_input);
        last_output = Some(chain.last_output);
        nodes_list.extend(chain.nodes);
        links.extend(chain.links);
    }
    let filter_graph = match (first_input, last_output) {
        (Some(input), Some(output)) => graph(nodes_list, links, vec![input], vec![output]),
        _ => graph(
            vec![copy_node("copy")],
            Vec::new(),
            vec!["copy:In".into()],
            vec!["copy:Out".into()],
        ),
    };

    let target = match mic.noise_suppression {
        Some(NoiseSuppressionPlan::Webrtc) => nodes::MIC_NS,
        _ => mic.input_source.as_str(),
    };
    let mut capture = vec![
        ("node.name", Spa::s(nodes::MIC_IN)),
        ("node.description", Spa::s("SteelSeries Microphone (input)")),
    ];
    // Like PipeWire's source-rnnoise example: the stage that reads the physical microphone is
    // passive, so the mic only runs while something records from ssgg_mic.
    if !matches!(mic.noise_suppression, Some(NoiseSuppressionPlan::Webrtc)) {
        capture.push(("node.passive", Spa::Bool(true)));
    }
    pin_to_target(&mut capture, target);
    audio_layout(&mut capture, &MONO);

    let mut playback = vec![
        ("node.name", Spa::s(nodes::MIC)),
        ("node.description", Spa::s(Channel::Mic.description())),
        ("media.class", Spa::s("Audio/Source")),
    ];
    audio_layout(&mut playback, &MONO);
    if rnnoise {
        // RNNoise only works at 48 kHz.
        capture.push(("audio.rate", Spa::Int(48_000)));
        playback.push(("audio.rate", Spa::Int(48_000)));
    }

    module(
        "libpipewire-module-filter-chain",
        Some(Spa::obj(vec![
            ("node.description", Spa::s(Channel::Mic.description())),
            ("media.name", Spa::s(Channel::Mic.description())),
            ("filter.graph", filter_graph),
            ("capture.props", Spa::obj(capture)),
            ("playback.props", Spa::obj(playback)),
        ])),
        &[],
    )
}

/// "SteelSeries Stream": a sink (OBS captures its monitor) whose loopback output is also
/// offered as a source. The source side keeps the session manager from linking the loopback
/// to the headset.
fn stream_sink_module() -> Spa {
    let mut capture = vec![
        ("node.name", Spa::s(nodes::STREAM)),
        ("node.description", Spa::s("SteelSeries Stream")),
        ("media.class", Spa::s("Audio/Sink")),
    ];
    audio_layout(&mut capture, &STEREO);
    let mut playback = vec![
        ("node.name", Spa::s(nodes::STREAM_OUT)),
        ("node.description", Spa::s("SteelSeries Stream (source)")),
        ("media.class", Spa::s("Audio/Source")),
    ];
    audio_layout(&mut playback, &STEREO);
    module(
        "libpipewire-module-loopback",
        Some(Spa::obj(vec![
            ("node.description", Spa::s("SteelSeries Stream")),
            ("capture.props", Spa::obj(capture)),
            ("playback.props", Spa::obj(playback)),
        ])),
        &[],
    )
}

/// Copies one channel into the stream sink. Output channels are tapped at their sink's
/// monitor, which PipeWire takes before the sink volume (`monitor.channel-volumes` defaults to
/// false), so the streaming mix does not follow the monitoring sliders or ChatMix.
fn stream_tap_module(channel: Channel) -> Spa {
    let tap = stream_tap_node(channel);
    let description = channel.description();
    let label = format!(
        "SteelSeries Stream: {}",
        description.strip_prefix("SteelSeries ").unwrap_or(description)
    );
    let positions: &[&str] = if channel.is_output() { &STEREO } else { &MONO };

    let mut capture = vec![
        ("node.name", Spa::s(format!("{tap}_in"))),
        ("node.description", Spa::s(format!("{label} (tap)"))),
    ];
    if channel.is_output() {
        capture.push(("stream.capture.sink", Spa::Bool(true)));
    }
    pin_to_target(&mut capture, channel.node_name());
    audio_layout(&mut capture, positions);

    let mut playback = vec![
        ("node.name", Spa::s(tap.as_str())),
        ("node.description", Spa::s(label.as_str())),
    ];
    pin_to_target(&mut playback, nodes::STREAM);
    audio_layout(&mut playback, positions);

    module(
        "libpipewire-module-loopback",
        Some(Spa::obj(vec![
            ("node.description", Spa::s(label.as_str())),
            ("capture.props", Spa::obj(capture)),
            ("playback.props", Spa::obj(playback)),
        ])),
        &[],
    )
}

/// Render the full config file for `plan`.
pub fn generate_pipewire_config(plan: &GraphPlan) -> String {
    let mut modules = vec![
        module(
            "libpipewire-module-rt",
            Some(Spa::Obj(Vec::new())),
            &["ifexists", "nofail"],
        ),
        module("libpipewire-module-protocol-native", None, &[]),
        module("libpipewire-module-client-node", None, &[]),
        module("libpipewire-module-adapter", None, &[]),
    ];
    modules.extend(
        plan.channels
            .iter()
            .map(|c| output_channel_module(c, &plan.output_sink)),
    );
    if let Some(mic) = &plan.mic {
        if mic.noise_suppression == Some(NoiseSuppressionPlan::Webrtc) {
            modules.push(echo_cancel_module(mic, &plan.output_sink));
        }
        modules.push(mic_module(mic));
    }
    if plan.streamer {
        modules.push(stream_sink_module());
        modules.extend(plan.stream_taps().into_iter().map(|(c, _)| stream_tap_module(c)));
    }

    let sections = [
        ("context.properties", Spa::obj(vec![("log.level", Spa::Int(2))])),
        (
            "context.spa-libs",
            Spa::obj(vec![
                ("audio.convert.*", Spa::s("audioconvert/libspa-audioconvert")),
                ("support.*", Spa::s("support/libspa-support")),
            ]),
        ),
        ("context.modules", Spa::Arr(modules)),
    ];

    let mut out = String::from(
        "# Generated by steelseries-gg-linux (ssgg mixer). Rewritten on every start; do not edit.\n\
         # Runs as a client of the user's PipeWire daemon: pipewire -c <this file>\n\n",
    );
    for (key, value) in sections {
        out.push_str(&spa_key(key));
        out.push_str(" = ");
        value.write(&mut out, 0);
        out.push_str("\n\n");
    }
    out
}

#[cfg(test)]
pub(crate) mod spa_parser {
    //! Minimal SPA-JSON parser used only to check the generated config's structure.

    #[derive(Clone, Debug, PartialEq)]
    pub enum Val {
        Obj(Vec<(String, Val)>),
        Arr(Vec<Val>),
        Str(String),
        Bare(String),
    }

    impl Val {
        pub fn get(&self, key: &str) -> Option<&Val> {
            match self {
                Val::Obj(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
                _ => None,
            }
        }

        pub fn path(&self, keys: &[&str]) -> Option<&Val> {
            keys.iter().try_fold(self, |v, k| v.get(k))
        }

        /// Text of a quoted string or a bare word.
        pub fn text(&self) -> Option<&str> {
            match self {
                Val::Str(s) | Val::Bare(s) => Some(s),
                _ => None,
            }
        }

        pub fn items(&self) -> &[Val] {
            match self {
                Val::Arr(items) => items,
                _ => &[],
            }
        }

        /// Every value stored under `key`, anywhere in the tree.
        pub fn find_all<'a>(&'a self, key: &str, out: &mut Vec<&'a Val>) {
            match self {
                Val::Obj(pairs) => {
                    for (k, v) in pairs {
                        if k == key {
                            out.push(v);
                        }
                        v.find_all(key, out);
                    }
                }
                Val::Arr(items) => items.iter().for_each(|v| v.find_all(key, out)),
                _ => {}
            }
        }
    }

    struct Parser {
        chars: Vec<char>,
        pos: usize,
    }

    const SPECIAL: &[char] = &['{', '}', '[', ']', '=', ':', ',', '"', '#'];

    impl Parser {
        fn skip(&mut self) {
            while let Some(&c) = self.chars.get(self.pos) {
                if c.is_whitespace() || c == ',' {
                    self.pos += 1;
                } else if c == '#' {
                    while self.chars.get(self.pos).is_some_and(|&c| c != '\n') {
                        self.pos += 1;
                    }
                } else {
                    break;
                }
            }
        }

        fn peek(&self) -> Option<char> {
            self.chars.get(self.pos).copied()
        }

        fn string(&mut self) -> Result<String, String> {
            self.pos += 1;
            let mut out = String::new();
            loop {
                match self.peek() {
                    None => return Err("unterminated string".into()),
                    Some('"') => {
                        self.pos += 1;
                        return Ok(out);
                    }
                    Some('\\') => {
                        self.pos += 1;
                        match self.peek() {
                            Some('n') => out.push('\n'),
                            Some('t') => out.push('\t'),
                            Some('r') => out.push('\r'),
                            Some(c) => out.push(c),
                            None => return Err("dangling escape".into()),
                        }
                        self.pos += 1;
                    }
                    Some(c) => {
                        out.push(c);
                        self.pos += 1;
                    }
                }
            }
        }

        fn bare(&mut self) -> Result<String, String> {
            let start = self.pos;
            while self.peek().is_some_and(|c| !c.is_whitespace() && !SPECIAL.contains(&c)) {
                self.pos += 1;
            }
            if start == self.pos {
                return Err(format!("unexpected {:?} at offset {}", self.peek(), self.pos));
            }
            Ok(self.chars[start..self.pos].iter().collect())
        }

        fn key(&mut self) -> Result<String, String> {
            self.skip();
            let key = if self.peek() == Some('"') {
                self.string()?
            } else {
                self.bare()?
            };
            self.skip();
            match self.peek() {
                Some('=') | Some(':') => {
                    self.pos += 1;
                    Ok(key)
                }
                other => Err(format!("expected '=' after key {key:?}, found {other:?}")),
            }
        }

        fn value(&mut self) -> Result<Val, String> {
            self.skip();
            match self.peek() {
                Some('{') => {
                    self.pos += 1;
                    let mut pairs = Vec::new();
                    loop {
                        self.skip();
                        match self.peek() {
                            Some('}') => {
                                self.pos += 1;
                                return Ok(Val::Obj(pairs));
                            }
                            None => return Err("unclosed '{'".into()),
                            _ => {
                                let key = self.key()?;
                                pairs.push((key, self.value()?));
                            }
                        }
                    }
                }
                Some('[') => {
                    self.pos += 1;
                    let mut items = Vec::new();
                    loop {
                        self.skip();
                        match self.peek() {
                            Some(']') => {
                                self.pos += 1;
                                return Ok(Val::Arr(items));
                            }
                            None => return Err("unclosed '['".into()),
                            _ => items.push(self.value()?),
                        }
                    }
                }
                Some('"') => Ok(Val::Str(self.string()?)),
                _ => Ok(Val::Bare(self.bare()?)),
            }
        }
    }

    /// Parse a whole config file (top level is an object without braces).
    pub fn parse(text: &str) -> Result<Val, String> {
        let mut parser = Parser {
            chars: text.chars().collect(),
            pos: 0,
        };
        let mut pairs = Vec::new();
        loop {
            parser.skip();
            if parser.peek().is_none() {
                return Ok(Val::Obj(pairs));
            }
            let key = parser.key()?;
            pairs.push((key, parser.value()?));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::spa_parser::{Val, parse};
    use super::*;
    use crate::mixer::config::NoiseSuppression;
    use crate::mixer::eq::{Eq, EqPreset};

    const HEADSET: &str = "alsa_output.usb-SteelSeries_Arctis_Nova_7-00.analog-stereo";
    const MIC_DEVICE: &str = "alsa_input.usb-SteelSeries_Arctis_Nova_7-00.mono-fallback";

    fn two_band_eq() -> Vec<EqBand> {
        vec![
            EqBand::new(BandType::LowShelf, 100.0, 0.707, 4.0),
            EqBand::new(BandType::Peaking, 3_000.0, 1.2, -2.5),
        ]
    }

    fn two_channel_plan() -> GraphPlan {
        GraphPlan {
            output_sink: HEADSET.to_string(),
            channels: vec![
                ChannelGraph {
                    channel: Channel::Game,
                    bands: two_band_eq(),
                    spatial_sofa: None,
                },
                ChannelGraph {
                    channel: Channel::Chat,
                    bands: Eq::from_preset(EqPreset::VoiceClarity).bands,
                    spatial_sofa: None,
                },
            ],
            mic: None,
            streamer: false,
        }
    }

    fn modules(doc: &Val) -> &[Val] {
        doc.get("context.modules").map(Val::items).unwrap_or(&[])
    }

    /// The module whose `capture.props` (or `source.props`) has `node.name == node`.
    fn module_by_capture<'a>(doc: &'a Val, node: &str) -> &'a Val {
        modules(doc)
            .iter()
            .find(|m| m.path(&["args", "capture.props", "node.name"]).and_then(Val::text) == Some(node))
            .unwrap_or_else(|| panic!("no module captures as {node}"))
    }

    fn text_at<'a>(v: &'a Val, keys: &[&str]) -> &'a str {
        v.path(keys)
            .and_then(Val::text)
            .unwrap_or_else(|| panic!("missing {keys:?}"))
    }

    fn assert_descriptions_quoted(doc: &Val) {
        let mut found = Vec::new();
        doc.find_all("node.description", &mut found);
        assert!(!found.is_empty());
        for value in found {
            assert!(matches!(value, Val::Str(_)), "unquoted description {value:?}");
        }
    }

    fn assert_balanced(text: &str) {
        let mut depth: Vec<char> = Vec::new();
        let mut in_string = false;
        let mut escaped = false;
        for c in text.chars() {
            if in_string {
                match (escaped, c) {
                    (true, _) => escaped = false,
                    (false, '\\') => escaped = true,
                    (false, '"') => in_string = false,
                    _ => {}
                }
                continue;
            }
            match c {
                '"' => in_string = true,
                '{' | '[' => depth.push(c),
                '}' => assert_eq!(depth.pop(), Some('{'), "unbalanced '}}'"),
                ']' => assert_eq!(depth.pop(), Some('['), "unbalanced ']'"),
                _ => {}
            }
        }
        assert!(!in_string, "unterminated string");
        assert!(depth.is_empty(), "unclosed {depth:?}");
    }

    fn generate_and_parse(plan: &GraphPlan) -> (String, Val) {
        let text = generate_pipewire_config(plan);
        assert_balanced(&text);
        let doc = parse(&text).unwrap_or_else(|e| panic!("{e}\n{text}"));
        assert_descriptions_quoted(&doc);
        (text, doc)
    }

    #[test]
    fn two_channel_eq_config() {
        let (text, doc) = generate_and_parse(&two_channel_plan());

        assert!(text.contains("context.modules = ["));
        assert!(text.contains("\"audio.convert.*\" = \"audioconvert/libspa-audioconvert\""));
        assert!(text.contains("name = \"libpipewire-module-filter-chain\""));
        assert!(text.contains("node.name = \"ssgg_game\""));
        assert!(text.contains("node.description = \"SteelSeries Game\""));
        assert!(text.contains("media.class = \"Audio/Sink\""));
        assert!(text.contains("{ output = \"eq_1:Out\" input = \"eq_2:In\" }"));
        assert!(text.contains("control = { Freq = 100.0 Q = 0.707 Gain = 4.0 }"));
        assert_eq!(text.matches("libpipewire-module-filter-chain").count(), 2);

        let names: Vec<&str> = modules(&doc).iter().filter_map(|m| m.get("name")?.text()).collect();
        assert_eq!(
            names[..4],
            [
                "libpipewire-module-rt",
                "libpipewire-module-protocol-native",
                "libpipewire-module-client-node",
                "libpipewire-module-adapter"
            ]
        );
        let rt_flags: Vec<&str> = modules(&doc)[0]
            .get("flags")
            .map(Val::items)
            .unwrap_or(&[])
            .iter()
            .filter_map(Val::text)
            .collect();
        assert_eq!(rt_flags, ["ifexists", "nofail"]);

        let game = module_by_capture(&doc, "ssgg_game");
        let graph = game.path(&["args", "filter.graph"]).unwrap();
        let nodes = graph.get("nodes").unwrap().items();
        assert_eq!(nodes.len(), 2);
        assert_eq!(text_at(&nodes[0], &["label"]), "bq_lowshelf");
        assert_eq!(text_at(&nodes[1], &["label"]), "bq_peaking");
        assert_eq!(text_at(&nodes[1], &["control", "Gain"]), "-2.5");
        assert_eq!(text_at(&nodes[1], &["control", "Freq"]), "3000.0");
        assert_eq!(graph.get("links").unwrap().items().len(), 1);
        assert_eq!(graph.get("inputs").unwrap().items()[0].text(), Some("eq_1:In"));
        assert_eq!(graph.get("outputs").unwrap().items()[0].text(), Some("eq_2:Out"));

        assert_eq!(text_at(game, &["args", "capture.props", "audio.channels"]), "2");
        let playback = game.path(&["args", "playback.props"]).unwrap();
        assert_eq!(text_at(playback, &["node.name"]), "ssgg_game_out");
        assert_eq!(text_at(playback, &["target.object"]), HEADSET);
        assert_eq!(text_at(playback, &["node.passive"]), "true");
        assert_eq!(text_at(playback, &["node.dont-reconnect"]), "true");
        assert_eq!(text_at(playback, &["node.dont-fallback"]), "true");

        let chat = module_by_capture(&doc, "ssgg_chat");
        let chat_nodes = chat.path(&["args", "filter.graph", "nodes"]).unwrap().items();
        assert_eq!(chat_nodes.len(), 10);
        assert_eq!(chat.path(&["args", "filter.graph", "links"]).unwrap().items().len(), 9);
        assert_eq!(
            text_at(chat, &["args", "capture.props", "node.description"]),
            "SteelSeries Chat"
        );
    }

    #[test]
    fn rnnoise_mic_chain_config() {
        let mut plan = two_channel_plan();
        plan.mic = Some(MicGraph {
            input_source: MIC_DEVICE.to_string(),
            noise_suppression: Some(NoiseSuppressionPlan::Rnnoise {
                plugin: "/usr/lib/ladspa/librnnoise_ladspa.so".into(),
                vad_threshold: 60,
            }),
            bands: two_band_eq(),
        });
        let (text, doc) = generate_and_parse(&plan);
        assert!(!text.contains("libpipewire-module-echo-cancel"));

        let mic = module_by_capture(&doc, "ssgg_mic_in");
        let graph = mic.path(&["args", "filter.graph"]).unwrap();
        let nodes = graph.get("nodes").unwrap().items();
        assert_eq!(nodes.len(), 3);
        assert_eq!(text_at(&nodes[0], &["type"]), "ladspa");
        assert_eq!(text_at(&nodes[0], &["plugin"]), "/usr/lib/ladspa/librnnoise_ladspa.so");
        assert_eq!(text_at(&nodes[0], &["label"]), "noise_suppressor_mono");
        assert_eq!(text_at(&nodes[0], &["control", "VAD Threshold (%)"]), "60.0");
        assert!(text.contains("\"VAD Threshold (%)\" = 60.0"));
        let links: Vec<(&str, &str)> = graph
            .get("links")
            .unwrap()
            .items()
            .iter()
            .map(|l| (text_at(l, &["output"]), text_at(l, &["input"])))
            .collect();
        assert_eq!(links, [("rnnoise:Output", "eq_1:In"), ("eq_1:Out", "eq_2:In")]);
        assert_eq!(graph.get("inputs").unwrap().items()[0].text(), Some("rnnoise:Input"));
        assert_eq!(graph.get("outputs").unwrap().items()[0].text(), Some("eq_2:Out"));

        let capture = mic.path(&["args", "capture.props"]).unwrap();
        assert_eq!(text_at(capture, &["target.object"]), MIC_DEVICE);
        assert_eq!(text_at(capture, &["node.passive"]), "true");
        assert_eq!(text_at(capture, &["audio.rate"]), "48000");
        assert_eq!(capture.get("audio.position").unwrap().items()[0].text(), Some("MONO"));
        let playback = mic.path(&["args", "playback.props"]).unwrap();
        assert_eq!(text_at(playback, &["node.name"]), "ssgg_mic");
        assert_eq!(text_at(playback, &["node.description"]), "SteelSeries Microphone");
        assert_eq!(text_at(playback, &["media.class"]), "Audio/Source");
        assert_eq!(text_at(playback, &["audio.rate"]), "48000");
    }

    #[test]
    fn webrtc_mic_chain_config() {
        let mut plan = two_channel_plan();
        plan.mic = Some(MicGraph {
            input_source: MIC_DEVICE.to_string(),
            noise_suppression: Some(NoiseSuppressionPlan::Webrtc),
            bands: Vec::new(),
        });
        let (_, doc) = generate_and_parse(&plan);

        let aec = module_by_capture(&doc, "ssgg_mic_ns_in");
        assert_eq!(text_at(aec, &["name"]), "libpipewire-module-echo-cancel");
        assert_eq!(text_at(aec, &["args", "library.name"]), "aec/libspa-aec-webrtc");
        assert_eq!(text_at(aec, &["args", "aec.args", "webrtc.noise_suppression"]), "true");
        assert_eq!(text_at(aec, &["args", "capture.props", "target.object"]), MIC_DEVICE);
        assert_eq!(text_at(aec, &["args", "source.props", "node.name"]), "ssgg_mic_ns");
        assert_eq!(text_at(aec, &["args", "playback.props", "target.object"]), HEADSET);

        let mic = module_by_capture(&doc, "ssgg_mic_in");
        assert_eq!(text_at(mic, &["args", "capture.props", "target.object"]), "ssgg_mic_ns");
        assert!(mic.path(&["args", "capture.props", "audio.rate"]).is_none());
        let nodes = mic.path(&["args", "filter.graph", "nodes"]).unwrap().items();
        assert_eq!(text_at(&nodes[0], &["label"]), "copy", "no EQ bands → passthrough");
    }

    #[test]
    fn streamer_mode_adds_stream_sink_and_taps() {
        let mut plan = two_channel_plan();
        plan.mic = Some(MicGraph {
            input_source: MIC_DEVICE.to_string(),
            noise_suppression: None,
            bands: Vec::new(),
        });
        plan.streamer = true;
        let (text, doc) = generate_and_parse(&plan);
        assert_eq!(
            text.matches("libpipewire-module-loopback").count(),
            4,
            "sink + game, chat, mic taps"
        );

        let stream = module_by_capture(&doc, "ssgg_stream");
        assert_eq!(text_at(stream, &["args", "capture.props", "media.class"]), "Audio/Sink");
        assert_eq!(
            text_at(stream, &["args", "capture.props", "node.description"]),
            "SteelSeries Stream"
        );
        assert_eq!(
            text_at(stream, &["args", "playback.props", "media.class"]),
            "Audio/Source"
        );

        let game_tap = module_by_capture(&doc, "ssgg_stream_game_in");
        assert_eq!(
            text_at(game_tap, &["args", "capture.props", "target.object"]),
            "ssgg_game"
        );
        assert_eq!(
            text_at(game_tap, &["args", "capture.props", "stream.capture.sink"]),
            "true"
        );
        assert_eq!(
            text_at(game_tap, &["args", "playback.props", "node.name"]),
            "ssgg_stream_game"
        );
        assert_eq!(
            text_at(game_tap, &["args", "playback.props", "target.object"]),
            "ssgg_stream"
        );

        let mic_tap = module_by_capture(&doc, "ssgg_stream_mic_in");
        assert_eq!(
            text_at(mic_tap, &["args", "capture.props", "target.object"]),
            "ssgg_mic"
        );
        assert!(
            mic_tap
                .path(&["args", "capture.props", "stream.capture.sink"])
                .is_none()
        );

        assert_eq!(plan.expected_sinks(), ["ssgg_game", "ssgg_chat", "ssgg_stream"]);
        assert_eq!(plan.expected_sources(), ["ssgg_mic", "ssgg_stream_out"]);
    }

    #[test]
    fn spatial_game_channel_config() {
        let mut plan = two_channel_plan();
        plan.channels[0].spatial_sofa = Some(PathBuf::from("/usr/share/sofa/hrtf b_nh724.sofa"));
        let (_, doc) = generate_and_parse(&plan);
        let game = module_by_capture(&doc, "ssgg_game");
        let graph = game.path(&["args", "filter.graph"]).unwrap();
        let nodes = graph.get("nodes").unwrap().items();
        let sofa: Vec<&Val> = nodes
            .iter()
            .filter(|n| n.get("type").and_then(Val::text) == Some("sofa"))
            .collect();
        assert_eq!(sofa.len(), 8);
        assert_eq!(
            text_at(sofa[0], &["config", "filename"]),
            "/usr/share/sofa/hrtf b_nh724.sofa"
        );
        assert_eq!(text_at(sofa[1], &["control", "Azimuth"]), "330.0");
        assert_eq!(nodes.len(), 8 + 2 + 2 * 2, "spatializers, two mixers, EQ per ear");
        assert_eq!(graph.get("inputs").unwrap().items().len(), 8);
        let outputs: Vec<&str> = graph
            .get("outputs")
            .unwrap()
            .items()
            .iter()
            .filter_map(Val::text)
            .collect();
        assert_eq!(outputs, ["eql_2:Out", "eqr_2:Out"]);
        assert_eq!(text_at(game, &["args", "capture.props", "audio.channels"]), "8");
        assert_eq!(text_at(game, &["args", "playback.props", "audio.channels"]), "2");
        assert!(plan.spatial_active());
    }

    #[test]
    fn device_names_are_escaped() {
        let mut plan = two_channel_plan();
        plan.output_sink = "weird \"sink\" \\ name".to_string();
        let (_, doc) = generate_and_parse(&plan);
        let game = module_by_capture(&doc, "ssgg_game");
        assert_eq!(
            text_at(game, &["args", "playback.props", "target.object"]),
            plan.output_sink
        );
    }

    #[test]
    fn topology_ignores_control_values_only() {
        let base = two_channel_plan();
        let mut gains = base.clone();
        gains.channels[0].bands[1].gain = 6.0;
        gains.channels[0].bands[1].freq = 2_000.0;
        assert_eq!(base.topology(), gains.topology());

        let mut kinds = base.clone();
        kinds.channels[0].bands[1].kind = BandType::Notch;
        assert_ne!(base.topology(), kinds.topology());

        let mut count = base.clone();
        count.channels[0].bands.pop();
        assert_ne!(base.topology(), count.topology());

        let mut output = base.clone();
        output.output_sink = "other".into();
        assert_ne!(base.topology(), output.topology());

        let mut streamer = base.clone();
        streamer.streamer = true;
        assert_ne!(base.topology(), streamer.topology());
    }

    #[test]
    fn eq_props_for_live_updates() {
        let plan = two_channel_plan();
        let (node, props) = plan.eq_props(Channel::Game, &two_band_eq()).unwrap();
        assert_eq!(node, "ssgg_game");
        assert_eq!(
            props,
            "{ params = [ \"eq_1:Freq\" 100.0 \"eq_1:Q\" 0.707 \"eq_1:Gain\" 4.0 \
             \"eq_2:Freq\" 3000.0 \"eq_2:Q\" 1.2 \"eq_2:Gain\" -2.5 ] }"
        );
        parse(&format!("x = {props}")).unwrap();

        let mut spatial = plan.clone();
        spatial.channels[0].spatial_sofa = Some(PathBuf::from("/x.sofa"));
        let (_, props) = spatial.eq_props(Channel::Game, &two_band_eq()).unwrap();
        assert!(props.contains("\"eql_2:Gain\" -2.5") && props.contains("\"eqr_1:Freq\" 100.0"));

        assert!(plan.eq_props(Channel::Mic, &two_band_eq()).is_none(), "no mic in plan");
        assert!(
            plan.eq_props(Channel::Aux, &two_band_eq()).is_none(),
            "channel not in plan"
        );
    }

    #[test]
    fn build_from_config_resolves_features() {
        let mut config = MixerConfig::default();
        config.mic.noise_suppression = NoiseSuppression {
            enabled: true,
            backend: NoiseSuppressionBackend::Auto,
            vad_threshold: 40,
        };
        config.spatial.enabled = true;
        config.spatial.sofa_path = Some(PathBuf::from("/missing.sofa"));

        let rnnoise = PathBuf::from("/usr/lib/ladspa/librnnoise_ladspa.so");
        let (plan, warnings) =
            GraphPlan::build(&config, HEADSET.into(), Some(MIC_DEVICE.into()), Some(&rnnoise), false);
        assert_eq!(plan.channels.len(), 4);
        assert!(!plan.spatial_active());
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("SOFA"));
        let mic = plan.mic.as_ref().unwrap();
        assert_eq!(
            mic.noise_suppression,
            Some(NoiseSuppressionPlan::Rnnoise {
                plugin: rnnoise.to_string_lossy().into_owned(),
                vad_threshold: 40
            })
        );
        assert!(plan.has_optional_features());
        assert!(!plan.without_optional_features().has_optional_features());

        let (plan, _) = GraphPlan::build(&config, HEADSET.into(), Some(MIC_DEVICE.into()), None, true);
        assert_eq!(
            plan.mic.as_ref().unwrap().noise_suppression,
            Some(NoiseSuppressionPlan::Webrtc)
        );
        assert!(plan.spatial_active());

        config.mic.noise_suppression.backend = NoiseSuppressionBackend::Rnnoise;
        let (plan, warnings) = GraphPlan::build(&config, HEADSET.into(), Some(MIC_DEVICE.into()), None, true);
        assert!(matches!(
            &plan.mic.as_ref().unwrap().noise_suppression,
            Some(NoiseSuppressionPlan::Rnnoise { plugin, .. }) if plugin == "librnnoise_ladspa"
        ));
        assert_eq!(warnings.len(), 1);

        let (plan, warnings) = GraphPlan::build(&config, HEADSET.into(), None, None, true);
        assert!(plan.mic.is_none());
        assert!(warnings.iter().any(|w| w.contains("no microphone")));
    }

    #[test]
    fn full_default_config_is_valid_spa_json() {
        let mut config = MixerConfig::default();
        config.streamer.enabled = true;
        config.mic.noise_suppression.enabled = true;
        let (plan, _) = GraphPlan::build(&config, HEADSET.into(), Some(MIC_DEVICE.into()), None, false);
        let (text, doc) = generate_and_parse(&plan);
        for name in [
            "ssgg_game",
            "ssgg_chat",
            "ssgg_media",
            "ssgg_aux",
            "ssgg_mic_in",
            "ssgg_stream",
        ] {
            module_by_capture(&doc, name);
        }
        assert_eq!(text.matches("libpipewire-module-filter-chain").count(), 5);
        assert_eq!(text.matches("libpipewire-module-echo-cancel").count(), 1);
        assert_eq!(text.matches("libpipewire-module-loopback").count(), 6);
    }
}
