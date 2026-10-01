//! Model-level tests: the PID table, every model's descriptors, golden encodings per model and
//! `read_status` against a scripted fake device.

use std::collections::{HashSet, VecDeque};
use std::sync::Arc;

use super::*;
use crate::devices::product_ids::*;
use crate::devices::settings::{SettingKind, Verification};
use crate::devices::{device_name_from_product_id, device_type_from_product_id};
use crate::rgb::Color;

// ---- Fake transport ---------------------------------------------------------------------------

#[derive(Default)]
struct FakeState {
    writes: Vec<(ReportKind, Vec<u8>)>,
    /// Reports waiting to be read.
    input: VecDeque<Vec<u8>>,
    /// One batch of replies queued per write, in order.
    replies: VecDeque<Vec<Vec<u8>>>,
}

#[derive(Clone, Default)]
struct Fake(Arc<Mutex<FakeState>>);

impl Fake {
    fn with_stale(self, report: &[u8]) -> Self {
        self.0.lock().input.push_back(report.to_vec());
        self
    }

    fn reply(self, batch: &[&[u8]]) -> Self {
        self.0
            .lock()
            .replies
            .push_back(batch.iter().map(|r| r.to_vec()).collect());
        self
    }

    fn writes(&self) -> Vec<(ReportKind, Vec<u8>)> {
        self.0.lock().writes.clone()
    }

    fn record(&self, kind: ReportKind, data: &[u8]) {
        let mut state = self.0.lock();
        state.writes.push((kind, data.to_vec()));
        if let Some(batch) = state.replies.pop_front() {
            state.input.extend(batch);
        }
    }
}

impl HidTransport for Fake {
    fn write_output(&self, data: &[u8]) -> Result<()> {
        self.record(ReportKind::Output, data);
        Ok(())
    }

    fn write_feature(&self, data: &[u8]) -> Result<()> {
        self.record(ReportKind::Feature, data);
        Ok(())
    }

    fn read_timeout(&self, buf: &mut [u8], _timeout_ms: i32) -> Result<usize> {
        let Some(report) = self.0.lock().input.pop_front() else {
            return Ok(0);
        };
        let n = report.len().min(buf.len());
        buf[..n].copy_from_slice(&report[..n]);
        Ok(n)
    }
}

fn headset(product_id: u16, fake: &Fake) -> SteelSeriesHeadset {
    let model = model_for_product_id(product_id).expect("model in table");
    let info = DeviceInfo {
        name: std::borrow::Cow::Borrowed(model.name),
        device_type: DeviceType::Headset,
        vendor_id: crate::STEELSERIES_VENDOR_ID,
        product_id,
        interface_number: model.interface_number,
        usage_page: model.usage_page.unwrap_or(0),
        usage: 1,
        serial_number: None,
        manufacturer: None,
        path: "fake".into(),
    };
    SteelSeriesHeadset::new(info, model, Box::new(fake.clone()))
}

fn padded(prefix: &[u8], len: usize) -> Vec<u8> {
    let mut v = prefix.to_vec();
    v.resize(len, 0);
    v
}

/// Apply `value` to `setting` on `product_id` and return what was written.
fn apply(product_id: u16, setting: &str, value: SettingValue) -> Vec<(ReportKind, Vec<u8>)> {
    let fake = Fake::default();
    let mut hs = headset(product_id, &fake);
    hs.apply_setting(setting, &value)
        .unwrap_or_else(|e| panic!("{setting} on {product_id:#06x}: {e}"));
    fake.writes()
}

fn output(prefix: &[u8], len: usize) -> (ReportKind, Vec<u8>) {
    (ReportKind::Output, padded(prefix, len))
}

fn feature(prefix: &[u8]) -> (ReportKind, Vec<u8>) {
    (ReportKind::Feature, padded(prefix, 64))
}

fn int(v: i64) -> SettingValue {
    SettingValue::Int(v)
}

fn choice(id: &str) -> SettingValue {
    SettingValue::Choice(id.to_owned())
}

// ---- The table -------------------------------------------------------------------------------

#[test]
fn pids_are_unique_and_sorted() {
    let mut seen = HashSet::new();
    for model in MODELS {
        assert!(seen.insert(model.product_id), "duplicate PID {:#06x}", model.product_id);
    }
    assert!(
        MODELS.windows(2).all(|w| w[0].product_id < w[1].product_id),
        "MODELS must stay ordered by PID"
    );
}

#[test]
fn every_model_is_a_named_headset() {
    for model in MODELS {
        assert_eq!(device_type_from_product_id(model.product_id), DeviceType::Headset);
        assert_eq!(device_name_from_product_id(model.product_id), model.name);
        assert!(!model.source.is_empty());
    }
}

