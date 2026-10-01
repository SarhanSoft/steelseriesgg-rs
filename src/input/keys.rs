//! Input key naming: evdev key/button codes with canonical names and friendly aliases.
//!
//! An [`InputKey`] is a Linux evdev `EV_KEY` code. Codes are layout-independent: `KEY_A` is the
//! physical key in the "A" position of a US keyboard, whatever layout the desktop applies on top.
//! Names are the kernel's `input-event-codes.h` names (`KEY_CAPSLOCK`, `BTN_SIDE`); parsing also
//! accepts aliases (`capslock`, `ctrl`, `mouse4`, `a`, `-`) and `CODE_<n>` for unnamed codes.

use std::fmt;
use std::str::FromStr;

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::devices::key_mapping::KeyId;
use crate::error::{Error, Result};

/// Highest valid evdev key code (`KEY_MAX`).
pub const KEY_MAX: u16 = 0x2ff;

/// A keyboard key or mouse button, identified by its evdev `EV_KEY` code.
///
/// Serialized as its canonical evdev name (`"KEY_CAPSLOCK"`), or `"CODE_<n>"` for codes without
/// a name in this table. Deserialization accepts any name [`InputKey::parse`] accepts, or a bare
/// integer code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InputKey(u16);

impl InputKey {
    pub const KEY_ESC: InputKey = InputKey(1);
    pub const KEY_ENTER: InputKey = InputKey(28);
    pub const KEY_TAB: InputKey = InputKey(15);
    pub const KEY_SPACE: InputKey = InputKey(57);
    pub const KEY_LEFTCTRL: InputKey = InputKey(29);
    pub const KEY_RIGHTCTRL: InputKey = InputKey(97);
    pub const KEY_LEFTSHIFT: InputKey = InputKey(42);
    pub const KEY_RIGHTSHIFT: InputKey = InputKey(54);
    pub const KEY_LEFTALT: InputKey = InputKey(56);
    pub const KEY_RIGHTALT: InputKey = InputKey(100);
    pub const KEY_LEFTMETA: InputKey = InputKey(125);
    pub const KEY_RIGHTMETA: InputKey = InputKey(126);
    pub const BTN_LEFT: InputKey = InputKey(0x110);
    pub const BTN_TASK: InputKey = InputKey(0x117);

    /// Wrap a raw evdev code. Codes above [`KEY_MAX`] are not valid key codes; use
    /// [`InputKey::try_from_code`] where the code comes from untrusted input.
    pub const fn from_code(code: u16) -> Self {
        InputKey(code)
    }

    /// Wrap a raw evdev code, rejecting `0` (`KEY_RESERVED`) and codes above [`KEY_MAX`].
    pub fn try_from_code(code: u16) -> Result<Self> {
        if code == 0 || code > KEY_MAX {
            return Err(Error::InvalidConfig(format!(
                "key code {code} is outside the valid range 1..={KEY_MAX}"
            )));
        }
        Ok(InputKey(code))
    }

    /// The evdev code.
    pub const fn code(self) -> u16 {
        self.0
    }

