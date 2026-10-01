//! Key-to-address mapping for SteelSeries keyboards.
//!
//! This module provides accurate key matrix mapping for per-key RGB control,
//! supporting different Apex Pro variants and layouts.

use crate::devices::product_ids;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Physical key identifier for SteelSeries keyboards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyId {
    // Function row
    Escape,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,

    // Number row
    Backtick,
    Key1,
    Key2,
    Key3,
    Key4,
    Key5,
    Key6,
    Key7,
    Key8,
    Key9,
    Key0,
    Minus,
    Equal,
    Backspace,

    // First letter row
    Tab,
    Q,
    W,
    E,
    R,
    T,
    Y,
    U,
    I,
    O,
    P,
    LeftBracket,
    RightBracket,
    Backslash,

    // Second letter row (home row)
    CapsLock,
    A,
    S,
    D,
    F,
    G,
    H,
    J,
    K,
    L,
    Semicolon,
    Quote,
    Enter,

    // Third letter row
    LeftShift,
    Z,
    X,
    C,
    V,
    B,
    N,
    M,
    Comma,
    Period,
    Slash,
    RightShift,

    // Bottom row
    LeftCtrl,
    LeftWin,
    LeftAlt,
    Space,
    RightAlt,
    RightWin,
    Menu,
    RightCtrl,

    // Arrow cluster
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,

    // Navigation cluster (full-size only)
    Insert,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,

    // Number pad (full-size only)
    NumLock,
    NumSlash,
    NumAsterisk,
    NumMinus,
    Num7,
    Num8,
    Num9,
    NumPlus,
    Num4,
    Num5,
    Num6,
    Num1,
    Num2,
    Num3,
    NumEnter,
    Num0,
    NumPeriod,

    // SteelSeries specific keys
    SteelSeriesKey, // The SteelSeries logo key
    VolumeWheel,    // Volume wheel (if applicable)

    // Further LEDs of the OpenRGB Apex LED table (`led_names`, SteelSeriesApexRegions.h)
    PrintScreen,
    ScrollLock,
    Pause,
    /// ISO `#` key next to Enter (HID 0x32).
    NonUsHash,
    /// ISO `\` key next to left Shift (HID 0x64).
    NonUsBackslash,
    /// Japanese layout keys (HID 0x87-0x8B).
    JpRo,
    JpKana,
    JpYen,
    JpHenkan,
    JpMuhenkan,
    /// Media key LED of the TKL boards (HID 0xFB), where full-size boards have Pause.
    MediaPlayPause,
}

impl fmt::Display for KeyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Function row
            KeyId::Escape => write!(f, "ESC"),
            KeyId::F1 => write!(f, "F1"),
            KeyId::F2 => write!(f, "F2"),
            KeyId::F3 => write!(f, "F3"),
            KeyId::F4 => write!(f, "F4"),
            KeyId::F5 => write!(f, "F5"),
            KeyId::F6 => write!(f, "F6"),
            KeyId::F7 => write!(f, "F7"),
            KeyId::F8 => write!(f, "F8"),
            KeyId::F9 => write!(f, "F9"),
            KeyId::F10 => write!(f, "F10"),
            KeyId::F11 => write!(f, "F11"),
            KeyId::F12 => write!(f, "F12"),

            // Number row
            KeyId::Backtick => write!(f, "`"),
            KeyId::Key1 => write!(f, "1"),
            KeyId::Key2 => write!(f, "2"),
            KeyId::Key3 => write!(f, "3"),
            KeyId::Key4 => write!(f, "4"),
            KeyId::Key5 => write!(f, "5"),
            KeyId::Key6 => write!(f, "6"),
            KeyId::Key7 => write!(f, "7"),
            KeyId::Key8 => write!(f, "8"),
            KeyId::Key9 => write!(f, "9"),
            KeyId::Key0 => write!(f, "0"),
            KeyId::Minus => write!(f, "-"),
            KeyId::Equal => write!(f, "="),
            KeyId::Backspace => write!(f, "BKSP"),

            // Letter rows
            KeyId::Tab => write!(f, "TAB"),
            KeyId::Q => write!(f, "Q"),
            KeyId::W => write!(f, "W"),
            KeyId::E => write!(f, "E"),
            KeyId::R => write!(f, "R"),
            KeyId::T => write!(f, "T"),
            KeyId::Y => write!(f, "Y"),
            KeyId::U => write!(f, "U"),
            KeyId::I => write!(f, "I"),
            KeyId::O => write!(f, "O"),
            KeyId::P => write!(f, "P"),
            KeyId::LeftBracket => write!(f, "["),
            KeyId::RightBracket => write!(f, "]"),
            KeyId::Backslash => write!(f, "\\"),

            KeyId::CapsLock => write!(f, "CAPS"),
            KeyId::A => write!(f, "A"),
            KeyId::S => write!(f, "S"),
            KeyId::D => write!(f, "D"),
            KeyId::F => write!(f, "F"),
            KeyId::G => write!(f, "G"),
            KeyId::H => write!(f, "H"),
            KeyId::J => write!(f, "J"),
            KeyId::K => write!(f, "K"),
            KeyId::L => write!(f, "L"),
            KeyId::Semicolon => write!(f, ";"),
            KeyId::Quote => write!(f, "'"),
            KeyId::Enter => write!(f, "ENTER"),

            KeyId::LeftShift => write!(f, "LSHIFT"),
            KeyId::Z => write!(f, "Z"),
            KeyId::X => write!(f, "X"),
            KeyId::C => write!(f, "C"),
            KeyId::V => write!(f, "V"),
            KeyId::B => write!(f, "B"),
            KeyId::N => write!(f, "N"),
            KeyId::M => write!(f, "M"),
            KeyId::Comma => write!(f, ","),
            KeyId::Period => write!(f, "."),
            KeyId::Slash => write!(f, "/"),
            KeyId::RightShift => write!(f, "RSHIFT"),

            // Bottom row
            KeyId::LeftCtrl => write!(f, "LCTRL"),
            KeyId::LeftWin => write!(f, "LWIN"),
            KeyId::LeftAlt => write!(f, "LALT"),
            KeyId::Space => write!(f, "SPACE"),
            KeyId::RightAlt => write!(f, "RALT"),
            KeyId::RightWin => write!(f, "RWIN"),
            KeyId::Menu => write!(f, "MENU"),
            KeyId::RightCtrl => write!(f, "RCTRL"),

            // Arrows
            KeyId::ArrowUp => write!(f, "UP"),
            KeyId::ArrowDown => write!(f, "DOWN"),
            KeyId::ArrowLeft => write!(f, "LEFT"),
            KeyId::ArrowRight => write!(f, "RIGHT"),

            // Navigation
            KeyId::Insert => write!(f, "INS"),
            KeyId::Delete => write!(f, "DEL"),
            KeyId::Home => write!(f, "HOME"),
            KeyId::End => write!(f, "END"),
            KeyId::PageUp => write!(f, "PGUP"),
            KeyId::PageDown => write!(f, "PGDN"),

            // Numpad
            KeyId::NumLock => write!(f, "NUMLK"),
            KeyId::NumSlash => write!(f, "NUM/"),
            KeyId::NumAsterisk => write!(f, "NUM*"),
            KeyId::NumMinus => write!(f, "NUM-"),
            KeyId::Num7 => write!(f, "NUM7"),
            KeyId::Num8 => write!(f, "NUM8"),
            KeyId::Num9 => write!(f, "NUM9"),
            KeyId::NumPlus => write!(f, "NUM+"),
            KeyId::Num4 => write!(f, "NUM4"),
            KeyId::Num5 => write!(f, "NUM5"),
            KeyId::Num6 => write!(f, "NUM6"),
            KeyId::Num1 => write!(f, "NUM1"),
            KeyId::Num2 => write!(f, "NUM2"),
            KeyId::Num3 => write!(f, "NUM3"),
            KeyId::NumEnter => write!(f, "NUMENTER"),
            KeyId::Num0 => write!(f, "NUM0"),
            KeyId::NumPeriod => write!(f, "NUM."),

            // Special
            KeyId::SteelSeriesKey => write!(f, "SS"),
            KeyId::VolumeWheel => write!(f, "VOL"),

            KeyId::PrintScreen => write!(f, "PRTSC"),
            KeyId::ScrollLock => write!(f, "SCRLK"),
            KeyId::Pause => write!(f, "PAUSE"),
            KeyId::NonUsHash => write!(f, "ISO#"),
            KeyId::NonUsBackslash => write!(f, "ISO\\"),
            KeyId::JpRo => write!(f, "RO"),
            KeyId::JpKana => write!(f, "KANA"),
            KeyId::JpYen => write!(f, "YEN"),
            KeyId::JpHenkan => write!(f, "HENKAN"),
            KeyId::JpMuhenkan => write!(f, "MUHENKAN"),
            KeyId::MediaPlayPause => write!(f, "MEDIA"),
        }
    }
}

