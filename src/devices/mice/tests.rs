//! Tests of the model table, the descriptors and the device glue, using a recording transport
//! in place of a HID handle.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;

use super::device::{SteelSeriesMouse, Transport};
use super::encode::{self, ReportKind};
use super::profile::{self, BatteryFormat, Role, Value};
use super::*;
use crate::devices::settings::{SettingKind, SettingValue, Verification};
use crate::devices::{DeviceType, device_name_from_product_id, device_type_from_product_id};
use crate::{Error, STEELSERIES_VENDOR_ID};

#[derive(Default)]
struct Log {
    writes: Vec<(ReportKind, Vec<u8>)>,
    reads: usize,
    responses: VecDeque<Vec<u8>>,
}

struct FakeTransport(Arc<Mutex<Log>>);

impl Transport for FakeTransport {
    fn write_output(&mut self, wire: &[u8]) -> Result<()> {
        self.0.lock().writes.push((ReportKind::Output, wire.to_vec()));
        Ok(())
    }

    fn write_feature(&mut self, wire: &[u8]) -> Result<()> {
        self.0.lock().writes.push((ReportKind::Feature, wire.to_vec()));
        Ok(())
    }

    fn read(&mut self, buf: &mut [u8], _timeout_ms: i32) -> Result<usize> {
        let mut log = self.0.lock();
        log.reads += 1;
        let Some(response) = log.responses.pop_front() else {
            return Ok(0);
        };
        let len = response.len().min(buf.len());
        buf[..len].copy_from_slice(&response[..len]);
        Ok(len)
    }
}

fn info(model: &MouseModel) -> DeviceInfo {
    DeviceInfo {
        name: model.name.into(),
        device_type: DeviceType::Mouse,
        vendor_id: STEELSERIES_VENDOR_ID,
        product_id: model.product_id,
        interface_number: model.interface_number,
        usage_page: 0xFFC0,
        usage: 0x0001,
        serial_number: None,
        manufacturer: Some("SteelSeries".to_string()),
        path: "/dev/hidraw-test".to_string(),
    }
}

fn mouse(product_id: u16) -> (SteelSeriesMouse, Arc<Mutex<Log>>) {
    let model = model_for_product_id(product_id).unwrap();
    mouse_for(model)
}

fn mouse_for(model: &'static MouseModel) -> (SteelSeriesMouse, Arc<Mutex<Log>>) {
    let log = Arc::new(Mutex::new(Log::default()));
    let device = SteelSeriesMouse::new(info(model), model, Box::new(FakeTransport(log.clone())))
        .with_command_spacing(Duration::ZERO);
    (device, log)
}

fn writes(log: &Arc<Mutex<Log>>) -> Vec<(ReportKind, Vec<u8>)> {
    std::mem::take(&mut log.lock().writes)
}

fn output(bytes: &[u8]) -> (ReportKind, Vec<u8>) {
    (ReportKind::Output, bytes.to_vec())
}

// ---- model table ---------------------------------------------------------------------------

#[test]
fn every_model_has_a_unique_product_id() {
    let mut seen = BTreeSet::new();
    for model in MODELS {
        assert!(seen.insert(model.product_id), "duplicate PID {:#06x}", model.product_id);
        assert!(!model.name.is_empty());
        assert!(!model.name.starts_with("SteelSeries"), "{}", model.name);
        assert!((0..=3).contains(&model.interface_number), "{}", model.name);
    }
    assert_eq!(MODELS.len(), 76, "rivalcfg f16c521 lists 76 mouse product IDs");
}

#[test]
fn every_model_is_classified_as_a_mouse() {
    for model in MODELS {
        assert_eq!(
            device_type_from_product_id(model.product_id),
            DeviceType::Mouse,
            "{} ({:#06x}) collides with another device family",
            model.name,
            model.product_id
        );
        assert_eq!(device_name_from_product_id(model.product_id), model.name);
    }
}

