//! Zone names of the GameSense SDK ("Zones by device type") and their mapping to keys.
//!
//! Key-level zones are defined as USB HID usage codes, the same numbers the SDK uses for
//! `custom-zone-keys` and that `KeyAddress::hid_code` stores. They are converted to [`KeyId`]
//! when every key of the zone has one.

use crate::devices::key_mapping::KeyId;

use super::output::{DeviceType, LightTarget};

/// Where a lighting handler applies: a fixed zone name or a list of HID codes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ZoneSpec {
    /// `"zone": "<name>"`.
    Named(String),
    /// `"custom-zone-keys": [<hid code>, ...]`, in effect order.
    CustomKeys(Vec<u8>),
}

/// USB HID usage code for each [`KeyId`] that has one.
///
/// `SteelSeriesKey` (240) follows `KeyMappingDatabase`, which took it from GG migration files.
const HID_KEYS: &[(u8, KeyId)] = &[
    (4, KeyId::A),
    (5, KeyId::B),
    (6, KeyId::C),
    (7, KeyId::D),
    (8, KeyId::E),
    (9, KeyId::F),
    (10, KeyId::G),
    (11, KeyId::H),
    (12, KeyId::I),
    (13, KeyId::J),
    (14, KeyId::K),
    (15, KeyId::L),
    (16, KeyId::M),
    (17, KeyId::N),
    (18, KeyId::O),
    (19, KeyId::P),
    (20, KeyId::Q),
    (21, KeyId::R),
    (22, KeyId::S),
    (23, KeyId::T),
    (24, KeyId::U),
    (25, KeyId::V),
    (26, KeyId::W),
    (27, KeyId::X),
    (28, KeyId::Y),
    (29, KeyId::Z),
    (30, KeyId::Key1),
    (31, KeyId::Key2),
    (32, KeyId::Key3),
    (33, KeyId::Key4),
    (34, KeyId::Key5),
    (35, KeyId::Key6),
    (36, KeyId::Key7),
    (37, KeyId::Key8),
    (38, KeyId::Key9),
    (39, KeyId::Key0),
    (40, KeyId::Enter),
    (41, KeyId::Escape),
    (42, KeyId::Backspace),
    (43, KeyId::Tab),
    (44, KeyId::Space),
    (45, KeyId::Minus),
    (46, KeyId::Equal),
    (47, KeyId::LeftBracket),
    (48, KeyId::RightBracket),
    (49, KeyId::Backslash),
    (51, KeyId::Semicolon),
    (52, KeyId::Quote),
    (53, KeyId::Backtick),
    (54, KeyId::Comma),
    (55, KeyId::Period),
    (56, KeyId::Slash),
    (57, KeyId::CapsLock),
    (58, KeyId::F1),
    (59, KeyId::F2),
    (60, KeyId::F3),
    (61, KeyId::F4),
    (62, KeyId::F5),
    (63, KeyId::F6),
    (64, KeyId::F7),
    (65, KeyId::F8),
    (66, KeyId::F9),
    (67, KeyId::F10),
    (68, KeyId::F11),
    (69, KeyId::F12),
    (73, KeyId::Insert),
    (74, KeyId::Home),
    (75, KeyId::PageUp),
    (76, KeyId::Delete),
    (77, KeyId::End),
    (78, KeyId::PageDown),
    (79, KeyId::ArrowRight),
    (80, KeyId::ArrowLeft),
    (81, KeyId::ArrowDown),
    (82, KeyId::ArrowUp),
    (83, KeyId::NumLock),
    (84, KeyId::NumSlash),
    (85, KeyId::NumAsterisk),
    (86, KeyId::NumMinus),
    (87, KeyId::NumPlus),
    (88, KeyId::NumEnter),
    (89, KeyId::Num1),
    (90, KeyId::Num2),
    (91, KeyId::Num3),
    (92, KeyId::Num4),
    (93, KeyId::Num5),
    (94, KeyId::Num6),
    (95, KeyId::Num7),
    (96, KeyId::Num8),
    (97, KeyId::Num9),
    (98, KeyId::Num0),
    (99, KeyId::NumPeriod),
    (101, KeyId::Menu),
    (224, KeyId::LeftCtrl),
    (225, KeyId::LeftShift),
    (226, KeyId::LeftAlt),
    (227, KeyId::LeftWin),
    (228, KeyId::RightCtrl),
    (229, KeyId::RightShift),
    (230, KeyId::RightAlt),
    (231, KeyId::RightWin),
    (240, KeyId::SteelSeriesKey),
];

