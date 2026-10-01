//! [EXPERIMENTAL] What a mouse profile is, and how it becomes reports and setting descriptors.
//!
//! A [`Profile`] mirrors one rivalcfg device file (`rivalcfg/devices/*.py`): an ordered list of
//! [`Setting`]s, each a command prefix plus a typed value, and optional save, battery and
//! firmware commands. The tables themselves live in `profiles.rs`. This module only interprets
//! them, so supporting a new mouse means adding data, not code.

use std::collections::BTreeMap;

use super::encode::{
    self, ButtonLayout, ChoiceEntry, ColorEffect, Gradient, GradientLayout, GradientStop, GradientV2Layout, MultiDpi,
    Report, ReportKind, Span,
};
use crate::devices::settings::{self, ChoiceOption, SettingDescriptor, SettingKind, SettingValue, Verification};
use crate::rgb::Color;
use crate::{Error, Result};

/// Bit OR-ed into the first command byte when a dual-mode mouse talks through its 2.4 GHz
/// dongle (`_WIRELESS_FLAG` in rivalcfg's `*_wireless_wireless.py`).
pub const WIRELESS_FLAG: u8 = 0x40;

/// Bytes rivalcfg reads back after each setting or save command in 2.4 GHz mode
/// (`_READBACK_LENGTH`). The content is discarded.
pub const WIRELESS_READBACK_LEN: usize = 64;

/// Descriptor id of the combined DPI setting.
pub const DPI_ID: &str = "dpi";
/// Descriptor id of the all-zones color setting.
pub const COLORS_ID: &str = "colors";
/// Descriptor id of the gradient cycle duration.
pub const GRADIENT_DURATION_ID: &str = "gradient_duration";
/// Descriptor id of the save-to-onboard-memory action.
pub const SAVE_ID: &str = "save";

/// Positions of the stops of a `<zone>_gradient` setting. rivalcfg's default gradients all
/// use these three positions, so the defaults round-trip exactly.
pub const GRADIENT_STOP_POSITIONS: [u8; 3] = [0, 33, 66];

/// How a profile setting is exposed beyond its own descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    /// Exposed as itself.
    Plain,
    /// One of several single-DPI commands (rivalcfg's `sensitivity1`, `sensitivity2`); they
    /// are exposed together as one [`DPI_ID`] setting, stage N driving preset N.
    DpiPreset,
    /// One LED zone, named for [`COLORS_ID`] and the direct-color API.
    Zone(&'static str),
}

/// A gradient default: three stops at [`GRADIENT_STOP_POSITIONS`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectDefault {
    pub duration_ms: u32,
    pub colors: [Color; 3],
}

