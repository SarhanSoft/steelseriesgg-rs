//! Parsed form of the JSON handlers sent to `bind_game_event`.
//!
//! Parsing works on `serde_json::Value` instead of derived types because the SDK schema is
//! loose: a handler may be an object or a stringified object, colors and patterns may be
//! values or nested range lists, and unknown device types must be skipped, not rejected.

use std::fmt;

use serde_json::{Map, Value};

use super::output::{DeviceType, LightTarget, ScreenSize, TactileStep};
use super::zone_map::{ZoneSpec, resolve_zone};
use crate::rgb::Color;

/// Deepest nesting accepted for range definitions.
const MAX_RANGE_DEPTH: usize = 8;
/// Most entries accepted in one range list, frame list or pattern.
const MAX_LIST_LEN: usize = 256;
/// The SDK caps a tactile pattern at 140 entries (Rival 700).
const MAX_TACTILE_STEPS: usize = 140;
/// The SDK caps `length-ms` and `delay-ms` of tactile steps at 2560.
const MAX_TACTILE_MS: u64 = 2560;

/// Why a handler was not accepted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HandlerError {
    /// The `device-type` is not one this server knows. The handler is ignored.
    UnknownDeviceType(String),
    /// The handler is malformed.
    Invalid(String),
}

impl fmt::Display for HandlerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownDeviceType(t) => write!(f, "unknown device-type '{t}' ignored"),
            Self::Invalid(msg) => write!(f, "invalid handler: {msg}"),
        }
    }
}

impl std::error::Error for HandlerError {}

fn invalid(msg: impl Into<String>) -> HandlerError {
    HandlerError::Invalid(msg.into())
}

/// A value that is either fixed or chosen by the event value from inclusive ranges.
#[derive(Clone, Debug, PartialEq)]
pub enum Ranged<T> {
    Static(T),
    Ranges(Vec<RangeEntry<T>>),
}

/// One `{"low": .., "high": .., <key>: ..}` entry. Bounds are inclusive.
#[derive(Clone, Debug, PartialEq)]
pub struct RangeEntry<T> {
    pub low: i64,
    pub high: i64,
    pub value: Ranged<T>,
}

impl<T> Ranged<T> {
    /// The value for an event value; `None` when no range contains it.
    pub fn resolve(&self, value: i64) -> Option<&T> {
        match self {
            Self::Static(v) => Some(v),
            Self::Ranges(ranges) => ranges
                .iter()
                .find(|r| r.low <= value && value <= r.high)
                .and_then(|r| r.value.resolve(value)),
        }
    }
}

/// A color leaf: fixed, or picked along a gradient by the event percentage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorValue {
    Solid(Color),
    Gradient { zero: Color, hundred: Color },
}

/// How a color handler applies its color to the zone (`mode`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorMode {
    /// Every LED gets the color.
    Color,
    /// Light the first `percent` of the zone's keys; the last lit key is dimmed by its fraction.
    Percent,
    /// Light exactly `value` keys.
    Count,
    /// Take the color from the event's context frame.
    ContextColor,
}

/// `rate`: flashing or repetition.
#[derive(Clone, Debug, PartialEq)]
pub struct RateSpec {
    pub frequency: Ranged<f64>,
    pub repeat_limit: Option<Ranged<u32>>,
}

/// A lighting handler with `color` / `percent` / `count` / `context-color` mode.
#[derive(Clone, Debug, PartialEq)]
pub struct ColorHandlerSpec {
    pub mode: ColorMode,
    /// `None` only for `context-color`.
    pub color: Option<Ranged<ColorValue>>,
    pub rate: Option<RateSpec>,
    pub context_frame_key: Option<String>,
}

/// Where a screen line takes its value from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Accessor {
    /// The event's `value` (default).
    EventValue,
    /// `"arg": ""`: no value, only prefix and suffix are shown.
    Empty,
    /// A path into the event's `frame` object, from `context-frame-key` or `arg`.
    FramePath(Vec<String>),
    /// A GoLisp expression this server cannot evaluate; the event value is shown instead.
    Unsupported(String),
}

/// One row of a text frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LineSpec {
    Text {
        /// `has-text`; when false only prefix and suffix are shown.
        show_value: bool,
        prefix: String,
        suffix: String,
        bold: bool,
        wrap: u32,
        accessor: Accessor,
    },
    ProgressBar {
        accessor: Accessor,
    },
}

