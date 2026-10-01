//! Device tests ported from rivalcfg (`test/devices/old_specs/*.py` and
//! `test/devices/specs/*.txt`, commit f16c521).
//!
//! Every case feeds the value a rivalcfg test passes (as the typed `Input` it parses to) to the
//! same profile setting here and compares the bytes with the test's expectation: report type
//! (02 = output, 03 = feature), report ID, payload. The cases were extracted by running
//! rivalcfg's tests with their asserts recorded, so each expected string is the literal one
//! from the cited test. The spec files also expect the CLI's save packet after each setting;
//! those are checked with `check_save`.

use std::collections::BTreeMap;

use super::encode::{ColorEffect, Gradient, GradientStop};
use super::profile::{Input, Profile};
use super::profiles::*;
use crate::rgb::Color;

fn rgb(hex: u32) -> Color {
    Color::from_hex(hex)
}

fn gradient(duration_ms: u32, stops: &[(u8, u32)]) -> ColorEffect {
    ColorEffect::Gradient(Gradient {
        duration_ms,
        stops: stops
            .iter()
            .map(|(position, hex)| GradientStop {
                position: *position,
                color: rgb(*hex),
            })
            .collect(),
    })
}

fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs.iter().map(|(b, a)| (b.to_string(), a.to_string())).collect()
}

fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).unwrap_or_else(|_| panic!("bad hex byte {b}")))
        .collect()
}

#[track_caller]
fn check(profile: &Profile, id: &str, input: Input<'_>, expected: &str) {
    let setting = profile
        .setting(id)
        .unwrap_or_else(|| panic!("{} has no setting {id}", profile.name));
    let report = profile
        .encode(setting, &input)
        .unwrap_or_else(|e| panic!("{} {id} {input:?}: {e}", profile.name));
    assert_eq!(report.rivalcfg_bytes(), hex(expected), "{} {id} {input:?}", profile.name);
}

/// For rivalcfg tests that check a padded report by its start and its total length.
#[track_caller]
fn check_padded(profile: &Profile, id: &str, input: Input<'_>, prefix: &str, total_len: usize) {
    let setting = profile
        .setting(id)
        .unwrap_or_else(|| panic!("{} has no setting {id}", profile.name));
    let bytes = profile
        .encode(setting, &input)
        .unwrap_or_else(|e| panic!("{} {id} {input:?}: {e}", profile.name))
        .rivalcfg_bytes();
    assert!(bytes.starts_with(&hex(prefix)), "{} {id} {input:?}: {bytes:02X?}", profile.name);
    assert_eq!(bytes.len(), total_len, "{} {id} {input:?}", profile.name);
}

#[track_caller]
fn check_save_padded(profile: &Profile, prefix: &str, total_len: usize) {
    let save = profile.save.unwrap_or_else(|| panic!("{} has no save command", profile.name));
    let bytes = profile.encode_command(&save).rivalcfg_bytes();
    assert!(bytes.starts_with(&hex(prefix)), "{} save: {bytes:02X?}", profile.name);
    assert_eq!(bytes.len(), total_len, "{} save", profile.name);
}

#[track_caller]
fn check_save(profile: &Profile, expected: &str) {
    let save = profile.save.unwrap_or_else(|| panic!("{} has no save command", profile.name));
    assert_eq!(profile.encode_command(&save).rivalcfg_bytes(), hex(expected), "{} save", profile.name);
}

#[track_caller]
fn check_battery(profile: &Profile, expected: &str) {
    let query = profile.battery.unwrap_or_else(|| panic!("{} has no battery query", profile.name));
    assert_eq!(profile.encode_command(&query.command).rivalcfg_bytes(), hex(expected), "{} battery", profile.name);
}

#[track_caller]
fn check_firmware(profile: &Profile, expected: &str) {
    let query = profile.firmware.unwrap_or_else(|| panic!("{} has no firmware query", profile.name));
    assert_eq!(profile.encode_command(&query.command).rivalcfg_bytes(), hex(expected), "{} firmware", profile.name);
}

// ---- AEROX_3 ---------------------------------------------------------------------------------

/// rivalcfg test/devices/old_specs/test_aerox3.py::TestDevice::test_save
#[test]
fn aerox3_save() {
    check_save(&AEROX_3, "02 00 11 00");
}

