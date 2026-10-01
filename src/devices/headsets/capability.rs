//! What a headset model can change, and how each change becomes HID reports.
//!
//! A [`Capability`] pairs a user-facing setting ([`SettingId`]) with a per-model [`Encoding`].
//! The same setting is encoded differently by different models (sidetone is a bare level on a
//! Nova 7, an on/off-prefixed level on an Arctis 7 and an offset byte on an Arctis 9), so the
//! encoding data lives in the model table and the encoders here stay generic.
//!
//! [EXPERIMENTAL] Every encoding is taken from a published reference driver and none has been
//! tested on hardware by this project.

use super::lighting::{self, LightingState, LightingStyle, Nova3Effect};
use super::report::{Command, Packet, Report};
use crate::devices::settings::{ChoiceOption, SettingDescriptor, SettingKind, SettingValue, Verification};
use crate::rgb::Color;
use crate::{Error, Result};

/// Commands sent after a change so the headset keeps it across power cycles. Empty when the
/// reference sends none.
pub type SaveSequence = &'static [Packet];

/// A user-facing headset setting. The id strings are stable: profiles and the CLI use them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingId {
    Sidetone,
    InactiveTime,
    Lights,
    EqualizerPreset,
    Equalizer,
    MicVolume,
    MicMuteLedBrightness,
    VolumeLimiter,
    BluetoothWhenPoweredOn,
    BluetoothCallVolume,
    ChatMixDial,
    SonarIcon,
    LedColors,
    LedEffect,
    LedEffectSpeed,
    Save,
}

impl SettingId {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Sidetone => "sidetone",
            Self::InactiveTime => "inactive_time",
            Self::Lights => "lights",
            Self::EqualizerPreset => "equalizer_preset",
            Self::Equalizer => "equalizer",
            Self::MicVolume => "mic_volume",
            Self::MicMuteLedBrightness => "mic_mute_led_brightness",
            Self::VolumeLimiter => "volume_limiter",
            Self::BluetoothWhenPoweredOn => "bluetooth_when_powered_on",
            Self::BluetoothCallVolume => "bluetooth_call_volume",
            Self::ChatMixDial => "chatmix_dial",
            Self::SonarIcon => "sonar_icon",
            Self::LedColors => "led_colors",
            Self::LedEffect => "led_effect",
            Self::LedEffectSpeed => "led_effect_speed",
            Self::Save => "save",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Sidetone => "Sidetone",
            Self::InactiveTime => "Auto power-off",
            Self::Lights => "Lights",
            Self::EqualizerPreset => "Equalizer preset",
            Self::Equalizer => "Custom equalizer",
            Self::MicVolume => "Microphone volume",
            Self::MicMuteLedBrightness => "Mic mute LED brightness",
            Self::VolumeLimiter => "Volume limiter",
            Self::BluetoothWhenPoweredOn => "Bluetooth on power-up",
            Self::BluetoothCallVolume => "Bluetooth call volume",
            Self::ChatMixDial => "ChatMix dial",
            Self::SonarIcon => "Sonar icon",
            Self::LedColors => "LED colors",
            Self::LedEffect => "LED effect",
            Self::LedEffectSpeed => "LED effect speed",
            Self::Save => "Save to headset",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::Sidetone => {
                "How much of your own voice you hear in the headset, in the device's native steps. 0 is off."
            }
            Self::InactiveTime => "Minutes of inactivity before the headset turns itself off. 0 never turns it off.",
            Self::Lights => "Turn the headset or base-station lights on or off.",
            Self::EqualizerPreset => "Built-in equalizer curve.",
            Self::Equalizer => {
                "Gain per band in dB. Band frequencies are display labels; the device receives gains only, \
                 except on models whose packet carries the frequency."
            }
            Self::MicVolume => "Microphone gain, in the device's native steps.",
            Self::MicMuteLedBrightness => "Brightness of the LED that shows the microphone is muted.",
            Self::VolumeLimiter => "Cap the maximum output volume.",
            Self::BluetoothWhenPoweredOn => "Turn Bluetooth on together with the headset.",
            Self::BluetoothCallVolume => "What happens to game audio during a Bluetooth call.",
            Self::ChatMixDial => {
                "Let the base-station dial switch between volume and ChatMix. While on, the base station \
                 reports dial moves, which feed the ChatMix reading."
            }
            Self::SonarIcon => "Show the Sonar icon on the base-station display.",
            Self::LedColors => "One color per ear cup (left, right). A single color sets both.",
            Self::LedEffect => "Lighting effect. Static and breathe use the LED colors.",
            Self::LedEffectSpeed => "Speed of the breathe and color-shift effects, 1 (slow) to 10 (fast).",
            Self::Save => "Write the current settings to the headset's memory so they survive a power cycle.",
        }
    }
}