/// A raw 1-bpp image frame.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ImageSpec {
    /// `image-data`: size is inferred from its length.
    pub data: Option<Vec<u8>>,
    /// `image-data-WxH` keys found on the frame.
    pub sized: Vec<(ScreenSize, Vec<u8>)>,
}

/// Body of a screen frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FrameBody {
    Lines(Vec<LineSpec>),
    Image(ImageSpec),
}

/// How often a frame list plays (`repeats` of the last frame).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Repeats {
    Once,
    Forever,
    /// Play the frame list this many times in total.
    Times(u32),
}

/// One frame of a screen handler.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScreenFrameSpec {
    pub body: FrameBody,
    /// `length-millis`; 0 keeps the frame until the next screen event.
    pub length_millis: u64,
    /// `icon-id`; 0 is no icon.
    pub icon_id: u32,
    pub repeats: Repeats,
    /// Non-SDK extension: frame priority, overrides the handler's.
    pub priority: Option<i32>,
}

/// A `screened` handler.
#[derive(Clone, Debug, PartialEq)]
pub struct ScreenHandlerSpec {
    pub datas: Ranged<Vec<ScreenFrameSpec>>,
    /// Non-SDK extension passed through to [`GameSenseOutput::Screen`](super::GameSenseOutput).
    pub priority: i32,
}

/// A `tactile` handler.
#[derive(Clone, Debug, PartialEq)]
pub struct TactileHandlerSpec {
    pub pattern: Ranged<Vec<TactileStep>>,
    pub rate: Option<RateSpec>,
}

/// What a handler does.
#[derive(Clone, Debug, PartialEq)]
pub enum HandlerKind {
    Color(ColorHandlerSpec),
    /// `bitmap` / `partial-bitmap` full-keyboard lighting.
    Bitmap {
        partial: bool,
        excluded_events: Vec<String>,
    },
    Screen(ScreenHandlerSpec),
    Tactile(TactileHandlerSpec),
}

/// A parsed, validated handler.
#[derive(Clone, Debug, PartialEq)]
pub struct HandlerSpec {
    pub device_type: DeviceType,
    pub zone: ZoneSpec,
    /// Resolved LEDs for color handlers; `None` for other kinds.
    pub target: Option<LightTarget>,
    pub kind: HandlerKind,
}

impl HandlerSpec {
    /// Zone name for screen and tactile outputs (`one` when not given).
    pub fn zone_name(&self) -> String {
        match &self.zone {
            ZoneSpec::Named(name) => name.clone(),
            ZoneSpec::CustomKeys(_) => "custom".to_string(),
        }
    }

    /// `arg` expressions of a screen handler that this server cannot evaluate.
    pub fn unsupported_args(&self) -> Vec<String> {
        let HandlerKind::Screen(screen) = &self.kind else {
            return Vec::new();
        };
        let mut found = Vec::new();
        for_each_frame(&screen.datas, &mut |frame| {
            if let FrameBody::Lines(lines) = &frame.body {
                for line in lines {
                    if let LineSpec::Text {
                        accessor: Accessor::Unsupported(arg),
                        ..
                    }
                    | LineSpec::ProgressBar {
                        accessor: Accessor::Unsupported(arg),
                    } = line
                    {
                        found.push(arg.clone());
                    }
                }
            }
        });
        found
    }
}

fn for_each_frame(datas: &Ranged<Vec<ScreenFrameSpec>>, visit: &mut dyn FnMut(&ScreenFrameSpec)) {
    match datas {
        Ranged::Static(frames) => {
            for frame in frames {
                visit(frame);
            }
        }
        Ranged::Ranges(ranges) => {
            for range in ranges {
                for_each_frame(&range.value, visit);
            }
        }
    }
}