#[test]
fn unknown_product_ids_are_rejected() {
    assert!(model_for_product_id(0xFFFF).is_none());
    let mut unknown = info(&MODELS[0]);
    unknown.product_id = 0xFFFF;
    assert!(matches!(
        model_for(&unknown),
        Err(Error::UnsupportedDevice { product_id: 0xFFFF, .. })
    ));
    for model in MODELS {
        assert_eq!(model_for(&info(model)).unwrap().product_id, model.product_id);
    }
}

#[test]
fn dual_mode_mice_reuse_their_wired_settings() {
    for model in MODELS.iter().filter(|m| m.profile.wireless) {
        assert!(model.name.ends_with("(2.4 GHz)"), "{}", model.name);
        assert!(
            model.profile.source.ends_with("_wireless_wireless.py"),
            "{}",
            model.name
        );
    }
}

// ---- profile tables ------------------------------------------------------------------------

fn all_profiles() -> Vec<&'static Profile> {
    let mut profiles: Vec<&'static Profile> = Vec::new();
    for model in MODELS {
        if !profiles.iter().any(|p| std::ptr::eq(*p, model.profile)) {
            profiles.push(model.profile);
        }
    }
    profiles
}

#[test]
fn every_rivalcfg_profile_is_used() {
    assert_eq!(all_profiles().len(), 33, "rivalcfg f16c521 has 33 mouse device files");
}

#[test]
fn profile_tables_are_consistent() {
    for profile in all_profiles() {
        let mut zones = BTreeSet::new();
        let mut ids = BTreeSet::new();
        for setting in profile.settings {
            assert!(ids.insert(setting.id), "{}: {} twice", profile.name, setting.id);
            if let Role::Zone(name) = setting.role {
                assert!(zones.insert(name), "{}: zone {name} twice", profile.name);
            }
            match setting.value {
                Value::Choice { entries, default } => {
                    assert!(
                        entries.iter().any(|e| e.id == default),
                        "{} {}",
                        profile.name,
                        setting.id
                    );
                }
                Value::Range { input, output, .. } => {
                    assert_eq!(input.len(), output.len(), "{} {}", profile.name, setting.id);
                }
                Value::RangeChoice { input, table, .. } => {
                    encode::check_table(input, table).unwrap();
                }
                Value::MultiDpi { spec, default } => {
                    if let encode::DpiEncoding::Table { input, table } = spec.encoding {
                        encode::check_table(input, table).unwrap();
                    }
                    assert!(!default.is_empty() && default.len() <= usize::from(spec.max_stages));
                }
                Value::Buttons(layout) => {
                    encode::encode_buttons(layout, &BTreeMap::new()).unwrap();
                }
                _ => {}
            }
        }
    }
}

// ---- descriptors ---------------------------------------------------------------------------

#[test]
fn every_model_opens_with_settings() {
    for model in MODELS {
        let (device, _) = mouse_for(model);
        let descriptors = device.setting_descriptors();
        assert!(!descriptors.is_empty(), "{}", model.name);
        let mut ids = BTreeSet::new();
        for d in &descriptors {
            assert!(ids.insert(d.id.clone()), "{}: descriptor {} twice", model.name, d.id);
            assert_eq!(d.verification, Verification::Reference, "{} {}", model.name, d.id);
            assert_eq!(
                d.persists_on_device,
                model.profile.save.is_some(),
                "{} {}",
                model.name,
                d.id
            );
            if let Some(default) = &d.default {
                d.validate(default)
                    .unwrap_or_else(|e| panic!("{} {}: default rejected: {e}", model.name, d.id));
            }
        }
        assert!(ids.contains("polling_rate"), "{}", model.name);
        assert!(
            ids.contains(profile::DPI_ID) || ids.contains("sensitivity1"),
            "{} has no DPI setting",
            model.name
        );
        assert_eq!(
            ids.contains(profile::SAVE_ID),
            model.profile.save.is_some(),
            "{}",
            model.name
        );
    }
}

