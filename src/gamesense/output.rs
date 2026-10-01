//! Output model of the GameSense server.
//!
//! The server never touches hardware. Every effect a game asks for is resolved into a
//! [`GameSenseOutput`] and published on a `tokio::sync::broadcast` channel
//! (see [`GameSenseServer::subscribe`](super::GameSenseServer::subscribe)). Device code
//! subscribes and decides how to show it.

use std::fmt;
use std::time::Duration;

use crate::devices::key_mapping::KeyId;
use crate::rgb::Color;

/// Device category named by the `device-type` key of a handler.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DeviceType {
    /// `keyboard`: any supported keyboard.
    Keyboard,
    /// `mouse`: any supported mouse.
    Mouse,
    /// `headset`: any supported headset.
    Headset,
    /// `indicator`: a simple single-LED indicator device.
    Indicator,
    /// `rgb-N-zone`: a device with exactly N lighting zones.
    RgbZoned(u16),
    /// `rgb-zoned-device`: any device with a fixed number of lighting zones.
    RgbZonedDevice,
    /// `rgb-per-key-zones`: a keyboard with one lighting zone per key.
    RgbPerKeyZones,
    /// `screened` (no size) or `screened-WxH`.
    Screened(Option<ScreenSize>),
    /// `tactile`: a device with a vibration motor.
    Tactile,
}

/// Highest N accepted in `rgb-N-zone`.
const MAX_RGB_ZONES: u16 = 1024;
/// Largest screen side accepted in `screened-WxH`.
const MAX_SCREEN_SIDE: u16 = 4096;

impl DeviceType {
    /// Parse an SDK `device-type` string. Returns `None` for unknown types.
    pub fn parse(raw: &str) -> Option<Self> {
        let lower = raw.trim().to_ascii_lowercase();
        let parsed = match lower.as_str() {
            "keyboard" => Self::Keyboard,
            "mouse" => Self::Mouse,
            "headset" => Self::Headset,
            "indicator" => Self::Indicator,
            "rgb-zoned-device" => Self::RgbZonedDevice,
            "rgb-per-key-zones" => Self::RgbPerKeyZones,
            "tactile" => Self::Tactile,
            "screened" => Self::Screened(None),
            other => {
                if let Some(count) = other.strip_prefix("rgb-").and_then(|s| s.strip_suffix("-zone")) {
                    let zones: u16 = count.parse().ok()?;
                    if zones == 0 || zones > MAX_RGB_ZONES {
                        return None;
                    }
                    Self::RgbZoned(zones)
                } else {
                    let size = other.strip_prefix("screened-")?;
                    Self::Screened(Some(ScreenSize::parse(size)?))
                }
            }
        };
        Some(parsed)
    }

    /// True for device types whose handlers drive LEDs.
    pub fn is_lighting(&self) -> bool {
        !matches!(self, Self::Screened(_) | Self::Tactile)
    }

    /// True for device types whose zones are keyboard keys.
    pub fn is_keyboard(&self) -> bool {
        matches!(self, Self::Keyboard | Self::RgbPerKeyZones)
    }
}

impl fmt::Display for DeviceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Keyboard => f.write_str("keyboard"),
            Self::Mouse => f.write_str("mouse"),
            Self::Headset => f.write_str("headset"),
            Self::Indicator => f.write_str("indicator"),
            Self::RgbZoned(n) => write!(f, "rgb-{n}-zone"),
            Self::RgbZonedDevice => f.write_str("rgb-zoned-device"),
            Self::RgbPerKeyZones => f.write_str("rgb-per-key-zones"),
            Self::Screened(None) => f.write_str("screened"),
            Self::Screened(Some(size)) => write!(f, "screened-{}x{}", size.width, size.height),
            Self::Tactile => f.write_str("tactile"),
        }
    }
}

/// Pixel size of an OLED/LCD screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ScreenSize {
    pub width: u16,
    pub height: u16,
}

impl ScreenSize {
    /// Create a screen size.
    pub const fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }

    /// Parse `WxH` (for example `128x40`).
    pub fn parse(raw: &str) -> Option<Self> {
        let (w, h) = raw.split_once(['x', 'X'])?;
        let width: u16 = w.parse().ok()?;
        let height: u16 = h.parse().ok()?;
        if width == 0 || height == 0 || width > MAX_SCREEN_SIDE || height > MAX_SCREEN_SIDE {
            return None;
        }
        Some(Self { width, height })
    }

    /// Bytes in a 1-bit-per-pixel bitmap of this size: `ceil(width * height / 8)`.
    pub fn bitmap_len(&self) -> usize {
        (usize::from(self.width) * usize::from(self.height)).div_ceil(8)
    }
}

/// Which LEDs a lighting output applies to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LightTarget {
    /// Every LED of the device (`all`).
    AllZones,
    /// A numbered zone, 0-based (`one` is `Zone(0)`).
    Zone(usize),
    /// A named zone with no key-level meaning, such as `logo`, `wheel`, `earcups` or `macro-keys`.
    NamedZone(String),
    /// Keyboard keys, in the order the SDK defines for the zone.
    Keys(Vec<KeyId>),
    /// Keys given as USB HID usage codes: `custom-zone-keys`, or a named zone that contains a key
    /// with no [`KeyId`] (for example `nav-cluster`, which includes Print Screen).
    HidCodes(Vec<u8>),
}