/// Parse one entry of the `handlers` array: an object or a stringified object.
pub fn parse_handler(raw: &Value) -> Result<HandlerSpec, HandlerError> {
    let parsed;
    let value = match raw {
        Value::String(text) => {
            parsed = serde_json::from_str::<Value>(text).map_err(|e| invalid(format!("not JSON: {e}")))?;
            &parsed
        }
        other => other,
    };
    let obj = value.as_object().ok_or_else(|| invalid("handler is not an object"))?;
    let type_name = str_field(obj, "device-type")?.ok_or_else(|| invalid("missing device-type"))?;
    let device_type =
        DeviceType::parse(type_name).ok_or_else(|| HandlerError::UnknownDeviceType(type_name.to_string()))?;
    let mode = str_field(obj, "mode")?.map(str::to_ascii_lowercase);

    match device_type {
        DeviceType::Screened(_) => {
            let zone = ZoneSpec::Named(str_field(obj, "zone")?.unwrap_or("one").to_string());
            let spec = parse_screen_handler(obj)?;
            Ok(HandlerSpec {
                device_type,
                zone,
                target: None,
                kind: HandlerKind::Screen(spec),
            })
        }
        DeviceType::Tactile => {
            let zone = ZoneSpec::Named(str_field(obj, "zone")?.unwrap_or("one").to_string());
            let pattern_raw = obj.get("pattern").ok_or_else(|| invalid("missing pattern"))?;
            let pattern = parse_ranged(pattern_raw, "pattern", 0, &parse_tactile_steps)?;
            let rate = obj.get("rate").map(parse_rate).transpose()?;
            Ok(HandlerSpec {
                device_type,
                zone,
                target: None,
                kind: HandlerKind::Tactile(TactileHandlerSpec { pattern, rate }),
            })
        }
        _ if matches!(mode.as_deref(), Some("bitmap" | "partial-bitmap")) => {
            let partial = mode.as_deref() == Some("partial-bitmap");
            let excluded_events = match obj.get("excluded-events") {
                Some(v) => string_list(v).ok_or_else(|| invalid("excluded-events must be a list of strings"))?,
                None => Vec::new(),
            };
            Ok(HandlerSpec {
                device_type,
                zone: ZoneSpec::Named("all".to_string()),
                target: Some(LightTarget::AllZones),
                kind: HandlerKind::Bitmap {
                    partial,
                    excluded_events,
                },
            })
        }
        _ => {
            let zone = parse_zone_spec(obj)?;
            let target = resolve_zone(&device_type, &zone).map_err(invalid)?;
            let mode = match mode.as_deref() {
                None | Some("color") => ColorMode::Color,
                Some("percent") => ColorMode::Percent,
                Some("count") => ColorMode::Count,
                Some("context-color") => ColorMode::ContextColor,
                Some(other) => return Err(invalid(format!("unknown lighting mode '{other}'"))),
            };
            let context_frame_key = str_field(obj, "context-frame-key")?.map(str::to_string);
            let color = match obj.get("color") {
                Some(v) => Some(parse_ranged(v, "color", 0, &parse_color_value)?),
                None => None,
            };
            if mode == ColorMode::ContextColor {
                if context_frame_key.is_none() {
                    return Err(invalid("context-color mode needs context-frame-key"));
                }
            } else if color.is_none() {
                return Err(invalid("missing color"));
            }
            let rate = obj.get("rate").map(parse_rate).transpose()?;
            Ok(HandlerSpec {
                device_type,
                zone,
                target: Some(target),
                kind: HandlerKind::Color(ColorHandlerSpec {
                    mode,
                    color,
                    rate,
                    context_frame_key,
                }),
            })
        }
    }
}

fn str_field<'a>(obj: &'a Map<String, Value>, key: &str) -> Result<Option<&'a str>, HandlerError> {
    match obj.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.as_str())),
        Some(_) => Err(invalid(format!("{key} must be a string"))),
    }
}

fn string_list(value: &Value) -> Option<Vec<String>> {
    value
        .as_array()?
        .iter()
        .take(MAX_LIST_LEN)
        .map(|v| v.as_str().map(str::to_string))
        .collect()
}

fn parse_zone_spec(obj: &Map<String, Value>) -> Result<ZoneSpec, HandlerError> {
    if let Some(keys) = obj.get("custom-zone-keys") {
        let list = keys
            .as_array()
            .ok_or_else(|| invalid("custom-zone-keys must be an array"))?;
        if list.len() > MAX_LIST_LEN {
            return Err(invalid("custom-zone-keys has too many entries"));
        }
        let codes = list
            .iter()
            .map(|v| v.as_u64().and_then(|n| u8::try_from(n).ok()))
            .collect::<Option<Vec<u8>>>()
            .ok_or_else(|| invalid("custom-zone-keys entries must be HID codes 0-255"))?;
        return Ok(ZoneSpec::CustomKeys(codes));
    }
    match str_field(obj, "zone")? {
        Some(zone) => Ok(ZoneSpec::Named(zone.to_string())),
        None => Err(invalid("missing zone or custom-zone-keys")),
    }
}