#[test]
fn reference_pids_are_present() {
    // Every PID HeadsetControl's SteelSeries drivers list, plus the OpenRGB Arctis 5 PIDs.
    let reference: &[u16] = &[
        0x12b3, 0x12b6, 0x12d7, 0x12d5, 0x1260, 0x12ad, 0x1252, 0x1280, 0x220e, 0x2212, 0x2216, 0x2236, 0x12c2, 0x230a,
        0x12ec, 0x2269, 0x226d, 0x2232, 0x2253, 0x2202, 0x22a1, 0x227e, 0x2206, 0x2258, 0x229e, 0x22ad, 0x223a, 0x22a9,
        0x227a, 0x22a4, 0x22a5, 0x220a, 0x22a7, 0x2298, 0x12e0, 0x12e5, 0x1290, 0x1250, 0x12aa,
    ];
    for pid in reference {
        assert!(model_for_product_id(*pid).is_some(), "missing PID {pid:#06x}");
    }
}

#[test]
fn removed_pids_are_no_longer_headsets() {
    // Earlier tables carried these without a source (see docs/development/devices.md).
    for pid in [0x12cf, 0x12e4, 0x12ea, 0x12ee] {
        assert_eq!(device_type_from_product_id(pid), DeviceType::Unknown, "{pid:#06x}");
    }
}

#[test]
fn interfaces_match_the_reference() {
    let expect = [
        (ARCTIS_1_WIRELESS, 3, Some(0xff43)),
        (ARCTIS_7, 5, None),
        (ARCTIS_5, 5, None),
        (ARCTIS_9, 0, None),
        (ARCTIS_PRO_WIRELESS, 0, None),
        (ARCTIS_NOVA_PRO_WIRELESS, 4, None),
        (ARCTIS_NOVA_3, 4, Some(0xffc0)),
        (ARCTIS_7_PLUS, 3, Some(0xffc0)),
        (ARCTIS_NOVA_3P_WIRELESS, 3, Some(0xffc0)),
        (ARCTIS_NOVA_5, 3, Some(0xffc0)),
        (ARCTIS_NOVA_7, 3, Some(0xffc0)),
        (ARCTIS_GAMEBUDS, 3, Some(0xffc0)),
    ];
    for (pid, interface, page) in expect {
        let model = model_for_product_id(pid).unwrap();
        assert_eq!(model.interface_number, interface, "{}", model.name);
        assert_eq!(model.usage_page, page, "{}", model.name);
    }
}

// ---- Descriptors -----------------------------------------------------------------------------

fn ids(pid: u16) -> Vec<String> {
    model_for_product_id(pid)
        .unwrap()
        .descriptors()
        .into_iter()
        .map(|d| d.id)
        .collect()
}

#[test]
fn every_model_yields_a_consistent_descriptor_list() {
    for model in MODELS {
        let descriptors = model.descriptors();
        assert_eq!(descriptors.len(), model.capabilities.len(), "{}", model.name);
        let unique: HashSet<_> = descriptors.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(unique.len(), descriptors.len(), "{} repeats a setting id", model.name);
        for d in &descriptors {
            assert_ne!(d.verification, Verification::Hardware, "{}: {}", model.name, d.id);
            assert!(!d.description.is_empty());
        }
    }
}

#[test]
fn descriptor_lists_per_family() {
    assert_eq!(ids(ARCTIS_1_WIRELESS), ["sidetone", "inactive_time"]);
    assert_eq!(ids(ARCTIS_7), ["sidetone", "inactive_time", "lights"]);
    assert_eq!(ids(ARCTIS_9), ["sidetone", "inactive_time"]);
    assert_eq!(ids(ARCTIS_PRO_WIRELESS), ["sidetone", "inactive_time"]);
    assert_eq!(ids(ARCTIS_5), ["led_colors"]);
    assert_eq!(
        ids(ARCTIS_NOVA_PRO_WIRELESS),
        [
            "sidetone",
            "lights",
            "inactive_time",
            "equalizer_preset",
            "equalizer",
            "chatmix_dial",
            "sonar_icon",
            "save"
        ]
    );
    assert_eq!(
        ids(ARCTIS_7_PLUS),
        ["sidetone", "inactive_time", "equalizer_preset", "equalizer"]
    );
    assert_eq!(
        ids(ARCTIS_NOVA_3),
        [
            "sidetone",
            "mic_volume",
            "mic_mute_led_brightness",
            "equalizer_preset",
            "equalizer",
            "led_colors",
            "led_effect",
            "led_effect_speed",
            "save"
        ]
    );
    assert_eq!(
        ids(ARCTIS_NOVA_3P_WIRELESS),
        [
            "sidetone",
            "inactive_time",
            "mic_volume",
            "equalizer_preset",
            "equalizer"
        ]
    );
    assert_eq!(
        ids(ARCTIS_NOVA_5),
        [
            "sidetone",
            "inactive_time",
            "mic_volume",
            "mic_mute_led_brightness",
            "volume_limiter",
            "equalizer_preset",
            "equalizer",
            "save"
        ]
    );
    let nova7 = [
        "sidetone",
        "inactive_time",
        "equalizer_preset",
        "equalizer",
        "mic_mute_led_brightness",
        "mic_volume",
        "volume_limiter",
        "bluetooth_when_powered_on",
        "bluetooth_call_volume",
        "save",
    ];
    assert_eq!(ids(ARCTIS_NOVA_7), nova7);
    assert_eq!(ids(ARCTIS_NOVA_7_GEN2), nova7);
    assert_eq!(ids(ARCTIS_NOVA_7P), nova7[1..]);
    assert!(ids(ARCTIS_GAMEBUDS).is_empty());
    assert!(ids(ARCTIS_NOVA_PRO_OMNI).is_empty());
}