    /// Canonical evdev name, if this code is in the name table.
    pub fn name(self) -> Option<&'static str> {
        KEY_NAMES
            .iter()
            .find(|(_, code)| *code == self.0)
            .map(|(name, _)| *name)
    }

    /// True for mouse buttons (`BTN_LEFT` ..= `BTN_TASK`).
    pub fn is_mouse_button(self) -> bool {
        (Self::BTN_LEFT.0..=Self::BTN_TASK.0).contains(&self.0)
    }

    /// True for the eight Ctrl/Shift/Alt/Meta keys.
    pub fn is_modifier(self) -> bool {
        matches!(
            self,
            Self::KEY_LEFTCTRL
                | Self::KEY_RIGHTCTRL
                | Self::KEY_LEFTSHIFT
                | Self::KEY_RIGHTSHIFT
                | Self::KEY_LEFTALT
                | Self::KEY_RIGHTALT
                | Self::KEY_LEFTMETA
                | Self::KEY_RIGHTMETA
        )
    }

    /// Every key in the name table, in code order.
    pub fn known_keys() -> impl Iterator<Item = InputKey> {
        KEY_NAMES.iter().map(|(_, code)| InputKey(*code))
    }

    /// Parse a key name.
    ///
    /// Accepted, case-insensitively and ignoring `_`, `-` and spaces:
    /// - evdev names: `KEY_CAPSLOCK`, `BTN_SIDE`
    /// - aliases: `ctrl`, `shift`, `alt`, `super`, `esc`, `return`, `pgup`, `mouse1`..`mouse8`,
    ///   `lmb`/`rmb`/`mmb`, `printscreen`, `num0`..`num9`, ...
    /// - evdev names without the prefix: `capslock`, `a`, `f13`, `kp5`, `side`
    /// - single punctuation characters of a US layout: `-` `=` `[` `]` `\` `;` `'` `` ` `` `,` `.` `/`
    /// - `CODE_<n>` with a decimal code for keys without a name
    pub fn parse(input: &str) -> Result<Self> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(Error::InvalidConfig("empty key name".to_string()));
        }
        if let Some(key) = punctuation_key(trimmed) {
            return Ok(key);
        }
        if let Some(rest) = strip_prefix_ignore_case(trimmed, "CODE_") {
            let code: u16 = rest
                .parse()
                .map_err(|_| Error::InvalidConfig(format!("invalid key code in {trimmed:?}")))?;
            return Self::try_from_code(code);
        }

        let norm = normalize(trimmed);
        if let Some(code) = KEY_NAMES.iter().find(|(name, _)| loose_eq(&norm, name)).map(|e| e.1) {
            return Ok(InputKey(code));
        }
        if let Some(code) = ALIASES.iter().find(|(alias, _)| norm == *alias).map(|e| e.1) {
            return Ok(InputKey(code));
        }
        // Prefix-less names; `KEY_` wins over `BTN_` so `left` is the arrow key, not the mouse.
        for prefix in ["KEY_", "BTN_"] {
            let found = KEY_NAMES
                .iter()
                .filter_map(|(name, code)| name.strip_prefix(prefix).map(|bare| (bare, *code)))
                .find(|(bare, _)| loose_eq(&norm, bare));
            if let Some((_, code)) = found {
                return Ok(InputKey(code));
            }
        }
        Err(Error::InvalidConfig(format!("unknown key name {trimmed:?}")))
    }

    /// The evdev key for a key of the per-key RGB layout ([`KeyId`]).
    ///
    /// Returns `None` for keys that have no standard evdev code (`SteelSeriesKey`,
    /// `VolumeWheel` are handled by keyboard firmware).
    pub fn from_key_id(id: KeyId) -> Option<Self> {
        let name = match id {
            KeyId::Escape => "KEY_ESC",
            KeyId::F1 => "KEY_F1",
            KeyId::F2 => "KEY_F2",
            KeyId::F3 => "KEY_F3",
            KeyId::F4 => "KEY_F4",
            KeyId::F5 => "KEY_F5",
            KeyId::F6 => "KEY_F6",
            KeyId::F7 => "KEY_F7",
            KeyId::F8 => "KEY_F8",
            KeyId::F9 => "KEY_F9",
            KeyId::F10 => "KEY_F10",
            KeyId::F11 => "KEY_F11",
            KeyId::F12 => "KEY_F12",
            KeyId::Backtick => "KEY_GRAVE",
            KeyId::Key1 => "KEY_1",
            KeyId::Key2 => "KEY_2",
            KeyId::Key3 => "KEY_3",
            KeyId::Key4 => "KEY_4",
            KeyId::Key5 => "KEY_5",
            KeyId::Key6 => "KEY_6",
            KeyId::Key7 => "KEY_7",
            KeyId::Key8 => "KEY_8",
            KeyId::Key9 => "KEY_9",
            KeyId::Key0 => "KEY_0",
            KeyId::Minus => "KEY_MINUS",
            KeyId::Equal => "KEY_EQUAL",
            KeyId::Backspace => "KEY_BACKSPACE",
            KeyId::Tab => "KEY_TAB",
            KeyId::Q => "KEY_Q",
            KeyId::W => "KEY_W",
            KeyId::E => "KEY_E",
            KeyId::R => "KEY_R",
            KeyId::T => "KEY_T",
            KeyId::Y => "KEY_Y",
            KeyId::U => "KEY_U",
            KeyId::I => "KEY_I",
            KeyId::O => "KEY_O",
            KeyId::P => "KEY_P",
            KeyId::LeftBracket => "KEY_LEFTBRACE",
            KeyId::RightBracket => "KEY_RIGHTBRACE",
            KeyId::Backslash => "KEY_BACKSLASH",
            KeyId::CapsLock => "KEY_CAPSLOCK",
            KeyId::A => "KEY_A",
            KeyId::S => "KEY_S",
            KeyId::D => "KEY_D",
            KeyId::F => "KEY_F",
            KeyId::G => "KEY_G",
            KeyId::H => "KEY_H",
            KeyId::J => "KEY_J",
            KeyId::K => "KEY_K",
            KeyId::L => "KEY_L",
            KeyId::Semicolon => "KEY_SEMICOLON",
            KeyId::Quote => "KEY_APOSTROPHE",
            KeyId::Enter => "KEY_ENTER",
            KeyId::LeftShift => "KEY_LEFTSHIFT",
            KeyId::Z => "KEY_Z",
            KeyId::X => "KEY_X",
            KeyId::C => "KEY_C",
            KeyId::V => "KEY_V",
            KeyId::B => "KEY_B",
            KeyId::N => "KEY_N",
            KeyId::M => "KEY_M",
            KeyId::Comma => "KEY_COMMA",
            KeyId::Period => "KEY_DOT",
            KeyId::Slash => "KEY_SLASH",
            KeyId::RightShift => "KEY_RIGHTSHIFT",
            KeyId::LeftCtrl => "KEY_LEFTCTRL",
            KeyId::LeftWin => "KEY_LEFTMETA",
            KeyId::LeftAlt => "KEY_LEFTALT",
            KeyId::Space => "KEY_SPACE",
            KeyId::RightAlt => "KEY_RIGHTALT",
            KeyId::RightWin => "KEY_RIGHTMETA",
            // The PC "context menu" key reports KEY_COMPOSE on Linux, not KEY_MENU.
            KeyId::Menu => "KEY_COMPOSE",
            KeyId::RightCtrl => "KEY_RIGHTCTRL",
            KeyId::ArrowUp => "KEY_UP",
            KeyId::ArrowDown => "KEY_DOWN",
            KeyId::ArrowLeft => "KEY_LEFT",
            KeyId::ArrowRight => "KEY_RIGHT",
            KeyId::Insert => "KEY_INSERT",
            KeyId::Delete => "KEY_DELETE",
            KeyId::Home => "KEY_HOME",
            KeyId::End => "KEY_END",
            KeyId::PageUp => "KEY_PAGEUP",
            KeyId::PageDown => "KEY_PAGEDOWN",
            KeyId::NumLock => "KEY_NUMLOCK",
            KeyId::NumSlash => "KEY_KPSLASH",
            KeyId::NumAsterisk => "KEY_KPASTERISK",
            KeyId::NumMinus => "KEY_KPMINUS",
            KeyId::Num7 => "KEY_KP7",
            KeyId::Num8 => "KEY_KP8",
            KeyId::Num9 => "KEY_KP9",
            KeyId::NumPlus => "KEY_KPPLUS",
            KeyId::Num4 => "KEY_KP4",
            KeyId::Num5 => "KEY_KP5",
            KeyId::Num6 => "KEY_KP6",
            KeyId::Num1 => "KEY_KP1",
            KeyId::Num2 => "KEY_KP2",
            KeyId::Num3 => "KEY_KP3",
            KeyId::NumEnter => "KEY_KPENTER",
            KeyId::Num0 => "KEY_KP0",
            KeyId::NumPeriod => "KEY_KPDOT",
            KeyId::PrintScreen => "KEY_SYSRQ",
            KeyId::ScrollLock => "KEY_SCROLLLOCK",
            KeyId::Pause => "KEY_PAUSE",
            // ISO "#" (HID 0x32) reports the same evdev code as "\" on ANSI boards.
            KeyId::NonUsHash => "KEY_BACKSLASH",
            KeyId::NonUsBackslash => "KEY_102ND",
            KeyId::JpRo => "KEY_RO",
            KeyId::JpKana => "KEY_KATAKANAHIRAGANA",
            KeyId::JpYen => "KEY_YEN",
            KeyId::JpHenkan => "KEY_HENKAN",
            KeyId::JpMuhenkan => "KEY_MUHENKAN",
            KeyId::MediaPlayPause => "KEY_PLAYPAUSE",
            KeyId::SteelSeriesKey | KeyId::VolumeWheel => return None,
        };
        KEY_NAMES
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, code)| InputKey(*code))
    }
}