/// Parse a value that may be a leaf or a list of `{low, high, <key>}` ranges.
///
/// The legacy object form `{"<key>": [ranges]}` is accepted too, because the old builder
/// types of this crate serialized ranges that way.
fn parse_ranged<T>(
    value: &Value,
    key: &str,
    depth: usize,
    leaf: &dyn Fn(&Value) -> Result<T, HandlerError>,
) -> Result<Ranged<T>, HandlerError> {
    if depth > MAX_RANGE_DEPTH {
        return Err(invalid(format!("{key} ranges are nested too deeply")));
    }
    if let Some(list) = value.as_array() {
        if is_range_list(list) {
            if list.len() > MAX_LIST_LEN {
                return Err(invalid(format!("{key} has too many ranges")));
            }
            let mut ranges = Vec::with_capacity(list.len());
            for entry in list {
                let obj = entry
                    .as_object()
                    .ok_or_else(|| invalid(format!("{key} range is not an object")))?;
                let low = int_field(obj, "low")?.ok_or_else(|| invalid(format!("{key} range missing low")))?;
                let high = int_field(obj, "high")?.ok_or_else(|| invalid(format!("{key} range missing high")))?;
                let inner = obj
                    .get(key)
                    .ok_or_else(|| invalid(format!("{key} range missing {key}")))?;
                ranges.push(RangeEntry {
                    low,
                    high,
                    value: parse_ranged(inner, key, depth + 1, leaf)?,
                });
            }
            return Ok(Ranged::Ranges(ranges));
        }
    }
    if let Some(obj) = value.as_object() {
        if obj.len() == 1 {
            if let Some(Value::Array(list)) = obj.get(key) {
                if is_range_list(list) {
                    return parse_ranged(&Value::Array(list.clone()), key, depth + 1, leaf);
                }
            }
        }
    }
    leaf(value).map(Ranged::Static)
}

fn is_range_list(list: &[Value]) -> bool {
    list.first()
        .and_then(Value::as_object)
        .is_some_and(|o| o.contains_key("low") && o.contains_key("high"))
}

/// Integer from a JSON number; floats are rounded.
pub(crate) fn json_to_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(n) => n
            .as_i64()
            .or_else(|| n.as_u64().map(|u| i64::try_from(u).unwrap_or(i64::MAX)))
            .or_else(|| n.as_f64().filter(|f| f.is_finite()).map(|f| f.round() as i64)),
        _ => None,
    }
}

fn int_field(obj: &Map<String, Value>, key: &str) -> Result<Option<i64>, HandlerError> {
    match obj.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => json_to_i64(v)
            .map(Some)
            .ok_or_else(|| invalid(format!("{key} must be a number"))),
    }
}

fn bool_field(obj: &Map<String, Value>, key: &str) -> Result<Option<bool>, HandlerError> {
    match obj.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(_) => Err(invalid(format!("{key} must be true or false"))),
    }
}

fn non_negative(obj: &Map<String, Value>, key: &str) -> Result<Option<u64>, HandlerError> {
    match int_field(obj, key)? {
        Some(n) if n < 0 => Err(invalid(format!("{key} must not be negative"))),
        Some(n) => Ok(Some(n.unsigned_abs())),
        None => Ok(None),
    }
}

fn channel(obj: &Map<String, Value>, key: &str) -> Result<u8, HandlerError> {
    let raw = int_field(obj, key)?.ok_or_else(|| invalid(format!("color missing {key}")))?;
    Ok(raw.clamp(0, 255) as u8)
}

/// `{"red", "green", "blue"}` with channels clamped to 0-255.
pub(crate) fn parse_static_color(value: &Value) -> Option<Color> {
    let obj = value.as_object()?;
    Some(Color::new(
        channel(obj, "red").ok()?,
        channel(obj, "green").ok()?,
        channel(obj, "blue").ok()?,
    ))
}

fn parse_color_value(value: &Value) -> Result<ColorValue, HandlerError> {
    let obj = value.as_object().ok_or_else(|| invalid("color must be an object"))?;
    if let Some(gradient) = obj.get("gradient") {
        let g = gradient
            .as_object()
            .ok_or_else(|| invalid("gradient must be an object"))?;
        let zero = g
            .get("zero")
            .and_then(parse_static_color)
            .ok_or_else(|| invalid("gradient.zero must be a color"))?;
        let hundred = g
            .get("hundred")
            .and_then(parse_static_color)
            .ok_or_else(|| invalid("gradient.hundred must be a color"))?;
        return Ok(ColorValue::Gradient { zero, hundred });
    }
    Ok(ColorValue::Solid(Color::new(
        channel(obj, "red")?,
        channel(obj, "green")?,
        channel(obj, "blue")?,
    )))
}

