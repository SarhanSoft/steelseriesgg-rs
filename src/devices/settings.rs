//! Uniform, data-driven device settings.
//!
//! Every device family (keyboards, mice, headsets) describes what it can change as a list of
//! [`SettingDescriptor`]s and applies a change through one entry point,
//! [`Configurable::apply_setting`]. The CLI, the daemon's control API, profiles and the web UI
//! all consume this one shape, so adding a setting to a device makes it available everywhere
//! without touching any of them.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::rgb::Color;
use crate::{Error, Result};

/// How much trust a setting's protocol deserves.
///
/// No setting is ever presented as working unless it is [`Verification::Hardware`].
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verification {
    /// Confirmed on real hardware by this project.
    Hardware,
    /// Byte layout taken from a published open-source driver whose users report it working
    /// (OpenRGB, rivalcfg, HeadsetControl, apex-tux, ...). Not tested by this project.
    Reference,
    /// Extrapolated from a sibling model. May do nothing on the real device.
    Guess,
}

/// One selectable value of a [`SettingKind::Choice`].
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChoiceOption {
    /// Stable identifier used on the CLI and in profiles, e.g. `"1000"` or `"bass_boost"`.
    pub id: String,
    /// Human-readable label.
    pub label: String,
}

impl ChoiceOption {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

/// The shape of a setting's value, which also tells a UI which control to draw.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SettingKind {
    /// Integer in `min..=max`, moving in `step`s.
    Range {
        min: i64,
        max: i64,
        step: i64,
        unit: Option<String>,
    },
    /// One of a fixed list of options.
    Choice { options: Vec<ChoiceOption> },
    /// On / off.
    Toggle,
    /// A single RGB color.
    Color,
    /// One RGB color per named zone, in this order.
    ColorZones { zones: Vec<String> },
    /// A list of 1..=`max_stages` DPI values, each in `min..=max` and a multiple of `step`.
    DpiStages {
        min: u32,
        max: u32,
        step: u32,
        max_stages: u8,
    },
    /// Gain per fixed frequency band, in dB.
    Equalizer {
        bands_hz: Vec<u32>,
        min_db: f32,
        max_db: f32,
        step_db: f32,
    },
    /// Physical button -> action name. `actions` lists the accepted action names.
    ButtonMap { buttons: Vec<String>, actions: Vec<String> },
    /// A one-shot command with no value, e.g. "save to onboard memory".
    Action,
}

/// A concrete value for a setting.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingValue {
    Int(i64),
    Choice(String),
    Bool(bool),
    Color(Color),
    Colors(Vec<Color>),
    Dpi(Vec<u32>),
    Gains(Vec<f32>),
    Buttons(BTreeMap<String, String>),
    Trigger,
}

/// Static description of one setting a device exposes.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SettingDescriptor {
    /// Stable identifier, `snake_case`, e.g. `"sidetone"`, `"dpi"`, `"logo_color"`.
    pub id: String,
    pub label: String,
    pub description: String,
    pub kind: SettingKind,
    /// Value the device ships with, when known.
    pub default: Option<SettingValue>,
    pub verification: Verification,
    /// Whether the value survives a replug without the daemon re-applying it.
    pub persists_on_device: bool,
}

