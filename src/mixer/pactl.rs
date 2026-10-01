//! `pactl` client (talks to `pipewire-pulse`).
//!
//! Prefers `pactl -f json` (pactl 16+). Falls back to the text format when JSON is unsupported
//! or does not parse: some pactl 16 builds emit invalid JSON for the `format` field of
//! sink-inputs. Text parsing relies on the runner setting `LC_ALL=C`.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::Value;

use super::nodes;
use super::routing::AppIdentity;
use super::runner::CommandRunner;
use crate::{Error, Result};

pub(crate) const PACTL: &str = "pactl";

/// A sink or source.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PaNode {
    pub index: u32,
    pub name: String,
    pub description: String,
    pub muted: bool,
    /// Average of the per-channel volumes, percent.
    pub volume_percent: Option<u32>,
    pub properties: BTreeMap<String, String>,
}

impl PaNode {
    /// A source that only mirrors a sink (`<sink>.monitor`).
    pub fn is_monitor(&self) -> bool {
        self.name.ends_with(".monitor") || self.properties.get("device.class").is_some_and(|c| c == "monitor")
    }

    pub fn is_ours(&self) -> bool {
        nodes::is_ours(&self.name)
    }

    /// Looks like SteelSeries hardware (name, description or USB vendor id 0x1038).
    pub fn is_steelseries(&self) -> bool {
        let text = format!("{} {}", self.name, self.description).to_lowercase();
        text.contains("steelseries")
            || text.contains("arctis")
            || self
                .properties
                .get("device.vendor.id")
                .is_some_and(|id| id.eq_ignore_ascii_case("0x1038"))
    }
}

/// An application playback stream.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PaSinkInput {
    pub index: u32,
    /// Index of the sink it plays to.
    pub sink: Option<u32>,
    pub muted: bool,
    pub volume_percent: Option<u32>,
    pub properties: BTreeMap<String, String>,
}

impl PaSinkInput {
    pub fn prop(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(String::as_str)
    }

    pub fn identity(&self) -> AppIdentity {
        AppIdentity {
            application_name: self.prop("application.name").map(str::to_string),
            process_binary: self.prop("application.process.binary").map(str::to_string),
        }
    }

    /// Streams that belong to a filter, loopback or this mixer, which routing must never move.
    pub fn is_internal(&self) -> bool {
        self.prop("node.name").is_some_and(nodes::is_ours)
            || self.properties.contains_key("node.link-group")
            || self.prop("node.dont-move").is_some_and(|v| v == "true" || v == "1")
    }
}

/// The parts of `pactl info` the mixer uses.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServerInfo {
    pub server_name: Option<String>,
    pub default_sink: Option<String>,
    pub default_source: Option<String>,
}

impl ServerInfo {
    pub fn is_pipewire(&self) -> bool {
        self.server_name.as_deref().is_some_and(|n| n.contains("PipeWire"))
    }
}

/// `pactl list <kind>` targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListKind {
    Sinks,
    Sources,
    SinkInputs,
}

impl ListKind {
    fn arg(self) -> &'static str {
        match self {
            ListKind::Sinks => "sinks",
            ListKind::Sources => "sources",
            ListKind::SinkInputs => "sink-inputs",
        }
    }
}

/// Argument lists for every pactl call the mixer makes.
pub mod args {
    use super::ListKind;

    fn owned(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    fn with_format(json: bool, rest: &[&str]) -> Vec<String> {
        let mut out = if json { owned(&["-f", "json"]) } else { Vec::new() };
        out.extend(owned(rest));
        out
    }

    fn flag(on: bool) -> &'static str {
        if on { "1" } else { "0" }
    }

    pub fn info(json: bool) -> Vec<String> {
        with_format(json, &["info"])
    }

    pub fn list(kind: ListKind, json: bool) -> Vec<String> {
        with_format(json, &["list", kind.arg()])
    }

    pub fn set_sink_volume(sink: &str, percent: u8) -> Vec<String> {
        owned(&["set-sink-volume", sink, &format!("{percent}%")])
    }