fn parse_rate(value: &Value) -> Result<RateSpec, HandlerError> {
    let obj = value.as_object().ok_or_else(|| invalid("rate must be an object"))?;
    let frequency_raw = obj.get("frequency").ok_or_else(|| invalid("rate missing frequency"))?;
    let frequency = parse_ranged(frequency_raw, "frequency", 0, &|v| match v {
        Value::Number(n) => n
            .as_f64()
            .filter(|f| f.is_finite() && *f >= 0.0)
            .ok_or_else(|| invalid("frequency must be a non-negative number")),
        _ => Err(invalid("frequency must be a number")),
    })?;
    let repeat_limit = match obj.get("repeat_limit") {
        None | Some(Value::Null) => None,
        Some(v) => Some(parse_ranged(v, "repeat_limit", 0, &|v| {
            json_to_i64(v)
                .filter(|n| *n >= 0)
                .map(|n| u32::try_from(n).unwrap_or(u32::MAX))
                .ok_or_else(|| invalid("repeat_limit must be a non-negative integer"))
        })?),
    };
    Ok(RateSpec {
        frequency,
        repeat_limit,
    })
}

fn parse_tactile_steps(value: &Value) -> Result<Vec<TactileStep>, HandlerError> {
    let list = value.as_array().ok_or_else(|| invalid("pattern must be an array"))?;
    if list.len() > MAX_TACTILE_STEPS {
        return Err(invalid(format!("pattern has more than {MAX_TACTILE_STEPS} entries")));
    }
    list.iter()
        .map(|entry| {
            let obj = entry
                .as_object()
                .ok_or_else(|| invalid("pattern entry is not an object"))?;
            let kind = str_field(obj, "type")?.ok_or_else(|| invalid("pattern entry missing type"))?;
            // Delays above the SDK maximum are ignored, as the SDK documents.
            let delay_ms = non_negative(obj, "delay-ms")?
                .filter(|d| *d <= MAX_TACTILE_MS)
                .map(|d| d as u32);
            if kind == "custom" {
                let length =
                    non_negative(obj, "length-ms")?.ok_or_else(|| invalid("custom pattern entry missing length-ms"))?;
                Ok(TactileStep::Custom {
                    length_ms: length.min(MAX_TACTILE_MS) as u32,
                    delay_ms,
                })
            } else {
                Ok(TactileStep::Predefined {
                    name: kind.to_string(),
                    delay_ms,
                })
            }
        })
        .collect()
}

fn parse_screen_handler(obj: &Map<String, Value>) -> Result<ScreenHandlerSpec, HandlerError> {
    let datas_raw = obj.get("datas").ok_or_else(|| invalid("missing datas"))?;
    if !datas_raw.is_array() {
        return Err(invalid("datas must be an array"));
    }
    let datas = parse_ranged(datas_raw, "datas", 0, &parse_frame_list)?;
    let priority = int_field(obj, "priority")?
        .unwrap_or(0)
        .clamp(i32::MIN.into(), i32::MAX.into()) as i32;
    Ok(ScreenHandlerSpec { datas, priority })
}

fn parse_frame_list(value: &Value) -> Result<Vec<ScreenFrameSpec>, HandlerError> {
    let list = value.as_array().ok_or_else(|| invalid("datas must be an array"))?;
    if list.len() > MAX_LIST_LEN {
        return Err(invalid("datas has too many frames"));
    }
    list.iter().map(parse_frame).collect()
}

fn parse_frame(value: &Value) -> Result<ScreenFrameSpec, HandlerError> {
    let obj = value
        .as_object()
        .ok_or_else(|| invalid("screen frame is not an object"))?;
    let length_millis = non_negative(obj, "length-millis")?.unwrap_or(0);
    let icon_id = non_negative(obj, "icon-id")?.unwrap_or(0).min(u64::from(u32::MAX)) as u32;
    let repeats = match obj.get("repeats") {
        None | Some(Value::Null) | Some(Value::Bool(false)) => Repeats::Once,
        Some(Value::Bool(true)) => Repeats::Forever,
        Some(v) => match json_to_i64(v) {
            Some(0) => Repeats::Forever,
            Some(n) if n > 0 => Repeats::Times(u32::try_from(n).unwrap_or(u32::MAX)),
            _ => return Err(invalid("repeats must be a boolean or a non-negative integer")),
        },
    };
    let priority = int_field(obj, "priority")?.map(|p| p.clamp(i32::MIN.into(), i32::MAX.into()) as i32);

    let body = if let Some(lines) = obj.get("lines") {
        let list = lines.as_array().ok_or_else(|| invalid("lines must be an array"))?;
        if list.is_empty() || list.len() > MAX_LIST_LEN {
            return Err(invalid("lines must hold 1 to 256 entries"));
        }
        let parsed = list
            .iter()
            .map(|line| {
                line.as_object()
                    .ok_or_else(|| invalid("line is not an object"))
                    .and_then(parse_line)
            })
            .collect::<Result<Vec<_>, _>>()?;
        FrameBody::Lines(parsed)
    } else if is_image_frame(obj)? {
        FrameBody::Image(parse_image(obj)?)
    } else {
        FrameBody::Lines(vec![parse_line(obj)?])
    };

    Ok(ScreenFrameSpec {
        body,
        length_millis,
        icon_id,
        repeats,
        priority,
    })
}

