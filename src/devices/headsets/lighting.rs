//! Headset lighting encoders for the Arctis 5 and the Arctis Nova 3.
//!
//! [EXPERIMENTAL] Byte layouts are taken from OpenRGB (`SteelSeriesArctis5Controller`,
//! `SteelSeriesArctisNova3Controller`, GPL-2.0, used as a source of facts only). Not tested on
//! hardware by this project. Bytes OpenRGB writes without explaining them are kept verbatim as
//! named constants.

use super::report::{Command, Frame, Report};
use crate::rgb::Color;

/// Zone names exposed to users, in this order, for every lit headset.
pub const ZONES: [&str; 2] = ["left", "right"];

/// Which headset's lighting protocol a model speaks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LightingStyle {
    /// Arctis 5: one static color per ear cup.
    Arctis5,
    /// Arctis Nova 3: per-ear effects (static, breathe, color shift) with a speed.
    Nova3,
}

/// Effects the Arctis Nova 3 offers (OpenRGB mode list).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Nova3Effect {
    Off,
    Static,
    Breathe,
    RainbowShift,
    HeatOrangeShift,
    FrostBlueShift,
}

impl Nova3Effect {
    /// Every effect with its stable id and label, in display order.
    pub const ALL: [(Self, &'static str, &'static str); 6] = [
        (Self::Off, "off", "Off"),
        (Self::Static, "static", "Static color"),
        (Self::Breathe, "breathe", "Breathe"),
        (Self::RainbowShift, "rainbow_shift", "Color shift - rainbow"),
        (Self::HeatOrangeShift, "heat_orange_shift", "Color shift - heat orange"),
        (Self::FrostBlueShift, "frost_blue_shift", "Color shift - frost blue"),
    ];

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.iter().find(|(_, i, _)| *i == id).map(|(e, _, _)| *e)
    }
}

/// Slowest and fastest Nova 3 effect speed (OpenRGB `speed_min` / `speed_max`).
pub const NOVA3_SPEED_MIN: u8 = 1;
pub const NOVA3_SPEED_MAX: u8 = 10;

/// What the daemon last asked the lights to show. Lighting commands always carry the whole
/// state, so changing one part re-sends the rest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LightingState {
    pub left: Color,
    pub right: Color,
    pub effect: Nova3Effect,
    pub speed: u8,
}

impl Default for LightingState {
    fn default() -> Self {
        Self {
            left: Color::WHITE,
            right: Color::WHITE,
            effect: Nova3Effect::Static,
            // OpenRGB's default speed for the color-shift modes.
            speed: 6,
        }
    }
}

/// Build every report needed to show `state` on a headset of `style`.
pub fn render(style: LightingStyle, state: &LightingState) -> Vec<Report> {
    match style {
        // OpenRGB zone 0 is the left cup on the Arctis 5.
        LightingStyle::Arctis5 => {
            let mut reports = arctis5_zone(ARCTIS5_ZONE_LEFT, state.left);
            reports.extend(arctis5_zone(ARCTIS5_ZONE_RIGHT, state.right));
            reports
        }
        // OpenRGB LED 0 is the right cup on the Nova 3, and it updates LED 0 first.
        LightingStyle::Nova3 => {
            let mut reports = nova3_zone(NOVA3_ZONE_RIGHT, state.effect, state.right, state.speed);
            reports.extend(nova3_zone(NOVA3_ZONE_LEFT, state.effect, state.left, state.speed));
            reports
        }
    }
}

// ---- Arctis 5 -------------------------------------------------------------------------------

const ARCTIS5_ZONE_LEFT: u8 = 0;
const ARCTIS5_ZONE_RIGHT: u8 = 1;

