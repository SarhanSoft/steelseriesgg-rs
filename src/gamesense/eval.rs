//! Evaluation of parsed handlers against one event update.
//!
//! Everything here is pure: given a handler and the event value/frame, return what should be
//! shown. The server turns the results into [`GameSenseOutput`](super::GameSenseOutput)s.

use std::ops::Range;
use std::time::Duration;

use serde_json::{Map, Value};

use super::handlers::{
    Accessor, ColorHandlerSpec, ColorMode, ColorValue, FrameBody, ImageSpec, LineSpec, Ranged, RateSpec, Repeats,
    ScreenHandlerSpec, TactileHandlerSpec, byte_array, json_to_i64, parse_static_color,
};
use super::output::{DeviceType, LightTarget, Rate, ScreenContent, ScreenLine, ScreenSize, TactileStep};
use crate::rgb::Color;

/// Number of colors in a full-keyboard `bitmap` frame (22 columns x 6 rows).
pub(crate) const BITMAP_LEN: usize = 132;

/// Screen sizes from "Zones by device type", used to infer the size of a plain `image-data`.
const KNOWN_SCREENS: [ScreenSize; 4] = [
    ScreenSize::new(128, 36),
    ScreenSize::new(128, 40),
    ScreenSize::new(128, 48),
    ScreenSize::new(128, 52),
];

/// The event update a handler is evaluated against.
#[derive(Clone, Copy, Debug)]
pub(crate) struct EventInput<'a> {
    pub value: i64,
    pub min_value: i64,
    pub max_value: i64,
    pub frame: Option<&'a Map<String, Value>>,
}

impl EventInput<'_> {
    /// Event value as a percentage of `min_value..=max_value`, clamped to 0-100.
    /// With the SDK defaults (0-100) this is the raw value.
    pub fn percent(&self) -> f64 {
        if self.max_value > self.min_value {
            let span = self.max_value.saturating_sub(self.min_value) as f64;
            let offset = self.value.saturating_sub(self.min_value) as f64;
            (offset / span * 100.0).clamp(0.0, 100.0)
        } else if self.value > self.min_value {
            100.0
        } else {
            0.0
        }
    }
}

impl ColorValue {
    /// Color at `percent` (0-100). Solid colors ignore it.
    pub fn at(&self, percent: f64) -> Color {
        match self {
            Self::Solid(c) => *c,
            Self::Gradient { zero, hundred } => {
                let t = (percent / 100.0).clamp(0.0, 1.0);
                Color::new(
                    lerp(zero.r, hundred.r, t),
                    lerp(zero.g, hundred.g, t),
                    lerp(zero.b, hundred.b, t),
                )
            }
        }
    }
}

fn lerp(a: u8, b: u8, t: f64) -> u8 {
    let (a, b) = (f64::from(a), f64::from(b));
    (a + (b - a) * t).round().clamp(0.0, 255.0) as u8
}

fn scale(color: Color, factor: f64) -> Color {
    let f = factor.clamp(0.0, 1.0);
    let s = |c: u8| (f64::from(c) * f).round().clamp(0.0, 255.0) as u8;
    Color::new(s(color.r), s(color.g), s(color.b))
}

/// Color for an event value.
///
/// A top-level static color is black for value 0, as the SDK specifies. Ranges compare the raw
/// value; `None` means no range contains it.
pub(crate) fn evaluate_color(def: &Ranged<ColorValue>, value: i64, percent: f64) -> Option<Color> {
    match def {
        Ranged::Static(ColorValue::Solid(c)) => Some(if value == 0 { Color::BLACK } else { *c }),
        _ => def.resolve(value).map(|cv| cv.at(percent)),
    }
}

/// Flash/repeat settings for an event value; `None` when the frequency is 0 or undefined.
pub(crate) fn evaluate_rate(rate: &RateSpec, value: i64) -> Option<Rate> {
    let frequency = rate.frequency.resolve(value).copied().unwrap_or(0.0);
    if frequency <= 0.0 {
        return None;
    }
    Some(Rate {
        frequency_hz: frequency as f32,
        repeat_limit: rate.repeat_limit.as_ref().and_then(|r| r.resolve(value).copied()),
    })
}