/// [EXPERIMENTAL] The Apex per-key LED table: `(key, HID usage)` in LED-index order.
///
/// Reference: OpenRGB `SteelSeriesApexController.cpp` (`keys[]`, the HID usage sent for each
/// LED) and `SteelSeriesApexRegions.h` (`led_names[]`, the key at each LED index). Both lists
/// have 112 entries in the same order. Every model of the Apex 5/7/9/Pro family receives the
/// whole table; OpenRGB notes that the firmware ignores keys a model does not have.
pub const APEX_LED_TABLE: [(KeyId, u8); 112] = [
    (KeyId::A, 0x04),
    (KeyId::B, 0x05),
    (KeyId::C, 0x06),
    (KeyId::D, 0x07),
    (KeyId::E, 0x08),
    (KeyId::F, 0x09),
    (KeyId::G, 0x0A),
    (KeyId::H, 0x0B),
    (KeyId::I, 0x0C),
    (KeyId::J, 0x0D),
    (KeyId::K, 0x0E),
    (KeyId::L, 0x0F),
    (KeyId::M, 0x10),
    (KeyId::N, 0x11),
    (KeyId::O, 0x12),
    (KeyId::P, 0x13),
    (KeyId::Q, 0x14),
    (KeyId::R, 0x15),
    (KeyId::S, 0x16),
    (KeyId::T, 0x17),
    (KeyId::U, 0x18),
    (KeyId::V, 0x19),
    (KeyId::W, 0x1A),
    (KeyId::X, 0x1B),
    (KeyId::Y, 0x1C),
    (KeyId::Z, 0x1D),
    (KeyId::Key1, 0x1E),
    (KeyId::Key2, 0x1F),
    (KeyId::Key3, 0x20),
    (KeyId::Key4, 0x21),
    (KeyId::Key5, 0x22),
    (KeyId::Key6, 0x23),
    (KeyId::Key7, 0x24),
    (KeyId::Key8, 0x25),
    (KeyId::Key9, 0x26),
    (KeyId::Key0, 0x27),
    (KeyId::Enter, 0x28),
    (KeyId::Escape, 0x29),
    (KeyId::Backspace, 0x2A),
    (KeyId::Tab, 0x2B),
    (KeyId::Space, 0x2C),
    (KeyId::Minus, 0x2D),
    (KeyId::Equal, 0x2E),
    (KeyId::LeftBracket, 0x2F),
    (KeyId::RightBracket, 0x30),
    (KeyId::NonUsHash, 0x32),
    (KeyId::Semicolon, 0x33),
    (KeyId::Quote, 0x34),
    (KeyId::Backtick, 0x35),
    (KeyId::Comma, 0x36),
    (KeyId::Period, 0x37),
    (KeyId::Slash, 0x38),
    (KeyId::CapsLock, 0x39),
    (KeyId::F1, 0x3A),
    (KeyId::F2, 0x3B),
    (KeyId::F3, 0x3C),
    (KeyId::F4, 0x3D),
    (KeyId::F5, 0x3E),
    (KeyId::F6, 0x3F),
    (KeyId::F7, 0x40),
    (KeyId::F8, 0x41),
    (KeyId::F9, 0x42),
    (KeyId::F10, 0x43),
    (KeyId::F11, 0x44),
    (KeyId::F12, 0x45),
    (KeyId::PrintScreen, 0x46),
    (KeyId::ScrollLock, 0x47),
    (KeyId::Pause, 0x48),
    (KeyId::Insert, 0x49),
    (KeyId::Home, 0x4A),
    (KeyId::PageUp, 0x4B),
    (KeyId::Delete, 0x4C),
    (KeyId::End, 0x4D),
    (KeyId::PageDown, 0x4E),
    (KeyId::ArrowRight, 0x4F),
    (KeyId::ArrowLeft, 0x50),
    (KeyId::ArrowDown, 0x51),
    (KeyId::ArrowUp, 0x52),
    (KeyId::NonUsBackslash, 0x64),
    (KeyId::LeftCtrl, 0xE0),
    (KeyId::LeftShift, 0xE1),
    (KeyId::LeftAlt, 0xE2),
    (KeyId::LeftWin, 0xE3),
    (KeyId::RightCtrl, 0xE4),
    (KeyId::RightShift, 0xE5),
    (KeyId::RightAlt, 0xE6),
    (KeyId::RightWin, 0xE7),
    // OpenRGB names this LED `KEY_EN_RIGHT_FUNCTION`: the SteelSeries / Fn key.
    (KeyId::SteelSeriesKey, 0xF0),
    (KeyId::Backslash, 0x31),
    (KeyId::JpRo, 0x87),
    (KeyId::JpKana, 0x88),
    (KeyId::JpYen, 0x89),
    (KeyId::JpHenkan, 0x8A),
    (KeyId::JpMuhenkan, 0x8B),
    (KeyId::NumLock, 0x53),
    (KeyId::NumSlash, 0x54),
    (KeyId::NumAsterisk, 0x55),
    (KeyId::NumMinus, 0x56),
    (KeyId::NumPlus, 0x57),
    (KeyId::NumEnter, 0x58),
    (KeyId::Num1, 0x59),
    (KeyId::Num2, 0x5A),
    (KeyId::Num3, 0x5B),
    (KeyId::Num4, 0x5C),
    (KeyId::Num5, 0x5D),
    (KeyId::Num6, 0x5E),
    (KeyId::Num7, 0x5F),
    (KeyId::Num8, 0x60),
    (KeyId::Num9, 0x61),
    (KeyId::Num0, 0x62),
    (KeyId::NumPeriod, 0x63),
    (KeyId::MediaPlayPause, 0xFB),
];