const ARCTIS5_PREPARE: Command = Command::new(Frame::ARCTIS_5_LIGHTING, 0x06, 0x81);
const ARCTIS5_PREPARE_PREFIX: [u8; 2] = [0x43, 0x01];
const ARCTIS5_PREPARE_STEPS: [u8; 2] = [0x22, 0x23];
const ARCTIS5_LED: Command = Command::new(Frame::ARCTIS_5_LIGHTING, 0x06, 0x8a);
const ARCTIS5_LED_PREFIX: [u8; 3] = [0x42, 0x00, 0x20];
const ARCTIS5_OP_COLOR: u8 = 0x41;
const ARCTIS5_OP_APPLY_ZONE: u8 = 0x60;
const ARCTIS5_OP_COMMIT: u8 = 0x05;
const ARCTIS5_SELECT_ZONE: u8 = 0x08;
const ARCTIS5_COLOR_TRAILER: [u8; 4] = [0xff, 0x32, 0xc8, 0xc8];

/// The six reports OpenRGB sends to set one Arctis 5 zone to `color`.
pub fn arctis5_zone(zone: u8, color: Color) -> Vec<Report> {
    let led = |body: &[u8]| {
        let mut payload = ARCTIS5_LED_PREFIX.to_vec();
        payload.extend_from_slice(body);
        ARCTIS5_LED.report(&payload)
    };
    let mut reports: Vec<Report> = ARCTIS5_PREPARE_STEPS
        .iter()
        .map(|step| {
            let mut payload = ARCTIS5_PREPARE_PREFIX.to_vec();
            payload.push(*step);
            ARCTIS5_PREPARE.report(&payload)
        })
        .collect();
    let mut color_body = vec![ARCTIS5_OP_COLOR, 0x00, color.r, color.g, color.b];
    color_body.extend_from_slice(&ARCTIS5_COLOR_TRAILER);
    reports.push(led(&color_body));
    reports.push(led(&[ARCTIS5_OP_COLOR, ARCTIS5_SELECT_ZONE, zone, 0x01]));
    reports.push(led(&[ARCTIS5_OP_APPLY_ZONE, zone]));
    reports.push(led(&[ARCTIS5_OP_COMMIT]));
    reports
}

// ---- Arctis Nova 3 --------------------------------------------------------------------------

const NOVA3_ZONE_RIGHT: u8 = 0;
const NOVA3_ZONE_LEFT: u8 = 1;

const NOVA3_EFFECT: Command = Command::new(Frame::NOVA_3_EFFECT, 0x06, 0xaa);
const NOVA3_SELECT: Command = Command::new(Frame::NOVA, 0x06, 0xa5);
const NOVA3_APPLY_RIGHT: Command = Command::new(Frame::NOVA, 0x06, 0x09);
const NOVA3_APPLY_LEFT: Command = Command::new(Frame::NOVA, 0x06, 0xa3);
const NOVA3_ZONE_ON: u8 = 0x0a;
const NOVA3_ZONE_OFF: u8 = 0x00;

// Offsets inside the 521-byte effect packet.
const NOVA3_AT_SPEED: usize = 0x0d;
const NOVA3_AT_MARKER_A: usize = 0x15;
const NOVA3_AT_MARKER_B: usize = 0x17;
const NOVA3_AT_STATIC_FLAG: usize = 0x1d;
const NOVA3_AT_MODE: usize = 0x25;
const NOVA3_AT_STOPS: usize = 0x26;
const NOVA3_AT_BREATHE_FIRST: usize = 0x29;
const NOVA3_AT_STATIC_COLOR: usize = 0x2d;
const NOVA3_AT_BREATHE_SECOND: usize = 0x31;

const NOVA3_MARKER_A: u8 = 0x55;
const NOVA3_MARKER_B: u8 = 0x96;
const NOVA3_MODE_STATIC: u8 = 0x02;
const NOVA3_MODE_GRADIENT: u8 = 0x03;
const NOVA3_MODE_RAINBOW: u8 = 0x07;
const NOVA3_STATIC_SPEED: [u8; 2] = [0x98, 0x3a];
const NOVA3_FULL: u8 = 0xff;
const NOVA3_HALF: u8 = 0x7f;