impl fmt::Display for InputKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.name() {
            Some(name) => f.write_str(name),
            None => write!(f, "CODE_{}", self.0),
        }
    }
}

impl FromStr for InputKey {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::parse(s)
    }
}

impl Serialize for InputKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for InputKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        struct KeyVisitor;

        impl Visitor<'_> for KeyVisitor {
            type Value = InputKey;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a key name such as \"KEY_CAPSLOCK\", \"capslock\" or \"mouse4\", or a key code")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<InputKey, E> {
                InputKey::parse(v).map_err(E::custom)
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> std::result::Result<InputKey, E> {
                let code = u16::try_from(v).map_err(|_| E::custom(format!("key code {v} is out of range")))?;
                InputKey::try_from_code(code).map_err(E::custom)
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> std::result::Result<InputKey, E> {
                let code = u16::try_from(v).map_err(|_| E::custom(format!("key code {v} is out of range")))?;
                InputKey::try_from_code(code).map_err(E::custom)
            }
        }

        deserializer.deserialize_any(KeyVisitor)
    }
}

/// Media keys (GG's "Media" binding category).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaKey {
    PlayPause,
    #[serde(alias = "next_track")]
    Next,
    #[serde(alias = "previous_track", alias = "prev")]
    Previous,
    Stop,
    VolumeUp,
    VolumeDown,
    Mute,
}