impl SettingDescriptor {
    pub fn new(id: impl Into<String>, label: impl Into<String>, kind: SettingKind) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: String::new(),
            kind,
            default: None,
            verification: Verification::Reference,
            persists_on_device: false,
        }
    }

    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.description = text.into();
        self
    }

    pub fn default_value(mut self, value: SettingValue) -> Self {
        self.default = Some(value);
        self
    }

    pub fn verification(mut self, verification: Verification) -> Self {
        self.verification = verification;
        self
    }

    pub fn persists_on_device(mut self, persists: bool) -> Self {
        self.persists_on_device = persists;
        self
    }

    /// Parse a CLI string into a value of this setting's kind, then validate it.
    ///
    /// Formats: integers `50`; choices by id or label; toggles `on|off|true|false|1|0`;
    /// colors `red` / `#ff0000` / `ff0000`; zones and lists comma-separated; button maps
    /// `button=action,button=action`.
    pub fn parse_value(&self, input: &str) -> Result<SettingValue> {
        let input = input.trim();
        let value = match &self.kind {
            SettingKind::Range { .. } => SettingValue::Int(
                input
                    .trim_end_matches('%')
                    .parse()
                    .map_err(|_| self.invalid(input, "expected an integer"))?,
            ),
            SettingKind::Choice { options } => {
                let found = options
                    .iter()
                    .find(|o| o.id.eq_ignore_ascii_case(input) || o.label.eq_ignore_ascii_case(input))
                    .ok_or_else(|| {
                        let ids: Vec<&str> = options.iter().map(|o| o.id.as_str()).collect();
                        self.invalid(input, &format!("expected one of: {}", ids.join(", ")))
                    })?;
                SettingValue::Choice(found.id.clone())
            }
            SettingKind::Toggle => {
                SettingValue::Bool(parse_bool(input).ok_or_else(|| self.invalid(input, "expected on/off"))?)
            }
            SettingKind::Color => {
                SettingValue::Color(Color::parse(input).ok_or_else(|| self.invalid(input, "expected a color"))?)
            }
            SettingKind::ColorZones { .. } => SettingValue::Colors(
                split_list(input)
                    .map(|c| Color::parse(c).ok_or_else(|| self.invalid(c, "expected a color")))
                    .collect::<Result<_>>()?,
            ),
            SettingKind::DpiStages { .. } => SettingValue::Dpi(
                split_list(input)
                    .map(|d| d.parse().map_err(|_| self.invalid(d, "expected a DPI number")))
                    .collect::<Result<_>>()?,
            ),
            SettingKind::Equalizer { .. } => SettingValue::Gains(
                split_list(input)
                    .map(|g| g.parse().map_err(|_| self.invalid(g, "expected a gain in dB")))
                    .collect::<Result<_>>()?,
            ),
            SettingKind::ButtonMap { .. } => {
                let mut map = BTreeMap::new();
                for pair in split_list(input) {
                    let (button, action) = pair
                        .split_once('=')
                        .ok_or_else(|| self.invalid(pair, "expected button=action"))?;
                    map.insert(button.trim().to_string(), action.trim().to_string());
                }
                SettingValue::Buttons(map)
            }
            SettingKind::Action => SettingValue::Trigger,
        };
        self.validate(&value)?;
        Ok(value)
    }

    /// Check that `value` has the right shape and lies within this setting's limits.
    pub fn validate(&self, value: &SettingValue) -> Result<()> {
        match (&self.kind, value) {
            (SettingKind::Range { min, max, step, .. }, SettingValue::Int(v)) => {
                if v < min || v > max {
                    return Err(self.invalid(&v.to_string(), &format!("must be between {min} and {max}")));
                }
                if *step > 1 && (v - min) % step != 0 {
                    return Err(self.invalid(&v.to_string(), &format!("must move in steps of {step}")));
                }
                Ok(())
            }
            (SettingKind::Choice { options }, SettingValue::Choice(id)) => {
                if options.iter().any(|o| &o.id == id) {
                    Ok(())
                } else {
                    Err(self.invalid(id, "unknown option"))
                }
            }
            (SettingKind::Toggle, SettingValue::Bool(_)) => Ok(()),
            (SettingKind::Color, SettingValue::Color(_)) => Ok(()),
            (SettingKind::ColorZones { zones }, SettingValue::Colors(colors)) => {
                if colors.len() == zones.len() || colors.len() == 1 {
                    Ok(())
                } else {
                    Err(self.invalid(
                        &format!("{} colors", colors.len()),
                        &format!("expected 1 or {} colors ({})", zones.len(), zones.join(", ")),
                    ))
                }
            }
            (
                SettingKind::DpiStages {
                    min,
                    max,
                    step,
                    max_stages,
                },
                SettingValue::Dpi(stages),
            ) => {
                if stages.is_empty() || stages.len() > *max_stages as usize {
                    return Err(self.invalid(
                        &format!("{} stages", stages.len()),
                        &format!("expected 1 to {max_stages} DPI values"),
                    ));
                }
                for dpi in stages {
                    if dpi < min || dpi > max {
                        return Err(self.invalid(&dpi.to_string(), &format!("DPI must be between {min} and {max}")));
                    }
                    if *step > 1 && dpi % step != 0 {
                        return Err(self.invalid(&dpi.to_string(), &format!("DPI must be a multiple of {step}")));
                    }
                }
                Ok(())
            }
            (
                SettingKind::Equalizer {
                    bands_hz,
                    min_db,
                    max_db,
                    ..
                },
                SettingValue::Gains(gains),
            ) => {
                if gains.len() != bands_hz.len() {
                    return Err(self.invalid(
                        &format!("{} gains", gains.len()),
                        &format!("expected {} gains", bands_hz.len()),
                    ));
                }
                if let Some(g) = gains.iter().find(|g| **g < *min_db || **g > *max_db || g.is_nan()) {
                    return Err(self.invalid(
                        &g.to_string(),
                        &format!("gain must be between {min_db} and {max_db} dB"),
                    ));
                }
                Ok(())
            }
            (SettingKind::ButtonMap { buttons, actions }, SettingValue::Buttons(map)) => {
                for (button, action) in map {
                    if !buttons.iter().any(|b| b == button) {
                        return Err(self.invalid(
                            button,
                            &format!("unknown button; expected one of: {}", buttons.join(", ")),
                        ));
                    }
                    if !actions.iter().any(|a| a == action) {
                        return Err(self.invalid(action, "unknown action"));
                    }
                }
                Ok(())
            }
            (SettingKind::Action, SettingValue::Trigger) => Ok(()),
            _ => Err(self.invalid(&format!("{value:?}"), "wrong value type for this setting")),
        }
    }

    fn invalid(&self, input: &str, why: &str) -> Error {
        Error::InvalidConfig(format!("{}: invalid value '{input}': {why}", self.id))
    }
}