/// Number of LEDs in [`APEX_LED_TABLE`].
pub const APEX_LED_COUNT: usize = APEX_LED_TABLE.len();

const NO_LED: u8 = u8::MAX;

const fn build_hid_to_led() -> [u8; 256] {
    let mut table = [NO_LED; 256];
    let mut i = 0;
    while i < APEX_LED_COUNT {
        table[APEX_LED_TABLE[i].1 as usize] = i as u8;
        i += 1;
    }
    table
}

const APEX_HID_TO_LED: [u8; 256] = build_hid_to_led();

/// LED index of a HID usage in [`APEX_LED_TABLE`].
pub const fn apex_led_index_for_hid(hid_code: u8) -> Option<usize> {
    match APEX_HID_TO_LED[hid_code as usize] {
        NO_LED => None,
        index => Some(index as usize),
    }
}

/// LED index of a key in [`APEX_LED_TABLE`].
pub fn apex_led_index(key: KeyId) -> Option<usize> {
    APEX_LED_TABLE.iter().position(|(k, _)| *k == key)
}

/// Physical form factor of an Apex per-key board.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ApexFormFactor {
    /// Full size with numpad (OpenRGB `MATRIX_MAP_ANSI`).
    FullSize,
    /// Tenkeyless (`apex_tkl_us_region_patch`).
    TenKeyLess,
    /// 60 % (`apex_mini_us_region_patch`).
    Mini,
}

impl ApexFormFactor {
    /// Matching [`KeyboardLayout`].
    pub const fn layout(self) -> KeyboardLayout {
        match self {
            Self::FullSize => KeyboardLayout::FullSize,
            Self::TenKeyLess => KeyboardLayout::TenKeyLess,
            Self::Mini => KeyboardLayout::Compact,
        }
    }

    /// Whether the LED at `led_index` of [`APEX_LED_TABLE`] belongs to this form factor.
    ///
    /// The full-size set is every LED in OpenRGB's ANSI matrix. The TKL set removes Print
    /// Screen, Scroll Lock and the numpad and puts the media LED in Pause's place. The mini set
    /// further removes the F-row, the backtick (Escape takes its place), the navigation block,
    /// the arrows and the media LED. The ISO and Japanese keys (`apex_iso_region_patch`,
    /// `apex_jp_region_patch`) are included in every set: OpenRGB picks the region from the
    /// serial number, which this crate does not read, and the firmware ignores absent keys.
    pub fn has_led(self, led_index: usize) -> bool {
        let Some((key, _)) = APEX_LED_TABLE.get(led_index) else {
            return false;
        };
        let numpad = (94..=110).contains(&led_index);
        match self {
            Self::FullSize => *key != KeyId::MediaPlayPause,
            Self::TenKeyLess => !numpad && !matches!(key, KeyId::PrintScreen | KeyId::ScrollLock | KeyId::Pause),
            Self::Mini => {
                Self::TenKeyLess.has_led(led_index)
                    && !(53..=64).contains(&led_index)
                    && !(68..=77).contains(&led_index)
                    && !matches!(key, KeyId::Backtick | KeyId::MediaPlayPause)
            }
        }
    }

    /// LED indices of this form factor, ascending.
    pub fn led_indices(self) -> Vec<usize> {
        (0..APEX_LED_COUNT).filter(|&i| self.has_led(i)).collect()
    }
}

/// [EXPERIMENTAL] Form factor of each Apex per-key product ID.
///
/// OpenRGB models: full-size Apex 5, Apex 7, Apex Pro, Apex Pro 3 (`0x1640`); TKL Apex 7 TKL,
/// Apex 9 TKL, Apex Pro TKL and the 2023 / Gen 3 TKL models; mini Apex 9 Mini. The Apex Pro Mini
/// PIDs (`0x161E`, `0x1624`, `0x1626`, `0x1648`) are not detected by OpenRGB; their form factor
/// comes from the Pro Mini SKUs in `SteelSeriesApexRegions.h`.
pub fn apex_form_factor(product_id: u16) -> Option<ApexFormFactor> {
    use product_ids::*;
    Some(match product_id {
        APEX_PRO | APEX_7 | APEX_5 | APEX_PRO_2024 => ApexFormFactor::FullSize,
        APEX_PRO_TKL
        | APEX_7_TKL
        | APEX_9_TKL
        | APEX_PRO_TKL_2023
        | APEX_PRO_TKL_2023_WIRELESS
        | APEX_PRO_TKL_2023_WIRELESS_2
        | APEX_PRO_TKL_2024
        | APEX_PRO_TKL_WIRELESS_2024_DONGLE
        | APEX_PRO_TKL_WIRELESS_2024 => ApexFormFactor::TenKeyLess,
        APEX_9_MINI | APEX_PRO_MINI | APEX_PRO_MINI_WIRELESS_DONGLE | APEX_PRO_MINI_WIRELESS | APEX_PRO_MINI_2024 => {
            ApexFormFactor::Mini
        }
        _ => return None,
    })
}