    pub fn set_sink_mute(sink: &str, muted: bool) -> Vec<String> {
        owned(&["set-sink-mute", sink, flag(muted)])
    }

    pub fn set_source_volume(source: &str, percent: u8) -> Vec<String> {
        owned(&["set-source-volume", source, &format!("{percent}%")])
    }

    pub fn set_source_mute(source: &str, muted: bool) -> Vec<String> {
        owned(&["set-source-mute", source, flag(muted)])
    }

    pub fn set_sink_input_volume(index: u32, percent: u8) -> Vec<String> {
        owned(&["set-sink-input-volume", &index.to_string(), &format!("{percent}%")])
    }

    pub fn set_sink_input_mute(index: u32, muted: bool) -> Vec<String> {
        owned(&["set-sink-input-mute", &index.to_string(), flag(muted)])
    }

    pub fn move_sink_input(index: u32, sink: &str) -> Vec<String> {
        owned(&["move-sink-input", &index.to_string(), sink])
    }

    pub fn set_default_sink(sink: &str) -> Vec<String> {
        owned(&["set-default-sink", sink])
    }

    pub fn set_default_source(source: &str) -> Vec<String> {
        owned(&["set-default-source", source])
    }

    pub fn subscribe() -> Vec<String> {
        owned(&["subscribe"])
    }
}

/// Thin pactl wrapper bound to a runner and an output format.
#[derive(Clone)]
pub struct Pactl {
    runner: Arc<dyn CommandRunner>,
    json: bool,
}

impl Pactl {
    pub fn new(runner: Arc<dyn CommandRunner>, json: bool) -> Self {
        Self { runner, json }
    }

    pub fn runner(&self) -> &Arc<dyn CommandRunner> {
        &self.runner
    }

    fn exec(&self, args: Vec<String>) -> Result<String> {
        let out = self.runner.run(PACTL, &args)?;
        if out.success() {
            Ok(out.stdout)
        } else {
            Err(Error::Audio(format!(
                "pactl {} failed: {}",
                args.join(" "),
                first_line(&out.stderr).unwrap_or("no error output")
            )))
        }
    }

    pub fn info(&self) -> Result<ServerInfo> {
        if self.json {
            if let Some(info) = parse_info_json(&self.exec(args::info(true))?) {
                return Ok(info);
            }
        }
        Ok(parse_info_text(&self.exec(args::info(false))?))
    }

    fn list_nodes(&self, kind: ListKind) -> Result<Vec<PaNode>> {
        if self.json {
            if let Some(nodes) = parse_nodes_json(&self.exec(args::list(kind, true))?) {
                return Ok(nodes);
            }
            tracing::debug!("pactl JSON for {} did not parse; using text output", kind.arg());
        }
        Ok(parse_nodes_text(&self.exec(args::list(kind, false))?))
    }

    pub fn sinks(&self) -> Result<Vec<PaNode>> {
        self.list_nodes(ListKind::Sinks)
    }

    pub fn sources(&self) -> Result<Vec<PaNode>> {
        self.list_nodes(ListKind::Sources)
    }

    pub fn sink_inputs(&self) -> Result<Vec<PaSinkInput>> {
        if self.json {
            if let Some(inputs) = parse_sink_inputs_json(&self.exec(args::list(ListKind::SinkInputs, true))?) {
                return Ok(inputs);
            }
            tracing::debug!("pactl JSON for sink-inputs did not parse; using text output");
        }
        Ok(parse_sink_inputs_text(
            &self.exec(args::list(ListKind::SinkInputs, false))?,
        ))
    }

    pub fn set_sink_volume(&self, sink: &str, percent: u8) -> Result<()> {
        self.exec(args::set_sink_volume(sink, percent)).map(drop)
    }

    pub fn set_sink_mute(&self, sink: &str, muted: bool) -> Result<()> {
        self.exec(args::set_sink_mute(sink, muted)).map(drop)
    }

