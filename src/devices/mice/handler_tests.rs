//! Ports of rivalcfg's handler tests (`test/handlers/*.py`, rivalcfg commit f16c521).
//!
//! Each test names the rivalcfg test it reproduces. Expected bytes are copied from those tests;
//! inputs rivalcfg gives as strings ("#F20", "rgbgradient(...)") are written here as the typed
//! values they parse to.

use std::collections::BTreeMap;

use super::encode::*;
use crate::rgb::Color;

fn rgb(hex: u32) -> Color {
    Color::from_hex(hex)
}

fn stop(position: u8, hex: u32) -> GradientStop {
    GradientStop {
        position,
        color: rgb(hex),
    }
}

// ---- helpers.py doctests -------------------------------------------------------------------

/// rivalcfg/helpers.py, `uint_to_little_endian_bytearray` doctests.
#[test]
fn helpers_uint_to_little_endian_bytearray() {
    assert_eq!(le_bytes(0x42, 1).unwrap(), vec![66]);
    assert_eq!(le_bytes(0x42, 2).unwrap(), vec![66, 0]);
    assert_eq!(le_bytes(0xFF42, 2).unwrap(), vec![66, 255]);
    assert_eq!(le_bytes(0xFF42, 4).unwrap(), vec![66, 255, 0, 0]);
    assert!(le_bytes(0xFF_FFFF, 2).is_err());
}

/// rivalcfg/handlers/range.py, `matches_value_in_range` doctests.
#[test]
fn range_matches_value_in_range() {
    assert_eq!(Span::new(0, 100, 10).snap(40), 40);
    assert_eq!(Span::new(0, 100, 10).snap(42), 40);
    assert_eq!(Span::new(0, 1000, 100).snap(51), 100);
    assert_eq!(Span::new(42, 1000, 100).snap(150), 142);
    assert_eq!(Span::new(500, 1000, 100).snap(100), 500);
    assert_eq!(Span::new(500, 1000, 100).snap(4000), 1000);
}

#[test]
fn frame_appends_suffix_and_pads_without_truncating() {
    assert_eq!(frame(&[0x05, 0x00], &[0xAA], &[0x64], 0), vec![0x05, 0x00, 0xAA, 0x64]);
    assert_eq!(frame(&[0x05], &[0xAA], &[], 4), vec![0x05, 0xAA, 0x00, 0x00]);
    assert_eq!(frame(&[0x05], &[0xAA, 0xBB], &[], 2), vec![0x05, 0xAA, 0xBB]);
}

#[test]
fn report_wire_and_rivalcfg_bytes() {
    let report = Report {
        kind: ReportKind::Feature,
        data: vec![0x5B, 0x00],
    };
    assert_eq!(report.wire_bytes(), vec![0x00, 0x5B, 0x00]);
    assert_eq!(report.rivalcfg_bytes(), vec![0x03, 0x00, 0x5B, 0x00]);
}

// ---- test_range.py -------------------------------------------------------------------------

/// test/handlers/test_range.py::TestProcessValue::test_range_values
#[test]
fn range_values() {
    let (input, output) = (Span::new(100, 2000, 100), Span::new(2, 40, 2));
    for (value, expected) in [(100, 2), (200, 4), (2000, 40), (149, 2), (150, 4), (300, 6)] {
        assert_eq!(
            encode_range(input, output, 1, value).unwrap(),
            vec![expected],
            "{value}"
        );
    }
}

/// test/handlers/test_range.py::TestProcessValue::test_range_values2
#[test]
fn range_values2() {
    let (input, output) = (Span::new(1, 20, 1), Span::new(1000, 40000, 2000));
    for (value, expected) in [
        (1, [0xE8, 0x03]),
        (2, [0xB8, 0x0B]),
        (20, [0x58, 0x98]),
        (14, [0x78, 0x69]),
    ] {
        assert_eq!(
            encode_range(input, output, 2, value).unwrap(),
            expected.to_vec(),
            "{value}"
        );
    }
}

#[test]
fn range_rejects_mismatched_lengths() {
    assert!(encode_range(Span::new(200, 7200, 100), Span::new(0x04, 0xA7, 2), 1, 200).is_err());
}