const NOVA3_RAINBOW_STOPS: [u8; 31] = [
    0xff, 0x00, 0x00, 0xff, 0x00, 0x00, 0x00, 0xff, 0xff, 0x00, 0x2b, 0x00, 0xff, 0x00, 0x2b, 0x00, 0xff, 0xff, 0x2b,
    0x00, 0x00, 0xff, 0x2b, 0xff, 0x00, 0xff, 0x2b, 0xff, 0x00, 0x00, 0x26,
];
const NOVA3_ORANGE_STOPS: [u8; 15] = [
    0xff, 0xea, 0x00, 0xff, 0xea, 0x00, 0x00, 0xff, 0x4d, 0x00, 0x7f, 0xff, 0xea, 0x00, 0x7f,
];
const NOVA3_BLUE_STOPS: [u8; 15] = [
    0x8c, 0x00, 0xff, 0x8c, 0x00, 0xff, 0x00, 0x3b, 0xd1, 0xff, 0x7f, 0x8c, 0x00, 0xff, 0x7f,
];

/// Speed bytes for speeds 1..=10; anything else falls back to speed 9, as OpenRGB does.
pub fn nova3_speed_bytes(speed: u8) -> [u8; 2] {
    match speed {
        1 => [0x30, 0x75],
        2 => [0x2e, 0x68],
        3 => [0x2c, 0x5b],
        4 => [0x20, 0x4e],
        5 => [0x1e, 0x41],
        6 => [0x1c, 0x34],
        7 => [0x1a, 0x27],
        8 => [0x0e, 0x1a],
        10 => [0x94, 0x02],
        _ => [0x0c, 0x0d],
    }
}

fn nova3_effect_base(zone: u8) -> Report {
    let zone_byte = if zone == NOVA3_ZONE_RIGHT { 0x01 } else { 0x00 };
    NOVA3_EFFECT
        .report(&[zone_byte])
        .with(NOVA3_AT_MARKER_A, &[NOVA3_MARKER_A])
        .with(NOVA3_AT_MARKER_B, &[NOVA3_MARKER_B])
}

fn nova3_select(zone: u8, state: u8) -> Report {
    let zone_byte = if zone == NOVA3_ZONE_RIGHT { 0x02 } else { 0x01 };
    NOVA3_SELECT.report(&[zone_byte, state])
}

fn nova3_apply(zone: u8) -> Report {
    if zone == NOVA3_ZONE_RIGHT {
        NOVA3_APPLY_RIGHT.report(&[])
    } else {
        NOVA3_APPLY_LEFT.report(&[])
    }
}