    pub fn set_source_volume(&self, source: &str, percent: u8) -> Result<()> {
        self.exec(args::set_source_volume(source, percent)).map(drop)
    }

    pub fn set_source_mute(&self, source: &str, muted: bool) -> Result<()> {
        self.exec(args::set_source_mute(source, muted)).map(drop)
    }

    pub fn set_sink_input_volume(&self, index: u32, percent: u8) -> Result<()> {
        self.exec(args::set_sink_input_volume(index, percent)).map(drop)
    }

    pub fn set_sink_input_mute(&self, index: u32, muted: bool) -> Result<()> {
        self.exec(args::set_sink_input_mute(index, muted)).map(drop)
    }

    pub fn move_sink_input(&self, index: u32, sink: &str) -> Result<()> {
        self.exec(args::move_sink_input(index, sink)).map(drop)
    }

    pub fn set_default_sink(&self, sink: &str) -> Result<()> {
        self.exec(args::set_default_sink(sink)).map(drop)
    }

    pub fn set_default_source(&self, source: &str) -> Result<()> {
        self.exec(args::set_default_source(source)).map(drop)
    }
}

fn first_line(text: &str) -> Option<&str> {
    text.lines().map(str::trim).find(|l| !l.is_empty())
}

// ---------------------------------------------------------------------------------------------
// JSON
// ---------------------------------------------------------------------------------------------