#[test]
fn ranges_use_native_device_steps() {
    let range = |pid: u16, id: &str| -> (i64, i64, i64) {
        let d = model_for_product_id(pid)
            .unwrap()
            .descriptors()
            .into_iter()
            .find(|d| d.id == id)
            .unwrap();
        match d.kind {
            SettingKind::Range { min, max, step, .. } => (min, max, step),
            other => panic!("{id} is {other:?}"),
        }
    };
    assert_eq!(range(ARCTIS_7, "sidetone"), (0, 18, 1));
    assert_eq!(range(ARCTIS_9, "sidetone"), (0, 61, 1));
    assert_eq!(range(ARCTIS_PRO_WIRELESS, "sidetone"), (0, 9, 1));
    assert_eq!(range(ARCTIS_PRO_WIRELESS, "inactive_time"), (0, 250, 10));
    assert_eq!(range(ARCTIS_NOVA_5, "sidetone"), (0, 10, 1));
    assert_eq!(range(ARCTIS_NOVA_5, "mic_volume"), (0, 15, 1));
    assert_eq!(range(ARCTIS_NOVA_3P_WIRELESS, "mic_volume"), (0, 14, 1));
    assert_eq!(range(ARCTIS_NOVA_7, "mic_volume"), (0, 7, 1));
    assert_eq!(range(ARCTIS_NOVA_3, "mic_volume"), (0, 10, 1));
    assert_eq!(range(ARCTIS_NOVA_3, "led_effect_speed"), (1, 10, 1));
}

#[test]
fn equalizer_descriptors_carry_band_layout() {
    let eq = |pid: u16| {
        let d = model_for_product_id(pid)
            .unwrap()
            .descriptors()
            .into_iter()
            .find(|d| d.id == "equalizer")
            .unwrap();
        match d.kind {
            SettingKind::Equalizer {
                bands_hz,
                min_db,
                max_db,
                step_db,
            } => (bands_hz.len(), min_db, max_db, step_db),
            other => panic!("{other:?}"),
        }
    };
    assert_eq!(eq(ARCTIS_7_PLUS), (10, -12.0, 12.0, 0.5));
    assert_eq!(eq(ARCTIS_NOVA_3), (6, -6.0, 6.0, 0.5));
    assert_eq!(eq(ARCTIS_NOVA_3P_WIRELESS), (10, -12.0, 12.0, 0.1));
    assert_eq!(eq(ARCTIS_NOVA_5), (10, -10.0, 10.0, 0.5));
    assert_eq!(eq(ARCTIS_NOVA_7), (10, -10.0, 10.0, 1.0));
    assert_eq!(eq(ARCTIS_NOVA_PRO_WIRELESS), (10, -10.0, 10.0, 0.5));
}

#[test]
fn persistence_follows_the_reference_save_commands() {
    let persists = |pid: u16, id: &str| {
        model_for_product_id(pid)
            .unwrap()
            .descriptors()
            .into_iter()
            .find(|d| d.id == id)
            .unwrap()
            .persists_on_device
    };
    assert!(persists(ARCTIS_7, "sidetone"));
    assert!(persists(ARCTIS_NOVA_5, "sidetone"));
    assert!(!persists(ARCTIS_NOVA_5, "inactive_time"));
    assert!(!persists(ARCTIS_NOVA_7, "sidetone"));
    assert!(persists(ARCTIS_NOVA_7_GEN2, "sidetone"));
    assert!(persists(ARCTIS_NOVA_7, "bluetooth_when_powered_on"));
    assert!(!persists(ARCTIS_NOVA_3, "equalizer"));
    assert!(persists(ARCTIS_NOVA_3, "equalizer_preset"));
    assert!(!persists(ARCTIS_NOVA_PRO_WIRELESS, "equalizer"));
    assert!(persists(ARCTIS_NOVA_3P_WIRELESS, "equalizer_preset"));
}

#[test]
fn nova7_save_is_a_guess_except_on_gen2() {
    let verification = |pid: u16| {
        model_for_product_id(pid)
            .unwrap()
            .descriptors()
            .into_iter()
            .find(|d| d.id == "save")
            .unwrap()
            .verification
    };
    assert_eq!(verification(ARCTIS_NOVA_7), Verification::Guess);
    assert_eq!(verification(ARCTIS_NOVA_7P), Verification::Guess);
    assert_eq!(verification(ARCTIS_NOVA_7_GEN2), Verification::Reference);
    assert_eq!(verification(ARCTIS_NOVA_5), Verification::Reference);
}

// ---- Golden encodings per model ----------------------------------------------------------------