/// Single-key zone names of `rgb-per-key-zones` that are not letters or numbered keys.
const NAMED_KEYS: &[(&str, u8)] = &[
    ("return", 40),
    ("escape", 41),
    ("esc", 41),
    ("backspace", 42),
    ("tab", 43),
    ("spacebar", 44),
    ("dash", 45),
    ("equal", 46),
    ("l-bracket", 47),
    ("r-bracket", 48),
    ("backslash", 49),
    ("pound", 50),
    ("semicolon", 51),
    ("quote", 52),
    ("backquote", 53),
    ("comma", 54),
    ("period", 55),
    ("slash", 56),
    ("caps", 57),
    ("printscreen", 70),
    ("scrolllock", 71),
    ("pause", 72),
    ("insert", 73),
    ("home", 74),
    ("pageup", 75),
    ("delete", 76),
    ("end", 77),
    ("pagedown", 78),
    ("rightarrow", 79),
    ("leftarrow", 80),
    ("downarrow", 81),
    ("uparrow", 82),
    ("keypad-num-lock", 83),
    ("keypad-divide", 84),
    ("keypad-times", 85),
    ("keypad-minus", 86),
    ("keypad-plus", 87),
    ("keypad-enter", 88),
    ("keypad-period", 99),
    ("win-menu", 101),
    ("l-ctrl", 224),
    ("l-shift", 225),
    ("l-alt", 226),
    ("l-win", 227),
    ("r-ctrl", 228),
    ("r-shift", 229),
    ("r-alt", 230),
    ("r-win", 231),
    ("ss-key", 240),
];

const FUNCTION_KEYS: &[u8] = &[58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69];
const NUMBER_KEYS: &[u8] = &[30, 31, 32, 33, 34, 35, 36, 37, 38, 39];
const Q_ROW: &[u8] = &[20, 26, 8, 21, 23, 28, 24, 12, 18, 19];
const A_ROW: &[u8] = &[4, 22, 7, 9, 10, 11, 13, 14, 15, 51];
const Z_ROW: &[u8] = &[29, 27, 6, 25, 5, 17, 16, 54, 55, 56];
const NAV_CLUSTER: &[u8] = &[70, 71, 72, 73, 74, 75, 76, 77, 78];
const ARROWS: &[u8] = &[79, 80, 81, 82];
const KEYPAD: &[u8] = &[83, 84, 85, 86, 95, 96, 97, 87, 92, 93, 94, 89, 90, 91, 88, 98, 99];
const KEYPAD_NUMS: &[u8] = &[89, 90, 91, 92, 93, 94, 95, 96, 97];
/// US ANSI central block, left to right and top to bottom, without the function row.
const MAIN_KEYBOARD: &[u8] = &[
    53, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 45, 46, 42, // number row
    43, 20, 26, 8, 21, 23, 28, 24, 12, 18, 19, 47, 48, 49, // q row
    57, 4, 22, 7, 9, 10, 11, 13, 14, 15, 51, 52, 40, // a row
    225, 29, 27, 6, 25, 5, 17, 16, 54, 55, 56, 229, // z row
    224, 227, 226, 44, 230, 231, 101, 228, // bottom row
];