/// Applying every descriptor's default (and triggering every action) must encode and write.
#[test]
fn every_default_applies_on_every_model() {
    for model in MODELS {
        let (mut device, log) = mouse_for(model);
        for d in device.setting_descriptors() {
            let value = match (&d.kind, &d.default) {
                (_, Some(default)) => default.clone(),
                (SettingKind::Action, None) => SettingValue::Trigger,
                // Steady colors of gradient zones: rivalcfg's defaults are gradients.
                (SettingKind::Color, None) => SettingValue::Color(Color::new(0x12, 0x34, 0x56)),
                (SettingKind::ColorZones { .. }, None) => SettingValue::Colors(vec![Color::new(0x12, 0x34, 0x56)]),
                (kind, None) => panic!("{} {}: no default for {kind:?}", model.name, d.id),
            };
            device
                .apply_setting(&d.id, &value)
                .unwrap_or_else(|e| panic!("{} {} = {value:?}: {e}", model.name, d.id));
        }
        assert!(!writes(&log).is_empty(), "{}", model.name);
    }
}

#[test]
fn invalid_values_are_rejected_before_anything_is_sent() {
    let (mut device, log) = mouse(0x1824);
    assert!(device.apply_setting("dpi", &SettingValue::Dpi(vec![150])).is_err());
    assert!(
        device
            .apply_setting("polling_rate", &SettingValue::Choice("2000".into()))
            .is_err()
    );
    assert!(device.apply_setting("nope", &SettingValue::Trigger).is_err());
    assert!(writes(&log).is_empty());
}

// ---- end to end ----------------------------------------------------------------------------

/// Rival 3: values from rivalcfg test/devices/old_specs/test_rival3.py, sent through
/// `apply_setting`.
#[test]
fn rival3_applies_settings_through_the_transport() {
    let (mut device, log) = mouse(0x1824);
    device.apply_setting("dpi", &SettingValue::Dpi(vec![200, 400])).unwrap();
    device
        .apply_setting("polling_rate", &SettingValue::Choice("125".into()))
        .unwrap();
    device
        .apply_setting("z1_color", &SettingValue::Color(Color::from_hex(0xABCDEF)))
        .unwrap();
    device.apply_setting("save", &SettingValue::Trigger).unwrap();
    assert_eq!(
        writes(&log),
        vec![
            output(&[0x00, 0x0B, 0x00, 0x02, 0x01, 0x04, 0x08]),
            output(&[0x00, 0x04, 0x00, 0x04]),
            output(&[0x00, 0x05, 0x00, 0x01, 0xAB, 0xCD, 0xEF, 0x64]),
            output(&[0x00, 0x09, 0x00]),
        ]
    );
    assert_eq!(log.lock().reads, 0, "wired mice send no read-back");
}

#[test]
fn colors_setting_writes_one_report_per_zone() {
    let (mut device, log) = mouse(0x1824);
    device
        .apply_setting("colors", &SettingValue::Colors(vec![Color::from_hex(0xFF0000)]))
        .unwrap();
    let sent = writes(&log);
    assert_eq!(sent.len(), 4);
    for (zone, (_, bytes)) in sent.iter().enumerate() {
        assert_eq!(bytes, &vec![0x00, 0x05, 0x00, zone as u8 + 1, 0xFF, 0x00, 0x00, 0x64]);
    }
}

#[test]
fn dpi_presets_drive_sensitivity1_and_sensitivity2() {
    // Rival 300: rivalcfg test/devices/old_specs/test_rival300.py sensitivity1/2 commands.
    let (mut device, log) = mouse(0x1710);
    let dpi = device
        .setting_descriptors()
        .into_iter()
        .find(|d| d.id == "dpi")
        .unwrap();
    assert!(matches!(dpi.kind, SettingKind::DpiStages { max_stages: 2, .. }));
    device.apply_setting("dpi", &SettingValue::Dpi(vec![800])).unwrap();
    assert_eq!(writes(&log).len(), 1);
    device
        .apply_setting("dpi", &SettingValue::Dpi(vec![800, 1600]))
        .unwrap();
    let sent = writes(&log);
    assert_eq!(sent[0].1[..3], [0x00, 0x03, 0x01]);
    assert_eq!(sent[1].1[..3], [0x00, 0x03, 0x02]);
}