// ---- test_range_choice.py ------------------------------------------------------------------

const NEAREST_CHOICES: &[(u32, u32)] = &[
    (100, 0x00),
    (200, 0x02),
    (300, 0x03),
    (400, 0x05),
    (500, 0x06),
    (600, 0x08),
    (700, 0x10),
    (800, 0x11),
    (900, 0x13),
    (1000, 0x1A),
];

/// test/handlers/test_range_choice.py::Test_find_nearest_choice::test_values
#[test]
fn range_choice_find_nearest_choice() {
    let as_identity: Vec<(u32, u32)> = NEAREST_CHOICES.iter().map(|(k, _)| (*k, *k)).collect();
    for (value, expected) in [
        (99, 100),
        (100, 100),
        (150, 100),
        (151, 200),
        (999, 1000),
        (1000, 1000),
        (3000, 1000),
    ] {
        assert_eq!(nearest_choice(&as_identity, value), Some(expected), "{value}");
    }
}

const PROCESS_CHOICES: &[(u32, u32)] = &[
    (100, 0x00),
    (200, 0x02),
    (300, 0x03),
    (400, 0x06),
    (500, 0x10),
    (600, 0x1A),
    (700, 0x2B),
    (800, 0xCC),
    (900, 0xDD),
    (1000, 0xFF),
];

/// test/handlers/test_range_choice.py::TestProcessValue::test_range_values
#[test]
fn range_choice_values() {
    let input = Span::new(100, 1000, 100);
    for (value, expected) in [
        (100, 0x00),
        (200, 0x02),
        (1000, 0xFF),
        (150, 0x00),
        (151, 0x02),
        (300, 0x03),
    ] {
        assert_eq!(
            encode_range_choice(input, PROCESS_CHOICES, 1, value).unwrap(),
            vec![expected],
            "{value}"
        );
    }
}

/// test/handlers/test_range_choice.py::TestProcessValue::test_range_values2
#[test]
fn range_choice_values2() {
    let mut table = PROCESS_CHOICES.to_vec();
    table[9] = (1000, 0xFFEE);
    let input = Span::new(100, 1000, 100);
    for (value, expected) in [
        (100, [0x00, 0x00]),
        (200, [0x02, 0x00]),
        (500, [0x10, 0x00]),
        (1000, [0xEE, 0xFF]),
    ] {
        assert_eq!(
            encode_range_choice(input, &table, 2, value).unwrap(),
            expected.to_vec(),
            "{value}"
        );
    }
}

// ---- test_choice.py ------------------------------------------------------------------------

const CHOICES: &[ChoiceEntry] = &[
    ChoiceEntry::new("foo", "foo", &[0xDD]),
    ChoiceEntry::new("0", "0", &[0xAA]),
    ChoiceEntry::new("10", "10", &[0xBB]),
    ChoiceEntry::new("20", "20", &[0xCC]),
    ChoiceEntry::new("multi", "multi", &[0x11, 0x22]),
];

/// test/handlers/test_choice.py::TestProcessValue (valid int-as-string, string, multibyte,
/// invalid).
#[test]
fn choice_process_value() {
    assert_eq!(encode_choice(CHOICES, "10").unwrap(), vec![0xBB]);
    assert_eq!(encode_choice(CHOICES, "foo").unwrap(), vec![0xDD]);
    assert_eq!(encode_choice(CHOICES, "multi").unwrap(), vec![0x11, 0x22]);
    assert!(encode_choice(CHOICES, "42").is_err());
}

// ---- test_multidpi_range.py ----------------------------------------------------------------

fn linear_dpi(width: u8, first_preset: u8, count_mode: CountMode) -> MultiDpi {
    MultiDpi {
        encoding: DpiEncoding::Range {
            input: Span::new(100, 2000, 100),
            output: Span::new(1, 20, 1),
        },
        width,
        first_preset,
        count_mode,
        max_stages: 5,
        xy: None,
    }
}

fn same_axes(dpis: &[u32]) -> Vec<(u32, u32)> {
    dpis.iter().map(|d| (*d, *d)).collect()
}

