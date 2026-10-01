//! [EXPERIMENTAL] Mouse profiles ported from rivalcfg (`rivalcfg/devices/*.py`, WTFPL,
//! commit f16c521).
//!
//! One `pub static` per rivalcfg device file, holding its commands, value ranges, defaults and
//! LED zones as data. The 2.4 GHz profiles of dual-mode mice reuse the wired settings table with
//! `wireless: true`, exactly as rivalcfg's `*_wireless_wireless.py` files patch theirs. Nothing
//! here has been tested on hardware by this project; every table is checked against rivalcfg's
//! own test expectations in `device_spec_tests.rs`.
//!
//! This file was generated from the rivalcfg sources and then reviewed; keep edits in step
//! with rivalcfg when changing it.

use super::dpi_tables;
use super::encode::{
    ButtonDef, ButtonLayout, ChoiceEntry, CountMode, DpiEncoding, GradientLayout, GradientV2Layout, MultiDpi,
    ReportKind::{Feature, Output},
    Span, XyLayout,
};
use super::profile::{BatteryFormat, BatteryQuery, Command, EffectDefault, FirmwareQuery, Profile, Setting, Value};
use crate::rgb::Color;

/// Used by: Aerox 3, Aerox 5, Kana v2, Kinzu v2, Prime, Prime Mini, Prime+, Rival 3, Rival 3 Gen 2,
/// Rival 5, Rival 95 / Rival 100 PC Bang, Rival 100 / Rival 105, Rival 110 / Rival 106, Rival 300 /
/// Rival, Rival 310, Rival 500, Rival 600, Rival 650 Wireless, Rival 700 / Rival 710, Sensei 310,
/// Sensei [RAW], Sensei TEN.
const POLLING_RATE_CODES_4_TO_1: &[ChoiceEntry] = &[
    ChoiceEntry::new("125", "125 Hz", &[0x04]),
    ChoiceEntry::new("250", "250 Hz", &[0x03]),
    ChoiceEntry::new("500", "500 Hz", &[0x02]),
    ChoiceEntry::new("1000", "1000 Hz", &[0x01]),
];

/// Used by: Aerox 3, Aerox 5, Rival 3 Gen 2.
const AEROX_3_RAINBOW_EFFECT: &[ChoiceEntry] = &[
    ChoiceEntry::new("all", "All", &[0x07]),
    ChoiceEntry::new("bottom", "Bottom", &[0x04]),
    ChoiceEntry::new("middle", "Middle", &[0x02]),
    ChoiceEntry::new("top", "Top", &[0x01]),
    ChoiceEntry::new("bottom-middle", "Bottom middle", &[0x06]),
    ChoiceEntry::new("middle-top", "Middle top", &[0x03]),
    ChoiceEntry::new("bottom-top", "Bottom top", &[0x05]),
];

/// Used by: Aerox 3, Aerox 3 Wireless, Prime Mini, Prime Wireless, Rival 3 Gen 2.
const AEROX_3_BUTTONS: ButtonLayout = ButtonLayout {
    buttons: &[
        ButtonDef::new("button1", 0x01, 0x00, "button1"),
        ButtonDef::new("button2", 0x02, 0x05, "button2"),
        ButtonDef::new("button3", 0x03, 0x0A, "button3"),
        ButtonDef::new("button4", 0x04, 0x0F, "button4"),
        ButtonDef::new("button5", 0x05, 0x14, "button5"),
        ButtonDef::new("button6", 0x06, 0x19, "dpi"),
        ButtonDef::new("scrollup", 0x31, 0x1E, "scrollup"),
        ButtonDef::new("scrolldown", 0x32, 0x23, "scrolldown"),
    ],
    field_len: 5,
    disable: Some(0x00),
    dpi_switch: Some(0x30),
    scroll_up: None,
    scroll_down: None,
    keyboard: Some(0x51),
    multimedia: Some(0x61),
};

/// Used by: Aerox 3, Aerox 3 Wireless, Aerox 5, Aerox 5 Wireless, Aerox 9 Wireless, Rival 3 Gen 2.
const AEROX_3_DEFAULT_LIGHTING: &[ChoiceEntry] = &[
    ChoiceEntry::new("off", "Off", &[0x00, 0x00]),
    ChoiceEntry::new("reactive", "Reactive", &[0x00, 0x01]),
    ChoiceEntry::new("rainbow", "Rainbow", &[0x01, 0x00]),
    ChoiceEntry::new("reactive-rainbow", "Reactive rainbow", &[0x01, 0x01]),
];

/// Used by: Aerox 3 Wireless, Aerox 5 Wireless, Aerox 9 Wireless, Prime Wireless, Rival 3 Wireless,
/// Rival 3 Wireless Gen 2.
const POLLING_RATE_CODES_3_TO_0: &[ChoiceEntry] = &[
    ChoiceEntry::new("125", "125 Hz", &[0x03]),
    ChoiceEntry::new("250", "250 Hz", &[0x02]),
    ChoiceEntry::new("500", "500 Hz", &[0x01]),
    ChoiceEntry::new("1000", "1000 Hz", &[0x00]),
];

/// Used by: Aerox 5, Aerox 5 Wireless.
const AEROX_5_BUTTONS: ButtonLayout = ButtonLayout {
    buttons: &[
        ButtonDef::new("button1", 0x01, 0x00, "button1"),
        ButtonDef::new("button2", 0x02, 0x05, "button2"),
        ButtonDef::new("button3", 0x03, 0x0A, "button3"),
        ButtonDef::new("button4", 0x04, 0x0F, "button4"),
        ButtonDef::new("button5", 0x05, 0x14, "button5"),
        ButtonDef::new("button6", 0x06, 0x19, "dpi"),
        ButtonDef::new("button7", 0x00, 0x1E, "disabled"),
        ButtonDef::new("button8", 0x00, 0x23, "disabled"),
        ButtonDef::new("button9", 0x00, 0x28, "disabled"),
        ButtonDef::new("scrollup", 0x31, 0x2D, "scrollup"),
        ButtonDef::new("scrolldown", 0x32, 0x32, "scrolldown"),
    ],
    field_len: 5,
    disable: Some(0x00),
    dpi_switch: Some(0x30),
    scroll_up: None,
    scroll_down: None,
    keyboard: Some(0x51),
    multimedia: Some(0x61),
};

const KANA_V2_SENSITIVITY1: &[ChoiceEntry] = &[
    ChoiceEntry::new("400", "400 DPI", &[0x08]),
    ChoiceEntry::new("800", "800 DPI", &[0x07]),
    ChoiceEntry::new("1200", "1200 DPI", &[0x06]),
    ChoiceEntry::new("1600", "1600 DPI", &[0x05]),
    ChoiceEntry::new("2000", "2000 DPI", &[0x04]),
    ChoiceEntry::new("2400", "2400 DPI", &[0x03]),
    ChoiceEntry::new("3200", "3200 DPI", &[0x02]),
    ChoiceEntry::new("4000", "4000 DPI", &[0x01]),
];

/// Used by: Kana v2, Sensei [RAW].
const KANA_V2_LED_BRIGHTNESS1: &[ChoiceEntry] = &[
    ChoiceEntry::new("off", "Off", &[0x01]),
    ChoiceEntry::new("low", "Low", &[0x02]),
    ChoiceEntry::new("medium", "Medium", &[0x03]),
    ChoiceEntry::new("high", "High", &[0x04]),
];

const KINZU_V2_SENSITIVITY1: &[ChoiceEntry] = &[
    ChoiceEntry::new("400", "400 DPI", &[0x04]),
    ChoiceEntry::new("800", "800 DPI", &[0x03]),
    ChoiceEntry::new("1600", "1600 DPI", &[0x02]),
    ChoiceEntry::new("3200", "3200 DPI", &[0x01]),
];

/// Used by: Prime, Prime+, Rival 3 Wireless, Rival 3 Wireless Gen 2, Rival 300 / Rival, Rival 310.
const PRIME_BUTTONS: ButtonLayout = ButtonLayout {
    buttons: &[
        ButtonDef::new("button1", 0x01, 0x00, "button1"),
        ButtonDef::new("button2", 0x02, 0x05, "button2"),
        ButtonDef::new("button3", 0x03, 0x0A, "button3"),
        ButtonDef::new("button4", 0x04, 0x0F, "button4"),
        ButtonDef::new("button5", 0x05, 0x14, "button5"),
        ButtonDef::new("button6", 0x06, 0x19, "dpi"),
    ],
    field_len: 5,
    disable: Some(0x00),
    dpi_switch: Some(0x30),
    scroll_up: Some(0x31),
    scroll_down: Some(0x32),
    keyboard: Some(0x51),
    multimedia: Some(0x61),
};

/// Used by: Prime Mini, Prime Wireless, Rival 5.
const PRIME_MINI_DEFAULT_LIGHTING: &[ChoiceEntry] = &[
    ChoiceEntry::new("off", "Off", &[0x00]),
    ChoiceEntry::new("rainbow", "Rainbow", &[0x01]),
];

const RIVAL_3_LIGHT_EFFECT: &[ChoiceEntry] = &[
    ChoiceEntry::new("rainbow-shift", "Rainbow shift", &[0x00]),
    ChoiceEntry::new("breath-fast", "Breath fast", &[0x01]),
    ChoiceEntry::new("breath", "Breath", &[0x02]),
    ChoiceEntry::new("breath-slow", "Breath slow", &[0x03]),
    ChoiceEntry::new("steady", "Steady", &[0x04]),
    ChoiceEntry::new("rainbow-breath", "Rainbow breath", &[0x05]),
    ChoiceEntry::new("disco", "Disco", &[0x06]),
];

const RIVAL_3_BUTTONS: ButtonLayout = ButtonLayout {
    buttons: &[
        ButtonDef::new("button1", 0x01, 0x00, "button1"),
        ButtonDef::new("button2", 0x02, 0x02, "button2"),
        ButtonDef::new("button3", 0x03, 0x04, "button3"),
        ButtonDef::new("button4", 0x04, 0x06, "button4"),
        ButtonDef::new("button5", 0x05, 0x08, "button5"),
        ButtonDef::new("button6", 0x06, 0x0A, "dpi"),
        ButtonDef::new("scrollup", 0x31, 0x0C, "scrollup"),
        ButtonDef::new("scrolldown", 0x32, 0x0E, "scrolldown"),
    ],
    field_len: 2,
    disable: Some(0x00),
    dpi_switch: Some(0x30),
    scroll_up: Some(0x31),
    scroll_down: Some(0x32),
    keyboard: Some(0x33),
    multimedia: Some(0x34),
};

const RIVAL_5_LED_BRIGHTNESS: &[ChoiceEntry] = &[
    ChoiceEntry::new("100", "100%", &[0x64]),
    ChoiceEntry::new("75", "75%", &[0x32]),
    ChoiceEntry::new("50", "50%", &[0x19]),
    ChoiceEntry::new("25", "25%", &[0x0C]),
    ChoiceEntry::new("0", "0%", &[0x00]),
];

const RIVAL_5_BUTTONS: ButtonLayout = ButtonLayout {
    buttons: &[
        ButtonDef::new("button1", 0x01, 0x00, "button1"),
        ButtonDef::new("button2", 0x02, 0x05, "button2"),
        ButtonDef::new("button3", 0x03, 0x0A, "button3"),
        ButtonDef::new("button4", 0x04, 0x0F, "button4"),
        ButtonDef::new("button5", 0x05, 0x14, "button5"),
        ButtonDef::new("button6", 0x06, 0x19, "disabled"),
        ButtonDef::new("button7", 0x00, 0x1E, "disabled"),
        ButtonDef::new("button8", 0x00, 0x23, "disabled"),
        ButtonDef::new("button9", 0x00, 0x28, "dpi"),
        ButtonDef::new("scrollup", 0x31, 0x2D, "scrollup"),
        ButtonDef::new("scrolldown", 0x32, 0x32, "scrolldown"),
    ],
    field_len: 5,
    disable: Some(0x00),
    dpi_switch: Some(0x30),
    scroll_up: None,
    scroll_down: None,
    keyboard: Some(0x51),
    multimedia: Some(0x61),
};

/// Used by: Rival 95 / Rival 100 PC Bang, Rival 100 / Rival 105.
const RIVAL_95_SENSITIVITY1: &[ChoiceEntry] = &[
    ChoiceEntry::new("250", "250 DPI", &[0x08]),
    ChoiceEntry::new("500", "500 DPI", &[0x07]),
    ChoiceEntry::new("1000", "1000 DPI", &[0x06]),
    ChoiceEntry::new("1250", "1250 DPI", &[0x05]),
    ChoiceEntry::new("1500", "1500 DPI", &[0x04]),
    ChoiceEntry::new("1750", "1750 DPI", &[0x03]),
    ChoiceEntry::new("2000", "2000 DPI", &[0x02]),
    ChoiceEntry::new("4000", "4000 DPI", &[0x01]),
];

/// Used by: Rival 95 / Rival 100 PC Bang, Rival 100 / Rival 105, Rival 110 / Rival 106.
const RIVAL_95_BTN6_MODE: &[ChoiceEntry] = &[
    ChoiceEntry::new("dpi", "DPI switch", &[0x00]),
    ChoiceEntry::new("os", "Sent to the OS", &[0x01]),
];