#[test]
fn wireless_commands_carry_the_flag_and_drain_the_reply() {
    // Aerox 3 Wireless, 2.4 GHz: rivalcfg test/devices/specs/aerox3_wireless_wireless.txt.
    let (mut device, log) = mouse(0x1838);
    device
        .apply_setting("polling_rate", &SettingValue::Choice("500".into()))
        .unwrap();
    device.apply_setting("save", &SettingValue::Trigger).unwrap();
    assert_eq!(
        writes(&log),
        vec![output(&[0x00, 0x6B, 0x01]), output(&[0x00, 0x51, 0x00])]
    );
    assert_eq!(log.lock().reads, 2);
}

#[test]
fn gradient_zones_use_feature_reports() {
    // Rival 310: rivalcfg test/devices/old_specs/test_rival310.py::test_set_wheel_color_with_color_string.
    let (mut device, log) = mouse(0x1720);
    device
        .apply_setting("wheel_color", &SettingValue::Color(Color::from_hex(0xFF1800)))
        .unwrap();
    let sent = writes(&log);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, ReportKind::Feature);
    assert_eq!(sent[0].1[..6], [0x00, 0x5B, 0x00, 0x01, 0xE8, 0x03]);
}

#[test]
fn gradient_duration_resends_stored_gradients() {
    let (mut device, log) = mouse(0x1720);
    let colors = SettingValue::Colors(vec![
        Color::from_hex(0xFF0000),
        Color::from_hex(0x00FF00),
        Color::from_hex(0x0000FF),
    ]);
    device.apply_setting("logo_gradient", &colors).unwrap();
    // rivalcfg test/devices/old_specs/test_rival310.py::test_set_logo_color: 1000 ms, 4 stops.
    let first = writes(&log);
    assert_eq!(first[0].1[..6], [0x00, 0x5B, 0x00, 0x00, 0xE8, 0x03]);
    assert_eq!(first[0].1[3 + 25], 4);
    assert!(
        device
            .apply_setting("gradient_duration", &SettingValue::Int(5000))
            .is_ok()
    );
    let resent = writes(&log);
    assert_eq!(resent.len(), 1);
    assert_eq!(resent[0].1[4..6], [0x88, 0x13]);
    // A steady color replaces the gradient, so it is no longer re-sent.
    device
        .apply_setting("logo_color", &SettingValue::Color(Color::from_hex(0x112233)))
        .unwrap();
    writes(&log);
    device
        .apply_setting("gradient_duration", &SettingValue::Int(1000))
        .unwrap();
    assert!(writes(&log).is_empty());
}

#[test]
fn reactive_black_means_off() {
    // rivalcfg test/devices/specs/aerox3_wireless_wired.txt: --reactive-color disable / ABCDEF.
    let (mut device, log) = mouse(0x183A);
    device
        .apply_setting("reactive_color", &SettingValue::Color(Color::new(0, 0, 0)))
        .unwrap();
    device
        .apply_setting("reactive_color", &SettingValue::Color(Color::from_hex(0xABCDEF)))
        .unwrap();
    assert_eq!(
        writes(&log),
        vec![
            output(&[0x00, 0x26, 0x00, 0x00, 0x00, 0x00, 0x00]),
            output(&[0x00, 0x26, 0x01, 0x00, 0xAB, 0xCD, 0xEF]),
        ]
    );
}

#[test]
fn buttons_accept_partial_maps() {
    // rivalcfg test/devices/old_specs/test_rival3.py::test_set_buttons_mapping.
    let (mut device, log) = mouse(0x1824);
    let map: BTreeMap<String, String> = [("button2".to_string(), "button6".to_string())].into();
    device.apply_setting("buttons", &SettingValue::Buttons(map)).unwrap();
    assert_eq!(
        writes(&log),
        vec![output(&[
            0x00, 0x07, 0x00, 0x01, 0x00, 0x06, 0x00, 0x03, 0x00, 0x04, 0x00, 0x05, 0x00, 0x30, 0x00, 0x31, 0x00, 0x32,
            0x00,
        ])]
    );
}

// ---- direct colors -------------------------------------------------------------------------