/// test/handlers/test_multidpi_range.py::TestProcessValue::test_values
#[test]
fn multidpi_range_values() {
    let spec = linear_dpi(1, 1, CountMode::Number);
    let cases: [(&[u32], &[u8]); 3] = [
        (&[100], &[0x01, 0x01, 0x01]),
        (&[100, 200, 300], &[0x03, 0x01, 0x01, 0x02, 0x03]),
        (&[100, 200, 300, 400, 500], &[0x05, 0x01, 0x01, 0x02, 0x03, 0x04, 0x05]),
    ];
    for (dpis, expected) in cases {
        assert_eq!(encode_multidpi(&spec, &same_axes(dpis), 0).unwrap(), expected.to_vec());
    }
}

/// test/handlers/test_multidpi_range.py: too many / too few presets, selected preset.
#[test]
fn multidpi_range_preset_checks() {
    let spec = linear_dpi(1, 1, CountMode::Number);
    assert!(encode_multidpi(&spec, &same_axes(&[100, 200, 300, 400, 500, 600]), 0).is_err());
    assert!(encode_multidpi(&spec, &[], 0).is_err());
    for selected in 0..5u8 {
        assert_eq!(
            encode_multidpi(&spec, &same_axes(&[100, 200, 300, 400, 500]), usize::from(selected)).unwrap(),
            vec![0x05, selected + 1, 0x01, 0x02, 0x03, 0x04, 0x05]
        );
    }
    assert!(encode_multidpi(&spec, &same_axes(&[100, 200]), 2).is_err());
}

/// test/handlers/test_multidpi_range.py::test_dpi_length_byte_count_mode_flag and
/// ::test_first_preset
#[test]
fn multidpi_range_flag_mode_and_first_preset() {
    let flag = linear_dpi(2, 1, CountMode::Flag);
    assert_eq!(
        encode_multidpi(&flag, &same_axes(&[100, 200]), 0).unwrap(),
        vec![0b0000_0011, 0x01, 0x01, 0x00, 0x02, 0x00]
    );
    let zero = linear_dpi(1, 0, CountMode::Number);
    assert_eq!(
        encode_multidpi(&zero, &same_axes(&[100, 200]), 0).unwrap(),
        vec![0x02, 0x00, 0x01, 0x02]
    );
}

#[test]
fn multidpi_single_axis_rejects_split_xy() {
    let spec = linear_dpi(1, 1, CountMode::Number);
    assert!(encode_multidpi(&spec, &[(100, 200)], 0).is_err());
}

// ---- test_multidpi_range_choice.py ---------------------------------------------------------

fn table_dpi(width: u8, first_preset: u8, xy: Option<XyLayout>) -> MultiDpi {
    MultiDpi {
        encoding: DpiEncoding::Table {
            input: Span::new(100, 1000, 100),
            table: NEAREST_CHOICES,
        },
        width,
        first_preset,
        count_mode: CountMode::Number,
        max_stages: 5,
        xy,
    }
}

/// test/handlers/test_multidpi_range_choice.py::TestProcessValue::test_values
#[test]
fn multidpi_range_choice_values() {
    let spec = table_dpi(1, 1, None);
    let cases: [(&[u32], &[u8]); 3] = [
        (&[100], &[0x01, 0x01, 0x00]),
        (&[100, 200, 300], &[0x03, 0x01, 0x00, 0x02, 0x03]),
        (&[100, 200, 300, 400, 500], &[0x05, 0x01, 0x00, 0x02, 0x03, 0x05, 0x06]),
    ];
    for (dpis, expected) in cases {
        assert_eq!(encode_multidpi(&spec, &same_axes(dpis), 0).unwrap(), expected.to_vec());
    }
    for selected in 0..5u8 {
        assert_eq!(
            encode_multidpi(&spec, &same_axes(&[100, 200, 300, 400, 500]), usize::from(selected)).unwrap(),
            vec![0x05, selected + 1, 0x00, 0x02, 0x03, 0x05, 0x06]
        );
    }
}