/// Zone names with no HID code: they reach device code as [`LightTarget::NamedZone`].
const NON_HID_KEYBOARD_ZONES: &[&str] = &[
    "logo",
    "macro-keys",
    "all-macro-keys",
    "m0",
    "m1",
    "m2",
    "m3",
    "m4",
    "m5",
];

/// [`KeyId`] for a USB HID usage code.
pub fn hid_to_key_id(code: u8) -> Option<KeyId> {
    HID_KEYS.iter().find(|(c, _)| *c == code).map(|(_, k)| *k)
}

/// USB HID usage code for a [`KeyId`]. `VolumeWheel` has none.
pub fn key_id_to_hid(key: KeyId) -> Option<u8> {
    HID_KEYS.iter().find(|(_, k)| *k == key).map(|(c, _)| *c)
}

/// HID codes of a key-level keyboard zone (`q`, `f5`, `keyboard-1`, `function-keys`, ...),
/// in SDK order. `None` for unknown names and for zones without HID codes (`logo`, `macro-keys`).
pub fn keyboard_zone_hid_codes(name: &str) -> Option<Vec<u8>> {
    let name = name.trim().to_ascii_lowercase();
    let group: Option<&[u8]> = match name.as_str() {
        "function-keys" => Some(FUNCTION_KEYS),
        "number-keys" => Some(NUMBER_KEYS),
        "q-row" => Some(Q_ROW),
        "a-row" => Some(A_ROW),
        "z-row" => Some(Z_ROW),
        "main-keyboard" => Some(MAIN_KEYBOARD),
        "nav-cluster" => Some(NAV_CLUSTER),
        "arrows" => Some(ARROWS),
        "keypad" => Some(KEYPAD),
        "keypad-nums" => Some(KEYPAD_NUMS),
        _ => None,
    };
    if let Some(codes) = group {
        return Some(codes.to_vec());
    }
    single_key_hid(&name).map(|code| vec![code])
}

fn single_key_hid(name: &str) -> Option<u8> {
    let bytes = name.as_bytes();
    if bytes.len() == 1 && bytes[0].is_ascii_lowercase() {
        return Some(4 + (bytes[0] - b'a'));
    }
    if let Some(digit) = name.strip_prefix("keyboard-") {
        return digit_key(digit, 30, 39);
    }
    if let Some(digit) = name.strip_prefix("keypad-") {
        if let Some(code) = digit_key(digit, 89, 98) {
            return Some(code);
        }
    }
    if let Some(number) = name.strip_prefix('f') {
        if let Ok(n) = number.parse::<u8>() {
            if (1..=12).contains(&n) && !number.starts_with('0') {
                return Some(57 + n);
            }
        }
    }
    NAMED_KEYS.iter().find(|(n, _)| *n == name).map(|(_, code)| *code)
}

/// `1`..`9` map to `first`.. and `0` maps to `zero` (HID puts 0 after 9).
fn digit_key(digit: &str, first: u8, zero: u8) -> Option<u8> {
    match digit.as_bytes() {
        [b'0'] => Some(zero),
        [d @ b'1'..=b'9'] => Some(first + (d - b'1')),
        _ => None,
    }
}

/// Key-level target for HID codes: [`LightTarget::Keys`] when every code has a [`KeyId`],
/// otherwise [`LightTarget::HidCodes`] with the full list.
pub fn target_for_hid_codes(codes: Vec<u8>) -> LightTarget {
    let keys: Option<Vec<KeyId>> = codes.iter().map(|c| hid_to_key_id(*c)).collect();
    match keys {
        Some(keys) => LightTarget::Keys(keys),
        None => LightTarget::HidCodes(codes),
    }
}

/// [`KeyId`]s of a key-level keyboard zone, when every key of the zone has one.
pub fn keyboard_zone_keys(name: &str) -> Option<Vec<KeyId>> {
    match target_for_hid_codes(keyboard_zone_hid_codes(name)?) {
        LightTarget::Keys(keys) => Some(keys),
        _ => None,
    }
}