#[test]
fn direct_colors_cover_every_zone() {
    let (mut device, log) = mouse(0x183C);
    let zones = device.color_zone_names();
    assert_eq!(zones.len(), 10);
    assert_eq!(zones[0], "wheel");
    assert_eq!(zones[9], "logo");
    let colors: Vec<Color> = (0..10).map(|i| Color::new(i, 0, 0)).collect();
    device.set_zone_colors_direct(&colors).unwrap();
    let sent = writes(&log);
    assert_eq!(sent.len(), 10);
    // rivalcfg rival5.py: z9 is 0x21, flags 0x0000/0x0100, eight zones of padding, then RGB.
    assert_eq!(sent[8].1[..4], [0x00, 0x21, 0x00, 0x01]);
    assert_eq!(sent[8].1[4 + 8 * 3..], [8, 0, 0]);
    assert!(device.set_zone_colors_direct(&colors[..3]).is_err());
}

#[test]
fn models_without_leds_have_no_direct_colors() {
    let (mut device, _) = mouse(0x1366); // Kinzu v2
    assert!(device.color_zone_names().is_empty());
    assert!(matches!(
        device.set_zone_colors_direct(&[Color::new(1, 2, 3)]),
        Err(Error::Unsupported(_))
    ));
}

// ---- status --------------------------------------------------------------------------------

#[test]
fn battery_responses_parse_like_rivalcfg() {
    // aerox3_wireless_wired.py: level = ((data[1] & ~0x80) - 1) * 5, charging = data[1] & 0x80.
    assert_eq!(
        profile::parse_battery(BatteryFormat::FlaggedSteps, &[0x92, 0x80 | 15]),
        (Some(70), Some(true))
    );
    assert_eq!(
        profile::parse_battery(BatteryFormat::FlaggedSteps, &[0x92, 21]),
        (Some(100), Some(false))
    );
    assert_eq!(
        profile::parse_battery(BatteryFormat::FlaggedSteps, &[0x92, 0]),
        (None, None)
    );
    assert_eq!(
        profile::parse_battery(BatteryFormat::FlaggedSteps, &[0x92, 22]),
        (None, None)
    );
    // rival650.py: level = data[0], charging = data[2].
    assert_eq!(
        profile::parse_battery(BatteryFormat::LevelThenCharging, &[42, 0, 1]),
        (Some(42), Some(true))
    );
    assert_eq!(
        profile::parse_battery(BatteryFormat::LevelThenCharging, &[101, 0, 0]),
        (None, None)
    );
    assert_eq!(
        profile::parse_battery(BatteryFormat::LevelThenCharging, &[]),
        (None, None)
    );
}

#[test]
fn read_status_reports_battery_and_firmware() {
    let (mut device, log) = mouse(0x1838);
    log.lock().responses.push_back(vec![0xD2, 0x80 | 11]);
    let status = device.read_status().unwrap();
    assert_eq!(status.battery_percent, Some(50));
    assert_eq!(status.charging, Some(true));
    assert_eq!(writes(&log), vec![output(&[0x00, 0xD2])]);

    let (mut device, log) = mouse(0x1720); // Rival 310: firmware 0x90 0x00
    log.lock().responses.push_back(vec![1, 33]);
    let status = device.read_status().unwrap();
    assert_eq!(status.extra.get("firmware").map(String::as_str), Some("1.33"));
    assert_eq!(status.battery_percent, None);
    assert_eq!(writes(&log), vec![output(&[0x00, 0x90, 0x00])]);

    let (mut device, _) = mouse(0x1366); // Kinzu v2 reports nothing
    assert!(device.read_status().unwrap().is_empty());
}

#[test]
fn closed_devices_refuse_io() {
    let (mut device, _) = mouse(0x1824);
    assert!(device.is_connected());
    device.close().unwrap();
    assert!(!device.is_connected());
    assert!(device.apply_setting("save", &SettingValue::Trigger).is_err());
}

#[test]
fn open_uses_the_model_table() {
    let model = model_for_product_id(0x1824).unwrap();
    let (device, _) = mouse_for(model);
    assert_eq!(device.model().name, "Rival 3");
    assert_eq!(device.device_type(), DeviceType::Mouse);
    let boxed = open_with_transport(info(model), model, Box::new(FakeTransport(Arc::default())));
    assert_eq!(boxed.model().product_id, 0x1824);
}