fn json_u32(value: &Value) -> Option<u32> {
    match value {
        Value::Number(n) => n.as_u64().and_then(|n| u32::try_from(n).ok()),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn json_string(value: Option<&Value>) -> String {
    value.and_then(Value::as_str).unwrap_or_default().to_string()
}

fn json_bool(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => s == "yes" || s == "true",
        _ => false,
    }
}

/// Average of `{"front-left": {"value_percent": "50%"}, ...}`.
fn json_volume(value: Option<&Value>) -> Option<u32> {
    let percents: Vec<u32> = value?
        .as_object()?
        .values()
        .filter_map(|ch| ch.get("value_percent")?.as_str())
        .filter_map(|p| p.trim().trim_end_matches('%').trim().parse().ok())
        .collect();
    average(&percents)
}

fn json_properties(value: Option<&Value>) -> BTreeMap<String, String> {
    value
        .and_then(Value::as_object)
        .map(|props| {
            props
                .iter()
                .map(|(k, v)| {
                    let v = match v {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    (k.clone(), v)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn json_array(text: &str) -> Option<Vec<Value>> {
    match serde_json::from_str::<Value>(text).ok()? {
        Value::Array(items) => Some(items),
        _ => None,
    }
}

pub(crate) fn parse_nodes_json(text: &str) -> Option<Vec<PaNode>> {
    json_array(text)?
        .iter()
        .map(|item| {
            Some(PaNode {
                index: json_u32(item.get("index")?)?,
                name: json_string(item.get("name")),
                description: json_string(item.get("description")),
                muted: json_bool(item.get("mute")),
                volume_percent: json_volume(item.get("volume")),
                properties: json_properties(item.get("properties")),
            })
        })
        .collect()
}

pub(crate) fn parse_sink_inputs_json(text: &str) -> Option<Vec<PaSinkInput>> {
    json_array(text)?
        .iter()
        .map(|item| {
            Some(PaSinkInput {
                index: json_u32(item.get("index")?)?,
                sink: item.get("sink").and_then(json_u32),
                muted: json_bool(item.get("mute")),
                volume_percent: json_volume(item.get("volume")),
                properties: json_properties(item.get("properties")),
            })
        })
        .collect()
}

pub(crate) fn parse_info_json(text: &str) -> Option<ServerInfo> {
    let value: Value = serde_json::from_str(text).ok()?;
    let field = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_string);
    Some(ServerInfo {
        server_name: field("server_name"),
        default_sink: field("default_sink_name"),
        default_source: field("default_source_name"),
    })
}

// ---------------------------------------------------------------------------------------------
// Text
// ---------------------------------------------------------------------------------------------

/// One `Sink #12` / `Sink Input #34` block of `pactl list` text output.
#[derive(Debug, Default)]
struct TextBlock {
    index: u32,
    fields: BTreeMap<String, String>,
    properties: BTreeMap<String, String>,
}

fn parse_blocks(text: &str) -> Vec<TextBlock> {
    let mut blocks = Vec::new();
    let mut current: Option<TextBlock> = None;
    let mut in_properties = false;

    for line in text.lines() {
        let depth = line.chars().take_while(|&c| c == '\t').count();
        let content = line.trim();
        if content.is_empty() {
            continue;
        }
        if depth == 0 && !line.starts_with(' ') {
            if let Some(block) = current.take() {
                blocks.push(block);
            }
            in_properties = false;
            current = content
                .rsplit_once('#')
                .and_then(|(_, index)| index.trim().parse().ok())
                .map(|index| TextBlock {
                    index,
                    ..TextBlock::default()
                });
            continue;
        }
        let Some(block) = current.as_mut() else { continue };
        if depth == 1 {
            in_properties = false;
            if let Some((key, value)) = content.split_once(':') {
                let (key, value) = (key.trim(), value.trim());
                if key == "Properties" && value.is_empty() {
                    in_properties = true;
                } else {
                    block.fields.entry(key.to_string()).or_insert_with(|| value.to_string());
                }
            }
        } else if depth >= 2 && in_properties {
            if let Some((key, value)) = content.split_once(" = ") {
                block.properties.insert(key.trim().to_string(), unquote(value.trim()));
            }
        }
    }
    if let Some(block) = current {
        blocks.push(block);
    }
    blocks
}

/// `"a \"b\""` → `a "b"`.
fn unquote(value: &str) -> String {
    let inner = value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .unwrap_or(value);
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Average of every `NN%` on a `Volume:` line.
fn text_volume(line: Option<&String>) -> Option<u32> {
    let percents: Vec<u32> = line?
        .split_whitespace()
        .filter_map(|token| token.strip_suffix('%'))
        .filter_map(|n| n.parse().ok())
        .collect();
    average(&percents)
}

fn average(values: &[u32]) -> Option<u32> {
    if values.is_empty() {
        return None;
    }
    let sum: u64 = values.iter().map(|&v| u64::from(v)).sum();
    let count = values.len() as u64;
    u32::try_from((sum + count / 2) / count).ok()
}

pub(crate) fn parse_nodes_text(text: &str) -> Vec<PaNode> {
    parse_blocks(text)
        .into_iter()
        .map(|block| PaNode {
            index: block.index,
            name: block.fields.get("Name").cloned().unwrap_or_default(),
            description: block.fields.get("Description").cloned().unwrap_or_default(),
            muted: block.fields.get("Mute").is_some_and(|m| m == "yes"),
            volume_percent: text_volume(block.fields.get("Volume")),
            properties: block.properties,
        })
        .collect()
}

pub(crate) fn parse_sink_inputs_text(text: &str) -> Vec<PaSinkInput> {
    parse_blocks(text)
        .into_iter()
        .map(|block| PaSinkInput {
            index: block.index,
            sink: block.fields.get("Sink").and_then(|s| s.parse().ok()),
            muted: block.fields.get("Mute").is_some_and(|m| m == "yes"),
            volume_percent: text_volume(block.fields.get("Volume")),
            properties: block.properties,
        })
        .collect()
}

pub(crate) fn parse_info_text(text: &str) -> ServerInfo {
    let mut info = ServerInfo::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = Some(value.trim().to_string()).filter(|v| !v.is_empty() && v != "n/a");
        match key.trim() {
            "Server Name" => info.server_name = value,
            "Default Sink" => info.default_sink = value,
            "Default Source" => info.default_source = value,
            _ => {}
        }
    }
    info
}

/// `Event 'new' on sink-input #42` → `Some(42)`.
pub(crate) fn parse_new_sink_input_event(line: &str) -> Option<u32> {
    let rest = line.trim().strip_prefix("Event 'new' on sink-input #")?;
    rest.trim().parse().ok()
}

#[cfg(test)]
pub(crate) mod samples {
    pub const SINKS_JSON: &str = r#"[
  {"index":56,"state":"SUSPENDED","name":"alsa_output.usb-SteelSeries_Arctis_Nova_7-00.analog-stereo",
   "description":"Arctis Nova 7 Analog Stereo","driver":"PipeWire","sample_specification":"s16le 2ch 48000Hz",
   "channel_map":"front-left,front-right","owner_module":4294967295,"mute":false,
   "volume":{"front-left":{"value":32768,"value_percent":"50%","db":"-18.06 dB"},
             "front-right":{"value":32768,"value_percent":"50%","db":"-18.06 dB"}},
   "balance":0.0,"base_volume":{"value":65536,"value_percent":"100%","db":"0.00 dB"},
   "monitor_source":"alsa_output.usb-SteelSeries_Arctis_Nova_7-00.analog-stereo.monitor",
   "latency":{"actual":0.0,"configured":0.0},"flags":["HARDWARE","HW_MUTE_CTRL"],
   "properties":{"alsa.card":"2","device.vendor.id":"0x1038","node.name":"alsa_output.usb-SteelSeries_Arctis_Nova_7-00.analog-stereo",
                 "object.serial":"56","media.class":"Audio/Sink"},
   "ports":[],"active_port":null,"formats":["pcm"]},
  {"index":48,"state":"RUNNING","name":"alsa_output.pci-0000_00_1f.3.analog-stereo",
   "description":"Built-in Audio Analog Stereo","mute":true,
   "volume":{"front-left":{"value":65536,"value_percent":"100%","db":"0.00 dB"},
             "front-right":{"value":52429,"value_percent":"80%","db":"-5.81 dB"}},
   "properties":{"media.class":"Audio/Sink","device.vendor.id":"0x8086"}}
]"#;

    pub const SINKS_TEXT: &str = "Sink #56
\tState: SUSPENDED
\tName: alsa_output.usb-SteelSeries_Arctis_Nova_7-00.analog-stereo
\tDescription: Arctis Nova 7 Analog Stereo
\tDriver: PipeWire
\tSample Specification: s16le 2ch 48000Hz
\tChannel Map: front-left,front-right
\tOwner Module: 4294967295
\tMute: no
\tVolume: front-left: 32768 /  50% / -18.06 dB,   front-right: 32768 /  50% / -18.06 dB
\t        balance 0.00
\tBase Volume: 65536 / 100% / 0.00 dB
\tMonitor Source: alsa_output.usb-SteelSeries_Arctis_Nova_7-00.analog-stereo.monitor
\tLatency: 0 usec, configured 0 usec
\tFlags: HARDWARE HW_MUTE_CTRL HW_VOLUME_CTRL DECIBEL_VOLUME LATENCY
\tProperties:
\t\talsa.card = \"2\"
\t\tdevice.description = \"Arctis Nova 7 Analog Stereo\"
\t\tdevice.vendor.id = \"0x1038\"
\t\tnode.name = \"alsa_output.usb-SteelSeries_Arctis_Nova_7-00.analog-stereo\"
\t\tmedia.class = \"Audio/Sink\"
\tPorts:
\t\tanalog-output: Analog Output (type: Headphones, priority: 9900, availability unknown)
\tActive Port: analog-output
\tFormats:
\t\tpcm

Sink #48
\tState: RUNNING
\tName: alsa_output.pci-0000_00_1f.3.analog-stereo
\tDescription: Built-in Audio Analog Stereo
\tMute: yes
\tVolume: front-left: 65536 / 100% / 0.00 dB,   front-right: 52429 /  80% / -5.81 dB
\t        balance -0.20
\tProperties:
\t\tmedia.class = \"Audio/Sink\"
";

    pub const SINK_INPUTS_JSON: &str = r#"[
  {"index":123,"driver":"PipeWire","owner_module":"4294967295","client":"77","sink":56,
   "sample_specification":"float32le 2ch 48000Hz","channel_map":"front-left,front-right",
   "format":"pcm, format.sample_format = \"\\\"float32le\\\"\"","corked":false,"mute":false,
   "volume":{"front-left":{"value":65536,"value_percent":"100%","db":"0.00 dB"},
             "front-right":{"value":65536,"value_percent":"100%","db":"0.00 dB"}},
   "balance":0.0,"buffer_latency":0.0,"sink_latency":0.0,"resample_method":"PipeWire",
   "properties":{"application.name":"Firefox","application.process.binary":"firefox",
                 "media.name":"AudioStream","node.name":"Firefox","object.serial":"123"}},
  {"index":130,"sink":"56","mute":true,"volume":{"mono":{"value":32768,"value_percent":"50%"}},
   "properties":{"application.name":"WEBRTC VoiceEngine","application.process.binary":"Discord",
                 "media.name":"playStream"}},
  {"index":140,"sink":56,"mute":false,"volume":{},
   "properties":{"node.name":"ssgg_game_out","node.link-group":"filter-chain-1234-27"}}
]"#;

    /// Escaped quotes inside a string value broke pactl 16's JSON; this is what it emitted.
    pub const SINK_INPUTS_BROKEN_JSON: &str =
        r#"[{"index":123,"sink":56,"format":"pcm, format.sample_format = "\"float32le\""","mute":false}]"#;

    pub const SINK_INPUTS_TEXT: &str = "Sink Input #123
\tDriver: PipeWire
\tOwner Module: n/a
\tClient: 77
\tSink: 56
\tSample Specification: float32le 2ch 48000Hz
\tChannel Map: front-left,front-right
\tFormat: pcm, format.sample_format = \"\\\"float32le\\\"\"  format.rate = \"48000\"
\tCorked: no
\tMute: no
\tVolume: front-left: 65536 / 100% / 0.00 dB,   front-right: 65536 / 100% / 0.00 dB
\t        balance 0.00
\tBuffer Latency: 0 usec
\tSink Latency: 0 usec
\tResample method: PipeWire
\tProperties:
\t\tmedia.name = \"AudioStream\"
\t\tapplication.name = \"Firefox\"
\t\tapplication.process.binary = \"firefox\"
\t\tnode.name = \"Firefox\"
\t\tmedia.title = \"Say \\\"hi\\\"\"

Sink Input #130
\tSink: 56
\tMute: yes
\tVolume: mono: 32768 /  50% / -18.06 dB
\tProperties:
\t\tapplication.name = \"WEBRTC VoiceEngine\"
\t\tapplication.process.binary = \"Discord\"
";

    pub const INFO_TEXT: &str = "Server String: /run/user/1000/pulse/native
Library Protocol Version: 35
Server Protocol Version: 35
Is Local: yes
Client Index: 98
Tile Size: 65472
User Name: mahdi
Host Name: box
Server Name: PulseAudio (on PipeWire 1.0.5)
Server Version: 15.0.0
Default Sample Specification: float32le 2ch 48000Hz
Default Channel Map: front-left,front-right
Default Sink: alsa_output.usb-SteelSeries_Arctis_Nova_7-00.analog-stereo
Default Source: alsa_input.usb-SteelSeries_Arctis_Nova_7-00.mono-fallback
Cookie: 3f2a:91bc
";

    pub const INFO_JSON: &str = r#"{"server_string":"/run/user/1000/pulse/native","server_name":"PulseAudio (on PipeWire 1.2.7)",
"server_version":"15.0.0","default_sink_name":"alsa_output.pci-0000_00_1f.3.analog-stereo",
"default_source_name":"alsa_input.pci-0000_00_1f.3.analog-stereo"}"#;
}

#[cfg(test)]
mod tests {
    use super::samples::*;
    use super::*;
    use crate::mixer::runner::{CommandOutput, FakeRunner};

    #[test]
    fn argument_construction() {
        assert_eq!(
            args::list(ListKind::SinkInputs, true),
            ["-f", "json", "list", "sink-inputs"]
        );
        assert_eq!(args::list(ListKind::Sources, false), ["list", "sources"]);
        assert_eq!(args::info(true), ["-f", "json", "info"]);
        assert_eq!(
            args::set_sink_volume("ssgg_game", 75),
            ["set-sink-volume", "ssgg_game", "75%"]
        );
        assert_eq!(
            args::set_sink_mute("ssgg_chat", true),
            ["set-sink-mute", "ssgg_chat", "1"]
        );
        assert_eq!(
            args::set_source_volume("ssgg_mic", 0),
            ["set-source-volume", "ssgg_mic", "0%"]
        );
        assert_eq!(
            args::set_source_mute("ssgg_mic", false),
            ["set-source-mute", "ssgg_mic", "0"]
        );
        assert_eq!(
            args::set_sink_input_volume(77, 30),
            ["set-sink-input-volume", "77", "30%"]
        );
        assert_eq!(args::set_sink_input_mute(77, true), ["set-sink-input-mute", "77", "1"]);
        assert_eq!(
            args::move_sink_input(123, "ssgg_media"),
            ["move-sink-input", "123", "ssgg_media"]
        );
        assert_eq!(args::set_default_sink("ssgg_game"), ["set-default-sink", "ssgg_game"]);
        assert_eq!(args::set_default_source("ssgg_mic"), ["set-default-source", "ssgg_mic"]);
        assert_eq!(args::subscribe(), ["subscribe"]);
    }

    #[test]
    fn parses_sinks_json() {
        let sinks = parse_nodes_json(SINKS_JSON).unwrap();
        assert_eq!(sinks.len(), 2);
        assert_eq!(sinks[0].index, 56);
        assert_eq!(
            sinks[0].name,
            "alsa_output.usb-SteelSeries_Arctis_Nova_7-00.analog-stereo"
        );
        assert_eq!(sinks[0].description, "Arctis Nova 7 Analog Stereo");
        assert!(!sinks[0].muted);
        assert_eq!(sinks[0].volume_percent, Some(50));
        assert!(sinks[0].is_steelseries());
        assert_eq!(sinks[0].properties["object.serial"], "56");
        assert!(sinks[1].muted);
        assert_eq!(sinks[1].volume_percent, Some(90));
        assert!(!sinks[1].is_steelseries());
    }

    #[test]
    fn parses_sinks_text_identically() {
        let json = parse_nodes_json(SINKS_JSON).unwrap();
        let text = parse_nodes_text(SINKS_TEXT);
        assert_eq!(text.len(), 2);
        for (t, j) in text.iter().zip(&json) {
            assert_eq!(t.index, j.index);
            assert_eq!(t.name, j.name);
            assert_eq!(t.description, j.description);
            assert_eq!(t.muted, j.muted);
            assert_eq!(t.volume_percent, j.volume_percent);
        }
        assert_eq!(text[0].properties["device.vendor.id"], "0x1038");
        assert_eq!(text[0].properties["media.class"], "Audio/Sink");
        assert!(
            !text[0].properties.contains_key("analog-output"),
            "port lines are not properties"
        );
    }

    #[test]
    fn parses_sink_inputs_json() {
        let inputs = parse_sink_inputs_json(SINK_INPUTS_JSON).unwrap();
        assert_eq!(inputs.len(), 3);
        assert_eq!(inputs[0].index, 123);
        assert_eq!(inputs[0].sink, Some(56));
        assert_eq!(inputs[0].identity().application_name.as_deref(), Some("Firefox"));
        assert_eq!(inputs[0].identity().process_binary.as_deref(), Some("firefox"));
        assert!(!inputs[0].is_internal());
        assert_eq!(inputs[1].sink, Some(56), "string sink index accepted");
        assert!(inputs[1].muted);
        assert_eq!(inputs[1].volume_percent, Some(50));
        assert!(inputs[2].is_internal());
        assert_eq!(inputs[2].volume_percent, None);
    }

    #[test]
    fn parses_sink_inputs_text() {
        let inputs = parse_sink_inputs_text(SINK_INPUTS_TEXT);
        assert_eq!(inputs.len(), 2);
        assert_eq!(inputs[0].index, 123);
        assert_eq!(inputs[0].sink, Some(56));
        assert!(!inputs[0].muted);
        assert_eq!(inputs[0].volume_percent, Some(100));
        assert_eq!(inputs[0].prop("application.process.binary"), Some("firefox"));
        assert_eq!(inputs[0].prop("media.title"), Some("Say \"hi\""));
        assert_eq!(inputs[1].index, 130);
        assert!(inputs[1].muted);
        assert_eq!(inputs[1].prop("application.name"), Some("WEBRTC VoiceEngine"));
    }

    #[test]
    fn broken_json_is_rejected_so_text_is_used() {
        assert!(parse_sink_inputs_json(SINK_INPUTS_BROKEN_JSON).is_none());

        let fake = FakeRunner::new();
        fake.on(
            "pactl",
            &["-f", "json", "list", "sink-inputs"],
            CommandOutput::ok(SINK_INPUTS_BROKEN_JSON),
        );
        fake.on("pactl", &["list", "sink-inputs"], CommandOutput::ok(SINK_INPUTS_TEXT));
        let pactl = Pactl::new(Arc::new(fake.clone()), true);
        let inputs = pactl.sink_inputs().unwrap();
        assert_eq!(inputs.len(), 2);
        assert_eq!(
            fake.command_lines("pactl"),
            ["pactl -f json list sink-inputs", "pactl list sink-inputs"]
        );
    }

    #[test]
    fn parses_info() {
        let info = parse_info_text(INFO_TEXT);
        assert!(info.is_pipewire());
        assert_eq!(
            info.default_sink.as_deref(),
            Some("alsa_output.usb-SteelSeries_Arctis_Nova_7-00.analog-stereo")
        );
        assert_eq!(
            info.default_source.as_deref(),
            Some("alsa_input.usb-SteelSeries_Arctis_Nova_7-00.mono-fallback")
        );
        let info = parse_info_json(INFO_JSON).unwrap();
        assert!(info.is_pipewire());
        assert_eq!(
            info.default_sink.as_deref(),
            Some("alsa_output.pci-0000_00_1f.3.analog-stereo")
        );
        let pulse = parse_info_text("Server Name: pulseaudio\nDefault Sink: x\n");
        assert!(!pulse.is_pipewire());
    }

    #[test]
    fn failed_commands_become_errors() {
        let fake = FakeRunner::new();
        fake.on_prefix(
            "pactl",
            &["set-sink-volume"],
            CommandOutput::failed(1, "Failure: No such entity\n"),
        );
        let pactl = Pactl::new(Arc::new(fake), false);
        let err = pactl.set_sink_volume("ssgg_game", 10).unwrap_err().to_string();
        assert!(err.contains("No such entity"), "{err}");
    }

    #[test]
    fn subscribe_event_parsing() {
        assert_eq!(parse_new_sink_input_event("Event 'new' on sink-input #42"), Some(42));
        assert_eq!(parse_new_sink_input_event("Event 'change' on sink-input #42"), None);
        assert_eq!(parse_new_sink_input_event("Event 'new' on source-output #3"), None);
    }

    #[test]
    fn monitor_detection() {
        let node = PaNode {
            name: "alsa_output.x.monitor".into(),
            ..PaNode::default()
        };
        assert!(node.is_monitor());
        let mut node = PaNode {
            name: "virtual".into(),
            ..PaNode::default()
        };
        node.properties.insert("device.class".into(), "monitor".into());
        assert!(node.is_monitor());
    }
}