/// Repetition of an effect: LED flashing or tactile pattern repeats.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rate {
    /// On/off cycles (or pattern repeats) per second. Always greater than zero.
    pub frequency_hz: f32,
    /// Stop after this many cycles. `None` repeats until the next update.
    pub repeat_limit: Option<u32>,
}

/// Flash settings of a lighting output.
pub type Flash = Rate;

/// One row of a text screen frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScreenLine {
    /// A line of text. `wrap` is the number of extra screen lines the text may wrap onto.
    Text { text: String, bold: bool, wrap: u32 },
    /// A progress bar filled to `percent` (0-100).
    ProgressBar { percent: u8 },
}

/// What a screen frame shows. Text is not rendered to pixels here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScreenContent {
    /// Text and progress-bar rows, top to bottom, with an optional icon on the left.
    Lines {
        lines: Vec<ScreenLine>,
        icon_id: Option<u32>,
    },
    /// Raw 1-bit-per-pixel image, rows packed MSB first, origin top-left.
    /// Length is `ceil(width * height / 8)` for the output's `size`.
    Bitmap(Vec<u8>),
}

/// One step of a vibration pattern.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TactileStep {
    /// A firmware effect such as `ti_predefined_strongclick_100`.
    Predefined { name: String, delay_ms: Option<u32> },
    /// The motor runs for `length_ms`.
    Custom { length_ms: u32, delay_ms: Option<u32> },
}

/// An effect requested by a game, resolved and ready for device code.
#[derive(Clone, Debug, PartialEq)]
pub enum GameSenseOutput {
    /// Set `target` to `color`. Stays until another output for the same LEDs or a `Clear`.
    ///
    /// `percent` and `count` modes produce several `Lighting` outputs in a row for one event:
    /// the lit keys, the partly lit key (dimmed color) and the unlit keys (black).
    Lighting {
        game: String,
        event: String,
        device_type: DeviceType,
        target: LightTarget,
        color: Color,
        flash: Option<Flash>,
        /// Always `None` today: lighting persists until replaced or cleared.
        duration: Option<Duration>,
    },
    /// Full-keyboard `bitmap` / `partial-bitmap` frame: 132 colors on a 22x6 grid, row by row
    /// from the top-left. Keys in `excluded` must keep their current color.
    Bitmap {
        game: String,
        event: String,
        device_type: DeviceType,
        colors: Vec<Color>,
        excluded: Vec<LightTarget>,
    },
    /// Show `content` on a screen. `duration: None` keeps it until the next screen output;
    /// otherwise return to the background image after `duration` unless another frame arrives.
    Screen {
        game: String,
        event: String,
        device_type: DeviceType,
        zone: String,
        /// Screen size this content is for; `None` for text on a generic `screened` handler.
        size: Option<ScreenSize>,
        content: ScreenContent,
        duration: Option<Duration>,
        priority: i32,
    },
    /// Play a vibration pattern, repeated per `rate`.
    Tactile {
        game: String,
        event: String,
        device_type: DeviceType,
        zone: String,
        pattern: Vec<TactileStep>,
        rate: Option<Rate>,
    },
    /// The game stopped or timed out: drop every output it produced and restore defaults.
    Clear { game: String },
}

impl GameSenseOutput {
    /// Name of the game that produced this output.
    pub fn game(&self) -> &str {
        match self {
            Self::Lighting { game, .. }
            | Self::Bitmap { game, .. }
            | Self::Screen { game, .. }
            | Self::Tactile { game, .. }
            | Self::Clear { game } => game,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_type_round_trip() {
        for raw in [
            "keyboard",
            "mouse",
            "headset",
            "indicator",
            "rgb-1-zone",
            "rgb-8-zone",
            "rgb-12-zone",
            "rgb-15-zone",
            "rgb-17-zone",
            "rgb-24-zone",
            "rgb-103-zone",
            "rgb-zoned-device",
            "rgb-per-key-zones",
            "screened",
            "screened-128x40",
            "tactile",
        ] {
            let parsed = DeviceType::parse(raw).unwrap_or_else(|| panic!("{raw} should parse"));
            assert_eq!(parsed.to_string(), raw);
        }
        assert_eq!(DeviceType::parse("RGB-5-ZONE"), Some(DeviceType::RgbZoned(5)));
        assert_eq!(
            DeviceType::parse("screened-128x36"),
            Some(DeviceType::Screened(Some(ScreenSize::new(128, 36))))
        );
    }

    #[test]
    fn device_type_rejects_unknown() {
        for raw in [
            "",
            "toaster",
            "rgb-0-zone",
            "rgb-x-zone",
            "screened-0x40",
            "screened-128",
            "rgb-zone",
        ] {
            assert_eq!(DeviceType::parse(raw), None, "{raw}");
        }
    }

    #[test]
    fn bitmap_len_rounds_up() {
        assert_eq!(ScreenSize::new(128, 40).bitmap_len(), 640);
        assert_eq!(ScreenSize::new(128, 36).bitmap_len(), 576);
        assert_eq!(ScreenSize::new(3, 3).bitmap_len(), 2);
    }
}