fn is_image_frame(obj: &Map<String, Value>) -> Result<bool, HandlerError> {
    let has_text = bool_field(obj, "has-text")?;
    let has_bar = bool_field(obj, "has-progress-bar")?.unwrap_or(false);
    let has_image_key = obj.keys().any(|k| k.starts_with("image-data"));
    Ok(!has_bar && (has_text == Some(false) || (has_text.is_none() && has_image_key)))
}

fn parse_line(obj: &Map<String, Value>) -> Result<LineSpec, HandlerError> {
    let accessor = match (str_field(obj, "arg")?, str_field(obj, "context-frame-key")?) {
        (Some(arg), _) => parse_arg(arg),
        (None, Some(key)) => Accessor::FramePath(vec![key.to_string()]),
        (None, None) => Accessor::EventValue,
    };
    if bool_field(obj, "has-progress-bar")?.unwrap_or(false) && bool_field(obj, "has-text")? != Some(true) {
        return Ok(LineSpec::ProgressBar { accessor });
    }
    Ok(LineSpec::Text {
        show_value: bool_field(obj, "has-text")?.unwrap_or(true),
        prefix: str_field(obj, "prefix")?.unwrap_or_default().to_string(),
        suffix: str_field(obj, "suffix")?.unwrap_or_default().to_string(),
        bold: bool_field(obj, "bold")?.unwrap_or(false),
        wrap: non_negative(obj, "wrap")?.unwrap_or(0).min(u64::from(u32::MAX)) as u32,
        accessor,
    })
}

/// Parse an `arg` accessor.
///
/// Supported forms: `""`, `(value: self)`, GoLisp accessor chains into the context frame such
/// as `(name: (context-frame: self))` or `(b: (a: (context-frame: self)))`, and plain dotted
/// paths such as `frame.a.b` or `a.b`. Anything else is [`Accessor::Unsupported`].
pub fn parse_arg(arg: &str) -> Accessor {
    let trimmed = arg.trim();
    if trimmed.is_empty() {
        return Accessor::Empty;
    }
    if !trimmed.starts_with('(') {
        let path = trimmed.strip_prefix("frame.").unwrap_or(trimmed);
        let parts: Vec<String> = path.split('.').map(str::to_string).collect();
        if parts.iter().all(|p| is_plain_key(p)) {
            return Accessor::FramePath(parts);
        }
        return Accessor::Unsupported(arg.to_string());
    }
    let mut keys = Vec::new();
    let mut rest = trimmed;
    loop {
        let Some(inner) = rest.strip_prefix('(').and_then(|r| r.strip_suffix(')')) else {
            return Accessor::Unsupported(arg.to_string());
        };
        let Some((key, operand)) = inner.split_once(':') else {
            return Accessor::Unsupported(arg.to_string());
        };
        let key = key.trim();
        let operand = operand.trim();
        if !is_plain_key(key) {
            return Accessor::Unsupported(arg.to_string());
        }
        if operand == "self" {
            keys.reverse();
            return match key {
                "value" if keys.is_empty() => Accessor::EventValue,
                "context-frame" => Accessor::FramePath(keys),
                _ => Accessor::Unsupported(arg.to_string()),
            };
        }
        keys.push(key.to_string());
        rest = operand;
    }
}

fn is_plain_key(key: &str) -> bool {
    !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
}

fn parse_image(obj: &Map<String, Value>) -> Result<ImageSpec, HandlerError> {
    let mut image = ImageSpec::default();
    for (key, value) in obj {
        if key == "image-data" {
            image.data = Some(byte_array(value).ok_or_else(|| invalid("image-data must be bytes"))?);
        } else if let Some(size) = key.strip_prefix("image-data-") {
            let size = ScreenSize::parse(size).ok_or_else(|| invalid(format!("bad image key '{key}'")))?;
            let bytes = byte_array(value).ok_or_else(|| invalid(format!("{key} must be bytes")))?;
            image.sized.push((size, bytes));
        }
    }
    Ok(image)
}