/// [EXPERIMENTAL] Apex M750 direct-frame slots: 6 rows of 22, bottom row first.
///
/// Reference: OpenRGB `SteelSeriesApexMController.cpp` (`keys_m`). The slot positions and their
/// key labels are taken from that table's comments. Its numpad indices predate the Japanese keys
/// in OpenRGB's current LED list and point five LEDs too low, so keys are matched by label here.
pub const APEX_M750_GRID: [Option<KeyId>; 132] = {
    use KeyId::*;
    [
        // LCTRL LWIN LALT - SPACE - - - - RALT RWIN FN RCTRL - - LEFT DOWN RIGHT - #0 - #.
        Some(LeftCtrl),
        Some(LeftWin),
        Some(LeftAlt),
        None,
        Some(Space),
        None,
        None,
        None,
        None,
        Some(RightAlt),
        Some(RightWin),
        Some(SteelSeriesKey),
        Some(RightCtrl),
        None,
        None,
        Some(ArrowLeft),
        Some(ArrowDown),
        Some(ArrowRight),
        None,
        Some(Num0),
        None,
        Some(NumPeriod),
        // LSHFT Z X C V B N M , . / - RSHFT - - - UP - #1 #2 #3 #ENTR
        Some(LeftShift),
        Some(Z),
        Some(X),
        Some(C),
        Some(V),
        Some(B),
        Some(N),
        Some(M),
        Some(Comma),
        Some(Period),
        Some(Slash),
        None,
        Some(RightShift),
        None,
        None,
        None,
        Some(ArrowUp),
        None,
        Some(Num1),
        Some(Num2),
        Some(Num3),
        Some(NumEnter),
        // CAPLK A S D F G H J K L ; ' - ENTER - - - - #4 #5 #6 -
        Some(CapsLock),
        Some(A),
        Some(S),
        Some(D),
        Some(F),
        Some(G),
        Some(H),
        Some(J),
        Some(K),
        Some(L),
        Some(Semicolon),
        Some(Quote),
        None,
        Some(Enter),
        None,
        None,
        None,
        None,
        Some(Num4),
        Some(Num5),
        Some(Num6),
        None,
        // TAB Q W E R T Y U I O P [ ] - \ DEL END PGDN #7 #8 #9 #+
        Some(Tab),
        Some(Q),
        Some(W),
        Some(E),
        Some(R),
        Some(T),
        Some(Y),
        Some(U),
        Some(I),
        Some(O),
        Some(P),
        Some(LeftBracket),
        Some(RightBracket),
        None,
        Some(Backslash),
        Some(Delete),
        Some(End),
        Some(PageDown),
        Some(Num7),
        Some(Num8),
        Some(Num9),
        Some(NumPlus),
        // ` 1 2 3 4 5 6 7 8 9 0 - = - BKSPC INS HOME PGUP NUMLK #/ #* #-
        Some(Backtick),
        Some(Key1),
        Some(Key2),
        Some(Key3),
        Some(Key4),
        Some(Key5),
        Some(Key6),
        Some(Key7),
        Some(Key8),
        Some(Key9),
        Some(Key0),
        Some(Minus),
        Some(Equal),
        None,
        Some(Backspace),
        Some(Insert),
        Some(Home),
        Some(PageUp),
        Some(NumLock),
        Some(NumSlash),
        Some(NumAsterisk),
        Some(NumMinus),
        // ESC F1 F2 F3 F4 - F5 F6 F7 F8 - F9 F10 F11 F12 PRTSC SCRLK PAUSE - - - -
        Some(Escape),
        Some(F1),
        Some(F2),
        Some(F3),
        Some(F4),
        None,
        Some(F5),
        Some(F6),
        Some(F7),
        Some(F8),
        None,
        Some(F9),
        Some(F10),
        Some(F11),
        Some(F12),
        Some(PrintScreen),
        Some(ScrollLock),
        Some(Pause),
        None,
        None,
        None,
        None,
    ]
};

/// HID address for a specific key (USB HID Usage ID).
///
/// SteelSeries keyboards use standard USB HID keycodes (Usage IDs) for per-key
/// addressing, NOT matrix row/column coordinates. This was discovered through
/// reverse engineering of SteelSeries GG 107.0.0.
///
/// Standard USB HID keycodes:
/// - 4-29: A-Z (4=A, 5=B, ..., 29=Z)
/// - 30-39: 1-0 (30=1, ..., 39=0)
/// - 40: Enter, 41: Escape, 42: Backspace, 43: Tab, 44: Space
/// - 45-55: - = [ ] \ ; ' ` , . /
/// - 56: Caps Lock, 57-68: F1-F12
/// - 69-77: Print/Scroll/Pause/Insert/Home/PgUp/Del/End/PgDn
/// - 78-81: Arrow keys (Right/Left/Down/Up)
/// - 82-97: Numpad keys
/// - 100: Menu, 133: Power
/// - 224-231: Left/Right Ctrl/Shift/Alt/GUI
/// - 240: SteelSeries logo key
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyAddress {
    /// USB HID Usage ID (keycode)
    pub hid_code: u8,
}

impl KeyAddress {
    /// Create a new key address from HID code.
    pub fn new(hid_code: u8) -> Self {
        Self { hid_code }
    }

    /// Create a new key address from HID code (alias for clarity).
    pub fn from_hid(hid_code: u8) -> Self {
        Self { hid_code }
    }
}

impl fmt::Display for KeyAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HID 0x{:02X}", self.hid_code)
    }
}

/// Keyboard layout variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyboardLayout {
    /// Full-size keyboard (104 keys)
    FullSize,
    /// Tenkeyless (87 keys)
    TenKeyLess,
    /// Compact layout
    Compact,
}

/// Complete key mapping for a specific keyboard model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyMapping {
    /// Product ID this mapping applies to
    pub product_id: u16,
    /// Keyboard layout type
    pub layout: KeyboardLayout,
    /// Human-readable name
    pub name: String,
    /// Key ID to HID address mapping
    pub key_map: HashMap<KeyId, KeyAddress>,
    /// Cached list of keys for stable iteration without allocation
    #[serde(skip)]
    pub cached_keys: Vec<KeyId>,
    /// Total number of individually addressable keys
    pub total_keys: usize,
    /// List of supported HID codes for this keyboard
    pub supported_hid_codes: Vec<u8>,
}