#[test]
fn arctis_1_encodings() {
    let save = output(&[0x06, 0x09], 31);
    assert_eq!(
        apply(ARCTIS_1_WIRELESS, "sidetone", int(18)),
        [output(&[0x06, 0x35, 0x01, 0x00, 0x12], 31), save.clone()]
    );
    assert_eq!(
        apply(ARCTIS_7X, "sidetone", int(0)),
        [output(&[0x06, 0x35], 31), save.clone()]
    );
    assert_eq!(
        apply(ARCTIS_7P, "inactive_time", int(90)),
        [output(&[0x06, 0x53, 90], 31), save]
    );
}

#[test]
fn arctis_7_encodings() {
    let save = output(&[0x06, 0x09], 31);
    assert_eq!(
        apply(ARCTIS_7, "sidetone", int(9)),
        [output(&[0x06, 0x35, 0x01, 0x00, 0x09], 31), save.clone()]
    );
    assert_eq!(
        apply(ARCTIS_7_2019, "inactive_time", int(30)),
        [output(&[0x06, 0x51, 30], 31), save.clone()]
    );
    assert_eq!(
        apply(ARCTIS_PRO, "lights", SettingValue::Bool(true)),
        [output(&[0x06, 0x55, 0x01, 0x02], 31), save.clone()]
    );
    assert_eq!(
        apply(ARCTIS_PRO_GAMEDAC, "lights", SettingValue::Bool(false)),
        [output(&[0x06, 0x55, 0x01, 0x00], 31), save]
    );
}

#[test]
fn arctis_9_encodings() {
    let save = output(&[0x90, 0x00], 31);
    assert_eq!(
        apply(ARCTIS_9, "sidetone", int(61)),
        [output(&[0x06, 0x00, 0xfd], 31), save.clone()]
    );
    assert_eq!(
        apply(ARCTIS_9, "sidetone", int(0)),
        [output(&[0x06, 0x00, 0xc0], 31), save.clone()]
    );
    assert_eq!(
        apply(ARCTIS_9, "inactive_time", int(10)),
        [output(&[0x04, 0x00, 0x02, 0x58], 31), save]
    );
}

#[test]
fn arctis_pro_wireless_encodings() {
    let save = output(&[0x90, 0xaa], 31);
    assert_eq!(
        apply(ARCTIS_PRO_WIRELESS, "sidetone", int(9)),
        [output(&[0x39, 0xaa, 0x09], 31), save.clone()]
    );
    assert_eq!(
        apply(ARCTIS_PRO_WIRELESS, "inactive_time", int(30)),
        [output(&[0x3c, 0xaa, 0x03], 31), save]
    );
}

#[test]
fn nova_pro_wireless_encodings() {
    let pid = ARCTIS_NOVA_PRO_WIRELESS;
    let save = output(&[0x06, 0x09], 31);
    assert_eq!(
        apply(pid, "sidetone", int(3)),
        [output(&[0x06, 0x39, 3], 31), save.clone()]
    );
    assert_eq!(
        apply(pid, "lights", SettingValue::Bool(true)),
        [output(&[0x06, 0xbf, 0x0a], 31), save.clone()]
    );
    assert_eq!(
        apply(pid, "lights", SettingValue::Bool(false)),
        [output(&[0x06, 0xbf, 0x01], 31), save.clone()]
    );
    assert_eq!(
        apply(pid, "inactive_time", choice("60")),
        [output(&[0x06, 0xc1, 6], 31), save.clone()]
    );
    assert_eq!(
        apply(pid, "inactive_time", choice("0")),
        [output(&[0x06, 0xc1, 0], 31), save.clone()]
    );
    assert_eq!(
        apply(pid, "equalizer_preset", choice("preset_2")),
        [output(&[0x06, 0x2e, 1], 31), save.clone()]
    );
    let mut gains = vec![0.0; 10];
    gains[0] = 10.0;
    gains[9] = -10.0;
    gains[4] = 2.5;
    let mut bands = vec![0x06, 0x33, 0x28, 0x14, 0x14, 0x14, 0x19, 0x14, 0x14, 0x14, 0x14, 0x00];
    bands.resize(31, 0);
    assert_eq!(
        apply(pid, "equalizer", SettingValue::Gains(gains)),
        [
            (ReportKind::Output, vec![0x06, 0x2e, 0x04]),
            (ReportKind::Output, bands)
        ]
    );
    assert_eq!(
        apply(pid, "chatmix_dial", SettingValue::Bool(true)),
        [output(&[0x06, 0x49, 0x01], 63)]
    );
    assert_eq!(
        apply(ARCTIS_NOVA_PRO_WIRELESS_XBOX, "sonar_icon", SettingValue::Bool(false)),
        [output(&[0x06, 0x8d, 0x00], 63)]
    );
    assert_eq!(apply(pid, "save", SettingValue::Trigger), [save]);
}