/// rivalcfg test/devices/old_specs/test_aerox3.py::TestDevice::test_set_buttons_mapping
#[test]
fn aerox3_set_buttons_mapping() {
    check(&AEROX_3, "buttons", Input::Buttons(&map(&[])), "02 00 2A 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 31 00 00 00 00 32 00 00 00 00");
    check(&AEROX_3, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 2A 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 31 00 00 00 00 32 00 00 00 00");
    check(&AEROX_3, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 2A 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 31 00 00 00 00 32 00 00 00 00");
    check(&AEROX_3, "buttons", Input::Buttons(&map(&[("ScrollUp", "ScrollDown"), ("ScrollDown", "ScrollUp")])), "02 00 2A 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 32 00 00 00 00 31 00 00 00 00");
}

/// rivalcfg test/devices/old_specs/test_aerox3.py::TestDevice::test_set_default_lighting
#[test]
fn aerox3_set_default_lighting() {
    check(&AEROX_3, "default_lighting", Input::Choice("off"), "02 00 27 00 00");
    check(&AEROX_3, "default_lighting", Input::Choice("reactive"), "02 00 27 00 01");
    check(&AEROX_3, "default_lighting", Input::Choice("rainbow"), "02 00 27 01 00");
    check(&AEROX_3, "default_lighting", Input::Choice("reactive-rainbow"), "02 00 27 01 01");
}

/// rivalcfg test/devices/old_specs/test_aerox3.py::TestDevice::test_set_led_brightness
#[test]
fn aerox3_set_led_brightness() {
    check(&AEROX_3, "led_brightness", Input::Int(0), "02 00 23 00");
    check(&AEROX_3, "led_brightness", Input::Int(100), "02 00 23 64");
}

/// rivalcfg test/devices/old_specs/test_aerox3.py::TestDevice::test_set_polling_rate
#[test]
fn aerox3_set_polling_rate() {
    check(&AEROX_3, "polling_rate", Input::Choice("125"), "02 00 2B 04");
    check(&AEROX_3, "polling_rate", Input::Choice("250"), "02 00 2B 03");
    check(&AEROX_3, "polling_rate", Input::Choice("500"), "02 00 2B 02");
    check(&AEROX_3, "polling_rate", Input::Choice("1000"), "02 00 2B 01");
}

/// rivalcfg test/devices/old_specs/test_aerox3.py::TestDevice::test_set_rainbow_effect
#[test]
fn aerox3_set_rainbow_effect() {
    check(&AEROX_3, "rainbow_effect", Input::Choice("all"), "02 00 22 07");
    check(&AEROX_3, "rainbow_effect", Input::Choice("bottom"), "02 00 22 04");
    check(&AEROX_3, "rainbow_effect", Input::Choice("middle"), "02 00 22 02");
    check(&AEROX_3, "rainbow_effect", Input::Choice("top"), "02 00 22 01");
    check(&AEROX_3, "rainbow_effect", Input::Choice("bottom-middle"), "02 00 22 06");
    check(&AEROX_3, "rainbow_effect", Input::Choice("middle-top"), "02 00 22 03");
    check(&AEROX_3, "rainbow_effect", Input::Choice("bottom-top"), "02 00 22 05");
}

/// rivalcfg test/devices/old_specs/test_aerox3.py::TestDevice::test_set_reactive_color
#[test]
fn aerox3_set_reactive_color() {
    check(&AEROX_3, "reactive_color", Input::Reactive(Some(rgb(0xFF0000))), "02 00 26 01 00 FF 00 00");
    check(&AEROX_3, "reactive_color", Input::Reactive(Some(rgb(0xFF1802))), "02 00 26 01 00 FF 18 02");
    check(&AEROX_3, "reactive_color", Input::Reactive(None), "02 00 26 00 00 00 00 00");
    check(&AEROX_3, "reactive_color", Input::Reactive(None), "02 00 26 00 00 00 00 00");
}

/// rivalcfg test/devices/old_specs/test_aerox3.py::TestDevice::test_set_sensitivity
#[test]
fn aerox3_set_sensitivity() {
    check(&AEROX_3, "sensitivity", Input::Dpi(&[(200, 200)]), "02 00 2D 01 01 04");
    check(&AEROX_3, "sensitivity", Input::Dpi(&[(200, 200)]), "02 00 2D 01 01 04");
    check(&AEROX_3, "sensitivity", Input::Dpi(&[(200, 200), (400, 400)]), "02 00 2D 02 01 04 08");
    check(&AEROX_3, "sensitivity", Input::Dpi(&[(200, 200), (400, 400), (800, 800), (1600, 1600)]), "02 00 2D 04 01 04 08 12 24");
}

/// rivalcfg test/devices/old_specs/test_aerox3.py::TestDevice::test_set_z1_color
#[test]
fn aerox3_set_z1_color() {
    check(&AEROX_3, "z1_color", Input::Color(rgb(0xABCDEF)), "02 00 21 01 AB CD EF");
    check(&AEROX_3, "z1_color", Input::Color(rgb(0xFF0000)), "02 00 21 01 FF 00 00");
}

/// rivalcfg test/devices/old_specs/test_aerox3.py::TestDevice::test_set_z2_color
#[test]
fn aerox3_set_z2_color() {
    check(&AEROX_3, "z2_color", Input::Color(rgb(0xABCDEF)), "02 00 21 02 00 00 00 AB CD EF");
    check(&AEROX_3, "z2_color", Input::Color(rgb(0xFF0000)), "02 00 21 02 00 00 00 FF 00 00");
}

/// rivalcfg test/devices/old_specs/test_aerox3.py::TestDevice::test_set_z3_color
#[test]
fn aerox3_set_z3_color() {
    check(&AEROX_3, "z3_color", Input::Color(rgb(0xABCDEF)), "02 00 21 04 00 00 00 00 00 00 AB CD EF");
    check(&AEROX_3, "z3_color", Input::Color(rgb(0xFF0000)), "02 00 21 04 00 00 00 00 00 00 FF 00 00");
}

/// Defaults of every `AEROX_3` setting and its save/battery/firmware commands, as rivalcfg f16c521
/// encodes them (computed by running rivalcfg, not copied from its test files).
#[test]
fn aerox3_rivalcfg_defaults() {
    check(&AEROX_3, "sensitivity", Input::Dpi(&[(800, 800), (1600, 1600)]), "02 00 2D 02 01 12 24");
    check(&AEROX_3, "polling_rate", Input::Choice("1000"), "02 00 2B 01");
    check(&AEROX_3, "z1_color", Input::Color(rgb(0xFF0000)), "02 00 21 01 FF 00 00");
    check(&AEROX_3, "z2_color", Input::Color(rgb(0x00FF00)), "02 00 21 02 00 00 00 00 FF 00");
    check(&AEROX_3, "z3_color", Input::Color(rgb(0x0000FF)), "02 00 21 04 00 00 00 00 00 00 00 00 FF");
    check(&AEROX_3, "reactive_color", Input::Reactive(None), "02 00 26 00 00 00 00 00");
    check(&AEROX_3, "led_brightness", Input::Int(100), "02 00 23 64");
    check(&AEROX_3, "rainbow_effect", Input::Choice("all"), "02 00 22 07");
    check(&AEROX_3, "buttons", Input::Buttons(&map(&[("button1", "button1"), ("button2", "button2"), ("button3", "button3"), ("button4", "button4"), ("button5", "button5"), ("button6", "dpi"), ("scrollup", "scrollup"), ("scrolldown", "scrolldown"), ("layout", "qwerty")])), "02 00 2A 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 31 00 00 00 00 32 00 00 00 00");
    check(&AEROX_3, "default_lighting", Input::Choice("rainbow"), "02 00 27 01 00");
    check_save(&AEROX_3, "02 00 11 00");
}

// ---- AEROX_3_WIRELESS_WIRED ------------------------------------------------------------------

/// rivalcfg test/devices/specs/aerox3_wireless_wired.txt, `sensitivity` cases (each followed by the CLI's save packet)
#[test]
fn aerox3_wireless_wired_spec_sensitivity() {
    check(&AEROX_3_WIRELESS_WIRED, "sensitivity", Input::Dpi(&[(100, 100)]), "02 00 2D 01 00 00");
    check_save(&AEROX_3_WIRELESS_WIRED, "02 00 11 00");
    check(&AEROX_3_WIRELESS_WIRED, "sensitivity", Input::Dpi(&[(200, 200)]), "02 00 2D 01 00 02");
    check(&AEROX_3_WIRELESS_WIRED, "sensitivity", Input::Dpi(&[(300, 300)]), "02 00 2D 01 00 03");
    check(&AEROX_3_WIRELESS_WIRED, "sensitivity", Input::Dpi(&[(18000, 18000)]), "02 00 2D 01 00 D6");
    check(&AEROX_3_WIRELESS_WIRED, "sensitivity", Input::Dpi(&[(200, 200), (400, 400)]), "02 00 2D 02 00 02 04");
    check(&AEROX_3_WIRELESS_WIRED, "sensitivity", Input::Dpi(&[(200, 200), (400, 400), (800, 800), (1600, 1600)]), "02 00 2D 04 00 02 04 09 12");
    check(&AEROX_3_WIRELESS_WIRED, "sensitivity", Input::Dpi(&[(200, 200), (400, 400), (800, 800), (1600, 1600), (18000, 18000)]), "02 00 2D 05 00 02 04 09 12 D6");
}

/// rivalcfg test/devices/specs/aerox3_wireless_wired.txt, `polling_rate` cases (each followed by the CLI's save packet)
#[test]
fn aerox3_wireless_wired_spec_polling_rate() {
    check(&AEROX_3_WIRELESS_WIRED, "polling_rate", Input::Choice("125"), "02 00 2B 03");
    check_save(&AEROX_3_WIRELESS_WIRED, "02 00 11 00");
    check(&AEROX_3_WIRELESS_WIRED, "polling_rate", Input::Choice("250"), "02 00 2B 02");
    check(&AEROX_3_WIRELESS_WIRED, "polling_rate", Input::Choice("500"), "02 00 2B 01");
    check(&AEROX_3_WIRELESS_WIRED, "polling_rate", Input::Choice("1000"), "02 00 2B 00");
}

/// rivalcfg test/devices/specs/aerox3_wireless_wired.txt, `z1_color` cases (each followed by the CLI's save packet)
#[test]
fn aerox3_wireless_wired_spec_z1_color() {
    check(&AEROX_3_WIRELESS_WIRED, "z1_color", Input::Color(rgb(0xABCDEF)), "02 00 21 01 00 AB CD EF");
    check_save(&AEROX_3_WIRELESS_WIRED, "02 00 11 00");
}

/// rivalcfg test/devices/specs/aerox3_wireless_wired.txt, `z2_color` cases (each followed by the CLI's save packet)
#[test]
fn aerox3_wireless_wired_spec_z2_color() {
    check(&AEROX_3_WIRELESS_WIRED, "z2_color", Input::Color(rgb(0xABCDEF)), "02 00 21 01 01 AB CD EF");
    check_save(&AEROX_3_WIRELESS_WIRED, "02 00 11 00");
}

/// rivalcfg test/devices/specs/aerox3_wireless_wired.txt, `z3_color` cases (each followed by the CLI's save packet)
#[test]
fn aerox3_wireless_wired_spec_z3_color() {
    check(&AEROX_3_WIRELESS_WIRED, "z3_color", Input::Color(rgb(0xABCDEF)), "02 00 21 01 02 AB CD EF");
    check_save(&AEROX_3_WIRELESS_WIRED, "02 00 11 00");
}

/// rivalcfg test/devices/specs/aerox3_wireless_wired.txt, `reactive_color` cases (each followed by the CLI's save packet)
#[test]
fn aerox3_wireless_wired_spec_reactive_color() {
    check(&AEROX_3_WIRELESS_WIRED, "reactive_color", Input::Reactive(Some(rgb(0xABCDEF))), "02 00 26 01 00 AB CD EF");
    check_save(&AEROX_3_WIRELESS_WIRED, "02 00 11 00");
    check(&AEROX_3_WIRELESS_WIRED, "reactive_color", Input::Reactive(None), "02 00 26 00 00 00 00 00");
}

/// rivalcfg test/devices/specs/aerox3_wireless_wired.txt, `sleep_timer` cases (each followed by the CLI's save packet)
#[test]
fn aerox3_wireless_wired_spec_sleep_timer() {
    check(&AEROX_3_WIRELESS_WIRED, "sleep_timer", Input::Int(0), "02 00 29 00 00 00");
    check_save(&AEROX_3_WIRELESS_WIRED, "02 00 11 00");
    check(&AEROX_3_WIRELESS_WIRED, "sleep_timer", Input::Int(1), "02 00 29 60 EA 00");
    check(&AEROX_3_WIRELESS_WIRED, "sleep_timer", Input::Int(5), "02 00 29 E0 93 04");
    check(&AEROX_3_WIRELESS_WIRED, "sleep_timer", Input::Int(20), "02 00 29 80 4F 12");
}

/// rivalcfg test/devices/specs/aerox3_wireless_wired.txt, `dim_timer` cases (each followed by the CLI's save packet)
#[test]
fn aerox3_wireless_wired_spec_dim_timer() {
    check(&AEROX_3_WIRELESS_WIRED, "dim_timer", Input::Int(0), "02 00 23 0F 01 00 00 00 00 00");
    check_save(&AEROX_3_WIRELESS_WIRED, "02 00 11 00");
    check(&AEROX_3_WIRELESS_WIRED, "dim_timer", Input::Int(30), "02 00 23 0F 01 00 00 30 75 00");
    check(&AEROX_3_WIRELESS_WIRED, "dim_timer", Input::Int(60), "02 00 23 0F 01 00 00 60 EA 00");
    check(&AEROX_3_WIRELESS_WIRED, "dim_timer", Input::Int(300), "02 00 23 0F 01 00 00 E0 93 04");
    check(&AEROX_3_WIRELESS_WIRED, "dim_timer", Input::Int(1200), "02 00 23 0F 01 00 00 80 4F 12");
}

/// rivalcfg test/devices/specs/aerox3_wireless_wired.txt, `buttons_mapping` cases (each followed by the CLI's save packet)
#[test]
fn aerox3_wireless_wired_spec_buttons_mapping() {
    check(&AEROX_3_WIRELESS_WIRED, "buttons", Input::Buttons(&map(&[])), "02 00 2A 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 31 00 00 00 00 32 00 00 00 00");
    check_save(&AEROX_3_WIRELESS_WIRED, "02 00 11 00");
    check(&AEROX_3_WIRELESS_WIRED, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 2A 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 31 00 00 00 00 32 00 00 00 00");
    check(&AEROX_3_WIRELESS_WIRED, "buttons", Input::Buttons(&map(&[("ScrollUp", "ScrollDown"), ("ScrollDown", "ScrollUp")])), "02 00 2A 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 32 00 00 00 00 31 00 00 00 00");
}

/// rivalcfg test/devices/specs/aerox3_wireless_wired.txt, `rainbow_effect` cases (each followed by the CLI's save packet)
#[test]
fn aerox3_wireless_wired_spec_rainbow_effect() {
    check(&AEROX_3_WIRELESS_WIRED, "rainbow_effect", Input::Trigger, "02 00 22 FF");
    check_save(&AEROX_3_WIRELESS_WIRED, "02 00 11 00");
}

/// rivalcfg test/devices/specs/aerox3_wireless_wired.txt, `default_lighting` cases (each followed by the CLI's save packet)
#[test]
fn aerox3_wireless_wired_spec_default_lighting() {
    check(&AEROX_3_WIRELESS_WIRED, "default_lighting", Input::Choice("off"), "02 00 27 00 00");
    check_save(&AEROX_3_WIRELESS_WIRED, "02 00 11 00");
    check(&AEROX_3_WIRELESS_WIRED, "default_lighting", Input::Choice("reactive"), "02 00 27 00 01");
    check(&AEROX_3_WIRELESS_WIRED, "default_lighting", Input::Choice("rainbow"), "02 00 27 01 00");
    check(&AEROX_3_WIRELESS_WIRED, "default_lighting", Input::Choice("reactive-rainbow"), "02 00 27 01 01");
}

/// rivalcfg test/devices/specs/aerox3_wireless_wired.txt, `battery_level` cases (each followed by the CLI's save packet)
#[test]
fn aerox3_wireless_wired_spec_battery_level() {
    check_battery(&AEROX_3_WIRELESS_WIRED, "02 00 92");
}

/// Defaults of every `AEROX_3_WIRELESS_WIRED` setting and its save/battery/firmware commands, as rivalcfg f16c521
/// encodes them (computed by running rivalcfg, not copied from its test files).
#[test]
fn aerox3_wireless_wired_rivalcfg_defaults() {
    check(&AEROX_3_WIRELESS_WIRED, "sensitivity", Input::Dpi(&[(400, 400), (800, 800), (1200, 1200), (2400, 2400), (3200, 3200)]), "02 00 2D 05 00 04 09 0D 1B 26");
    check(&AEROX_3_WIRELESS_WIRED, "polling_rate", Input::Choice("1000"), "02 00 2B 00");
    check(&AEROX_3_WIRELESS_WIRED, "z1_color", Input::Color(rgb(0xFF0000)), "02 00 21 01 00 FF 00 00");
    check(&AEROX_3_WIRELESS_WIRED, "z2_color", Input::Color(rgb(0x00FF00)), "02 00 21 01 01 00 FF 00");
    check(&AEROX_3_WIRELESS_WIRED, "z3_color", Input::Color(rgb(0x0000FF)), "02 00 21 01 02 00 00 FF");
    check(&AEROX_3_WIRELESS_WIRED, "reactive_color", Input::Reactive(None), "02 00 26 00 00 00 00 00");
    check(&AEROX_3_WIRELESS_WIRED, "sleep_timer", Input::Int(5), "02 00 29 E0 93 04");
    check(&AEROX_3_WIRELESS_WIRED, "dim_timer", Input::Int(30), "02 00 23 0F 01 00 00 30 75 00");
    check(&AEROX_3_WIRELESS_WIRED, "buttons", Input::Buttons(&map(&[("button1", "button1"), ("button2", "button2"), ("button3", "button3"), ("button4", "button4"), ("button5", "button5"), ("button6", "dpi"), ("scrollup", "scrollup"), ("scrolldown", "scrolldown"), ("layout", "qwerty")])), "02 00 2A 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 31 00 00 00 00 32 00 00 00 00");
    check(&AEROX_3_WIRELESS_WIRED, "rainbow_effect", Input::Trigger, "02 00 22 FF");
    check(&AEROX_3_WIRELESS_WIRED, "default_lighting", Input::Choice("rainbow"), "02 00 27 01 00");
    check_save(&AEROX_3_WIRELESS_WIRED, "02 00 11 00");
    check_battery(&AEROX_3_WIRELESS_WIRED, "02 00 92");
}

// ---- AEROX_3_WIRELESS_WIRELESS ---------------------------------------------------------------

/// rivalcfg test/devices/specs/aerox3_wireless_wireless.txt, `sensitivity` cases (each followed by the CLI's save packet)
#[test]
fn aerox3_wireless_wireless_spec_sensitivity() {
    check(&AEROX_3_WIRELESS_WIRELESS, "sensitivity", Input::Dpi(&[(100, 100)]), "02 00 6D 01 00 00");
    check_save(&AEROX_3_WIRELESS_WIRELESS, "02 00 51 00");
    check(&AEROX_3_WIRELESS_WIRELESS, "sensitivity", Input::Dpi(&[(200, 200)]), "02 00 6D 01 00 02");
    check(&AEROX_3_WIRELESS_WIRELESS, "sensitivity", Input::Dpi(&[(300, 300)]), "02 00 6D 01 00 03");
    check(&AEROX_3_WIRELESS_WIRELESS, "sensitivity", Input::Dpi(&[(18000, 18000)]), "02 00 6D 01 00 D6");
    check(&AEROX_3_WIRELESS_WIRELESS, "sensitivity", Input::Dpi(&[(200, 200), (400, 400)]), "02 00 6D 02 00 02 04");
    check(&AEROX_3_WIRELESS_WIRELESS, "sensitivity", Input::Dpi(&[(200, 200), (400, 400), (800, 800), (1600, 1600)]), "02 00 6D 04 00 02 04 09 12");
    check(&AEROX_3_WIRELESS_WIRELESS, "sensitivity", Input::Dpi(&[(200, 200), (400, 400), (800, 800), (1600, 1600), (18000, 18000)]), "02 00 6D 05 00 02 04 09 12 D6");
}

/// rivalcfg test/devices/specs/aerox3_wireless_wireless.txt, `polling_rate` cases (each followed by the CLI's save packet)
#[test]
fn aerox3_wireless_wireless_spec_polling_rate() {
    check(&AEROX_3_WIRELESS_WIRELESS, "polling_rate", Input::Choice("125"), "02 00 6B 03");
    check_save(&AEROX_3_WIRELESS_WIRELESS, "02 00 51 00");
    check(&AEROX_3_WIRELESS_WIRELESS, "polling_rate", Input::Choice("250"), "02 00 6B 02");
    check(&AEROX_3_WIRELESS_WIRELESS, "polling_rate", Input::Choice("500"), "02 00 6B 01");
    check(&AEROX_3_WIRELESS_WIRELESS, "polling_rate", Input::Choice("1000"), "02 00 6B 00");
}

/// rivalcfg test/devices/specs/aerox3_wireless_wireless.txt, `z1_color` cases (each followed by the CLI's save packet)
#[test]
fn aerox3_wireless_wireless_spec_z1_color() {
    check(&AEROX_3_WIRELESS_WIRELESS, "z1_color", Input::Color(rgb(0xABCDEF)), "02 00 61 01 00 AB CD EF");
    check_save(&AEROX_3_WIRELESS_WIRELESS, "02 00 51 00");
}

/// rivalcfg test/devices/specs/aerox3_wireless_wireless.txt, `z2_color` cases (each followed by the CLI's save packet)
#[test]
fn aerox3_wireless_wireless_spec_z2_color() {
    check(&AEROX_3_WIRELESS_WIRELESS, "z2_color", Input::Color(rgb(0xABCDEF)), "02 00 61 01 01 AB CD EF");
    check_save(&AEROX_3_WIRELESS_WIRELESS, "02 00 51 00");
}

/// rivalcfg test/devices/specs/aerox3_wireless_wireless.txt, `z3_color` cases (each followed by the CLI's save packet)
#[test]
fn aerox3_wireless_wireless_spec_z3_color() {
    check(&AEROX_3_WIRELESS_WIRELESS, "z3_color", Input::Color(rgb(0xABCDEF)), "02 00 61 01 02 AB CD EF");
    check_save(&AEROX_3_WIRELESS_WIRELESS, "02 00 51 00");
}

/// rivalcfg test/devices/specs/aerox3_wireless_wireless.txt, `reactive_color` cases (each followed by the CLI's save packet)
#[test]
fn aerox3_wireless_wireless_spec_reactive_color() {
    check(&AEROX_3_WIRELESS_WIRELESS, "reactive_color", Input::Reactive(Some(rgb(0xABCDEF))), "02 00 66 01 00 AB CD EF");
    check_save(&AEROX_3_WIRELESS_WIRELESS, "02 00 51 00");
    check(&AEROX_3_WIRELESS_WIRELESS, "reactive_color", Input::Reactive(None), "02 00 66 00 00 00 00 00");
}

/// rivalcfg test/devices/specs/aerox3_wireless_wireless.txt, `sleep_timer` cases (each followed by the CLI's save packet)
#[test]
fn aerox3_wireless_wireless_spec_sleep_timer() {
    check(&AEROX_3_WIRELESS_WIRELESS, "sleep_timer", Input::Int(0), "02 00 69 00 00 00");
    check_save(&AEROX_3_WIRELESS_WIRELESS, "02 00 51 00");
    check(&AEROX_3_WIRELESS_WIRELESS, "sleep_timer", Input::Int(1), "02 00 69 60 EA 00");
    check(&AEROX_3_WIRELESS_WIRELESS, "sleep_timer", Input::Int(5), "02 00 69 E0 93 04");
    check(&AEROX_3_WIRELESS_WIRELESS, "sleep_timer", Input::Int(20), "02 00 69 80 4F 12");
}

/// rivalcfg test/devices/specs/aerox3_wireless_wireless.txt, `dim_timer` cases (each followed by the CLI's save packet)
#[test]
fn aerox3_wireless_wireless_spec_dim_timer() {
    check(&AEROX_3_WIRELESS_WIRELESS, "dim_timer", Input::Int(0), "02 00 63 0F 01 00 00 00 00 00");
    check_save(&AEROX_3_WIRELESS_WIRELESS, "02 00 51 00");
    check(&AEROX_3_WIRELESS_WIRELESS, "dim_timer", Input::Int(30), "02 00 63 0F 01 00 00 30 75 00");
    check(&AEROX_3_WIRELESS_WIRELESS, "dim_timer", Input::Int(60), "02 00 63 0F 01 00 00 60 EA 00");
    check(&AEROX_3_WIRELESS_WIRELESS, "dim_timer", Input::Int(300), "02 00 63 0F 01 00 00 E0 93 04");
    check(&AEROX_3_WIRELESS_WIRELESS, "dim_timer", Input::Int(1200), "02 00 63 0F 01 00 00 80 4F 12");
}

/// rivalcfg test/devices/specs/aerox3_wireless_wireless.txt, `buttons_mapping` cases (each followed by the CLI's save packet)
#[test]
fn aerox3_wireless_wireless_spec_buttons_mapping() {
    check(&AEROX_3_WIRELESS_WIRELESS, "buttons", Input::Buttons(&map(&[])), "02 00 6A 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 31 00 00 00 00 32 00 00 00 00");
    check_save(&AEROX_3_WIRELESS_WIRELESS, "02 00 51 00");
    check(&AEROX_3_WIRELESS_WIRELESS, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 6A 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 31 00 00 00 00 32 00 00 00 00");
    check(&AEROX_3_WIRELESS_WIRELESS, "buttons", Input::Buttons(&map(&[("ScrollUp", "ScrollDown"), ("ScrollDown", "ScrollUp")])), "02 00 6A 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 32 00 00 00 00 31 00 00 00 00");
}

/// rivalcfg test/devices/specs/aerox3_wireless_wireless.txt, `rainbow_effect` cases (each followed by the CLI's save packet)
#[test]
fn aerox3_wireless_wireless_spec_rainbow_effect() {
    check(&AEROX_3_WIRELESS_WIRELESS, "rainbow_effect", Input::Trigger, "02 00 62 FF");
    check_save(&AEROX_3_WIRELESS_WIRELESS, "02 00 51 00");
}

/// rivalcfg test/devices/specs/aerox3_wireless_wireless.txt, `default_lighting` cases (each followed by the CLI's save packet)
#[test]
fn aerox3_wireless_wireless_spec_default_lighting() {
    check(&AEROX_3_WIRELESS_WIRELESS, "default_lighting", Input::Choice("off"), "02 00 67 00 00");
    check_save(&AEROX_3_WIRELESS_WIRELESS, "02 00 51 00");
    check(&AEROX_3_WIRELESS_WIRELESS, "default_lighting", Input::Choice("reactive"), "02 00 67 00 01");
    check(&AEROX_3_WIRELESS_WIRELESS, "default_lighting", Input::Choice("rainbow"), "02 00 67 01 00");
    check(&AEROX_3_WIRELESS_WIRELESS, "default_lighting", Input::Choice("reactive-rainbow"), "02 00 67 01 01");
}

/// rivalcfg test/devices/specs/aerox3_wireless_wireless.txt, `battery_level` cases (each followed by the CLI's save packet)
#[test]
fn aerox3_wireless_wireless_spec_battery_level() {
    check_battery(&AEROX_3_WIRELESS_WIRELESS, "02 00 D2");
}

/// Defaults of every `AEROX_3_WIRELESS_WIRELESS` setting and its save/battery/firmware commands, as rivalcfg f16c521
/// encodes them (computed by running rivalcfg, not copied from its test files).
#[test]
fn aerox3_wireless_wireless_rivalcfg_defaults() {
    check(&AEROX_3_WIRELESS_WIRELESS, "sensitivity", Input::Dpi(&[(400, 400), (800, 800), (1200, 1200), (2400, 2400), (3200, 3200)]), "02 00 6D 05 00 04 09 0D 1B 26");
    check(&AEROX_3_WIRELESS_WIRELESS, "polling_rate", Input::Choice("1000"), "02 00 6B 00");
    check(&AEROX_3_WIRELESS_WIRELESS, "z1_color", Input::Color(rgb(0xFF0000)), "02 00 61 01 00 FF 00 00");
    check(&AEROX_3_WIRELESS_WIRELESS, "z2_color", Input::Color(rgb(0x00FF00)), "02 00 61 01 01 00 FF 00");
    check(&AEROX_3_WIRELESS_WIRELESS, "z3_color", Input::Color(rgb(0x0000FF)), "02 00 61 01 02 00 00 FF");
    check(&AEROX_3_WIRELESS_WIRELESS, "reactive_color", Input::Reactive(None), "02 00 66 00 00 00 00 00");
    check(&AEROX_3_WIRELESS_WIRELESS, "sleep_timer", Input::Int(5), "02 00 69 E0 93 04");
    check(&AEROX_3_WIRELESS_WIRELESS, "dim_timer", Input::Int(30), "02 00 63 0F 01 00 00 30 75 00");
    check(&AEROX_3_WIRELESS_WIRELESS, "buttons", Input::Buttons(&map(&[("button1", "button1"), ("button2", "button2"), ("button3", "button3"), ("button4", "button4"), ("button5", "button5"), ("button6", "dpi"), ("scrollup", "scrollup"), ("scrolldown", "scrolldown"), ("layout", "qwerty")])), "02 00 6A 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 31 00 00 00 00 32 00 00 00 00");
    check(&AEROX_3_WIRELESS_WIRELESS, "rainbow_effect", Input::Trigger, "02 00 62 FF");
    check(&AEROX_3_WIRELESS_WIRELESS, "default_lighting", Input::Choice("rainbow"), "02 00 67 01 00");
    check_save(&AEROX_3_WIRELESS_WIRELESS, "02 00 51 00");
    check_battery(&AEROX_3_WIRELESS_WIRELESS, "02 00 D2");
}

// ---- AEROX_5 ---------------------------------------------------------------------------------

/// rivalcfg test/devices/old_specs/test_aerox5.py::TestDevice::test_save
#[test]
fn aerox5_save() {
    check_save(&AEROX_5, "02 00 11 00");
}

/// rivalcfg test/devices/old_specs/test_aerox5.py::TestDevice::test_set_buttons_mapping
#[test]
fn aerox5_set_buttons_mapping() {
    check(&AEROX_5, "buttons", Input::Buttons(&map(&[])), "02 00 2A 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 31 00 00 00 00 32 00 00 00 00");
    check(&AEROX_5, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 2A 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 31 00 00 00 00 32 00 00 00 00");
    check(&AEROX_5, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 2A 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 31 00 00 00 00 32 00 00 00 00");
    check(&AEROX_5, "buttons", Input::Buttons(&map(&[("ScrollUp", "ScrollDown"), ("ScrollDown", "ScrollUp")])), "02 00 2A 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 32 00 00 00 00 31 00 00 00 00");
}

/// rivalcfg test/devices/old_specs/test_aerox5.py::TestDevice::test_set_default_lighting
#[test]
fn aerox5_set_default_lighting() {
    check(&AEROX_5, "default_lighting", Input::Choice("off"), "02 00 27 00 00");
    check(&AEROX_5, "default_lighting", Input::Choice("reactive"), "02 00 27 00 01");
    check(&AEROX_5, "default_lighting", Input::Choice("rainbow"), "02 00 27 01 00");
    check(&AEROX_5, "default_lighting", Input::Choice("reactive-rainbow"), "02 00 27 01 01");
}

/// rivalcfg test/devices/old_specs/test_aerox5.py::TestDevice::test_set_led_brightness
#[test]
fn aerox5_set_led_brightness() {
    check(&AEROX_5, "led_brightness", Input::Int(0), "02 00 23 00");
    check(&AEROX_5, "led_brightness", Input::Int(50), "02 00 23 32");
    check(&AEROX_5, "led_brightness", Input::Int(100), "02 00 23 64");
}

/// rivalcfg test/devices/old_specs/test_aerox5.py::TestDevice::test_set_polling_rate
#[test]
fn aerox5_set_polling_rate() {
    check(&AEROX_5, "polling_rate", Input::Choice("125"), "02 00 2B 04");
    check(&AEROX_5, "polling_rate", Input::Choice("250"), "02 00 2B 03");
    check(&AEROX_5, "polling_rate", Input::Choice("500"), "02 00 2B 02");
    check(&AEROX_5, "polling_rate", Input::Choice("1000"), "02 00 2B 01");
}

/// rivalcfg test/devices/old_specs/test_aerox5.py::TestDevice::test_set_rainbow_effect
#[test]
fn aerox5_set_rainbow_effect() {
    check(&AEROX_5, "rainbow_effect", Input::Choice("all"), "02 00 22 07");
    check(&AEROX_5, "rainbow_effect", Input::Choice("bottom"), "02 00 22 04");
    check(&AEROX_5, "rainbow_effect", Input::Choice("middle"), "02 00 22 02");
    check(&AEROX_5, "rainbow_effect", Input::Choice("top"), "02 00 22 01");
    check(&AEROX_5, "rainbow_effect", Input::Choice("bottom-middle"), "02 00 22 06");
    check(&AEROX_5, "rainbow_effect", Input::Choice("middle-top"), "02 00 22 03");
    check(&AEROX_5, "rainbow_effect", Input::Choice("bottom-top"), "02 00 22 05");
}

/// rivalcfg test/devices/old_specs/test_aerox5.py::TestDevice::test_set_reactive_color
#[test]
fn aerox5_set_reactive_color() {
    check(&AEROX_5, "reactive_color", Input::Reactive(Some(rgb(0xFF0000))), "02 00 26 01 00 FF 00 00");
    check(&AEROX_5, "reactive_color", Input::Reactive(Some(rgb(0xFF1802))), "02 00 26 01 00 FF 18 02");
    check(&AEROX_5, "reactive_color", Input::Reactive(None), "02 00 26 00 00 00 00 00");
    check(&AEROX_5, "reactive_color", Input::Reactive(None), "02 00 26 00 00 00 00 00");
}

/// rivalcfg test/devices/old_specs/test_aerox5.py::TestDevice::test_set_sensitivity
#[test]
fn aerox5_set_sensitivity() {
    check(&AEROX_5, "sensitivity", Input::Dpi(&[(100, 100)]), "02 00 2D 01 00 00");
    check(&AEROX_5, "sensitivity", Input::Dpi(&[(200, 200)]), "02 00 2D 01 00 02");
    check(&AEROX_5, "sensitivity", Input::Dpi(&[(300, 300)]), "02 00 2D 01 00 03");
    check(&AEROX_5, "sensitivity", Input::Dpi(&[(18000, 18000)]), "02 00 2D 01 00 D6");
    check(&AEROX_5, "sensitivity", Input::Dpi(&[(200, 200), (400, 400)]), "02 00 2D 02 00 02 04");
    check(&AEROX_5, "sensitivity", Input::Dpi(&[(200, 200), (400, 400), (800, 800), (1600, 1600)]), "02 00 2D 04 00 02 04 09 12");
}

/// rivalcfg test/devices/old_specs/test_aerox5.py::TestDevice::test_set_z1_color
#[test]
fn aerox5_set_z1_color() {
    check(&AEROX_5, "z1_color", Input::Color(rgb(0xABCDEF)), "02 00 21 01 AB CD EF");
    check(&AEROX_5, "z1_color", Input::Color(rgb(0xFF0000)), "02 00 21 01 FF 00 00");
}

/// rivalcfg test/devices/old_specs/test_aerox5.py::TestDevice::test_set_z2_color
#[test]
fn aerox5_set_z2_color() {
    check(&AEROX_5, "z2_color", Input::Color(rgb(0xABCDEF)), "02 00 21 02 00 00 00 AB CD EF");
    check(&AEROX_5, "z2_color", Input::Color(rgb(0xFF0000)), "02 00 21 02 00 00 00 FF 00 00");
}

/// rivalcfg test/devices/old_specs/test_aerox5.py::TestDevice::test_set_z3_color
#[test]
fn aerox5_set_z3_color() {
    check(&AEROX_5, "z3_color", Input::Color(rgb(0xABCDEF)), "02 00 21 04 00 00 00 00 00 00 AB CD EF");
    check(&AEROX_5, "z3_color", Input::Color(rgb(0xFF0000)), "02 00 21 04 00 00 00 00 00 00 FF 00 00");
}

/// Defaults of every `AEROX_5` setting and its save/battery/firmware commands, as rivalcfg f16c521
/// encodes them (computed by running rivalcfg, not copied from its test files).
#[test]
fn aerox5_rivalcfg_defaults() {
    check(&AEROX_5, "sensitivity", Input::Dpi(&[(400, 400), (800, 800), (1200, 1200), (2400, 2400), (3200, 3200)]), "02 00 2D 05 00 04 09 0D 1B 26");
    check(&AEROX_5, "polling_rate", Input::Choice("1000"), "02 00 2B 01");
    check(&AEROX_5, "z1_color", Input::Color(rgb(0xFF0000)), "02 00 21 01 FF 00 00");
    check(&AEROX_5, "z2_color", Input::Color(rgb(0x00FF00)), "02 00 21 02 00 00 00 00 FF 00");
    check(&AEROX_5, "z3_color", Input::Color(rgb(0x0000FF)), "02 00 21 04 00 00 00 00 00 00 00 00 FF");
    check(&AEROX_5, "reactive_color", Input::Reactive(None), "02 00 26 00 00 00 00 00");
    check(&AEROX_5, "led_brightness", Input::Int(100), "02 00 23 64");
    check(&AEROX_5, "rainbow_effect", Input::Choice("all"), "02 00 22 07");
    check(&AEROX_5, "buttons", Input::Buttons(&map(&[("button1", "button1"), ("button2", "button2"), ("button3", "button3"), ("button4", "button4"), ("button5", "button5"), ("button6", "dpi"), ("button7", "disabled"), ("button8", "disabled"), ("button9", "disabled"), ("scrollup", "scrollup"), ("scrolldown", "scrolldown"), ("layout", "qwerty")])), "02 00 2A 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 31 00 00 00 00 32 00 00 00 00");
    check(&AEROX_5, "default_lighting", Input::Choice("rainbow"), "02 00 27 01 00");
    check_save(&AEROX_5, "02 00 11 00");
}

// ---- AEROX_5_WIRELESS_WIRED ------------------------------------------------------------------

/// rivalcfg test/devices/old_specs/test_aerox5_wireless_wired.py::TestDevice::test_battery_level
#[test]
fn aerox5_wireless_wired_battery_level() {
    check_battery(&AEROX_5_WIRELESS_WIRED, "02 00 92");
}

/// rivalcfg test/devices/old_specs/test_aerox5_wireless_wired.py::TestDevice::test_save
#[test]
fn aerox5_wireless_wired_save() {
    check_save(&AEROX_5_WIRELESS_WIRED, "02 00 11 00");
}

/// rivalcfg test/devices/old_specs/test_aerox5_wireless_wired.py::TestDevice::test_set_buttons_mapping
#[test]
fn aerox5_wireless_wired_set_buttons_mapping() {
    check(&AEROX_5_WIRELESS_WIRED, "buttons", Input::Buttons(&map(&[])), "02 00 2A 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 31 00 00 00 00 32 00 00 00 00");
    check(&AEROX_5_WIRELESS_WIRED, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 2A 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 31 00 00 00 00 32 00 00 00 00");
    check(&AEROX_5_WIRELESS_WIRED, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 2A 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 31 00 00 00 00 32 00 00 00 00");
    check(&AEROX_5_WIRELESS_WIRED, "buttons", Input::Buttons(&map(&[("ScrollUp", "ScrollDown"), ("ScrollDown", "ScrollUp")])), "02 00 2A 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 32 00 00 00 00 31 00 00 00 00");
}

/// rivalcfg test/devices/old_specs/test_aerox5_wireless_wired.py::TestDevice::test_set_default_lighting
#[test]
fn aerox5_wireless_wired_set_default_lighting() {
    check(&AEROX_5_WIRELESS_WIRED, "default_lighting", Input::Choice("off"), "02 00 27 00 00");
    check(&AEROX_5_WIRELESS_WIRED, "default_lighting", Input::Choice("reactive"), "02 00 27 00 01");
    check(&AEROX_5_WIRELESS_WIRED, "default_lighting", Input::Choice("rainbow"), "02 00 27 01 00");
    check(&AEROX_5_WIRELESS_WIRED, "default_lighting", Input::Choice("reactive-rainbow"), "02 00 27 01 01");
}

/// rivalcfg test/devices/old_specs/test_aerox5_wireless_wired.py::TestDevice::test_set_dim_timer
#[test]
fn aerox5_wireless_wired_set_dim_timer() {
    check(&AEROX_5_WIRELESS_WIRED, "dim_timer", Input::Int(0), "02 00 23 0F 01 00 00 00 00 00");
    check(&AEROX_5_WIRELESS_WIRED, "dim_timer", Input::Int(30), "02 00 23 0F 01 00 00 30 75 00");
    check(&AEROX_5_WIRELESS_WIRED, "dim_timer", Input::Int(60), "02 00 23 0F 01 00 00 60 EA 00");
    check(&AEROX_5_WIRELESS_WIRED, "dim_timer", Input::Int(300), "02 00 23 0F 01 00 00 E0 93 04");
    check(&AEROX_5_WIRELESS_WIRED, "dim_timer", Input::Int(1200), "02 00 23 0F 01 00 00 80 4F 12");
}

/// rivalcfg test/devices/old_specs/test_aerox5_wireless_wired.py::TestDevice::test_set_polling_rate
#[test]
fn aerox5_wireless_wired_set_polling_rate() {
    check(&AEROX_5_WIRELESS_WIRED, "polling_rate", Input::Choice("125"), "02 00 2B 03");
    check(&AEROX_5_WIRELESS_WIRED, "polling_rate", Input::Choice("250"), "02 00 2B 02");
    check(&AEROX_5_WIRELESS_WIRED, "polling_rate", Input::Choice("500"), "02 00 2B 01");
    check(&AEROX_5_WIRELESS_WIRED, "polling_rate", Input::Choice("1000"), "02 00 2B 00");
}

/// rivalcfg test/devices/old_specs/test_aerox5_wireless_wired.py::TestDevice::test_set_rainbow_effect
#[test]
fn aerox5_wireless_wired_set_rainbow_effect() {
    check(&AEROX_5_WIRELESS_WIRED, "rainbow_effect", Input::Trigger, "02 00 22 FF");
}

/// rivalcfg test/devices/old_specs/test_aerox5_wireless_wired.py::TestDevice::test_set_reactive_color
#[test]
fn aerox5_wireless_wired_set_reactive_color() {
    check(&AEROX_5_WIRELESS_WIRED, "reactive_color", Input::Reactive(Some(rgb(0xFF0000))), "02 00 26 01 00 FF 00 00");
    check(&AEROX_5_WIRELESS_WIRED, "reactive_color", Input::Reactive(Some(rgb(0xFF1802))), "02 00 26 01 00 FF 18 02");
    check(&AEROX_5_WIRELESS_WIRED, "reactive_color", Input::Reactive(None), "02 00 26 00 00 00 00 00");
    check(&AEROX_5_WIRELESS_WIRED, "reactive_color", Input::Reactive(None), "02 00 26 00 00 00 00 00");
}

/// rivalcfg test/devices/old_specs/test_aerox5_wireless_wired.py::TestDevice::test_set_sensitivity
#[test]
fn aerox5_wireless_wired_set_sensitivity() {
    check(&AEROX_5_WIRELESS_WIRED, "sensitivity", Input::Dpi(&[(100, 100)]), "02 00 2D 01 00 00");
    check(&AEROX_5_WIRELESS_WIRED, "sensitivity", Input::Dpi(&[(200, 200)]), "02 00 2D 01 00 02");
    check(&AEROX_5_WIRELESS_WIRED, "sensitivity", Input::Dpi(&[(300, 300)]), "02 00 2D 01 00 03");
    check(&AEROX_5_WIRELESS_WIRED, "sensitivity", Input::Dpi(&[(18000, 18000)]), "02 00 2D 01 00 D6");
    check(&AEROX_5_WIRELESS_WIRED, "sensitivity", Input::Dpi(&[(200, 200), (400, 400)]), "02 00 2D 02 00 02 04");
    check(&AEROX_5_WIRELESS_WIRED, "sensitivity", Input::Dpi(&[(200, 200), (400, 400), (800, 800), (1600, 1600)]), "02 00 2D 04 00 02 04 09 12");
}

/// rivalcfg test/devices/old_specs/test_aerox5_wireless_wired.py::TestDevice::test_set_sleep_timer
#[test]
fn aerox5_wireless_wired_set_sleep_timer() {
    check(&AEROX_5_WIRELESS_WIRED, "sleep_timer", Input::Int(0), "02 00 29 00 00 00");
    check(&AEROX_5_WIRELESS_WIRED, "sleep_timer", Input::Int(1), "02 00 29 60 EA 00");
    check(&AEROX_5_WIRELESS_WIRED, "sleep_timer", Input::Int(5), "02 00 29 E0 93 04");
    check(&AEROX_5_WIRELESS_WIRED, "sleep_timer", Input::Int(20), "02 00 29 80 4F 12");
}

/// rivalcfg test/devices/old_specs/test_aerox5_wireless_wired.py::TestDevice::test_set_z1_color
#[test]
fn aerox5_wireless_wired_set_z1_color() {
    check(&AEROX_5_WIRELESS_WIRED, "z1_color", Input::Color(rgb(0xABCDEF)), "02 00 21 01 00 AB CD EF");
    check(&AEROX_5_WIRELESS_WIRED, "z1_color", Input::Color(rgb(0xFF0000)), "02 00 21 01 00 FF 00 00");
}

/// rivalcfg test/devices/old_specs/test_aerox5_wireless_wired.py::TestDevice::test_set_z2_color
#[test]
fn aerox5_wireless_wired_set_z2_color() {
    check(&AEROX_5_WIRELESS_WIRED, "z2_color", Input::Color(rgb(0xABCDEF)), "02 00 21 01 01 AB CD EF");
    check(&AEROX_5_WIRELESS_WIRED, "z2_color", Input::Color(rgb(0xFF0000)), "02 00 21 01 01 FF 00 00");
}

/// rivalcfg test/devices/old_specs/test_aerox5_wireless_wired.py::TestDevice::test_set_z3_color
#[test]
fn aerox5_wireless_wired_set_z3_color() {
    check(&AEROX_5_WIRELESS_WIRED, "z3_color", Input::Color(rgb(0xABCDEF)), "02 00 21 01 02 AB CD EF");
    check(&AEROX_5_WIRELESS_WIRED, "z3_color", Input::Color(rgb(0xFF0000)), "02 00 21 01 02 FF 00 00");
}

/// Defaults of every `AEROX_5_WIRELESS_WIRED` setting and its save/battery/firmware commands, as rivalcfg f16c521
/// encodes them (computed by running rivalcfg, not copied from its test files).
#[test]
fn aerox5_wireless_wired_rivalcfg_defaults() {
    check(&AEROX_5_WIRELESS_WIRED, "sensitivity", Input::Dpi(&[(400, 400), (800, 800), (1200, 1200), (2400, 2400), (3200, 3200)]), "02 00 2D 05 00 04 09 0D 1B 26");
    check(&AEROX_5_WIRELESS_WIRED, "polling_rate", Input::Choice("1000"), "02 00 2B 00");
    check(&AEROX_5_WIRELESS_WIRED, "z1_color", Input::Color(rgb(0xFF0000)), "02 00 21 01 00 FF 00 00");
    check(&AEROX_5_WIRELESS_WIRED, "z2_color", Input::Color(rgb(0x00FF00)), "02 00 21 01 01 00 FF 00");
    check(&AEROX_5_WIRELESS_WIRED, "z3_color", Input::Color(rgb(0x0000FF)), "02 00 21 01 02 00 00 FF");
    check(&AEROX_5_WIRELESS_WIRED, "reactive_color", Input::Reactive(None), "02 00 26 00 00 00 00 00");
    check(&AEROX_5_WIRELESS_WIRED, "sleep_timer", Input::Int(5), "02 00 29 E0 93 04");
    check(&AEROX_5_WIRELESS_WIRED, "dim_timer", Input::Int(30), "02 00 23 0F 01 00 00 30 75 00");
    check(&AEROX_5_WIRELESS_WIRED, "buttons", Input::Buttons(&map(&[("button1", "button1"), ("button2", "button2"), ("button3", "button3"), ("button4", "button4"), ("button5", "button5"), ("button6", "dpi"), ("button7", "disabled"), ("button8", "disabled"), ("button9", "disabled"), ("scrollup", "scrollup"), ("scrolldown", "scrolldown"), ("layout", "qwerty")])), "02 00 2A 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 31 00 00 00 00 32 00 00 00 00");
    check(&AEROX_5_WIRELESS_WIRED, "rainbow_effect", Input::Trigger, "02 00 22 FF");
    check(&AEROX_5_WIRELESS_WIRED, "default_lighting", Input::Choice("rainbow"), "02 00 27 01 00");
    check_save(&AEROX_5_WIRELESS_WIRED, "02 00 11 00");
    check_battery(&AEROX_5_WIRELESS_WIRED, "02 00 92");
}

// ---- AEROX_5_WIRELESS_WIRELESS ---------------------------------------------------------------

/// rivalcfg test/devices/old_specs/test_aerox5_wireless_wireless.py::TestDevice::test_battery_level
#[test]
fn aerox5_wireless_wireless_battery_level() {
    check_battery(&AEROX_5_WIRELESS_WIRELESS, "02 00 D2");
}

/// rivalcfg test/devices/old_specs/test_aerox5_wireless_wireless.py::TestDevice::test_save
#[test]
fn aerox5_wireless_wireless_save() {
    check_save(&AEROX_5_WIRELESS_WIRELESS, "02 00 51 00");
}

/// rivalcfg test/devices/old_specs/test_aerox5_wireless_wireless.py::TestDevice::test_set_buttons_mapping
#[test]
fn aerox5_wireless_wireless_set_buttons_mapping() {
    check(&AEROX_5_WIRELESS_WIRELESS, "buttons", Input::Buttons(&map(&[])), "02 00 6A 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 31 00 00 00 00 32 00 00 00 00");
    check(&AEROX_5_WIRELESS_WIRELESS, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 6A 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 31 00 00 00 00 32 00 00 00 00");
    check(&AEROX_5_WIRELESS_WIRELESS, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 6A 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 31 00 00 00 00 32 00 00 00 00");
    check(&AEROX_5_WIRELESS_WIRELESS, "buttons", Input::Buttons(&map(&[("ScrollUp", "ScrollDown"), ("ScrollDown", "ScrollUp")])), "02 00 6A 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 32 00 00 00 00 31 00 00 00 00");
}

/// rivalcfg test/devices/old_specs/test_aerox5_wireless_wireless.py::TestDevice::test_set_default_lighting
#[test]
fn aerox5_wireless_wireless_set_default_lighting() {
    check(&AEROX_5_WIRELESS_WIRELESS, "default_lighting", Input::Choice("off"), "02 00 67 00 00");
    check(&AEROX_5_WIRELESS_WIRELESS, "default_lighting", Input::Choice("reactive"), "02 00 67 00 01");
    check(&AEROX_5_WIRELESS_WIRELESS, "default_lighting", Input::Choice("rainbow"), "02 00 67 01 00");
    check(&AEROX_5_WIRELESS_WIRELESS, "default_lighting", Input::Choice("reactive-rainbow"), "02 00 67 01 01");
}

/// rivalcfg test/devices/old_specs/test_aerox5_wireless_wireless.py::TestDevice::test_set_dim_timer
#[test]
fn aerox5_wireless_wireless_set_dim_timer() {
    check(&AEROX_5_WIRELESS_WIRELESS, "dim_timer", Input::Int(0), "02 00 63 0F 01 00 00 00 00 00");
    check(&AEROX_5_WIRELESS_WIRELESS, "dim_timer", Input::Int(30), "02 00 63 0F 01 00 00 30 75 00");
    check(&AEROX_5_WIRELESS_WIRELESS, "dim_timer", Input::Int(60), "02 00 63 0F 01 00 00 60 EA 00");
    check(&AEROX_5_WIRELESS_WIRELESS, "dim_timer", Input::Int(300), "02 00 63 0F 01 00 00 E0 93 04");
    check(&AEROX_5_WIRELESS_WIRELESS, "dim_timer", Input::Int(1200), "02 00 63 0F 01 00 00 80 4F 12");
}

/// rivalcfg test/devices/old_specs/test_aerox5_wireless_wireless.py::TestDevice::test_set_polling_rate
#[test]
fn aerox5_wireless_wireless_set_polling_rate() {
    check(&AEROX_5_WIRELESS_WIRELESS, "polling_rate", Input::Choice("125"), "02 00 6B 03");
    check(&AEROX_5_WIRELESS_WIRELESS, "polling_rate", Input::Choice("250"), "02 00 6B 02");
    check(&AEROX_5_WIRELESS_WIRELESS, "polling_rate", Input::Choice("500"), "02 00 6B 01");
    check(&AEROX_5_WIRELESS_WIRELESS, "polling_rate", Input::Choice("1000"), "02 00 6B 00");
}

/// rivalcfg test/devices/old_specs/test_aerox5_wireless_wireless.py::TestDevice::test_set_rainbow_effect
#[test]
fn aerox5_wireless_wireless_set_rainbow_effect() {
    check(&AEROX_5_WIRELESS_WIRELESS, "rainbow_effect", Input::Trigger, "02 00 62 FF");
}

/// rivalcfg test/devices/old_specs/test_aerox5_wireless_wireless.py::TestDevice::test_set_reactive_color
#[test]
fn aerox5_wireless_wireless_set_reactive_color() {
    check(&AEROX_5_WIRELESS_WIRELESS, "reactive_color", Input::Reactive(Some(rgb(0xFF0000))), "02 00 66 01 00 FF 00 00");
    check(&AEROX_5_WIRELESS_WIRELESS, "reactive_color", Input::Reactive(Some(rgb(0xFF1802))), "02 00 66 01 00 FF 18 02");
    check(&AEROX_5_WIRELESS_WIRELESS, "reactive_color", Input::Reactive(None), "02 00 66 00 00 00 00 00");
    check(&AEROX_5_WIRELESS_WIRELESS, "reactive_color", Input::Reactive(None), "02 00 66 00 00 00 00 00");
}

/// rivalcfg test/devices/old_specs/test_aerox5_wireless_wireless.py::TestDevice::test_set_sensitivity
#[test]
fn aerox5_wireless_wireless_set_sensitivity() {
    check(&AEROX_5_WIRELESS_WIRELESS, "sensitivity", Input::Dpi(&[(100, 100)]), "02 00 6D 01 00 00");
    check(&AEROX_5_WIRELESS_WIRELESS, "sensitivity", Input::Dpi(&[(200, 200)]), "02 00 6D 01 00 02");
    check(&AEROX_5_WIRELESS_WIRELESS, "sensitivity", Input::Dpi(&[(300, 300)]), "02 00 6D 01 00 03");
    check(&AEROX_5_WIRELESS_WIRELESS, "sensitivity", Input::Dpi(&[(18000, 18000)]), "02 00 6D 01 00 D6");
    check(&AEROX_5_WIRELESS_WIRELESS, "sensitivity", Input::Dpi(&[(200, 200), (400, 400)]), "02 00 6D 02 00 02 04");
    check(&AEROX_5_WIRELESS_WIRELESS, "sensitivity", Input::Dpi(&[(200, 200), (400, 400), (800, 800), (1600, 1600)]), "02 00 6D 04 00 02 04 09 12");
}

/// rivalcfg test/devices/old_specs/test_aerox5_wireless_wireless.py::TestDevice::test_set_sleep_timer
#[test]
fn aerox5_wireless_wireless_set_sleep_timer() {
    check(&AEROX_5_WIRELESS_WIRELESS, "sleep_timer", Input::Int(0), "02 00 69 00 00 00");
    check(&AEROX_5_WIRELESS_WIRELESS, "sleep_timer", Input::Int(1), "02 00 69 60 EA 00");
    check(&AEROX_5_WIRELESS_WIRELESS, "sleep_timer", Input::Int(5), "02 00 69 E0 93 04");
    check(&AEROX_5_WIRELESS_WIRELESS, "sleep_timer", Input::Int(20), "02 00 69 80 4F 12");
}

/// rivalcfg test/devices/old_specs/test_aerox5_wireless_wireless.py::TestDevice::test_set_z1_color
#[test]
fn aerox5_wireless_wireless_set_z1_color() {
    check(&AEROX_5_WIRELESS_WIRELESS, "z1_color", Input::Color(rgb(0xABCDEF)), "02 00 61 01 00 AB CD EF");
    check(&AEROX_5_WIRELESS_WIRELESS, "z1_color", Input::Color(rgb(0xFF0000)), "02 00 61 01 00 FF 00 00");
}

/// rivalcfg test/devices/old_specs/test_aerox5_wireless_wireless.py::TestDevice::test_set_z2_color
#[test]
fn aerox5_wireless_wireless_set_z2_color() {
    check(&AEROX_5_WIRELESS_WIRELESS, "z2_color", Input::Color(rgb(0xABCDEF)), "02 00 61 01 01 AB CD EF");
    check(&AEROX_5_WIRELESS_WIRELESS, "z2_color", Input::Color(rgb(0xFF0000)), "02 00 61 01 01 FF 00 00");
}

/// rivalcfg test/devices/old_specs/test_aerox5_wireless_wireless.py::TestDevice::test_set_z3_color
#[test]
fn aerox5_wireless_wireless_set_z3_color() {
    check(&AEROX_5_WIRELESS_WIRELESS, "z3_color", Input::Color(rgb(0xABCDEF)), "02 00 61 01 02 AB CD EF");
    check(&AEROX_5_WIRELESS_WIRELESS, "z3_color", Input::Color(rgb(0xFF0000)), "02 00 61 01 02 FF 00 00");
}

/// Defaults of every `AEROX_5_WIRELESS_WIRELESS` setting and its save/battery/firmware commands, as rivalcfg f16c521
/// encodes them (computed by running rivalcfg, not copied from its test files).
#[test]
fn aerox5_wireless_wireless_rivalcfg_defaults() {
    check(&AEROX_5_WIRELESS_WIRELESS, "sensitivity", Input::Dpi(&[(400, 400), (800, 800), (1200, 1200), (2400, 2400), (3200, 3200)]), "02 00 6D 05 00 04 09 0D 1B 26");
    check(&AEROX_5_WIRELESS_WIRELESS, "polling_rate", Input::Choice("1000"), "02 00 6B 00");
    check(&AEROX_5_WIRELESS_WIRELESS, "z1_color", Input::Color(rgb(0xFF0000)), "02 00 61 01 00 FF 00 00");
    check(&AEROX_5_WIRELESS_WIRELESS, "z2_color", Input::Color(rgb(0x00FF00)), "02 00 61 01 01 00 FF 00");
    check(&AEROX_5_WIRELESS_WIRELESS, "z3_color", Input::Color(rgb(0x0000FF)), "02 00 61 01 02 00 00 FF");
    check(&AEROX_5_WIRELESS_WIRELESS, "reactive_color", Input::Reactive(None), "02 00 66 00 00 00 00 00");
    check(&AEROX_5_WIRELESS_WIRELESS, "sleep_timer", Input::Int(5), "02 00 69 E0 93 04");
    check(&AEROX_5_WIRELESS_WIRELESS, "dim_timer", Input::Int(30), "02 00 63 0F 01 00 00 30 75 00");
    check(&AEROX_5_WIRELESS_WIRELESS, "buttons", Input::Buttons(&map(&[("button1", "button1"), ("button2", "button2"), ("button3", "button3"), ("button4", "button4"), ("button5", "button5"), ("button6", "dpi"), ("button7", "disabled"), ("button8", "disabled"), ("button9", "disabled"), ("scrollup", "scrollup"), ("scrolldown", "scrolldown"), ("layout", "qwerty")])), "02 00 6A 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 31 00 00 00 00 32 00 00 00 00");
    check(&AEROX_5_WIRELESS_WIRELESS, "rainbow_effect", Input::Trigger, "02 00 62 FF");
    check(&AEROX_5_WIRELESS_WIRELESS, "default_lighting", Input::Choice("rainbow"), "02 00 67 01 00");
    check_save(&AEROX_5_WIRELESS_WIRELESS, "02 00 51 00");
    check_battery(&AEROX_5_WIRELESS_WIRELESS, "02 00 D2");
}

// ---- AEROX_9_WIRELESS_WIRED ------------------------------------------------------------------

/// rivalcfg test/devices/old_specs/test_aerox9_wireless_wired.py::TestDevice::test_battery_level
#[test]
fn aerox9_wireless_wired_battery_level() {
    check_battery(&AEROX_9_WIRELESS_WIRED, "02 00 92");
}

/// rivalcfg test/devices/old_specs/test_aerox9_wireless_wired.py::TestDevice::test_save
#[test]
fn aerox9_wireless_wired_save() {
    check_save(&AEROX_9_WIRELESS_WIRED, "02 00 11 00");
}

/// rivalcfg test/devices/old_specs/test_aerox9_wireless_wired.py::TestDevice::test_set_default_lighting
#[test]
fn aerox9_wireless_wired_set_default_lighting() {
    check(&AEROX_9_WIRELESS_WIRED, "default_lighting", Input::Choice("off"), "02 00 27 00 00");
    check(&AEROX_9_WIRELESS_WIRED, "default_lighting", Input::Choice("reactive"), "02 00 27 00 01");
    check(&AEROX_9_WIRELESS_WIRED, "default_lighting", Input::Choice("rainbow"), "02 00 27 01 00");
    check(&AEROX_9_WIRELESS_WIRED, "default_lighting", Input::Choice("reactive-rainbow"), "02 00 27 01 01");
}

/// rivalcfg test/devices/old_specs/test_aerox9_wireless_wired.py::TestDevice::test_set_dim_timer
#[test]
fn aerox9_wireless_wired_set_dim_timer() {
    check(&AEROX_9_WIRELESS_WIRED, "dim_timer", Input::Int(0), "02 00 23 0F 01 00 00 00 00 00");
    check(&AEROX_9_WIRELESS_WIRED, "dim_timer", Input::Int(30), "02 00 23 0F 01 00 00 30 75 00");
    check(&AEROX_9_WIRELESS_WIRED, "dim_timer", Input::Int(60), "02 00 23 0F 01 00 00 60 EA 00");
    check(&AEROX_9_WIRELESS_WIRED, "dim_timer", Input::Int(300), "02 00 23 0F 01 00 00 E0 93 04");
    check(&AEROX_9_WIRELESS_WIRED, "dim_timer", Input::Int(1200), "02 00 23 0F 01 00 00 80 4F 12");
}

/// rivalcfg test/devices/old_specs/test_aerox9_wireless_wired.py::TestDevice::test_set_polling_rate
#[test]
fn aerox9_wireless_wired_set_polling_rate() {
    check(&AEROX_9_WIRELESS_WIRED, "polling_rate", Input::Choice("125"), "02 00 2B 03");
    check(&AEROX_9_WIRELESS_WIRED, "polling_rate", Input::Choice("250"), "02 00 2B 02");
    check(&AEROX_9_WIRELESS_WIRED, "polling_rate", Input::Choice("500"), "02 00 2B 01");
    check(&AEROX_9_WIRELESS_WIRED, "polling_rate", Input::Choice("1000"), "02 00 2B 00");
}

/// rivalcfg test/devices/old_specs/test_aerox9_wireless_wired.py::TestDevice::test_set_rainbow_effect
#[test]
fn aerox9_wireless_wired_set_rainbow_effect() {
    check(&AEROX_9_WIRELESS_WIRED, "rainbow_effect", Input::Trigger, "02 00 22 FF");
}

/// rivalcfg test/devices/old_specs/test_aerox9_wireless_wired.py::TestDevice::test_set_reactive_color
#[test]
fn aerox9_wireless_wired_set_reactive_color() {
    check(&AEROX_9_WIRELESS_WIRED, "reactive_color", Input::Reactive(Some(rgb(0xFF0000))), "02 00 26 01 00 FF 00 00");
    check(&AEROX_9_WIRELESS_WIRED, "reactive_color", Input::Reactive(Some(rgb(0xFF1802))), "02 00 26 01 00 FF 18 02");
    check(&AEROX_9_WIRELESS_WIRED, "reactive_color", Input::Reactive(None), "02 00 26 00 00 00 00 00");
    check(&AEROX_9_WIRELESS_WIRED, "reactive_color", Input::Reactive(None), "02 00 26 00 00 00 00 00");
}

/// rivalcfg test/devices/old_specs/test_aerox9_wireless_wired.py::TestDevice::test_set_sensitivity
#[test]
fn aerox9_wireless_wired_set_sensitivity() {
    check(&AEROX_9_WIRELESS_WIRED, "sensitivity", Input::Dpi(&[(100, 100)]), "02 00 2D 01 00 00");
    check(&AEROX_9_WIRELESS_WIRED, "sensitivity", Input::Dpi(&[(200, 200)]), "02 00 2D 01 00 02");
    check(&AEROX_9_WIRELESS_WIRED, "sensitivity", Input::Dpi(&[(300, 300)]), "02 00 2D 01 00 03");
    check(&AEROX_9_WIRELESS_WIRED, "sensitivity", Input::Dpi(&[(18000, 18000)]), "02 00 2D 01 00 D6");
    check(&AEROX_9_WIRELESS_WIRED, "sensitivity", Input::Dpi(&[(200, 200), (400, 400)]), "02 00 2D 02 00 02 04");
    check(&AEROX_9_WIRELESS_WIRED, "sensitivity", Input::Dpi(&[(200, 200), (400, 400), (800, 800), (1600, 1600)]), "02 00 2D 04 00 02 04 09 12");
}

/// rivalcfg test/devices/old_specs/test_aerox9_wireless_wired.py::TestDevice::test_set_sleep_timer
#[test]
fn aerox9_wireless_wired_set_sleep_timer() {
    check(&AEROX_9_WIRELESS_WIRED, "sleep_timer", Input::Int(0), "02 00 29 00 00 00");
    check(&AEROX_9_WIRELESS_WIRED, "sleep_timer", Input::Int(1), "02 00 29 60 EA 00");
    check(&AEROX_9_WIRELESS_WIRED, "sleep_timer", Input::Int(5), "02 00 29 E0 93 04");
    check(&AEROX_9_WIRELESS_WIRED, "sleep_timer", Input::Int(20), "02 00 29 80 4F 12");
}

/// rivalcfg test/devices/old_specs/test_aerox9_wireless_wired.py::TestDevice::test_set_z1_color
#[test]
fn aerox9_wireless_wired_set_z1_color() {
    check(&AEROX_9_WIRELESS_WIRED, "z1_color", Input::Color(rgb(0xABCDEF)), "02 00 21 01 00 AB CD EF");
    check(&AEROX_9_WIRELESS_WIRED, "z1_color", Input::Color(rgb(0xFF0000)), "02 00 21 01 00 FF 00 00");
}

/// rivalcfg test/devices/old_specs/test_aerox9_wireless_wired.py::TestDevice::test_set_z2_color
#[test]
fn aerox9_wireless_wired_set_z2_color() {
    check(&AEROX_9_WIRELESS_WIRED, "z2_color", Input::Color(rgb(0xABCDEF)), "02 00 21 01 01 AB CD EF");
    check(&AEROX_9_WIRELESS_WIRED, "z2_color", Input::Color(rgb(0xFF0000)), "02 00 21 01 01 FF 00 00");
}

/// rivalcfg test/devices/old_specs/test_aerox9_wireless_wired.py::TestDevice::test_set_z3_color
#[test]
fn aerox9_wireless_wired_set_z3_color() {
    check(&AEROX_9_WIRELESS_WIRED, "z3_color", Input::Color(rgb(0xABCDEF)), "02 00 21 01 02 AB CD EF");
    check(&AEROX_9_WIRELESS_WIRED, "z3_color", Input::Color(rgb(0xFF0000)), "02 00 21 01 02 FF 00 00");
}

/// Defaults of every `AEROX_9_WIRELESS_WIRED` setting and its save/battery/firmware commands, as rivalcfg f16c521
/// encodes them (computed by running rivalcfg, not copied from its test files).
#[test]
fn aerox9_wireless_wired_rivalcfg_defaults() {
    check(&AEROX_9_WIRELESS_WIRED, "sensitivity", Input::Dpi(&[(400, 400), (800, 800), (1200, 1200), (2400, 2400), (3200, 3200)]), "02 00 2D 05 00 04 09 0D 1B 26");
    check(&AEROX_9_WIRELESS_WIRED, "polling_rate", Input::Choice("1000"), "02 00 2B 00");
    check(&AEROX_9_WIRELESS_WIRED, "z1_color", Input::Color(rgb(0xFF0000)), "02 00 21 01 00 FF 00 00");
    check(&AEROX_9_WIRELESS_WIRED, "z2_color", Input::Color(rgb(0x00FF00)), "02 00 21 01 01 00 FF 00");
    check(&AEROX_9_WIRELESS_WIRED, "z3_color", Input::Color(rgb(0x0000FF)), "02 00 21 01 02 00 00 FF");
    check(&AEROX_9_WIRELESS_WIRED, "reactive_color", Input::Reactive(None), "02 00 26 00 00 00 00 00");
    check(&AEROX_9_WIRELESS_WIRED, "sleep_timer", Input::Int(5), "02 00 29 E0 93 04");
    check(&AEROX_9_WIRELESS_WIRED, "dim_timer", Input::Int(30), "02 00 23 0F 01 00 00 30 75 00");
    check(&AEROX_9_WIRELESS_WIRED, "rainbow_effect", Input::Trigger, "02 00 22 FF");
    check(&AEROX_9_WIRELESS_WIRED, "default_lighting", Input::Choice("rainbow"), "02 00 27 01 00");
    check_save(&AEROX_9_WIRELESS_WIRED, "02 00 11 00");
    check_battery(&AEROX_9_WIRELESS_WIRED, "02 00 92");
}

// ---- AEROX_9_WIRELESS_WIRELESS ---------------------------------------------------------------

/// rivalcfg test/devices/old_specs/test_aerox9_wireless_wireless.py::TestDevice::test_battery_level
#[test]
fn aerox9_wireless_wireless_battery_level() {
    check_battery(&AEROX_9_WIRELESS_WIRELESS, "02 00 D2");
}

/// rivalcfg test/devices/old_specs/test_aerox9_wireless_wireless.py::TestDevice::test_save
#[test]
fn aerox9_wireless_wireless_save() {
    check_save(&AEROX_9_WIRELESS_WIRELESS, "02 00 51 00");
}

/// rivalcfg test/devices/old_specs/test_aerox9_wireless_wireless.py::TestDevice::test_set_default_lighting
#[test]
fn aerox9_wireless_wireless_set_default_lighting() {
    check(&AEROX_9_WIRELESS_WIRELESS, "default_lighting", Input::Choice("off"), "02 00 67 00 00");
    check(&AEROX_9_WIRELESS_WIRELESS, "default_lighting", Input::Choice("reactive"), "02 00 67 00 01");
    check(&AEROX_9_WIRELESS_WIRELESS, "default_lighting", Input::Choice("rainbow"), "02 00 67 01 00");
    check(&AEROX_9_WIRELESS_WIRELESS, "default_lighting", Input::Choice("reactive-rainbow"), "02 00 67 01 01");
}

/// rivalcfg test/devices/old_specs/test_aerox9_wireless_wireless.py::TestDevice::test_set_dim_timer
#[test]
fn aerox9_wireless_wireless_set_dim_timer() {
    check(&AEROX_9_WIRELESS_WIRELESS, "dim_timer", Input::Int(0), "02 00 63 0F 01 00 00 00 00 00");
    check(&AEROX_9_WIRELESS_WIRELESS, "dim_timer", Input::Int(30), "02 00 63 0F 01 00 00 30 75 00");
    check(&AEROX_9_WIRELESS_WIRELESS, "dim_timer", Input::Int(60), "02 00 63 0F 01 00 00 60 EA 00");
    check(&AEROX_9_WIRELESS_WIRELESS, "dim_timer", Input::Int(300), "02 00 63 0F 01 00 00 E0 93 04");
    check(&AEROX_9_WIRELESS_WIRELESS, "dim_timer", Input::Int(1200), "02 00 63 0F 01 00 00 80 4F 12");
}

/// rivalcfg test/devices/old_specs/test_aerox9_wireless_wireless.py::TestDevice::test_set_polling_rate
#[test]
fn aerox9_wireless_wireless_set_polling_rate() {
    check(&AEROX_9_WIRELESS_WIRELESS, "polling_rate", Input::Choice("125"), "02 00 6B 03");
    check(&AEROX_9_WIRELESS_WIRELESS, "polling_rate", Input::Choice("250"), "02 00 6B 02");
    check(&AEROX_9_WIRELESS_WIRELESS, "polling_rate", Input::Choice("500"), "02 00 6B 01");
    check(&AEROX_9_WIRELESS_WIRELESS, "polling_rate", Input::Choice("1000"), "02 00 6B 00");
}

/// rivalcfg test/devices/old_specs/test_aerox9_wireless_wireless.py::TestDevice::test_set_rainbow_effect
#[test]
fn aerox9_wireless_wireless_set_rainbow_effect() {
    check(&AEROX_9_WIRELESS_WIRELESS, "rainbow_effect", Input::Trigger, "02 00 62 FF");
}

/// rivalcfg test/devices/old_specs/test_aerox9_wireless_wireless.py::TestDevice::test_set_reactive_color
#[test]
fn aerox9_wireless_wireless_set_reactive_color() {
    check(&AEROX_9_WIRELESS_WIRELESS, "reactive_color", Input::Reactive(Some(rgb(0xFF0000))), "02 00 66 01 00 FF 00 00");
    check(&AEROX_9_WIRELESS_WIRELESS, "reactive_color", Input::Reactive(Some(rgb(0xFF1802))), "02 00 66 01 00 FF 18 02");
    check(&AEROX_9_WIRELESS_WIRELESS, "reactive_color", Input::Reactive(None), "02 00 66 00 00 00 00 00");
    check(&AEROX_9_WIRELESS_WIRELESS, "reactive_color", Input::Reactive(None), "02 00 66 00 00 00 00 00");
}

/// rivalcfg test/devices/old_specs/test_aerox9_wireless_wireless.py::TestDevice::test_set_sensitivity
#[test]
fn aerox9_wireless_wireless_set_sensitivity() {
    check(&AEROX_9_WIRELESS_WIRELESS, "sensitivity", Input::Dpi(&[(100, 100)]), "02 00 6D 01 00 00");
    check(&AEROX_9_WIRELESS_WIRELESS, "sensitivity", Input::Dpi(&[(200, 200)]), "02 00 6D 01 00 02");
    check(&AEROX_9_WIRELESS_WIRELESS, "sensitivity", Input::Dpi(&[(300, 300)]), "02 00 6D 01 00 03");
    check(&AEROX_9_WIRELESS_WIRELESS, "sensitivity", Input::Dpi(&[(18000, 18000)]), "02 00 6D 01 00 D6");
    check(&AEROX_9_WIRELESS_WIRELESS, "sensitivity", Input::Dpi(&[(200, 200), (400, 400)]), "02 00 6D 02 00 02 04");
    check(&AEROX_9_WIRELESS_WIRELESS, "sensitivity", Input::Dpi(&[(200, 200), (400, 400), (800, 800), (1600, 1600)]), "02 00 6D 04 00 02 04 09 12");
}

/// rivalcfg test/devices/old_specs/test_aerox9_wireless_wireless.py::TestDevice::test_set_sleep_timer
#[test]
fn aerox9_wireless_wireless_set_sleep_timer() {
    check(&AEROX_9_WIRELESS_WIRELESS, "sleep_timer", Input::Int(0), "02 00 69 00 00 00");
    check(&AEROX_9_WIRELESS_WIRELESS, "sleep_timer", Input::Int(1), "02 00 69 60 EA 00");
    check(&AEROX_9_WIRELESS_WIRELESS, "sleep_timer", Input::Int(5), "02 00 69 E0 93 04");
    check(&AEROX_9_WIRELESS_WIRELESS, "sleep_timer", Input::Int(20), "02 00 69 80 4F 12");
}

/// rivalcfg test/devices/old_specs/test_aerox9_wireless_wireless.py::TestDevice::test_set_z1_color
#[test]
fn aerox9_wireless_wireless_set_z1_color() {
    check(&AEROX_9_WIRELESS_WIRELESS, "z1_color", Input::Color(rgb(0xABCDEF)), "02 00 61 01 00 AB CD EF");
    check(&AEROX_9_WIRELESS_WIRELESS, "z1_color", Input::Color(rgb(0xFF0000)), "02 00 61 01 00 FF 00 00");
}

/// rivalcfg test/devices/old_specs/test_aerox9_wireless_wireless.py::TestDevice::test_set_z2_color
#[test]
fn aerox9_wireless_wireless_set_z2_color() {
    check(&AEROX_9_WIRELESS_WIRELESS, "z2_color", Input::Color(rgb(0xABCDEF)), "02 00 61 01 01 AB CD EF");
    check(&AEROX_9_WIRELESS_WIRELESS, "z2_color", Input::Color(rgb(0xFF0000)), "02 00 61 01 01 FF 00 00");
}

/// rivalcfg test/devices/old_specs/test_aerox9_wireless_wireless.py::TestDevice::test_set_z3_color
#[test]
fn aerox9_wireless_wireless_set_z3_color() {
    check(&AEROX_9_WIRELESS_WIRELESS, "z3_color", Input::Color(rgb(0xABCDEF)), "02 00 61 01 02 AB CD EF");
    check(&AEROX_9_WIRELESS_WIRELESS, "z3_color", Input::Color(rgb(0xFF0000)), "02 00 61 01 02 FF 00 00");
}

/// Defaults of every `AEROX_9_WIRELESS_WIRELESS` setting and its save/battery/firmware commands, as rivalcfg f16c521
/// encodes them (computed by running rivalcfg, not copied from its test files).
#[test]
fn aerox9_wireless_wireless_rivalcfg_defaults() {
    check(&AEROX_9_WIRELESS_WIRELESS, "sensitivity", Input::Dpi(&[(400, 400), (800, 800), (1200, 1200), (2400, 2400), (3200, 3200)]), "02 00 6D 05 00 04 09 0D 1B 26");
    check(&AEROX_9_WIRELESS_WIRELESS, "polling_rate", Input::Choice("1000"), "02 00 6B 00");
    check(&AEROX_9_WIRELESS_WIRELESS, "z1_color", Input::Color(rgb(0xFF0000)), "02 00 61 01 00 FF 00 00");
    check(&AEROX_9_WIRELESS_WIRELESS, "z2_color", Input::Color(rgb(0x00FF00)), "02 00 61 01 01 00 FF 00");
    check(&AEROX_9_WIRELESS_WIRELESS, "z3_color", Input::Color(rgb(0x0000FF)), "02 00 61 01 02 00 00 FF");
    check(&AEROX_9_WIRELESS_WIRELESS, "reactive_color", Input::Reactive(None), "02 00 66 00 00 00 00 00");
    check(&AEROX_9_WIRELESS_WIRELESS, "sleep_timer", Input::Int(5), "02 00 69 E0 93 04");
    check(&AEROX_9_WIRELESS_WIRELESS, "dim_timer", Input::Int(30), "02 00 63 0F 01 00 00 30 75 00");
    check(&AEROX_9_WIRELESS_WIRELESS, "rainbow_effect", Input::Trigger, "02 00 62 FF");
    check(&AEROX_9_WIRELESS_WIRELESS, "default_lighting", Input::Choice("rainbow"), "02 00 67 01 00");
    check_save(&AEROX_9_WIRELESS_WIRELESS, "02 00 51 00");
    check_battery(&AEROX_9_WIRELESS_WIRELESS, "02 00 D2");
}

// ---- KANA_V2 ---------------------------------------------------------------------------------

/// rivalcfg test/devices/old_specs/test_kanav2.py::TestDevice::test_save
#[test]
fn kanav2_save() {
    check_save(&KANA_V2, "02 00 09 00");
}

/// rivalcfg test/devices/old_specs/test_kanav2.py::TestDevice::test_set_led_brightness1
#[test]
fn kanav2_set_led_brightness1() {
    check(&KANA_V2, "led_brightness1", Input::Choice("high"), "02 00 05 01 04");
    check(&KANA_V2, "led_brightness1", Input::Choice("medium"), "02 00 05 01 03");
    check(&KANA_V2, "led_brightness1", Input::Choice("low"), "02 00 05 01 02");
    check(&KANA_V2, "led_brightness1", Input::Choice("off"), "02 00 05 01 01");
}

/// rivalcfg test/devices/old_specs/test_kanav2.py::TestDevice::test_set_led_brightness2
#[test]
fn kanav2_set_led_brightness2() {
    check(&KANA_V2, "led_brightness2", Input::Choice("high"), "02 00 05 02 04");
    check(&KANA_V2, "led_brightness2", Input::Choice("medium"), "02 00 05 02 03");
    check(&KANA_V2, "led_brightness2", Input::Choice("low"), "02 00 05 02 02");
    check(&KANA_V2, "led_brightness2", Input::Choice("off"), "02 00 05 02 01");
}

/// rivalcfg test/devices/old_specs/test_kanav2.py::TestDevice::test_set_polling_rate
#[test]
fn kanav2_set_polling_rate() {
    check(&KANA_V2, "polling_rate", Input::Choice("125"), "02 00 04 00 04");
    check(&KANA_V2, "polling_rate", Input::Choice("250"), "02 00 04 00 03");
    check(&KANA_V2, "polling_rate", Input::Choice("500"), "02 00 04 00 02");
    check(&KANA_V2, "polling_rate", Input::Choice("1000"), "02 00 04 00 01");
}

/// rivalcfg test/devices/old_specs/test_kanav2.py::TestDevice::test_set_sensitivity1
#[test]
fn kanav2_set_sensitivity1() {
    check(&KANA_V2, "sensitivity1", Input::Choice("400"), "02 00 03 01 08");
    check(&KANA_V2, "sensitivity1", Input::Choice("800"), "02 00 03 01 07");
    check(&KANA_V2, "sensitivity1", Input::Choice("1200"), "02 00 03 01 06");
    check(&KANA_V2, "sensitivity1", Input::Choice("1600"), "02 00 03 01 05");
    check(&KANA_V2, "sensitivity1", Input::Choice("2000"), "02 00 03 01 04");
    check(&KANA_V2, "sensitivity1", Input::Choice("2400"), "02 00 03 01 03");
    check(&KANA_V2, "sensitivity1", Input::Choice("3200"), "02 00 03 01 02");
    check(&KANA_V2, "sensitivity1", Input::Choice("4000"), "02 00 03 01 01");
}

/// rivalcfg test/devices/old_specs/test_kanav2.py::TestDevice::test_set_sensitivity2
#[test]
fn kanav2_set_sensitivity2() {
    check(&KANA_V2, "sensitivity2", Input::Choice("400"), "02 00 03 02 08");
    check(&KANA_V2, "sensitivity2", Input::Choice("800"), "02 00 03 02 07");
    check(&KANA_V2, "sensitivity2", Input::Choice("1200"), "02 00 03 02 06");
    check(&KANA_V2, "sensitivity2", Input::Choice("1600"), "02 00 03 02 05");
    check(&KANA_V2, "sensitivity2", Input::Choice("2000"), "02 00 03 02 04");
    check(&KANA_V2, "sensitivity2", Input::Choice("2400"), "02 00 03 02 03");
    check(&KANA_V2, "sensitivity2", Input::Choice("3200"), "02 00 03 02 02");
    check(&KANA_V2, "sensitivity2", Input::Choice("4000"), "02 00 03 02 01");
}

/// Defaults of every `KANA_V2` setting and its save/battery/firmware commands, as rivalcfg f16c521
/// encodes them (computed by running rivalcfg, not copied from its test files).
#[test]
fn kanav2_rivalcfg_defaults() {
    check(&KANA_V2, "sensitivity1", Input::Choice("800"), "02 00 03 01 07");
    check(&KANA_V2, "sensitivity2", Input::Choice("1600"), "02 00 03 02 05");
    check(&KANA_V2, "polling_rate", Input::Choice("1000"), "02 00 04 00 01");
    check(&KANA_V2, "led_brightness1", Input::Choice("off"), "02 00 05 01 01");
    check(&KANA_V2, "led_brightness2", Input::Choice("high"), "02 00 05 02 04");
    check_save(&KANA_V2, "02 00 09 00");
}

// ---- KINZU_V2 --------------------------------------------------------------------------------

/// rivalcfg test/devices/old_specs/test_kinzuv2.py::TestDevice::test_save
#[test]
fn kinzuv2_save() {
    check_save(&KINZU_V2, "02 00 09 00");
}

/// rivalcfg test/devices/old_specs/test_kinzuv2.py::TestDevice::test_set_polling_rate
#[test]
fn kinzuv2_set_polling_rate() {
    check(&KINZU_V2, "polling_rate", Input::Choice("125"), "02 00 04 00 04");
    check(&KINZU_V2, "polling_rate", Input::Choice("250"), "02 00 04 00 03");
    check(&KINZU_V2, "polling_rate", Input::Choice("500"), "02 00 04 00 02");
    check(&KINZU_V2, "polling_rate", Input::Choice("1000"), "02 00 04 00 01");
}

/// rivalcfg test/devices/old_specs/test_kinzuv2.py::TestDevice::test_set_sensitivity1
#[test]
fn kinzuv2_set_sensitivity1() {
    check(&KINZU_V2, "sensitivity1", Input::Choice("400"), "02 00 03 01 04");
    check(&KINZU_V2, "sensitivity1", Input::Choice("800"), "02 00 03 01 03");
    check(&KINZU_V2, "sensitivity1", Input::Choice("1600"), "02 00 03 01 02");
    check(&KINZU_V2, "sensitivity1", Input::Choice("3200"), "02 00 03 01 01");
}

/// rivalcfg test/devices/old_specs/test_kinzuv2.py::TestDevice::test_set_sensitivity2
#[test]
fn kinzuv2_set_sensitivity2() {
    check(&KINZU_V2, "sensitivity2", Input::Choice("400"), "02 00 03 02 04");
    check(&KINZU_V2, "sensitivity2", Input::Choice("800"), "02 00 03 02 03");
    check(&KINZU_V2, "sensitivity2", Input::Choice("1600"), "02 00 03 02 02");
    check(&KINZU_V2, "sensitivity2", Input::Choice("3200"), "02 00 03 02 01");
}

/// Defaults of every `KINZU_V2` setting and its save/battery/firmware commands, as rivalcfg f16c521
/// encodes them (computed by running rivalcfg, not copied from its test files).
#[test]
fn kinzuv2_rivalcfg_defaults() {
    check(&KINZU_V2, "sensitivity1", Input::Choice("800"), "02 00 03 01 03");
    check(&KINZU_V2, "sensitivity2", Input::Choice("3200"), "02 00 03 02 01");
    check(&KINZU_V2, "polling_rate", Input::Choice("1000"), "02 00 04 00 01");
    check_save(&KINZU_V2, "02 00 09 00");
}

// ---- PRIME -----------------------------------------------------------------------------------

/// rivalcfg test/devices/old_specs/test_prime.py::TestDevice::test_set_buttons_mapping
#[test]
fn prime_set_buttons_mapping() {
    check(&PRIME, "buttons", Input::Buttons(&map(&[])), "02 00 5B 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00");
    check(&PRIME, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 5B 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00");
    check(&PRIME, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 5B 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00");
    check(&PRIME, "buttons", Input::Buttons(&map(&[("button1", "ScrollUp"), ("button2", "ScrollDown")])), "02 00 5B 31 00 00 00 00 32 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00");
}

/// rivalcfg test/devices/old_specs/test_prime.py::TestDevice::test_set_color
#[test]
fn prime_set_color() {
    check(&PRIME, "color", Input::Color(rgb(0xABCDEF)), "02 00 62 01 AB CD EF 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 FF");
    check(&PRIME, "color", Input::Color(rgb(0xFF0000)), "02 00 62 01 FF 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 FF");
}

/// rivalcfg test/devices/old_specs/test_prime.py::TestDevice::test_set_led_brightness
#[test]
fn prime_set_led_brightness() {
    check(&PRIME, "led_brightness", Input::Int(0), "02 00 5F 00 00");
    check(&PRIME, "led_brightness", Input::Int(256), "02 00 5F 00 01");
    check(&PRIME, "led_brightness", Input::Int(111), "02 00 5F 6F 00");
}

/// rivalcfg test/devices/old_specs/test_prime.py::TestDevice::test_set_polling_rate
#[test]
fn prime_set_polling_rate() {
    check(&PRIME, "polling_rate", Input::Choice("125"), "02 00 5D 04");
    check(&PRIME, "polling_rate", Input::Choice("250"), "02 00 5D 03");
    check(&PRIME, "polling_rate", Input::Choice("500"), "02 00 5D 02");
    check(&PRIME, "polling_rate", Input::Choice("1000"), "02 00 5D 01");
}

/// rivalcfg test/devices/old_specs/test_prime.py::TestDevice::test_set_sensitivity
#[test]
fn prime_set_sensitivity() {
    check(&PRIME, "sensitivity", Input::Dpi(&[(100, 100)]), "02 00 61 01 00 02 00");
    check(&PRIME, "sensitivity", Input::Dpi(&[(100, 100)]), "02 00 61 01 00 02 00");
    check(&PRIME, "sensitivity", Input::Dpi(&[(500, 500), (2500, 2500)]), "02 00 61 02 00 0A 00 32 00");
    check(&PRIME, "sensitivity", Input::Dpi(&[(500, 500), (2500, 2500), (11050, 11050), (18000, 18000)]), "02 00 61 04 00 0A 00 32 00 DD 00 68 01");
}

/// Defaults of every `PRIME` setting and its save/battery/firmware commands, as rivalcfg f16c521
/// encodes them (computed by running rivalcfg, not copied from its test files).
#[test]
fn prime_rivalcfg_defaults() {
    check(&PRIME, "sensitivity", Input::Dpi(&[(400, 400), (800, 800), (1200, 1200), (2400, 2400), (3200, 3200)]), "02 00 61 05 00 08 00 10 00 18 00 30 00 40 00");
    check(&PRIME, "polling_rate", Input::Choice("1000"), "02 00 5D 01");
    check(&PRIME, "color", Input::Color(rgb(0xFF5200)), "02 00 62 01 FF 52 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 FF");
    check(&PRIME, "led_brightness", Input::Int(256), "02 00 5F 00 01");
    check(&PRIME, "buttons", Input::Buttons(&map(&[("button1", "button1"), ("button2", "button2"), ("button3", "button3"), ("button4", "button4"), ("button5", "button5"), ("button6", "dpi"), ("layout", "qwerty")])), "02 00 5B 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00");
    check_save(&PRIME, "02 00 59");
}

// ---- PRIME_MINI ------------------------------------------------------------------------------

/// rivalcfg test/devices/old_specs/test_prime_mini.py::TestDevice::test_save
#[test]
fn prime_mini_save() {
    check_save(&PRIME_MINI, "02 00 11 00");
}

/// rivalcfg test/devices/old_specs/test_prime_mini.py::TestDevice::test_set_buttons_mapping
#[test]
fn prime_mini_set_buttons_mapping() {
    check(&PRIME_MINI, "buttons", Input::Buttons(&map(&[])), "02 00 2A 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 31 00 00 00 00 32 00 00 00 00");
    check(&PRIME_MINI, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 2A 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 31 00 00 00 00 32 00 00 00 00");
    check(&PRIME_MINI, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 2A 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 31 00 00 00 00 32 00 00 00 00");
    check(&PRIME_MINI, "buttons", Input::Buttons(&map(&[("ScrollUp", "ScrollDown"), ("ScrollDown", "ScrollUp")])), "02 00 2A 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 32 00 00 00 00 31 00 00 00 00");
}

/// rivalcfg test/devices/old_specs/test_prime_mini.py::TestDevice::test_set_color
#[test]
fn prime_mini_set_color() {
    check(&PRIME_MINI, "color", Input::Color(rgb(0xABCDEF)), "02 00 21 00 AB CD EF");
    check(&PRIME_MINI, "color", Input::Color(rgb(0xFF0000)), "02 00 21 00 FF 00 00");
}

/// rivalcfg test/devices/old_specs/test_prime_mini.py::TestDevice::test_set_default_lighting
#[test]
fn prime_mini_set_default_lighting() {
    check(&PRIME_MINI, "default_lighting", Input::Choice("off"), "02 00 27 00");
    check(&PRIME_MINI, "default_lighting", Input::Choice("rainbow"), "02 00 27 01");
}

/// rivalcfg test/devices/old_specs/test_prime_mini.py::TestDevice::test_set_polling_rate
#[test]
fn prime_mini_set_polling_rate() {
    check(&PRIME_MINI, "polling_rate", Input::Choice("125"), "02 00 2B 04");
    check(&PRIME_MINI, "polling_rate", Input::Choice("250"), "02 00 2B 03");
    check(&PRIME_MINI, "polling_rate", Input::Choice("500"), "02 00 2B 02");
    check(&PRIME_MINI, "polling_rate", Input::Choice("1000"), "02 00 2B 01");
}

/// rivalcfg test/devices/old_specs/test_prime_mini.py::TestDevice::test_set_sensitivity
#[test]
fn prime_mini_set_sensitivity() {
    check(&PRIME_MINI, "sensitivity", Input::Dpi(&[(100, 100)]), "02 00 2D 01 00 02 00");
    check(&PRIME_MINI, "sensitivity", Input::Dpi(&[(100, 100)]), "02 00 2D 01 00 02 00");
    check(&PRIME_MINI, "sensitivity", Input::Dpi(&[(500, 500), (2500, 2500)]), "02 00 2D 02 00 0A 00 32 00");
    check(&PRIME_MINI, "sensitivity", Input::Dpi(&[(500, 500), (2500, 2500), (11050, 11050), (18000, 18000)]), "02 00 2D 04 00 0A 00 32 00 DD 00 68 01");
}

/// Defaults of every `PRIME_MINI` setting and its save/battery/firmware commands, as rivalcfg f16c521
/// encodes them (computed by running rivalcfg, not copied from its test files).
#[test]
fn prime_mini_rivalcfg_defaults() {
    check(&PRIME_MINI, "sensitivity", Input::Dpi(&[(400, 400), (800, 800), (1200, 1200), (2400, 2400), (3200, 3200)]), "02 00 2D 05 00 08 00 10 00 18 00 30 00 40 00");
    check(&PRIME_MINI, "polling_rate", Input::Choice("1000"), "02 00 2B 01");
    check(&PRIME_MINI, "color", Input::Color(rgb(0xFF0000)), "02 00 21 00 FF 00 00");
    check(&PRIME_MINI, "buttons", Input::Buttons(&map(&[("button1", "button1"), ("button2", "button2"), ("button3", "button3"), ("button4", "button4"), ("button5", "button5"), ("button6", "dpi"), ("scrollup", "scrollup"), ("scrolldown", "scrolldown"), ("layout", "qwerty")])), "02 00 2A 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 31 00 00 00 00 32 00 00 00 00");
    check(&PRIME_MINI, "default_lighting", Input::Choice("rainbow"), "02 00 27 01");
    check_save(&PRIME_MINI, "02 00 11 00");
}

// ---- PRIME_PLUS ------------------------------------------------------------------------------

/// rivalcfg test/devices/old_specs/test_prime_plus.py::TestDevice::test_set_buttons_mapping
#[test]
fn prime_plus_set_buttons_mapping() {
    check(&PRIME_PLUS, "buttons", Input::Buttons(&map(&[])), "02 00 5B 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00");
    check(&PRIME_PLUS, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 5B 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00");
    check(&PRIME_PLUS, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 5B 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00");
    check(&PRIME_PLUS, "buttons", Input::Buttons(&map(&[("button1", "ScrollUp"), ("button2", "ScrollDown")])), "02 00 5B 31 00 00 00 00 32 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00");
}

/// rivalcfg test/devices/old_specs/test_prime_plus.py::TestDevice::test_set_color
#[test]
fn prime_plus_set_color() {
    check(&PRIME_PLUS, "color", Input::Color(rgb(0xABCDEF)), "02 00 62 01 AB CD EF 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 FF");
    check(&PRIME_PLUS, "color", Input::Color(rgb(0xFF0000)), "02 00 62 01 FF 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 FF");
}

/// rivalcfg test/devices/old_specs/test_prime_plus.py::TestDevice::test_set_led_brightness
#[test]
fn prime_plus_set_led_brightness() {
    check(&PRIME_PLUS, "led_brightness", Input::Int(0), "02 00 5F 00 00");
    check(&PRIME_PLUS, "led_brightness", Input::Int(256), "02 00 5F 00 01");
    check(&PRIME_PLUS, "led_brightness", Input::Int(111), "02 00 5F 6F 00");
}

/// rivalcfg test/devices/old_specs/test_prime_plus.py::TestDevice::test_set_polling_rate
#[test]
fn prime_plus_set_polling_rate() {
    check(&PRIME_PLUS, "polling_rate", Input::Choice("125"), "02 00 5D 04");
    check(&PRIME_PLUS, "polling_rate", Input::Choice("250"), "02 00 5D 03");
    check(&PRIME_PLUS, "polling_rate", Input::Choice("500"), "02 00 5D 02");
    check(&PRIME_PLUS, "polling_rate", Input::Choice("1000"), "02 00 5D 01");
}

/// rivalcfg test/devices/old_specs/test_prime_plus.py::TestDevice::test_set_sensitivity
#[test]
fn prime_plus_set_sensitivity() {
    check(&PRIME_PLUS, "sensitivity", Input::Dpi(&[(100, 100)]), "02 00 61 01 00 02 00");
    check(&PRIME_PLUS, "sensitivity", Input::Dpi(&[(100, 100)]), "02 00 61 01 00 02 00");
    check(&PRIME_PLUS, "sensitivity", Input::Dpi(&[(500, 500), (2500, 2500)]), "02 00 61 02 00 0A 00 32 00");
    check(&PRIME_PLUS, "sensitivity", Input::Dpi(&[(500, 500), (2500, 2500), (11050, 11050), (18000, 18000)]), "02 00 61 04 00 0A 00 32 00 DD 00 68 01");
}

/// Defaults of every `PRIME_PLUS` setting and its save/battery/firmware commands, as rivalcfg f16c521
/// encodes them (computed by running rivalcfg, not copied from its test files).
#[test]
fn prime_plus_rivalcfg_defaults() {
    check(&PRIME_PLUS, "sensitivity", Input::Dpi(&[(400, 400), (800, 800), (1200, 1200), (2400, 2400), (3200, 3200)]), "02 00 61 05 00 08 00 10 00 18 00 30 00 40 00");
    check(&PRIME_PLUS, "polling_rate", Input::Choice("1000"), "02 00 5D 01");
    check(&PRIME_PLUS, "color", Input::Color(rgb(0xFF5200)), "02 00 62 01 FF 52 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 FF");
    check(&PRIME_PLUS, "led_brightness", Input::Int(256), "02 00 5F 00 01");
    check(&PRIME_PLUS, "buttons", Input::Buttons(&map(&[("button1", "button1"), ("button2", "button2"), ("button3", "button3"), ("button4", "button4"), ("button5", "button5"), ("button6", "dpi"), ("layout", "qwerty")])), "02 00 5B 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00");
    check_save(&PRIME_PLUS, "02 00 59");
}

// ---- PRIME_WIRELESS_WIRED --------------------------------------------------------------------

/// rivalcfg test/devices/old_specs/test_prime_wireless_wired.py::TestDevice::test_battery_level
#[test]
fn prime_wireless_wired_battery_level() {
    check_battery(&PRIME_WIRELESS_WIRED, "02 00 92");
}

/// rivalcfg test/devices/old_specs/test_prime_wireless_wired.py::TestDevice::test_save
#[test]
fn prime_wireless_wired_save() {
    check_save(&PRIME_WIRELESS_WIRED, "02 00 11 00");
}

/// rivalcfg test/devices/old_specs/test_prime_wireless_wired.py::TestDevice::test_set_buttons_mapping
#[test]
fn prime_wireless_wired_set_buttons_mapping() {
    check(&PRIME_WIRELESS_WIRED, "buttons", Input::Buttons(&map(&[])), "02 00 2A 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 31 00 00 00 00 32 00 00 00 00");
    check(&PRIME_WIRELESS_WIRED, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 2A 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 31 00 00 00 00 32 00 00 00 00");
    check(&PRIME_WIRELESS_WIRED, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 2A 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 31 00 00 00 00 32 00 00 00 00");
    check(&PRIME_WIRELESS_WIRED, "buttons", Input::Buttons(&map(&[("ScrollUp", "ScrollDown"), ("ScrollDown", "ScrollUp")])), "02 00 2A 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 32 00 00 00 00 31 00 00 00 00");
}

/// rivalcfg test/devices/old_specs/test_prime_wireless_wired.py::TestDevice::test_set_color
#[test]
fn prime_wireless_wired_set_color() {
    check(&PRIME_WIRELESS_WIRED, "color", Input::Color(rgb(0xABCDEF)), "02 00 21 01 00 AB CD EF");
    check(&PRIME_WIRELESS_WIRED, "color", Input::Color(rgb(0xFF0000)), "02 00 21 01 00 FF 00 00");
}

/// rivalcfg test/devices/old_specs/test_prime_wireless_wired.py::TestDevice::test_set_default_lighting
#[test]
fn prime_wireless_wired_set_default_lighting() {
    check(&PRIME_WIRELESS_WIRED, "default_lighting", Input::Choice("off"), "02 00 27 00");
    check(&PRIME_WIRELESS_WIRED, "default_lighting", Input::Choice("rainbow"), "02 00 27 01");
}

/// rivalcfg test/devices/old_specs/test_prime_wireless_wired.py::TestDevice::test_set_dim_timer
#[test]
fn prime_wireless_wired_set_dim_timer() {
    check(&PRIME_WIRELESS_WIRED, "dim_timer", Input::Int(0), "02 00 23 0F 01 00 00 00 00 00");
    check(&PRIME_WIRELESS_WIRED, "dim_timer", Input::Int(30), "02 00 23 0F 01 00 00 30 75 00");
    check(&PRIME_WIRELESS_WIRED, "dim_timer", Input::Int(60), "02 00 23 0F 01 00 00 60 EA 00");
    check(&PRIME_WIRELESS_WIRED, "dim_timer", Input::Int(300), "02 00 23 0F 01 00 00 E0 93 04");
    check(&PRIME_WIRELESS_WIRED, "dim_timer", Input::Int(1200), "02 00 23 0F 01 00 00 80 4F 12");
}

/// rivalcfg test/devices/old_specs/test_prime_wireless_wired.py::TestDevice::test_set_polling_rate
#[test]
fn prime_wireless_wired_set_polling_rate() {
    check(&PRIME_WIRELESS_WIRED, "polling_rate", Input::Choice("125"), "02 00 2B 03");
    check(&PRIME_WIRELESS_WIRED, "polling_rate", Input::Choice("250"), "02 00 2B 02");
    check(&PRIME_WIRELESS_WIRED, "polling_rate", Input::Choice("500"), "02 00 2B 01");
    check(&PRIME_WIRELESS_WIRED, "polling_rate", Input::Choice("1000"), "02 00 2B 00");
}

/// rivalcfg test/devices/old_specs/test_prime_wireless_wired.py::TestDevice::test_set_sensitivity
#[test]
fn prime_wireless_wired_set_sensitivity() {
    check(&PRIME_WIRELESS_WIRED, "sensitivity", Input::Dpi(&[(100, 100)]), "02 00 2D 01 00 00");
    check(&PRIME_WIRELESS_WIRED, "sensitivity", Input::Dpi(&[(200, 200)]), "02 00 2D 01 00 02");
    check(&PRIME_WIRELESS_WIRED, "sensitivity", Input::Dpi(&[(300, 300)]), "02 00 2D 01 00 03");
    check(&PRIME_WIRELESS_WIRED, "sensitivity", Input::Dpi(&[(18000, 18000)]), "02 00 2D 01 00 D6");
    check(&PRIME_WIRELESS_WIRED, "sensitivity", Input::Dpi(&[(200, 200), (400, 400)]), "02 00 2D 02 00 02 04");
    check(&PRIME_WIRELESS_WIRED, "sensitivity", Input::Dpi(&[(200, 200), (400, 400), (800, 800), (1600, 1600)]), "02 00 2D 04 00 02 04 09 12");
}

/// rivalcfg test/devices/old_specs/test_prime_wireless_wired.py::TestDevice::test_set_sleep_timer
#[test]
fn prime_wireless_wired_set_sleep_timer() {
    check(&PRIME_WIRELESS_WIRED, "sleep_timer", Input::Int(0), "02 00 29 00 00 00");
    check(&PRIME_WIRELESS_WIRED, "sleep_timer", Input::Int(1), "02 00 29 60 EA 00");
    check(&PRIME_WIRELESS_WIRED, "sleep_timer", Input::Int(5), "02 00 29 E0 93 04");
    check(&PRIME_WIRELESS_WIRED, "sleep_timer", Input::Int(20), "02 00 29 80 4F 12");
}

/// Defaults of every `PRIME_WIRELESS_WIRED` setting and its save/battery/firmware commands, as rivalcfg f16c521
/// encodes them (computed by running rivalcfg, not copied from its test files).
#[test]
fn prime_wireless_wired_rivalcfg_defaults() {
    check(&PRIME_WIRELESS_WIRED, "sensitivity", Input::Dpi(&[(400, 400), (800, 800), (1200, 1200), (2400, 2400), (3200, 3200)]), "02 00 2D 05 00 04 09 0D 1B 26");
    check(&PRIME_WIRELESS_WIRED, "polling_rate", Input::Choice("1000"), "02 00 2B 00");
    check(&PRIME_WIRELESS_WIRED, "color", Input::Color(rgb(0xFF0000)), "02 00 21 01 00 FF 00 00");
    check(&PRIME_WIRELESS_WIRED, "buttons", Input::Buttons(&map(&[("button1", "button1"), ("button2", "button2"), ("button3", "button3"), ("button4", "button4"), ("button5", "button5"), ("button6", "dpi"), ("scrollup", "scrollup"), ("scrolldown", "scrolldown"), ("layout", "qwerty")])), "02 00 2A 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 31 00 00 00 00 32 00 00 00 00");
    check(&PRIME_WIRELESS_WIRED, "sleep_timer", Input::Int(5), "02 00 29 E0 93 04");
    check(&PRIME_WIRELESS_WIRED, "dim_timer", Input::Int(30), "02 00 23 0F 01 00 00 30 75 00");
    check(&PRIME_WIRELESS_WIRED, "default_lighting", Input::Choice("rainbow"), "02 00 27 01");
    check_save(&PRIME_WIRELESS_WIRED, "02 00 11 00");
    check_battery(&PRIME_WIRELESS_WIRED, "02 00 92");
}

// ---- PRIME_WIRELESS_WIRELESS -----------------------------------------------------------------

/// rivalcfg test/devices/old_specs/test_prime_wireless_wireless.py::TestDevice::test_battery_level
#[test]
fn prime_wireless_wireless_battery_level() {
    check_battery(&PRIME_WIRELESS_WIRELESS, "02 00 D2");
}

/// rivalcfg test/devices/old_specs/test_prime_wireless_wireless.py::TestDevice::test_save
#[test]
fn prime_wireless_wireless_save() {
    check_save(&PRIME_WIRELESS_WIRELESS, "02 00 51 00");
}

/// rivalcfg test/devices/old_specs/test_prime_wireless_wireless.py::TestDevice::test_set_buttons_mapping
#[test]
fn prime_wireless_wireless_set_buttons_mapping() {
    check(&PRIME_WIRELESS_WIRELESS, "buttons", Input::Buttons(&map(&[])), "02 00 6A 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 31 00 00 00 00 32 00 00 00 00");
    check(&PRIME_WIRELESS_WIRELESS, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 6A 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 31 00 00 00 00 32 00 00 00 00");
    check(&PRIME_WIRELESS_WIRELESS, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 6A 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 31 00 00 00 00 32 00 00 00 00");
    check(&PRIME_WIRELESS_WIRELESS, "buttons", Input::Buttons(&map(&[("ScrollUp", "ScrollDown"), ("ScrollDown", "ScrollUp")])), "02 00 6A 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 32 00 00 00 00 31 00 00 00 00");
}

/// rivalcfg test/devices/old_specs/test_prime_wireless_wireless.py::TestDevice::test_set_color
#[test]
fn prime_wireless_wireless_set_color() {
    check(&PRIME_WIRELESS_WIRELESS, "color", Input::Color(rgb(0xABCDEF)), "02 00 61 01 00 AB CD EF");
    check(&PRIME_WIRELESS_WIRELESS, "color", Input::Color(rgb(0xFF0000)), "02 00 61 01 00 FF 00 00");
}

/// rivalcfg test/devices/old_specs/test_prime_wireless_wireless.py::TestDevice::test_set_default_lighting
#[test]
fn prime_wireless_wireless_set_default_lighting() {
    check(&PRIME_WIRELESS_WIRELESS, "default_lighting", Input::Choice("off"), "02 00 67 00");
    check(&PRIME_WIRELESS_WIRELESS, "default_lighting", Input::Choice("rainbow"), "02 00 67 01");
}

/// rivalcfg test/devices/old_specs/test_prime_wireless_wireless.py::TestDevice::test_set_dim_timer
#[test]
fn prime_wireless_wireless_set_dim_timer() {
    check(&PRIME_WIRELESS_WIRELESS, "dim_timer", Input::Int(0), "02 00 63 0F 01 00 00 00 00 00");
    check(&PRIME_WIRELESS_WIRELESS, "dim_timer", Input::Int(30), "02 00 63 0F 01 00 00 30 75 00");
    check(&PRIME_WIRELESS_WIRELESS, "dim_timer", Input::Int(60), "02 00 63 0F 01 00 00 60 EA 00");
    check(&PRIME_WIRELESS_WIRELESS, "dim_timer", Input::Int(300), "02 00 63 0F 01 00 00 E0 93 04");
    check(&PRIME_WIRELESS_WIRELESS, "dim_timer", Input::Int(1200), "02 00 63 0F 01 00 00 80 4F 12");
}

/// rivalcfg test/devices/old_specs/test_prime_wireless_wireless.py::TestDevice::test_set_polling_rate
#[test]
fn prime_wireless_wireless_set_polling_rate() {
    check(&PRIME_WIRELESS_WIRELESS, "polling_rate", Input::Choice("125"), "02 00 6B 03");
    check(&PRIME_WIRELESS_WIRELESS, "polling_rate", Input::Choice("250"), "02 00 6B 02");
    check(&PRIME_WIRELESS_WIRELESS, "polling_rate", Input::Choice("500"), "02 00 6B 01");
    check(&PRIME_WIRELESS_WIRELESS, "polling_rate", Input::Choice("1000"), "02 00 6B 00");
}

/// rivalcfg test/devices/old_specs/test_prime_wireless_wireless.py::TestDevice::test_set_sensitivity
#[test]
fn prime_wireless_wireless_set_sensitivity() {
    check(&PRIME_WIRELESS_WIRELESS, "sensitivity", Input::Dpi(&[(100, 100)]), "02 00 6D 01 00 00");
    check(&PRIME_WIRELESS_WIRELESS, "sensitivity", Input::Dpi(&[(200, 200)]), "02 00 6D 01 00 02");
    check(&PRIME_WIRELESS_WIRELESS, "sensitivity", Input::Dpi(&[(300, 300)]), "02 00 6D 01 00 03");
    check(&PRIME_WIRELESS_WIRELESS, "sensitivity", Input::Dpi(&[(18000, 18000)]), "02 00 6D 01 00 D6");
    check(&PRIME_WIRELESS_WIRELESS, "sensitivity", Input::Dpi(&[(200, 200), (400, 400)]), "02 00 6D 02 00 02 04");
    check(&PRIME_WIRELESS_WIRELESS, "sensitivity", Input::Dpi(&[(200, 200), (400, 400), (800, 800), (1600, 1600)]), "02 00 6D 04 00 02 04 09 12");
}

/// rivalcfg test/devices/old_specs/test_prime_wireless_wireless.py::TestDevice::test_set_sleep_timer
#[test]
fn prime_wireless_wireless_set_sleep_timer() {
    check(&PRIME_WIRELESS_WIRELESS, "sleep_timer", Input::Int(0), "02 00 69 00 00 00");
    check(&PRIME_WIRELESS_WIRELESS, "sleep_timer", Input::Int(1), "02 00 69 60 EA 00");
    check(&PRIME_WIRELESS_WIRELESS, "sleep_timer", Input::Int(5), "02 00 69 E0 93 04");
    check(&PRIME_WIRELESS_WIRELESS, "sleep_timer", Input::Int(20), "02 00 69 80 4F 12");
}

/// Defaults of every `PRIME_WIRELESS_WIRELESS` setting and its save/battery/firmware commands, as rivalcfg f16c521
/// encodes them (computed by running rivalcfg, not copied from its test files).
#[test]
fn prime_wireless_wireless_rivalcfg_defaults() {
    check(&PRIME_WIRELESS_WIRELESS, "sensitivity", Input::Dpi(&[(400, 400), (800, 800), (1200, 1200), (2400, 2400), (3200, 3200)]), "02 00 6D 05 00 04 09 0D 1B 26");
    check(&PRIME_WIRELESS_WIRELESS, "polling_rate", Input::Choice("1000"), "02 00 6B 00");
    check(&PRIME_WIRELESS_WIRELESS, "color", Input::Color(rgb(0xFF0000)), "02 00 61 01 00 FF 00 00");
    check(&PRIME_WIRELESS_WIRELESS, "buttons", Input::Buttons(&map(&[("button1", "button1"), ("button2", "button2"), ("button3", "button3"), ("button4", "button4"), ("button5", "button5"), ("button6", "dpi"), ("scrollup", "scrollup"), ("scrolldown", "scrolldown"), ("layout", "qwerty")])), "02 00 6A 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 31 00 00 00 00 32 00 00 00 00");
    check(&PRIME_WIRELESS_WIRELESS, "sleep_timer", Input::Int(5), "02 00 69 E0 93 04");
    check(&PRIME_WIRELESS_WIRELESS, "dim_timer", Input::Int(30), "02 00 63 0F 01 00 00 30 75 00");
    check(&PRIME_WIRELESS_WIRELESS, "default_lighting", Input::Choice("rainbow"), "02 00 67 01");
    check_save(&PRIME_WIRELESS_WIRELESS, "02 00 51 00");
    check_battery(&PRIME_WIRELESS_WIRELESS, "02 00 D2");
}

// ---- RIVAL_3 ---------------------------------------------------------------------------------

/// rivalcfg test/devices/old_specs/test_rival3.py::TestDevice::test_firmware_version
#[test]
fn rival3_firmware_version() {
    check_firmware(&RIVAL_3, "02 00 10 00");
}

/// rivalcfg test/devices/old_specs/test_rival3.py::TestDevice::test_save
#[test]
fn rival3_save() {
    check_save(&RIVAL_3, "02 00 09 00");
}

/// rivalcfg test/devices/old_specs/test_rival3.py::TestDevice::test_set_buttons_mapping
#[test]
fn rival3_set_buttons_mapping() {
    check(&RIVAL_3, "buttons", Input::Buttons(&map(&[])), "02 00 07 00 01 00 02 00 03 00 04 00 05 00 30 00 31 00 32 00");
    check(&RIVAL_3, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 07 00 01 00 06 00 03 00 04 00 05 00 30 00 31 00 32 00");
    check(&RIVAL_3, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 07 00 01 00 06 00 03 00 04 00 05 00 30 00 31 00 32 00");
    check(&RIVAL_3, "buttons", Input::Buttons(&map(&[("ScrollUp", "ScrollDown"), ("ScrollDown", "ScrollUp")])), "02 00 07 00 01 00 02 00 03 00 04 00 05 00 30 00 32 00 31 00");
}

/// rivalcfg test/devices/old_specs/test_rival3.py::TestDevice::test_set_light_effect
#[test]
fn rival3_set_light_effect() {
    check(&RIVAL_3, "light_effect", Input::Choice("rainbow-shift"), "02 00 06 00 00");
    check(&RIVAL_3, "light_effect", Input::Choice("breath-fast"), "02 00 06 00 01");
    check(&RIVAL_3, "light_effect", Input::Choice("breath"), "02 00 06 00 02");
    check(&RIVAL_3, "light_effect", Input::Choice("breath-slow"), "02 00 06 00 03");
    check(&RIVAL_3, "light_effect", Input::Choice("steady"), "02 00 06 00 04");
    check(&RIVAL_3, "light_effect", Input::Choice("rainbow-breath"), "02 00 06 00 05");
    check(&RIVAL_3, "light_effect", Input::Choice("disco"), "02 00 06 00 06");
}

/// rivalcfg test/devices/old_specs/test_rival3.py::TestDevice::test_set_logo_color
#[test]
fn rival3_set_logo_color() {
    check(&RIVAL_3, "logo_color", Input::Color(rgb(0xABCDEF)), "02 00 05 00 04 AB CD EF 64");
    check(&RIVAL_3, "logo_color", Input::Color(rgb(0xFF0000)), "02 00 05 00 04 FF 00 00 64");
}

/// rivalcfg test/devices/old_specs/test_rival3.py::TestDevice::test_set_polling_rate
#[test]
fn rival3_set_polling_rate() {
    check(&RIVAL_3, "polling_rate", Input::Choice("125"), "02 00 04 00 04");
    check(&RIVAL_3, "polling_rate", Input::Choice("250"), "02 00 04 00 03");
    check(&RIVAL_3, "polling_rate", Input::Choice("500"), "02 00 04 00 02");
    check(&RIVAL_3, "polling_rate", Input::Choice("1000"), "02 00 04 00 01");
}

/// rivalcfg test/devices/old_specs/test_rival3.py::TestDevice::test_set_sensitivity
#[test]
fn rival3_set_sensitivity() {
    check(&RIVAL_3, "sensitivity", Input::Dpi(&[(200, 200)]), "02 00 0B 00 01 01 04");
    check(&RIVAL_3, "sensitivity", Input::Dpi(&[(200, 200)]), "02 00 0B 00 01 01 04");
    check(&RIVAL_3, "sensitivity", Input::Dpi(&[(200, 200), (400, 400)]), "02 00 0B 00 02 01 04 08");
    check(&RIVAL_3, "sensitivity", Input::Dpi(&[(200, 200), (400, 400), (800, 800), (1600, 1600)]), "02 00 0B 00 04 01 04 08 12 24");
}

/// rivalcfg test/devices/old_specs/test_rival3.py::TestDevice::test_set_z1_color
#[test]
fn rival3_set_z1_color() {
    check(&RIVAL_3, "z1_color", Input::Color(rgb(0xABCDEF)), "02 00 05 00 01 AB CD EF 64");
    check(&RIVAL_3, "z1_color", Input::Color(rgb(0xFF0000)), "02 00 05 00 01 FF 00 00 64");
}

/// rivalcfg test/devices/old_specs/test_rival3.py::TestDevice::test_set_z2_color
#[test]
fn rival3_set_z2_color() {
    check(&RIVAL_3, "z2_color", Input::Color(rgb(0xABCDEF)), "02 00 05 00 02 AB CD EF 64");
    check(&RIVAL_3, "z2_color", Input::Color(rgb(0xFF0000)), "02 00 05 00 02 FF 00 00 64");
}

/// rivalcfg test/devices/old_specs/test_rival3.py::TestDevice::test_set_z3_color
#[test]
fn rival3_set_z3_color() {
    check(&RIVAL_3, "z3_color", Input::Color(rgb(0xABCDEF)), "02 00 05 00 03 AB CD EF 64");
    check(&RIVAL_3, "z3_color", Input::Color(rgb(0xFF0000)), "02 00 05 00 03 FF 00 00 64");
}

/// Defaults of every `RIVAL_3` setting and its save/battery/firmware commands, as rivalcfg f16c521
/// encodes them (computed by running rivalcfg, not copied from its test files).
#[test]
fn rival3_rivalcfg_defaults() {
    check(&RIVAL_3, "sensitivity", Input::Dpi(&[(800, 800), (1600, 1600)]), "02 00 0B 00 02 01 12 24");
    check(&RIVAL_3, "polling_rate", Input::Choice("1000"), "02 00 04 00 01");
    check(&RIVAL_3, "z1_color", Input::Color(rgb(0xFF0000)), "02 00 05 00 01 FF 00 00 64");
    check(&RIVAL_3, "z2_color", Input::Color(rgb(0x00FF00)), "02 00 05 00 02 00 FF 00 64");
    check(&RIVAL_3, "z3_color", Input::Color(rgb(0x0000FF)), "02 00 05 00 03 00 00 FF 64");
    check(&RIVAL_3, "logo_color", Input::Color(rgb(0x800080)), "02 00 05 00 04 80 00 80 64");
    check(&RIVAL_3, "light_effect", Input::Choice("steady"), "02 00 06 00 04");
    check(&RIVAL_3, "buttons", Input::Buttons(&map(&[("button1", "button1"), ("button2", "button2"), ("button3", "button3"), ("button4", "button4"), ("button5", "button5"), ("button6", "dpi"), ("scrollup", "scrollup"), ("scrolldown", "scrolldown"), ("layout", "qwerty")])), "02 00 07 00 01 00 02 00 03 00 04 00 05 00 30 00 31 00 32 00");
    check_save(&RIVAL_3, "02 00 09 00");
    check_firmware(&RIVAL_3, "02 00 10 00");
}

// ---- RIVAL_3_GEN_2 ---------------------------------------------------------------------------

/// rivalcfg test/devices/old_specs/test_rival3_gen2.py::TestDevice::test_save
#[test]
fn rival3_gen2_save() {
    check_save(&RIVAL_3_GEN_2, "02 00 11 00");
}

/// rivalcfg test/devices/old_specs/test_rival3_gen2.py::TestDevice::test_set_buttons_mapping
#[test]
fn rival3_gen2_set_buttons_mapping() {
    check(&RIVAL_3_GEN_2, "buttons", Input::Buttons(&map(&[])), "02 00 2A 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 31 00 00 00 00 32 00 00 00 00");
    check(&RIVAL_3_GEN_2, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 2A 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 31 00 00 00 00 32 00 00 00 00");
    check(&RIVAL_3_GEN_2, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 2A 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 31 00 00 00 00 32 00 00 00 00");
    check(&RIVAL_3_GEN_2, "buttons", Input::Buttons(&map(&[("ScrollUp", "ScrollDown"), ("ScrollDown", "ScrollUp")])), "02 00 2A 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 32 00 00 00 00 31 00 00 00 00");
}

/// rivalcfg test/devices/old_specs/test_rival3_gen2.py::TestDevice::test_set_default_lighting
#[test]
fn rival3_gen2_set_default_lighting() {
    check(&RIVAL_3_GEN_2, "default_lighting", Input::Choice("off"), "02 00 27 00 00");
    check(&RIVAL_3_GEN_2, "default_lighting", Input::Choice("reactive"), "02 00 27 00 01");
    check(&RIVAL_3_GEN_2, "default_lighting", Input::Choice("rainbow"), "02 00 27 01 00");
    check(&RIVAL_3_GEN_2, "default_lighting", Input::Choice("reactive-rainbow"), "02 00 27 01 01");
}

/// rivalcfg test/devices/old_specs/test_rival3_gen2.py::TestDevice::test_set_led_brightness
#[test]
fn rival3_gen2_set_led_brightness() {
    check(&RIVAL_3_GEN_2, "led_brightness", Input::Int(0), "02 00 23 00");
    check(&RIVAL_3_GEN_2, "led_brightness", Input::Int(100), "02 00 23 64");
}

/// rivalcfg test/devices/old_specs/test_rival3_gen2.py::TestDevice::test_set_polling_rate
#[test]
fn rival3_gen2_set_polling_rate() {
    check(&RIVAL_3_GEN_2, "polling_rate", Input::Choice("125"), "02 00 2B 04");
    check(&RIVAL_3_GEN_2, "polling_rate", Input::Choice("250"), "02 00 2B 03");
    check(&RIVAL_3_GEN_2, "polling_rate", Input::Choice("500"), "02 00 2B 02");
    check(&RIVAL_3_GEN_2, "polling_rate", Input::Choice("1000"), "02 00 2B 01");
}

/// rivalcfg test/devices/old_specs/test_rival3_gen2.py::TestDevice::test_set_rainbow_effect
#[test]
fn rival3_gen2_set_rainbow_effect() {
    check(&RIVAL_3_GEN_2, "rainbow_effect", Input::Choice("all"), "02 00 22 07");
    check(&RIVAL_3_GEN_2, "rainbow_effect", Input::Choice("bottom"), "02 00 22 04");
    check(&RIVAL_3_GEN_2, "rainbow_effect", Input::Choice("middle"), "02 00 22 02");
    check(&RIVAL_3_GEN_2, "rainbow_effect", Input::Choice("top"), "02 00 22 01");
    check(&RIVAL_3_GEN_2, "rainbow_effect", Input::Choice("bottom-middle"), "02 00 22 06");
    check(&RIVAL_3_GEN_2, "rainbow_effect", Input::Choice("middle-top"), "02 00 22 03");
    check(&RIVAL_3_GEN_2, "rainbow_effect", Input::Choice("bottom-top"), "02 00 22 05");
}

/// rivalcfg test/devices/old_specs/test_rival3_gen2.py::TestDevice::test_set_reactive_color
#[test]
fn rival3_gen2_set_reactive_color() {
    check(&RIVAL_3_GEN_2, "reactive_color", Input::Reactive(Some(rgb(0xFF0000))), "02 00 26 01 00 FF 00 00");
    check(&RIVAL_3_GEN_2, "reactive_color", Input::Reactive(Some(rgb(0xFF1802))), "02 00 26 01 00 FF 18 02");
    check(&RIVAL_3_GEN_2, "reactive_color", Input::Reactive(None), "02 00 26 00 00 00 00 00");
    check(&RIVAL_3_GEN_2, "reactive_color", Input::Reactive(None), "02 00 26 00 00 00 00 00");
}

/// rivalcfg test/devices/old_specs/test_rival3_gen2.py::TestDevice::test_set_sensitivity
#[test]
fn rival3_gen2_set_sensitivity() {
    check(&RIVAL_3_GEN_2, "sensitivity", Input::Dpi(&[(200, 200)]), "02 00 34 01 01 04 04");
    check(&RIVAL_3_GEN_2, "sensitivity", Input::Dpi(&[(200, 200)]), "02 00 34 01 01 04 04");
    check(&RIVAL_3_GEN_2, "sensitivity", Input::Dpi(&[(200, 300), (400, 500)]), "02 00 34 02 01 04 06 08 0B");
    check(&RIVAL_3_GEN_2, "sensitivity", Input::Dpi(&[(200, 200), (400, 400), (800, 800), (1600, 1800)]), "02 00 34 04 01 04 04 08 08 12 12 24 29");
}

/// rivalcfg test/devices/old_specs/test_rival3_gen2.py::TestDevice::test_set_z1_color
#[test]
fn rival3_gen2_set_z1_color() {
    check(&RIVAL_3_GEN_2, "z1_color", Input::Color(rgb(0xABCDEF)), "02 00 21 01 AB CD EF");
    check(&RIVAL_3_GEN_2, "z1_color", Input::Color(rgb(0xFF0000)), "02 00 21 01 FF 00 00");
}

/// rivalcfg test/devices/old_specs/test_rival3_gen2.py::TestDevice::test_set_z2_color
#[test]
fn rival3_gen2_set_z2_color() {
    check(&RIVAL_3_GEN_2, "z2_color", Input::Color(rgb(0xABCDEF)), "02 00 21 02 00 00 00 AB CD EF");
    check(&RIVAL_3_GEN_2, "z2_color", Input::Color(rgb(0xFF0000)), "02 00 21 02 00 00 00 FF 00 00");
}

/// rivalcfg test/devices/old_specs/test_rival3_gen2.py::TestDevice::test_set_z3_color
#[test]
fn rival3_gen2_set_z3_color() {
    check(&RIVAL_3_GEN_2, "z3_color", Input::Color(rgb(0xABCDEF)), "02 00 21 04 00 00 00 00 00 00 AB CD EF");
    check(&RIVAL_3_GEN_2, "z3_color", Input::Color(rgb(0xFF0000)), "02 00 21 04 00 00 00 00 00 00 FF 00 00");
}

/// Defaults of every `RIVAL_3_GEN_2` setting and its save/battery/firmware commands, as rivalcfg f16c521
/// encodes them (computed by running rivalcfg, not copied from its test files).
#[test]
fn rival3_gen2_rivalcfg_defaults() {
    check(&RIVAL_3_GEN_2, "sensitivity", Input::Dpi(&[(800, 800), (1600, 1600)]), "02 00 34 02 01 12 12 24 24");
    check(&RIVAL_3_GEN_2, "polling_rate", Input::Choice("1000"), "02 00 2B 01");
    check(&RIVAL_3_GEN_2, "z1_color", Input::Color(rgb(0xFF0000)), "02 00 21 01 FF 00 00");
    check(&RIVAL_3_GEN_2, "z2_color", Input::Color(rgb(0x00FF00)), "02 00 21 02 00 00 00 00 FF 00");
    check(&RIVAL_3_GEN_2, "z3_color", Input::Color(rgb(0x0000FF)), "02 00 21 04 00 00 00 00 00 00 00 00 FF");
    check(&RIVAL_3_GEN_2, "reactive_color", Input::Reactive(None), "02 00 26 00 00 00 00 00");
    check(&RIVAL_3_GEN_2, "led_brightness", Input::Int(100), "02 00 23 64");
    check(&RIVAL_3_GEN_2, "rainbow_effect", Input::Choice("all"), "02 00 22 07");
    check(&RIVAL_3_GEN_2, "default_lighting", Input::Choice("rainbow"), "02 00 27 01 00");
    check(&RIVAL_3_GEN_2, "buttons", Input::Buttons(&map(&[("button1", "button1"), ("button2", "button2"), ("button3", "button3"), ("button4", "button4"), ("button5", "button5"), ("button6", "dpi"), ("scrollup", "scrollup"), ("scrolldown", "scrolldown"), ("layout", "qwerty")])), "02 00 2A 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00 31 00 00 00 00 32 00 00 00 00");
    check_save(&RIVAL_3_GEN_2, "02 00 11 00");
}

// ---- RIVAL_3_WIRELESS ------------------------------------------------------------------------

/// rivalcfg test/devices/old_specs/test_rival3_wireless.py::TestDevice::test_battery_level
#[test]
fn rival3_wireless_battery_level() {
    check_battery(&RIVAL_3_WIRELESS, "02 00 AA 01");
}

/// rivalcfg test/devices/old_specs/test_rival3_wireless.py::TestDevice::test_save
#[test]
fn rival3_wireless_save() {
    check_save(&RIVAL_3_WIRELESS, "02 00 09");
}

/// rivalcfg test/devices/old_specs/test_rival3_wireless.py::TestDevice::test_set_buttons_mapping
#[test]
fn rival3_wireless_set_buttons_mapping() {
    check(&RIVAL_3_WIRELESS, "buttons", Input::Buttons(&map(&[])), "02 00 19 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00");
    check(&RIVAL_3_WIRELESS, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 19 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00");
    check(&RIVAL_3_WIRELESS, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 19 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00");
    check(&RIVAL_3_WIRELESS, "buttons", Input::Buttons(&map(&[("Button1", "ScrollDown"), ("Button2", "ScrollUp")])), "02 00 19 32 00 00 00 00 31 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00");
}

/// rivalcfg test/devices/old_specs/test_rival3_wireless.py::TestDevice::test_set_polling_rate
#[test]
fn rival3_wireless_set_polling_rate() {
    check(&RIVAL_3_WIRELESS, "polling_rate", Input::Choice("125"), "02 00 17 03");
    check(&RIVAL_3_WIRELESS, "polling_rate", Input::Choice("250"), "02 00 17 02");
    check(&RIVAL_3_WIRELESS, "polling_rate", Input::Choice("500"), "02 00 17 01");
    check(&RIVAL_3_WIRELESS, "polling_rate", Input::Choice("1000"), "02 00 17 00");
}

/// rivalcfg test/devices/old_specs/test_rival3_wireless.py::TestDevice::test_set_sensitivity
#[test]
fn rival3_wireless_set_sensitivity() {
    check(&RIVAL_3_WIRELESS, "sensitivity", Input::Dpi(&[(100, 100)]), "02 00 20 01 01 00 00");
    check(&RIVAL_3_WIRELESS, "sensitivity", Input::Dpi(&[(200, 200)]), "02 00 20 01 01 02 00");
    check(&RIVAL_3_WIRELESS, "sensitivity", Input::Dpi(&[(300, 300)]), "02 00 20 01 01 03 00");
    check(&RIVAL_3_WIRELESS, "sensitivity", Input::Dpi(&[(18000, 18000)]), "02 00 20 01 01 D6 00");
    check(&RIVAL_3_WIRELESS, "sensitivity", Input::Dpi(&[(200, 200), (400, 400)]), "02 00 20 02 01 02 00 04 00");
    check(&RIVAL_3_WIRELESS, "sensitivity", Input::Dpi(&[(200, 200), (400, 400), (800, 800), (1600, 1600)]), "02 00 20 04 01 02 00 04 00 09 00 12 00");
}

/// Defaults of every `RIVAL_3_WIRELESS` setting and its save/battery/firmware commands, as rivalcfg f16c521
/// encodes them (computed by running rivalcfg, not copied from its test files).
#[test]
fn rival3_wireless_rivalcfg_defaults() {
    check(&RIVAL_3_WIRELESS, "sensitivity", Input::Dpi(&[(400, 400), (800, 800), (1200, 1200), (2400, 2400), (3200, 3200)]), "02 00 20 05 01 04 00 09 00 0D 00 1B 00 26 00");
    check(&RIVAL_3_WIRELESS, "polling_rate", Input::Choice("1000"), "02 00 17 00");
    check(&RIVAL_3_WIRELESS, "buttons", Input::Buttons(&map(&[("button1", "button1"), ("button2", "button2"), ("button3", "button3"), ("button4", "button4"), ("button5", "button5"), ("button6", "dpi")])), "02 00 19 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00");
    check_save(&RIVAL_3_WIRELESS, "02 00 09");
    check_battery(&RIVAL_3_WIRELESS, "02 00 AA 01");
    check_firmware(&RIVAL_3_WIRELESS, "02 00 90 00");
}

// ---- RIVAL_3_WIRELESS_GEN_2 ------------------------------------------------------------------

/// rivalcfg test/devices/old_specs/test_rival3_wireless_gen2.py::TestDevice::test_battery_level
#[test]
fn rival3_wireless_gen2_battery_level() {
    check_battery(&RIVAL_3_WIRELESS_GEN_2, "02 00 AA 01");
}

/// rivalcfg test/devices/old_specs/test_rival3_wireless_gen2.py::TestDevice::test_save
#[test]
fn rival3_wireless_gen2_save() {
    check_save(&RIVAL_3_WIRELESS_GEN_2, "02 00 09");
}

/// rivalcfg test/devices/old_specs/test_rival3_wireless_gen2.py::TestDevice::test_set_buttons_mapping
#[test]
fn rival3_wireless_gen2_set_buttons_mapping() {
    check(&RIVAL_3_WIRELESS_GEN_2, "buttons", Input::Buttons(&map(&[])), "02 00 19 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00");
    check(&RIVAL_3_WIRELESS_GEN_2, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 19 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00");
    check(&RIVAL_3_WIRELESS_GEN_2, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 19 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00");
    check(&RIVAL_3_WIRELESS_GEN_2, "buttons", Input::Buttons(&map(&[("Button1", "ScrollDown"), ("Button2", "ScrollUp")])), "02 00 19 32 00 00 00 00 31 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00");
}

/// rivalcfg test/devices/old_specs/test_rival3_wireless_gen2.py::TestDevice::test_set_polling_rate
#[test]
fn rival3_wireless_gen2_set_polling_rate() {
    check(&RIVAL_3_WIRELESS_GEN_2, "polling_rate", Input::Choice("125"), "02 00 17 03");
    check(&RIVAL_3_WIRELESS_GEN_2, "polling_rate", Input::Choice("250"), "02 00 17 02");
    check(&RIVAL_3_WIRELESS_GEN_2, "polling_rate", Input::Choice("500"), "02 00 17 01");
    check(&RIVAL_3_WIRELESS_GEN_2, "polling_rate", Input::Choice("1000"), "02 00 17 00");
}

/// rivalcfg test/devices/old_specs/test_rival3_wireless_gen2.py::TestDevice::test_set_sensitivity
#[test]
fn rival3_wireless_gen2_set_sensitivity() {
    check(&RIVAL_3_WIRELESS_GEN_2, "sensitivity", Input::Dpi(&[(100, 100)]), "02 00 2C 01 01 00 00 00 00 00 00 00 00 00 00");
    check(&RIVAL_3_WIRELESS_GEN_2, "sensitivity", Input::Dpi(&[(200, 200)]), "02 00 2C 01 01 02 00 00 00 00 02 00 00 00 00");
    check(&RIVAL_3_WIRELESS_GEN_2, "sensitivity", Input::Dpi(&[(300, 300)]), "02 00 2C 01 01 03 00 00 00 00 03 00 00 00 00");
    check(&RIVAL_3_WIRELESS_GEN_2, "sensitivity", Input::Dpi(&[(18000, 18000)]), "02 00 2C 01 01 D6 00 00 00 00 D6 00 00 00 00");
    check(&RIVAL_3_WIRELESS_GEN_2, "sensitivity", Input::Dpi(&[(200, 200), (400, 400)]), "02 00 2C 02 01 02 04 00 00 00 02 04 00 00 00");
    check(&RIVAL_3_WIRELESS_GEN_2, "sensitivity", Input::Dpi(&[(200, 300), (400, 500), (800, 800), (1600, 1600)]), "02 00 2C 04 01 02 04 09 12 00 03 05 09 12 00");
}

/// Defaults of every `RIVAL_3_WIRELESS_GEN_2` setting and its save/battery/firmware commands, as rivalcfg f16c521
/// encodes them (computed by running rivalcfg, not copied from its test files).
#[test]
fn rival3_wireless_gen2_rivalcfg_defaults() {
    check(&RIVAL_3_WIRELESS_GEN_2, "sensitivity", Input::Dpi(&[(800, 800), (1600, 1600)]), "02 00 2C 02 01 09 12 00 00 00 09 12 00 00 00");
    check(&RIVAL_3_WIRELESS_GEN_2, "polling_rate", Input::Choice("1000"), "02 00 17 00");
    check(&RIVAL_3_WIRELESS_GEN_2, "buttons", Input::Buttons(&map(&[("button1", "button1"), ("button2", "button2"), ("button3", "button3"), ("button4", "button4"), ("button5", "button5"), ("button6", "dpi")])), "02 00 19 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 00 \
         00");
    check_save(&RIVAL_3_WIRELESS_GEN_2, "02 00 09");
    check_battery(&RIVAL_3_WIRELESS_GEN_2, "02 00 AA 01");
}

// ---- RIVAL_5 ---------------------------------------------------------------------------------

/// rivalcfg test/devices/old_specs/test_rival5.py::TestDevice::test_set_buttons_mapping
#[test]
fn rival5_set_buttons_mapping() {
    check(&RIVAL_5, "buttons", Input::Buttons(&map(&[("button1", "button2"), ("button2", "a"), ("button3", "ScrollUp")])), "02 00 2A 02 00 00 00 00 51 04 00 00 00 31 00 00 00 00 04 00 00 00 00 05 00 00 00 00 00 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 30 00 00 00 00 31 00 00 00 00 32 00 00 00 00");
}

/// rivalcfg test/devices/old_specs/test_rival5.py::TestDevice::test_set_default_lighting
#[test]
fn rival5_set_default_lighting() {
    check(&RIVAL_5, "default_lighting", Input::Choice("off"), "02 00 27 00");
    check(&RIVAL_5, "default_lighting", Input::Choice("rainbow"), "02 00 27 01");
}

/// rivalcfg test/devices/old_specs/test_rival5.py::TestDevice::test_set_led_brightness
#[test]
fn rival5_set_led_brightness() {
    check(&RIVAL_5, "led_brightness", Input::Choice("100"), "02 00 23 64");
    check(&RIVAL_5, "led_brightness", Input::Choice("75"), "02 00 23 32");
    check(&RIVAL_5, "led_brightness", Input::Choice("50"), "02 00 23 19");
    check(&RIVAL_5, "led_brightness", Input::Choice("25"), "02 00 23 0C");
    check(&RIVAL_5, "led_brightness", Input::Choice("0"), "02 00 23 00");
}

/// rivalcfg test/devices/old_specs/test_rival5.py::TestDevice::test_set_logo_color
#[test]
fn rival5_set_logo_color() {
    check(&RIVAL_5, "logo_color", Input::Color(rgb(0xABCDEF)), "02 00 21 00 02 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         AB CD EF");
}

/// rivalcfg test/devices/old_specs/test_rival5.py::TestDevice::test_set_polling_rate
#[test]
fn rival5_set_polling_rate() {
    check(&RIVAL_5, "polling_rate", Input::Choice("125"), "02 00 2B 04");
    check(&RIVAL_5, "polling_rate", Input::Choice("250"), "02 00 2B 03");
    check(&RIVAL_5, "polling_rate", Input::Choice("500"), "02 00 2B 02");
    check(&RIVAL_5, "polling_rate", Input::Choice("1000"), "02 00 2B 01");
}

/// rivalcfg test/devices/old_specs/test_rival5.py::TestDevice::test_set_rainbow_effect
#[test]
fn rival5_set_rainbow_effect() {
    check(&RIVAL_5, "rainbow_effect", Input::Trigger, "02 00 22 FF 03");
}

/// rivalcfg test/devices/old_specs/test_rival5.py::TestDevice::test_set_reactive_color
#[test]
fn rival5_set_reactive_color() {
    check(&RIVAL_5, "reactive_color", Input::Reactive(Some(rgb(0xABCDEF))), "02 00 26 01 00 AB CD EF");
    check(&RIVAL_5, "reactive_color", Input::Reactive(None), "02 00 26 00 00 00 00 00");
}

/// rivalcfg test/devices/old_specs/test_rival5.py::TestDevice::test_set_sensitivity
#[test]
fn rival5_set_sensitivity() {
    check(&RIVAL_5, "sensitivity", Input::Dpi(&[(100, 100)]), "02 00 2D 01 00 00");
    check(&RIVAL_5, "sensitivity", Input::Dpi(&[(200, 200)]), "02 00 2D 01 00 02");
    check(&RIVAL_5, "sensitivity", Input::Dpi(&[(300, 300)]), "02 00 2D 01 00 03");
    check(&RIVAL_5, "sensitivity", Input::Dpi(&[(18000, 18000)]), "02 00 2D 01 00 D6");
    check(&RIVAL_5, "sensitivity", Input::Dpi(&[(200, 200), (400, 400)]), "02 00 2D 02 00 02 04");
    check(&RIVAL_5, "sensitivity", Input::Dpi(&[(200, 200), (400, 400), (800, 800), (1600, 1600)]), "02 00 2D 04 00 02 04 09 12");
}

/// rivalcfg test/devices/old_specs/test_rival5.py::TestDevice::test_set_wheel_color
#[test]
fn rival5_set_wheel_color() {
    check(&RIVAL_5, "wheel_color", Input::Color(rgb(0xABCDEF)), "02 00 21 01 00 AB CD EF");
}

/// rivalcfg test/devices/old_specs/test_rival5.py::TestDevice::test_set_z2_color
#[test]
fn rival5_set_z2_color() {
    check(&RIVAL_5, "z2_color", Input::Color(rgb(0xABCDEF)), "02 00 21 02 00 00 00 00 AB CD EF");
}

/// rivalcfg test/devices/old_specs/test_rival5.py::TestDevice::test_set_z3_color
#[test]
fn rival5_set_z3_color() {
    check(&RIVAL_5, "z3_color", Input::Color(rgb(0xABCDEF)), "02 00 21 04 00 00 00 00 00 00 00 AB CD EF");
}

/// rivalcfg test/devices/old_specs/test_rival5.py::TestDevice::test_set_z4_color
#[test]
fn rival5_set_z4_color() {
    check(&RIVAL_5, "z4_color", Input::Color(rgb(0xABCDEF)), "02 00 21 08 00 00 00 00 00 00 00 00 00 00 AB CD EF");
}

/// rivalcfg test/devices/old_specs/test_rival5.py::TestDevice::test_set_z5_color
#[test]
fn rival5_set_z5_color() {
    check(&RIVAL_5, "z5_color", Input::Color(rgb(0xABCDEF)), "02 00 21 10 00 00 00 00 00 00 00 00 00 00 00 00 00 AB CD EF");
}

/// rivalcfg test/devices/old_specs/test_rival5.py::TestDevice::test_set_z6_color
#[test]
fn rival5_set_z6_color() {
    check(&RIVAL_5, "z6_color", Input::Color(rgb(0xABCDEF)), "02 00 21 20 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 AB CD EF");
}

/// rivalcfg test/devices/old_specs/test_rival5.py::TestDevice::test_set_z7_color
#[test]
fn rival5_set_z7_color() {
    check(&RIVAL_5, "z7_color", Input::Color(rgb(0xABCDEF)), "02 00 21 40 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 AB CD EF");
}

/// rivalcfg test/devices/old_specs/test_rival5.py::TestDevice::test_set_z8_color
#[test]
fn rival5_set_z8_color() {
    check(&RIVAL_5, "z8_color", Input::Color(rgb(0xABCDEF)), "02 00 21 80 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 AB CD EF");
}

/// rivalcfg test/devices/old_specs/test_rival5.py::TestDevice::test_set_z9_color
#[test]
fn rival5_set_z9_color() {
    check(&RIVAL_5, "z9_color", Input::Color(rgb(0xABCDEF)), "02 00 21 00 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 AB CD EF");
}

/// Defaults of every `RIVAL_5` setting and its save/battery/firmware commands, as rivalcfg f16c521
/// encodes them (computed by running rivalcfg, not copied from its test files).
#[test]
fn rival5_rivalcfg_defaults() {
    check(&RIVAL_5, "sensitivity", Input::Dpi(&[(400, 400), (800, 800), (1200, 1200), (2400, 2400), (3200, 3200)]), "02 00 2D 05 00 04 09 0D 1B 26");
    check(&RIVAL_5, "polling_rate", Input::Choice("1000"), "02 00 2B 01");
    check(&RIVAL_5, "wheel_color", Input::Color(rgb(0xFF1800)), "02 00 21 01 00 FF 18 00");
    check(&RIVAL_5, "z2_color", Input::Color(rgb(0xFF1800)), "02 00 21 02 00 00 00 00 FF 18 00");
    check(&RIVAL_5, "z3_color", Input::Color(rgb(0xFF1800)), "02 00 21 04 00 00 00 00 00 00 00 FF 18 00");
    check(&RIVAL_5, "z4_color", Input::Color(rgb(0xFF1800)), "02 00 21 08 00 00 00 00 00 00 00 00 00 00 FF 18 00");
    check(&RIVAL_5, "z5_color", Input::Color(rgb(0xFF1800)), "02 00 21 10 00 00 00 00 00 00 00 00 00 00 00 00 00 FF 18 00");
    check(&RIVAL_5, "z6_color", Input::Color(rgb(0xFF1800)), "02 00 21 20 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 FF 18 00");
    check(&RIVAL_5, "z7_color", Input::Color(rgb(0xFF1800)), "02 00 21 40 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 FF 18 00");
    check(&RIVAL_5, "z8_color", Input::Color(rgb(0xFF1800)), "02 00 21 80 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 FF 18 00");
    check(&RIVAL_5, "z9_color", Input::Color(rgb(0xFF1800)), "02 00 21 00 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 FF 18 00");
    check(&RIVAL_5, "logo_color", Input::Color(rgb(0xFF1800)), "02 00 21 00 02 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         FF 18 00");
    check(&RIVAL_5, "reactive_color", Input::Reactive(Some(rgb(0xFFFFFF))), "02 00 26 01 00 FF FF FF");
    check(&RIVAL_5, "led_brightness", Input::Choice("100"), "02 00 23 64");
    check(&RIVAL_5, "buttons", Input::Buttons(&map(&[("button1", "button1"), ("button2", "button2"), ("button3", "button3"), ("button4", "button4"), ("button5", "button5"), ("button6", "disabled"), ("button7", "disabled"), ("button8", "disabled"), ("button9", "dpi"), ("scrollup", "scrollup"), ("scrolldown", "scrolldown"), ("layout", "qwerty")])), "02 00 2A 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 00 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 30 00 00 00 00 31 00 00 00 00 32 00 00 00 00");
    check(&RIVAL_5, "rainbow_effect", Input::Trigger, "02 00 22 FF 03");
    check(&RIVAL_5, "default_lighting", Input::Choice("rainbow"), "02 00 27 01");
    check_save(&RIVAL_5, "02 00 11 00");
}

// ---- RIVAL_95 --------------------------------------------------------------------------------

/// rivalcfg test/devices/old_specs/test_rival95.py::TestDevice::test_save
#[test]
fn rival95_save() {
    check_save(&RIVAL_95, "02 00 09 00");
}

/// rivalcfg test/devices/old_specs/test_rival95.py::TestDevice::test_set_btn6_mode
#[test]
fn rival95_set_btn6_mode() {
    check(&RIVAL_95, "btn6_mode", Input::Choice("dpi"), "02 00 0B 00");
    check(&RIVAL_95, "btn6_mode", Input::Choice("os"), "02 00 0B 01");
}

/// rivalcfg test/devices/old_specs/test_rival95.py::TestDevice::test_set_polling_rate
#[test]
fn rival95_set_polling_rate() {
    check(&RIVAL_95, "polling_rate", Input::Choice("125"), "02 00 04 00 04");
    check(&RIVAL_95, "polling_rate", Input::Choice("250"), "02 00 04 00 03");
    check(&RIVAL_95, "polling_rate", Input::Choice("500"), "02 00 04 00 02");
    check(&RIVAL_95, "polling_rate", Input::Choice("1000"), "02 00 04 00 01");
}

/// rivalcfg test/devices/old_specs/test_rival95.py::TestDevice::test_set_sensitivity1
#[test]
fn rival95_set_sensitivity1() {
    check(&RIVAL_95, "sensitivity1", Input::Choice("250"), "02 00 03 01 08");
    check(&RIVAL_95, "sensitivity1", Input::Choice("500"), "02 00 03 01 07");
    check(&RIVAL_95, "sensitivity1", Input::Choice("1000"), "02 00 03 01 06");
    check(&RIVAL_95, "sensitivity1", Input::Choice("1250"), "02 00 03 01 05");
    check(&RIVAL_95, "sensitivity1", Input::Choice("1500"), "02 00 03 01 04");
    check(&RIVAL_95, "sensitivity1", Input::Choice("1750"), "02 00 03 01 03");
    check(&RIVAL_95, "sensitivity1", Input::Choice("2000"), "02 00 03 01 02");
    check(&RIVAL_95, "sensitivity1", Input::Choice("4000"), "02 00 03 01 01");
}

/// rivalcfg test/devices/old_specs/test_rival95.py::TestDevice::test_set_sensitivity2
#[test]
fn rival95_set_sensitivity2() {
    check(&RIVAL_95, "sensitivity2", Input::Choice("250"), "02 00 03 02 08");
    check(&RIVAL_95, "sensitivity2", Input::Choice("500"), "02 00 03 02 07");
    check(&RIVAL_95, "sensitivity2", Input::Choice("1000"), "02 00 03 02 06");
    check(&RIVAL_95, "sensitivity2", Input::Choice("1250"), "02 00 03 02 05");
    check(&RIVAL_95, "sensitivity2", Input::Choice("1500"), "02 00 03 02 04");
    check(&RIVAL_95, "sensitivity2", Input::Choice("1750"), "02 00 03 02 03");
    check(&RIVAL_95, "sensitivity2", Input::Choice("2000"), "02 00 03 02 02");
    check(&RIVAL_95, "sensitivity2", Input::Choice("4000"), "02 00 03 02 01");
}

/// Defaults of every `RIVAL_95` setting and its save/battery/firmware commands, as rivalcfg f16c521
/// encodes them (computed by running rivalcfg, not copied from its test files).
#[test]
fn rival95_rivalcfg_defaults() {
    check(&RIVAL_95, "sensitivity1", Input::Choice("1000"), "02 00 03 01 06");
    check(&RIVAL_95, "sensitivity2", Input::Choice("2000"), "02 00 03 02 02");
    check(&RIVAL_95, "polling_rate", Input::Choice("1000"), "02 00 04 00 01");
    check(&RIVAL_95, "btn6_mode", Input::Choice("dpi"), "02 00 0B 00");
    check_save(&RIVAL_95, "02 00 09 00");
}

// ---- RIVAL_100 -------------------------------------------------------------------------------

/// rivalcfg test/devices/specs/rival100.txt, `sensitivity1` cases (each followed by the CLI's save packet)
#[test]
fn rival100_spec_sensitivity1() {
    check(&RIVAL_100, "sensitivity1", Input::Choice("250"), "02 00 03 01 08");
    check_save(&RIVAL_100, "02 00 09 00");
    check(&RIVAL_100, "sensitivity1", Input::Choice("500"), "02 00 03 01 07");
    check(&RIVAL_100, "sensitivity1", Input::Choice("1000"), "02 00 03 01 06");
    check(&RIVAL_100, "sensitivity1", Input::Choice("1250"), "02 00 03 01 05");
    check(&RIVAL_100, "sensitivity1", Input::Choice("1500"), "02 00 03 01 04");
    check(&RIVAL_100, "sensitivity1", Input::Choice("1750"), "02 00 03 01 03");
    check(&RIVAL_100, "sensitivity1", Input::Choice("2000"), "02 00 03 01 02");
    check(&RIVAL_100, "sensitivity1", Input::Choice("4000"), "02 00 03 01 01");
}

/// rivalcfg test/devices/specs/rival100.txt, `sensitivity2` cases (each followed by the CLI's save packet)
#[test]
fn rival100_spec_sensitivity2() {
    check(&RIVAL_100, "sensitivity2", Input::Choice("250"), "02 00 03 02 08");
    check_save(&RIVAL_100, "02 00 09 00");
    check(&RIVAL_100, "sensitivity2", Input::Choice("500"), "02 00 03 02 07");
    check(&RIVAL_100, "sensitivity2", Input::Choice("1000"), "02 00 03 02 06");
    check(&RIVAL_100, "sensitivity2", Input::Choice("1250"), "02 00 03 02 05");
    check(&RIVAL_100, "sensitivity2", Input::Choice("1500"), "02 00 03 02 04");
    check(&RIVAL_100, "sensitivity2", Input::Choice("1750"), "02 00 03 02 03");
    check(&RIVAL_100, "sensitivity2", Input::Choice("2000"), "02 00 03 02 02");
    check(&RIVAL_100, "sensitivity2", Input::Choice("4000"), "02 00 03 02 01");
}

/// rivalcfg test/devices/specs/rival100.txt, `polling_rate` cases (each followed by the CLI's save packet)
#[test]
fn rival100_spec_polling_rate() {
    check(&RIVAL_100, "polling_rate", Input::Choice("125"), "02 00 04 00 04");
    check_save(&RIVAL_100, "02 00 09 00");
    check(&RIVAL_100, "polling_rate", Input::Choice("250"), "02 00 04 00 03");
    check(&RIVAL_100, "polling_rate", Input::Choice("500"), "02 00 04 00 02");
    check(&RIVAL_100, "polling_rate", Input::Choice("1000"), "02 00 04 00 01");
}

/// rivalcfg test/devices/specs/rival100.txt, `color` cases (each followed by the CLI's save packet)
#[test]
fn rival100_spec_color() {
    check(&RIVAL_100, "color", Input::Color(rgb(0xABCDEF)), "02 00 05 00 AB CD EF");
    check_save(&RIVAL_100, "02 00 09 00");
    check(&RIVAL_100, "color", Input::Color(rgb(0xFF0000)), "02 00 05 00 FF 00 00");
}

/// rivalcfg test/devices/specs/rival100.txt, `light_effect` cases (each followed by the CLI's save packet)
#[test]
fn rival100_spec_light_effect() {
    check(&RIVAL_100, "light_effect", Input::Choice("steady"), "02 00 07 00 01");
    check_save(&RIVAL_100, "02 00 09 00");
    check(&RIVAL_100, "light_effect", Input::Choice("breath"), "02 00 07 00 03");
}

/// rivalcfg test/devices/specs/rival100.txt, `btn6_mode` cases (each followed by the CLI's save packet)
#[test]
fn rival100_spec_btn6_mode() {
    check(&RIVAL_100, "btn6_mode", Input::Choice("dpi"), "02 00 0B 00");
    check_save(&RIVAL_100, "02 00 09 00");
    check(&RIVAL_100, "btn6_mode", Input::Choice("os"), "02 00 0B 01");
}

/// rivalcfg test/devices/specs/rival100.txt, `firmware_version` cases (each followed by the CLI's save packet)
#[test]
fn rival100_spec_firmware_version() {
    check_firmware(&RIVAL_100, "02 00 10 00");
}

/// Defaults of every `RIVAL_100` setting and its save/battery/firmware commands, as rivalcfg f16c521
/// encodes them (computed by running rivalcfg, not copied from its test files).
#[test]
fn rival100_rivalcfg_defaults() {
    check(&RIVAL_100, "sensitivity1", Input::Choice("1000"), "02 00 03 01 06");
    check(&RIVAL_100, "sensitivity2", Input::Choice("2000"), "02 00 03 02 02");
    check(&RIVAL_100, "polling_rate", Input::Choice("1000"), "02 00 04 00 01");
    check(&RIVAL_100, "color", Input::Color(rgb(0xFF1800)), "02 00 05 00 FF 18 00");
    check(&RIVAL_100, "light_effect", Input::Choice("steady"), "02 00 07 00 01");
    check(&RIVAL_100, "btn6_mode", Input::Choice("dpi"), "02 00 0B 00");
    check_save(&RIVAL_100, "02 00 09 00");
    check_firmware(&RIVAL_100, "02 00 10 00");
}

// ---- RIVAL_110 -------------------------------------------------------------------------------

/// rivalcfg test/devices/old_specs/test_rival110.py::TestDevice::test_save
#[test]
fn rival110_save() {
    check_save_padded(&RIVAL_110, "02 00 09 00", 34);
}

/// rivalcfg test/devices/old_specs/test_rival110.py::TestDevice::test_set_btn6_mode
#[test]
fn rival110_set_btn6_mode() {
    check_padded(&RIVAL_110, "btn6_mode", Input::Choice("dpi"), "02 00 0B 00", 34);
    check_padded(&RIVAL_110, "btn6_mode", Input::Choice("os"), "02 00 0B 01", 34);
}

/// rivalcfg test/devices/old_specs/test_rival110.py::TestDevice::test_set_color
#[test]
fn rival110_set_color() {
    check_padded(&RIVAL_110, "color", Input::Color(rgb(0xABCDEF)), "02 00 05 00 AB CD EF", 34);
    check_padded(&RIVAL_110, "color", Input::Color(rgb(0xFF0000)), "02 00 05 00 FF 00 00", 34);
}

/// rivalcfg test/devices/old_specs/test_rival110.py::TestDevice::test_set_light_effect
#[test]
fn rival110_set_light_effect() {
    check_padded(&RIVAL_110, "light_effect", Input::Choice("steady"), "02 00 07 00 01", 34);
    check_padded(&RIVAL_110, "light_effect", Input::Choice("breath"), "02 00 07 00 03", 34);
    check_padded(&RIVAL_110, "light_effect", Input::Choice("1"), "02 00 07 00 01", 34);
    check_padded(&RIVAL_110, "light_effect", Input::Choice("2"), "02 00 07 00 02", 34);
    check_padded(&RIVAL_110, "light_effect", Input::Choice("3"), "02 00 07 00 03", 34);
    check_padded(&RIVAL_110, "light_effect", Input::Choice("4"), "02 00 07 00 04", 34);
}

/// rivalcfg test/devices/old_specs/test_rival110.py::TestDevice::test_set_polling_rate
#[test]
fn rival110_set_polling_rate() {
    check_padded(&RIVAL_110, "polling_rate", Input::Choice("125"), "02 00 04 00 04", 34);
    check_padded(&RIVAL_110, "polling_rate", Input::Choice("250"), "02 00 04 00 03", 34);
    check_padded(&RIVAL_110, "polling_rate", Input::Choice("500"), "02 00 04 00 02", 34);
    check_padded(&RIVAL_110, "polling_rate", Input::Choice("1000"), "02 00 04 00 01", 34);
}

/// rivalcfg test/devices/old_specs/test_rival110.py::TestDevice::test_set_sensitivity1
#[test]
fn rival110_set_sensitivity1() {
    check_padded(&RIVAL_110, "sensitivity1", Input::Int(200), "02 00 03 01 04", 34);
    check_padded(&RIVAL_110, "sensitivity1", Input::Int(210), "02 00 03 01 04", 34);
    check_padded(&RIVAL_110, "sensitivity1", Input::Int(290), "02 00 03 01 06", 34);
    check_padded(&RIVAL_110, "sensitivity1", Input::Int(4000), "02 00 03 01 5C", 34);
    check_padded(&RIVAL_110, "sensitivity1", Input::Int(7200), "02 00 03 01 A7", 34);
}

/// rivalcfg test/devices/old_specs/test_rival110.py::TestDevice::test_set_sensitivity2
#[test]
fn rival110_set_sensitivity2() {
    check_padded(&RIVAL_110, "sensitivity2", Input::Int(200), "02 00 03 02 04", 34);
    check_padded(&RIVAL_110, "sensitivity2", Input::Int(210), "02 00 03 02 04", 34);
    check_padded(&RIVAL_110, "sensitivity2", Input::Int(290), "02 00 03 02 06", 34);
    check_padded(&RIVAL_110, "sensitivity2", Input::Int(4000), "02 00 03 02 5C", 34);
    check_padded(&RIVAL_110, "sensitivity2", Input::Int(7200), "02 00 03 02 A7", 34);
}

/// Defaults of every `RIVAL_110` setting and its save/battery/firmware commands, as rivalcfg f16c521
/// encodes them (computed by running rivalcfg, not copied from its test files).
#[test]
fn rival110_rivalcfg_defaults() {
    check(&RIVAL_110, "sensitivity1", Input::Int(800), "02 00 03 01 12 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00");
    check(&RIVAL_110, "sensitivity2", Input::Int(1600), "02 00 03 02 24 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00");
    check(&RIVAL_110, "polling_rate", Input::Choice("1000"), "02 00 04 00 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00");
    check(&RIVAL_110, "color", Input::Color(rgb(0xFF1800)), "02 00 05 00 FF 18 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00");
    check(&RIVAL_110, "light_effect", Input::Choice("steady"), "02 00 07 00 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00");
    check(&RIVAL_110, "btn6_mode", Input::Choice("dpi"), "02 00 0B 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00");
    check_save(&RIVAL_110, "02 00 09 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00");
}

// ---- RIVAL_300 -------------------------------------------------------------------------------

/// rivalcfg test/devices/old_specs/test_rival300.py::TestDevice::test_firmware_version
#[test]
fn rival300_firmware_version() {
    check_firmware(&RIVAL_300, "02 00 10 00");
}

/// rivalcfg test/devices/old_specs/test_rival300.py::TestDevice::test_save
#[test]
fn rival300_save() {
    check_save(&RIVAL_300, "02 00 09 00");
}

/// rivalcfg test/devices/old_specs/test_rival300.py::TestDevice::test_set_buttons_mapping
#[test]
fn rival300_set_buttons_mapping() {
    check(&RIVAL_300, "buttons", Input::Buttons(&map(&[])), "02 00 31 00 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 \
         00 00");
    check(&RIVAL_300, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 31 00 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 \
         00 00");
    check(&RIVAL_300, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 31 00 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 \
         00 00");
}

/// rivalcfg test/devices/old_specs/test_rival300.py::TestDevice::test_set_logo_color
#[test]
fn rival300_set_logo_color() {
    check(&RIVAL_300, "logo_color", Input::Color(rgb(0xABCDEF)), "02 00 08 01 AB CD EF");
    check(&RIVAL_300, "logo_color", Input::Color(rgb(0xFF0000)), "02 00 08 01 FF 00 00");
}

/// rivalcfg test/devices/old_specs/test_rival300.py::TestDevice::test_set_logo_light_effect
#[test]
fn rival300_set_logo_light_effect() {
    check(&RIVAL_300, "logo_light_effect", Input::Choice("steady"), "02 00 07 01 01");
    check(&RIVAL_300, "logo_light_effect", Input::Choice("breath"), "02 00 07 01 03");
    check(&RIVAL_300, "logo_light_effect", Input::Choice("1"), "02 00 07 01 01");
    check(&RIVAL_300, "logo_light_effect", Input::Choice("2"), "02 00 07 01 02");
    check(&RIVAL_300, "logo_light_effect", Input::Choice("3"), "02 00 07 01 03");
    check(&RIVAL_300, "logo_light_effect", Input::Choice("4"), "02 00 07 01 04");
}

/// rivalcfg test/devices/old_specs/test_rival300.py::TestDevice::test_set_polling_rate
#[test]
fn rival300_set_polling_rate() {
    check(&RIVAL_300, "polling_rate", Input::Choice("125"), "02 00 04 00 04");
    check(&RIVAL_300, "polling_rate", Input::Choice("250"), "02 00 04 00 03");
    check(&RIVAL_300, "polling_rate", Input::Choice("500"), "02 00 04 00 02");
    check(&RIVAL_300, "polling_rate", Input::Choice("1000"), "02 00 04 00 01");
}

/// rivalcfg test/devices/old_specs/test_rival300.py::TestDevice::test_set_sensitivity1
#[test]
fn rival300_set_sensitivity1() {
    check(&RIVAL_300, "sensitivity1", Input::Int(50), "02 00 03 01 01");
    check(&RIVAL_300, "sensitivity1", Input::Int(100), "02 00 03 01 02");
    check(&RIVAL_300, "sensitivity1", Input::Int(120), "02 00 03 01 02");
    check(&RIVAL_300, "sensitivity1", Input::Int(1000), "02 00 03 01 14");
    check(&RIVAL_300, "sensitivity1", Input::Int(2000), "02 00 03 01 28");
    check(&RIVAL_300, "sensitivity1", Input::Int(6500), "02 00 03 01 82");
}

/// rivalcfg test/devices/old_specs/test_rival300.py::TestDevice::test_set_sensitivity2
#[test]
fn rival300_set_sensitivity2() {
    check(&RIVAL_300, "sensitivity2", Input::Int(50), "02 00 03 02 01");
    check(&RIVAL_300, "sensitivity2", Input::Int(100), "02 00 03 02 02");
    check(&RIVAL_300, "sensitivity2", Input::Int(120), "02 00 03 02 02");
    check(&RIVAL_300, "sensitivity2", Input::Int(1000), "02 00 03 02 14");
    check(&RIVAL_300, "sensitivity2", Input::Int(2000), "02 00 03 02 28");
    check(&RIVAL_300, "sensitivity2", Input::Int(6500), "02 00 03 02 82");
}

/// rivalcfg test/devices/old_specs/test_rival300.py::TestDevice::test_set_wheel_color
#[test]
fn rival300_set_wheel_color() {
    check(&RIVAL_300, "wheel_color", Input::Color(rgb(0xABCDEF)), "02 00 08 02 AB CD EF");
    check(&RIVAL_300, "wheel_color", Input::Color(rgb(0xFF0000)), "02 00 08 02 FF 00 00");
}

/// rivalcfg test/devices/old_specs/test_rival300.py::TestDevice::test_set_wheel_light_effect
#[test]
fn rival300_set_wheel_light_effect() {
    check(&RIVAL_300, "wheel_light_effect", Input::Choice("steady"), "02 00 07 02 01");
    check(&RIVAL_300, "wheel_light_effect", Input::Choice("breath"), "02 00 07 02 03");
    check(&RIVAL_300, "wheel_light_effect", Input::Choice("1"), "02 00 07 02 01");
    check(&RIVAL_300, "wheel_light_effect", Input::Choice("2"), "02 00 07 02 02");
    check(&RIVAL_300, "wheel_light_effect", Input::Choice("3"), "02 00 07 02 03");
    check(&RIVAL_300, "wheel_light_effect", Input::Choice("4"), "02 00 07 02 04");
}

/// Defaults of every `RIVAL_300` setting and its save/battery/firmware commands, as rivalcfg f16c521
/// encodes them (computed by running rivalcfg, not copied from its test files).
#[test]
fn rival300_rivalcfg_defaults() {
    check(&RIVAL_300, "sensitivity1", Input::Int(800), "02 00 03 01 10");
    check(&RIVAL_300, "sensitivity2", Input::Int(1600), "02 00 03 02 20");
    check(&RIVAL_300, "polling_rate", Input::Choice("1000"), "02 00 04 00 01");
    check(&RIVAL_300, "logo_color", Input::Color(rgb(0xFF1800)), "02 00 08 01 FF 18 00");
    check(&RIVAL_300, "wheel_color", Input::Color(rgb(0xFF1800)), "02 00 08 02 FF 18 00");
    check(&RIVAL_300, "logo_light_effect", Input::Choice("steady"), "02 00 07 01 01");
    check(&RIVAL_300, "wheel_light_effect", Input::Choice("steady"), "02 00 07 02 01");
    check(&RIVAL_300, "buttons", Input::Buttons(&map(&[("button1", "button1"), ("button2", "button2"), ("button3", "button3"), ("button4", "button4"), ("button5", "button5"), ("button6", "dpi"), ("layout", "qwerty")])), "02 00 31 00 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 \
         00 00");
    check_save(&RIVAL_300, "02 00 09 00");
    check_firmware(&RIVAL_300, "02 00 10 00");
}

// ---- RIVAL_300S ------------------------------------------------------------------------------

/// rivalcfg test/devices/old_specs/test_rival300s.py::TestDevice::test_save
#[test]
fn rival300s_save() {
    check_save_padded(&RIVAL_300S, "02 00 09 00", 34);
}

/// rivalcfg test/devices/old_specs/test_rival300s.py::TestDevice::test_set_btn6_mode
#[test]
fn rival300s_set_btn6_mode() {
    check_padded(&RIVAL_300S, "btn6_mode", Input::Choice("dpi"), "02 00 0B 00", 34);
    check_padded(&RIVAL_300S, "btn6_mode", Input::Choice("os"), "02 00 0B 01", 34);
}

/// rivalcfg test/devices/old_specs/test_rival300s.py::TestDevice::test_set_color
#[test]
fn rival300s_set_color() {
    check_padded(&RIVAL_300S, "color", Input::Color(rgb(0xABCDEF)), "02 00 05 00 AB CD EF", 34);
    check_padded(&RIVAL_300S, "color", Input::Color(rgb(0xFF0000)), "02 00 05 00 FF 00 00", 34);
}

/// rivalcfg test/devices/old_specs/test_rival300s.py::TestDevice::test_set_light_effect
#[test]
fn rival300s_set_light_effect() {
    check_padded(&RIVAL_300S, "light_effect", Input::Choice("steady"), "02 00 07 00 01", 34);
    check_padded(&RIVAL_300S, "light_effect", Input::Choice("breath"), "02 00 07 00 03", 34);
    check_padded(&RIVAL_300S, "light_effect", Input::Choice("1"), "02 00 07 00 01", 34);
    check_padded(&RIVAL_300S, "light_effect", Input::Choice("2"), "02 00 07 00 02", 34);
    check_padded(&RIVAL_300S, "light_effect", Input::Choice("3"), "02 00 07 00 03", 34);
    check_padded(&RIVAL_300S, "light_effect", Input::Choice("4"), "02 00 07 00 04", 34);
}

/// rivalcfg test/devices/old_specs/test_rival300s.py::TestDevice::test_set_polling_rate
#[test]
fn rival300s_set_polling_rate() {
    check_padded(&RIVAL_300S, "polling_rate", Input::Choice("125"), "02 00 04 00 04", 34);
    check_padded(&RIVAL_300S, "polling_rate", Input::Choice("250"), "02 00 04 00 03", 34);
    check_padded(&RIVAL_300S, "polling_rate", Input::Choice("500"), "02 00 04 00 02", 34);
    check_padded(&RIVAL_300S, "polling_rate", Input::Choice("1000"), "02 00 04 00 01", 34);
}

/// rivalcfg test/devices/old_specs/test_rival300s.py::TestDevice::test_set_sensitivity1
#[test]
fn rival300s_set_sensitivity1() {
    check_padded(&RIVAL_300S, "sensitivity1", Input::Int(200), "02 00 03 01 04", 34);
    check_padded(&RIVAL_300S, "sensitivity1", Input::Int(210), "02 00 03 01 04", 34);
    check_padded(&RIVAL_300S, "sensitivity1", Input::Int(290), "02 00 03 01 06", 34);
    check_padded(&RIVAL_300S, "sensitivity1", Input::Int(4000), "02 00 03 01 5C", 34);
    check_padded(&RIVAL_300S, "sensitivity1", Input::Int(7200), "02 00 03 01 A7", 34);
}

/// rivalcfg test/devices/old_specs/test_rival300s.py::TestDevice::test_set_sensitivity2
#[test]
fn rival300s_set_sensitivity2() {
    check_padded(&RIVAL_300S, "sensitivity2", Input::Int(200), "02 00 03 02 04", 34);
    check_padded(&RIVAL_300S, "sensitivity2", Input::Int(210), "02 00 03 02 04", 34);
    check_padded(&RIVAL_300S, "sensitivity2", Input::Int(290), "02 00 03 02 06", 34);
    check_padded(&RIVAL_300S, "sensitivity2", Input::Int(4000), "02 00 03 02 5C", 34);
    check_padded(&RIVAL_300S, "sensitivity2", Input::Int(7200), "02 00 03 02 A7", 34);
}

/// Defaults of every `RIVAL_300S` setting and its save/battery/firmware commands, as rivalcfg f16c521
/// encodes them (computed by running rivalcfg, not copied from its test files).
#[test]
fn rival300s_rivalcfg_defaults() {
    check(&RIVAL_300S, "sensitivity1", Input::Int(800), "02 00 03 01 12 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00");
    check(&RIVAL_300S, "sensitivity2", Input::Int(1600), "02 00 03 02 24 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00");
    check(&RIVAL_300S, "polling_rate", Input::Choice("1000"), "02 00 04 00 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00");
    check(&RIVAL_300S, "color", Input::Color(rgb(0xFF1800)), "02 00 05 00 FF 18 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00");
    check(&RIVAL_300S, "light_effect", Input::Choice("steady"), "02 00 07 00 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00");
    check(&RIVAL_300S, "btn6_mode", Input::Choice("dpi"), "02 00 0B 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00");
    check_save(&RIVAL_300S, "02 00 09 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00");
}

// ---- RIVAL_310 -------------------------------------------------------------------------------

/// rivalcfg test/devices/old_specs/test_rival310.py::TestDevice::test_firmware_version
#[test]
fn rival310_firmware_version() {
    check_firmware(&RIVAL_310, "02 00 90 00");
}

/// rivalcfg test/devices/old_specs/test_rival310.py::TestDevice::test_save
#[test]
fn rival310_save() {
    check_save(&RIVAL_310, "02 00 59 00");
}

/// rivalcfg test/devices/old_specs/test_rival310.py::TestDevice::test_set_buttons_mapping
#[test]
fn rival310_set_buttons_mapping() {
    check(&RIVAL_310, "buttons", Input::Buttons(&map(&[])), "02 00 31 00 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 \
         00 00");
    check(&RIVAL_310, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 31 00 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 \
         00 00");
    check(&RIVAL_310, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 31 00 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 \
         00 00");
}

/// rivalcfg test/devices/old_specs/test_rival310.py::TestDevice::test_set_logo_color
#[test]
fn rival310_set_logo_color() {
    check(&RIVAL_310, "logo_color", Input::Effect(&gradient(1000, &[(0, 0xFF0000), (33, 0x00FF00), (66, 0x0000FF)])), "03 00 5B 00 00 E8 03 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 04 FF 00 \
         00 FF 00 00 00 00 FF 00 54 00 00 FF 54 FF 00 00 57");
}

/// rivalcfg test/devices/old_specs/test_rival310.py::TestDevice::test_set_polling_rate
#[test]
fn rival310_set_polling_rate() {
    check(&RIVAL_310, "polling_rate", Input::Choice("125"), "02 00 54 00 04");
    check(&RIVAL_310, "polling_rate", Input::Choice("250"), "02 00 54 00 03");
    check(&RIVAL_310, "polling_rate", Input::Choice("500"), "02 00 54 00 02");
    check(&RIVAL_310, "polling_rate", Input::Choice("1000"), "02 00 54 00 01");
}

/// rivalcfg test/devices/old_specs/test_rival310.py::TestDevice::test_set_sensitivity1
#[test]
fn rival310_set_sensitivity1() {
    check(&RIVAL_310, "sensitivity1", Input::Int(100), "02 00 53 00 01 00 00 42");
    check(&RIVAL_310, "sensitivity1", Input::Int(200), "02 00 53 00 01 01 00 42");
    check(&RIVAL_310, "sensitivity1", Input::Int(1000), "02 00 53 00 01 09 00 42");
    check(&RIVAL_310, "sensitivity1", Input::Int(12000), "02 00 53 00 01 77 00 42");
}

/// rivalcfg test/devices/old_specs/test_rival310.py::TestDevice::test_set_sensitivity2
#[test]
fn rival310_set_sensitivity2() {
    check(&RIVAL_310, "sensitivity2", Input::Int(100), "02 00 53 00 02 00 00 42");
    check(&RIVAL_310, "sensitivity2", Input::Int(200), "02 00 53 00 02 01 00 42");
    check(&RIVAL_310, "sensitivity2", Input::Int(1000), "02 00 53 00 02 09 00 42");
    check(&RIVAL_310, "sensitivity2", Input::Int(12000), "02 00 53 00 02 77 00 42");
}

/// rivalcfg test/devices/old_specs/test_rival310.py::TestDevice::test_set_wheel_color
#[test]
fn rival310_set_wheel_color() {
    check(&RIVAL_310, "wheel_color", Input::Effect(&gradient(5000, &[(0, 0x112233), (25, 0x445566), (50, 0x778899), (75, 0xAABBCC)])), "03 00 5B 00 01 88 13 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 05 11 22 \
         33 11 22 33 00 44 55 66 3F 77 88 99 40 AA BB CC 40 11 22 33 40");
}

/// rivalcfg test/devices/old_specs/test_rival310.py::TestDevice::test_set_wheel_color_with_color_string
#[test]
fn rival310_set_wheel_color_with_color_string() {
    check(&RIVAL_310, "wheel_color", Input::Color(rgb(0xFF1800)), "03 00 5B 00 01 E8 03 00 00 00 00 00 00 00 00 00 00 00 00 00 00 01 00 00 00 00 00 00 00 01 FF 18 \
         00 FF 18 00 00");
}

/// Defaults of every `RIVAL_310` setting and its save/battery/firmware commands, as rivalcfg f16c521
/// encodes them (computed by running rivalcfg, not copied from its test files).
#[test]
fn rival310_rivalcfg_defaults() {
    check(&RIVAL_310, "sensitivity1", Input::Int(800), "02 00 53 00 01 07 00 42");
    check(&RIVAL_310, "sensitivity2", Input::Int(1600), "02 00 53 00 02 0F 00 42");
    check(&RIVAL_310, "polling_rate", Input::Choice("1000"), "02 00 54 00 01");
    check(&RIVAL_310, "logo_color", Input::Effect(&gradient(1000, &[(0, 0xFF0000), (33, 0x00FF00), (66, 0x0000FF)])), "03 00 5B 00 00 E8 03 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 04 FF 00 \
         00 FF 00 00 00 00 FF 00 54 00 00 FF 54 FF 00 00 57");
    check(&RIVAL_310, "wheel_color", Input::Effect(&gradient(1000, &[(0, 0xFF0000), (33, 0x00FF00), (66, 0x0000FF)])), "03 00 5B 00 01 E8 03 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 04 FF 00 \
         00 FF 00 00 00 00 FF 00 54 00 00 FF 54 FF 00 00 57");
    check(&RIVAL_310, "buttons", Input::Buttons(&map(&[("button1", "button1"), ("button2", "button2"), ("button3", "button3"), ("button4", "button4"), ("button5", "button5"), ("button6", "dpi"), ("layout", "qwerty")])), "02 00 31 00 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 30 00 00 \
         00 00");
    check_save(&RIVAL_310, "02 00 59 00");
    check_firmware(&RIVAL_310, "02 00 90 00");
}

// ---- RIVAL_500 -------------------------------------------------------------------------------

/// rivalcfg test/devices/old_specs/test_rival500.py::TestDevice::test_firmware_version
#[test]
fn rival500_firmware_version() {
    check_firmware(&RIVAL_500, "02 00 10 00");
}

/// rivalcfg test/devices/old_specs/test_rival500.py::TestDevice::test_save
#[test]
fn rival500_save() {
    check_save(&RIVAL_500, "02 00 09 00");
}

/// rivalcfg test/devices/old_specs/test_rival500.py::TestDevice::test_set_buttons_mapping
#[test]
fn rival500_set_buttons_mapping() {
    check(&RIVAL_500, "buttons", Input::Buttons(&map(&[])), "03 00 31 00 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 06 00 00 \
         00 00 07 00 00 00 00 00 00 00 00 00 30 00 00 00 00 00 00 00 00 00 00 00 00 00 00 33 00 00 00 00 \
         34 00 00 00 00 00 00 00 00 00 08 00 00 00 00");
    check(&RIVAL_500, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "03 00 31 00 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 06 00 00 \
         00 00 07 00 00 00 00 00 00 00 00 00 30 00 00 00 00 00 00 00 00 00 00 00 00 00 00 33 00 00 00 00 \
         34 00 00 00 00 00 00 00 00 00 08 00 00 00 00");
}

/// rivalcfg test/devices/old_specs/test_rival500.py::TestDevice::test_set_polling_rate
#[test]
fn rival500_set_polling_rate() {
    check(&RIVAL_500, "polling_rate", Input::Choice("125"), "02 00 04 00 04");
    check(&RIVAL_500, "polling_rate", Input::Choice("250"), "02 00 04 00 03");
    check(&RIVAL_500, "polling_rate", Input::Choice("500"), "02 00 04 00 02");
    check(&RIVAL_500, "polling_rate", Input::Choice("1000"), "02 00 04 00 01");
}

/// rivalcfg test/devices/old_specs/test_rival500.py::TestDevice::test_set_sensitivity1
#[test]
fn rival500_set_sensitivity1() {
    check(&RIVAL_500, "sensitivity1", Input::Int(100), "02 00 03 00 01 00 00 42");
    check(&RIVAL_500, "sensitivity1", Input::Int(200), "02 00 03 00 01 01 00 42");
    check(&RIVAL_500, "sensitivity1", Input::Int(1000), "02 00 03 00 01 09 00 42");
    check(&RIVAL_500, "sensitivity1", Input::Int(12000), "02 00 03 00 01 77 00 42");
}

/// rivalcfg test/devices/old_specs/test_rival500.py::TestDevice::test_set_sensitivity2
#[test]
fn rival500_set_sensitivity2() {
    check(&RIVAL_500, "sensitivity2", Input::Int(100), "02 00 03 00 02 00 00 42");
    check(&RIVAL_500, "sensitivity2", Input::Int(200), "02 00 03 00 02 01 00 42");
    check(&RIVAL_500, "sensitivity2", Input::Int(1000), "02 00 03 00 02 09 00 42");
    check(&RIVAL_500, "sensitivity2", Input::Int(12000), "02 00 03 00 02 77 00 42");
}

/// rivalcfg test/devices/old_specs/test_rival500.py::TestDevice::test_set_wheel_color
#[test]
fn rival500_set_wheel_color() {
    check(&RIVAL_500, "wheel_color", Input::Effect(&gradient(5000, &[(0, 0x112233), (25, 0x445566), (50, 0x778899), (75, 0xAABBCC)])), "03 00 05 00 01 1D 01 02 31 51 FF C8 00 00 00 00 00 00 00 E2 04 01 00 00 00 00 00 E2 04 02 00 00 \
         00 00 00 E2 04 03 00 FF FF FF 00 E2 04 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 10 01 20 02 30 03 FF 00 DC 05 8A 02 00 00 00 00 01 \
         00 04 00 88 13");
}

/// rivalcfg test/devices/old_specs/test_rival500.py::TestDevice::test_set_wheel_color_with_color_string
#[test]
fn rival500_set_wheel_color_with_color_string() {
    check(&RIVAL_500, "wheel_color", Input::Color(rgb(0xFF1800)), "03 00 05 00 01 1D 01 02 31 51 FF C8 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 F0 0F 80 01 00 00 FF 00 DC 05 8A 02 00 00 00 00 01 \
         00 00 00 E8 03");
}

/// Defaults of every `RIVAL_500` setting and its save/battery/firmware commands, as rivalcfg f16c521
/// encodes them (computed by running rivalcfg, not copied from its test files).
#[test]
fn rival500_rivalcfg_defaults() {
    check(&RIVAL_500, "sensitivity1", Input::Int(800), "02 00 03 00 01 07 00 42");
    check(&RIVAL_500, "sensitivity2", Input::Int(1600), "02 00 03 00 02 0F 00 42");
    check(&RIVAL_500, "polling_rate", Input::Choice("1000"), "02 00 04 00 01");
    check(&RIVAL_500, "logo_color", Input::Effect(&gradient(1000, &[(0, 0xFF00E1), (33, 0xFFEA00), (66, 0x00CCFF)])), "03 00 05 00 00 1D 01 02 31 51 FF C8 00 00 00 00 0B F6 00 4A 01 01 00 F4 FF 0C 00 4A 01 02 00 0C \
         F7 FF 00 54 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 F0 0F 00 00 10 0E FF 00 DC 05 8A 02 00 00 00 00 01 \
         00 03 00 E8 03");
    check(&RIVAL_500, "wheel_color", Input::Effect(&gradient(1000, &[(0, 0xFF00E1), (33, 0xFFEA00), (66, 0x00CCFF)])), "03 00 05 00 01 1D 01 02 31 51 FF C8 00 00 00 00 0B F6 00 4A 01 01 00 F4 FF 0C 00 4A 01 02 00 0C \
         F7 FF 00 54 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 F0 0F 00 00 10 0E FF 00 DC 05 8A 02 00 00 00 00 01 \
         00 03 00 E8 03");
    check(&RIVAL_500, "buttons", Input::Buttons(&map(&[])), "03 00 31 00 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 06 00 00 \
         00 00 07 00 00 00 00 00 00 00 00 00 30 00 00 00 00 00 00 00 00 00 00 00 00 00 00 33 00 00 00 00 \
         34 00 00 00 00 00 00 00 00 00 08 00 00 00 00");
    check_save(&RIVAL_500, "02 00 09 00");
    check_firmware(&RIVAL_500, "02 00 10 00");
}

// ---- RIVAL_600 -------------------------------------------------------------------------------

/// rivalcfg test/devices/old_specs/test_rival600.py::TestDevice::test_save
#[test]
fn rival600_save() {
    check_save(&RIVAL_600, "02 00 09 00");
}

/// rivalcfg test/devices/old_specs/test_rival600.py::TestDevice::test_set_buttons_mapping
#[test]
fn rival600_set_buttons_mapping() {
    check(&RIVAL_600, "buttons", Input::Buttons(&map(&[])), "03 00 31 00 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 00 00 00 \
         00 00 30 00 00 00 00");
    check(&RIVAL_600, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "03 00 31 00 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 00 00 00 \
         00 00 30 00 00 00 00");
    check(&RIVAL_600, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "03 00 31 00 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 00 00 00 \
         00 00 30 00 00 00 00");
    check(&RIVAL_600, "buttons", Input::Buttons(&map(&[("Button1", "ScrollDown"), ("Button2", "ScrollUp")])), "03 00 31 00 32 00 00 00 00 31 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 00 00 00 \
         00 00 30 00 00 00 00");
}

/// rivalcfg test/devices/old_specs/test_rival600.py::TestDevice::test_set_logo_color
#[test]
fn rival600_set_logo_color() {
    check(&RIVAL_600, "logo_color", Input::Effect(&gradient(5000, &[(0, 0xFF0000), (33, 0x00FF00), (66, 0x0000FF)])), "03 00 05 00 01 00 00 00 00 01 88 13 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 04 \
         FF 00 00 FF 00 00 00 00 FF 00 54 00 00 FF 54 FF 00 00 57");
}

/// rivalcfg test/devices/old_specs/test_rival600.py::TestDevice::test_set_polling_rate
#[test]
fn rival600_set_polling_rate() {
    check(&RIVAL_600, "polling_rate", Input::Choice("125"), "02 00 04 00 04");
    check(&RIVAL_600, "polling_rate", Input::Choice("250"), "02 00 04 00 03");
    check(&RIVAL_600, "polling_rate", Input::Choice("500"), "02 00 04 00 02");
    check(&RIVAL_600, "polling_rate", Input::Choice("1000"), "02 00 04 00 01");
}

/// rivalcfg test/devices/old_specs/test_rival600.py::TestDevice::test_set_sensitivity1
#[test]
fn rival600_set_sensitivity1() {
    check(&RIVAL_600, "sensitivity1", Input::Int(100), "02 00 03 00 01 00 00 42");
    check(&RIVAL_600, "sensitivity1", Input::Int(200), "02 00 03 00 01 01 00 42");
    check(&RIVAL_600, "sensitivity1", Input::Int(1000), "02 00 03 00 01 09 00 42");
    check(&RIVAL_600, "sensitivity1", Input::Int(12000), "02 00 03 00 01 77 00 42");
}

/// rivalcfg test/devices/old_specs/test_rival600.py::TestDevice::test_set_sensitivity2
#[test]
fn rival600_set_sensitivity2() {
    check(&RIVAL_600, "sensitivity2", Input::Int(100), "02 00 03 00 02 00 00 42");
    check(&RIVAL_600, "sensitivity2", Input::Int(200), "02 00 03 00 02 01 00 42");
    check(&RIVAL_600, "sensitivity2", Input::Int(1000), "02 00 03 00 02 09 00 42");
    check(&RIVAL_600, "sensitivity2", Input::Int(12000), "02 00 03 00 02 77 00 42");
}

/// rivalcfg test/devices/old_specs/test_rival600.py::TestDevice::test_set_wheel_color
#[test]
fn rival600_set_wheel_color() {
    check(&RIVAL_600, "wheel_color", Input::Effect(&gradient(5000, &[(0, 0xFF0000), (33, 0x00FF00), (66, 0x0000FF)])), "03 00 05 00 00 00 00 00 00 00 88 13 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 04 \
         FF 00 00 FF 00 00 00 00 FF 00 54 00 00 FF 54 FF 00 00 57");
}

/// rivalcfg test/devices/old_specs/test_rival600.py::TestDevice::test_set_z2_color
#[test]
fn rival600_set_z2_color() {
    check(&RIVAL_600, "z2_color", Input::Effect(&gradient(5000, &[(0, 0xFF0000), (33, 0x00FF00), (66, 0x0000FF)])), "03 00 05 00 02 00 00 00 00 02 88 13 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 04 \
         FF 00 00 FF 00 00 00 00 FF 00 54 00 00 FF 54 FF 00 00 57");
}

/// rivalcfg test/devices/old_specs/test_rival600.py::TestDevice::test_set_z3_color
#[test]
fn rival600_set_z3_color() {
    check(&RIVAL_600, "z3_color", Input::Effect(&gradient(5000, &[(0, 0xFF0000), (33, 0x00FF00), (66, 0x0000FF)])), "03 00 05 00 03 00 00 00 00 03 88 13 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 04 \
         FF 00 00 FF 00 00 00 00 FF 00 54 00 00 FF 54 FF 00 00 57");
}

/// rivalcfg test/devices/old_specs/test_rival600.py::TestDevice::test_set_z4_color
#[test]
fn rival600_set_z4_color() {
    check(&RIVAL_600, "z4_color", Input::Effect(&gradient(5000, &[(0, 0xFF0000), (33, 0x00FF00), (66, 0x0000FF)])), "03 00 05 00 04 00 00 00 00 04 88 13 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 04 \
         FF 00 00 FF 00 00 00 00 FF 00 54 00 00 FF 54 FF 00 00 57");
}

/// rivalcfg test/devices/old_specs/test_rival600.py::TestDevice::test_set_z5_color
#[test]
fn rival600_set_z5_color() {
    check(&RIVAL_600, "z5_color", Input::Effect(&gradient(5000, &[(0, 0xFF0000), (33, 0x00FF00), (66, 0x0000FF)])), "03 00 05 00 05 00 00 00 00 05 88 13 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 04 \
         FF 00 00 FF 00 00 00 00 FF 00 54 00 00 FF 54 FF 00 00 57");
}

/// rivalcfg test/devices/old_specs/test_rival600.py::TestDevice::test_set_z6_color
#[test]
fn rival600_set_z6_color() {
    check(&RIVAL_600, "z6_color", Input::Effect(&gradient(5000, &[(0, 0xFF0000), (33, 0x00FF00), (66, 0x0000FF)])), "03 00 05 00 06 00 00 00 00 06 88 13 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 04 \
         FF 00 00 FF 00 00 00 00 FF 00 54 00 00 FF 54 FF 00 00 57");
}

/// rivalcfg test/devices/old_specs/test_rival600.py::TestDevice::test_set_z7_color
#[test]
fn rival600_set_z7_color() {
    check(&RIVAL_600, "z7_color", Input::Effect(&gradient(5000, &[(0, 0xFF0000), (33, 0x00FF00), (66, 0x0000FF)])), "03 00 05 00 07 00 00 00 00 07 88 13 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 04 \
         FF 00 00 FF 00 00 00 00 FF 00 54 00 00 FF 54 FF 00 00 57");
}

/// Defaults of every `RIVAL_600` setting and its save/battery/firmware commands, as rivalcfg f16c521
/// encodes them (computed by running rivalcfg, not copied from its test files).
#[test]
fn rival600_rivalcfg_defaults() {
    check(&RIVAL_600, "sensitivity1", Input::Int(800), "02 00 03 00 01 07 00 42");
    check(&RIVAL_600, "sensitivity2", Input::Int(1600), "02 00 03 00 02 0F 00 42");
    check(&RIVAL_600, "polling_rate", Input::Choice("1000"), "02 00 04 00 01");
    check(&RIVAL_600, "wheel_color", Input::Effect(&gradient(1000, &[(0, 0xFF0000), (33, 0x00FF00), (66, 0x0000FF)])), "03 00 05 00 00 00 00 00 00 00 E8 03 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 04 \
         FF 00 00 FF 00 00 00 00 FF 00 54 00 00 FF 54 FF 00 00 57");
    check(&RIVAL_600, "logo_color", Input::Effect(&gradient(1000, &[(0, 0xFF0000), (33, 0x00FF00), (66, 0x0000FF)])), "03 00 05 00 01 00 00 00 00 01 E8 03 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 04 \
         FF 00 00 FF 00 00 00 00 FF 00 54 00 00 FF 54 FF 00 00 57");
    check(&RIVAL_600, "z2_color", Input::Effect(&gradient(1000, &[(0, 0xFF0000), (33, 0x00FF00), (66, 0x0000FF)])), "03 00 05 00 02 00 00 00 00 02 E8 03 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 04 \
         FF 00 00 FF 00 00 00 00 FF 00 54 00 00 FF 54 FF 00 00 57");
    check(&RIVAL_600, "z3_color", Input::Effect(&gradient(1000, &[(0, 0xFF0000), (33, 0x00FF00), (66, 0x0000FF)])), "03 00 05 00 03 00 00 00 00 03 E8 03 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 04 \
         FF 00 00 FF 00 00 00 00 FF 00 54 00 00 FF 54 FF 00 00 57");
    check(&RIVAL_600, "z4_color", Input::Effect(&gradient(1000, &[(0, 0xFF0000), (33, 0x00FF00), (66, 0x0000FF)])), "03 00 05 00 04 00 00 00 00 04 E8 03 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 04 \
         FF 00 00 FF 00 00 00 00 FF 00 54 00 00 FF 54 FF 00 00 57");
    check(&RIVAL_600, "z5_color", Input::Effect(&gradient(1000, &[(0, 0xFF0000), (33, 0x00FF00), (66, 0x0000FF)])), "03 00 05 00 05 00 00 00 00 05 E8 03 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 04 \
         FF 00 00 FF 00 00 00 00 FF 00 54 00 00 FF 54 FF 00 00 57");
    check(&RIVAL_600, "z6_color", Input::Effect(&gradient(1000, &[(0, 0xFF0000), (33, 0x00FF00), (66, 0x0000FF)])), "03 00 05 00 06 00 00 00 00 06 E8 03 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 04 \
         FF 00 00 FF 00 00 00 00 FF 00 54 00 00 FF 54 FF 00 00 57");
    check(&RIVAL_600, "z7_color", Input::Effect(&gradient(1000, &[(0, 0xFF0000), (33, 0x00FF00), (66, 0x0000FF)])), "03 00 05 00 07 00 00 00 00 07 E8 03 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 04 \
         FF 00 00 FF 00 00 00 00 FF 00 54 00 00 FF 54 FF 00 00 57");
    check(&RIVAL_600, "buttons", Input::Buttons(&map(&[("button1", "button1"), ("button2", "button2"), ("button3", "button3"), ("button4", "button4"), ("button5", "button5"), ("button6", "disabled"), ("button7", "dpi")])), "03 00 31 00 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 00 00 00 \
         00 00 30 00 00 00 00");
    check_save(&RIVAL_600, "02 00 09 00");
}

// ---- RIVAL_650 -------------------------------------------------------------------------------

/// rivalcfg test/devices/specs/rival650.txt, `sensitivity1` cases (each followed by the CLI's save packet)
#[test]
fn rival650_spec_sensitivity1() {
    check(&RIVAL_650, "sensitivity1", Input::Int(100), "02 00 15 01 00");
    check_save(&RIVAL_650, "02 00 09");
    check(&RIVAL_650, "sensitivity1", Input::Int(200), "02 00 15 01 01");
    check(&RIVAL_650, "sensitivity1", Input::Int(1000), "02 00 15 01 09");
    check(&RIVAL_650, "sensitivity1", Input::Int(12000), "02 00 15 01 77");
}

/// rivalcfg test/devices/specs/rival650.txt, `sensitivity2` cases (each followed by the CLI's save packet)
#[test]
fn rival650_spec_sensitivity2() {
    check(&RIVAL_650, "sensitivity2", Input::Int(100), "02 00 15 02 00");
    check_save(&RIVAL_650, "02 00 09");
    check(&RIVAL_650, "sensitivity2", Input::Int(200), "02 00 15 02 01");
    check(&RIVAL_650, "sensitivity2", Input::Int(1000), "02 00 15 02 09");
    check(&RIVAL_650, "sensitivity2", Input::Int(12000), "02 00 15 02 77");
}

/// rivalcfg test/devices/specs/rival650.txt, `polling_rate` cases (each followed by the CLI's save packet)
#[test]
fn rival650_spec_polling_rate() {
    check(&RIVAL_650, "polling_rate", Input::Choice("125"), "02 00 17 04");
    check_save(&RIVAL_650, "02 00 09");
    check(&RIVAL_650, "polling_rate", Input::Choice("250"), "02 00 17 03");
    check(&RIVAL_650, "polling_rate", Input::Choice("500"), "02 00 17 02");
    check(&RIVAL_650, "polling_rate", Input::Choice("1000"), "02 00 17 01");
}

/// rivalcfg test/devices/specs/rival650.txt, `lift_off_distance` cases (each followed by the CLI's save packet)
#[test]
fn rival650_spec_lift_off_distance() {
    check(&RIVAL_650, "lift_off_distance", Input::Int(1), "02 00 20 01 74 78");
    check_save(&RIVAL_650, "02 00 09");
    check(&RIVAL_650, "lift_off_distance", Input::Int(2), "02 00 20 01 6F 73");
    check(&RIVAL_650, "lift_off_distance", Input::Int(3), "02 00 20 01 6A 6E");
    check(&RIVAL_650, "lift_off_distance", Input::Int(4), "02 00 20 01 65 69");
    check(&RIVAL_650, "lift_off_distance", Input::Int(5), "02 00 20 01 60 64");
    check(&RIVAL_650, "lift_off_distance", Input::Int(6), "02 00 20 01 5B 5F");
    check(&RIVAL_650, "lift_off_distance", Input::Int(7), "02 00 20 01 56 5A");
    check(&RIVAL_650, "lift_off_distance", Input::Int(8), "02 00 20 01 51 55");
}

/// rivalcfg test/devices/specs/rival650.txt, `buttons_mapping` cases (each followed by the CLI's save packet)
#[test]
fn rival650_spec_buttons_mapping() {
    check(&RIVAL_650, "buttons", Input::Buttons(&map(&[])), "02 00 19 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 00 00 00 00 \
         00 30 00 00 00 00");
    check_save(&RIVAL_650, "02 00 09");
    check(&RIVAL_650, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 19 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 00 00 00 00 \
         00 30 00 00 00 00");
    check(&RIVAL_650, "buttons", Input::Buttons(&map(&[("Button1", "ScrollDown"), ("Button2", "ScrollUp")])), "02 00 19 32 00 00 00 00 31 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 00 00 00 00 \
         00 30 00 00 00 00");
}

/// rivalcfg test/devices/specs/rival650.txt, `sleep_timer` cases (each followed by the CLI's save packet)
#[test]
fn rival650_spec_sleep_timer() {
    check(&RIVAL_650, "sleep_timer", Input::Int(1), "02 00 2B 01 01 00 00 00 3C 00");
    check_save(&RIVAL_650, "02 00 09");
    check(&RIVAL_650, "sleep_timer", Input::Int(2), "02 00 2B 01 01 00 00 00 78 00");
    check(&RIVAL_650, "sleep_timer", Input::Int(5), "02 00 2B 01 01 00 00 00 2C 01");
    check(&RIVAL_650, "sleep_timer", Input::Int(20), "02 00 2B 01 01 00 00 00 B0 04");
}

/// rivalcfg test/devices/specs/rival650.txt, `battery_level` cases (each followed by the CLI's save packet)
#[test]
fn rival650_spec_battery_level() {
    check_battery(&RIVAL_650, "02 00 AA 01");
}

/// Defaults of every `RIVAL_650` setting and its save/battery/firmware commands, as rivalcfg f16c521
/// encodes them (computed by running rivalcfg, not copied from its test files).
#[test]
fn rival650_rivalcfg_defaults() {
    check(&RIVAL_650, "sensitivity1", Input::Int(800), "02 00 15 01 07");
    check(&RIVAL_650, "sensitivity2", Input::Int(1600), "02 00 15 02 0F");
    check(&RIVAL_650, "polling_rate", Input::Choice("1000"), "02 00 17 01");
    check(&RIVAL_650, "lift_off_distance", Input::Int(8), "02 00 20 01 51 55");
    check(&RIVAL_650, "buttons", Input::Buttons(&map(&[("button1", "button1"), ("button2", "button2"), ("button3", "button3"), ("button4", "button4"), ("button5", "button5"), ("button6", "disabled"), ("button7", "dpi")])), "02 00 19 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 00 00 00 00 \
         00 30 00 00 00 00");
    check(&RIVAL_650, "sleep_timer", Input::Int(5), "02 00 2B 01 01 00 00 00 2C 01");
    check_save(&RIVAL_650, "02 00 09");
    check_battery(&RIVAL_650, "02 00 AA 01");
}

// ---- RIVAL_700 -------------------------------------------------------------------------------

/// rivalcfg test/devices/old_specs/test_rival700.py::TestDevice::test_firmware_version
#[test]
fn rival700_firmware_version() {
    check_firmware(&RIVAL_700, "02 00 10 00");
}

/// rivalcfg test/devices/old_specs/test_rival700.py::TestDevice::test_save
#[test]
fn rival700_save() {
    check_save(&RIVAL_700, "02 00 09 00");
}

/// rivalcfg test/devices/old_specs/test_rival700.py::TestDevice::test_set_logo_color
#[test]
fn rival700_set_logo_color() {
    check(&RIVAL_700, "logo_color", Input::Effect(&gradient(1000, &[(0, 0xFF0000), (33, 0x00FF00), (66, 0x0000FF)])), "03 00 05 00 00 1D 01 02 31 51 FF C8 00 00 00 F4 0C 00 00 4A 01 01 00 00 F4 0C 00 4A 01 02 00 0C \
         00 F4 00 54 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 F0 0F 00 00 00 00 FF 00 DC 05 8A 02 00 00 00 00 01 \
         00 03 00 E8 03");
}

/// rivalcfg test/devices/old_specs/test_rival700.py::TestDevice::test_set_polling_rate
#[test]
fn rival700_set_polling_rate() {
    check(&RIVAL_700, "polling_rate", Input::Choice("125"), "02 00 04 00 04");
    check(&RIVAL_700, "polling_rate", Input::Choice("250"), "02 00 04 00 03");
    check(&RIVAL_700, "polling_rate", Input::Choice("500"), "02 00 04 00 02");
    check(&RIVAL_700, "polling_rate", Input::Choice("1000"), "02 00 04 00 01");
}

/// rivalcfg test/devices/old_specs/test_rival700.py::TestDevice::test_set_sensitivity1
#[test]
fn rival700_set_sensitivity1() {
    check(&RIVAL_700, "sensitivity1", Input::Int(100), "02 00 03 00 01 00 00 42");
    check(&RIVAL_700, "sensitivity1", Input::Int(200), "02 00 03 00 01 01 00 42");
    check(&RIVAL_700, "sensitivity1", Input::Int(1000), "02 00 03 00 01 09 00 42");
    check(&RIVAL_700, "sensitivity1", Input::Int(12000), "02 00 03 00 01 77 00 42");
}

/// rivalcfg test/devices/old_specs/test_rival700.py::TestDevice::test_set_sensitivity2
#[test]
fn rival700_set_sensitivity2() {
    check(&RIVAL_700, "sensitivity2", Input::Int(100), "02 00 03 00 02 00 00 42");
    check(&RIVAL_700, "sensitivity2", Input::Int(200), "02 00 03 00 02 01 00 42");
    check(&RIVAL_700, "sensitivity2", Input::Int(1000), "02 00 03 00 02 09 00 42");
    check(&RIVAL_700, "sensitivity2", Input::Int(12000), "02 00 03 00 02 77 00 42");
}

/// rivalcfg test/devices/old_specs/test_rival700.py::TestDevice::test_set_wheel_color
#[test]
fn rival700_set_wheel_color() {
    check(&RIVAL_700, "wheel_color", Input::Effect(&gradient(5000, &[(0, 0x112233), (25, 0x445566), (50, 0x778899), (75, 0xAABBCC)])), "03 00 05 00 01 1D 01 02 31 51 FF C8 00 00 00 00 00 00 00 E2 04 01 00 00 00 00 00 E2 04 02 00 00 \
         00 00 00 E2 04 03 00 FF FF FF 00 E2 04 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 10 01 20 02 30 03 FF 00 DC 05 8A 02 00 00 00 00 01 \
         00 04 00 88 13");
}

/// rivalcfg test/devices/old_specs/test_rival700.py::TestDevice::test_set_wheel_color_with_color_string
#[test]
fn rival700_set_wheel_color_with_color_string() {
    check(&RIVAL_700, "wheel_color", Input::Color(rgb(0xFF1800)), "03 00 05 00 01 1D 01 02 31 51 FF C8 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 F0 0F 80 01 00 00 FF 00 DC 05 8A 02 00 00 00 00 01 \
         00 00 00 E8 03");
}

/// Defaults of every `RIVAL_700` setting and its save/battery/firmware commands, as rivalcfg f16c521
/// encodes them (computed by running rivalcfg, not copied from its test files).
#[test]
fn rival700_rivalcfg_defaults() {
    check(&RIVAL_700, "sensitivity1", Input::Int(800), "02 00 03 00 01 07 00 42");
    check(&RIVAL_700, "sensitivity2", Input::Int(1600), "02 00 03 00 02 0F 00 42");
    check(&RIVAL_700, "polling_rate", Input::Choice("1000"), "02 00 04 00 01");
    check(&RIVAL_700, "logo_color", Input::Effect(&gradient(1000, &[(0, 0xFF00E1), (33, 0xFFEA00), (66, 0x00CCFF)])), "03 00 05 00 00 1D 01 02 31 51 FF C8 00 00 00 00 0B F6 00 4A 01 01 00 F4 FF 0C 00 4A 01 02 00 0C \
         F7 FF 00 54 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 F0 0F 00 00 10 0E FF 00 DC 05 8A 02 00 00 00 00 01 \
         00 03 00 E8 03");
    check(&RIVAL_700, "wheel_color", Input::Effect(&gradient(1000, &[(0, 0xFF00E1), (33, 0xFFEA00), (66, 0x00CCFF)])), "03 00 05 00 01 1D 01 02 31 51 FF C8 00 00 00 00 0B F6 00 4A 01 01 00 F4 FF 0C 00 4A 01 02 00 0C \
         F7 FF 00 54 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 \
         00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 F0 0F 00 00 10 0E FF 00 DC 05 8A 02 00 00 00 00 01 \
         00 03 00 E8 03");
    check_save(&RIVAL_700, "02 00 09 00");
    check_firmware(&RIVAL_700, "02 00 10 00");
}

// ---- SENSEI_310 ------------------------------------------------------------------------------

/// rivalcfg test/devices/old_specs/test_sensei310.py::TestDevice::test_firmware_version
#[test]
fn sensei310_firmware_version() {
    check_firmware(&SENSEI_310, "02 00 90 00");
}

/// rivalcfg test/devices/old_specs/test_sensei310.py::TestDevice::test_save
#[test]
fn sensei310_save() {
    check_save(&SENSEI_310, "02 00 59 00");
}

/// rivalcfg test/devices/old_specs/test_sensei310.py::TestDevice::test_set_buttons_mapping
#[test]
fn sensei310_set_buttons_mapping() {
    check(&SENSEI_310, "buttons", Input::Buttons(&map(&[])), "02 00 31 00 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 51 4E 00 \
         00 00 51 4B 00 00 00 30 00 00 00 00");
    check(&SENSEI_310, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 31 00 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 51 4E 00 \
         00 00 51 4B 00 00 00 30 00 00 00 00");
    check(&SENSEI_310, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 31 00 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 51 4E 00 \
         00 00 51 4B 00 00 00 30 00 00 00 00");
}

/// rivalcfg test/devices/old_specs/test_sensei310.py::TestDevice::test_set_logo_color
#[test]
fn sensei310_set_logo_color() {
    check(&SENSEI_310, "logo_color", Input::Effect(&gradient(1000, &[(0, 0xFF0000), (33, 0x00FF00), (66, 0x0000FF)])), "03 00 5B 00 00 E8 03 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 04 FF 00 \
         00 FF 00 00 00 00 FF 00 54 00 00 FF 54 FF 00 00 57");
}

/// rivalcfg test/devices/old_specs/test_sensei310.py::TestDevice::test_set_polling_rate
#[test]
fn sensei310_set_polling_rate() {
    check(&SENSEI_310, "polling_rate", Input::Choice("125"), "02 00 54 00 04");
    check(&SENSEI_310, "polling_rate", Input::Choice("250"), "02 00 54 00 03");
    check(&SENSEI_310, "polling_rate", Input::Choice("500"), "02 00 54 00 02");
    check(&SENSEI_310, "polling_rate", Input::Choice("1000"), "02 00 54 00 01");
}

/// rivalcfg test/devices/old_specs/test_sensei310.py::TestDevice::test_set_sensitivity1
#[test]
fn sensei310_set_sensitivity1() {
    check(&SENSEI_310, "sensitivity1", Input::Int(100), "02 00 53 00 01 00 00 42");
    check(&SENSEI_310, "sensitivity1", Input::Int(200), "02 00 53 00 01 01 00 42");
    check(&SENSEI_310, "sensitivity1", Input::Int(1000), "02 00 53 00 01 09 00 42");
    check(&SENSEI_310, "sensitivity1", Input::Int(12000), "02 00 53 00 01 77 00 42");
}

/// rivalcfg test/devices/old_specs/test_sensei310.py::TestDevice::test_set_sensitivity2
#[test]
fn sensei310_set_sensitivity2() {
    check(&SENSEI_310, "sensitivity2", Input::Int(100), "02 00 53 00 02 00 00 42");
    check(&SENSEI_310, "sensitivity2", Input::Int(200), "02 00 53 00 02 01 00 42");
    check(&SENSEI_310, "sensitivity2", Input::Int(1000), "02 00 53 00 02 09 00 42");
    check(&SENSEI_310, "sensitivity2", Input::Int(12000), "02 00 53 00 02 77 00 42");
}

/// rivalcfg test/devices/old_specs/test_sensei310.py::TestDevice::test_set_wheel_color
#[test]
fn sensei310_set_wheel_color() {
    check(&SENSEI_310, "wheel_color", Input::Effect(&gradient(5000, &[(0, 0x112233), (25, 0x445566), (50, 0x778899), (75, 0xAABBCC)])), "03 00 5B 00 01 88 13 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 05 11 22 \
         33 11 22 33 00 44 55 66 3F 77 88 99 40 AA BB CC 40 11 22 33 40");
}

/// rivalcfg test/devices/old_specs/test_sensei310.py::TestDevice::test_set_wheel_color_with_color_string
#[test]
fn sensei310_set_wheel_color_with_color_string() {
    check(&SENSEI_310, "wheel_color", Input::Color(rgb(0xFF1800)), "03 00 5B 00 01 E8 03 00 00 00 00 00 00 00 00 00 00 00 00 00 00 01 00 00 00 00 00 00 00 01 FF 18 \
         00 FF 18 00 00");
}

/// Defaults of every `SENSEI_310` setting and its save/battery/firmware commands, as rivalcfg f16c521
/// encodes them (computed by running rivalcfg, not copied from its test files).
#[test]
fn sensei310_rivalcfg_defaults() {
    check(&SENSEI_310, "sensitivity1", Input::Int(800), "02 00 53 00 01 07 00 42");
    check(&SENSEI_310, "sensitivity2", Input::Int(1600), "02 00 53 00 02 0F 00 42");
    check(&SENSEI_310, "polling_rate", Input::Choice("1000"), "02 00 54 00 01");
    check(&SENSEI_310, "logo_color", Input::Effect(&gradient(1000, &[(0, 0xFF0000), (33, 0x00FF00), (66, 0x0000FF)])), "03 00 5B 00 00 E8 03 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 04 FF 00 \
         00 FF 00 00 00 00 FF 00 54 00 00 FF 54 FF 00 00 57");
    check(&SENSEI_310, "wheel_color", Input::Effect(&gradient(1000, &[(0, 0xFF0000), (33, 0x00FF00), (66, 0x0000FF)])), "03 00 5B 00 01 E8 03 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 04 FF 00 \
         00 FF 00 00 00 00 FF 00 54 00 00 FF 54 FF 00 00 57");
    check(&SENSEI_310, "buttons", Input::Buttons(&map(&[("button1", "button1"), ("button2", "button2"), ("button3", "button3"), ("button4", "button4"), ("button5", "button5"), ("button6", "PageDown"), ("button7", "PageUp"), ("button8", "dpi"), ("layout", "qwerty")])), "02 00 31 00 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 51 4E 00 \
         00 00 51 4B 00 00 00 30 00 00 00 00");
    check_save(&SENSEI_310, "02 00 59 00");
    check_firmware(&SENSEI_310, "02 00 90 00");
}

// ---- SENSEI_RAW ------------------------------------------------------------------------------

/// rivalcfg test/devices/old_specs/test_sensei_raw.py::TestDevice::test_save
#[test]
fn sensei_raw_save() {
    check_save(&SENSEI_RAW, "02 00 09 00");
}

/// rivalcfg test/devices/old_specs/test_sensei_raw.py::TestDevice::test_set_buttons_mapping
#[test]
fn sensei_raw_set_buttons_mapping() {
    check(&SENSEI_RAW, "buttons", Input::Buttons(&map(&[])), "02 00 31 00 01 00 00 02 00 00 03 00 00 04 00 00 05 00 00 10 4B 00 10 4E 00 30 00 00");
    check(&SENSEI_RAW, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 31 00 01 00 00 06 00 00 03 00 00 04 00 00 05 00 00 10 4B 00 10 4E 00 30 00 00");
    check(&SENSEI_RAW, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 31 00 01 00 00 06 00 00 03 00 00 04 00 00 05 00 00 10 4B 00 10 4E 00 30 00 00");
}

/// rivalcfg test/devices/old_specs/test_sensei_raw.py::TestDevice::test_set_led_brightness
#[test]
fn sensei_raw_set_led_brightness() {
    check(&SENSEI_RAW, "led_brightness", Input::Choice("off"), "02 00 05 01 01");
    check(&SENSEI_RAW, "led_brightness", Input::Choice("low"), "02 00 05 01 02");
    check(&SENSEI_RAW, "led_brightness", Input::Choice("medium"), "02 00 05 01 03");
    check(&SENSEI_RAW, "led_brightness", Input::Choice("high"), "02 00 05 01 04");
}

/// rivalcfg test/devices/old_specs/test_sensei_raw.py::TestDevice::test_set_light_effect
#[test]
fn sensei_raw_set_light_effect() {
    check(&SENSEI_RAW, "light_effect", Input::Choice("steady"), "02 00 07 01 01");
    check(&SENSEI_RAW, "light_effect", Input::Choice("breath"), "02 00 07 01 03");
    check(&SENSEI_RAW, "light_effect", Input::Choice("1"), "02 00 07 01 01");
    check(&SENSEI_RAW, "light_effect", Input::Choice("2"), "02 00 07 01 02");
    check(&SENSEI_RAW, "light_effect", Input::Choice("3"), "02 00 07 01 03");
    check(&SENSEI_RAW, "light_effect", Input::Choice("4"), "02 00 07 01 04");
    check(&SENSEI_RAW, "light_effect", Input::Choice("trigger"), "02 00 07 01 05");
}

/// rivalcfg test/devices/old_specs/test_sensei_raw.py::TestDevice::test_set_polling_rate
#[test]
fn sensei_raw_set_polling_rate() {
    check(&SENSEI_RAW, "polling_rate", Input::Choice("125"), "02 00 04 00 04");
    check(&SENSEI_RAW, "polling_rate", Input::Choice("250"), "02 00 04 00 03");
    check(&SENSEI_RAW, "polling_rate", Input::Choice("500"), "02 00 04 00 02");
    check(&SENSEI_RAW, "polling_rate", Input::Choice("1000"), "02 00 04 00 01");
}

/// rivalcfg test/devices/old_specs/test_sensei_raw.py::TestDevice::test_set_sensitivity1
#[test]
fn sensei_raw_set_sensitivity1() {
    check(&SENSEI_RAW, "sensitivity1", Input::Int(90), "02 00 03 01 01");
    check(&SENSEI_RAW, "sensitivity1", Input::Int(180), "02 00 03 01 02");
    check(&SENSEI_RAW, "sensitivity1", Input::Int(1530), "02 00 03 01 11");
    check(&SENSEI_RAW, "sensitivity1", Input::Int(2520), "02 00 03 01 1C");
    check(&SENSEI_RAW, "sensitivity1", Input::Int(5400), "02 00 03 01 3C");
    check(&SENSEI_RAW, "sensitivity1", Input::Int(5670), "02 00 03 01 3F");
}

/// rivalcfg test/devices/old_specs/test_sensei_raw.py::TestDevice::test_set_sensitivity2
#[test]
fn sensei_raw_set_sensitivity2() {
    check(&SENSEI_RAW, "sensitivity2", Input::Int(90), "02 00 03 02 01");
    check(&SENSEI_RAW, "sensitivity2", Input::Int(180), "02 00 03 02 02");
    check(&SENSEI_RAW, "sensitivity2", Input::Int(1530), "02 00 03 02 11");
    check(&SENSEI_RAW, "sensitivity2", Input::Int(2520), "02 00 03 02 1C");
    check(&SENSEI_RAW, "sensitivity2", Input::Int(5400), "02 00 03 02 3C");
    check(&SENSEI_RAW, "sensitivity2", Input::Int(5670), "02 00 03 02 3F");
}

/// Defaults of every `SENSEI_RAW` setting and its save/battery/firmware commands, as rivalcfg f16c521
/// encodes them (computed by running rivalcfg, not copied from its test files).
#[test]
fn sensei_raw_rivalcfg_defaults() {
    check(&SENSEI_RAW, "sensitivity1", Input::Int(1620), "02 00 03 01 12");
    check(&SENSEI_RAW, "sensitivity2", Input::Int(3240), "02 00 03 02 24");
    check(&SENSEI_RAW, "polling_rate", Input::Choice("1000"), "02 00 04 00 01");
    check(&SENSEI_RAW, "led_brightness", Input::Choice("off"), "02 00 05 01 01");
    check(&SENSEI_RAW, "light_effect", Input::Choice("breath"), "02 00 07 01 03");
    check(&SENSEI_RAW, "buttons", Input::Buttons(&map(&[("button1", "button1"), ("button2", "button2"), ("button3", "button3"), ("button4", "button4"), ("button5", "button5"), ("button6", "PageDown"), ("button7", "PageUp"), ("button8", "dpi"), ("layout", "qwerty")])), "02 00 31 00 01 00 00 02 00 00 03 00 00 04 00 00 05 00 00 10 4B 00 10 4E 00 30 00 00");
    check_save(&SENSEI_RAW, "02 00 09 00");
}

// ---- SENSEI_TEN ------------------------------------------------------------------------------

/// rivalcfg test/devices/old_specs/test_sensei_ten.py::TestDevice::test_firmware_version
#[test]
fn sensei_ten_firmware_version() {
    check_firmware(&SENSEI_TEN, "02 00 90 00");
}

/// rivalcfg test/devices/old_specs/test_sensei_ten.py::TestDevice::test_save
#[test]
fn sensei_ten_save() {
    check_save(&SENSEI_TEN, "02 00 59 00");
}

/// rivalcfg test/devices/old_specs/test_sensei_ten.py::TestDevice::test_set_buttons_mapping
#[test]
fn sensei_ten_set_buttons_mapping() {
    check(&SENSEI_TEN, "buttons", Input::Buttons(&map(&[])), "02 00 31 00 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 51 4E 00 \
         00 00 51 4B 00 00 00 30 00 00 00 00");
    check(&SENSEI_TEN, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 31 00 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 51 4E 00 \
         00 00 51 4B 00 00 00 30 00 00 00 00");
    check(&SENSEI_TEN, "buttons", Input::Buttons(&map(&[("button2", "button6")])), "02 00 31 00 01 00 00 00 00 06 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 51 4E 00 \
         00 00 51 4B 00 00 00 30 00 00 00 00");
}

/// rivalcfg test/devices/old_specs/test_sensei_ten.py::TestDevice::test_set_logo_color
#[test]
fn sensei_ten_set_logo_color() {
    check(&SENSEI_TEN, "logo_color", Input::Effect(&gradient(1000, &[(0, 0xFF0000), (33, 0x00FF00), (66, 0x0000FF)])), "03 00 5B 00 00 E8 03 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 04 FF 00 \
         00 FF 00 00 00 00 FF 00 54 00 00 FF 54 FF 00 00 57");
}

/// rivalcfg test/devices/old_specs/test_sensei_ten.py::TestDevice::test_set_polling_rate
#[test]
fn sensei_ten_set_polling_rate() {
    check(&SENSEI_TEN, "polling_rate", Input::Choice("125"), "02 00 54 00 04");
    check(&SENSEI_TEN, "polling_rate", Input::Choice("250"), "02 00 54 00 03");
    check(&SENSEI_TEN, "polling_rate", Input::Choice("500"), "02 00 54 00 02");
    check(&SENSEI_TEN, "polling_rate", Input::Choice("1000"), "02 00 54 00 01");
}

/// rivalcfg test/devices/old_specs/test_sensei_ten.py::TestDevice::test_set_sensitivity
#[test]
fn sensei_ten_set_sensitivity() {
    check(&SENSEI_TEN, "sensitivity", Input::Dpi(&[(200, 200)]), "02 00 55 00 01 01 04 00");
    check(&SENSEI_TEN, "sensitivity", Input::Dpi(&[(200, 200)]), "02 00 55 00 01 01 04 00");
    check(&SENSEI_TEN, "sensitivity", Input::Dpi(&[(200, 200), (400, 400)]), "02 00 55 00 03 01 04 00 08 00");
    check(&SENSEI_TEN, "sensitivity", Input::Dpi(&[(200, 200), (400, 400), (800, 800), (18000, 18000)]), "02 00 55 00 0F 01 04 00 08 00 10 00 68 01");
}

/// rivalcfg test/devices/old_specs/test_sensei_ten.py::TestDevice::test_set_wheel_color
#[test]
fn sensei_ten_set_wheel_color() {
    check(&SENSEI_TEN, "wheel_color", Input::Effect(&gradient(5000, &[(0, 0x112233), (25, 0x445566), (50, 0x778899), (75, 0xAABBCC)])), "03 00 5B 00 01 88 13 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 05 11 22 \
         33 11 22 33 00 44 55 66 3F 77 88 99 40 AA BB CC 40 11 22 33 40");
}

/// Defaults of every `SENSEI_TEN` setting and its save/battery/firmware commands, as rivalcfg f16c521
/// encodes them (computed by running rivalcfg, not copied from its test files).
#[test]
fn sensei_ten_rivalcfg_defaults() {
    check(&SENSEI_TEN, "sensitivity", Input::Dpi(&[(400, 400), (800, 800), (1200, 1200), (2400, 2400), (3200, 3200)]), "02 00 55 00 1F 01 08 00 10 00 18 00 30 00 40 00");
    check(&SENSEI_TEN, "polling_rate", Input::Choice("1000"), "02 00 54 00 01");
    check(&SENSEI_TEN, "logo_color", Input::Effect(&gradient(10000, &[(0, 0xFF0000), (33, 0x00FF00), (66, 0x0000FF)])), "03 00 5B 00 00 10 27 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 04 FF 00 \
         00 FF 00 00 00 00 FF 00 54 00 00 FF 54 FF 00 00 57");
    check(&SENSEI_TEN, "wheel_color", Input::Effect(&gradient(10000, &[(0, 0xFF0000), (33, 0x00FF00), (66, 0x0000FF)])), "03 00 5B 00 01 10 27 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 04 FF 00 \
         00 FF 00 00 00 00 FF 00 54 00 00 FF 54 FF 00 00 57");
    check(&SENSEI_TEN, "buttons", Input::Buttons(&map(&[("button1", "button1"), ("button2", "button2"), ("button3", "button3"), ("button4", "button4"), ("button5", "button5"), ("button6", "PageDown"), ("button7", "PageUp"), ("button8", "dpi"), ("layout", "qwerty")])), "02 00 31 00 01 00 00 00 00 02 00 00 00 00 03 00 00 00 00 04 00 00 00 00 05 00 00 00 00 51 4E 00 \
         00 00 51 4B 00 00 00 30 00 00 00 00");
    check_save(&SENSEI_TEN, "02 00 59 00");
    check_firmware(&SENSEI_TEN, "02 00 90 00");
}