/// How a numeric level is placed after the command header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LevelStyle {
    /// `[header, level]`.
    Plain,
    /// Arctis 1 / 7: level 0 is the bare header, any other level is `[header, 0x01, 0x00, level]`.
    Switched,
    /// Arctis 9: `[header, base + level]`.
    Offset(u8),
}

/// A setting that is one integer in `0..=max`.
#[derive(Debug)]
pub struct Level {
    pub command: Command,
    pub max: u8,
    pub style: LevelStyle,
    pub save: SaveSequence,
}

const SWITCHED_ON_PREFIX: [u8; 2] = [0x01, 0x00];

impl Level {
    pub fn encode(&self, level: u8) -> Vec<Report> {
        let report = match self.style {
            LevelStyle::Plain => self.command.report(&[level]),
            LevelStyle::Switched if level == 0 => self.command.report(&[]),
            LevelStyle::Switched => self
                .command
                .report(&[SWITCHED_ON_PREFIX[0], SWITCHED_ON_PREFIX[1], level]),
            LevelStyle::Offset(base) => self.command.report(&[base.saturating_add(level)]),
        };
        with_save(vec![report], self.save)
    }
}

/// One selectable auto-off time and the byte the device expects for it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InactiveChoice {
    pub minutes: u8,
    pub raw: u8,
}

/// How the auto power-off time is encoded.
#[derive(Clone, Copy, Debug)]
pub enum InactiveStyle {
    /// `[header, minutes]`.
    Minutes { max: u8 },
    /// Arctis 9: `[header, seconds_hi, seconds_lo]`.
    Seconds { max_minutes: u8 },
    /// Arctis Pro Wireless: `[header, minutes / 10]`.
    TenMinuteSteps { max_minutes: u8 },
    /// A fixed list of times, each with its own byte.
    Choices(&'static [InactiveChoice]),
}

#[derive(Debug)]
pub struct Inactive {
    pub command: Command,
    pub style: InactiveStyle,
    pub save: SaveSequence,
}

impl Inactive {
    pub fn encode_minutes(&self, minutes: u8) -> Result<Vec<Report>> {
        let report = match self.style {
            InactiveStyle::Minutes { .. } => self.command.report(&[minutes]),
            InactiveStyle::Seconds { .. } => {
                let seconds = u16::from(minutes) * 60;
                self.command.report(&seconds.to_be_bytes())
            }
            InactiveStyle::TenMinuteSteps { .. } => self.command.report(&[minutes / 10]),
            InactiveStyle::Choices(choices) => {
                let choice = choices
                    .iter()
                    .find(|c| c.minutes == minutes)
                    .ok_or_else(|| invalid(SettingId::InactiveTime, &format!("{minutes} minutes is not offered")))?;
                self.command.report(&[choice.raw])
            }
        };
        Ok(with_save(vec![report], self.save))
    }
}

/// An on/off setting: `[header, prefix..., on | off]`.
#[derive(Debug)]
pub struct Toggle {
    pub command: Command,
    pub prefix: &'static [u8],
    pub on: u8,
    pub off: u8,
    pub save: SaveSequence,
}

impl Toggle {
    pub fn encode(&self, enabled: bool) -> Vec<Report> {
        let mut payload = self.prefix.to_vec();
        payload.push(if enabled { self.on } else { self.off });
        with_save(vec![self.command.report(&payload)], self.save)
    }
}

/// One option of a [`Choice`] and its device byte.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChoiceDef {
    pub id: &'static str,
    pub label: &'static str,
    pub raw: u8,
}