#[test]
fn arctis_7_plus_encodings() {
    let save = output(&[0x00, 0x09], 64);
    assert_eq!(
        apply(ARCTIS_7_PLUS, "sidetone", int(2)),
        [output(&[0x00, 0x39, 2], 64), save.clone()]
    );
    assert_eq!(
        apply(ARCTIS_7_PLUS_PS5, "inactive_time", int(90)),
        [output(&[0x00, 0xa3, 90], 64), save.clone()]
    );
    assert_eq!(
        apply(ARCTIS_7_PLUS_XBOX, "equalizer_preset", choice("bass")),
        [
            output(
                &[0x00, 0x33, 0x1f, 0x20, 0x1a, 0x15, 0x15, 0x16, 0x16, 0x16, 0x16, 0x23],
                64
            ),
            save.clone()
        ]
    );
    let gains = vec![12.0, -12.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, -0.5];
    assert_eq!(
        apply(ARCTIS_7_PLUS_DESTINY, "equalizer", SettingValue::Gains(gains)),
        [
            output(
                &[0x00, 0x33, 0x30, 0x00, 0x18, 0x19, 0x18, 0x18, 0x18, 0x18, 0x18, 0x17],
                64
            ),
            save
        ]
    );
}

#[test]
fn nova_3_encodings() {
    let save = feature(&[0x06, 0x09]);
    assert_eq!(
        apply(ARCTIS_NOVA_3, "sidetone", int(3)),
        [feature(&[0x06, 0x39, 3]), save.clone()]
    );
    assert_eq!(
        apply(ARCTIS_NOVA_3, "mic_volume", int(10)),
        [feature(&[0x06, 0x37, 10]), save.clone()]
    );
    assert_eq!(
        apply(ARCTIS_NOVA_3, "mic_mute_led_brightness", int(2)),
        [feature(&[0x06, 0xae, 2]), save.clone()]
    );
    assert_eq!(
        apply(ARCTIS_NOVA_3, "equalizer_preset", choice("smiley")),
        [feature(&[0x06, 0x33, 0x1a, 0x17, 0x0f, 0x12, 0x17, 0x1a]), save.clone()]
    );
    assert_eq!(
        apply(
            ARCTIS_NOVA_3,
            "equalizer",
            SettingValue::Gains(vec![6.0, -6.0, 0.0, 1.5, 0.0, 0.0])
        ),
        [feature(&[0x06, 0x33, 0x20, 0x08, 0x14, 0x17, 0x14, 0x14])]
    );
    assert_eq!(apply(ARCTIS_NOVA_3, "save", SettingValue::Trigger), [save]);
}

#[test]
fn nova_3_lighting_goes_out_as_output_reports() {
    let writes = apply(
        ARCTIS_NOVA_3,
        "led_colors",
        SettingValue::Colors(vec![Color::RED, Color::BLUE]),
    );
    assert_eq!(writes.len(), 6);
    assert!(writes.iter().all(|(kind, _)| *kind == ReportKind::Output));
    // Right cup (zone 0) first, carrying the right color.
    assert_eq!(&writes[0].1[..3], &[0x06, 0xaa, 0x01]);
    assert_eq!(&writes[0].1[0x2d..0x30], &[0x00, 0x00, 0xff]);
    assert_eq!(&writes[3].1[0x2d..0x30], &[0xff, 0x00, 0x00]);

    let writes = apply(ARCTIS_NOVA_3, "led_effect", choice("off"));
    assert_eq!(writes.len(), 4);

    let fake = Fake::default();
    let mut hs = headset(ARCTIS_NOVA_3, &fake);
    hs.apply_setting("led_effect", &choice("breathe")).unwrap();
    hs.apply_setting("led_effect_speed", &int(1)).unwrap();
    let writes = fake.writes();
    let last_effect = &writes[writes.len() - 3].1;
    assert_eq!(last_effect[0x25], 0x03);
    assert_eq!(&last_effect[0x0d..0x0f], &[0x30, 0x75]);
}

#[test]
fn arctis_5_lighting() {
    let writes = apply(ARCTIS_5_2017, "led_colors", SettingValue::Colors(vec![Color::GREEN]));
    assert_eq!(writes.len(), 12);
    assert!(
        writes
            .iter()
            .all(|(kind, r)| *kind == ReportKind::Output && r.len() == 37)
    );
    assert_eq!(&writes[2].1[7..10], &[0x00, 0xff, 0x00]);
    assert_eq!(&writes[8].1[7..10], &[0x00, 0xff, 0x00]);
}