fn split_list(input: &str) -> impl Iterator<Item = &str> {
    input.split(',').map(str::trim).filter(|s| !s.is_empty())
}

/// Parse common on/off spellings.
pub fn parse_bool(input: &str) -> Option<bool> {
    match input.to_ascii_lowercase().as_str() {
        "on" | "true" | "1" | "yes" | "enable" | "enabled" => Some(true),
        "off" | "false" | "0" | "no" | "disable" | "disabled" => Some(false),
        _ => None,
    }
}

/// Find a descriptor by id.
pub fn find_descriptor<'a>(descriptors: &'a [SettingDescriptor], id: &str) -> Option<&'a SettingDescriptor> {
    descriptors.iter().find(|d| d.id == id)
}

/// ChatMix dial position, as two volumes that always sum to roughly 100.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChatMix {
    /// Game channel volume, 0-100.
    pub game: u8,
    /// Chat channel volume, 0-100.
    pub chat: u8,
}

/// Live readings a device can report.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct DeviceStatus {
    /// Battery charge, 0-100.
    pub battery_percent: Option<u8>,
    pub charging: Option<bool>,
    /// For wireless devices: whether the device is linked to its dongle / base station.
    pub wireless_connected: Option<bool>,
    pub chatmix: Option<ChatMix>,
    pub mic_muted: Option<bool>,
    /// Anything model-specific, e.g. `"anc_mode" => "transparency"`.
    pub extra: BTreeMap<String, String>,
}

impl DeviceStatus {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Implemented by every device that exposes settings or live readings.
pub trait Configurable {
    /// Settings this device supports, in display order.
    fn setting_descriptors(&self) -> Vec<SettingDescriptor> {
        Vec::new()
    }