/// Color from the context frame (`context-color` mode).
pub(crate) fn context_color(frame: Option<&Map<String, Value>>, key: &str) -> Option<Color> {
    frame?.get(key).and_then(parse_static_color)
}

/// Part of a zone and the color it gets.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LightSegment {
    pub target: LightTarget,
    pub color: Color,
    /// False for the keys `percent`/`count` leave off.
    pub lit: bool,
}

/// Result of a color handler for one event update.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LightingResult {
    pub segments: Vec<LightSegment>,
    pub flash: Option<Rate>,
}

/// Evaluate a color handler. `None` when nothing should change (for example a
/// `context-color` event without that key in its frame).
pub(crate) fn evaluate_lighting(
    target: &LightTarget,
    spec: &ColorHandlerSpec,
    input: &EventInput<'_>,
) -> Option<LightingResult> {
    let color = match spec.mode {
        ColorMode::ContextColor => context_color(input.frame, spec.context_frame_key.as_deref()?)?,
        _ => evaluate_color(spec.color.as_ref()?, input.value, input.percent()).unwrap_or(Color::BLACK),
    };
    let flash = spec.rate.as_ref().and_then(|r| evaluate_rate(r, input.value));

    let mut segments = Vec::with_capacity(3);
    let mut push = |range: Range<usize>, color: Color, lit: bool| {
        if let Some(target) = slice_target(target, range) {
            segments.push(LightSegment { target, color, lit });
        }
    };
    match (spec.mode, key_count(target)) {
        (ColorMode::Percent, Some(n)) => {
            let lit = input.percent() / 100.0 * n as f64;
            let full = (lit.floor() as usize).min(n);
            let fraction = lit - full as f64;
            push(0..full, color, true);
            let mut next = full;
            if full < n && fraction > 1e-6 {
                push(full..full + 1, scale(color, fraction), true);
                next += 1;
            }
            push(next..n, Color::BLACK, false);
        }
        (ColorMode::Count, Some(n)) => {
            let count = input.value.clamp(0, n as i64) as usize;
            push(0..count, color, true);
            push(count..n, Color::BLACK, false);
        }
        _ => segments.push(LightSegment {
            target: target.clone(),
            color,
            lit: true,
        }),
    }
    Some(LightingResult { segments, flash })
}

fn key_count(target: &LightTarget) -> Option<usize> {
    match target {
        LightTarget::Keys(keys) => Some(keys.len()),
        LightTarget::HidCodes(codes) => Some(codes.len()),
        _ => None,
    }
}

fn slice_target(target: &LightTarget, range: Range<usize>) -> Option<LightTarget> {
    if range.is_empty() {
        return None;
    }
    match target {
        LightTarget::Keys(keys) => keys.get(range).map(|k| LightTarget::Keys(k.to_vec())),
        LightTarget::HidCodes(codes) => codes.get(range).map(|c| LightTarget::HidCodes(c.to_vec())),
        other => Some(other.clone()),
    }
}

/// One resolved screen frame. `variants` holds one content per screen size; a text frame on a
/// generic `screened` handler has a single variant with no size.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ResolvedFrame {
    pub variants: Vec<(Option<ScreenSize>, ScreenContent)>,
    pub duration: Option<Duration>,
    pub priority: i32,
}

/// The frames a screen handler shows for one event update.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ScreenPlan {
    pub frames: Vec<ResolvedFrame>,
    pub repeats: Repeats,
}