impl MediaKey {
    /// The evdev key this media function sends.
    pub fn key(self) -> InputKey {
        InputKey(match self {
            MediaKey::PlayPause => 164,
            MediaKey::Next => 163,
            MediaKey::Previous => 165,
            MediaKey::Stop => 166,
            MediaKey::VolumeUp => 115,
            MediaKey::VolumeDown => 114,
            MediaKey::Mute => 113,
        })
    }
}

/// Mouse buttons, named after their evdev codes.
///
/// `side` (`mouse4`) and `extra` (`mouse5`) are the usual back/forward thumb buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseButton {
    #[serde(alias = "mouse1")]
    Left,
    #[serde(alias = "mouse2")]
    Right,
    #[serde(alias = "mouse3")]
    Middle,
    #[serde(alias = "mouse4")]
    Side,
    #[serde(alias = "mouse5")]
    Extra,
    #[serde(alias = "mouse6")]
    Forward,
    #[serde(alias = "mouse7")]
    Back,
    #[serde(alias = "mouse8")]
    Task,
}

impl MouseButton {
    /// The evdev `BTN_*` code of this button.
    pub fn key(self) -> InputKey {
        InputKey(match self {
            MouseButton::Left => 0x110,
            MouseButton::Right => 0x111,
            MouseButton::Middle => 0x112,
            MouseButton::Side => 0x113,
            MouseButton::Extra => 0x114,
            MouseButton::Forward => 0x115,
            MouseButton::Back => 0x116,
            MouseButton::Task => 0x117,
        })
    }
}

fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, '_' | '-' | ' '))
        .flat_map(char::to_lowercase)
        .collect()
}

/// `normalized` (already passed through [`normalize`]) equals `name` under the same rules.
fn loose_eq(normalized: &str, name: &str) -> bool {
    normalized.chars().eq(name
        .chars()
        .filter(|c| !matches!(c, '_' | '-' | ' '))
        .flat_map(char::to_lowercase))
}

fn strip_prefix_ignore_case<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let head = s.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix).then(|| &s[prefix.len()..])
}

fn punctuation_key(s: &str) -> Option<InputKey> {
    let code = match s {
        "-" => 12,
        "=" => 13,
        "[" => 26,
        "]" => 27,
        ";" => 39,
        "'" => 40,
        "`" => 41,
        "\\" => 43,
        "," => 51,
        "." => 52,
        "/" => 53,
        _ => return None,
    };
    Some(InputKey(code))
}

/// Friendly aliases, already normalized (lowercase, no `_`/`-`/space).
const ALIASES: &[(&str, u16)] = &[
    ("escape", 1),
    ("ctrl", 29),
    ("control", 29),
    ("lctrl", 29),
    ("leftcontrol", 29),
    ("rctrl", 97),
    ("rightcontrol", 97),
    ("shift", 42),
    ("lshift", 42),
    ("rshift", 54),
    ("alt", 56),
    ("lalt", 56),
    ("ralt", 100),
    ("altgr", 100),
    ("super", 125),
    ("meta", 125),
    ("win", 125),
    ("windows", 125),
    ("cmd", 125),
    ("lsuper", 125),
    ("lmeta", 125),
    ("lwin", 125),
    ("leftsuper", 125),
    ("leftwin", 125),
    ("rsuper", 126),
    ("rmeta", 126),
    ("rwin", 126),
    ("rightsuper", 126),
    ("rightwin", 126),
    ("return", 28),
    ("bksp", 14),
    ("del", 111),
    ("ins", 110),
    ("pgup", 104),
    ("pgdn", 109),
    ("pgdown", 109),
    ("caps", 58),
    ("numlk", 69),
    ("scrlk", 70),
    ("scroll", 70),
    ("printscreen", 99),
    ("prtsc", 99),
    ("print", 99),
    ("apps", 127),
    ("contextmenu", 127),
    ("backtick", 41),
    ("tilde", 41),
    ("quote", 40),
    ("lbracket", 26),
    ("leftbracket", 26),
    ("rbracket", 27),
    ("rightbracket", 27),
    ("period", 52),
    ("equals", 13),
    ("dash", 12),
    ("hyphen", 12),
    ("arrowup", 103),
    ("arrowdown", 108),
    ("arrowleft", 105),
    ("arrowright", 106),
    ("num0", 82),
    ("num1", 79),
    ("num2", 80),
    ("num3", 81),
    ("num4", 75),
    ("num5", 76),
    ("num6", 77),
    ("num7", 71),
    ("num8", 72),
    ("num9", 73),
    ("numenter", 96),
    ("numplus", 78),
    ("numminus", 74),
    ("numasterisk", 55),
    ("nummultiply", 55),
    ("numslash", 98),
    ("numdivide", 98),
    ("numperiod", 83),
    ("numdot", 83),
    ("play", 164),
    ("nexttrack", 163),
    ("next", 163),
    ("prevtrack", 165),
    ("previoustrack", 165),
    ("prev", 165),
    ("previous", 165),
    ("mediastop", 166),
    ("volup", 115),
    ("voldown", 114),
    // Pre-5.17 kernel name of KEY_ALL_APPLICATIONS.
    ("keydashboard", 204),
    ("dashboard", 204),
    ("mouse1", 0x110),
    ("lmb", 0x110),
    ("leftclick", 0x110),
    ("mouseleft", 0x110),
    ("mouse2", 0x111),
    ("rmb", 0x111),
    ("rightclick", 0x111),
    ("mouseright", 0x111),
    ("mouse3", 0x112),
    ("mmb", 0x112),
    ("middleclick", 0x112),
    ("mousemiddle", 0x112),
    ("mouse4", 0x113),
    ("mouseback", 0x113),
    ("mouse5", 0x114),
    ("mouseforward", 0x114),
    ("mouse6", 0x115),
    ("mouse7", 0x116),
    ("mouse8", 0x117),
];