/// Used by: Rival 100 / Rival 105, Rival 110 / Rival 106, Rival 300 / Rival.
const RIVAL_100_LIGHT_EFFECT: &[ChoiceEntry] = &[
    ChoiceEntry::new("steady", "Steady", &[0x01]),
    ChoiceEntry::new("breath", "Breath", &[0x03]),
    ChoiceEntry::new("1", "Effect 1", &[0x01]),
    ChoiceEntry::new("2", "Effect 2", &[0x02]),
    ChoiceEntry::new("3", "Effect 3", &[0x03]),
    ChoiceEntry::new("4", "Effect 4", &[0x04]),
];

/// Used by: Rival 310, Sensei 310, Sensei TEN.
const RIVAL_310_GRADIENT: GradientLayout = GradientLayout {
    header_len: 26,
    led_id_offsets: &[0],
    duration_offset: 1,
    duration_len: 2,
    repeat_offset: 17,
    triggers_offset: 21,
    color_count_offset: 25,
};

/// Used by: Rival 500, Rival 700 / Rival 710.
const RIVAL_500_GRADIENT_V2: GradientV2Layout = GradientV2Layout {
    color_field_len: 139,
    duration_len: 2,
    max_stops: 14,
};

const RIVAL_500_BUTTONS: ButtonLayout = ButtonLayout {
    buttons: &[
        ButtonDef::new("button1", 0x01, 0x00, "button1"),
        ButtonDef::new("button2", 0x02, 0x05, "button2"),
        ButtonDef::new("button3", 0x03, 0x0A, "button3"),
        ButtonDef::new("button4", 0x04, 0x0F, "button4"),
        ButtonDef::new("button5", 0x05, 0x14, "button5"),
        ButtonDef::new("button6", 0x06, 0x19, "button6"),
        ButtonDef::new("button7", 0x07, 0x1E, "button7"),
        ButtonDef::new("button9", 0x00, 0x23, "disabled"),
        ButtonDef::new("button10", 0x00, 0x28, "dpi"),
        ButtonDef::new("button11", 0x00, 0x2D, "disabled"),
        ButtonDef::new("button12", 0x00, 0x32, "disabled"),
        ButtonDef::new("tiltleft", 0x33, 0x37, "tiltleft"),
        ButtonDef::new("tiltright", 0x34, 0x3C, "tiltright"),
        ButtonDef::new("button13", 0x00, 0x41, "disabled"),
        ButtonDef::new("button8", 0x08, 0x46, "button8"),
    ],
    field_len: 5,
    disable: Some(0x00),
    dpi_switch: Some(0x30),
    scroll_up: Some(0x31),
    scroll_down: Some(0x32),
    keyboard: Some(0x51),
    multimedia: Some(0x61),
};

const RIVAL_600_GRADIENT: GradientLayout = GradientLayout {
    header_len: 28,
    led_id_offsets: &[0, 5],
    duration_offset: 6,
    duration_len: 2,
    repeat_offset: 22,
    triggers_offset: 23,
    color_count_offset: 27,
};

/// Used by: Rival 600, Rival 650 Wireless.
const RIVAL_600_BUTTONS: ButtonLayout = ButtonLayout {
    buttons: &[
        ButtonDef::new("button1", 0x01, 0x00, "button1"),
        ButtonDef::new("button2", 0x02, 0x05, "button2"),
        ButtonDef::new("button3", 0x03, 0x0A, "button3"),
        ButtonDef::new("button4", 0x04, 0x0F, "button4"),
        ButtonDef::new("button5", 0x05, 0x14, "button5"),
        ButtonDef::new("button6", 0x06, 0x19, "disabled"),
        ButtonDef::new("button7", 0x00, 0x1E, "dpi"),
    ],
    field_len: 5,
    disable: Some(0x00),
    dpi_switch: Some(0x30),
    scroll_up: Some(0x31),
    scroll_down: Some(0x32),
    keyboard: Some(0x51),
    multimedia: Some(0x61),
};

const RIVAL_650_LIFT_OFF_DISTANCE: &[(u32, u32)] = &[
    (1, 0x7874),
    (2, 0x736F),
    (3, 0x6E6A),
    (4, 0x6965),
    (5, 0x6460),
    (6, 0x5F5B),
    (7, 0x5A56),
    (8, 0x5551),
];

/// Used by: Sensei 310, Sensei TEN.
const SENSEI_310_BUTTONS: ButtonLayout = ButtonLayout {
    buttons: &[
        ButtonDef::new("button1", 0x01, 0x00, "button1"),
        ButtonDef::new("button2", 0x02, 0x05, "button2"),
        ButtonDef::new("button3", 0x03, 0x0A, "button3"),
        ButtonDef::new("button4", 0x04, 0x0F, "button4"),
        ButtonDef::new("button5", 0x05, 0x14, "button5"),
        ButtonDef::new("button6", 0x06, 0x19, "pagedown"),
        ButtonDef::new("button7", 0x07, 0x1E, "pageup"),
        ButtonDef::new("button8", 0x08, 0x23, "dpi"),
    ],
    field_len: 5,
    disable: Some(0x00),
    dpi_switch: Some(0x30),
    scroll_up: Some(0x31),
    scroll_down: Some(0x32),
    keyboard: Some(0x51),
    multimedia: Some(0x61),
};

const SENSEI_RAW_LIGHT_EFFECT: &[ChoiceEntry] = &[
    ChoiceEntry::new("steady", "Steady", &[0x01]),
    ChoiceEntry::new("breath", "Breath", &[0x03]),
    ChoiceEntry::new("1", "Effect 1", &[0x01]),
    ChoiceEntry::new("2", "Effect 2", &[0x02]),
    ChoiceEntry::new("3", "Effect 3", &[0x03]),
    ChoiceEntry::new("4", "Effect 4", &[0x04]),
    ChoiceEntry::new("trigger", "Trigger", &[0x05]),
];

const SENSEI_RAW_BUTTONS: ButtonLayout = ButtonLayout {
    buttons: &[
        ButtonDef::new("button1", 0x01, 0x00, "button1"),
        ButtonDef::new("button2", 0x02, 0x03, "button2"),
        ButtonDef::new("button3", 0x03, 0x06, "button3"),
        ButtonDef::new("button4", 0x04, 0x09, "button4"),
        ButtonDef::new("button5", 0x05, 0x0C, "button5"),
        ButtonDef::new("button6", 0x06, 0x12, "pagedown"),
        ButtonDef::new("button7", 0x07, 0x0F, "pageup"),
        ButtonDef::new("button8", 0x08, 0x15, "dpi"),
    ],
    field_len: 3,
    disable: Some(0x00),
    dpi_switch: Some(0x30),
    scroll_up: None,
    scroll_down: None,
    keyboard: Some(0x10),
    multimedia: Some(0x11),
};