/// Resolve a screen handler into text lines, progress bars and bitmaps.
pub(crate) fn resolve_screen(
    device_type: &DeviceType,
    spec: &ScreenHandlerSpec,
    input: &EventInput<'_>,
) -> Option<ScreenPlan> {
    let frames = spec.datas.resolve(input.value)?;
    let size = match device_type {
        DeviceType::Screened(size) => *size,
        _ => None,
    };
    let mut resolved = Vec::with_capacity(frames.len());
    for frame in frames {
        let variants = match &frame.body {
            FrameBody::Lines(lines) => vec![(
                size,
                ScreenContent::Lines {
                    lines: lines.iter().map(|line| resolve_line(line, input)).collect(),
                    icon_id: (frame.icon_id != 0).then_some(frame.icon_id),
                },
            )],
            FrameBody::Image(image) => resolve_image(image, size, input.frame),
        };
        if variants.is_empty() {
            tracing::debug!("GameSense: image frame has no data matching the screen size; skipped");
            continue;
        }
        resolved.push(ResolvedFrame {
            variants,
            duration: (frame.length_millis > 0).then(|| Duration::from_millis(frame.length_millis)),
            priority: frame.priority.unwrap_or(spec.priority),
        });
    }
    if resolved.is_empty() {
        return None;
    }
    let repeats = frames.last().map_or(Repeats::Once, |f| f.repeats);
    Some(ScreenPlan {
        frames: resolved,
        repeats,
    })
}

fn resolve_line(line: &LineSpec, input: &EventInput<'_>) -> ScreenLine {
    match line {
        LineSpec::Text {
            show_value,
            prefix,
            suffix,
            bold,
            wrap,
            accessor,
        } => {
            let value = if *show_value {
                accessor_text(accessor, input)
            } else {
                String::new()
            };
            ScreenLine::Text {
                text: format!("{prefix}{value}{suffix}"),
                bold: *bold,
                wrap: *wrap,
            }
        }
        LineSpec::ProgressBar { accessor } => ScreenLine::ProgressBar {
            percent: accessor_percent(accessor, input),
        },
    }
}