/// test/handlers/test_multidpi_range_choice.py::test_dpi_length_byte and ::test_first_preset
#[test]
fn multidpi_range_choice_width_and_first_preset() {
    assert_eq!(
        encode_multidpi(&table_dpi(2, 1, None), &same_axes(&[100, 200]), 0).unwrap(),
        vec![0x02, 0x01, 0x00, 0x00, 0x02, 0x00]
    );
    assert_eq!(
        encode_multidpi(&table_dpi(1, 0, None), &same_axes(&[100, 200]), 0).unwrap(),
        vec![0x02, 0x00, 0x00, 0x02]
    );
}

/// test/handlers/test_multidpi_range_choice.py::test_mismatch_input_range_and_output_choices_*
#[test]
fn multidpi_range_choice_table_mismatch() {
    const SHORT: &[(u32, u32)] = &[(100, 0x00), (200, 0x02), (300, 0x03), (400, 0x05), (500, 0x06)];
    const BAD_MIN: &[(u32, u32)] = &[(10, 0x00), (200, 0x02), (300, 0x03), (400, 0x05), (500, 0x06)];
    const BAD_MAX: &[(u32, u32)] = &[(100, 0x00), (200, 0x02), (300, 0x03), (400, 0x05), (1000, 0x06)];
    for (input, table) in [
        (Span::new(100, 1000, 100), SHORT),
        (Span::new(100, 500, 100), BAD_MIN),
        (Span::new(100, 500, 100), BAD_MAX),
    ] {
        let spec = MultiDpi {
            encoding: DpiEncoding::Table { input, table },
            ..table_dpi(1, 1, None)
        };
        assert!(encode_multidpi(&spec, &[(100, 100)], 0).is_err());
    }
}

// ---- test_multidpi_range_choice_xy.py ------------------------------------------------------

/// test/handlers/test_multidpi_range_choice_xy.py::TestProcessValue::test_values_xyxy
#[test]
fn multidpi_xy_values_xyxy() {
    type Case<'a> = (&'a [(u32, u32)], &'a [u8]);
    let spec = table_dpi(1, 1, Some(XyLayout::Interleaved));
    let cases: [Case<'_>; 4] = [
        (&[(100, 100)], &[0x01, 0x01, 0x00, 0x00]),
        (&[(100, 200)], &[0x01, 0x01, 0x00, 0x02]),
        (&[(100, 100), (200, 300)], &[0x02, 0x01, 0x00, 0x00, 0x02, 0x03]),
        (
            &[(100, 100), (200, 300), (1000, 1000)],
            &[0x03, 0x01, 0x00, 0x00, 0x02, 0x03, 0x1A, 0x1A],
        ),
    ];
    for (stages, expected) in cases {
        assert_eq!(encode_multidpi(&spec, stages, 0).unwrap(), expected.to_vec());
    }
}

/// test/handlers/test_multidpi_range_choice_xy.py::TestProcessValue::test_values_xxyy
#[test]
fn multidpi_xy_values_xxyy() {
    let spec = table_dpi(1, 1, Some(XyLayout::Grouped));
    assert_eq!(
        encode_multidpi(&spec, &[(100, 100), (200, 300)], 0).unwrap(),
        vec![0x02, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00]
    );
    assert_eq!(
        encode_multidpi(&spec, &[(100, 100), (200, 300), (1000, 1000)], 0).unwrap(),
        vec![0x03, 0x01, 0x00, 0x02, 0x1A, 0x00, 0x00, 0x00, 0x03, 0x1A, 0x00, 0x00]
    );
}