#[test]
fn nova_3p_encodings() {
    let pid = ARCTIS_NOVA_3P_WIRELESS;
    let save = output(&[0x00, 0x09], 64);
    assert_eq!(
        apply(pid, "sidetone", int(10)),
        [output(&[0x00, 0x39, 10], 64), save.clone()]
    );
    assert_eq!(
        apply(ARCTIS_NOVA_3X_WIRELESS, "inactive_time", choice("45")),
        [output(&[0x00, 0xa3, 45], 64), save.clone()]
    );
    assert_eq!(
        apply(pid, "mic_volume", int(14)),
        [output(&[0x00, 0x37, 14], 64), save.clone()]
    );

    let writes = apply(pid, "equalizer_preset", choice("bass"));
    assert_eq!(writes.len(), 2);
    let eq = &writes[0].1;
    assert_eq!(eq.len(), 64);
    assert_eq!(&eq[..2], &[0x00, 0x33]);
    // Band 1: 32 Hz, peaking, +3.5 dB = 35 tenths, Q 1.414.
    assert_eq!(&eq[2..8], &[0x20, 0x00, 0x01, 35, 0x86, 0x05]);
    // Band 5: 500 Hz, -1.5 dB in two's complement.
    assert_eq!(&eq[2 + 6 * 4..2 + 6 * 5], &[0xf4, 0x01, 0x01, 0xf1, 0x86, 0x05]);
    // Band 10: 16 kHz.
    assert_eq!(&eq[2 + 6 * 9..2 + 6 * 9 + 2], &[0x80, 0x3e]);
    assert_eq!(writes[1], save);
}

#[test]
fn nova_5_encodings() {
    let pid = ARCTIS_NOVA_5;
    let save = [output(&[0x00, 0x09], 64), output(&[0x00, 0x35, 0x01], 64)];
    let with_save = |first: (ReportKind, Vec<u8>)| {
        let mut v = vec![first];
        v.extend(save.iter().cloned());
        v
    };
    assert_eq!(
        apply(pid, "sidetone", int(10)),
        with_save(output(&[0x00, 0x39, 10], 64))
    );
    assert_eq!(
        apply(ARCTIS_NOVA_5X, "inactive_time", int(255)),
        [output(&[0x00, 0xa3, 0xff], 64)]
    );
    assert_eq!(
        apply(pid, "mic_volume", int(15)),
        with_save(output(&[0x00, 0x37, 15], 64))
    );
    assert_eq!(
        apply(pid, "mic_mute_led_brightness", choice("medium")),
        with_save(output(&[0x00, 0xae, 0x04], 64))
    );
    assert_eq!(
        apply(pid, "mic_mute_led_brightness", choice("high")),
        with_save(output(&[0x00, 0xae, 0x0a], 64))
    );
    assert_eq!(
        apply(pid, "volume_limiter", SettingValue::Bool(true)),
        with_save(output(&[0x00, 0x27, 0x01], 64))
    );

    let mut gains = vec![0.0; 10];
    gains[0] = 1.0;
    gains[4] = 1.0;
    gains[9] = -1.0;
    let writes = apply(pid, "equalizer", SettingValue::Gains(gains));
    assert_eq!(writes.len(), 3);
    let eq = &writes[0].1;
    // First band moved: low-shelf flag 0x04, 20 + 2 = 22.
    assert_eq!(&eq[2..8], &[0x20, 0x00, 0x04, 22, 0x86, 0x05]);
    // Second band flat: flag 0x01, baseline 20.
    assert_eq!(&eq[8..14], &[0x40, 0x00, 0x01, 20, 0x86, 0x05]);
    // A moved middle band keeps flag 0x01.
    assert_eq!(&eq[2 + 6 * 4..2 + 6 * 5], &[0xf4, 0x01, 0x01, 22, 0x86, 0x05]);
    // Last band moved: high-shelf flag 0x05, 20 - 2 = 18.
    assert_eq!(&eq[2 + 6 * 9..2 + 6 * 10], &[0x80, 0x3e, 0x05, 18, 0x86, 0x05]);
    assert_eq!(&writes[1..], &save);

    let writes = apply(pid, "equalizer_preset", choice("focus"));
    assert_eq!(writes[0].1[2 + 3], 10); // -5 dB -> 20 - 10
    assert_eq!(apply(pid, "save", SettingValue::Trigger), save);
}

#[test]
fn nova_7_encodings() {
    let pid = ARCTIS_NOVA_7;
    assert_eq!(apply(pid, "sidetone", int(3)), [output(&[0x00, 0x39, 3], 64)]);
    assert_eq!(
        apply(ARCTIS_NOVA_7_GEN2, "sidetone", int(3)),
        [output(&[0x00, 0x39, 3], 64), output(&[0x00, 0x09], 64)]
    );
    assert_eq!(apply(pid, "inactive_time", int(120)), [output(&[0x00, 0xa3, 120], 64)]);
    assert_eq!(
        apply(ARCTIS_NOVA_7X, "equalizer_preset", choice("bass")),
        [output(&[0x00, 0x33, 23, 25, 24, 21, 18, 18, 19, 19, 19, 19], 64)]
    );
    assert_eq!(
        apply(
            ARCTIS_NOVA_7_V2,
            "equalizer",
            SettingValue::Gains(vec![10.0, -10.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0])
        ),
        [output(&[0x00, 0x33, 30, 10, 20, 20, 20, 20, 20, 20, 20, 21], 64)]
    );
    assert_eq!(
        apply(pid, "mic_mute_led_brightness", int(3)),
        [output(&[0x00, 0xae, 3], 64)]
    );
    assert_eq!(apply(pid, "mic_volume", int(7)), [output(&[0x00, 0x37, 7], 64)]);
    assert_eq!(
        apply(pid, "volume_limiter", SettingValue::Bool(true)),
        [output(&[0x00, 0x3a, 1], 64)]
    );
    assert_eq!(
        apply(pid, "bluetooth_when_powered_on", SettingValue::Bool(true)),
        [output(&[0x00, 0xb2, 1], 64), output(&[0x06, 0x09], 64)]
    );
    assert_eq!(
        apply(pid, "bluetooth_call_volume", choice("mute_game")),
        [output(&[0x00, 0xb3, 2], 64)]
    );
    assert_eq!(
        apply(ARCTIS_NOVA_7P, "save", SettingValue::Trigger),
        [output(&[0x00, 0x09], 64)]
    );
}