/// JSON array of integers 0-255.
pub(crate) fn byte_array(value: &Value) -> Option<Vec<u8>> {
    value
        .as_array()?
        .iter()
        .map(|v| v.as_u64().and_then(|n| u8::try_from(n).ok()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse(v: Value) -> Result<HandlerSpec, HandlerError> {
        parse_handler(&v)
    }

    #[test]
    fn parses_official_keyboard_example() {
        let spec = parse(json!({
            "device-type": "keyboard",
            "zone": "function-keys",
            "color": {"gradient": {"zero": {"red": 255, "green": 0, "blue": 0},
                                   "hundred": {"red": 0, "green": 255, "blue": 0}}},
            "mode": "percent"
        }))
        .unwrap();
        assert_eq!(spec.device_type, DeviceType::Keyboard);
        assert!(matches!(spec.target, Some(LightTarget::Keys(ref k)) if k.len() == 12));
        match spec.kind {
            HandlerKind::Color(c) => {
                assert_eq!(c.mode, ColorMode::Percent);
                assert_eq!(
                    c.color,
                    Some(Ranged::Static(ColorValue::Gradient {
                        zero: Color::new(255, 0, 0),
                        hundred: Color::new(0, 255, 0)
                    }))
                );
            }
            other => panic!("unexpected kind {other:?}"),
        }
    }

    #[test]
    fn parses_stringified_handler() {
        let text = r#"{"device-type":"rgb-2-zone","zone":"two","color":{"red":1,"green":2,"blue":3}}"#;
        let spec = parse_handler(&Value::String(text.to_string())).unwrap();
        assert_eq!(spec.device_type, DeviceType::RgbZoned(2));
        assert_eq!(spec.target, Some(LightTarget::Zone(1)));
    }

    #[test]
    fn unknown_device_type_is_reported_separately() {
        let err = parse(json!({"device-type": "toaster", "zone": "one", "color": {"red": 1, "green": 1, "blue": 1}}))
            .unwrap_err();
        assert_eq!(err, HandlerError::UnknownDeviceType("toaster".to_string()));
    }

    #[test]
    fn rejects_malformed_handlers() {
        assert!(parse(json!({"zone": "one"})).is_err());
        assert!(parse(json!({"device-type": "keyboard", "zone": "q"})).is_err());
        assert!(parse(json!({"device-type": "keyboard", "color": {"red": 1, "green": 1, "blue": 1}})).is_err());
        assert!(parse(json!({"device-type": "keyboard", "zone": "q", "mode": "context-color"})).is_err());
        assert!(
            parse(json!({"device-type": "rgb-3-zone", "zone": "four", "color": {"red": 1, "green": 1, "blue": 1}}))
                .is_err()
        );
        assert!(parse(json!({"device-type": "tactile", "zone": "one"})).is_err());
        assert!(parse(json!({"device-type": "screened", "zone": "one"})).is_err());
    }

    #[test]
    fn parses_nested_color_ranges_and_legacy_form() {
        let official = parse(json!({
            "device-type": "rgb-per-key-zones",
            "zone": "esc",
            "color": [
                {"low": 0, "high": 10, "color": {"red": 255, "green": 0, "blue": 0}},
                {"low": 11, "high": 100, "color": [
                    {"low": 11, "high": 50, "color": {"gradient": {"zero": {"red": 0, "green": 0, "blue": 0},
                                                                   "hundred": {"red": 0, "green": 0, "blue": 255}}}},
                    {"low": 51, "high": 100, "color": {"red": 0, "green": 255, "blue": 0}}
                ]}
            ]
        }))
        .unwrap();
        let HandlerKind::Color(c) = official.kind else {
            panic!("expected color handler")
        };
        let Some(Ranged::Ranges(ranges)) = c.color else {
            panic!("expected ranges")
        };
        assert_eq!(ranges.len(), 2);
        assert!(matches!(ranges[1].value, Ranged::Ranges(ref inner) if inner.len() == 2));

        let legacy = parse(json!({
            "device-type": "keyboard",
            "zone": "q",
            "color": {"color": [{"low": 0, "high": 100, "color": {"red": 1, "green": 2, "blue": 3}}]}
        }))
        .unwrap();
        let HandlerKind::Color(c) = legacy.kind else {
            panic!("expected color handler")
        };
        assert!(matches!(c.color, Some(Ranged::Ranges(ref r)) if r.len() == 1));
    }

    #[test]
    fn parses_bitmap_handler() {
        let spec = parse(json!({
            "device-type": "rgb-per-key-zones",
            "mode": "partial-bitmap",
            "excluded-events": ["AMMO", "HEALTH"]
        }))
        .unwrap();
        assert_eq!(
            spec.kind,
            HandlerKind::Bitmap {
                partial: true,
                excluded_events: vec!["AMMO".to_string(), "HEALTH".to_string()]
            }
        );
    }

    #[test]
    fn parses_tactile_handler() {
        let spec = parse(json!({
            "device-type": "tactile",
            "zone": "one",
            "mode": "vibrate",
            "pattern": [
                {"low": 61, "high": 100, "pattern": []},
                {"low": 1, "high": 60, "pattern": [
                    {"type": "custom", "length-ms": 9000, "delay-ms": 150},
                    {"type": "ti_predefined_sharpclick_60", "delay-ms": 3000}
                ]}
            ],
            "rate": {"frequency": 2, "repeat_limit": 3}
        }))
        .unwrap();
        let HandlerKind::Tactile(t) = spec.kind else {
            panic!("expected tactile")
        };
        assert_eq!(
            t.pattern.resolve(10),
            Some(&vec![
                TactileStep::Custom {
                    length_ms: 2560,
                    delay_ms: Some(150)
                },
                TactileStep::Predefined {
                    name: "ti_predefined_sharpclick_60".to_string(),
                    delay_ms: None
                }
            ])
        );
        assert_eq!(t.pattern.resolve(80), Some(&Vec::new()));
        assert_eq!(t.pattern.resolve(0), None);
        assert!(t.rate.is_some());
    }

    #[test]
    fn parses_screen_frames() {
        let spec = parse(json!({
            "device-type": "screened-128x40",
            "mode": "screen",
            "zone": "one",
            "datas": [
                {"has-text": true, "prefix": "HP ", "icon-id": 1, "length-millis": 250},
                {"lines": [{"has-text": true, "context-frame-key": "name", "bold": true, "wrap": 1},
                           {"has-progress-bar": true}],
                 "repeats": 2},
                {"has-text": false, "image-data-128x40": [0, 255]}
            ]
        }))
        .unwrap();
        let HandlerKind::Screen(s) = spec.kind else {
            panic!("expected screen")
        };
        let Ranged::Static(frames) = s.datas else {
            panic!("expected static frames")
        };
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].length_millis, 250);
        assert_eq!(frames[0].icon_id, 1);
        assert!(matches!(&frames[0].body, FrameBody::Lines(l) if l.len() == 1));
        assert_eq!(frames[1].repeats, Repeats::Times(2));
        match &frames[1].body {
            FrameBody::Lines(lines) => {
                assert!(
                    matches!(&lines[0], LineSpec::Text { bold: true, wrap: 1, accessor: Accessor::FramePath(p), .. } if p == &["name".to_string()])
                );
                assert!(matches!(
                    &lines[1],
                    LineSpec::ProgressBar {
                        accessor: Accessor::EventValue
                    }
                ));
            }
            other => panic!("unexpected body {other:?}"),
        }
        assert!(
            matches!(&frames[2].body, FrameBody::Image(img) if img.sized == vec![(ScreenSize::new(128, 40), vec![0, 255])])
        );
    }

    #[test]
    fn parses_arg_accessors() {
        assert_eq!(parse_arg(""), Accessor::Empty);
        assert_eq!(parse_arg("(value: self)"), Accessor::EventValue);
        assert_eq!(
            parse_arg("(textvalue: (context-frame: self))"),
            Accessor::FramePath(vec!["textvalue".to_string()])
        );
        assert_eq!(
            parse_arg("(hp: (player: (context-frame: self)))"),
            Accessor::FramePath(vec!["player".to_string(), "hp".to_string()])
        );
        assert_eq!(
            parse_arg("frame.player.hp"),
            Accessor::FramePath(vec!["player".to_string(), "hp".to_string()])
        );
        assert_eq!(parse_arg("ammo"), Accessor::FramePath(vec!["ammo".to_string()]));
        assert!(matches!(
            parse_arg("(/ (numericalvalue: (context-frame: self)) 44)"),
            Accessor::Unsupported(_)
        ));
        assert!(matches!(
            parse_arg("(string-upcase (textvalue: (context-frame: self)))"),
            Accessor::Unsupported(_)
        ));
    }
}