/// Follow a path of object keys (or array indexes) into the context frame.
pub(crate) fn frame_lookup<'a>(frame: Option<&'a Map<String, Value>>, path: &[String]) -> Option<&'a Value> {
    let (first, rest) = path.split_first()?;
    let mut current = frame?.get(first)?;
    for key in rest {
        current = match current {
            Value::Object(map) => map.get(key)?,
            Value::Array(list) => list.get(key.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(current)
}

/// Text for a JSON value: strings as-is, numbers and booleans formatted, null empty.
pub(crate) fn value_to_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn accessor_text(accessor: &Accessor, input: &EventInput<'_>) -> String {
    match accessor {
        Accessor::EventValue | Accessor::Unsupported(_) => input.value.to_string(),
        Accessor::Empty => String::new(),
        Accessor::FramePath(path) => frame_lookup(input.frame, path).map(value_to_text).unwrap_or_default(),
    }
}

fn accessor_percent(accessor: &Accessor, input: &EventInput<'_>) -> u8 {
    let raw = match accessor {
        Accessor::EventValue | Accessor::Unsupported(_) => Some(input.value),
        Accessor::Empty => None,
        Accessor::FramePath(path) => frame_lookup(input.frame, path).and_then(json_to_i64),
    };
    raw.map_or(0, |v| v.clamp(0, 100) as u8)
}

/// Pick the bitmap(s) of an image frame. Event data (`image-data-WxH` in the frame) wins over
/// the handler's default image. Data longer than the screen needs is truncated; shorter data
/// is rejected.
fn resolve_image(
    image: &ImageSpec,
    size: Option<ScreenSize>,
    frame: Option<&Map<String, Value>>,
) -> Vec<(Option<ScreenSize>, ScreenContent)> {
    let mut candidates: Vec<(ScreenSize, Vec<u8>)> = Vec::new();
    let mut add = |sz: ScreenSize, bytes: &[u8]| {
        if !candidates.iter().any(|(s, _)| *s == sz) {
            candidates.push((sz, bytes.to_vec()));
        }
    };
    if let Some(frame) = frame {
        for (key, value) in frame {
            if let Some(sz) = key.strip_prefix("image-data-").and_then(ScreenSize::parse) {
                if let Some(bytes) = byte_array(value) {
                    add(sz, &bytes);
                }
            }
        }
    }
    for (sz, bytes) in &image.sized {
        add(*sz, bytes);
    }
    if let Some(data) = &image.data {
        let inferred = size.or_else(|| KNOWN_SCREENS.iter().copied().find(|s| s.bitmap_len() == data.len()));
        if let Some(sz) = inferred {
            add(sz, data);
        }
    }
    candidates
        .into_iter()
        .filter(|(sz, _)| size.is_none_or(|s| s == *sz))
        .filter_map(|(sz, mut bytes)| {
            let needed = sz.bitmap_len();
            if bytes.len() < needed {
                tracing::debug!(
                    "GameSense: {}x{} image has {} bytes, needs {}",
                    sz.width,
                    sz.height,
                    bytes.len(),
                    needed
                );
                return None;
            }
            bytes.truncate(needed);
            Some((Some(sz), ScreenContent::Bitmap(bytes)))
        })
        .collect()
}

/// Pattern and repetition of a tactile handler; `None` for an empty or undefined pattern.
pub(crate) fn resolve_tactile(
    spec: &TactileHandlerSpec,
    input: &EventInput<'_>,
) -> Option<(Vec<TactileStep>, Option<Rate>)> {
    let pattern = spec.pattern.resolve(input.value)?;
    if pattern.is_empty() {
        return None;
    }
    let rate = spec.rate.as_ref().and_then(|r| evaluate_rate(r, input.value));
    Some((pattern.clone(), rate))
}

/// The 132 colors of a full-keyboard `bitmap` frame, from `frame.bitmap`.
pub(crate) fn parse_bitmap(frame: Option<&Map<String, Value>>) -> Option<Vec<Color>> {
    let list = frame?.get("bitmap")?.as_array()?;
    if list.len() != BITMAP_LEN {
        tracing::debug!("GameSense: bitmap has {} colors, expected {}", list.len(), BITMAP_LEN);
        return None;
    }
    list.iter()
        .map(|entry| match entry.as_array().map(Vec::as_slice) {
            Some([r, g, b]) => {
                let ch = |v: &Value| json_to_i64(v).map(|n| n.clamp(0, 255) as u8);
                Some(Color::new(ch(r)?, ch(g)?, ch(b)?))
            }
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::key_mapping::KeyId;
    use crate::gamesense::handlers::{HandlerKind, parse_handler};
    use crate::gamesense::zone_map::keyboard_zone_keys;
    use serde_json::json;

    const RED: Color = Color::new(255, 0, 0);
    const GREEN: Color = Color::new(0, 255, 0);

    fn input(value: i64) -> EventInput<'static> {
        EventInput {
            value,
            min_value: 0,
            max_value: 100,
            frame: None,
        }
    }

    fn color_handler(v: Value) -> (LightTarget, ColorHandlerSpec) {
        let spec = parse_handler(&v).unwrap();
        let target = spec.target.clone().unwrap();
        match spec.kind {
            HandlerKind::Color(c) => (target, c),
            other => panic!("not a color handler: {other:?}"),
        }
    }

    fn gradient() -> Ranged<ColorValue> {
        Ranged::Static(ColorValue::Gradient {
            zero: RED,
            hundred: GREEN,
        })
    }

    #[test]
    fn gradient_math() {
        let g = gradient();
        assert_eq!(evaluate_color(&g, 0, 0.0), Some(RED));
        assert_eq!(evaluate_color(&g, 100, 100.0), Some(GREEN));
        assert_eq!(evaluate_color(&g, 50, 50.0), Some(Color::new(128, 128, 0)));
        assert_eq!(evaluate_color(&g, 25, 25.0), Some(Color::new(191, 64, 0)));
        assert_eq!(evaluate_color(&g, 500, 150.0), Some(GREEN));
    }

    #[test]
    fn percent_uses_min_and_max() {
        let i = EventInput {
            value: 5,
            min_value: 0,
            max_value: 10,
            frame: None,
        };
        assert!((i.percent() - 50.0).abs() < f64::EPSILON);
        let i = EventInput {
            value: 150,
            min_value: 100,
            max_value: 200,
            frame: None,
        };
        assert!((i.percent() - 50.0).abs() < f64::EPSILON);
        assert!((input(-20).percent()).abs() < f64::EPSILON);
        assert!((input(130).percent() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn static_color_is_black_at_zero() {
        let s = Ranged::Static(ColorValue::Solid(RED));
        assert_eq!(evaluate_color(&s, 0, 0.0), Some(Color::BLACK));
        assert_eq!(evaluate_color(&s, 1, 1.0), Some(RED));
    }

    #[test]
    fn color_ranges() {
        let (_, spec) = color_handler(json!({
            "device-type": "keyboard", "zone": "q",
            "color": [
                {"low": 0, "high": 10, "color": {"red": 255, "green": 0, "blue": 0}},
                {"low": 11, "high": 100, "color": {"gradient": {"zero": {"red": 0, "green": 0, "blue": 0},
                                                                "hundred": {"red": 0, "green": 0, "blue": 200}}}}
            ]
        }));
        let def = spec.color.unwrap();
        assert_eq!(evaluate_color(&def, 0, 0.0), Some(RED));
        assert_eq!(evaluate_color(&def, 10, 10.0), Some(RED));
        assert_eq!(evaluate_color(&def, 50, 50.0), Some(Color::new(0, 0, 100)));
        assert_eq!(evaluate_color(&def, 101, 100.0), None);
        assert_eq!(evaluate_color(&def, -1, 0.0), None);
    }

    #[test]
    fn color_mode_lights_whole_zone() {
        let (target, spec) = color_handler(json!({
            "device-type": "rgb-per-key-zones", "zone": "function-keys", "mode": "color",
            "color": {"red": 255, "green": 0, "blue": 0}
        }));
        let result = evaluate_lighting(&target, &spec, &input(40)).unwrap();
        assert_eq!(result.segments.len(), 1);
        assert_eq!(result.segments[0].target, target);
        assert_eq!(result.segments[0].color, RED);
        assert_eq!(result.flash, None);
    }

    fn lit_keys(result: &LightingResult) -> Vec<(usize, Color)> {
        result
            .segments
            .iter()
            .filter(|s| s.lit)
            .map(|s| match &s.target {
                LightTarget::Keys(k) => (k.len(), s.color),
                other => panic!("unexpected target {other:?}"),
            })
            .collect()
    }

    #[test]
    fn percent_mode_lights_a_fraction_of_keys() {
        let (target, spec) = color_handler(json!({
            "device-type": "keyboard", "zone": "function-keys", "mode": "percent",
            "color": {"red": 0, "green": 255, "blue": 0}
        }));
        let r = evaluate_lighting(&target, &spec, &input(50)).unwrap();
        assert_eq!(lit_keys(&r), vec![(6, GREEN)]);
        assert_eq!(
            r.segments.last().map(|s| (&s.target, s.color, s.lit)),
            Some((
                &LightTarget::Keys(keyboard_zone_keys("function-keys").unwrap()[6..].to_vec()),
                Color::BLACK,
                false
            ))
        );

        // 55% of 12 keys = 6.6: six full keys, the seventh at 60% brightness.
        let r = evaluate_lighting(&target, &spec, &input(55)).unwrap();
        assert_eq!(lit_keys(&r), vec![(6, GREEN), (1, Color::new(0, 153, 0))]);
        assert_eq!(r.segments[1].target, LightTarget::Keys(vec![KeyId::F7]));

        let r = evaluate_lighting(&target, &spec, &input(100)).unwrap();
        assert_eq!(lit_keys(&r), vec![(12, GREEN)]);
        assert_eq!(r.segments.len(), 1);

        let r = evaluate_lighting(&target, &spec, &input(1)).unwrap();
        assert_eq!(r.segments.len(), 2, "one dimmed key and eleven off");
    }

    #[test]
    fn percent_mode_on_zoned_device_behaves_like_color() {
        let (target, spec) = color_handler(json!({
            "device-type": "rgb-3-zone", "zone": "two", "mode": "percent",
            "color": {"red": 255, "green": 0, "blue": 0}
        }));
        let r = evaluate_lighting(&target, &spec, &input(30)).unwrap();
        assert_eq!(
            r.segments,
            vec![LightSegment {
                target: LightTarget::Zone(1),
                color: RED,
                lit: true
            }]
        );
    }

    #[test]
    fn count_mode_lights_value_keys() {
        let (target, spec) = color_handler(json!({
            "device-type": "rgb-per-key-zones", "custom-zone-keys": [26, 4, 22, 7], "mode": "count",
            "color": {"red": 255, "green": 255, "blue": 255}
        }));
        let r = evaluate_lighting(&target, &spec, &input(3)).unwrap();
        assert_eq!(r.segments[0].target, LightTarget::HidCodes(vec![26, 4, 22]));
        assert_eq!(r.segments[0].color, Color::WHITE);
        assert_eq!(r.segments[1].target, LightTarget::HidCodes(vec![7]));
        assert_eq!(r.segments[1].color, Color::BLACK);

        let r = evaluate_lighting(&target, &spec, &input(9)).unwrap();
        assert_eq!(r.segments.len(), 1);
        assert_eq!(r.segments[0].target, LightTarget::HidCodes(vec![26, 4, 22, 7]));
    }

    #[test]
    fn context_color_mode_reads_frame() {
        let (target, spec) = color_handler(json!({
            "mode": "context-color", "device-type": "rgb-3-zone", "zone": "one",
            "context-frame-key": "zone-one-color"
        }));
        let frame = json!({"zone-one-color": {"red": 0, "green": 255, "blue": 0}});
        let ev = EventInput {
            value: 0,
            min_value: 0,
            max_value: 100,
            frame: frame.as_object(),
        };
        let r = evaluate_lighting(&target, &spec, &ev).unwrap();
        assert_eq!(r.segments[0].color, GREEN);
        assert_eq!(r.segments[0].target, LightTarget::Zone(0));
        assert_eq!(evaluate_lighting(&target, &spec, &input(0)), None);
    }

    #[test]
    fn rate_static_and_ranges() {
        let (target, spec) = color_handler(json!({
            "device-type": "rgb-per-key-zones", "zone": "esc", "mode": "color",
            "color": {"red": 255, "green": 0, "blue": 0},
            "rate": {"frequency": 2, "repeat_limit": 5}
        }));
        let r = evaluate_lighting(&target, &spec, &input(1)).unwrap();
        assert_eq!(
            r.flash,
            Some(Rate {
                frequency_hz: 2.0,
                repeat_limit: Some(5)
            })
        );

        let (_, spec) = color_handler(json!({
            "device-type": "headset", "zone": "earcups", "mode": "color",
            "color": {"red": 255, "green": 0, "blue": 0},
            "rate": {
                "frequency": [{"low": 0, "high": 10, "frequency": 10}, {"low": 11, "high": 20, "frequency": 5}],
                "repeat_limit": [{"low": 0, "high": 10, "repeat_limit": 10}, {"low": 11, "high": 100, "repeat_limit": 2}]
            }
        }));
        let rate = spec.rate.unwrap();
        assert_eq!(
            evaluate_rate(&rate, 5),
            Some(Rate {
                frequency_hz: 10.0,
                repeat_limit: Some(10)
            })
        );
        assert_eq!(
            evaluate_rate(&rate, 15),
            Some(Rate {
                frequency_hz: 5.0,
                repeat_limit: Some(2)
            })
        );
        assert_eq!(evaluate_rate(&rate, 50), None, "undefined range means no flashing");
    }

    fn screen_spec(v: Value) -> (DeviceType, ScreenHandlerSpec) {
        let spec = parse_handler(&v).unwrap();
        match spec.kind {
            HandlerKind::Screen(s) => (spec.device_type, s),
            other => panic!("not a screen handler: {other:?}"),
        }
    }

    fn lines_of(plan: &ScreenPlan, frame: usize) -> (&Vec<ScreenLine>, Option<u32>) {
        match &plan.frames[frame].variants[0].1 {
            ScreenContent::Lines { lines, icon_id } => (lines, *icon_id),
            other => panic!("expected lines, got {other:?}"),
        }
    }

    fn text(t: &str) -> ScreenLine {
        ScreenLine::Text {
            text: t.to_string(),
            bold: false,
            wrap: 0,
        }
    }

    #[test]
    fn screen_text_with_prefix_suffix_and_icon() {
        let (dt, spec) = screen_spec(json!({
            "device-type": "screened", "mode": "screen", "zone": "one",
            "datas": [
                {"has-text": true, "suffix": "Headshot!", "length-millis": 250, "arg": "", "icon-id": 7},
                {"has-text": true, "prefix": "Got ", "suffix": " kills", "icon-id": 6}
            ]
        }));
        let plan = resolve_screen(&dt, &spec, &input(15)).unwrap();
        assert_eq!(plan.frames.len(), 2);
        assert_eq!(plan.frames[0].duration, Some(Duration::from_millis(250)));
        assert_eq!(plan.frames[0].variants[0].0, None);
        assert_eq!(lines_of(&plan, 0), (&vec![text("Headshot!")], Some(7)));
        assert_eq!(lines_of(&plan, 1), (&vec![text("Got 15 kills")], Some(6)));
        assert_eq!(plan.frames[1].duration, None);
        assert_eq!(plan.repeats, Repeats::Once);
    }

    #[test]
    fn screen_lines_follow_arg_paths_into_frame() {
        let (dt, spec) = screen_spec(json!({
            "device-type": "screened-128x40", "mode": "screen", "zone": "one",
            "datas": [{
                "lines": [
                    {"has-text": true, "context-frame-key": "textvalue", "bold": true, "wrap": 1},
                    {"has-text": true, "arg": "(numericalvalue: (context-frame: self))", "suffix": " pts"},
                    {"has-text": true, "arg": "(hp: (player: (context-frame: self)))", "prefix": "HP "},
                    {"has-text": true, "arg": "frame.player.name"},
                    {"has-text": true, "arg": "(value: self)"},
                    {"has-text": true, "context-frame-key": "missing", "prefix": "x"},
                    {"has-progress-bar": true, "context-frame-key": "numericalvalue"},
                    {"has-progress-bar": true}
                ],
                "repeats": true
            }]
        }));
        let frame = json!({
            "textvalue": "this is some text",
            "numericalvalue": 88,
            "player": {"hp": 42, "name": "Mahdi"}
        });
        let ev = EventInput {
            value: 56,
            min_value: 0,
            max_value: 100,
            frame: frame.as_object(),
        };
        let plan = resolve_screen(&dt, &spec, &ev).unwrap();
        assert_eq!(plan.frames[0].variants[0].0, Some(ScreenSize::new(128, 40)));
        assert_eq!(plan.repeats, Repeats::Forever);
        let (lines, icon) = lines_of(&plan, 0);
        assert_eq!(icon, None);
        assert_eq!(
            lines,
            &vec![
                ScreenLine::Text {
                    text: "this is some text".to_string(),
                    bold: true,
                    wrap: 1
                },
                text("88 pts"),
                text("HP 42"),
                text("Mahdi"),
                text("56"),
                text("x"),
                ScreenLine::ProgressBar { percent: 88 },
                ScreenLine::ProgressBar { percent: 56 },
            ]
        );
    }

    #[test]
    fn screen_ranged_datas() {
        let (dt, spec) = screen_spec(json!({
            "device-type": "screened", "zone": "one", "mode": "screen",
            "datas": [
                {"low": 0, "high": 15, "datas": [{"has-text": true, "prefix": "low "}]},
                {"low": 16, "high": 100, "datas": [{"has-text": true, "prefix": "high "}]}
            ]
        }));
        let plan = resolve_screen(&dt, &spec, &input(3)).unwrap();
        assert_eq!(lines_of(&plan, 0).0, &vec![text("low 3")]);
        let plan = resolve_screen(&dt, &spec, &input(70)).unwrap();
        assert_eq!(lines_of(&plan, 0).0, &vec![text("high 70")]);
        assert_eq!(resolve_screen(&dt, &spec, &input(101)), None);
    }

    #[test]
    fn screen_image_frames() {
        let default_image: Vec<u8> = vec![0xF0; 640];
        let (dt, spec) = screen_spec(json!({
            "device-type": "screened-128x40", "zone": "one", "mode": "screen",
            "datas": [{"has-text": false, "image-data": default_image, "length-millis": 100}]
        }));
        let plan = resolve_screen(&dt, &spec, &input(1)).unwrap();
        assert_eq!(
            plan.frames[0].variants,
            vec![(Some(ScreenSize::new(128, 40)), ScreenContent::Bitmap(vec![0xF0; 640]))]
        );

        // Event data replaces the default image.
        let frame = json!({"image-data-128x40": vec![1u8; 640], "image-data-128x36": vec![2u8; 576]});
        let ev = EventInput {
            value: 1,
            min_value: 0,
            max_value: 100,
            frame: frame.as_object(),
        };
        let plan = resolve_screen(&dt, &spec, &ev).unwrap();
        assert_eq!(
            plan.frames[0].variants,
            vec![(Some(ScreenSize::new(128, 40)), ScreenContent::Bitmap(vec![1; 640]))]
        );

        // A generic `screened` handler gets one bitmap per size present.
        let (dt, spec) = screen_spec(json!({
            "device-type": "screened", "zone": "one", "mode": "screen",
            "datas": [{"has-text": false}]
        }));
        let plan = resolve_screen(&dt, &spec, &ev).unwrap();
        let sizes: Vec<_> = plan.frames[0].variants.iter().map(|(s, _)| *s).collect();
        assert_eq!(
            sizes,
            vec![Some(ScreenSize::new(128, 36)), Some(ScreenSize::new(128, 40))]
        );

        // Too short for the screen: frame dropped.
        let short = json!({"image-data-128x40": [1, 2, 3]});
        let ev = EventInput {
            value: 1,
            min_value: 0,
            max_value: 100,
            frame: short.as_object(),
        };
        let (dt, spec) = screen_spec(json!({
            "device-type": "screened-128x40", "zone": "one", "mode": "screen",
            "datas": [{"has-text": false}]
        }));
        assert_eq!(resolve_screen(&dt, &spec, &ev), None);
    }

    #[test]
    fn tactile_resolution() {
        let spec = parse_handler(&json!({
            "device-type": "tactile", "zone": "one", "mode": "vibrate",
            "pattern": [{"low": 0, "high": 50, "pattern": [{"type": "ti_predefined_strongclick_100"}]},
                        {"low": 51, "high": 100, "pattern": []}],
            "rate": {"frequency": 1, "repeat_limit": 2}
        }))
        .unwrap();
        let HandlerKind::Tactile(t) = spec.kind else {
            panic!("expected tactile")
        };
        let (pattern, rate) = resolve_tactile(&t, &input(10)).unwrap();
        assert_eq!(
            pattern,
            vec![TactileStep::Predefined {
                name: "ti_predefined_strongclick_100".to_string(),
                delay_ms: None
            }]
        );
        assert_eq!(
            rate,
            Some(Rate {
                frequency_hz: 1.0,
                repeat_limit: Some(2)
            })
        );
        assert_eq!(resolve_tactile(&t, &input(80)), None);
    }

    #[test]
    fn bitmap_frame() {
        let colors: Vec<Value> = (0..BITMAP_LEN).map(|i| json!([i % 256, 0, 300])).collect();
        let frame = json!({ "bitmap": colors });
        let parsed = parse_bitmap(frame.as_object()).unwrap();
        assert_eq!(parsed.len(), BITMAP_LEN);
        assert_eq!(parsed[5], Color::new(5, 0, 255));
        let short = json!({"bitmap": [[1, 2, 3]]});
        assert_eq!(parse_bitmap(short.as_object()), None);
        assert_eq!(parse_bitmap(None), None);
    }
}