/// A setting picked from a fixed list: `[header, raw]`.
#[derive(Debug)]
pub struct Choice {
    pub command: Command,
    pub options: &'static [ChoiceDef],
    pub save: SaveSequence,
}

impl Choice {
    pub fn encode(&self, setting: SettingId, id: &str) -> Result<Vec<Report>> {
        let option = self
            .options
            .iter()
            .find(|o| o.id == id)
            .ok_or_else(|| invalid(setting, &format!("unknown option '{id}'")))?;
        Ok(with_save(vec![self.command.report(&[option.raw])], self.save))
    }

    fn kind(&self) -> SettingKind {
        SettingKind::Choice {
            options: self.options.iter().map(|o| ChoiceOption::new(o.id, o.label)).collect(),
        }
    }
}

/// How band gains are laid out after the equalizer header.
#[derive(Clone, Copy, Debug)]
pub enum BandEncoding {
    /// One byte per band: `baseline + gain * per_db`, truncated like the reference's `uint8_t` cast.
    Linear { baseline: u8, per_db: f32 },
    /// Arctis Nova 3P Wireless: six bytes per band - frequency (LE), filter (peaking), gain in
    /// 0.1 dB two's complement, Q x 1000 (LE).
    PeakingTenths,
    /// Arctis Nova 5: six bytes per band - frequency (LE), shelf flag, `20 + 2 * gain`,
    /// Q x 1000 (LE).
    FlaggedHalves,
}

/// A custom equalizer with fixed bands.
#[derive(Debug)]
pub struct Equalizer {
    pub command: Command,
    pub bands_hz: &'static [u32],
    pub min_db: f32,
    pub max_db: f32,
    pub step_db: f32,
    pub encoding: BandEncoding,
    /// Sent before the band values, e.g. "switch to the custom preset slot".
    pub prelude: &'static [Packet],
    pub save: SaveSequence,
}

const FILTER_PEAKING: u8 = 0x01;
const FLAG_BASELINE: u8 = 0x01;
const FLAG_FIRST_BAND: u8 = 0x04;
const FLAG_LAST_BAND: u8 = 0x05;
const HALVES_BASELINE: u8 = 20;
/// Q factor 1.414 x 1000, the reference's default for every band.
const DEFAULT_Q_MILLI: u16 = 1414;
const TENTH_DB: f32 = 0.1;

/// Gain in 0.1 dB units, negative values as two's complement - the reference's arithmetic,
/// including its single-precision truncation.
pub fn gain_tenths_byte(gain: f32) -> u8 {
    if gain < 0.0 {
        ((-gain / TENTH_DB) as u8).wrapping_neg()
    } else {
        (gain / TENTH_DB) as u8
    }
}

impl Equalizer {
    /// The bytes that follow the header for `gains` (one per band, already validated).
    pub fn band_bytes(&self, gains: &[f32]) -> Vec<u8> {
        match self.encoding {
            BandEncoding::Linear { baseline, per_db } => {
                gains.iter().map(|g| (f32::from(baseline) + g * per_db) as u8).collect()
            }
            BandEncoding::PeakingTenths => self
                .bands_hz
                .iter()
                .zip(gains)
                .flat_map(|(hz, g)| band_block(*hz, FILTER_PEAKING, gain_tenths_byte(*g)))
                .collect(),
            BandEncoding::FlaggedHalves => {
                let last = gains.len().saturating_sub(1);
                self.bands_hz
                    .iter()
                    .zip(gains)
                    .enumerate()
                    .flat_map(|(i, (hz, g))| {
                        let raw = (f32::from(HALVES_BASELINE) + g * 2.0) as u8;
                        let flag = match i {
                            _ if raw == HALVES_BASELINE => FLAG_BASELINE,
                            0 => FLAG_FIRST_BAND,
                            i if i == last => FLAG_LAST_BAND,
                            _ => FLAG_BASELINE,
                        };
                        band_block(*hz, flag, raw)
                    })
                    .collect()
            }
        }
    }