impl KeyMapping {
    /// Create a new key mapping.
    pub fn new(product_id: u16, layout: KeyboardLayout, name: String, supported_hid_codes: Vec<u8>) -> Self {
        Self {
            product_id,
            layout,
            name,
            key_map: HashMap::new(),
            cached_keys: Vec::new(),
            total_keys: 0,
            supported_hid_codes,
        }
    }

    /// Add a key mapping.
    pub fn add_key(&mut self, key_id: KeyId, address: KeyAddress) {
        if self.key_map.insert(key_id, address).is_none() {
            self.cached_keys.push(key_id);
        }
        self.total_keys = self.key_map.len();
    }

    /// Get the address for a specific key.
    pub fn get_key_address(&self, key_id: KeyId) -> Option<KeyAddress> {
        self.key_map.get(&key_id).copied()
    }

    /// Get all keys in this mapping.
    pub fn get_all_keys(&self) -> &[KeyId] {
        &self.cached_keys
    }

    /// Check if a key is supported.
    pub fn supports_key(&self, key_id: KeyId) -> bool {
        self.key_map.contains_key(&key_id)
    }

    /// Get mapping statistics.
    pub fn get_stats(&self) -> KeyMappingStats {
        KeyMappingStats {
            total_keys: self.total_keys,
            supported_hid_codes: self.supported_hid_codes.len(),
            mapped_keys: self.key_map.len(),
            utilization: (self.key_map.len() as f32 / self.supported_hid_codes.len().max(1) as f32) * 100.0,
        }
    }
}

/// Key mapping statistics.
#[derive(Debug, Clone)]
pub struct KeyMappingStats {
    pub total_keys: usize,
    pub supported_hid_codes: usize,
    pub mapped_keys: usize,
    pub utilization: f32,
}

impl fmt::Display for KeyMappingStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Keys: {}/{} mapped ({:.1}% utilization)",
            self.mapped_keys, self.supported_hid_codes, self.utilization
        )
    }
}

/// Key mapping database for all supported keyboards.
pub struct KeyMappingDatabase {
    mappings: HashMap<u16, KeyMapping>,
}

impl KeyMappingDatabase {
    /// Create a new key mapping database.
    pub fn new() -> Self {
        let mut db = Self {
            mappings: HashMap::new(),
        };
        db.initialize_known_mappings();
        db
    }

    /// Initialize mappings for known keyboard models.
    fn initialize_known_mappings(&mut self) {
        // NOTE: These mappings are based on reverse engineering of SteelSeries GG 107.0.0
        // The keycodes are standard USB HID Usage IDs discovered from configuration migration files.
        //
        // Source: apex_7+pro.migration, apex_pro_tkl_2022.migration
        // Complete TKL keycode list: (4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31 32 33 34 35 36 37 38 39 40 41 42 43 44 45 46 47 48 49 50 51 52 53 54 55 56 57 58 59 60 61 62 63 64 65 66 67 68 69 73 74 75 76 77 78 79 80 81 82 100 133 135 136 137 138 139 224 225 226 227 228 229 230 231 240)

        // Apex Pro TKL (2023) - Verified HID codes from SteelSeries GG
        self.add_apex_pro_tkl_2023_mapping();

        // Every other Apex per-key model, from the OpenRGB LED table.
        self.add_openrgb_apex_mappings();
        self.add_apex_m750_mapping();
    }

    /// [EXPERIMENTAL] Mappings built from [`APEX_LED_TABLE`] for every per-key PID except the
    /// Apex Pro TKL (2023), which keeps its GG-derived mapping above.
    ///
    /// These replace the earlier GG-derived Apex Pro mapping and the empty Apex Pro TKL
    /// placeholder. The HID usages are the same; the differences are that OpenRGB lists no Menu
    /// key on these boards (the key beside right Win is the SteelSeries key, `0xF0`) and adds
    /// Print Screen, Scroll Lock, Pause, the media LED and the ISO / Japanese keys.
    fn add_openrgb_apex_mappings(&mut self) {
        use product_ids::*;
        let pids = [
            APEX_PRO,
            APEX_7,
            APEX_5,
            APEX_PRO_2024,
            APEX_PRO_TKL,
            APEX_7_TKL,
            APEX_9_TKL,
            APEX_PRO_TKL_2023_WIRELESS,
            APEX_PRO_TKL_2023_WIRELESS_2,
            APEX_PRO_TKL_2024,
            APEX_PRO_TKL_WIRELESS_2024_DONGLE,
            APEX_PRO_TKL_WIRELESS_2024,
            APEX_9_MINI,
            APEX_PRO_MINI,
            APEX_PRO_MINI_WIRELESS_DONGLE,
            APEX_PRO_MINI_WIRELESS,
            APEX_PRO_MINI_2024,
        ];
        for pid in pids {
            let Some(form_factor) = apex_form_factor(pid) else {
                continue;
            };
            let indices = form_factor.led_indices();
            let hid_codes = indices.iter().map(|&i| APEX_LED_TABLE[i].1).collect();
            let mut mapping = KeyMapping::new(
                pid,
                form_factor.layout(),
                crate::devices::device_name_from_product_id(pid).to_string(),
                hid_codes,
            );
            for i in indices {
                let (key, hid_code) = APEX_LED_TABLE[i];
                mapping.add_key(key, KeyAddress::new(hid_code));
            }
            self.mappings.insert(pid, mapping);
        }
    }

    /// [EXPERIMENTAL] Apex M750 mapping: every labelled slot of [`APEX_M750_GRID`], addressed by
    /// the key's HID usage from [`APEX_LED_TABLE`].
    fn add_apex_m750_mapping(&mut self) {
        let keys: Vec<(KeyId, u8)> = APEX_M750_GRID
            .iter()
            .flatten()
            .filter_map(|&key| apex_led_index(key).map(|i| (key, APEX_LED_TABLE[i].1)))
            .collect();
        let mut mapping = KeyMapping::new(
            product_ids::APEX_M750,
            KeyboardLayout::FullSize,
            "Apex M750".to_string(),
            keys.iter().map(|(_, hid)| *hid).collect(),
        );
        for (key, hid_code) in keys {
            mapping.add_key(key, KeyAddress::new(hid_code));
        }
        self.mappings.insert(product_ids::APEX_M750, mapping);
    }