/// test/handlers/test_multidpi_range_choice_xy.py: selected preset, dpi_length_byte (xyxy and
/// xxyy), first_preset_zero.
#[test]
fn multidpi_xy_width_selection_and_first_preset() {
    let spec = table_dpi(1, 1, Some(XyLayout::Interleaved));
    for selected in 0..5u8 {
        assert_eq!(
            encode_multidpi(&spec, &same_axes(&[100, 200, 300, 400, 500]), usize::from(selected)).unwrap(),
            vec![
                0x05,
                selected + 1,
                0x00,
                0x00,
                0x02,
                0x02,
                0x03,
                0x03,
                0x05,
                0x05,
                0x06,
                0x06
            ]
        );
    }
    assert!(encode_multidpi(&spec, &same_axes(&[100, 200]), 2).is_err());
    assert_eq!(
        encode_multidpi(
            &table_dpi(2, 1, Some(XyLayout::Interleaved)),
            &same_axes(&[100, 200]),
            0
        )
        .unwrap(),
        vec![0x02, 0x01, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x02, 0x00]
    );
    assert_eq!(
        encode_multidpi(&table_dpi(2, 1, Some(XyLayout::Grouped)), &same_axes(&[100, 200]), 0).unwrap(),
        vec![
            0x02, 0x01, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ]
    );
    assert_eq!(
        encode_multidpi(
            &table_dpi(1, 0, Some(XyLayout::Interleaved)),
            &same_axes(&[100, 200]),
            0
        )
        .unwrap(),
        vec![0x02, 0x00, 0x00, 0x00, 0x02, 0x02]
    );
}

// ---- test_rgbcolor.py / test_reactive_rgbcolor.py ------------------------------------------

/// test/handlers/test_rgbcolor.py::test_valid_color_hex_string and ::test_valid_color_tuple
#[test]
fn rgbcolor_process_value() {
    assert_eq!(encode_rgb(rgb(0xFF2200)), [0xFF, 0x22, 0x00]);
    assert_eq!(encode_rgb(Color::new(0xFF, 0x18, 0x00)), [0xFF, 0x18, 0x00]);
}

/// test/handlers/test_reactive_rgbcolor.py::test_valid_color and ::test_disabling_effect
#[test]
fn reactive_rgbcolor_process_value() {
    assert_eq!(encode_reactive(Some(rgb(0xFF0000))), vec![0x01, 0x00, 0xFF, 0x00, 0x00]);
    assert_eq!(encode_reactive(None), vec![0x00, 0x00, 0x00, 0x00, 0x00]);
}

// ---- test_rgbgradient.py -------------------------------------------------------------------

const GRADIENT_LAYOUT: GradientLayout = GradientLayout {
    header_len: 26,
    led_id_offsets: &[0],
    duration_offset: 1,
    duration_len: 2,
    repeat_offset: 17,
    triggers_offset: 21,
    color_count_offset: 25,
};

fn rivalcfg_default_gradient() -> ColorEffect {
    ColorEffect::Gradient(Gradient {
        duration_ms: 1000,
        stops: vec![stop(0, 0xFF0000), stop(33, 0x00FF00), stop(66, 0x0000FF)],
    })
}

/// test/handlers/test_rgbgradient.py::test_valid_color_hex_string, ::test_named_colors and
/// ::test_valid_color_tuple (led_id 2).
#[test]
fn rgbgradient_steady_colors() {
    let header: [u8; 26] = [
        0x02, 0xe8, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
    ];
    for (color, body) in [
        (rgb(0xFF2200), [0xFF, 0x22, 0x00, 0xFF, 0x22, 0x00, 0x00]),
        (rgb(0xFF0000), [0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00]),
        (rgb(0xFF1800), [0xFF, 0x18, 0x00, 0xFF, 0x18, 0x00, 0x00]),
    ] {
        let mut expected = header.to_vec();
        expected.extend(body);
        assert_eq!(
            encode_gradient(&GRADIENT_LAYOUT, 2, &ColorEffect::Steady(color)).unwrap(),
            expected
        );
    }
}

/// test/handlers/test_rgbgradient.py::test_valid_rgbgradient_dict and ::test_valid_rgbgradient
#[test]
fn rgbgradient_gradient() {
    let expected = vec![
        0x02, 0xe8, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00, 0x00, 0xFF, 0x00,
        0x54, 0x00, 0x00, 0xFF, 0x54, 0xFF, 0x00, 0x00, 0x57,
    ];
    assert_eq!(
        encode_gradient(&GRADIENT_LAYOUT, 2, &rivalcfg_default_gradient()).unwrap(),
        expected
    );
}