    pub fn encode(&self, gains: &[f32]) -> Vec<Report> {
        let mut reports: Vec<Report> = self.prelude.iter().map(Packet::report).collect();
        reports.push(self.command.report(&self.band_bytes(gains)));
        with_save(reports, self.save)
    }

    fn kind(&self) -> SettingKind {
        SettingKind::Equalizer {
            bands_hz: self.bands_hz.to_vec(),
            min_db: self.min_db,
            max_db: self.max_db,
            step_db: self.step_db,
        }
    }
}

fn band_block(hz: u32, flag: u8, gain: u8) -> [u8; 6] {
    let [f_lo, f_hi] = u16::try_from(hz).unwrap_or(u16::MAX).to_le_bytes();
    let [q_lo, q_hi] = DEFAULT_Q_MILLI.to_le_bytes();
    [f_lo, f_hi, flag, gain, q_lo, q_hi]
}

/// A preset stored as the raw band bytes the device expects.
#[derive(Clone, Copy, Debug)]
pub struct RawPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub bands: &'static [u8],
}

/// A preset stored as dB gains and sent through the model's custom equalizer encoder.
#[derive(Clone, Copy, Debug)]
pub struct GainPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub gains: &'static [f32],
}

/// How a model selects an equalizer preset.
#[derive(Debug)]
pub enum Preset {
    /// `[header, band bytes...]` from a stored table.
    RawBands {
        command: Command,
        presets: &'static [RawPreset],
        save: SaveSequence,
    },
    /// The preset's gains go through the model's custom equalizer.
    Gains {
        equalizer: &'static Equalizer,
        presets: &'static [GainPreset],
    },
    /// The device stores the curves; select one by index.
    Slot(Choice),
}

impl Preset {
    pub fn encode(&self, id: &str) -> Result<Vec<Report>> {
        let unknown = || invalid(SettingId::EqualizerPreset, &format!("unknown preset '{id}'"));
        match self {
            Self::RawBands { command, presets, save } => {
                let preset = presets.iter().find(|p| p.id == id).ok_or_else(unknown)?;
                Ok(with_save(vec![command.report(preset.bands)], save))
            }
            Self::Gains { equalizer, presets } => {
                let preset = presets.iter().find(|p| p.id == id).ok_or_else(unknown)?;
                Ok(equalizer.encode(preset.gains))
            }
            Self::Slot(choice) => choice.encode(SettingId::EqualizerPreset, id),
        }
    }

    fn kind(&self) -> SettingKind {
        let options = match self {
            Self::RawBands { presets, .. } => presets.iter().map(|p| ChoiceOption::new(p.id, p.label)).collect(),
            Self::Gains { presets, .. } => presets.iter().map(|p| ChoiceOption::new(p.id, p.label)).collect(),
            Self::Slot(choice) => return choice.kind(),
        };
        SettingKind::Choice { options }
    }

    fn persists(&self) -> bool {
        match self {
            Self::RawBands { save, .. } => !save.is_empty(),
            Self::Gains { equalizer, .. } => !equalizer.save.is_empty(),
            Self::Slot(choice) => !choice.save.is_empty(),
        }
    }
}

/// Which part of the lighting state a lighting setting changes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LightingPart {
    Colors,
    Effect,
    Speed,
}

/// Per-model encoding of one capability.
#[derive(Debug)]
pub enum Encoding {
    Level(Level),
    Inactive(Inactive),
    Toggle(Toggle),
    Choice(Choice),
    Equalizer(&'static Equalizer),
    Preset(Preset),
    Lighting(LightingStyle, LightingPart),
    /// A one-shot save: the packets to send.
    Save(SaveSequence),
}

/// One setting a headset model exposes, with its encoding and trust level.
#[derive(Debug)]
pub struct Capability {
    pub setting: SettingId,
    pub encoding: Encoding,
    pub verification: Verification,
}

impl Capability {
    /// A capability whose bytes come from a reference driver.
    pub const fn new(setting: SettingId, encoding: Encoding) -> Self {
        Self {
            setting,
            encoding,
            verification: Verification::Reference,
        }
    }

    /// Mark a capability extrapolated from a sibling model.
    pub const fn guess(self) -> Self {
        Self {
            verification: Verification::Guess,
            ..self
        }
    }