    /// Add Apex Pro TKL 2023 key mapping with verified HID codes.
    ///
    /// HID codes discovered from SteelSeries GG 107.0.0 configuration migration files.
    /// Complete TKL keycode list: 87 keys total
    fn add_apex_pro_tkl_2023_mapping(&mut self) {
        // Complete TKL HID code list from apex_7+pro.migration
        let tkl_hid_codes = vec![
            4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31,
            32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58,
            59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 100, 133, 135, 136,
            137, 138, 139, 224, 225, 226, 227, 228, 229, 230, 231, 240,
        ];

        let mut mapping = KeyMapping::new(
            product_ids::APEX_PRO_TKL_2023,
            KeyboardLayout::TenKeyLess,
            "Apex Pro TKL (2023)".to_string(),
            tkl_hid_codes,
        );

        // Function row (F1-F12 = 58-69, Escape = 41)
        mapping.add_key(KeyId::Escape, KeyAddress::new(41));
        mapping.add_key(KeyId::F1, KeyAddress::new(58));
        mapping.add_key(KeyId::F2, KeyAddress::new(59));
        mapping.add_key(KeyId::F3, KeyAddress::new(60));
        mapping.add_key(KeyId::F4, KeyAddress::new(61));
        mapping.add_key(KeyId::F5, KeyAddress::new(62));
        mapping.add_key(KeyId::F6, KeyAddress::new(63));
        mapping.add_key(KeyId::F7, KeyAddress::new(64));
        mapping.add_key(KeyId::F8, KeyAddress::new(65));
        mapping.add_key(KeyId::F9, KeyAddress::new(66));
        mapping.add_key(KeyId::F10, KeyAddress::new(67));
        mapping.add_key(KeyId::F11, KeyAddress::new(68));
        mapping.add_key(KeyId::F12, KeyAddress::new(69));

        // Number row (1-0 = 30-39, Backtick = 53, Minus = 45, Equal = 46, Backspace = 42)
        mapping.add_key(KeyId::Backtick, KeyAddress::new(53));
        mapping.add_key(KeyId::Key1, KeyAddress::new(30));
        mapping.add_key(KeyId::Key2, KeyAddress::new(31));
        mapping.add_key(KeyId::Key3, KeyAddress::new(32));
        mapping.add_key(KeyId::Key4, KeyAddress::new(33));
        mapping.add_key(KeyId::Key5, KeyAddress::new(34));
        mapping.add_key(KeyId::Key6, KeyAddress::new(35));
        mapping.add_key(KeyId::Key7, KeyAddress::new(36));
        mapping.add_key(KeyId::Key8, KeyAddress::new(37));
        mapping.add_key(KeyId::Key9, KeyAddress::new(38));
        mapping.add_key(KeyId::Key0, KeyAddress::new(39));
        mapping.add_key(KeyId::Minus, KeyAddress::new(45));
        mapping.add_key(KeyId::Equal, KeyAddress::new(46));
        mapping.add_key(KeyId::Backspace, KeyAddress::new(42));

        // QWERTY row (Tab = 43, Q-P = 20-25, Brackets = 47-48, Backslash = 49)
        mapping.add_key(KeyId::Tab, KeyAddress::new(43));
        mapping.add_key(KeyId::Q, KeyAddress::new(20));
        mapping.add_key(KeyId::W, KeyAddress::new(26));
        mapping.add_key(KeyId::E, KeyAddress::new(8));
        mapping.add_key(KeyId::R, KeyAddress::new(21));
        mapping.add_key(KeyId::T, KeyAddress::new(23));
        mapping.add_key(KeyId::Y, KeyAddress::new(28));
        mapping.add_key(KeyId::U, KeyAddress::new(24));
        mapping.add_key(KeyId::I, KeyAddress::new(12));
        mapping.add_key(KeyId::O, KeyAddress::new(18));
        mapping.add_key(KeyId::P, KeyAddress::new(19));
        mapping.add_key(KeyId::LeftBracket, KeyAddress::new(47));
        mapping.add_key(KeyId::RightBracket, KeyAddress::new(48));
        mapping.add_key(KeyId::Backslash, KeyAddress::new(49));

        // Home row (CapsLock = 57, A-L = 4-15, Semicolon = 51, Quote = 52, Enter = 40)
        mapping.add_key(KeyId::CapsLock, KeyAddress::new(57));
        mapping.add_key(KeyId::A, KeyAddress::new(4));
        mapping.add_key(KeyId::S, KeyAddress::new(22));
        mapping.add_key(KeyId::D, KeyAddress::new(7));
        mapping.add_key(KeyId::F, KeyAddress::new(9));
        mapping.add_key(KeyId::G, KeyAddress::new(10));
        mapping.add_key(KeyId::H, KeyAddress::new(11));
        mapping.add_key(KeyId::J, KeyAddress::new(13));
        mapping.add_key(KeyId::K, KeyAddress::new(14));
        mapping.add_key(KeyId::L, KeyAddress::new(15));
        mapping.add_key(KeyId::Semicolon, KeyAddress::new(51));
        mapping.add_key(KeyId::Quote, KeyAddress::new(52));
        mapping.add_key(KeyId::Enter, KeyAddress::new(40));

        // Bottom letter row (LeftShift = 225, Z-M = 29-16, Comma = 54, Period = 55, Slash = 56, RightShift = 229)
        mapping.add_key(KeyId::LeftShift, KeyAddress::new(225));
        mapping.add_key(KeyId::Z, KeyAddress::new(29));
        mapping.add_key(KeyId::X, KeyAddress::new(27));
        mapping.add_key(KeyId::C, KeyAddress::new(6));
        mapping.add_key(KeyId::V, KeyAddress::new(25));
        mapping.add_key(KeyId::B, KeyAddress::new(5));
        mapping.add_key(KeyId::N, KeyAddress::new(17));
        mapping.add_key(KeyId::M, KeyAddress::new(16));
        mapping.add_key(KeyId::Comma, KeyAddress::new(54));
        mapping.add_key(KeyId::Period, KeyAddress::new(55));
        mapping.add_key(KeyId::Slash, KeyAddress::new(56));
        mapping.add_key(KeyId::RightShift, KeyAddress::new(229));

        // Bottom row (LeftCtrl = 224, LeftWin = 227, LeftAlt = 226, Space = 44, RightAlt = 230, RightWin = 231, Menu = 101, RightCtrl = 228)
        mapping.add_key(KeyId::LeftCtrl, KeyAddress::new(224));
        mapping.add_key(KeyId::LeftWin, KeyAddress::new(227));
        mapping.add_key(KeyId::LeftAlt, KeyAddress::new(226));
        mapping.add_key(KeyId::Space, KeyAddress::new(44));
        mapping.add_key(KeyId::RightAlt, KeyAddress::new(230));
        mapping.add_key(KeyId::RightWin, KeyAddress::new(231));
        mapping.add_key(KeyId::Menu, KeyAddress::new(101));
        mapping.add_key(KeyId::RightCtrl, KeyAddress::new(228));

        // Navigation cluster (Insert = 73, Home = 74, PageUp = 75, Delete = 76, End = 77, PageDown = 78)
        mapping.add_key(KeyId::Insert, KeyAddress::new(73));
        mapping.add_key(KeyId::Home, KeyAddress::new(74));
        mapping.add_key(KeyId::PageUp, KeyAddress::new(75));
        mapping.add_key(KeyId::Delete, KeyAddress::new(76));
        mapping.add_key(KeyId::End, KeyAddress::new(77));
        mapping.add_key(KeyId::PageDown, KeyAddress::new(78));

        // Arrow cluster (Right = 79, Left = 80, Down = 81, Up = 82)
        mapping.add_key(KeyId::ArrowRight, KeyAddress::new(79));
        mapping.add_key(KeyId::ArrowLeft, KeyAddress::new(80));
        mapping.add_key(KeyId::ArrowDown, KeyAddress::new(81));
        mapping.add_key(KeyId::ArrowUp, KeyAddress::new(82));

        // SteelSeries key (HID 240 - discovered from migration files)
        mapping.add_key(KeyId::SteelSeriesKey, KeyAddress::new(240));

        self.mappings.insert(product_ids::APEX_PRO_TKL_2023, mapping);
    }