const UNITS: [&str; 9] = ["one", "two", "three", "four", "five", "six", "seven", "eight", "nine"];
const TEENS: [&str; 10] = [
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
];
const TENS: [&str; 8] = [
    "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];

/// Number of an SDK numbered zone: `one` is 1, `twenty-four` is 24, `one-hundred-three` is 103.
pub fn parse_zone_number(name: &str) -> Option<u16> {
    let name = name.trim().to_ascii_lowercase();
    if let Some(rest) = name.strip_prefix("one-hundred") {
        if rest.is_empty() {
            return Some(100);
        }
        return rest.strip_prefix('-').and_then(below_hundred).map(|n| 100 + n);
    }
    below_hundred(&name)
}

fn below_hundred(word: &str) -> Option<u16> {
    let index_of = |list: &[&str], w: &str| list.iter().position(|x| *x == w).map(|i| i as u16);
    if let Some(i) = index_of(&UNITS, word) {
        return Some(i + 1);
    }
    if let Some(i) = index_of(&TEENS, word) {
        return Some(i + 10);
    }
    if let Some(i) = index_of(&TENS, word) {
        return Some((i + 2) * 10);
    }
    let (tens, unit) = word.split_once('-')?;
    Some((index_of(&TENS, tens)? + 2) * 10 + index_of(&UNITS, unit)? + 1)
}

/// Resolve a lighting handler's zone for its device type.
///
/// Returns an error message for zones the device type cannot have, such as `five` on
/// `rgb-2-zone`. Unknown names on keyboards, mice and headsets are passed through as
/// [`LightTarget::NamedZone`] so device code can decide; they never fall back to the whole device.
pub fn resolve_zone(device_type: &DeviceType, zone: &ZoneSpec) -> Result<LightTarget, String> {
    let name = match zone {
        ZoneSpec::CustomKeys(codes) => {
            if codes.is_empty() {
                return Err("custom-zone-keys is empty".to_string());
            }
            return Ok(LightTarget::HidCodes(codes.clone()));
        }
        ZoneSpec::Named(name) => name.trim().to_ascii_lowercase(),
    };
    if name.is_empty() {
        return Err("zone is empty".to_string());
    }
    if name == "all" {
        return Ok(LightTarget::AllZones);
    }
    let numbered = parse_zone_number(&name).map(|n| usize::from(n) - 1);
    match device_type {
        DeviceType::Keyboard | DeviceType::RgbPerKeyZones => {
            if let Some(codes) = keyboard_zone_hid_codes(&name) {
                return Ok(target_for_hid_codes(codes));
            }
            if !NON_HID_KEYBOARD_ZONES.contains(&name.as_str()) {
                tracing::debug!("GameSense: keyboard zone '{}' is not a known SDK zone", name);
            }
            Ok(LightTarget::NamedZone(name))
        }
        DeviceType::RgbZoned(count) => match numbered {
            Some(index) if index < usize::from(*count) => Ok(LightTarget::Zone(index)),
            Some(_) => Err(format!("zone '{name}' does not exist on {device_type}")),
            None => Err(format!("zone '{name}' is not a numbered zone")),
        },
        DeviceType::RgbZonedDevice | DeviceType::Indicator => match numbered {
            Some(index) => Ok(LightTarget::Zone(index)),
            None => Err(format!("zone '{name}' is not a numbered zone")),
        },
        DeviceType::Mouse | DeviceType::Headset => Ok(match numbered {
            Some(index) => LightTarget::Zone(index),
            None => LightTarget::NamedZone(name),
        }),
        DeviceType::Screened(_) | DeviceType::Tactile => Err(format!("{device_type} has no lighting zones")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(name: &str) -> ZoneSpec {
        ZoneSpec::Named(name.to_string())
    }

    #[test]
    fn hid_and_key_id_round_trip() {
        for (code, key) in HID_KEYS {
            assert_eq!(hid_to_key_id(*code), Some(*key));
            assert_eq!(key_id_to_hid(*key), Some(*code));
        }
        assert_eq!(hid_to_key_id(70), None);
        assert_eq!(key_id_to_hid(KeyId::VolumeWheel), None);
    }

    #[test]
    fn single_key_zone_names() {
        assert_eq!(keyboard_zone_keys("q"), Some(vec![KeyId::Q]));
        assert_eq!(keyboard_zone_keys("w"), Some(vec![KeyId::W]));
        assert_eq!(keyboard_zone_keys("z"), Some(vec![KeyId::Z]));
        assert_eq!(keyboard_zone_keys("keyboard-1"), Some(vec![KeyId::Key1]));
        assert_eq!(keyboard_zone_keys("keyboard-0"), Some(vec![KeyId::Key0]));
        assert_eq!(keyboard_zone_keys("keypad-0"), Some(vec![KeyId::Num0]));
        assert_eq!(keyboard_zone_keys("keypad-7"), Some(vec![KeyId::Num7]));
        assert_eq!(keyboard_zone_keys("f1"), Some(vec![KeyId::F1]));
        assert_eq!(keyboard_zone_keys("f12"), Some(vec![KeyId::F12]));
        assert_eq!(keyboard_zone_keys("esc"), Some(vec![KeyId::Escape]));
        assert_eq!(keyboard_zone_keys("escape"), Some(vec![KeyId::Escape]));
        assert_eq!(keyboard_zone_keys("spacebar"), Some(vec![KeyId::Space]));
        assert_eq!(keyboard_zone_keys("l-shift"), Some(vec![KeyId::LeftShift]));
        assert_eq!(keyboard_zone_keys("win-menu"), Some(vec![KeyId::Menu]));
        assert_eq!(keyboard_zone_keys("ss-key"), Some(vec![KeyId::SteelSeriesKey]));
        assert_eq!(keyboard_zone_keys("uparrow"), Some(vec![KeyId::ArrowUp]));
        assert_eq!(keyboard_zone_keys("F5"), Some(vec![KeyId::F5]));
        assert_eq!(keyboard_zone_hid_codes("f13"), None);
        assert_eq!(keyboard_zone_hid_codes("f0"), None);
        assert_eq!(keyboard_zone_hid_codes("printscreen"), Some(vec![70]));
        assert_eq!(keyboard_zone_keys("printscreen"), None);
    }

    #[test]
    fn group_zone_names() {
        let fkeys = keyboard_zone_keys("function-keys").unwrap();
        assert_eq!(fkeys.len(), 12);
        assert_eq!(fkeys[0], KeyId::F1);
        assert_eq!(fkeys[11], KeyId::F12);

        let numbers = keyboard_zone_keys("number-keys").unwrap();
        assert_eq!(numbers.first(), Some(&KeyId::Key1));
        assert_eq!(numbers.last(), Some(&KeyId::Key0));

        assert_eq!(
            keyboard_zone_keys("q-row").unwrap(),
            vec![
                KeyId::Q,
                KeyId::W,
                KeyId::E,
                KeyId::R,
                KeyId::T,
                KeyId::Y,
                KeyId::U,
                KeyId::I,
                KeyId::O,
                KeyId::P
            ]
        );
        assert_eq!(keyboard_zone_keys("a-row").unwrap().last(), Some(&KeyId::Semicolon));
        assert_eq!(keyboard_zone_keys("z-row").unwrap().last(), Some(&KeyId::Slash));
        assert_eq!(
            keyboard_zone_keys("arrows").unwrap(),
            vec![KeyId::ArrowRight, KeyId::ArrowLeft, KeyId::ArrowDown, KeyId::ArrowUp]
        );
        assert_eq!(keyboard_zone_keys("keypad").unwrap().len(), 17);
        assert_eq!(keyboard_zone_keys("keypad-nums").unwrap().first(), Some(&KeyId::Num1));

        let main = keyboard_zone_keys("main-keyboard").unwrap();
        assert!(main.contains(&KeyId::Space));
        assert!(main.contains(&KeyId::Q));
        assert!(!main.contains(&KeyId::F1));
        assert!(!main.contains(&KeyId::Escape));
    }

    #[test]
    fn zone_numbers() {
        assert_eq!(parse_zone_number("one"), Some(1));
        assert_eq!(parse_zone_number("nine"), Some(9));
        assert_eq!(parse_zone_number("ten"), Some(10));
        assert_eq!(parse_zone_number("seventeen"), Some(17));
        assert_eq!(parse_zone_number("twenty"), Some(20));
        assert_eq!(parse_zone_number("twenty-four"), Some(24));
        assert_eq!(parse_zone_number("ninety-nine"), Some(99));
        assert_eq!(parse_zone_number("one-hundred"), Some(100));
        assert_eq!(parse_zone_number("one-hundred-three"), Some(103));
        assert_eq!(parse_zone_number("zero"), None);
        assert_eq!(parse_zone_number("twenty-"), None);
        assert_eq!(parse_zone_number("one-hundred-"), None);
        assert_eq!(parse_zone_number("logo"), None);
    }

    #[test]
    fn resolve_zone_per_device_type() {
        let kb = DeviceType::Keyboard;
        assert_eq!(
            resolve_zone(&kb, &named("function-keys")),
            Ok(LightTarget::Keys(keyboard_zone_keys("function-keys").unwrap()))
        );
        assert_eq!(resolve_zone(&kb, &named("all")), Ok(LightTarget::AllZones));
        assert_eq!(
            resolve_zone(&kb, &named("macro-keys")),
            Ok(LightTarget::NamedZone("macro-keys".to_string()))
        );
        assert_eq!(
            resolve_zone(&DeviceType::RgbPerKeyZones, &named("nav-cluster")),
            Ok(LightTarget::HidCodes(NAV_CLUSTER.to_vec()))
        );
        assert_eq!(
            resolve_zone(&DeviceType::RgbPerKeyZones, &ZoneSpec::CustomKeys(vec![26, 4, 22, 7])),
            Ok(LightTarget::HidCodes(vec![26, 4, 22, 7]))
        );
        assert_eq!(
            resolve_zone(&DeviceType::Mouse, &named("wheel")),
            Ok(LightTarget::NamedZone("wheel".to_string()))
        );
        assert_eq!(
            resolve_zone(&DeviceType::Mouse, &named("logo")),
            Ok(LightTarget::NamedZone("logo".to_string()))
        );
        assert_eq!(
            resolve_zone(&DeviceType::Headset, &named("earcups")),
            Ok(LightTarget::NamedZone("earcups".to_string()))
        );
        assert_eq!(
            resolve_zone(&DeviceType::Indicator, &named("one")),
            Ok(LightTarget::Zone(0))
        );
        assert_eq!(
            resolve_zone(&DeviceType::RgbZoned(2), &named("two")),
            Ok(LightTarget::Zone(1))
        );
        assert!(resolve_zone(&DeviceType::RgbZoned(2), &named("three")).is_err());
        assert_eq!(
            resolve_zone(&DeviceType::RgbZoned(103), &named("one-hundred-three")),
            Ok(LightTarget::Zone(102))
        );
        assert_eq!(
            resolve_zone(&DeviceType::RgbZonedDevice, &named("twelve")),
            Ok(LightTarget::Zone(11))
        );
        assert!(resolve_zone(&DeviceType::RgbZoned(5), &named("logo")).is_err());
        assert!(resolve_zone(&DeviceType::Tactile, &named("one")).is_err());
        assert!(resolve_zone(&kb, &ZoneSpec::CustomKeys(Vec::new())).is_err());
    }
}