#[test]
fn rgbgradient_rejects_bad_stops() {
    let gradient = |stops: Vec<GradientStop>| {
        ColorEffect::Gradient(Gradient {
            duration_ms: 1000,
            stops,
        })
    };
    assert!(encode_gradient(&GRADIENT_LAYOUT, 0, &gradient(vec![])).is_err());
    assert!(
        encode_gradient(
            &GRADIENT_LAYOUT,
            0,
            &gradient(vec![stop(50, 0xFF0000), stop(10, 0x00FF00)])
        )
        .is_err()
    );
    assert!(
        encode_gradient(
            &GRADIENT_LAYOUT,
            0,
            &gradient((0..15).map(|i| stop(i * 6, 0x112233)).collect())
        )
        .is_err()
    );
    assert!(encode_gradient(&GRADIENT_LAYOUT, 0, &gradient(vec![stop(0, 0xFF0000), stop(101, 0)])).is_err());
}

// ---- test_rgbgradientv2.py -----------------------------------------------------------------

const GRADIENT_V2_LAYOUT: GradientV2Layout = GradientV2Layout {
    color_field_len: 139,
    duration_len: 2,
    max_stops: 14,
};

fn v2_expected(stages: &[u8], tail: &[u8]) -> Vec<u8> {
    let mut expected = vec![0x02, 0x1D, 0x01, 0x02, 0x31, 0x51, 0xFF, 0xC8, 0x00];
    expected.extend_from_slice(stages);
    expected.resize(139, 0x00);
    expected.extend_from_slice(tail);
    expected
}

/// test/handlers/test_rgbgradientv2.py::test_valid_color_hex_string, ::test_named_colors and
/// ::test_valid_color_tuple (led_id 2).
#[test]
fn rgbgradientv2_steady_colors() {
    for (color, nibbles) in [
        (rgb(0xFF2200), [0xF0, 0x0F, 0x20, 0x02, 0x00, 0x00]),
        (rgb(0xFF0000), [0xF0, 0x0F, 0x00, 0x00, 0x00, 0x00]),
        (rgb(0xFF1800), [0xF0, 0x0F, 0x80, 0x01, 0x00, 0x00]),
    ] {
        let mut tail = nibbles.to_vec();
        tail.extend([
            0xFF, 0x00, 0xDC, 0x05, 0x8A, 0x02, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0xE8, 0x03,
        ]);
        assert_eq!(
            encode_gradient_v2(&GRADIENT_V2_LAYOUT, 2, &ColorEffect::Steady(color)).unwrap(),
            v2_expected(&[], &tail)
        );
    }
}

/// test/handlers/test_rgbgradientv2.py::test_valid_rgbgradient_dict and ::test_valid_rgbgradient
#[test]
fn rgbgradientv2_gradient() {
    let stages = [
        0x00, 0x00, 0xF4, 0x0C, 0x00, 0x00, 0x4A, 0x01, 0x01, 0x00, 0x00, 0xF4, 0x0C, 0x00, 0x4A, 0x01, 0x02, 0x00,
        0x0C, 0x00, 0xF4, 0x00, 0x54, 0x01,
    ];
    let tail = [
        0xF0, 0x0F, 0x00, 0x00, 0x00, 0x00, 0xFF, 0x00, 0xDC, 0x05, 0x8A, 0x02, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,
        0x03, 0x00, 0xE8, 0x03,
    ];
    assert_eq!(
        encode_gradient_v2(&GRADIENT_V2_LAYOUT, 2, &rivalcfg_default_gradient()).unwrap(),
        v2_expected(&stages, &tail)
    );
}

/// rivalcfg/handlers/rgbgradientv2.py limits: minimum duration `int(stops * 33.3)`, at most
/// 30 s, strictly ascending positions.
#[test]
fn rgbgradientv2_limits() {
    let gradient = |duration_ms: u32, stops: Vec<GradientStop>| ColorEffect::Gradient(Gradient { duration_ms, stops });
    let three = || vec![stop(0, 0xFF0000), stop(33, 0x00FF00), stop(66, 0x0000FF)];
    assert_eq!(gradient_v2_min_duration(4), 133);
    assert!(encode_gradient_v2(&GRADIENT_V2_LAYOUT, 0, &gradient(132, three())).is_err());
    assert!(encode_gradient_v2(&GRADIENT_V2_LAYOUT, 0, &gradient(133, three())).is_ok());
    assert!(encode_gradient_v2(&GRADIENT_V2_LAYOUT, 0, &gradient(30_001, three())).is_err());
    assert!(
        encode_gradient_v2(
            &GRADIENT_V2_LAYOUT,
            0,
            &gradient(1000, vec![stop(0, 0xFF0000), stop(0, 0x00FF00)])
        )
        .is_err()
    );
}