    /// Get key mapping for a specific product ID.
    pub fn get_mapping(&self, product_id: u16) -> Option<&KeyMapping> {
        self.mappings.get(&product_id)
    }

    /// Get all supported product IDs.
    pub fn get_supported_products(&self) -> Vec<u16> {
        self.mappings.keys().copied().collect()
    }

    /// Check if a product ID is supported.
    pub fn supports_product(&self, product_id: u16) -> bool {
        self.mappings.contains_key(&product_id)
    }
}

impl Default for KeyMappingDatabase {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_address_creation() {
        let addr = KeyAddress::new(0x04); // HID code for 'A'
        assert_eq!(addr.hid_code, 0x04);
        assert_eq!(addr.to_string(), "HID 0x04");

        // Test from_hid alias
        let addr2 = KeyAddress::from_hid(0x1E); // HID code for '1'
        assert_eq!(addr2.hid_code, 0x1E);
    }

    #[test]
    fn test_key_mapping_creation() {
        let supported_codes = vec![4, 5, 6, 7, 8]; // A, B, C, D, E
        let mut mapping = KeyMapping::new(
            product_ids::APEX_PRO_TKL_2023,
            KeyboardLayout::TenKeyLess,
            "Test Keyboard".to_string(),
            supported_codes,
        );

        // Initially empty
        assert_eq!(mapping.total_keys, 0);
        assert!(!mapping.supports_key(KeyId::A));

        // Add a key
        mapping.add_key(KeyId::A, KeyAddress::new(4)); // HID 0x04 = A
        assert_eq!(mapping.total_keys, 1);
        assert!(mapping.supports_key(KeyId::A));
        assert_eq!(mapping.get_key_address(KeyId::A), Some(KeyAddress::new(4)));
        assert_eq!(mapping.get_key_address(KeyId::B), None);
    }

    #[test]
    fn test_key_mapping_database() {
        let db = KeyMappingDatabase::new();

        // Should support known products
        assert!(db.supports_product(product_ids::APEX_PRO_TKL_2023));
        assert!(db.supports_product(product_ids::APEX_PRO));
        assert!(!db.supports_product(0xFFFF)); // Unknown product

        // Should have mappings
        let mapping = db.get_mapping(product_ids::APEX_PRO_TKL_2023);
        assert!(mapping.is_some());

        let mapping = mapping.unwrap();
        assert_eq!(mapping.layout, KeyboardLayout::TenKeyLess);
        assert!(mapping.total_keys > 0);

        // Verify specific HID codes are mapped
        let a_key = mapping.get_key_address(KeyId::A);
        assert!(a_key.is_some());
        assert_eq!(a_key.unwrap().hid_code, 4); // HID 0x04 = A
    }

    #[test]
    fn test_key_mapping_stats() {
        let supported_codes = vec![4, 5, 6, 7, 8, 9, 10]; // A-G
        let mut mapping = KeyMapping::new(
            product_ids::APEX_PRO_TKL_2023,
            KeyboardLayout::TenKeyLess,
            "Test".to_string(),
            supported_codes,
        );

        // Add some keys
        mapping.add_key(KeyId::A, KeyAddress::new(4));
        mapping.add_key(KeyId::B, KeyAddress::new(5));
        mapping.add_key(KeyId::C, KeyAddress::new(6));

        let stats = mapping.get_stats();
        assert_eq!(stats.total_keys, 3);
        assert_eq!(stats.supported_hid_codes, 7);
        assert_eq!(stats.mapped_keys, 3);
        assert!(stats.utilization > 0.0);
        assert!(stats.utilization <= 100.0);
    }

    #[test]
    fn test_key_id_display() {
        assert_eq!(KeyId::A.to_string(), "A");
        assert_eq!(KeyId::Escape.to_string(), "ESC");
        assert_eq!(KeyId::LeftShift.to_string(), "LSHIFT");
        assert_eq!(KeyId::Space.to_string(), "SPACE");
        assert_eq!(KeyId::ArrowUp.to_string(), "UP");
        assert_eq!(KeyId::Num1.to_string(), "NUM1");
        assert_eq!(KeyId::SteelSeriesKey.to_string(), "SS");
    }