    pub fn id(&self) -> &'static str {
        self.setting.id()
    }

    /// The uniform settings descriptor for this capability.
    pub fn descriptor(&self) -> SettingDescriptor {
        let (kind, persists) = match &self.encoding {
            Encoding::Level(level) => (range(0, i64::from(level.max), 1, None), !level.save.is_empty()),
            Encoding::Inactive(inactive) => (inactive_kind(inactive.style), !inactive.save.is_empty()),
            Encoding::Toggle(toggle) => (SettingKind::Toggle, !toggle.save.is_empty()),
            Encoding::Choice(choice) => (choice.kind(), !choice.save.is_empty()),
            Encoding::Equalizer(eq) => (eq.kind(), !eq.save.is_empty()),
            Encoding::Preset(preset) => (preset.kind(), preset.persists()),
            Encoding::Lighting(_, part) => (lighting_kind(*part), false),
            Encoding::Save(_) => (SettingKind::Action, false),
        };
        SettingDescriptor::new(self.setting.id(), self.setting.label(), kind)
            .description(self.setting.description())
            .verification(self.verification)
            .persists_on_device(persists)
    }

    /// Turn an already validated value into the reports to send, updating `lighting` when the
    /// capability is a lighting one.
    pub fn encode(&self, value: &SettingValue, lighting: &mut LightingState) -> Result<Vec<Report>> {
        let setting = self.setting;
        match &self.encoding {
            Encoding::Level(level) => Ok(level.encode(as_u8(setting, value)?)),
            Encoding::Inactive(inactive) => {
                let minutes = match value {
                    SettingValue::Choice(id) => id
                        .parse::<u8>()
                        .map_err(|_| invalid(setting, &format!("'{id}' is not a number of minutes")))?,
                    other => as_u8(setting, other)?,
                };
                inactive.encode_minutes(minutes)
            }
            Encoding::Toggle(toggle) => Ok(toggle.encode(as_bool(setting, value)?)),
            Encoding::Choice(choice) => choice.encode(setting, as_choice(setting, value)?),
            Encoding::Equalizer(eq) => Ok(eq.encode(as_gains(setting, value)?)),
            Encoding::Preset(preset) => preset.encode(as_choice(setting, value)?),
            Encoding::Lighting(style, part) => {
                match part {
                    LightingPart::Colors => {
                        let colors = as_colors(setting, value)?;
                        let (left, right) = match colors {
                            [one] => (*one, *one),
                            [left, right] => (*left, *right),
                            _ => return Err(invalid(setting, "expected 1 or 2 colors")),
                        };
                        lighting.left = left;
                        lighting.right = right;
                    }
                    LightingPart::Effect => {
                        let id = as_choice(setting, value)?;
                        lighting.effect = Nova3Effect::from_id(id)
                            .ok_or_else(|| invalid(setting, &format!("unknown effect '{id}'")))?;
                    }
                    LightingPart::Speed => lighting.speed = as_u8(setting, value)?,
                }
                Ok(lighting::render(*style, lighting))
            }
            Encoding::Save(packets) => Ok(packets.iter().map(Packet::report).collect()),
        }
    }
}

fn with_save(mut reports: Vec<Report>, save: SaveSequence) -> Vec<Report> {
    reports.extend(save.iter().map(Packet::report));
    reports
}

fn range(min: i64, max: i64, step: i64, unit: Option<&str>) -> SettingKind {
    SettingKind::Range {
        min,
        max,
        step,
        unit: unit.map(str::to_owned),
    }
}

fn inactive_kind(style: InactiveStyle) -> SettingKind {
    match style {
        InactiveStyle::Minutes { max } => range(0, i64::from(max), 1, Some("min")),
        InactiveStyle::Seconds { max_minutes } => range(0, i64::from(max_minutes), 1, Some("min")),
        InactiveStyle::TenMinuteSteps { max_minutes } => range(0, i64::from(max_minutes), 10, Some("min")),
        InactiveStyle::Choices(choices) => SettingKind::Choice {
            options: choices
                .iter()
                .map(|c| {
                    let label = if c.minutes == 0 {
                        "Never".to_owned()
                    } else {
                        format!("{} min", c.minutes)
                    };
                    ChoiceOption::new(c.minutes.to_string(), label)
                })
                .collect(),
        },
    }
}