/// The value a setting takes, with everything its encoder needs. Each variant is one rivalcfg
/// `value_type`.
#[derive(Clone, Copy, Debug)]
pub enum Value {
    /// "choice"
    Choice {
        entries: &'static [ChoiceEntry],
        default: &'static str,
    },
    /// "range"
    Range {
        input: Span,
        output: Span,
        width: u8,
        default: u32,
        unit: Option<&'static str>,
    },
    /// "range_choice"
    RangeChoice {
        input: Span,
        table: &'static [(u32, u32)],
        width: u8,
        default: u32,
        unit: Option<&'static str>,
    },
    /// "multidpi_range", "multidpi_range_choice", "multidpi_range_choice_xy"
    MultiDpi { spec: MultiDpi, default: &'static [u32] },
    /// "rgbcolor"
    Rgb { default: Color },
    /// "reactive_rgbcolor"; `None` means off.
    Reactive { default: Option<Color> },
    /// "rgbgradient"
    Gradient {
        layout: GradientLayout,
        led_id: u8,
        default: EffectDefault,
    },
    /// "rgbgradientv2"
    GradientV2 {
        layout: GradientV2Layout,
        led_id: u8,
        default: EffectDefault,
    },
    /// "buttons"
    Buttons(&'static ButtonLayout),
    /// "none": a command without a value.
    Trigger,
}

impl Value {
    const fn is_gradient(&self) -> bool {
        matches!(self, Value::Gradient { .. } | Value::GradientV2 { .. })
    }

    const fn effect_default(&self) -> Option<EffectDefault> {
        match self {
            Value::Gradient { default, .. } | Value::GradientV2 { default, .. } => Some(*default),
            _ => None,
        }
    }
}

/// One rivalcfg setting.
#[derive(Clone, Copy, Debug)]
pub struct Setting {
    /// rivalcfg's setting name (`buttons_mapping` is shortened to `buttons`).
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub report: ReportKind,
    pub command: &'static [u8],
    /// Bytes after the value (`command_suffix`).
    pub suffix: &'static [u8],
    /// Fixed payload length, zero-padded (`packet_length`); `0` = none.
    pub packet_length: usize,
    pub value: Value,
    pub role: Role,
}

impl Setting {
    pub const fn new(
        id: &'static str,
        label: &'static str,
        description: &'static str,
        report: ReportKind,
        command: &'static [u8],
        value: Value,
    ) -> Self {
        Self {
            id,
            label,
            description,
            report,
            command,
            suffix: &[],
            packet_length: 0,
            value,
            role: Role::Plain,
        }
    }

    pub const fn suffix(self, suffix: &'static [u8]) -> Self {
        Self { suffix, ..self }
    }

    pub const fn packet_length(self, packet_length: usize) -> Self {
        Self { packet_length, ..self }
    }

    pub const fn zone(self, name: &'static str) -> Self {
        Self {
            role: Role::Zone(name),
            ..self
        }
    }

    pub const fn dpi_preset(self) -> Self {
        Self {
            role: Role::DpiPreset,
            ..self
        }
    }

    /// Descriptor id of this zone's gradient setting: `logo_color` -> `logo_gradient`.
    pub fn gradient_id(&self) -> String {
        format!("{}_gradient", self.id.strip_suffix("_color").unwrap_or(self.id))
    }
}

/// A command without a value: save, battery query, firmware query.
#[derive(Clone, Copy, Debug)]
pub struct Command {
    pub report: ReportKind,
    pub bytes: &'static [u8],
    pub packet_length: usize,
}

impl Command {
    pub const fn output(bytes: &'static [u8]) -> Self {
        Self {
            report: ReportKind::Output,
            bytes,
            packet_length: 0,
        }
    }

    pub const fn packet_length(self, packet_length: usize) -> Self {
        Self { packet_length, ..self }
    }
}

/// How a battery response is read (`battery_level` lambdas in rivalcfg profiles).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BatteryFormat {
    /// `level = data[0]`, `charging = data[2] != 0` (Rival 3 Wireless, Rival 650).
    LevelThenCharging,
    /// `data[1]`: bit 7 = charging, low bits = level in 5 % steps plus one (Aerox and Prime
    /// Wireless).
    FlaggedSteps,
}

#[derive(Clone, Copy, Debug)]
pub struct BatteryQuery {
    pub command: Command,
    pub response_len: usize,
    pub format: BatteryFormat,
}

#[derive(Clone, Copy, Debug)]
pub struct FirmwareQuery {
    pub command: Command,
    pub response_len: usize,
}

/// One rivalcfg device file.
#[derive(Clone, Copy, Debug)]
pub struct Profile {
    /// rivalcfg's profile name without the "SteelSeries " prefix.
    pub name: &'static str,
    /// rivalcfg file the tables were ported from.
    pub source: &'static str,
    /// rivalcfg test file whose expectations the tests reproduce.
    pub test_source: &'static str,
    pub settings: &'static [Setting],
    /// 2.4 GHz mode of a dual-mode mouse: commands get [`WIRELESS_FLAG`] and a read-back.
    pub wireless: bool,
    pub save: Option<Command>,
    pub battery: Option<BatteryQuery>,
    pub firmware: Option<FirmwareQuery>,
}

/// A typed value handed to [`Profile::encode`].
#[derive(Clone, Copy, Debug)]
pub enum Input<'a> {
    Choice(&'a str),
    Int(i64),
    /// DPI stages as `(x, y)`.
    Dpi(&'a [(u32, u32)]),
    Color(Color),
    Reactive(Option<Color>),
    Effect(&'a ColorEffect),
    Buttons(&'a BTreeMap<String, String>),
    Trigger,
}

impl Profile {
    /// The profile setting with rivalcfg name `id`.
    pub fn setting(&self, id: &str) -> Option<&'static Setting> {
        self.settings.iter().find(|s| s.id == id)
    }

    /// Bytes to read back after each setting command; `0` when nothing is read.
    pub const fn readback_len(&self) -> usize {
        if self.wireless { WIRELESS_READBACK_LEN } else { 0 }
    }

    fn command_bytes(&self, bytes: &[u8]) -> Vec<u8> {
        let mut command = bytes.to_vec();
        if self.wireless {
            if let Some(first) = command.first_mut() {
                *first |= WIRELESS_FLAG;
            }
        }
        command
    }

    /// Report for a value-less command (save, battery, firmware).
    pub fn encode_command(&self, command: &Command) -> Report {
        Report {
            kind: command.report,
            data: encode::frame(&self.command_bytes(command.bytes), &[], &[], command.packet_length),
        }
    }

    /// Report that applies `input` to `setting`.
    pub fn encode(&self, setting: &Setting, input: &Input<'_>) -> Result<Report> {
        let payload = match (&setting.value, input) {
            (Value::Choice { entries, .. }, Input::Choice(id)) => encode::encode_choice(entries, id)?,
            (
                Value::Range {
                    input, output, width, ..
                },
                Input::Int(v),
            ) => encode::encode_range(*input, *output, *width, *v)?,
            (
                Value::RangeChoice {
                    input, table, width, ..
                },
                Input::Int(v),
            ) => encode::encode_range_choice(*input, table, *width, *v)?,
            (Value::MultiDpi { spec, .. }, Input::Dpi(stages)) => encode::encode_multidpi(spec, stages, 0)?,
            (Value::Rgb { .. }, Input::Color(c)) => encode::encode_rgb(*c).to_vec(),
            (Value::Reactive { .. }, Input::Reactive(c)) => encode::encode_reactive(*c),
            (Value::Gradient { layout, led_id, .. }, Input::Effect(effect)) => {
                encode::encode_gradient(layout, *led_id, effect)?
            }
            (Value::Gradient { layout, led_id, .. }, Input::Color(c)) => {
                encode::encode_gradient(layout, *led_id, &ColorEffect::Steady(*c))?
            }
            (Value::GradientV2 { layout, led_id, .. }, Input::Effect(effect)) => {
                encode::encode_gradient_v2(layout, *led_id, effect)?
            }
            (Value::GradientV2 { layout, led_id, .. }, Input::Color(c)) => {
                encode::encode_gradient_v2(layout, *led_id, &ColorEffect::Steady(*c))?
            }
            (Value::Buttons(layout), Input::Buttons(mapping)) => encode::encode_buttons(layout, mapping)?,
            (Value::Trigger, Input::Trigger) => Vec::new(),
            (_, input) => {
                return Err(Error::InvalidConfig(format!(
                    "{}: {input:?} is the wrong kind of value for this setting",
                    setting.id
                )));
            }
        };
        Ok(Report {
            kind: setting.report,
            data: encode::frame(
                &self.command_bytes(setting.command),
                &payload,
                setting.suffix,
                setting.packet_length,
            ),
        })
    }

    fn zones(&self) -> impl Iterator<Item = (&'static str, &'static Setting)> {
        self.settings.iter().filter_map(|s| match s.role {
            Role::Zone(name) => Some((name, s)),
            _ => None,
        })
    }

    fn dpi_presets(&self) -> impl Iterator<Item = &'static Setting> {
        self.settings.iter().filter(|s| s.role == Role::DpiPreset)
    }

    /// LED zone names, in command order.
    pub fn zone_names(&self) -> Vec<String> {
        self.zones().map(|(name, _)| name.to_string()).collect()
    }
}

/// Battery level (0-100) and charging flag from a battery response. Like rivalcfg, a level
/// outside 0-100 (mouse asleep or out of range) yields `(None, None)`.
pub fn parse_battery(format: BatteryFormat, data: &[u8]) -> (Option<u8>, Option<bool>) {
    let (level, charging) = match format {
        BatteryFormat::LevelThenCharging => (data.first().map(|l| i32::from(*l)), data.get(2).map(|c| *c != 0)),
        BatteryFormat::FlaggedSteps => (
            data.get(1).map(|b| (i32::from(*b & 0x7F) - 1) * 5),
            data.get(1).map(|b| b & 0x80 != 0),
        ),
    };
    match level.and_then(|l| u8::try_from(l).ok()).filter(|l| *l <= 100) {
        Some(level) => (Some(level), charging),
        None => (None, None),
    }
}

/// Firmware version as rivalcfg prints it: the response bytes joined with dots (`[1, 33]` ->
/// `"1.33"`). `None` when the mouse sent nothing.
pub fn parse_firmware(data: &[u8]) -> Option<String> {
    if data.is_empty() {
        return None;
    }
    Some(data.iter().map(u8::to_string).collect::<Vec<_>>().join("."))
}

/// Gradient state the device cannot report back: the cycle duration and the last gradient
/// sent to each zone, so a duration change can be re-applied.
#[derive(Clone, Debug, Default)]
pub struct EffectState {
    duration_ms: Option<u32>,
    gradients: BTreeMap<&'static str, Vec<Color>>,
}

impl EffectState {
    /// Forget every stored gradient, after steady colors replaced them on all zones.
    pub fn clear_gradients(&mut self) {
        self.gradients.clear();
    }
}

fn black() -> Color {
    Color::new(0, 0, 0)
}

fn descriptor(id: &str, label: &str, description: &str, kind: SettingKind, persists: bool) -> SettingDescriptor {
    SettingDescriptor::new(id, label, kind)
        .description(description)
        .verification(Verification::Reference)
        .persists_on_device(persists)
}

/// `(min, max, default)` of the gradient cycle time, from the first gradient zone: any 16-bit
/// value for "rgbgradient"; for "rgbgradientv2", rivalcfg's limits for a three-stop gradient
/// plus its closing stop.
fn gradient_duration_limits(profile: &Profile) -> Option<(u32, u32, u32)> {
    profile.zones().find_map(|(_, zone)| {
        let (min, max) = match zone.value {
            Value::Gradient { layout, .. } => {
                let bits = 8 * u32::from(layout.duration_len);
                (0, u32::try_from((1u64 << bits) - 1).unwrap_or(u32::MAX))
            }
            Value::GradientV2 { .. } => (
                encode::gradient_v2_min_duration(GRADIENT_STOP_POSITIONS.len() + 1),
                encode::GRADIENT_V2_MAX_DURATION_MS,
            ),
            _ => return None,
        };
        let default = zone.value.effect_default().map_or(min, |d| d.duration_ms);
        Some((min, max, default))
    })
}

/// Setting descriptors for a profile, in display order.
pub fn descriptors(profile: &Profile) -> Vec<SettingDescriptor> {
    let persists = profile.save.is_some();
    let persist_note = if persists {
        " Kept after unplugging only once 'save' is run."
    } else {
        " Lost when the mouse is unplugged."
    };
    let note = |text: &str| format!("{text}.{persist_note}");
    let mut out = Vec::new();
    let mut zones_done = false;
    let mut presets_done = false;

    for setting in profile.settings {
        match setting.role {
            Role::DpiPreset => {
                if presets_done {
                    continue;
                }
                presets_done = true;
                let presets: Vec<&Setting> = profile.dpi_presets().collect();
                let input = preset_input(setting);
                let defaults: Vec<u32> = presets.iter().filter_map(|p| preset_default(p)).collect();
                out.push(
                    descriptor(
                        DPI_ID,
                        "DPI presets",
                        &note(
                            "Sensitivity presets in DPI, switched with the DPI button. The first value sets \
                             preset 1, the second preset 2; a preset left out keeps its value",
                        ),
                        SettingKind::DpiStages {
                            min: input.start,
                            max: input.stop,
                            step: input.step,
                            max_stages: u8::try_from(presets.len()).unwrap_or(u8::MAX),
                        },
                        persists,
                    )
                    .default_value(SettingValue::Dpi(defaults)),
                );
            }
            Role::Zone(_) => {
                if !zones_done {
                    zones_done = true;
                    zone_group_descriptors(profile, persists, &note, &mut out);
                }
                let default = match setting.value {
                    Value::Rgb { default } => Some(SettingValue::Color(default)),
                    _ => None,
                };
                let mut d = descriptor(
                    setting.id,
                    setting.label,
                    &note(&if setting.value.is_gradient() {
                        format!("{}, as one steady color", setting.description)
                    } else {
                        setting.description.to_string()
                    }),
                    SettingKind::Color,
                    persists,
                );
                d.default = default;
                out.push(d);
                if let Some(effect) = setting.value.effect_default() {
                    out.push(
                        descriptor(
                            &setting.gradient_id(),
                            &format!("{} gradient", setting.label.trim_end_matches(" color")),
                            &note(&format!(
                                "Looping gradient through three colors placed at {}%, {}% and {}% of the cycle",
                                GRADIENT_STOP_POSITIONS[0], GRADIENT_STOP_POSITIONS[1], GRADIENT_STOP_POSITIONS[2]
                            )),
                            SettingKind::ColorZones {
                                zones: GRADIENT_STOP_POSITIONS.iter().map(|p| format!("{p}%")).collect(),
                            },
                            persists,
                        )
                        .default_value(SettingValue::Colors(effect.colors.to_vec())),
                    );
                }
            }
            Role::Plain => out.push(plain_descriptor(setting, persists, &note)),
        }
    }

    if profile.save.is_some() {
        out.push(descriptor(
            SAVE_ID,
            "Save to mouse",
            "Store the current settings in the mouse's onboard memory so they survive unplugging.",
            SettingKind::Action,
            true,
        ));
    }
    out
}

fn preset_input(setting: &Setting) -> Span {
    match setting.value {
        Value::Range { input, .. } | Value::RangeChoice { input, .. } => input,
        _ => Span::new(0, 0, 1),
    }
}

fn preset_default(setting: &Setting) -> Option<u32> {
    match setting.value {
        Value::Range { default, .. } | Value::RangeChoice { default, .. } => Some(default),
        _ => None,
    }
}

fn zone_group_descriptors(
    profile: &Profile,
    persists: bool,
    note: &dyn Fn(&str) -> String,
    out: &mut Vec<SettingDescriptor>,
) {
    let zones: Vec<(&str, &Setting)> = profile.zones().collect();
    if zones.len() > 1 {
        let defaults: Option<Vec<Color>> = zones
            .iter()
            .map(|(_, s)| match s.value {
                Value::Rgb { default } => Some(default),
                _ => None,
            })
            .collect();
        let mut d = descriptor(
            COLORS_ID,
            "LED colors",
            &note("One steady color per LED zone, or one color for all zones"),
            SettingKind::ColorZones {
                zones: zones.iter().map(|(name, _)| name.to_string()).collect(),
            },
            persists,
        );
        d.default = defaults.map(SettingValue::Colors);
        out.push(d);
    }
    if let Some((min, max, default)) = gradient_duration_limits(profile) {
        out.push(
            descriptor(
                GRADIENT_DURATION_ID,
                "Gradient cycle time",
                &note("Duration of one gradient cycle, used by the *_gradient settings"),
                SettingKind::Range {
                    min: i64::from(min),
                    max: i64::from(max),
                    step: 1,
                    unit: Some("ms".to_string()),
                },
                persists,
            )
            .default_value(SettingValue::Int(i64::from(default))),
        );
    }
}

fn plain_descriptor(setting: &Setting, persists: bool, note: &dyn Fn(&str) -> String) -> SettingDescriptor {
    let description = note(setting.description);
    let unit = |u: Option<&str>| u.map(str::to_string);
    match setting.value {
        Value::Choice { entries, default } => descriptor(
            setting.id,
            setting.label,
            &description,
            SettingKind::Choice {
                options: entries.iter().map(|e| ChoiceOption::new(e.id, e.label)).collect(),
            },
            persists,
        )
        .default_value(SettingValue::Choice(default.to_string())),
        Value::Range {
            input,
            default,
            unit: u,
            ..
        }
        | Value::RangeChoice {
            input,
            default,
            unit: u,
            ..
        } => descriptor(
            setting.id,
            setting.label,
            &description,
            SettingKind::Range {
                min: i64::from(input.start),
                max: i64::from(input.stop),
                step: i64::from(input.step),
                unit: unit(u),
            },
            persists,
        )
        .default_value(SettingValue::Int(i64::from(default))),
        Value::MultiDpi { spec, default } => {
            let input = spec.encoding.input();
            descriptor(
                DPI_ID,
                "DPI presets",
                &note(&format!(
                    "Up to {} sensitivity presets in DPI; the mouse's DPI button cycles through them",
                    spec.max_stages
                )),
                SettingKind::DpiStages {
                    min: input.start,
                    max: input.stop,
                    step: input.step,
                    max_stages: spec.max_stages,
                },
                persists,
            )
            .default_value(SettingValue::Dpi(default.to_vec()))
        }
        Value::Reactive { default } => descriptor(
            setting.id,
            setting.label,
            &note(&format!("{}; black turns the reaction off", setting.description)),
            SettingKind::Color,
            persists,
        )
        .default_value(SettingValue::Color(default.unwrap_or_else(black))),
        Value::Rgb { default } => descriptor(setting.id, setting.label, &description, SettingKind::Color, persists)
            .default_value(SettingValue::Color(default)),
        Value::Gradient { .. } | Value::GradientV2 { .. } => {
            descriptor(setting.id, setting.label, &description, SettingKind::Color, persists)
        }
        Value::Buttons(layout) => descriptor(
            setting.id,
            setting.label,
            &note(&format!(
                "{}. Give only the buttons to change, as button=action; names are lowercase, \
                 and actions are another button, a special action, a key or a media key",
                setting.description
            )),
            SettingKind::ButtonMap {
                buttons: layout.buttons.iter().map(|b| b.name.to_string()).collect(),
                actions: layout.action_names(),
            },
            persists,
        )
        .default_value(SettingValue::Buttons(layout.default_mapping())),
        Value::Trigger => descriptor(setting.id, setting.label, &description, SettingKind::Action, persists),
    }
}

/// Reports that apply descriptor `id` = `value`, after validating it against the profile's
/// descriptors.
pub fn reports_for(profile: &Profile, state: &mut EffectState, id: &str, value: &SettingValue) -> Result<Vec<Report>> {
    let descriptors = descriptors(profile);
    settings::validate_against(&descriptors, id, value)?;

    match (id, value) {
        (SAVE_ID, SettingValue::Trigger) => {
            let save = profile
                .save
                .as_ref()
                .ok_or_else(|| Error::Unsupported(format!("{} has no save command", profile.name)))?;
            return Ok(vec![profile.encode_command(save)]);
        }
        (DPI_ID, SettingValue::Dpi(stages)) => return dpi_reports(profile, stages),
        (COLORS_ID, SettingValue::Colors(colors)) => {
            state.clear_gradients();
            return direct_color_reports(profile, colors);
        }
        (GRADIENT_DURATION_ID, SettingValue::Int(ms)) => {
            let ms = u32::try_from(*ms).map_err(|_| Error::InvalidConfig(format!("{id}: invalid duration {ms}")))?;
            state.duration_ms = Some(ms);
            let mut reports = Vec::new();
            for (zone_id, colors) in &state.gradients {
                if let Some(setting) = profile.setting(zone_id) {
                    reports.push(gradient_report(profile, setting, ms, colors)?);
                }
            }
            return Ok(reports);
        }
        _ => {}
    }

    for (_, zone) in profile.zones() {
        if zone.value.is_gradient() && zone.gradient_id() == id {
            let SettingValue::Colors(colors) = value else {
                break;
            };
            let colors: Vec<Color> = match colors.as_slice() {
                [one] => vec![*one; GRADIENT_STOP_POSITIONS.len()],
                all => all.to_vec(),
            };
            let duration = state
                .duration_ms
                .or_else(|| zone.value.effect_default().map(|d| d.duration_ms))
                .unwrap_or(encode::DEFAULT_GRADIENT_DURATION_MS);
            let report = gradient_report(profile, zone, duration, &colors)?;
            state.gradients.insert(zone.id, colors);
            return Ok(vec![report]);
        }
    }

    let setting = profile
        .setting(id)
        .ok_or_else(|| Error::Unsupported(format!("setting '{id}' is not supported by {}", profile.name)))?;
    let report = match (&setting.value, value) {
        (Value::Choice { .. }, SettingValue::Choice(choice)) => profile.encode(setting, &Input::Choice(choice))?,
        (Value::Range { .. } | Value::RangeChoice { .. }, SettingValue::Int(v)) => {
            profile.encode(setting, &Input::Int(*v))?
        }
        (Value::Reactive { .. }, SettingValue::Color(c)) => {
            let reaction = (*c != black()).then_some(*c);
            profile.encode(setting, &Input::Reactive(reaction))?
        }
        (Value::Rgb { .. } | Value::Gradient { .. } | Value::GradientV2 { .. }, SettingValue::Color(c)) => {
            state.gradients.remove(setting.id);
            profile.encode(setting, &Input::Color(*c))?
        }
        (Value::Buttons(_), SettingValue::Buttons(map)) => profile.encode(setting, &Input::Buttons(map))?,
        (Value::Trigger, SettingValue::Trigger) => profile.encode(setting, &Input::Trigger)?,
        _ => {
            return Err(Error::InvalidConfig(format!("{id}: wrong value type for this setting")));
        }
    };
    Ok(vec![report])
}

fn gradient_report(profile: &Profile, zone: &Setting, duration_ms: u32, colors: &[Color]) -> Result<Report> {
    let effect = ColorEffect::Gradient(Gradient {
        duration_ms,
        stops: GRADIENT_STOP_POSITIONS
            .iter()
            .zip(colors)
            .map(|(position, color)| GradientStop {
                position: *position,
                color: *color,
            })
            .collect(),
    });
    profile.encode(zone, &Input::Effect(&effect))
}

fn dpi_reports(profile: &Profile, stages: &[u32]) -> Result<Vec<Report>> {
    if let Some(setting) = profile
        .settings
        .iter()
        .find(|s| matches!(s.value, Value::MultiDpi { .. }))
    {
        let pairs: Vec<(u32, u32)> = stages.iter().map(|d| (*d, *d)).collect();
        return Ok(vec![profile.encode(setting, &Input::Dpi(&pairs))?]);
    }
    let presets: Vec<&Setting> = profile.dpi_presets().collect();
    if presets.is_empty() {
        return Err(Error::Unsupported(format!("{} has no DPI presets", profile.name)));
    }
    presets
        .iter()
        .zip(stages)
        .map(|(preset, dpi)| profile.encode(preset, &Input::Int(i64::from(*dpi))))
        .collect()
}

/// One steady-color report per LED zone. `colors` holds one color per zone, or one color for
/// every zone.
pub fn direct_color_reports(profile: &Profile, colors: &[Color]) -> Result<Vec<Report>> {
    let zones: Vec<&Setting> = profile.zones().map(|(_, s)| s).collect();
    if zones.is_empty() {
        return Err(Error::Unsupported(format!("{} has no LED zones", profile.name)));
    }
    if colors.len() != zones.len() && colors.len() != 1 {
        return Err(Error::InvalidConfig(format!(
            "expected 1 or {} colors ({}), got {}",
            zones.len(),
            profile.zone_names().join(", "),
            colors.len()
        )));
    }
    zones
        .iter()
        .enumerate()
        .map(|(i, zone)| {
            let color = colors.get(i).or_else(|| colors.first()).copied().unwrap_or_else(black);
            profile.encode(zone, &Input::Color(color))
        })
        .collect()
}