#[test]
fn every_capability_of_every_model_encodes_a_sample_value() {
    for model in MODELS {
        for descriptor in model.descriptors() {
            let value = match &descriptor.kind {
                SettingKind::Range { max, .. } => SettingValue::Int(*max),
                SettingKind::Choice { options } => SettingValue::Choice(options[options.len() - 1].id.clone()),
                SettingKind::Toggle => SettingValue::Bool(true),
                SettingKind::ColorZones { zones } => SettingValue::Colors(vec![Color::RED; zones.len()]),
                SettingKind::Equalizer { bands_hz, min_db, .. } => SettingValue::Gains(vec![*min_db; bands_hz.len()]),
                SettingKind::Action => SettingValue::Trigger,
                other => panic!("{}: unexpected kind {other:?}", model.name),
            };
            let writes = apply(model.product_id, &descriptor.id, value);
            assert!(!writes.is_empty(), "{} {} wrote nothing", model.name, descriptor.id);
            for (_, bytes) in writes {
                assert!(
                    bytes.len() >= 2 && bytes.len() <= 521,
                    "{} {}",
                    model.name,
                    descriptor.id
                );
            }
        }
    }
}

// ---- Validation --------------------------------------------------------------------------------

#[test]
fn invalid_values_write_nothing() {
    let fake = Fake::default();
    let mut hs = headset(ARCTIS_NOVA_7, &fake);
    assert!(hs.apply_setting("sidetone", &int(4)).is_err());
    assert!(
        hs.apply_setting("equalizer", &SettingValue::Gains(vec![0.0; 9]))
            .is_err()
    );
    assert!(
        hs.apply_setting("equalizer", &SettingValue::Gains(vec![11.0; 10]))
            .is_err()
    );
    assert!(hs.apply_setting("bluetooth_call_volume", &choice("loud")).is_err());
    assert!(matches!(
        hs.apply_setting("lights", &SettingValue::Bool(true)),
        Err(Error::Unsupported(_))
    ));
    assert!(fake.writes().is_empty());

    let mut hs = headset(ARCTIS_PRO_WIRELESS, &fake);
    assert!(hs.apply_setting("inactive_time", &int(25)).is_err());
    assert!(fake.writes().is_empty());
}

#[test]
fn closed_headset_refuses_io() {
    let fake = Fake::default();
    let mut hs = headset(ARCTIS_NOVA_7, &fake);
    hs.close().unwrap();
    assert!(!hs.is_connected());
    assert!(hs.apply_setting("sidetone", &int(1)).is_err());
    assert!(hs.read_status().is_err());
}

// ---- read_status against a scripted device -------------------------------------------------------

#[test]
fn nova7_status_skips_stale_reports() {
    let fake = Fake::default()
        .with_stale(&padded(&[0x99], 64))
        .reply(&[&padded(&[0xb0, 0x00, 3, 0x03, 100, 60], 64)]);
    let mut hs = headset(ARCTIS_NOVA_7, &fake);
    let status = hs.read_status().unwrap();
    assert_eq!(status.battery_percent, Some(75));
    assert_eq!(status.charging, Some(false));
    assert_eq!(status.wireless_connected, Some(true));
    assert_eq!(status.chatmix, Some(ChatMix { game: 100, chat: 60 }));
    assert_eq!(fake.writes(), [(ReportKind::Output, vec![0x00, 0xb0])]);
}

#[test]
fn nova7_status_prefers_the_tagged_answer() {
    let fake = Fake::default().reply(&[&padded(&[0x20, 0x00, 0x01], 64), &padded(&[0xb0, 0x00, 80, 0x01], 64)]);
    let mut hs = headset(ARCTIS_NOVA_7_V2, &fake);
    let status = hs.read_status().unwrap();
    assert_eq!(status.battery_percent, Some(80));
    assert_eq!(status.charging, Some(true));
}

#[test]
fn nova7_gen2_reads_sidetone_past_async_status() {
    let fake = Fake::default()
        .reply(&[&padded(&[0xb0, 0x00, 64, 0x03, 100, 100], 64)])
        .reply(&[&padded(&[0xb0, 0x00, 64, 0x03], 64), &padded(&[0x20, 0x00, 0x02], 64)]);
    let mut hs = headset(ARCTIS_NOVA_7_GEN2, &fake);
    let status = hs.read_status().unwrap();
    assert_eq!(status.battery_percent, Some(64));
    assert_eq!(status.extra.get("sidetone").map(String::as_str), Some("2"));
    let writes = fake.writes();
    assert_eq!(writes.len(), 2);
    assert_eq!(writes[1], output(&[0x00, 0x20], 64));
}