/// Python's `int()` truncates toward zero and `& 255` wraps negatives; the port must match.
#[test]
fn rgbgradientv2_negative_ramps_wrap_like_python() {
    let effect = ColorEffect::Gradient(Gradient {
        duration_ms: 1000,
        stops: vec![stop(0, 0xFF0000), stop(50, 0x000000), stop(100, 0xFF0000)],
    });
    let bytes = encode_gradient_v2(&GRADIENT_V2_LAYOUT, 0x01, &effect).unwrap();
    // 500 ms from red to black: int(-255 / 500 * 16) = -8 -> 0xF8; time 500 = 0x01F4.
    assert_eq!(&bytes[9..17], &[0x00, 0x00, 0xF8, 0x00, 0x00, 0x00, 0xF4, 0x01]);
    assert_eq!(&bytes[17..25], &[0x01, 0x00, 0x08, 0x00, 0x00, 0x00, 0xF4, 0x01]);
}

// ---- test_buttons.py -----------------------------------------------------------------------

const SPECIALS: ButtonLayout = ButtonLayout {
    buttons: &[],
    field_len: 3,
    disable: Some(0x00),
    dpi_switch: Some(0x30),
    scroll_up: Some(0x31),
    scroll_down: Some(0x32),
    keyboard: Some(0x51),
    multimedia: Some(0x61),
};

const BUTTONS1: ButtonLayout = ButtonLayout {
    buttons: &[
        ButtonDef::new("button1", 0x01, 0x00, "button1"),
        ButtonDef::new("button2", 0x02, 0x03, "button2"),
    ],
    ..SPECIALS
};

const BUTTONS2: ButtonLayout = ButtonLayout {
    buttons: &[
        ButtonDef::new("button1", 0x01, 0x00, "button1"),
        ButtonDef::new("button2", 0x02, 0x03, "button2"),
        ButtonDef::new("button3", 0x03, 0x06, "next"),
        ButtonDef::new("button4", 0x04, 0x09, "dpi"),
    ],
    ..SPECIALS
};

fn mapping(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs.iter().map(|(b, a)| (b.to_string(), a.to_string())).collect()
}

fn button1(action: &str) -> Vec<u8> {
    encode_buttons(&BUTTONS1, &mapping(&[("button1", action)])).unwrap()
}

/// test/handlers/test_buttons.py::TestProcessValue::test_string_values_buttons
#[test]
fn buttons_string_values_buttons() {
    assert_eq!(button1("button1"), vec![0x01, 0x00, 0x00, 0x02, 0x00, 0x00]);
    assert_eq!(button1("button2"), vec![0x02, 0x00, 0x00, 0x02, 0x00, 0x00]);
    assert_eq!(
        encode_buttons(&BUTTONS1, &mapping(&[("button1", "button2"), ("button2", "button1")])).unwrap(),
        vec![0x02, 0x00, 0x00, 0x01, 0x00, 0x00]
    );
    assert_eq!(button1("default"), vec![0x01, 0x00, 0x00, 0x02, 0x00, 0x00]);
}

/// test/handlers/test_buttons.py::TestProcessValue::test_string_values_keyboard_qwerty
#[test]
fn buttons_string_values_keyboard_qwerty() {
    for (action, code) in [
        ("A", 0x04),
        ("a", 0x04),
        ("1", 0x1E),
        ("Escape", 0x29),
        ("Esc", 0x29),
        ("equal", 0x2E),
        ("eq", 0x2E),
        ("F1", 0x3A),
        (".", 0x37),
        ("DOT", 0x37),
    ] {
        assert_eq!(button1(action), vec![0x51, code, 0x00, 0x02, 0x00, 0x00], "{action}");
    }
    assert_eq!(
        encode_buttons(&BUTTONS1, &mapping(&[("layout", "qwerty"), ("button1", "A")])).unwrap(),
        vec![0x51, 0x04, 0x00, 0x02, 0x00, 0x00]
    );
}