    /// OpenRGB SteelSeriesApexController.cpp `keys[]` and SteelSeriesApexRegions.h `led_names[]`
    /// both have 112 entries.
    #[test]
    fn apex_led_table_has_112_unique_leds() {
        use std::collections::HashSet;
        assert_eq!(APEX_LED_COUNT, 112);
        let hids: HashSet<u8> = APEX_LED_TABLE.iter().map(|(_, hid)| *hid).collect();
        let keys: HashSet<KeyId> = APEX_LED_TABLE.iter().map(|(key, _)| *key).collect();
        assert_eq!(hids.len(), 112, "HID usages must be unique");
        assert_eq!(keys.len(), 112, "keys must be unique");
        // Spot checks against OpenRGB's LED indices.
        assert_eq!(APEX_LED_TABLE[0], (KeyId::A, 0x04));
        assert_eq!(APEX_LED_TABLE[37], (KeyId::Escape, 0x29));
        assert_eq!(APEX_LED_TABLE[45], (KeyId::NonUsHash, 0x32));
        assert_eq!(APEX_LED_TABLE[78], (KeyId::NonUsBackslash, 0x64));
        assert_eq!(APEX_LED_TABLE[87], (KeyId::SteelSeriesKey, 0xF0));
        assert_eq!(APEX_LED_TABLE[88], (KeyId::Backslash, 0x31));
        assert_eq!(APEX_LED_TABLE[94], (KeyId::NumLock, 0x53));
        assert_eq!(APEX_LED_TABLE[111], (KeyId::MediaPlayPause, 0xFB));
    }

    #[test]
    fn apex_led_lookups_are_inverse() {
        for (index, (key, hid)) in APEX_LED_TABLE.iter().enumerate() {
            assert_eq!(apex_led_index_for_hid(*hid), Some(index));
            assert_eq!(apex_led_index(*key), Some(index));
        }
        assert_eq!(apex_led_index_for_hid(0x65), None, "no Menu LED on Apex boards");
        assert_eq!(apex_led_index_for_hid(0x00), None);
        assert_eq!(apex_led_index(KeyId::Menu), None);
        assert_eq!(apex_led_index(KeyId::VolumeWheel), None);
    }

    /// OpenRGB `MATRIX_MAP_ANSI` plus `apex_tkl_us_region_patch` / `apex_mini_us_region_patch`,
    /// with the 7 ISO / Japanese LEDs in every set.
    #[test]
    fn apex_form_factor_counts() {
        assert_eq!(ApexFormFactor::FullSize.led_indices().len(), 111);
        assert_eq!(ApexFormFactor::TenKeyLess.led_indices().len(), 92);
        assert_eq!(ApexFormFactor::Mini.led_indices().len(), 68);

        let has = |ff: ApexFormFactor, key| apex_led_index(key).is_some_and(|i| ff.has_led(i));
        assert!(has(ApexFormFactor::FullSize, KeyId::Num5));
        assert!(has(ApexFormFactor::FullSize, KeyId::Pause));
        assert!(!has(ApexFormFactor::FullSize, KeyId::MediaPlayPause));
        assert!(!has(ApexFormFactor::TenKeyLess, KeyId::Num5));
        assert!(!has(ApexFormFactor::TenKeyLess, KeyId::PrintScreen));
        assert!(has(ApexFormFactor::TenKeyLess, KeyId::MediaPlayPause));
        assert!(has(ApexFormFactor::TenKeyLess, KeyId::F12));
        assert!(!has(ApexFormFactor::Mini, KeyId::F1));
        assert!(!has(ApexFormFactor::Mini, KeyId::ArrowUp));
        assert!(!has(ApexFormFactor::Mini, KeyId::Backtick));
        assert!(!has(ApexFormFactor::Mini, KeyId::Delete));
        assert!(has(ApexFormFactor::Mini, KeyId::Escape));
        assert!(has(ApexFormFactor::Mini, KeyId::SteelSeriesKey));
        for ff in [
            ApexFormFactor::FullSize,
            ApexFormFactor::TenKeyLess,
            ApexFormFactor::Mini,
        ] {
            assert!(has(ff, KeyId::NonUsBackslash) && has(ff, KeyId::JpYen));
        }
    }

    /// OpenRGB SteelSeriesApexMController.cpp `keys_m`: 132 slots, 22 per row.
    #[test]
    fn apex_m750_grid_has_unique_labelled_keys() {
        use std::collections::HashSet;
        let keys: Vec<KeyId> = APEX_M750_GRID.iter().flatten().copied().collect();
        let unique: HashSet<KeyId> = keys.iter().copied().collect();
        assert_eq!(unique.len(), keys.len(), "a key appears in two slots");
        assert_eq!(keys.len(), 104);
        assert!(keys.iter().all(|key| apex_led_index(*key).is_some()));
        assert_eq!(APEX_M750_GRID[0], Some(KeyId::LeftCtrl));
        assert_eq!(APEX_M750_GRID[4], Some(KeyId::Space));
        assert_eq!(APEX_M750_GRID[21], Some(KeyId::NumPeriod));
        assert_eq!(APEX_M750_GRID[110], Some(KeyId::Escape));
        assert_eq!(APEX_M750_GRID[127], Some(KeyId::Pause));
    }

    #[test]
    fn openrgb_mappings_cover_every_per_key_pid() {
        let db = KeyMappingDatabase::new();
        for (pid, expected) in [
            (product_ids::APEX_PRO, 111),
            (product_ids::APEX_7, 111),
            (product_ids::APEX_5, 111),
            (product_ids::APEX_PRO_2024, 111),
            (product_ids::APEX_PRO_TKL, 92),
            (product_ids::APEX_7_TKL, 92),
            (product_ids::APEX_9_TKL, 92),
            (product_ids::APEX_PRO_TKL_2023_WIRELESS, 92),
            (product_ids::APEX_PRO_TKL_2024, 92),
            (product_ids::APEX_PRO_TKL_WIRELESS_2024, 92),
            (product_ids::APEX_9_MINI, 68),
            (product_ids::APEX_PRO_MINI, 68),
            (product_ids::APEX_PRO_MINI_2024, 68),
            (product_ids::APEX_M750, 104),
        ] {
            let mapping = db.get_mapping(pid).unwrap();
            assert_eq!(mapping.total_keys, expected, "PID {pid:#06x}");
            assert_eq!(mapping.supported_hid_codes.len(), expected, "PID {pid:#06x}");
            assert!(mapping.get_all_keys().iter().all(|k| apex_led_index(*k).is_some()));
        }
        // The Apex Pro TKL (2023) keeps its GG-derived mapping.
        let tkl_2023 = db.get_mapping(product_ids::APEX_PRO_TKL_2023).unwrap();
        assert_eq!(tkl_2023.get_key_address(KeyId::Menu), Some(KeyAddress::new(101)));
        assert!(db.get_mapping(product_ids::APEX_3_TKL).is_none());
    }

    #[test]
    fn test_keyboard_layout_types() {
        let full = KeyboardLayout::FullSize;
        let tkl = KeyboardLayout::TenKeyLess;
        let compact = KeyboardLayout::Compact;

        assert_ne!(full, tkl);
        assert_ne!(tkl, compact);
        assert_ne!(full, compact);
    }
}