/// evdev key names from `linux/input-event-codes.h`, in code order. Each code appears once.
const KEY_NAMES: &[(&str, u16)] = &[
    ("KEY_ESC", 1),
    ("KEY_1", 2),
    ("KEY_2", 3),
    ("KEY_3", 4),
    ("KEY_4", 5),
    ("KEY_5", 6),
    ("KEY_6", 7),
    ("KEY_7", 8),
    ("KEY_8", 9),
    ("KEY_9", 10),
    ("KEY_0", 11),
    ("KEY_MINUS", 12),
    ("KEY_EQUAL", 13),
    ("KEY_BACKSPACE", 14),
    ("KEY_TAB", 15),
    ("KEY_Q", 16),
    ("KEY_W", 17),
    ("KEY_E", 18),
    ("KEY_R", 19),
    ("KEY_T", 20),
    ("KEY_Y", 21),
    ("KEY_U", 22),
    ("KEY_I", 23),
    ("KEY_O", 24),
    ("KEY_P", 25),
    ("KEY_LEFTBRACE", 26),
    ("KEY_RIGHTBRACE", 27),
    ("KEY_ENTER", 28),
    ("KEY_LEFTCTRL", 29),
    ("KEY_A", 30),
    ("KEY_S", 31),
    ("KEY_D", 32),
    ("KEY_F", 33),
    ("KEY_G", 34),
    ("KEY_H", 35),
    ("KEY_J", 36),
    ("KEY_K", 37),
    ("KEY_L", 38),
    ("KEY_SEMICOLON", 39),
    ("KEY_APOSTROPHE", 40),
    ("KEY_GRAVE", 41),
    ("KEY_LEFTSHIFT", 42),
    ("KEY_BACKSLASH", 43),
    ("KEY_Z", 44),
    ("KEY_X", 45),
    ("KEY_C", 46),
    ("KEY_V", 47),
    ("KEY_B", 48),
    ("KEY_N", 49),
    ("KEY_M", 50),
    ("KEY_COMMA", 51),
    ("KEY_DOT", 52),
    ("KEY_SLASH", 53),
    ("KEY_RIGHTSHIFT", 54),
    ("KEY_KPASTERISK", 55),
    ("KEY_LEFTALT", 56),
    ("KEY_SPACE", 57),
    ("KEY_CAPSLOCK", 58),
    ("KEY_F1", 59),
    ("KEY_F2", 60),
    ("KEY_F3", 61),
    ("KEY_F4", 62),
    ("KEY_F5", 63),
    ("KEY_F6", 64),
    ("KEY_F7", 65),
    ("KEY_F8", 66),
    ("KEY_F9", 67),
    ("KEY_F10", 68),
    ("KEY_NUMLOCK", 69),
    ("KEY_SCROLLLOCK", 70),
    ("KEY_KP7", 71),
    ("KEY_KP8", 72),
    ("KEY_KP9", 73),
    ("KEY_KPMINUS", 74),
    ("KEY_KP4", 75),
    ("KEY_KP5", 76),
    ("KEY_KP6", 77),
    ("KEY_KPPLUS", 78),
    ("KEY_KP1", 79),
    ("KEY_KP2", 80),
    ("KEY_KP3", 81),
    ("KEY_KP0", 82),
    ("KEY_KPDOT", 83),
    ("KEY_ZENKAKUHANKAKU", 85),
    ("KEY_102ND", 86),
    ("KEY_F11", 87),
    ("KEY_F12", 88),
    ("KEY_RO", 89),
    ("KEY_KATAKANA", 90),
    ("KEY_HIRAGANA", 91),
    ("KEY_HENKAN", 92),
    ("KEY_KATAKANAHIRAGANA", 93),
    ("KEY_MUHENKAN", 94),
    ("KEY_KPJPCOMMA", 95),
    ("KEY_KPENTER", 96),
    ("KEY_RIGHTCTRL", 97),
    ("KEY_KPSLASH", 98),
    ("KEY_SYSRQ", 99),
    ("KEY_RIGHTALT", 100),
    ("KEY_LINEFEED", 101),
    ("KEY_HOME", 102),
    ("KEY_UP", 103),
    ("KEY_PAGEUP", 104),
    ("KEY_LEFT", 105),
    ("KEY_RIGHT", 106),
    ("KEY_END", 107),
    ("KEY_DOWN", 108),
    ("KEY_PAGEDOWN", 109),
    ("KEY_INSERT", 110),
    ("KEY_DELETE", 111),
    ("KEY_MACRO", 112),
    ("KEY_MUTE", 113),
    ("KEY_VOLUMEDOWN", 114),
    ("KEY_VOLUMEUP", 115),
    ("KEY_POWER", 116),
    ("KEY_KPEQUAL", 117),
    ("KEY_KPPLUSMINUS", 118),
    ("KEY_PAUSE", 119),
    ("KEY_SCALE", 120),
    ("KEY_KPCOMMA", 121),
    ("KEY_HANGEUL", 122),
    ("KEY_HANJA", 123),
    ("KEY_YEN", 124),
    ("KEY_LEFTMETA", 125),
    ("KEY_RIGHTMETA", 126),
    ("KEY_COMPOSE", 127),
    ("KEY_STOP", 128),
    ("KEY_AGAIN", 129),
    ("KEY_PROPS", 130),
    ("KEY_UNDO", 131),
    ("KEY_FRONT", 132),
    ("KEY_COPY", 133),
    ("KEY_OPEN", 134),
    ("KEY_PASTE", 135),
    ("KEY_FIND", 136),
    ("KEY_CUT", 137),
    ("KEY_HELP", 138),
    ("KEY_MENU", 139),
    ("KEY_CALC", 140),
    ("KEY_SETUP", 141),
    ("KEY_SLEEP", 142),
    ("KEY_WAKEUP", 143),
    ("KEY_FILE", 144),
    ("KEY_SENDFILE", 145),
    ("KEY_DELETEFILE", 146),
    ("KEY_XFER", 147),
    ("KEY_PROG1", 148),
    ("KEY_PROG2", 149),
    ("KEY_WWW", 150),
    ("KEY_MSDOS", 151),
    ("KEY_COFFEE", 152),
    ("KEY_ROTATE_DISPLAY", 153),
    ("KEY_CYCLEWINDOWS", 154),
    ("KEY_MAIL", 155),
    ("KEY_BOOKMARKS", 156),
    ("KEY_COMPUTER", 157),
    ("KEY_BACK", 158),
    ("KEY_FORWARD", 159),
    ("KEY_CLOSECD", 160),
    ("KEY_EJECTCD", 161),
    ("KEY_EJECTCLOSECD", 162),
    ("KEY_NEXTSONG", 163),
    ("KEY_PLAYPAUSE", 164),
    ("KEY_PREVIOUSSONG", 165),
    ("KEY_STOPCD", 166),
    ("KEY_RECORD", 167),
    ("KEY_REWIND", 168),
    ("KEY_PHONE", 169),
    ("KEY_ISO", 170),
    ("KEY_CONFIG", 171),
    ("KEY_HOMEPAGE", 172),
    ("KEY_REFRESH", 173),
    ("KEY_EXIT", 174),
    ("KEY_MOVE", 175),
    ("KEY_EDIT", 176),
    ("KEY_SCROLLUP", 177),
    ("KEY_SCROLLDOWN", 178),
    ("KEY_KPLEFTPAREN", 179),
    ("KEY_KPRIGHTPAREN", 180),
    ("KEY_NEW", 181),
    ("KEY_REDO", 182),
    ("KEY_F13", 183),
    ("KEY_F14", 184),
    ("KEY_F15", 185),
    ("KEY_F16", 186),
    ("KEY_F17", 187),
    ("KEY_F18", 188),
    ("KEY_F19", 189),
    ("KEY_F20", 190),
    ("KEY_F21", 191),
    ("KEY_F22", 192),
    ("KEY_F23", 193),
    ("KEY_F24", 194),
    ("KEY_PLAYCD", 200),
    ("KEY_PAUSECD", 201),
    ("KEY_PROG3", 202),
    ("KEY_PROG4", 203),
    ("KEY_ALL_APPLICATIONS", 204),
    ("KEY_SUSPEND", 205),
    ("KEY_CLOSE", 206),
    ("KEY_PLAY", 207),
    ("KEY_FASTFORWARD", 208),
    ("KEY_BASSBOOST", 209),
    ("KEY_PRINT", 210),
    ("KEY_HP", 211),
    ("KEY_CAMERA", 212),
    ("KEY_SOUND", 213),
    ("KEY_QUESTION", 214),
    ("KEY_EMAIL", 215),
    ("KEY_CHAT", 216),
    ("KEY_SEARCH", 217),
    ("KEY_CONNECT", 218),
    ("KEY_FINANCE", 219),
    ("KEY_SPORT", 220),
    ("KEY_SHOP", 221),
    ("KEY_ALTERASE", 222),
    ("KEY_CANCEL", 223),
    ("KEY_BRIGHTNESSDOWN", 224),
    ("KEY_BRIGHTNESSUP", 225),
    ("KEY_MEDIA", 226),
    ("KEY_SWITCHVIDEOMODE", 227),
    ("KEY_KBDILLUMTOGGLE", 228),
    ("KEY_KBDILLUMDOWN", 229),
    ("KEY_KBDILLUMUP", 230),
    ("KEY_SEND", 231),
    ("KEY_REPLY", 232),
    ("KEY_FORWARDMAIL", 233),
    ("KEY_SAVE", 234),
    ("KEY_DOCUMENTS", 235),
    ("KEY_BATTERY", 236),
    ("KEY_BLUETOOTH", 237),
    ("KEY_WLAN", 238),
    ("KEY_UWB", 239),
    ("KEY_UNKNOWN", 240),
    ("KEY_VIDEO_NEXT", 241),
    ("KEY_VIDEO_PREV", 242),
    ("KEY_BRIGHTNESS_CYCLE", 243),
    ("KEY_BRIGHTNESS_AUTO", 244),
    ("KEY_DISPLAY_OFF", 245),
    ("KEY_WWAN", 246),
    ("KEY_RFKILL", 247),
    ("KEY_MICMUTE", 248),
    ("BTN_LEFT", 0x110),
    ("BTN_RIGHT", 0x111),
    ("BTN_MIDDLE", 0x112),
    ("BTN_SIDE", 0x113),
    ("BTN_EXTRA", 0x114),
    ("BTN_FORWARD", 0x115),
    ("BTN_BACK", 0x116),
    ("BTN_TASK", 0x117),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn key(name: &str) -> InputKey {
        InputKey::parse(name).unwrap()
    }

    #[test]
    fn canonical_names_parse_and_display() {
        assert_eq!(key("KEY_CAPSLOCK").code(), 58);
        assert_eq!(key("BTN_SIDE").code(), 0x113);
        assert_eq!(key("KEY_CAPSLOCK").to_string(), "KEY_CAPSLOCK");
        for k in InputKey::known_keys() {
            assert_eq!(InputKey::parse(&k.to_string()).unwrap(), k, "round trip of {k}");
        }
    }

    #[test]
    fn name_table_has_unique_codes_in_order() {
        for pair in KEY_NAMES.windows(2) {
            assert!(pair[0].1 < pair[1].1, "{} / {} out of order", pair[0].0, pair[1].0);
        }
    }

    #[test]
    fn aliases_resolve() {
        assert_eq!(key("capslock"), key("KEY_CAPSLOCK"));
        assert_eq!(key("Caps Lock"), key("KEY_CAPSLOCK"));
        assert_eq!(key("caps"), key("KEY_CAPSLOCK"));
        assert_eq!(key("mouse4"), key("BTN_SIDE"));
        assert_eq!(key("mouse5"), key("BTN_EXTRA"));
        assert_eq!(key("lmb"), InputKey::BTN_LEFT);
        assert_eq!(key("ctrl"), InputKey::KEY_LEFTCTRL);
        assert_eq!(key("rctrl"), InputKey::KEY_RIGHTCTRL);
        assert_eq!(key("super"), InputKey::KEY_LEFTMETA);
        assert_eq!(key("a"), key("KEY_A"));
        assert_eq!(key("F13"), key("KEY_F13"));
        assert_eq!(key("esc"), InputKey::KEY_ESC);
        assert_eq!(key("escape"), InputKey::KEY_ESC);
        assert_eq!(key("-"), key("KEY_MINUS"));
        assert_eq!(key("/"), key("KEY_SLASH"));
        assert_eq!(key("num5"), key("KEY_KP5"));
        assert_eq!(key("printscreen"), key("KEY_SYSRQ"));
        assert_eq!(key("key_leftshift"), InputKey::KEY_LEFTSHIFT);
        assert_eq!(key("side"), key("BTN_SIDE"));
    }

    #[test]
    fn key_prefix_wins_over_button_prefix() {
        assert_eq!(key("left"), key("KEY_LEFT"));
        assert_eq!(key("forward"), key("KEY_FORWARD"));
        assert_eq!(key("BTN_FORWARD").code(), 0x115);
    }

    #[test]
    fn numeric_codes() {
        assert_eq!(key("CODE_58"), key("KEY_CAPSLOCK"));
        assert_eq!(InputKey::from_code(0x2b0).to_string(), "CODE_688");
        assert_eq!(key("CODE_688").code(), 0x2b0);
        assert!(InputKey::parse("CODE_0").is_err());
        assert!(InputKey::parse("CODE_4000").is_err());
        assert!(InputKey::parse("CODE_x").is_err());
    }

    #[test]
    fn unknown_names_are_errors() {
        assert!(InputKey::parse("").is_err());
        assert!(InputKey::parse("KEY_NOPE").is_err());
        assert!(InputKey::parse("mouse9").is_err());
    }

    #[test]
    fn serde_uses_canonical_names_and_accepts_aliases() {
        assert_eq!(serde_json::to_string(&key("capslock")).unwrap(), "\"KEY_CAPSLOCK\"");
        let k: InputKey = serde_json::from_str("\"mouse4\"").unwrap();
        assert_eq!(k, key("BTN_SIDE"));
        let k: InputKey = serde_json::from_str("58").unwrap();
        assert_eq!(k, key("KEY_CAPSLOCK"));
        assert!(serde_json::from_str::<InputKey>("\"KEY_NOPE\"").is_err());
        assert!(serde_json::from_str::<InputKey>("70000").is_err());
    }

    #[test]
    fn media_and_mouse_button_codes() {
        assert_eq!(MediaKey::PlayPause.key(), key("KEY_PLAYPAUSE"));
        assert_eq!(MediaKey::Next.key(), key("KEY_NEXTSONG"));
        assert_eq!(MediaKey::Previous.key(), key("KEY_PREVIOUSSONG"));
        assert_eq!(MediaKey::Stop.key(), key("KEY_STOPCD"));
        assert_eq!(MediaKey::VolumeUp.key(), key("KEY_VOLUMEUP"));
        assert_eq!(MediaKey::VolumeDown.key(), key("KEY_VOLUMEDOWN"));
        assert_eq!(MediaKey::Mute.key(), key("KEY_MUTE"));
        assert_eq!(MouseButton::Side.key(), key("mouse4"));
        assert_eq!(MouseButton::Task.key(), InputKey::BTN_TASK);
        assert!(MouseButton::Left.key().is_mouse_button());
        assert!(!InputKey::KEY_ESC.is_mouse_button());
        let b: MouseButton = serde_json::from_str("\"mouse4\"").unwrap();
        assert_eq!(b, MouseButton::Side);
    }

    #[test]
    fn key_id_mapping() {
        assert_eq!(InputKey::from_key_id(KeyId::CapsLock), Some(key("KEY_CAPSLOCK")));
        assert_eq!(InputKey::from_key_id(KeyId::Menu), Some(key("KEY_COMPOSE")));
        assert_eq!(InputKey::from_key_id(KeyId::Num0), Some(key("KEY_KP0")));
        assert_eq!(InputKey::from_key_id(KeyId::LeftWin), Some(InputKey::KEY_LEFTMETA));
        assert_eq!(InputKey::from_key_id(KeyId::SteelSeriesKey), None);
        assert_eq!(InputKey::from_key_id(KeyId::VolumeWheel), None);
        assert!(InputKey::from_key_id(KeyId::NumPeriod).is_some());
        assert!(InputKey::from_key_id(KeyId::ArrowRight).is_some());
    }

    #[test]
    fn modifiers() {
        assert!(InputKey::KEY_LEFTCTRL.is_modifier());
        assert!(InputKey::KEY_RIGHTMETA.is_modifier());
        assert!(!InputKey::KEY_ESC.is_modifier());
    }
}