/// test/handlers/test_buttons.py::TestProcessValue::test_string_values_multimedia
#[test]
fn buttons_string_values_multimedia() {
    for (action, code) in [
        ("Mute", 0xE2),
        ("Next", 0xB5),
        ("PlayPause", 0xCD),
        ("Previous", 0xB6),
        ("VolumeUp", 0xE9),
        ("VolumeDown", 0xEA),
        ("Vol+", 0xE9),
        ("vol+", 0xE9),
    ] {
        assert_eq!(button1(action), vec![0x61, code, 0x00, 0x02, 0x00, 0x00], "{action}");
    }
}

/// test/handlers/test_buttons.py::TestProcessValue::test_string_values_scroll,
/// ::test_string_value_dpi and ::test_string_value_disable
#[test]
fn buttons_special_values() {
    for (action, code) in [
        ("ScrollUp", 0x31),
        ("ScrollDown", 0x32),
        ("scrollup", 0x31),
        ("scrolldown", 0x32),
        ("scrolldwn", 0x32),
        ("scrollDn", 0x32),
        ("dpi", 0x30),
        ("disable", 0x00),
        ("disabled", 0x00),
    ] {
        assert_eq!(button1(action), vec![code, 0x00, 0x00, 0x02, 0x00, 0x00], "{action}");
    }
}

/// test/handlers/test_buttons.py::TestProcessValue::test_dict_values
#[test]
fn buttons_dict_values() {
    let bytes = encode_buttons(
        &BUTTONS2,
        &mapping(&[
            ("button1", "button4"),
            ("button2", "PlayPause"),
            ("button3", "Enter"),
            ("button4", "dpi"),
        ]),
    )
    .unwrap();
    assert_eq!(
        bytes,
        vec![0x04, 0x00, 0x00, 0x61, 0xCD, 0x00, 0x51, 0x28, 0x00, 0x30, 0x00, 0x00]
    );
}

/// test/handlers/test_buttons.py::TestProcessValue::test_default_values
#[test]
fn buttons_default_values() {
    let expected = vec![0x01, 0x00, 0x00, 0x02, 0x00, 0x00, 0x61, 0xB5, 0x00, 0x30, 0x00, 0x00];
    for pairs in [
        vec![("layout", "QWERTY")],
        vec![("Layout", "qwerty")],
        vec![("button1", "default")],
        vec![("Button1", "Default")],
        vec![("button1", "button1")],
        vec![],
    ] {
        assert_eq!(
            encode_buttons(&BUTTONS2, &mapping(&pairs)).unwrap(),
            expected,
            "{pairs:?}"
        );
    }
}

#[test]
fn buttons_reject_unknown_names() {
    assert!(encode_buttons(&BUTTONS1, &mapping(&[("button9", "button1")])).is_err());
    assert!(encode_buttons(&BUTTONS1, &mapping(&[("button1", "fly")])).is_err());
    assert!(encode_buttons(&BUTTONS1, &mapping(&[("layout", "azerty")])).is_err());
}

#[test]
fn button_actions_are_lowercase_unique_and_encodable() {
    let names = BUTTONS2.action_names();
    assert_eq!(names[0], DEFAULT_ACTION);
    for name in &names {
        assert_eq!(name, &name.to_ascii_lowercase());
        assert_eq!(names.iter().filter(|n| *n == name).count(), 1, "{name} listed twice");
        encode_buttons(&BUTTONS2, &mapping(&[("button1", name)])).unwrap();
    }
    for action in [
        "button4", "disabled", "dpi", "scrollup", "scrolldn", "mute", "volup", "a", "f24", "lctrl",
    ] {
        assert!(names.iter().any(|n| n == action), "{action} missing");
    }
}