const AEROX_3_SETTINGS: &[Setting] = &[
    Setting::new(
        "sensitivity",
        "Sensitivity presets",
        "Set sensitivity presets (DPI)",
        Output,
        &[0x2D],
        Value::MultiDpi {
            spec: MultiDpi {
                encoding: DpiEncoding::Table {
                    input: Span::new(200, 8500, 100),
                    table: dpi_tables::TRUEMOVE_CORE,
                },
                width: 1,
                first_preset: 1,
                count_mode: CountMode::Number,
                max_stages: 5,
                xy: None,
            },
            default: &[800, 1600],
        },
    ),
    Setting::new(
        "polling_rate",
        "Polling rate",
        "Set polling rate (Hz)",
        Output,
        &[0x2B],
        Value::Choice {
            entries: POLLING_RATE_CODES_4_TO_1,
            default: "1000",
        },
    ),
    Setting::new(
        "z1_color",
        "Strip top LED color",
        "Set the color of the top LED",
        Output,
        &[0x21, 0x01],
        Value::Rgb {
            default: Color::new(0xFF, 0x00, 0x00),
        },
    )
    .zone("top"),
    Setting::new(
        "z2_color",
        "Strip middle LED color",
        "Set the color of the middle LED",
        Output,
        &[0x21, 0x02, 0x00, 0x00, 0x00],
        Value::Rgb {
            default: Color::new(0x00, 0xFF, 0x00),
        },
    )
    .zone("middle"),
    Setting::new(
        "z3_color",
        "Strip bottom LED color",
        "Set the color of the bottom LED",
        Output,
        &[0x21, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        Value::Rgb {
            default: Color::new(0x00, 0x00, 0xFF),
        },
    )
    .zone("bottom"),
    Setting::new(
        "reactive_color",
        "Reactive color",
        "Set the color of the LEDs in reaction to a button click",
        Output,
        &[0x26],
        Value::Reactive { default: None },
    ),
    Setting::new(
        "led_brightness",
        "LED Brightness",
        "Set the brightness of the LEDs",
        Output,
        &[0x23],
        Value::Range {
            input: Span::new(0, 100, 1),
            output: Span::new(0x00, 0x64, 1),
            width: 1,
            default: 100,
            unit: Some("%"),
        },
    ),
    Setting::new(
        "rainbow_effect",
        "Rainbow effect",
        "Set the rainbow effect (can be cleared by setting a color)",
        Output,
        &[0x22],
        Value::Choice {
            entries: AEROX_3_RAINBOW_EFFECT,
            default: "all",
        },
    ),
    Setting::new(
        "buttons",
        "Buttons mapping",
        "Set the mapping of the buttons",
        Output,
        &[0x2A],
        Value::Buttons(&AEROX_3_BUTTONS),
    ),
    Setting::new(
        "default_lighting",
        "Default lighting",
        "Set default lighting at mouse startup",
        Output,
        &[0x27],
        Value::Choice {
            entries: AEROX_3_DEFAULT_LIGHTING,
            default: "rainbow",
        },
    ),
];

const AEROX_3_WIRELESS_WIRED_SETTINGS: &[Setting] = &[
    Setting::new(
        "sensitivity",
        "Sensitivity presets",
        "Set sensitivity presets (DPI)",
        Output,
        &[0x2D],
        Value::MultiDpi {
            spec: MultiDpi {
                encoding: DpiEncoding::Table {
                    input: Span::new(100, 18000, 100),
                    table: dpi_tables::TRUEMOVE_AIR,
                },
                width: 1,
                first_preset: 0,
                count_mode: CountMode::Number,
                max_stages: 5,
                xy: None,
            },
            default: &[400, 800, 1200, 2400, 3200],
        },
    ),
    Setting::new(
        "polling_rate",
        "Polling rate",
        "Set polling rate (Hz)",
        Output,
        &[0x2B],
        Value::Choice {
            entries: POLLING_RATE_CODES_3_TO_0,
            default: "1000",
        },
    ),
    Setting::new(
        "z1_color",
        "Strip top LED color",
        "Set the color of the top LED",
        Output,
        &[0x21, 0x01, 0x00],
        Value::Rgb {
            default: Color::new(0xFF, 0x00, 0x00),
        },
    )
    .zone("top"),
    Setting::new(
        "z2_color",
        "Strip middle LED color",
        "Set the color of the middle LED",
        Output,
        &[0x21, 0x01, 0x01],
        Value::Rgb {
            default: Color::new(0x00, 0xFF, 0x00),
        },
    )
    .zone("middle"),
    Setting::new(
        "z3_color",
        "Strip bottom LED color",
        "Set the color of the bottom LED",
        Output,
        &[0x21, 0x01, 0x02],
        Value::Rgb {
            default: Color::new(0x00, 0x00, 0xFF),
        },
    )
    .zone("bottom"),
    Setting::new(
        "reactive_color",
        "Reactive color",
        "Set the color of the LEDs in reaction to a button click",
        Output,
        &[0x26],
        Value::Reactive { default: None },
    ),
    Setting::new(
        "sleep_timer",
        "Sleep timer",
        "Set the IDLE time before the mouse goes to sleep mode (minutes, 0 = disable)",
        Output,
        &[0x29],
        Value::Range {
            input: Span::new(0, 20, 1),
            output: Span::new(0x00, 0x124F80, 60000),
            width: 3,
            default: 5,
            unit: Some("min"),
        },
    ),
    Setting::new(
        "dim_timer",
        "Dim timer",
        "Set the IDLE time before the mouse light is dimmed (seconds, 0 = disable)",
        Output,
        &[0x23, 0x0F, 0x01, 0x00, 0x00],
        Value::Range {
            input: Span::new(0, 1200, 1),
            output: Span::new(0x00, 0x124F80, 1000),
            width: 3,
            default: 30,
            unit: Some("s"),
        },
    ),
    Setting::new(
        "buttons",
        "Buttons mapping",
        "Set the mapping of the buttons",
        Output,
        &[0x2A],
        Value::Buttons(&AEROX_3_BUTTONS),
    ),
    Setting::new(
        "rainbow_effect",
        "Rainbow effect",
        "Enable the rainbow effect (can be disabled by setting a color)",
        Output,
        &[0x22, 0xFF],
        Value::Trigger,
    ),
    Setting::new(
        "default_lighting",
        "Default lighting",
        "Set default lighting at mouse startup",
        Output,
        &[0x27],
        Value::Choice {
            entries: AEROX_3_DEFAULT_LIGHTING,
            default: "rainbow",
        },
    ),
];

const AEROX_5_SETTINGS: &[Setting] = &[
    Setting::new(
        "sensitivity",
        "Sensitivity presets",
        "Set sensitivity presets (DPI)",
        Output,
        &[0x2D],
        Value::MultiDpi {
            spec: MultiDpi {
                encoding: DpiEncoding::Table {
                    input: Span::new(100, 18000, 100),
                    table: dpi_tables::TRUEMOVE_AIR,
                },
                width: 1,
                first_preset: 0,
                count_mode: CountMode::Number,
                max_stages: 5,
                xy: None,
            },
            default: &[400, 800, 1200, 2400, 3200],
        },
    ),
    Setting::new(
        "polling_rate",
        "Polling rate",
        "Set polling rate (Hz)",
        Output,
        &[0x2B],
        Value::Choice {
            entries: POLLING_RATE_CODES_4_TO_1,
            default: "1000",
        },
    ),
    Setting::new(
        "z1_color",
        "Strip top LED color",
        "Set the color of the top LED",
        Output,
        &[0x21, 0x01],
        Value::Rgb {
            default: Color::new(0xFF, 0x00, 0x00),
        },
    )
    .zone("top"),
    Setting::new(
        "z2_color",
        "Strip middle LED color",
        "Set the color of the middle LED",
        Output,
        &[0x21, 0x02, 0x00, 0x00, 0x00],
        Value::Rgb {
            default: Color::new(0x00, 0xFF, 0x00),
        },
    )
    .zone("middle"),
    Setting::new(
        "z3_color",
        "Strip bottom LED color",
        "Set the color of the bottom LED",
        Output,
        &[0x21, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        Value::Rgb {
            default: Color::new(0x00, 0x00, 0xFF),
        },
    )
    .zone("bottom"),
    Setting::new(
        "reactive_color",
        "Reactive color",
        "Set the color of the LEDs in reaction to a button click",
        Output,
        &[0x26],
        Value::Reactive { default: None },
    ),
    Setting::new(
        "led_brightness",
        "LED Brightness",
        "Set the brightness of the LEDs",
        Output,
        &[0x23],
        Value::Range {
            input: Span::new(0, 100, 1),
            output: Span::new(0x00, 0x64, 1),
            width: 1,
            default: 100,
            unit: Some("%"),
        },
    ),
    Setting::new(
        "rainbow_effect",
        "Rainbow effect",
        "Set the rainbow effect (can be cleared by setting a color)",
        Output,
        &[0x22],
        Value::Choice {
            entries: AEROX_3_RAINBOW_EFFECT,
            default: "all",
        },
    ),
    Setting::new(
        "buttons",
        "Buttons mapping",
        "Set the mapping of the buttons",
        Output,
        &[0x2A],
        Value::Buttons(&AEROX_5_BUTTONS),
    ),
    Setting::new(
        "default_lighting",
        "Default lighting",
        "Set default lighting at mouse startup",
        Output,
        &[0x27],
        Value::Choice {
            entries: AEROX_3_DEFAULT_LIGHTING,
            default: "rainbow",
        },
    ),
];

const AEROX_5_WIRELESS_WIRED_SETTINGS: &[Setting] = &[
    Setting::new(
        "sensitivity",
        "Sensitivity presets",
        "Set sensitivity presets (DPI)",
        Output,
        &[0x2D],
        Value::MultiDpi {
            spec: MultiDpi {
                encoding: DpiEncoding::Table {
                    input: Span::new(100, 18000, 100),
                    table: dpi_tables::TRUEMOVE_AIR,
                },
                width: 1,
                first_preset: 0,
                count_mode: CountMode::Number,
                max_stages: 5,
                xy: None,
            },
            default: &[400, 800, 1200, 2400, 3200],
        },
    ),
    Setting::new(
        "polling_rate",
        "Polling rate",
        "Set polling rate (Hz)",
        Output,
        &[0x2B],
        Value::Choice {
            entries: POLLING_RATE_CODES_3_TO_0,
            default: "1000",
        },
    ),
    Setting::new(
        "z1_color",
        "Strip top LED color",
        "Set the color of the top LED",
        Output,
        &[0x21, 0x01, 0x00],
        Value::Rgb {
            default: Color::new(0xFF, 0x00, 0x00),
        },
    )
    .zone("top"),
    Setting::new(
        "z2_color",
        "Strip middle LED color",
        "Set the color of the middle LED",
        Output,
        &[0x21, 0x01, 0x01],
        Value::Rgb {
            default: Color::new(0x00, 0xFF, 0x00),
        },
    )
    .zone("middle"),
    Setting::new(
        "z3_color",
        "Strip bottom LED color",
        "Set the color of the bottom LED",
        Output,
        &[0x21, 0x01, 0x02],
        Value::Rgb {
            default: Color::new(0x00, 0x00, 0xFF),
        },
    )
    .zone("bottom"),
    Setting::new(
        "reactive_color",
        "Reactive color",
        "Set the color of the LEDs in reaction to a button click",
        Output,
        &[0x26],
        Value::Reactive { default: None },
    ),
    Setting::new(
        "sleep_timer",
        "Sleep timer",
        "Set the IDLE time before the mouse goes to sleep mode (minutes, 0 = disable)",
        Output,
        &[0x29],
        Value::Range {
            input: Span::new(0, 20, 1),
            output: Span::new(0x00, 0x124F80, 60000),
            width: 3,
            default: 5,
            unit: Some("min"),
        },
    ),
    Setting::new(
        "dim_timer",
        "Dim timer",
        "Set the IDLE time before the mouse light is dimmed (seconds, 0 = disable)",
        Output,
        &[0x23, 0x0F, 0x01, 0x00, 0x00],
        Value::Range {
            input: Span::new(0, 1200, 1),
            output: Span::new(0x00, 0x124F80, 1000),
            width: 3,
            default: 30,
            unit: Some("s"),
        },
    ),
    Setting::new(
        "buttons",
        "Buttons mapping",
        "Set the mapping of the buttons",
        Output,
        &[0x2A],
        Value::Buttons(&AEROX_5_BUTTONS),
    ),
    Setting::new(
        "rainbow_effect",
        "Rainbow effect",
        "Enable the rainbow effect (can be disabled by setting a color)",
        Output,
        &[0x22, 0xFF],
        Value::Trigger,
    ),
    Setting::new(
        "default_lighting",
        "Default lighting",
        "Set default lighting at mouse startup",
        Output,
        &[0x27],
        Value::Choice {
            entries: AEROX_3_DEFAULT_LIGHTING,
            default: "rainbow",
        },
    ),
];

const AEROX_9_WIRELESS_WIRED_SETTINGS: &[Setting] = &[
    Setting::new(
        "sensitivity",
        "Sensitivity presets",
        "Set sensitivity presets (DPI)",
        Output,
        &[0x2D],
        Value::MultiDpi {
            spec: MultiDpi {
                encoding: DpiEncoding::Table {
                    input: Span::new(100, 18000, 100),
                    table: dpi_tables::TRUEMOVE_AIR,
                },
                width: 1,
                first_preset: 0,
                count_mode: CountMode::Number,
                max_stages: 5,
                xy: None,
            },
            default: &[400, 800, 1200, 2400, 3200],
        },
    ),
    Setting::new(
        "polling_rate",
        "Polling rate",
        "Set polling rate (Hz)",
        Output,
        &[0x2B],
        Value::Choice {
            entries: POLLING_RATE_CODES_3_TO_0,
            default: "1000",
        },
    ),
    Setting::new(
        "z1_color",
        "Strip top LED color",
        "Set the color of the top LED",
        Output,
        &[0x21, 0x01, 0x00],
        Value::Rgb {
            default: Color::new(0xFF, 0x00, 0x00),
        },
    )
    .zone("top"),
    Setting::new(
        "z2_color",
        "Strip middle LED color",
        "Set the color of the middle LED",
        Output,
        &[0x21, 0x01, 0x01],
        Value::Rgb {
            default: Color::new(0x00, 0xFF, 0x00),
        },
    )
    .zone("middle"),
    Setting::new(
        "z3_color",
        "Strip bottom LED color",
        "Set the color of the bottom LED",
        Output,
        &[0x21, 0x01, 0x02],
        Value::Rgb {
            default: Color::new(0x00, 0x00, 0xFF),
        },
    )
    .zone("bottom"),
    Setting::new(
        "reactive_color",
        "Reactive color",
        "Set the color of the LEDs in reaction to a button click",
        Output,
        &[0x26],
        Value::Reactive { default: None },
    ),
    Setting::new(
        "sleep_timer",
        "Sleep timer",
        "Set the IDLE time before the mouse goes to sleep mode (minutes, 0 = disable)",
        Output,
        &[0x29],
        Value::Range {
            input: Span::new(0, 20, 1),
            output: Span::new(0x00, 0x124F80, 60000),
            width: 3,
            default: 5,
            unit: Some("min"),
        },
    ),
    Setting::new(
        "dim_timer",
        "Dim timer",
        "Set the IDLE time before the mouse light is dimmed (seconds, 0 = disable)",
        Output,
        &[0x23, 0x0F, 0x01, 0x00, 0x00],
        Value::Range {
            input: Span::new(0, 1200, 1),
            output: Span::new(0x00, 0x124F80, 1000),
            width: 3,
            default: 30,
            unit: Some("s"),
        },
    ),
    Setting::new(
        "rainbow_effect",
        "Rainbow effect",
        "Enable the rainbow effect (can be disabled by setting a color)",
        Output,
        &[0x22, 0xFF],
        Value::Trigger,
    ),
    Setting::new(
        "default_lighting",
        "Default lighting",
        "Set default lighting at mouse startup",
        Output,
        &[0x27],
        Value::Choice {
            entries: AEROX_3_DEFAULT_LIGHTING,
            default: "rainbow",
        },
    ),
];

const KANA_V2_SETTINGS: &[Setting] = &[
    Setting::new(
        "sensitivity1",
        "Sensitivity preset 1",
        "Set sensitivity preset 1 (DPI)",
        Output,
        &[0x03, 0x01],
        Value::Choice {
            entries: KANA_V2_SENSITIVITY1,
            default: "800",
        },
    ),
    Setting::new(
        "sensitivity2",
        "Sensitivity preset 2",
        "Set sensitivity preset 2 (DPI)",
        Output,
        &[0x03, 0x02],
        Value::Choice {
            entries: KANA_V2_SENSITIVITY1,
            default: "1600",
        },
    ),
    Setting::new(
        "polling_rate",
        "Polling rate",
        "Set polling rate (Hz)",
        Output,
        &[0x04, 0x00],
        Value::Choice {
            entries: POLLING_RATE_CODES_4_TO_1,
            default: "1000",
        },
    ),
    Setting::new(
        "led_brightness1",
        "LED Brightness 1",
        "Set the brightness of the LEDs while sensitivity preset 1 is selected",
        Output,
        &[0x05, 0x01],
        Value::Choice {
            entries: KANA_V2_LED_BRIGHTNESS1,
            default: "off",
        },
    ),
    Setting::new(
        "led_brightness2",
        "LED Brightness 2",
        "Set the brightness of the LEDs while sensitivity preset 2 is selected",
        Output,
        &[0x05, 0x02],
        Value::Choice {
            entries: KANA_V2_LED_BRIGHTNESS1,
            default: "high",
        },
    ),
];

const KINZU_V2_SETTINGS: &[Setting] = &[
    Setting::new(
        "sensitivity1",
        "Sensitivity preset 1",
        "Set sensitivity preset 1 (DPI)",
        Output,
        &[0x03, 0x01],
        Value::Choice {
            entries: KINZU_V2_SENSITIVITY1,
            default: "800",
        },
    ),
    Setting::new(
        "sensitivity2",
        "Sensitivity preset 2",
        "Set sensitivity preset 2 (DPI)",
        Output,
        &[0x03, 0x02],
        Value::Choice {
            entries: KINZU_V2_SENSITIVITY1,
            default: "3200",
        },
    ),
    Setting::new(
        "polling_rate",
        "Polling rate",
        "Set polling rate (Hz)",
        Output,
        &[0x04, 0x00],
        Value::Choice {
            entries: POLLING_RATE_CODES_4_TO_1,
            default: "1000",
        },
    ),
];

const PRIME_SETTINGS: &[Setting] = &[
    Setting::new(
        "sensitivity",
        "Sensitivity presets",
        "Set sensitivity presets (DPI)",
        Output,
        &[0x61],
        Value::MultiDpi {
            spec: MultiDpi {
                encoding: DpiEncoding::Range {
                    input: Span::new(50, 18000, 50),
                    output: Span::new(0x01, 0x168, 1),
                },
                width: 2,
                first_preset: 0,
                count_mode: CountMode::Number,
                max_stages: 5,
                xy: None,
            },
            default: &[400, 800, 1200, 2400, 3200],
        },
    ),
    Setting::new(
        "polling_rate",
        "Polling rate",
        "Set polling rate (Hz)",
        Output,
        &[0x5D],
        Value::Choice {
            entries: POLLING_RATE_CODES_4_TO_1,
            default: "1000",
        },
    ),
    Setting::new(
        "color",
        "Wheel LED color",
        "Set the color of the wheel LED",
        Output,
        &[0x62, 0x01],
        Value::Rgb {
            default: Color::new(0xFF, 0x52, 0x00),
        },
    )
    .suffix(&[
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF,
    ])
    .zone("wheel"),
    Setting::new(
        "led_brightness",
        "Wheel LED brightness",
        "Set the brightness of the wheel LED",
        Output,
        &[0x5F],
        Value::Range {
            input: Span::new(0, 256, 1),
            output: Span::new(0x00, 0x100, 1),
            width: 2,
            default: 256,
            unit: None,
        },
    ),
    Setting::new(
        "buttons",
        "Buttons mapping",
        "Set the mapping of the buttons",
        Output,
        &[0x5B],
        Value::Buttons(&PRIME_BUTTONS),
    ),
];

const PRIME_MINI_SETTINGS: &[Setting] = &[
    Setting::new(
        "sensitivity",
        "Sensitivity presets",
        "Set sensitivity presets (DPI)",
        Output,
        &[0x2D],
        Value::MultiDpi {
            spec: MultiDpi {
                encoding: DpiEncoding::Range {
                    input: Span::new(50, 18000, 50),
                    output: Span::new(0x01, 0x168, 1),
                },
                width: 2,
                first_preset: 0,
                count_mode: CountMode::Number,
                max_stages: 5,
                xy: None,
            },
            default: &[400, 800, 1200, 2400, 3200],
        },
    ),
    Setting::new(
        "polling_rate",
        "Polling rate",
        "Set polling rate (Hz)",
        Output,
        &[0x2B],
        Value::Choice {
            entries: POLLING_RATE_CODES_4_TO_1,
            default: "1000",
        },
    ),
    Setting::new(
        "color",
        "LED color",
        "Set the mouse LED color",
        Output,
        &[0x21, 0x00],
        Value::Rgb {
            default: Color::new(0xFF, 0x00, 0x00),
        },
    )
    .zone("led"),
    Setting::new(
        "buttons",
        "Buttons mapping",
        "Set the mapping of the buttons",
        Output,
        &[0x2A],
        Value::Buttons(&AEROX_3_BUTTONS),
    ),
    Setting::new(
        "default_lighting",
        "Default lighting",
        "Set default lighting at mouse startup",
        Output,
        &[0x27],
        Value::Choice {
            entries: PRIME_MINI_DEFAULT_LIGHTING,
            default: "rainbow",
        },
    ),
];

const PRIME_PLUS_SETTINGS: &[Setting] = &[
    Setting::new(
        "sensitivity",
        "Sensitivity presets",
        "Set sensitivity presets (DPI)",
        Output,
        &[0x61],
        Value::MultiDpi {
            spec: MultiDpi {
                encoding: DpiEncoding::Range {
                    input: Span::new(50, 18000, 50),
                    output: Span::new(0x01, 0x168, 1),
                },
                width: 2,
                first_preset: 0,
                count_mode: CountMode::Number,
                max_stages: 5,
                xy: None,
            },
            default: &[400, 800, 1200, 2400, 3200],
        },
    ),
    Setting::new(
        "polling_rate",
        "Polling rate",
        "Set polling rate (Hz)",
        Output,
        &[0x5D],
        Value::Choice {
            entries: POLLING_RATE_CODES_4_TO_1,
            default: "1000",
        },
    ),
    Setting::new(
        "color",
        "Wheel LED color",
        "Set the color of the wheel LED",
        Output,
        &[0x62, 0x01],
        Value::Rgb {
            default: Color::new(0xFF, 0x52, 0x00),
        },
    )
    .suffix(&[
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF,
    ])
    .zone("wheel"),
    Setting::new(
        "led_brightness",
        "Wheel LED brightness",
        "Set the brightness of the wheel LED",
        Output,
        &[0x5F],
        Value::Range {
            input: Span::new(0, 256, 1),
            output: Span::new(0x00, 0x100, 1),
            width: 2,
            default: 256,
            unit: None,
        },
    ),
    Setting::new(
        "buttons",
        "Buttons mapping",
        "Set the mapping of the buttons",
        Output,
        &[0x5B],
        Value::Buttons(&PRIME_BUTTONS),
    ),
];

const PRIME_WIRELESS_WIRED_SETTINGS: &[Setting] = &[
    Setting::new(
        "sensitivity",
        "Sensitivity presets",
        "Set sensitivity presets (DPI)",
        Output,
        &[0x2D],
        Value::MultiDpi {
            spec: MultiDpi {
                encoding: DpiEncoding::Table {
                    input: Span::new(100, 18000, 100),
                    table: dpi_tables::TRUEMOVE_AIR,
                },
                width: 1,
                first_preset: 0,
                count_mode: CountMode::Number,
                max_stages: 5,
                xy: None,
            },
            default: &[400, 800, 1200, 2400, 3200],
        },
    ),
    Setting::new(
        "polling_rate",
        "Polling rate",
        "Set polling rate (Hz)",
        Output,
        &[0x2B],
        Value::Choice {
            entries: POLLING_RATE_CODES_3_TO_0,
            default: "1000",
        },
    ),
    Setting::new(
        "color",
        "LED color",
        "Set the mouse LED color",
        Output,
        &[0x21, 0x01, 0x00],
        Value::Rgb {
            default: Color::new(0xFF, 0x00, 0x00),
        },
    )
    .zone("led"),
    Setting::new(
        "buttons",
        "Buttons mapping",
        "Set the mapping of the buttons",
        Output,
        &[0x2A],
        Value::Buttons(&AEROX_3_BUTTONS),
    ),
    Setting::new(
        "sleep_timer",
        "Sleep timer",
        "Set the IDLE time before the mouse goes to sleep mode (minutes, 0 = disable)",
        Output,
        &[0x29],
        Value::Range {
            input: Span::new(0, 20, 1),
            output: Span::new(0x00, 0x124F80, 60000),
            width: 3,
            default: 5,
            unit: Some("min"),
        },
    ),
    Setting::new(
        "dim_timer",
        "Dim timer",
        "Set the IDLE time before the mouse light is dimmed (seconds, 0 = disable)",
        Output,
        &[0x23, 0x0F, 0x01, 0x00, 0x00],
        Value::Range {
            input: Span::new(0, 1200, 1),
            output: Span::new(0x00, 0x124F80, 1000),
            width: 3,
            default: 30,
            unit: Some("s"),
        },
    ),
    Setting::new(
        "default_lighting",
        "Default lighting",
        "Set default lighting at mouse startup",
        Output,
        &[0x27],
        Value::Choice {
            entries: PRIME_MINI_DEFAULT_LIGHTING,
            default: "rainbow",
        },
    ),
];

const RIVAL_3_SETTINGS: &[Setting] = &[
    Setting::new(
        "sensitivity",
        "Sensitivity presets",
        "Set sensitivity presets (DPI)",
        Output,
        &[0x0B, 0x00],
        Value::MultiDpi {
            spec: MultiDpi {
                encoding: DpiEncoding::Table {
                    input: Span::new(200, 8500, 100),
                    table: dpi_tables::TRUEMOVE_CORE,
                },
                width: 1,
                first_preset: 1,
                count_mode: CountMode::Number,
                max_stages: 5,
                xy: None,
            },
            default: &[800, 1600],
        },
    ),
    Setting::new(
        "polling_rate",
        "Polling rate",
        "Set polling rate (Hz)",
        Output,
        &[0x04, 0x00],
        Value::Choice {
            entries: POLLING_RATE_CODES_4_TO_1,
            default: "1000",
        },
    ),
    Setting::new(
        "z1_color",
        "Strip top LED color",
        "Set the color of the top LED of the strip",
        Output,
        &[0x05, 0x00, 0x01],
        Value::Rgb {
            default: Color::new(0xFF, 0x00, 0x00),
        },
    )
    .suffix(&[0x64])
    .zone("strip_top"),
    Setting::new(
        "z2_color",
        "Strip middle LED color",
        "Set the color of the middle LED of the strip",
        Output,
        &[0x05, 0x00, 0x02],
        Value::Rgb {
            default: Color::new(0x00, 0xFF, 0x00),
        },
    )
    .suffix(&[0x64])
    .zone("strip_middle"),
    Setting::new(
        "z3_color",
        "Strip bottom LED color",
        "Set the color of the bottom LED of the strip",
        Output,
        &[0x05, 0x00, 0x03],
        Value::Rgb {
            default: Color::new(0x00, 0x00, 0xFF),
        },
    )
    .suffix(&[0x64])
    .zone("strip_bottom"),
    Setting::new(
        "logo_color",
        "Logo LED color",
        "Set the color of the logo LED",
        Output,
        &[0x05, 0x00, 0x04],
        Value::Rgb {
            default: Color::new(0x80, 0x00, 0x80),
        },
    )
    .suffix(&[0x64])
    .zone("logo"),
    Setting::new(
        "light_effect",
        "Light effect",
        "Set the light effect",
        Output,
        &[0x06, 0x00],
        Value::Choice {
            entries: RIVAL_3_LIGHT_EFFECT,
            default: "steady",
        },
    ),
    Setting::new(
        "buttons",
        "Buttons mapping",
        "Set the mapping of the buttons",
        Output,
        &[0x07, 0x00],
        Value::Buttons(&RIVAL_3_BUTTONS),
    ),
];

const RIVAL_3_GEN_2_SETTINGS: &[Setting] = &[
    Setting::new(
        "sensitivity",
        "Sensitivity presets",
        "Set sensitivity presets (DPI)",
        Output,
        &[0x34],
        Value::MultiDpi {
            spec: MultiDpi {
                encoding: DpiEncoding::Table {
                    input: Span::new(200, 8500, 100),
                    table: dpi_tables::TRUEMOVE_CORE,
                },
                width: 1,
                first_preset: 1,
                count_mode: CountMode::Number,
                max_stages: 5,
                xy: Some(XyLayout::Interleaved),
            },
            default: &[800, 1600],
        },
    ),
    Setting::new(
        "polling_rate",
        "Polling rate",
        "Set polling rate (Hz)",
        Output,
        &[0x2B],
        Value::Choice {
            entries: POLLING_RATE_CODES_4_TO_1,
            default: "1000",
        },
    ),
    Setting::new(
        "z1_color",
        "Strip top LED color",
        "Set the color of the top LED of the strip",
        Output,
        &[0x21, 0x01],
        Value::Rgb {
            default: Color::new(0xFF, 0x00, 0x00),
        },
    )
    .zone("strip_top"),
    Setting::new(
        "z2_color",
        "Strip middle LED color",
        "Set the color of the middle LED of the strip",
        Output,
        &[0x21, 0x02, 0x00, 0x00, 0x00],
        Value::Rgb {
            default: Color::new(0x00, 0xFF, 0x00),
        },
    )
    .zone("strip_middle"),
    Setting::new(
        "z3_color",
        "Strip bottom LED color",
        "Set the color of the bottom LED of the strip",
        Output,
        &[0x21, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        Value::Rgb {
            default: Color::new(0x00, 0x00, 0xFF),
        },
    )
    .zone("strip_bottom"),
    Setting::new(
        "reactive_color",
        "Reactive color",
        "Set the color of the LEDs in reaction to a button click",
        Output,
        &[0x26],
        Value::Reactive { default: None },
    ),
    Setting::new(
        "led_brightness",
        "LED Brightness",
        "Set the brightness of the LEDs",
        Output,
        &[0x23],
        Value::Range {
            input: Span::new(0, 100, 1),
            output: Span::new(0x00, 0x64, 1),
            width: 1,
            default: 100,
            unit: Some("%"),
        },
    ),
    Setting::new(
        "rainbow_effect",
        "Rainbow effect",
        "Set the rainbow effect (can be cleared by setting a color)",
        Output,
        &[0x22],
        Value::Choice {
            entries: AEROX_3_RAINBOW_EFFECT,
            default: "all",
        },
    ),
    Setting::new(
        "default_lighting",
        "Default lighting",
        "Set default lighting at mouse startup",
        Output,
        &[0x27],
        Value::Choice {
            entries: AEROX_3_DEFAULT_LIGHTING,
            default: "rainbow",
        },
    ),
    Setting::new(
        "buttons",
        "Buttons mapping",
        "Set the mapping of the buttons",
        Output,
        &[0x2A],
        Value::Buttons(&AEROX_3_BUTTONS),
    ),
];

const RIVAL_3_WIRELESS_SETTINGS: &[Setting] = &[
    Setting::new(
        "sensitivity",
        "Sensitivity presets",
        "Set sensitivity presets (DPI)",
        Output,
        &[0x20],
        Value::MultiDpi {
            spec: MultiDpi {
                encoding: DpiEncoding::Table {
                    input: Span::new(100, 18000, 100),
                    table: dpi_tables::TRUEMOVE_AIR,
                },
                width: 2,
                first_preset: 1,
                count_mode: CountMode::Number,
                max_stages: 5,
                xy: None,
            },
            default: &[400, 800, 1200, 2400, 3200],
        },
    ),
    Setting::new(
        "polling_rate",
        "Polling rate",
        "Set polling rate (Hz)",
        Output,
        &[0x17],
        Value::Choice {
            entries: POLLING_RATE_CODES_3_TO_0,
            default: "1000",
        },
    ),
    Setting::new(
        "buttons",
        "Buttons mapping",
        "Set the mapping of the buttons",
        Output,
        &[0x19],
        Value::Buttons(&PRIME_BUTTONS),
    ),
];

const RIVAL_3_WIRELESS_GEN_2_SETTINGS: &[Setting] = &[
    Setting::new(
        "sensitivity",
        "Sensitivity presets",
        "Set sensitivity presets (DPI)",
        Output,
        &[0x2C],
        Value::MultiDpi {
            spec: MultiDpi {
                encoding: DpiEncoding::Table {
                    input: Span::new(100, 18000, 100),
                    table: dpi_tables::TRUEMOVE_AIR,
                },
                width: 1,
                first_preset: 1,
                count_mode: CountMode::Number,
                max_stages: 5,
                xy: Some(XyLayout::Grouped),
            },
            default: &[800, 1600],
        },
    ),
    Setting::new(
        "polling_rate",
        "Polling rate",
        "Set polling rate (Hz)",
        Output,
        &[0x17],
        Value::Choice {
            entries: POLLING_RATE_CODES_3_TO_0,
            default: "1000",
        },
    ),
    Setting::new(
        "buttons",
        "Buttons mapping",
        "Set the mapping of the buttons",
        Output,
        &[0x19],
        Value::Buttons(&PRIME_BUTTONS),
    ),
];

const RIVAL_5_SETTINGS: &[Setting] = &[
    Setting::new(
        "sensitivity",
        "Sensitivity presets",
        "Set sensitivity presets (DPI)",
        Output,
        &[0x2D],
        Value::MultiDpi {
            spec: MultiDpi {
                encoding: DpiEncoding::Table {
                    input: Span::new(100, 18000, 100),
                    table: dpi_tables::TRUEMOVE_AIR,
                },
                width: 1,
                first_preset: 0,
                count_mode: CountMode::Number,
                max_stages: 5,
                xy: None,
            },
            default: &[400, 800, 1200, 2400, 3200],
        },
    ),
    Setting::new(
        "polling_rate",
        "Polling rate",
        "Set polling rate (Hz)",
        Output,
        &[0x2B],
        Value::Choice {
            entries: POLLING_RATE_CODES_4_TO_1,
            default: "1000",
        },
    ),
    Setting::new(
        "wheel_color",
        "Wheel LED color",
        "Set the color of the wheel LED",
        Output,
        &[0x21, 0x01, 0x00],
        Value::Rgb {
            default: Color::new(0xFF, 0x18, 0x00),
        },
    )
    .zone("wheel"),
    Setting::new(
        "z2_color",
        "Left strip, top LED color",
        "Set the color of the top LED of the left strip",
        Output,
        &[0x21, 0x02, 0x00, 0x00, 0x00, 0x00],
        Value::Rgb {
            default: Color::new(0xFF, 0x18, 0x00),
        },
    )
    .zone("left_strip_top"),
    Setting::new(
        "z3_color",
        "Right strip, top LED color",
        "Set the color of the top LED of the right strip",
        Output,
        &[0x21, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        Value::Rgb {
            default: Color::new(0xFF, 0x18, 0x00),
        },
    )
    .zone("right_strip_top"),
    Setting::new(
        "z4_color",
        "Left strip, middle top LED color",
        "Set the color of the middle top LED of the left strip",
        Output,
        &[0x21, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        Value::Rgb {
            default: Color::new(0xFF, 0x18, 0x00),
        },
    )
    .zone("left_strip_middle_top"),
    Setting::new(
        "z5_color",
        "Right strip, middle top LED color",
        "Set the color of the middle top LED of the right strip",
        Output,
        &[
            0x21, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ],
        Value::Rgb {
            default: Color::new(0xFF, 0x18, 0x00),
        },
    )
    .zone("right_strip_middle_top"),
    Setting::new(
        "z6_color",
        "Left strip, middle bottom LED color",
        "Set the color of the middle bottom LED of the left strip",
        Output,
        &[
            0x21, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ],
        Value::Rgb {
            default: Color::new(0xFF, 0x18, 0x00),
        },
    )
    .zone("left_strip_middle_bottom"),
    Setting::new(
        "z7_color",
        "Right strip, middle bottom LED color",
        "Set the color of the middle bottom LED of the right strip",
        Output,
        &[
            0x21, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00,
        ],
        Value::Rgb {
            default: Color::new(0xFF, 0x18, 0x00),
        },
    )
    .zone("right_strip_middle_bottom"),
    Setting::new(
        "z8_color",
        "Left strip, bottom LED color",
        "Set the color of the bottom LED of the left strip",
        Output,
        &[
            0x21, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ],
        Value::Rgb {
            default: Color::new(0xFF, 0x18, 0x00),
        },
    )
    .zone("left_strip_bottom"),
    Setting::new(
        "z9_color",
        "Right strip, bottom LED color",
        "Set the color of the bottom LED of the right strip",
        Output,
        &[
            0x21, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ],
        Value::Rgb {
            default: Color::new(0xFF, 0x18, 0x00),
        },
    )
    .zone("right_strip_bottom"),
    Setting::new(
        "logo_color",
        "Logo LED color",
        "Set the color of the logo LED",
        Output,
        &[
            0x21, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ],
        Value::Rgb {
            default: Color::new(0xFF, 0x18, 0x00),
        },
    )
    .zone("logo"),
    Setting::new(
        "reactive_color",
        "Reactive color",
        "Set the color of the LEDs in reaction to a button click",
        Output,
        &[0x26],
        Value::Reactive {
            default: Some(Color::new(0xFF, 0xFF, 0xFF)),
        },
    ),
    Setting::new(
        "led_brightness",
        "LED Brightness",
        "Set the brightness of the LEDs",
        Output,
        &[0x23],
        Value::Choice {
            entries: RIVAL_5_LED_BRIGHTNESS,
            default: "100",
        },
    ),
    Setting::new(
        "buttons",
        "Buttons mapping",
        "Set the mapping of the buttons",
        Output,
        &[0x2A],
        Value::Buttons(&RIVAL_5_BUTTONS),
    ),
    Setting::new(
        "rainbow_effect",
        "Rainbow effect",
        "Enable the rainbow effect (can be disabled by setting a color)",
        Output,
        &[0x22, 0xFF, 0x03],
        Value::Trigger,
    ),
    Setting::new(
        "default_lighting",
        "Default lighting",
        "Set default lighting at mouse startup, Reactive-color must be toggled separately!",
        Output,
        &[0x27],
        Value::Choice {
            entries: PRIME_MINI_DEFAULT_LIGHTING,
            default: "rainbow",
        },
    ),
];

const RIVAL_95_SETTINGS: &[Setting] = &[
    Setting::new(
        "sensitivity1",
        "Sensitivity preset 1",
        "Set sensitivity preset 1 (DPI)",
        Output,
        &[0x03, 0x01],
        Value::Choice {
            entries: RIVAL_95_SENSITIVITY1,
            default: "1000",
        },
    ),
    Setting::new(
        "sensitivity2",
        "Sensitivity preset 2",
        "Set sensitivity preset 2 (DPI)",
        Output,
        &[0x03, 0x02],
        Value::Choice {
            entries: RIVAL_95_SENSITIVITY1,
            default: "2000",
        },
    ),
    Setting::new(
        "polling_rate",
        "Polling rate",
        "Set polling rate (Hz)",
        Output,
        &[0x04, 0x00],
        Value::Choice {
            entries: POLLING_RATE_CODES_4_TO_1,
            default: "1000",
        },
    ),
    Setting::new(
        "btn6_mode",
        "Button 6 mode",
        "Set the mode of the button under the wheel",
        Output,
        &[0x0B],
        Value::Choice {
            entries: RIVAL_95_BTN6_MODE,
            default: "dpi",
        },
    ),
];

const RIVAL_100_SETTINGS: &[Setting] = &[
    Setting::new(
        "sensitivity1",
        "Sensitivity preset 1",
        "Set sensitivity preset 1 (DPI)",
        Output,
        &[0x03, 0x01],
        Value::Choice {
            entries: RIVAL_95_SENSITIVITY1,
            default: "1000",
        },
    ),
    Setting::new(
        "sensitivity2",
        "Sensitivity preset 2",
        "Set sensitivity preset 2 (DPI)",
        Output,
        &[0x03, 0x02],
        Value::Choice {
            entries: RIVAL_95_SENSITIVITY1,
            default: "2000",
        },
    ),
    Setting::new(
        "polling_rate",
        "Polling rate",
        "Set polling rate (Hz)",
        Output,
        &[0x04, 0x00],
        Value::Choice {
            entries: POLLING_RATE_CODES_4_TO_1,
            default: "1000",
        },
    ),
    Setting::new(
        "color",
        "LED color",
        "Set the mouse LED color",
        Output,
        &[0x05, 0x00],
        Value::Rgb {
            default: Color::new(0xFF, 0x18, 0x00),
        },
    )
    .zone("led"),
    Setting::new(
        "light_effect",
        "Light effect",
        "Set the light effect",
        Output,
        &[0x07, 0x00],
        Value::Choice {
            entries: RIVAL_100_LIGHT_EFFECT,
            default: "steady",
        },
    ),
    Setting::new(
        "btn6_mode",
        "Button 6 mode",
        "Set the mode of the button under the wheel",
        Output,
        &[0x0B],
        Value::Choice {
            entries: RIVAL_95_BTN6_MODE,
            default: "dpi",
        },
    ),
];

const RIVAL_110_SETTINGS: &[Setting] = &[
    Setting::new(
        "sensitivity1",
        "Sensitivity preset 1",
        "Set sensitivity preset 1 (DPI)",
        Output,
        &[0x03, 0x01],
        Value::RangeChoice {
            input: Span::new(200, 7200, 100),
            table: dpi_tables::PMW3326,
            width: 1,
            default: 800,
            unit: Some("DPI"),
        },
    )
    .packet_length(32)
    .dpi_preset(),
    Setting::new(
        "sensitivity2",
        "Sensitivity preset 2",
        "Set sensitivity preset 2 (DPI)",
        Output,
        &[0x03, 0x02],
        Value::RangeChoice {
            input: Span::new(200, 7200, 100),
            table: dpi_tables::PMW3326,
            width: 1,
            default: 1600,
            unit: Some("DPI"),
        },
    )
    .packet_length(32)
    .dpi_preset(),
    Setting::new(
        "polling_rate",
        "Polling rate",
        "Set polling rate (Hz)",
        Output,
        &[0x04, 0x00],
        Value::Choice {
            entries: POLLING_RATE_CODES_4_TO_1,
            default: "1000",
        },
    )
    .packet_length(32),
    Setting::new(
        "color",
        "LED color",
        "Set the mouse LED color",
        Output,
        &[0x05, 0x00],
        Value::Rgb {
            default: Color::new(0xFF, 0x18, 0x00),
        },
    )
    .packet_length(32)
    .zone("led"),
    Setting::new(
        "light_effect",
        "Light effect",
        "Set the light effect",
        Output,
        &[0x07, 0x00],
        Value::Choice {
            entries: RIVAL_100_LIGHT_EFFECT,
            default: "steady",
        },
    )
    .packet_length(32),
    Setting::new(
        "btn6_mode",
        "Button 6 mode",
        "Set the mode of the button under the wheel",
        Output,
        &[0x0B],
        Value::Choice {
            entries: RIVAL_95_BTN6_MODE,
            default: "dpi",
        },
    )
    .packet_length(32),
];

const RIVAL_300_SETTINGS: &[Setting] = &[
    Setting::new(
        "sensitivity1",
        "Sensitivity preset 1",
        "Set sensitivity preset 1 (DPI)",
        Output,
        &[0x03, 0x01],
        Value::Range {
            input: Span::new(50, 6500, 50),
            output: Span::new(0x01, 0x82, 1),
            width: 1,
            default: 800,
            unit: Some("DPI"),
        },
    )
    .dpi_preset(),
    Setting::new(
        "sensitivity2",
        "Sensitivity preset 2",
        "Set sensitivity preset 2 (DPI)",
        Output,
        &[0x03, 0x02],
        Value::Range {
            input: Span::new(50, 6500, 50),
            output: Span::new(0x01, 0x82, 1),
            width: 1,
            default: 1600,
            unit: Some("DPI"),
        },
    )
    .dpi_preset(),
    Setting::new(
        "polling_rate",
        "Polling rate",
        "Set polling rate (Hz)",
        Output,
        &[0x04, 0x00],
        Value::Choice {
            entries: POLLING_RATE_CODES_4_TO_1,
            default: "1000",
        },
    ),
    Setting::new(
        "logo_color",
        "Logo LED color",
        "Set the color of the logo LED",
        Output,
        &[0x08, 0x01],
        Value::Rgb {
            default: Color::new(0xFF, 0x18, 0x00),
        },
    )
    .zone("logo"),
    Setting::new(
        "wheel_color",
        "Wheel LED color",
        "Set the color of the wheel LED",
        Output,
        &[0x08, 0x02],
        Value::Rgb {
            default: Color::new(0xFF, 0x18, 0x00),
        },
    )
    .zone("wheel"),
    Setting::new(
        "logo_light_effect",
        "Logo light effect",
        "Set the light effect of the logo",
        Output,
        &[0x07, 0x01],
        Value::Choice {
            entries: RIVAL_100_LIGHT_EFFECT,
            default: "steady",
        },
    ),
    Setting::new(
        "wheel_light_effect",
        "Wheel light effect",
        "Set the light effect of the wheel",
        Output,
        &[0x07, 0x02],
        Value::Choice {
            entries: RIVAL_100_LIGHT_EFFECT,
            default: "steady",
        },
    ),
    Setting::new(
        "buttons",
        "Buttons mapping",
        "Set the mapping of the buttons",
        Output,
        &[0x31, 0x00],
        Value::Buttons(&PRIME_BUTTONS),
    ),
];

const RIVAL_310_SETTINGS: &[Setting] = &[
    Setting::new(
        "sensitivity1",
        "Sensitivity preset 1",
        "Set sensitivity preset 1 (DPI)",
        Output,
        &[0x53, 0x00, 0x01],
        Value::Range {
            input: Span::new(100, 12000, 100),
            output: Span::new(0x00, 0x77, 1),
            width: 1,
            default: 800,
            unit: Some("DPI"),
        },
    )
    .suffix(&[0x00, 0x42])
    .dpi_preset(),
    Setting::new(
        "sensitivity2",
        "Sensitivity preset 2",
        "Set sensitivity preset 2 (DPI)",
        Output,
        &[0x53, 0x00, 0x02],
        Value::Range {
            input: Span::new(100, 12000, 100),
            output: Span::new(0x00, 0x77, 1),
            width: 1,
            default: 1600,
            unit: Some("DPI"),
        },
    )
    .suffix(&[0x00, 0x42])
    .dpi_preset(),
    Setting::new(
        "polling_rate",
        "Polling rate",
        "Set polling rate (Hz)",
        Output,
        &[0x54, 0x00],
        Value::Choice {
            entries: POLLING_RATE_CODES_4_TO_1,
            default: "1000",
        },
    ),
    Setting::new(
        "logo_color",
        "Logo LED color",
        "Set the color of the logo LED",
        Feature,
        &[0x5B, 0x00],
        Value::Gradient {
            layout: RIVAL_310_GRADIENT,
            led_id: 0x00,
            default: EffectDefault {
                duration_ms: 1000,
                colors: [
                    Color::new(0xFF, 0x00, 0x00),
                    Color::new(0x00, 0xFF, 0x00),
                    Color::new(0x00, 0x00, 0xFF),
                ],
            },
        },
    )
    .zone("logo"),
    Setting::new(
        "wheel_color",
        "Wheel LED color",
        "Set the color of the wheel LED",
        Feature,
        &[0x5B, 0x00],
        Value::Gradient {
            layout: RIVAL_310_GRADIENT,
            led_id: 0x01,
            default: EffectDefault {
                duration_ms: 1000,
                colors: [
                    Color::new(0xFF, 0x00, 0x00),
                    Color::new(0x00, 0xFF, 0x00),
                    Color::new(0x00, 0x00, 0xFF),
                ],
            },
        },
    )
    .zone("wheel"),
    Setting::new(
        "buttons",
        "Buttons mapping",
        "Set the mapping of the buttons",
        Output,
        &[0x31, 0x00],
        Value::Buttons(&PRIME_BUTTONS),
    ),
];

const RIVAL_500_SETTINGS: &[Setting] = &[
    Setting::new(
        "sensitivity1",
        "Sensitivity preset 1",
        "Set sensitivity preset 1 (DPI)",
        Output,
        &[0x03, 0x00, 0x01],
        Value::Range {
            input: Span::new(100, 12000, 100),
            output: Span::new(0x00, 0x77, 1),
            width: 1,
            default: 800,
            unit: Some("DPI"),
        },
    )
    .suffix(&[0x00, 0x42])
    .dpi_preset(),
    Setting::new(
        "sensitivity2",
        "Sensitivity preset 2",
        "Set sensitivity preset 2 (DPI)",
        Output,
        &[0x03, 0x00, 0x02],
        Value::Range {
            input: Span::new(100, 12000, 100),
            output: Span::new(0x00, 0x77, 1),
            width: 1,
            default: 1600,
            unit: Some("DPI"),
        },
    )
    .suffix(&[0x00, 0x42])
    .dpi_preset(),
    Setting::new(
        "polling_rate",
        "Polling rate",
        "Set polling rate (Hz)",
        Output,
        &[0x04, 0x00],
        Value::Choice {
            entries: POLLING_RATE_CODES_4_TO_1,
            default: "1000",
        },
    ),
    Setting::new(
        "logo_color",
        "Logo LED color",
        "Set the color of the logo LED",
        Feature,
        &[0x05, 0x00],
        Value::GradientV2 {
            layout: RIVAL_500_GRADIENT_V2,
            led_id: 0x00,
            default: EffectDefault {
                duration_ms: 1000,
                colors: [
                    Color::new(0xFF, 0x00, 0xE1),
                    Color::new(0xFF, 0xEA, 0x00),
                    Color::new(0x00, 0xCC, 0xFF),
                ],
            },
        },
    )
    .zone("logo"),
    Setting::new(
        "wheel_color",
        "Wheel LED color",
        "Set the color of the wheel LED",
        Feature,
        &[0x05, 0x00],
        Value::GradientV2 {
            layout: RIVAL_500_GRADIENT_V2,
            led_id: 0x01,
            default: EffectDefault {
                duration_ms: 1000,
                colors: [
                    Color::new(0xFF, 0x00, 0xE1),
                    Color::new(0xFF, 0xEA, 0x00),
                    Color::new(0x00, 0xCC, 0xFF),
                ],
            },
        },
    )
    .zone("wheel"),
    Setting::new(
        "buttons",
        "Buttons mapping",
        "Set the mapping of the buttons",
        Feature,
        &[0x31, 0x00],
        Value::Buttons(&RIVAL_500_BUTTONS),
    ),
];

const RIVAL_600_SETTINGS: &[Setting] = &[
    Setting::new(
        "sensitivity1",
        "Sensitivity preset 1",
        "Set sensitivity preset 1 (DPI)",
        Output,
        &[0x03, 0x00, 0x01],
        Value::Range {
            input: Span::new(100, 12000, 100),
            output: Span::new(0x00, 0x77, 1),
            width: 1,
            default: 800,
            unit: Some("DPI"),
        },
    )
    .suffix(&[0x00, 0x42])
    .dpi_preset(),
    Setting::new(
        "sensitivity2",
        "Sensitivity preset 2",
        "Set sensitivity preset 2 (DPI)",
        Output,
        &[0x03, 0x00, 0x02],
        Value::Range {
            input: Span::new(100, 12000, 100),
            output: Span::new(0x00, 0x77, 1),
            width: 1,
            default: 1600,
            unit: Some("DPI"),
        },
    )
    .suffix(&[0x00, 0x42])
    .dpi_preset(),
    Setting::new(
        "polling_rate",
        "Polling rate",
        "Set polling rate (Hz)",
        Output,
        &[0x04, 0x00],
        Value::Choice {
            entries: POLLING_RATE_CODES_4_TO_1,
            default: "1000",
        },
    ),
    Setting::new(
        "wheel_color",
        "Wheel LED color",
        "Set the color of the wheel LED",
        Feature,
        &[0x05, 0x00],
        Value::Gradient {
            layout: RIVAL_600_GRADIENT,
            led_id: 0x00,
            default: EffectDefault {
                duration_ms: 1000,
                colors: [
                    Color::new(0xFF, 0x00, 0x00),
                    Color::new(0x00, 0xFF, 0x00),
                    Color::new(0x00, 0x00, 0xFF),
                ],
            },
        },
    )
    .zone("wheel"),
    Setting::new(
        "logo_color",
        "Logo LED color",
        "Set the color of the logo LED",
        Feature,
        &[0x05, 0x00],
        Value::Gradient {
            layout: RIVAL_600_GRADIENT,
            led_id: 0x01,
            default: EffectDefault {
                duration_ms: 1000,
                colors: [
                    Color::new(0xFF, 0x00, 0x00),
                    Color::new(0x00, 0xFF, 0x00),
                    Color::new(0x00, 0x00, 0xFF),
                ],
            },
        },
    )
    .zone("logo"),
    Setting::new(
        "z2_color",
        "Left strip, top LED color",
        "Set the color of the top LED of the left strip",
        Feature,
        &[0x05, 0x00],
        Value::Gradient {
            layout: RIVAL_600_GRADIENT,
            led_id: 0x02,
            default: EffectDefault {
                duration_ms: 1000,
                colors: [
                    Color::new(0xFF, 0x00, 0x00),
                    Color::new(0x00, 0xFF, 0x00),
                    Color::new(0x00, 0x00, 0xFF),
                ],
            },
        },
    )
    .zone("left_strip_top"),
    Setting::new(
        "z3_color",
        "Right strip, top LED color",
        "Set the color of the top LED of the right strip",
        Feature,
        &[0x05, 0x00],
        Value::Gradient {
            layout: RIVAL_600_GRADIENT,
            led_id: 0x03,
            default: EffectDefault {
                duration_ms: 1000,
                colors: [
                    Color::new(0xFF, 0x00, 0x00),
                    Color::new(0x00, 0xFF, 0x00),
                    Color::new(0x00, 0x00, 0xFF),
                ],
            },
        },
    )
    .zone("right_strip_top"),
    Setting::new(
        "z4_color",
        "Left strip, middle LED color",
        "Set the color of the middle LED of the left strip",
        Feature,
        &[0x05, 0x00],
        Value::Gradient {
            layout: RIVAL_600_GRADIENT,
            led_id: 0x04,
            default: EffectDefault {
                duration_ms: 1000,
                colors: [
                    Color::new(0xFF, 0x00, 0x00),
                    Color::new(0x00, 0xFF, 0x00),
                    Color::new(0x00, 0x00, 0xFF),
                ],
            },
        },
    )
    .zone("left_strip_middle"),
    Setting::new(
        "z5_color",
        "Right strip, middle LED color",
        "Set the color of the middle LED of the right strip",
        Feature,
        &[0x05, 0x00],
        Value::Gradient {
            layout: RIVAL_600_GRADIENT,
            led_id: 0x05,
            default: EffectDefault {
                duration_ms: 1000,
                colors: [
                    Color::new(0xFF, 0x00, 0x00),
                    Color::new(0x00, 0xFF, 0x00),
                    Color::new(0x00, 0x00, 0xFF),
                ],
            },
        },
    )
    .zone("right_strip_middle"),
    Setting::new(
        "z6_color",
        "Left strip, bottom LED color",
        "Set the color of the bottom LED of the left strip",
        Feature,
        &[0x05, 0x00],
        Value::Gradient {
            layout: RIVAL_600_GRADIENT,
            led_id: 0x06,
            default: EffectDefault {
                duration_ms: 1000,
                colors: [
                    Color::new(0xFF, 0x00, 0x00),
                    Color::new(0x00, 0xFF, 0x00),
                    Color::new(0x00, 0x00, 0xFF),
                ],
            },
        },
    )
    .zone("left_strip_bottom"),
    Setting::new(
        "z7_color",
        "Right strip, bottom LED color",
        "Set the color of the bottom LED of the right strip",
        Feature,
        &[0x05, 0x00],
        Value::Gradient {
            layout: RIVAL_600_GRADIENT,
            led_id: 0x07,
            default: EffectDefault {
                duration_ms: 1000,
                colors: [
                    Color::new(0xFF, 0x00, 0x00),
                    Color::new(0x00, 0xFF, 0x00),
                    Color::new(0x00, 0x00, 0xFF),
                ],
            },
        },
    )
    .zone("right_strip_bottom"),
    Setting::new(
        "buttons",
        "Buttons mapping",
        "Set the mapping of the buttons",
        Feature,
        &[0x31, 0x00],
        Value::Buttons(&RIVAL_600_BUTTONS),
    ),
];

const RIVAL_650_SETTINGS: &[Setting] = &[
    Setting::new(
        "sensitivity1",
        "Sensitivity preset 1",
        "Set sensitivity preset 1 (DPI)",
        Output,
        &[0x15, 0x01],
        Value::Range {
            input: Span::new(100, 12000, 100),
            output: Span::new(0x00, 0x77, 1),
            width: 1,
            default: 800,
            unit: Some("DPI"),
        },
    )
    .dpi_preset(),
    Setting::new(
        "sensitivity2",
        "Sensitivity preset 2",
        "Set sensitivity preset 2 (DPI)",
        Output,
        &[0x15, 0x02],
        Value::Range {
            input: Span::new(100, 12000, 100),
            output: Span::new(0x00, 0x77, 1),
            width: 1,
            default: 1600,
            unit: Some("DPI"),
        },
    )
    .dpi_preset(),
    Setting::new(
        "polling_rate",
        "Polling rate",
        "Set the polling rate (Hz)",
        Output,
        &[0x17],
        Value::Choice {
            entries: POLLING_RATE_CODES_4_TO_1,
            default: "1000",
        },
    ),
    Setting::new(
        "lift_off_distance",
        "Lift-off distance",
        "Set the lift-off distance",
        Output,
        &[0x20, 0x01],
        Value::RangeChoice {
            input: Span::new(1, 8, 1),
            table: RIVAL_650_LIFT_OFF_DISTANCE,
            width: 2,
            default: 8,
            unit: None,
        },
    ),
    Setting::new(
        "buttons",
        "Buttons mapping",
        "Set the mapping of the buttons",
        Output,
        &[0x19],
        Value::Buttons(&RIVAL_600_BUTTONS),
    ),
    Setting::new(
        "sleep_timer",
        "Sleep timer",
        "Set the IDLE time before the mouse goes to sleep mode (minutes)",
        Output,
        &[0x2B, 0x01, 0x01, 0x00, 0x00, 0x00],
        Value::Range {
            input: Span::new(1, 20, 1),
            output: Span::new(0x3C, 0x4B0, 60),
            width: 2,
            default: 5,
            unit: Some("min"),
        },
    ),
];

const RIVAL_700_SETTINGS: &[Setting] = &[
    Setting::new(
        "sensitivity1",
        "Sensitivity preset 1",
        "Set sensitivity preset 1 (DPI)",
        Output,
        &[0x03, 0x00, 0x01],
        Value::Range {
            input: Span::new(100, 12000, 100),
            output: Span::new(0x00, 0x77, 1),
            width: 1,
            default: 800,
            unit: Some("DPI"),
        },
    )
    .suffix(&[0x00, 0x42])
    .dpi_preset(),
    Setting::new(
        "sensitivity2",
        "Sensitivity preset 2",
        "Set sensitivity preset 2 (DPI)",
        Output,
        &[0x03, 0x00, 0x02],
        Value::Range {
            input: Span::new(100, 12000, 100),
            output: Span::new(0x00, 0x77, 1),
            width: 1,
            default: 1600,
            unit: Some("DPI"),
        },
    )
    .suffix(&[0x00, 0x42])
    .dpi_preset(),
    Setting::new(
        "polling_rate",
        "Polling rate",
        "Set polling rate (Hz)",
        Output,
        &[0x04, 0x00],
        Value::Choice {
            entries: POLLING_RATE_CODES_4_TO_1,
            default: "1000",
        },
    ),
    Setting::new(
        "logo_color",
        "Logo LED color",
        "Set the color of the logo LED",
        Feature,
        &[0x05, 0x00],
        Value::GradientV2 {
            layout: RIVAL_500_GRADIENT_V2,
            led_id: 0x00,
            default: EffectDefault {
                duration_ms: 1000,
                colors: [
                    Color::new(0xFF, 0x00, 0xE1),
                    Color::new(0xFF, 0xEA, 0x00),
                    Color::new(0x00, 0xCC, 0xFF),
                ],
            },
        },
    )
    .zone("logo"),
    Setting::new(
        "wheel_color",
        "Wheel LED color",
        "Set the color of the wheel LED",
        Feature,
        &[0x05, 0x00],
        Value::GradientV2 {
            layout: RIVAL_500_GRADIENT_V2,
            led_id: 0x01,
            default: EffectDefault {
                duration_ms: 1000,
                colors: [
                    Color::new(0xFF, 0x00, 0xE1),
                    Color::new(0xFF, 0xEA, 0x00),
                    Color::new(0x00, 0xCC, 0xFF),
                ],
            },
        },
    )
    .zone("wheel"),
];

const SENSEI_310_SETTINGS: &[Setting] = &[
    Setting::new(
        "sensitivity1",
        "Sensitivity preset 1",
        "Set sensitivity preset 1 (DPI)",
        Output,
        &[0x53, 0x00, 0x01],
        Value::Range {
            input: Span::new(100, 12000, 100),
            output: Span::new(0x00, 0x77, 1),
            width: 1,
            default: 800,
            unit: Some("DPI"),
        },
    )
    .suffix(&[0x00, 0x42])
    .dpi_preset(),
    Setting::new(
        "sensitivity2",
        "Sensitivity preset 2",
        "Set sensitivity preset 2 (DPI)",
        Output,
        &[0x53, 0x00, 0x02],
        Value::Range {
            input: Span::new(100, 12000, 100),
            output: Span::new(0x00, 0x77, 1),
            width: 1,
            default: 1600,
            unit: Some("DPI"),
        },
    )
    .suffix(&[0x00, 0x42])
    .dpi_preset(),
    Setting::new(
        "polling_rate",
        "Polling rate",
        "Set polling rate (Hz)",
        Output,
        &[0x54, 0x00],
        Value::Choice {
            entries: POLLING_RATE_CODES_4_TO_1,
            default: "1000",
        },
    ),
    Setting::new(
        "logo_color",
        "Logo LED color",
        "Set the color of the logo LED",
        Feature,
        &[0x5B, 0x00],
        Value::Gradient {
            layout: RIVAL_310_GRADIENT,
            led_id: 0x00,
            default: EffectDefault {
                duration_ms: 1000,
                colors: [
                    Color::new(0xFF, 0x00, 0x00),
                    Color::new(0x00, 0xFF, 0x00),
                    Color::new(0x00, 0x00, 0xFF),
                ],
            },
        },
    )
    .zone("logo"),
    Setting::new(
        "wheel_color",
        "Wheel LED color",
        "Set the color of the wheel LED",
        Feature,
        &[0x5B, 0x00],
        Value::Gradient {
            layout: RIVAL_310_GRADIENT,
            led_id: 0x01,
            default: EffectDefault {
                duration_ms: 1000,
                colors: [
                    Color::new(0xFF, 0x00, 0x00),
                    Color::new(0x00, 0xFF, 0x00),
                    Color::new(0x00, 0x00, 0xFF),
                ],
            },
        },
    )
    .zone("wheel"),
    Setting::new(
        "buttons",
        "Buttons mapping",
        "Set the mapping of the buttons",
        Output,
        &[0x31, 0x00],
        Value::Buttons(&SENSEI_310_BUTTONS),
    ),
];

const SENSEI_RAW_SETTINGS: &[Setting] = &[
    Setting::new(
        "sensitivity1",
        "Sensitivity preset 1",
        "Set sensitivity preset 1 (DPI)",
        Output,
        &[0x03, 0x01],
        Value::Range {
            input: Span::new(90, 5670, 90),
            output: Span::new(0x01, 0x3F, 1),
            width: 1,
            default: 1620,
            unit: Some("DPI"),
        },
    )
    .dpi_preset(),
    Setting::new(
        "sensitivity2",
        "Sensitivity preset 2",
        "Set sensitivity preset 2 (DPI)",
        Output,
        &[0x03, 0x02],
        Value::Range {
            input: Span::new(90, 5670, 90),
            output: Span::new(0x01, 0x3F, 1),
            width: 1,
            default: 3240,
            unit: Some("DPI"),
        },
    )
    .dpi_preset(),
    Setting::new(
        "polling_rate",
        "Polling rate",
        "Set polling rate (Hz)",
        Output,
        &[0x04, 0x00],
        Value::Choice {
            entries: POLLING_RATE_CODES_4_TO_1,
            default: "1000",
        },
    ),
    Setting::new(
        "led_brightness",
        "LED Brightness",
        "Set the brightness of the LEDs",
        Output,
        &[0x05, 0x01],
        Value::Choice {
            entries: KANA_V2_LED_BRIGHTNESS1,
            default: "off",
        },
    ),
    Setting::new(
        "light_effect",
        "Light effect",
        "Set the light effect",
        Output,
        &[0x07, 0x01],
        Value::Choice {
            entries: SENSEI_RAW_LIGHT_EFFECT,
            default: "breath",
        },
    ),
    Setting::new(
        "buttons",
        "Buttons mapping",
        "Set the mapping of the buttons",
        Output,
        &[0x31, 0x00],
        Value::Buttons(&SENSEI_RAW_BUTTONS),
    ),
];

const SENSEI_TEN_SETTINGS: &[Setting] = &[
    Setting::new(
        "sensitivity",
        "Sensitivity presets",
        "Set sensitivity presets (DPI)",
        Output,
        &[0x55, 0x00],
        Value::MultiDpi {
            spec: MultiDpi {
                encoding: DpiEncoding::Range {
                    input: Span::new(50, 18000, 50),
                    output: Span::new(0x01, 0x168, 1),
                },
                width: 2,
                first_preset: 1,
                count_mode: CountMode::Flag,
                max_stages: 5,
                xy: None,
            },
            default: &[400, 800, 1200, 2400, 3200],
        },
    ),
    Setting::new(
        "polling_rate",
        "Polling rate",
        "Set polling rate (Hz)",
        Output,
        &[0x54, 0x00],
        Value::Choice {
            entries: POLLING_RATE_CODES_4_TO_1,
            default: "1000",
        },
    ),
    Setting::new(
        "logo_color",
        "Logo LED color",
        "Set the color of the logo LED",
        Feature,
        &[0x5B, 0x00],
        Value::Gradient {
            layout: RIVAL_310_GRADIENT,
            led_id: 0x00,
            default: EffectDefault {
                duration_ms: 10000,
                colors: [
                    Color::new(0xFF, 0x00, 0x00),
                    Color::new(0x00, 0xFF, 0x00),
                    Color::new(0x00, 0x00, 0xFF),
                ],
            },
        },
    )
    .zone("logo"),
    Setting::new(
        "wheel_color",
        "Wheel LED color",
        "Set the color of the wheel LED",
        Feature,
        &[0x5B, 0x00],
        Value::Gradient {
            layout: RIVAL_310_GRADIENT,
            led_id: 0x01,
            default: EffectDefault {
                duration_ms: 10000,
                colors: [
                    Color::new(0xFF, 0x00, 0x00),
                    Color::new(0x00, 0xFF, 0x00),
                    Color::new(0x00, 0x00, 0xFF),
                ],
            },
        },
    )
    .zone("wheel"),
    Setting::new(
        "buttons",
        "Buttons mapping",
        "Set the mapping of the buttons",
        Output,
        &[0x31, 0x00],
        Value::Buttons(&SENSEI_310_BUTTONS),
    ),
];

/// Aerox 3, from `rivalcfg/devices/aerox3.py`.
pub static AEROX_3: Profile = Profile {
    name: "Aerox 3",
    source: "rivalcfg/devices/aerox3.py",
    test_source: "test/devices/old_specs/test_aerox3.py",
    settings: AEROX_3_SETTINGS,
    wireless: false,
    save: Some(Command::output(&[0x11, 0x00])),
    battery: None,
    firmware: None,
};

/// Aerox 3 Wireless, from `rivalcfg/devices/aerox3_wireless_wired.py`.
pub static AEROX_3_WIRELESS_WIRED: Profile = Profile {
    name: "Aerox 3 Wireless",
    source: "rivalcfg/devices/aerox3_wireless_wired.py",
    test_source: "test/devices/specs/aerox3_wireless_wired.txt",
    settings: AEROX_3_WIRELESS_WIRED_SETTINGS,
    wireless: false,
    save: Some(Command::output(&[0x11, 0x00])),
    battery: Some(BatteryQuery {
        command: Command::output(&[0x92]),
        response_len: 2,
        format: BatteryFormat::FlaggedSteps,
    }),
    firmware: None,
};

/// Aerox 3 Wireless, from `rivalcfg/devices/aerox3_wireless_wireless.py`.
pub static AEROX_3_WIRELESS_WIRELESS: Profile = Profile {
    name: "Aerox 3 Wireless",
    source: "rivalcfg/devices/aerox3_wireless_wireless.py",
    test_source: "test/devices/specs/aerox3_wireless_wireless.txt",
    settings: AEROX_3_WIRELESS_WIRED_SETTINGS,
    wireless: true,
    save: Some(Command::output(&[0x11, 0x00])),
    battery: Some(BatteryQuery {
        command: Command::output(&[0x92]),
        response_len: 2,
        format: BatteryFormat::FlaggedSteps,
    }),
    firmware: None,
};

/// Aerox 5, from `rivalcfg/devices/aerox5.py`.
pub static AEROX_5: Profile = Profile {
    name: "Aerox 5",
    source: "rivalcfg/devices/aerox5.py",
    test_source: "test/devices/old_specs/test_aerox5.py",
    settings: AEROX_5_SETTINGS,
    wireless: false,
    save: Some(Command::output(&[0x11, 0x00])),
    battery: None,
    firmware: None,
};

/// Aerox 5 Wireless, from `rivalcfg/devices/aerox5_wireless_wired.py`.
pub static AEROX_5_WIRELESS_WIRED: Profile = Profile {
    name: "Aerox 5 Wireless",
    source: "rivalcfg/devices/aerox5_wireless_wired.py",
    test_source: "test/devices/old_specs/test_aerox5_wireless_wired.py",
    settings: AEROX_5_WIRELESS_WIRED_SETTINGS,
    wireless: false,
    save: Some(Command::output(&[0x11, 0x00])),
    battery: Some(BatteryQuery {
        command: Command::output(&[0x92]),
        response_len: 2,
        format: BatteryFormat::FlaggedSteps,
    }),
    firmware: None,
};

/// Aerox 5 Wireless, from `rivalcfg/devices/aerox5_wireless_wireless.py`.
pub static AEROX_5_WIRELESS_WIRELESS: Profile = Profile {
    name: "Aerox 5 Wireless",
    source: "rivalcfg/devices/aerox5_wireless_wireless.py",
    test_source: "test/devices/old_specs/test_aerox5_wireless_wireless.py",
    settings: AEROX_5_WIRELESS_WIRED_SETTINGS,
    wireless: true,
    save: Some(Command::output(&[0x11, 0x00])),
    battery: Some(BatteryQuery {
        command: Command::output(&[0x92]),
        response_len: 2,
        format: BatteryFormat::FlaggedSteps,
    }),
    firmware: None,
};

/// Aerox 9 Wireless, from `rivalcfg/devices/aerox9_wireless_wired.py`.
pub static AEROX_9_WIRELESS_WIRED: Profile = Profile {
    name: "Aerox 9 Wireless",
    source: "rivalcfg/devices/aerox9_wireless_wired.py",
    test_source: "test/devices/old_specs/test_aerox9_wireless_wired.py",
    settings: AEROX_9_WIRELESS_WIRED_SETTINGS,
    wireless: false,
    save: Some(Command::output(&[0x11, 0x00])),
    battery: Some(BatteryQuery {
        command: Command::output(&[0x92]),
        response_len: 2,
        format: BatteryFormat::FlaggedSteps,
    }),
    firmware: None,
};

/// Aerox 9 Wireless, from `rivalcfg/devices/aerox9_wireless_wireless.py`.
pub static AEROX_9_WIRELESS_WIRELESS: Profile = Profile {
    name: "Aerox 9 Wireless",
    source: "rivalcfg/devices/aerox9_wireless_wireless.py",
    test_source: "test/devices/old_specs/test_aerox9_wireless_wireless.py",
    settings: AEROX_9_WIRELESS_WIRED_SETTINGS,
    wireless: true,
    save: Some(Command::output(&[0x11, 0x00])),
    battery: Some(BatteryQuery {
        command: Command::output(&[0x92]),
        response_len: 2,
        format: BatteryFormat::FlaggedSteps,
    }),
    firmware: None,
};

/// Kana v2, from `rivalcfg/devices/kanav2.py`.
pub static KANA_V2: Profile = Profile {
    name: "Kana v2",
    source: "rivalcfg/devices/kanav2.py",
    test_source: "test/devices/old_specs/test_kanav2.py",
    settings: KANA_V2_SETTINGS,
    wireless: false,
    save: Some(Command::output(&[0x09, 0x00])),
    battery: None,
    firmware: None,
};

/// Kinzu v2, from `rivalcfg/devices/kinzuv2.py`.
pub static KINZU_V2: Profile = Profile {
    name: "Kinzu v2",
    source: "rivalcfg/devices/kinzuv2.py",
    test_source: "test/devices/old_specs/test_kinzuv2.py",
    settings: KINZU_V2_SETTINGS,
    wireless: false,
    save: Some(Command::output(&[0x09, 0x00])),
    battery: None,
    firmware: None,
};

/// Prime, from `rivalcfg/devices/prime.py`.
pub static PRIME: Profile = Profile {
    name: "Prime",
    source: "rivalcfg/devices/prime.py",
    test_source: "test/devices/old_specs/test_prime.py",
    settings: PRIME_SETTINGS,
    wireless: false,
    save: Some(Command::output(&[0x59])),
    battery: None,
    firmware: None,
};

/// Prime Mini, from `rivalcfg/devices/prime_mini.py`.
pub static PRIME_MINI: Profile = Profile {
    name: "Prime Mini",
    source: "rivalcfg/devices/prime_mini.py",
    test_source: "test/devices/old_specs/test_prime_mini.py",
    settings: PRIME_MINI_SETTINGS,
    wireless: false,
    save: Some(Command::output(&[0x11, 0x00])),
    battery: None,
    firmware: None,
};

/// Prime+, from `rivalcfg/devices/prime_plus.py`.
pub static PRIME_PLUS: Profile = Profile {
    name: "Prime+",
    source: "rivalcfg/devices/prime_plus.py",
    test_source: "test/devices/old_specs/test_prime_plus.py",
    settings: PRIME_PLUS_SETTINGS,
    wireless: false,
    save: Some(Command::output(&[0x59])),
    battery: None,
    firmware: None,
};

/// Prime Wireless, from `rivalcfg/devices/prime_wireless_wired.py`.
pub static PRIME_WIRELESS_WIRED: Profile = Profile {
    name: "Prime Wireless",
    source: "rivalcfg/devices/prime_wireless_wired.py",
    test_source: "test/devices/old_specs/test_prime_wireless_wired.py",
    settings: PRIME_WIRELESS_WIRED_SETTINGS,
    wireless: false,
    save: Some(Command::output(&[0x11, 0x00])),
    battery: Some(BatteryQuery {
        command: Command::output(&[0x92]),
        response_len: 2,
        format: BatteryFormat::FlaggedSteps,
    }),
    firmware: None,
};

/// Prime Wireless, from `rivalcfg/devices/prime_wireless_wireless.py`.
pub static PRIME_WIRELESS_WIRELESS: Profile = Profile {
    name: "Prime Wireless",
    source: "rivalcfg/devices/prime_wireless_wireless.py",
    test_source: "test/devices/old_specs/test_prime_wireless_wireless.py",
    settings: PRIME_WIRELESS_WIRED_SETTINGS,
    wireless: true,
    save: Some(Command::output(&[0x11, 0x00])),
    battery: Some(BatteryQuery {
        command: Command::output(&[0x92]),
        response_len: 2,
        format: BatteryFormat::FlaggedSteps,
    }),
    firmware: None,
};

/// Rival 3, from `rivalcfg/devices/rival3.py`.
pub static RIVAL_3: Profile = Profile {
    name: "Rival 3",
    source: "rivalcfg/devices/rival3.py",
    test_source: "test/devices/old_specs/test_rival3.py",
    settings: RIVAL_3_SETTINGS,
    wireless: false,
    save: Some(Command::output(&[0x09, 0x00])),
    battery: None,
    firmware: Some(FirmwareQuery {
        command: Command::output(&[0x10, 0x00]),
        response_len: 2,
    }),
};

/// Rival 3 Gen 2, from `rivalcfg/devices/rival3_gen2.py`.
pub static RIVAL_3_GEN_2: Profile = Profile {
    name: "Rival 3 Gen 2",
    source: "rivalcfg/devices/rival3_gen2.py",
    test_source: "test/devices/old_specs/test_rival3_gen2.py",
    settings: RIVAL_3_GEN_2_SETTINGS,
    wireless: false,
    save: Some(Command::output(&[0x11, 0x00])),
    battery: None,
    firmware: None,
};

/// Rival 3 Wireless, from `rivalcfg/devices/rival3_wireless.py`.
pub static RIVAL_3_WIRELESS: Profile = Profile {
    name: "Rival 3 Wireless",
    source: "rivalcfg/devices/rival3_wireless.py",
    test_source: "test/devices/old_specs/test_rival3_wireless.py",
    settings: RIVAL_3_WIRELESS_SETTINGS,
    wireless: false,
    save: Some(Command::output(&[0x09])),
    battery: Some(BatteryQuery {
        command: Command::output(&[0xAA, 0x01]),
        response_len: 3,
        format: BatteryFormat::LevelThenCharging,
    }),
    firmware: Some(FirmwareQuery {
        command: Command::output(&[0x90, 0x00]),
        response_len: 2,
    }),
};

/// Rival 3 Wireless Gen 2, from `rivalcfg/devices/rival3_wireless_gen2.py`.
pub static RIVAL_3_WIRELESS_GEN_2: Profile = Profile {
    name: "Rival 3 Wireless Gen 2",
    source: "rivalcfg/devices/rival3_wireless_gen2.py",
    test_source: "test/devices/old_specs/test_rival3_wireless_gen2.py",
    settings: RIVAL_3_WIRELESS_GEN_2_SETTINGS,
    wireless: false,
    save: Some(Command::output(&[0x09])),
    battery: Some(BatteryQuery {
        command: Command::output(&[0xAA, 0x01]),
        response_len: 3,
        format: BatteryFormat::LevelThenCharging,
    }),
    firmware: None,
};

/// Rival 5, from `rivalcfg/devices/rival5.py`.
pub static RIVAL_5: Profile = Profile {
    name: "Rival 5",
    source: "rivalcfg/devices/rival5.py",
    test_source: "test/devices/old_specs/test_rival5.py",
    settings: RIVAL_5_SETTINGS,
    wireless: false,
    save: Some(Command::output(&[0x11, 0x00])),
    battery: None,
    firmware: None,
};

/// Rival 95 / Rival 100 PC Bang, from `rivalcfg/devices/rival95.py`.
pub static RIVAL_95: Profile = Profile {
    name: "Rival 95 / Rival 100 PC Bang",
    source: "rivalcfg/devices/rival95.py",
    test_source: "test/devices/old_specs/test_rival95.py",
    settings: RIVAL_95_SETTINGS,
    wireless: false,
    save: Some(Command::output(&[0x09, 0x00])),
    battery: None,
    firmware: None,
};

/// Rival 100 / Rival 105, from `rivalcfg/devices/rival100.py`.
pub static RIVAL_100: Profile = Profile {
    name: "Rival 100 / Rival 105",
    source: "rivalcfg/devices/rival100.py",
    test_source: "test/devices/specs/rival100.txt",
    settings: RIVAL_100_SETTINGS,
    wireless: false,
    save: Some(Command::output(&[0x09, 0x00])),
    battery: None,
    firmware: Some(FirmwareQuery {
        command: Command::output(&[0x10, 0x00]),
        response_len: 2,
    }),
};

/// Rival 110 / Rival 106, from `rivalcfg/devices/rival110.py`.
pub static RIVAL_110: Profile = Profile {
    name: "Rival 110 / Rival 106",
    source: "rivalcfg/devices/rival110.py",
    test_source: "test/devices/old_specs/test_rival110.py",
    settings: RIVAL_110_SETTINGS,
    wireless: false,
    save: Some(Command::output(&[0x09, 0x00]).packet_length(32)),
    battery: None,
    firmware: None,
};

/// Rival 300 / Rival, from `rivalcfg/devices/rival300.py`.
pub static RIVAL_300: Profile = Profile {
    name: "Rival 300 / Rival",
    source: "rivalcfg/devices/rival300.py",
    test_source: "test/devices/old_specs/test_rival300.py",
    settings: RIVAL_300_SETTINGS,
    wireless: false,
    save: Some(Command::output(&[0x09, 0x00])),
    battery: None,
    firmware: Some(FirmwareQuery {
        command: Command::output(&[0x10, 0x00]),
        response_len: 2,
    }),
};

/// Rival 300S, from `rivalcfg/devices/rival300s.py`.
pub static RIVAL_300S: Profile = Profile {
    name: "Rival 300S",
    source: "rivalcfg/devices/rival300s.py",
    test_source: "test/devices/old_specs/test_rival300s.py",
    settings: RIVAL_110_SETTINGS,
    wireless: false,
    save: Some(Command::output(&[0x09, 0x00]).packet_length(32)),
    battery: None,
    firmware: None,
};

/// Rival 310, from `rivalcfg/devices/rival310.py`.
pub static RIVAL_310: Profile = Profile {
    name: "Rival 310",
    source: "rivalcfg/devices/rival310.py",
    test_source: "test/devices/old_specs/test_rival310.py",
    settings: RIVAL_310_SETTINGS,
    wireless: false,
    save: Some(Command::output(&[0x59, 0x00])),
    battery: None,
    firmware: Some(FirmwareQuery {
        command: Command::output(&[0x90, 0x00]),
        response_len: 2,
    }),
};

/// Rival 500, from `rivalcfg/devices/rival500.py`.
pub static RIVAL_500: Profile = Profile {
    name: "Rival 500",
    source: "rivalcfg/devices/rival500.py",
    test_source: "test/devices/old_specs/test_rival500.py",
    settings: RIVAL_500_SETTINGS,
    wireless: false,
    save: Some(Command::output(&[0x09, 0x00])),
    battery: None,
    firmware: Some(FirmwareQuery {
        command: Command::output(&[0x10, 0x00]),
        response_len: 2,
    }),
};

/// Rival 600, from `rivalcfg/devices/rival600.py`.
pub static RIVAL_600: Profile = Profile {
    name: "Rival 600",
    source: "rivalcfg/devices/rival600.py",
    test_source: "test/devices/old_specs/test_rival600.py",
    settings: RIVAL_600_SETTINGS,
    wireless: false,
    save: Some(Command::output(&[0x09, 0x00])),
    battery: None,
    firmware: None,
};

/// Rival 650 Wireless, from `rivalcfg/devices/rival650.py`.
pub static RIVAL_650: Profile = Profile {
    name: "Rival 650 Wireless",
    source: "rivalcfg/devices/rival650.py",
    test_source: "test/devices/specs/rival650.txt",
    settings: RIVAL_650_SETTINGS,
    wireless: false,
    save: Some(Command::output(&[0x09])),
    battery: Some(BatteryQuery {
        command: Command::output(&[0xAA, 0x01]),
        response_len: 3,
        format: BatteryFormat::LevelThenCharging,
    }),
    firmware: None,
};

/// Rival 700 / Rival 710, from `rivalcfg/devices/rival700.py`.
pub static RIVAL_700: Profile = Profile {
    name: "Rival 700 / Rival 710",
    source: "rivalcfg/devices/rival700.py",
    test_source: "test/devices/old_specs/test_rival700.py",
    settings: RIVAL_700_SETTINGS,
    wireless: false,
    save: Some(Command::output(&[0x09, 0x00])),
    battery: None,
    firmware: Some(FirmwareQuery {
        command: Command::output(&[0x10, 0x00]),
        response_len: 2,
    }),
};

/// Sensei 310, from `rivalcfg/devices/sensei310.py`.
pub static SENSEI_310: Profile = Profile {
    name: "Sensei 310",
    source: "rivalcfg/devices/sensei310.py",
    test_source: "test/devices/old_specs/test_sensei310.py",
    settings: SENSEI_310_SETTINGS,
    wireless: false,
    save: Some(Command::output(&[0x59, 0x00])),
    battery: None,
    firmware: Some(FirmwareQuery {
        command: Command::output(&[0x90, 0x00]),
        response_len: 2,
    }),
};

/// Sensei [RAW], from `rivalcfg/devices/sensei_raw.py`.
pub static SENSEI_RAW: Profile = Profile {
    name: "Sensei [RAW]",
    source: "rivalcfg/devices/sensei_raw.py",
    test_source: "test/devices/old_specs/test_sensei_raw.py",
    settings: SENSEI_RAW_SETTINGS,
    wireless: false,
    save: Some(Command::output(&[0x09, 0x00])),
    battery: None,
    firmware: None,
};

/// Sensei TEN, from `rivalcfg/devices/sensei_ten.py`.
pub static SENSEI_TEN: Profile = Profile {
    name: "Sensei TEN",
    source: "rivalcfg/devices/sensei_ten.py",
    test_source: "test/devices/old_specs/test_sensei_ten.py",
    settings: SENSEI_TEN_SETTINGS,
    wireless: false,
    save: Some(Command::output(&[0x59, 0x00])),
    battery: None,
    firmware: Some(FirmwareQuery {
        command: Command::output(&[0x90, 0x00]),
        response_len: 2,
    }),
};