    /// Apply one setting. Implementations should call [`SettingDescriptor::validate`] first.
    fn apply_setting(&mut self, id: &str, _value: &SettingValue) -> Result<()> {
        Err(Error::Unsupported(format!(
            "setting '{id}' is not supported by this device"
        )))
    }

    /// Read battery, ChatMix and other live state. Returns an empty status when the device
    /// reports nothing.
    fn read_status(&mut self) -> Result<DeviceStatus> {
        Ok(DeviceStatus::default())
    }
}

/// Validate `value` against the descriptor `id` in `descriptors`, returning the descriptor.
pub fn validate_against<'a>(
    descriptors: &'a [SettingDescriptor],
    id: &str,
    value: &SettingValue,
) -> Result<&'a SettingDescriptor> {
    let descriptor = find_descriptor(descriptors, id)
        .ok_or_else(|| Error::Unsupported(format!("setting '{id}' is not supported by this device")))?;
    descriptor.validate(value)?;
    Ok(descriptor)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range() -> SettingDescriptor {
        SettingDescriptor::new(
            "sidetone",
            "Sidetone",
            SettingKind::Range {
                min: 0,
                max: 128,
                step: 1,
                unit: None,
            },
        )
    }

    #[test]
    fn parses_and_bounds_ranges() {
        assert_eq!(range().parse_value("64").unwrap(), SettingValue::Int(64));
        assert_eq!(range().parse_value("64%").unwrap(), SettingValue::Int(64));
        assert!(range().parse_value("129").is_err());
        assert!(range().parse_value("abc").is_err());
    }

    #[test]
    fn parses_choices_by_id_or_label() {
        let d = SettingDescriptor::new(
            "polling_rate",
            "Polling rate",
            SettingKind::Choice {
                options: vec![ChoiceOption::new("1000", "1000 Hz"), ChoiceOption::new("125", "125 Hz")],
            },
        );
        assert_eq!(d.parse_value("1000").unwrap(), SettingValue::Choice("1000".into()));
        assert_eq!(d.parse_value("125 hz").unwrap(), SettingValue::Choice("125".into()));
        assert!(d.parse_value("500").is_err());
    }

    #[test]
    fn parses_dpi_stages_with_limits() {
        let d = SettingDescriptor::new(
            "dpi",
            "DPI",
            SettingKind::DpiStages {
                min: 100,
                max: 8500,
                step: 100,
                max_stages: 5,
            },
        );
        assert_eq!(d.parse_value("800, 1600").unwrap(), SettingValue::Dpi(vec![800, 1600]));
        assert!(d.parse_value("850").is_err());
        assert!(d.parse_value("100,200,300,400,500,600").is_err());
    }

    #[test]
    fn color_zones_accept_one_or_all() {
        let d = SettingDescriptor::new(
            "colors",
            "Colors",
            SettingKind::ColorZones {
                zones: vec!["logo".into(), "wheel".into()],
            },
        );
        assert!(d.parse_value("red").is_ok());
        assert!(d.parse_value("red,#00ff00").is_ok());
        assert!(d.parse_value("red,green,blue").is_err());
    }

    #[test]
    fn button_map_checks_names() {
        let d = SettingDescriptor::new(
            "buttons",
            "Buttons",
            SettingKind::ButtonMap {
                buttons: vec!["button1".into(), "button2".into()],
                actions: vec!["button1".into(), "button2".into(), "disabled".into()],
            },
        );
        assert!(d.parse_value("button1=button2, button2=disabled").is_ok());
        assert!(d.parse_value("button9=disabled").is_err());
        assert!(d.parse_value("button1=fly").is_err());
    }

    #[test]
    fn values_round_trip_through_json() {
        let v = SettingValue::Colors(vec![Color::RED, Color::BLUE]);
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(serde_json::from_str::<SettingValue>(&json).unwrap(), v);
    }
}