/// The reports OpenRGB sends to show `effect` on one Nova 3 zone.
pub fn nova3_zone(zone: u8, effect: Nova3Effect, color: Color, speed: u8) -> Vec<Report> {
    let rgb = [color.r, color.g, color.b];
    let packet = match effect {
        Nova3Effect::Off => return vec![nova3_select(zone, NOVA3_ZONE_OFF), nova3_apply(zone)],
        Nova3Effect::Static => nova3_effect_base(zone)
            .with(NOVA3_AT_SPEED, &NOVA3_STATIC_SPEED)
            .with(NOVA3_AT_STATIC_FLAG, &[0x01])
            .with(NOVA3_AT_MODE, &[NOVA3_MODE_STATIC])
            .with(NOVA3_AT_STATIC_COLOR, &rgb)
            .with(NOVA3_AT_STATIC_COLOR + 3, &[NOVA3_FULL]),
        Nova3Effect::Breathe => nova3_effect_base(zone)
            .with(NOVA3_AT_SPEED, &nova3_speed_bytes(speed))
            .with(NOVA3_AT_MODE, &[NOVA3_MODE_GRADIENT])
            .with(NOVA3_AT_BREATHE_FIRST, &rgb)
            .with(NOVA3_AT_BREATHE_FIRST + 7, &[NOVA3_HALF])
            .with(NOVA3_AT_BREATHE_SECOND, &rgb)
            .with(NOVA3_AT_BREATHE_SECOND + 3, &[NOVA3_HALF]),
        Nova3Effect::RainbowShift => nova3_effect_base(zone)
            .with(NOVA3_AT_SPEED, &nova3_speed_bytes(speed))
            .with(NOVA3_AT_MODE, &[NOVA3_MODE_RAINBOW])
            .with(NOVA3_AT_STOPS, &NOVA3_RAINBOW_STOPS),
        Nova3Effect::HeatOrangeShift => nova3_effect_base(zone)
            .with(NOVA3_AT_SPEED, &nova3_speed_bytes(speed))
            .with(NOVA3_AT_MODE, &[NOVA3_MODE_GRADIENT])
            .with(NOVA3_AT_STOPS, &NOVA3_ORANGE_STOPS),
        Nova3Effect::FrostBlueShift => nova3_effect_base(zone)
            .with(NOVA3_AT_SPEED, &nova3_speed_bytes(speed))
            .with(NOVA3_AT_MODE, &[NOVA3_MODE_GRADIENT])
            .with(NOVA3_AT_STOPS, &NOVA3_BLUE_STOPS),
    };
    vec![packet, nova3_select(zone, NOVA3_ZONE_ON), nova3_apply(zone)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::headsets::report::ReportKind;

    #[test]
    fn arctis5_zone_matches_openrgb_sequence() {
        let reports = arctis5_zone(1, Color::new(0x11, 0x22, 0x33));
        assert_eq!(reports.len(), 6);
        assert!(
            reports
                .iter()
                .all(|r| r.bytes.len() == 37 && r.kind == ReportKind::Output)
        );
        assert_eq!(&reports[0].bytes[..5], &[0x06, 0x81, 0x43, 0x01, 0x22]);
        assert_eq!(&reports[1].bytes[..5], &[0x06, 0x81, 0x43, 0x01, 0x23]);
        assert_eq!(
            &reports[2].bytes[..14],
            &[
                0x06, 0x8a, 0x42, 0x00, 0x20, 0x41, 0x00, 0x11, 0x22, 0x33, 0xff, 0x32, 0xc8, 0xc8
            ]
        );
        assert_eq!(
            &reports[3].bytes[..9],
            &[0x06, 0x8a, 0x42, 0x00, 0x20, 0x41, 0x08, 0x01, 0x01]
        );
        assert_eq!(&reports[4].bytes[..7], &[0x06, 0x8a, 0x42, 0x00, 0x20, 0x60, 0x01]);
        assert_eq!(&reports[5].bytes[..6], &[0x06, 0x8a, 0x42, 0x00, 0x20, 0x05]);
        assert!(reports[5].bytes[6..].iter().all(|b| *b == 0));
    }

    #[test]
    fn arctis5_render_sends_left_then_right() {
        let state = LightingState {
            left: Color::RED,
            right: Color::BLUE,
            ..LightingState::default()
        };
        let reports = render(LightingStyle::Arctis5, &state);
        assert_eq!(reports.len(), 12);
        assert_eq!(&reports[2].bytes[7..10], &[0xff, 0x00, 0x00]);
        assert_eq!(reports[3].bytes[7], 0);
        assert_eq!(&reports[8].bytes[7..10], &[0x00, 0x00, 0xff]);
        assert_eq!(reports[9].bytes[7], 1);
    }

    #[test]
    fn nova3_static_matches_openrgb_layout() {
        let reports = nova3_zone(NOVA3_ZONE_RIGHT, Nova3Effect::Static, Color::new(1, 2, 3), 6);
        assert_eq!(reports.len(), 3);
        let effect = &reports[0].bytes;
        assert_eq!(effect.len(), 521);
        assert_eq!(&effect[..3], &[0x06, 0xaa, 0x01]);
        assert_eq!(&effect[0x0d..0x0f], &[0x98, 0x3a]);
        assert_eq!(effect[0x15], 0x55);
        assert_eq!(effect[0x17], 0x96);
        assert_eq!(effect[0x1d], 0x01);
        assert_eq!(effect[0x25], 0x02);
        assert_eq!(&effect[0x2d..0x31], &[1, 2, 3, 0xff]);
        assert_eq!(&reports[1].bytes[..4], &[0x06, 0xa5, 0x02, 0x0a]);
        assert_eq!(reports[1].bytes.len(), 64);
        assert_eq!(&reports[2].bytes[..2], &[0x06, 0x09]);
    }

    #[test]
    fn nova3_left_zone_uses_left_selectors() {
        let reports = nova3_zone(NOVA3_ZONE_LEFT, Nova3Effect::Static, Color::WHITE, 6);
        assert_eq!(reports[0].bytes[2], 0x00);
        assert_eq!(&reports[1].bytes[..4], &[0x06, 0xa5, 0x01, 0x0a]);
        assert_eq!(&reports[2].bytes[..2], &[0x06, 0xa3]);
    }

    #[test]
    fn nova3_breathe_places_both_color_copies() {
        let reports = nova3_zone(NOVA3_ZONE_RIGHT, Nova3Effect::Breathe, Color::new(9, 8, 7), 1);
        let effect = &reports[0].bytes;
        assert_eq!(&effect[0x0d..0x0f], &[0x30, 0x75]);
        assert_eq!(effect[0x25], 0x03);
        assert_eq!(&effect[0x29..0x2c], &[9, 8, 7]);
        assert_eq!(effect[0x30], 0x7f);
        assert_eq!(&effect[0x31..0x34], &[9, 8, 7]);
        assert_eq!(effect[0x34], 0x7f);
    }

    #[test]
    fn nova3_color_shifts_carry_their_stop_tables() {
        let rainbow = &nova3_zone(NOVA3_ZONE_RIGHT, Nova3Effect::RainbowShift, Color::BLACK, 10)[0].bytes;
        assert_eq!(&rainbow[0x0d..0x0f], &[0x94, 0x02]);
        assert_eq!(rainbow[0x25], 0x07);
        assert_eq!(&rainbow[0x26..0x26 + 31], &NOVA3_RAINBOW_STOPS);
        let orange = &nova3_zone(NOVA3_ZONE_RIGHT, Nova3Effect::HeatOrangeShift, Color::BLACK, 6)[0].bytes;
        assert_eq!(orange[0x25], 0x03);
        assert_eq!(&orange[0x26..0x26 + 15], &NOVA3_ORANGE_STOPS);
        let blue = &nova3_zone(NOVA3_ZONE_RIGHT, Nova3Effect::FrostBlueShift, Color::BLACK, 6)[0].bytes;
        assert_eq!(&blue[0x26..0x26 + 15], &NOVA3_BLUE_STOPS);
    }

    #[test]
    fn nova3_off_only_deselects_and_applies() {
        let reports = nova3_zone(NOVA3_ZONE_LEFT, Nova3Effect::Off, Color::WHITE, 6);
        assert_eq!(reports.len(), 2);
        assert_eq!(&reports[0].bytes[..4], &[0x06, 0xa5, 0x01, 0x00]);
        assert_eq!(&reports[1].bytes[..2], &[0x06, 0xa3]);
    }

    #[test]
    fn nova3_render_sends_right_cup_first() {
        let state = LightingState {
            left: Color::GREEN,
            right: Color::RED,
            ..LightingState::default()
        };
        let reports = render(LightingStyle::Nova3, &state);
        assert_eq!(reports.len(), 6);
        assert_eq!(reports[0].bytes[2], 0x01);
        assert_eq!(&reports[0].bytes[0x2d..0x30], &[0xff, 0x00, 0x00]);
        assert_eq!(reports[3].bytes[2], 0x00);
        assert_eq!(&reports[3].bytes[0x2d..0x30], &[0x00, 0xff, 0x00]);
    }

    #[test]
    fn nova3_speed_falls_back_like_openrgb() {
        assert_eq!(nova3_speed_bytes(9), [0x0c, 0x0d]);
        assert_eq!(nova3_speed_bytes(0), [0x0c, 0x0d]);
        assert_eq!(nova3_speed_bytes(5), [0x1e, 0x41]);
    }

    #[test]
    fn effect_ids_round_trip() {
        for (effect, id, _) in Nova3Effect::ALL {
            assert_eq!(Nova3Effect::from_id(id), Some(effect));
        }
        assert_eq!(Nova3Effect::from_id("disco"), None);
    }
}