#[test]
fn nova_pro_wireless_keeps_the_last_dial_position() {
    let fake = Fake::default().with_stale(&padded(&[0x07, 0x45, 100, 40], 64)).reply(&[
        &padded(&[0x07, 0x45, 90, 100], 64),
        &{
            let mut r = padded(&[0x07, 0xb0], 64);
            r[6] = 8;
            r[15] = 0x08;
            r
        },
    ]);
    let mut hs = headset(ARCTIS_NOVA_PRO_WIRELESS, &fake);
    let status = hs.read_status().unwrap();
    assert_eq!(status.battery_percent, Some(100));
    assert_eq!(status.chatmix, Some(ChatMix { game: 90, chat: 100 }));
    assert_eq!(fake.writes(), [output(&[0x06, 0xb0], 31)]);

    // No dial movement since: the cached position is still reported.
    let status = hs.read_status().unwrap();
    assert_eq!(status.chatmix, Some(ChatMix { game: 90, chat: 100 }));
    assert_eq!(status.battery_percent, None);
}

#[test]
fn pro_wireless_offline_skips_battery_query() {
    let fake = Fake::default().reply(&[&[0x02, 0x00]]);
    let mut hs = headset(ARCTIS_PRO_WIRELESS, &fake);
    let status = hs.read_status().unwrap();
    assert_eq!(status.wireless_connected, Some(false));
    assert_eq!(status.battery_percent, None);
    assert_eq!(fake.writes().len(), 1);
}

#[test]
fn pro_wireless_online_reads_battery() {
    let fake = Fake::default().reply(&[&[0x04, 0x00]]).reply(&[&[0x02]]);
    let mut hs = headset(ARCTIS_PRO_WIRELESS, &fake);
    let status = hs.read_status().unwrap();
    assert_eq!(status.wireless_connected, Some(true));
    assert_eq!(status.battery_percent, Some(50));
    let writes = fake.writes();
    assert_eq!(writes[1], output(&[0x40, 0xaa], 31));
}

#[test]
fn arctis_7_reads_battery_then_chatmix() {
    let fake = Fake::default()
        .reply(&[&[0x06, 0x18, 42, 0, 0, 0, 0, 0]])
        .reply(&[&[0x06, 0x24, 0xff, 0xdf, 0, 0, 0, 0]]);
    let mut hs = headset(ARCTIS_7, &fake);
    let status = hs.read_status().unwrap();
    assert_eq!(status.battery_percent, Some(42));
    assert_eq!(status.chatmix, Some(ChatMix { game: 100, chat: 50 }));
    assert_eq!(
        fake.writes(),
        [
            (ReportKind::Output, vec![0x06, 0x18]),
            (ReportKind::Output, vec![0x06, 0x24])
        ]
    );
}

#[test]
fn wired_arctis_pro_only_reads_chatmix() {
    let fake = Fake::default().reply(&[&[0x06, 0x24, 0x00, 0x00]]);
    let mut hs = headset(ARCTIS_PRO, &fake);
    let status = hs.read_status().unwrap();
    assert_eq!(status.battery_percent, None);
    assert_eq!(status.chatmix, Some(ChatMix { game: 100, chat: 100 }));
    assert_eq!(fake.writes().len(), 1);
}

#[test]
fn silent_device_returns_empty_status_without_blocking() {
    let fake = Fake::default();
    let mut hs = headset(ARCTIS_NOVA_5, &fake);
    let started = Instant::now();
    let status = hs.read_status().unwrap();
    assert!(status.is_empty());
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[test]
fn models_without_status_send_nothing() {
    let fake = Fake::default();
    let mut hs = headset(ARCTIS_NOVA_3, &fake);
    assert!(hs.read_status().unwrap().is_empty());
    assert!(fake.writes().is_empty());
}

#[test]
fn gamebuds_status() {
    let fake = Fake::default().reply(&[&padded(&[0xb0, 0, 0, 0x03, 0x03, 55, 70], 64)]);
    let mut hs = headset(ARCTIS_GAMEBUDS, &fake);
    let status = hs.read_status().unwrap();
    assert_eq!(status.battery_percent, Some(55));
    assert_eq!(status.extra.get("battery_right").map(String::as_str), Some("70"));
}

#[test]
fn headset_trait_exposes_the_model() {
    let fake = Fake::default();
    let hs: Box<dyn Headset> = Box::new(headset(ARCTIS_NOVA_5X, &fake));
    assert_eq!(hs.model().product_id, ARCTIS_NOVA_5X);
    assert_eq!(hs.device_type(), DeviceType::Headset);
    assert_eq!(hs.setting_descriptors().len(), hs.model().capabilities.len());
}