fn lighting_kind(part: LightingPart) -> SettingKind {
    match part {
        LightingPart::Colors => SettingKind::ColorZones {
            zones: lighting::ZONES.iter().map(|z| (*z).to_owned()).collect(),
        },
        LightingPart::Effect => SettingKind::Choice {
            options: Nova3Effect::ALL
                .iter()
                .map(|(_, id, label)| ChoiceOption::new(*id, *label))
                .collect(),
        },
        LightingPart::Speed => range(
            i64::from(lighting::NOVA3_SPEED_MIN),
            i64::from(lighting::NOVA3_SPEED_MAX),
            1,
            None,
        ),
    }
}

fn invalid(setting: SettingId, why: &str) -> Error {
    Error::InvalidConfig(format!("{}: {why}", setting.id()))
}

fn as_u8(setting: SettingId, value: &SettingValue) -> Result<u8> {
    match value {
        SettingValue::Int(v) => u8::try_from(*v).map_err(|_| invalid(setting, &format!("{v} is out of range"))),
        other => Err(invalid(setting, &format!("expected an integer, got {other:?}"))),
    }
}

fn as_bool(setting: SettingId, value: &SettingValue) -> Result<bool> {
    match value {
        SettingValue::Bool(b) => Ok(*b),
        other => Err(invalid(setting, &format!("expected on/off, got {other:?}"))),
    }
}

fn as_choice(setting: SettingId, value: &SettingValue) -> Result<&str> {
    match value {
        SettingValue::Choice(id) => Ok(id),
        other => Err(invalid(setting, &format!("expected a choice, got {other:?}"))),
    }
}

fn as_gains(setting: SettingId, value: &SettingValue) -> Result<&[f32]> {
    match value {
        SettingValue::Gains(gains) => Ok(gains),
        other => Err(invalid(setting, &format!("expected band gains, got {other:?}"))),
    }
}

fn as_colors(setting: SettingId, value: &SettingValue) -> Result<&[Color]> {
    match value {
        SettingValue::Colors(colors) => Ok(colors),
        SettingValue::Color(color) => Ok(std::slice::from_ref(color)),
        other => Err(invalid(setting, &format!("expected colors, got {other:?}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::headsets::report::Frame;

    const CMD: Command = Command::new(Frame::NOVA, 0x00, 0x39);
    const SAVE: &[Packet] = &[Packet::new(Command::new(Frame::NOVA, 0x00, 0x09), &[])];

    #[test]
    fn level_styles_place_the_value() {
        let plain = Level {
            command: CMD,
            max: 3,
            style: LevelStyle::Plain,
            save: &[],
        };
        assert_eq!(&plain.encode(2)[0].bytes[..3], &[0x00, 0x39, 0x02]);

        let switched = Level {
            command: Command::new(Frame::LEGACY, 0x06, 0x35),
            max: 18,
            style: LevelStyle::Switched,
            save: SAVE,
        };
        let on = switched.encode(18);
        assert_eq!(&on[0].bytes[..5], &[0x06, 0x35, 0x01, 0x00, 0x12]);
        assert_eq!(&on[1].bytes[..2], &[0x00, 0x09]);
        let off = switched.encode(0);
        assert_eq!(&off[0].bytes[..3], &[0x06, 0x35, 0x00]);
        assert!(off[0].bytes[2..].iter().all(|b| *b == 0));

        let offset = Level {
            command: Command::new(Frame::LEGACY, 0x06, 0x00),
            max: 61,
            style: LevelStyle::Offset(0xc0),
            save: &[],
        };
        assert_eq!(offset.encode(0)[0].bytes[2], 0xc0);
        assert_eq!(offset.encode(61)[0].bytes[2], 0xfd);
    }

    #[test]
    fn inactive_styles() {
        let seconds = Inactive {
            command: Command::new(Frame::LEGACY, 0x04, 0x00),
            style: InactiveStyle::Seconds { max_minutes: 255 },
            save: &[],
        };
        assert_eq!(
            &seconds.encode_minutes(10).unwrap()[0].bytes[..4],
            &[0x04, 0x00, 0x02, 0x58]
        );

        let tens = Inactive {
            command: Command::new(Frame::LEGACY, 0x3c, 0xaa),
            style: InactiveStyle::TenMinuteSteps { max_minutes: 250 },
            save: &[],
        };
        assert_eq!(tens.encode_minutes(30).unwrap()[0].bytes[2], 3);

        const CHOICES: &[InactiveChoice] = &[
            InactiveChoice { minutes: 0, raw: 0 },
            InactiveChoice { minutes: 60, raw: 6 },
        ];
        let choices = Inactive {
            command: Command::new(Frame::LEGACY, 0x06, 0xc1),
            style: InactiveStyle::Choices(CHOICES),
            save: SAVE,
        };
        let reports = choices.encode_minutes(60).unwrap();
        assert_eq!(&reports[0].bytes[..3], &[0x06, 0xc1, 0x06]);
        assert_eq!(reports.len(), 2);
        assert!(choices.encode_minutes(7).is_err());
    }

    #[test]
    fn gain_tenths_matches_reference_arithmetic() {
        assert_eq!(gain_tenths_byte(0.0), 0x00);
        assert_eq!(gain_tenths_byte(12.0), 120);
        assert_eq!(gain_tenths_byte(-12.0), 0x88);
        assert_eq!(gain_tenths_byte(-1.0), 0xf6);
        assert_eq!(gain_tenths_byte(3.5), 35);
        assert_eq!(gain_tenths_byte(-1.5), 0xf1);
        // Below one step the cast truncates to zero, as the reference's does.
        assert_eq!(gain_tenths_byte(-0.05), 0x00);
        assert_eq!(gain_tenths_byte(0.09), 0x00);
    }

    #[test]
    fn linear_bands_truncate_like_a_uint8_cast() {
        let eq = Equalizer {
            command: Command::new(Frame::NOVA, 0x00, 0x33),
            bands_hz: &[32, 64, 125],
            min_db: -10.0,
            max_db: 10.0,
            step_db: 1.0,
            encoding: BandEncoding::Linear {
                baseline: 0x14,
                per_db: 1.0,
            },
            prelude: &[],
            save: &[],
        };
        assert_eq!(eq.band_bytes(&[3.5, -1.5, -10.0]), vec![23, 18, 10]);
    }

    #[test]
    fn descriptors_reflect_encoding() {
        let cap = Capability::new(
            SettingId::Sidetone,
            Encoding::Level(Level {
                command: CMD,
                max: 3,
                style: LevelStyle::Plain,
                save: SAVE,
            }),
        );
        let d = cap.descriptor();
        assert_eq!(d.id, "sidetone");
        assert!(d.persists_on_device);
        assert_eq!(d.verification, Verification::Reference);
        assert!(matches!(d.kind, SettingKind::Range { min: 0, max: 3, .. }));
        assert_eq!(cap.guess().descriptor().verification, Verification::Guess);
    }

    #[test]
    fn encode_rejects_wrong_value_shape() {
        let cap = Capability::new(
            SettingId::Sidetone,
            Encoding::Level(Level {
                command: CMD,
                max: 3,
                style: LevelStyle::Plain,
                save: &[],
            }),
        );
        let mut lighting = LightingState::default();
        assert!(cap.encode(&SettingValue::Bool(true), &mut lighting).is_err());
        assert!(cap.encode(&SettingValue::Int(300), &mut lighting).is_err());
    }

    #[test]
    fn lighting_capability_updates_state() {
        let cap = Capability::new(
            SettingId::LedColors,
            Encoding::Lighting(LightingStyle::Arctis5, LightingPart::Colors),
        );
        let mut lighting = LightingState::default();
        let reports = cap
            .encode(&SettingValue::Colors(vec![Color::RED]), &mut lighting)
            .unwrap();
        assert_eq!(lighting.left, Color::RED);
        assert_eq!(lighting.right, Color::RED);
        assert_eq!(reports.len(), 12);
    }
}
