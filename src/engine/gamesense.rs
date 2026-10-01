//! Turns GameSense outputs (what a game asked for) into engine actions on real devices:
//! lighting overlays, per-key colours, OLED screens.

use std::time::{Duration, Instant};

use crate::devices::DeviceType;
use crate::devices::key_mapping::KeyId;
use crate::gamesense::{self, GameSenseOutput, LightTarget, ScreenLine};
use crate::rgb::Color;

use super::screen::ScreenContent;
use super::{Overlay, OverlayTarget, OverlayZone};

/// GameSense lighting has no duration: it stays until the game replaces or clears it. The
/// heartbeat watchdog clears a game that stops talking, so this is only a safety net.
const HOLD: Duration = Duration::from_secs(3600);

/// What the engine should do for one output.
pub enum Action {
    Overlays(Vec<Overlay>),
    Screen {
        game: String,
        content: ScreenContent,
        duration: Option<Duration>,
    },
    Clear {
        game: String,
    },
    Ignore,
}

fn device_kind(device_type: &gamesense::DeviceType) -> Option<DeviceType> {
    use gamesense::DeviceType as G;
    match device_type {
        G::Keyboard | G::RgbPerKeyZones | G::Indicator => Some(DeviceType::Keyboard),
        G::Mouse => Some(DeviceType::Mouse),
        G::Headset => Some(DeviceType::Headset),
        // Generic zoned devices: the SDK's "any RGB device with N zones" — keyboards are the
        // only zoned family the engine animates, so route them there.
        G::RgbZoned(_) | G::RgbZonedDevice => Some(DeviceType::Keyboard),
        G::Screened(_) | G::Tactile => None,
    }
}

fn zone_for(target: &LightTarget) -> OverlayZone {
    match target {
        LightTarget::AllZones => OverlayZone::All,
        LightTarget::Zone(index) => OverlayZone::Index(*index),
        LightTarget::NamedZone(name) => OverlayZone::Named(name.clone()),
        LightTarget::Keys(keys) => OverlayZone::Keys(keys.clone()),
        LightTarget::HidCodes(codes) => {
            OverlayZone::Keys(codes.iter().filter_map(|c| gamesense::hid_to_key_id(*c)).collect())
        }
    }
}

/// Map one GameSense output to an engine action.
pub fn translate(output: GameSenseOutput, now: Instant) -> Action {
    match output {
        GameSenseOutput::Lighting {
            game,
            device_type,
            target,
            color,
            flash,
            duration,
            ..
        } => {
            let Some(kind) = device_kind(&device_type) else {
                return Action::Ignore;
            };
            Action::Overlays(vec![Overlay {
                target: OverlayTarget::Kind(kind),
                zone: zone_for(&target),
                color,
                expires: now + duration.unwrap_or(HOLD),
                flash_hz: flash.map(|f| f.frequency_hz).filter(|hz| *hz > 0.0),
                source: Some(game),
            }])
        }
        GameSenseOutput::Bitmap { game, colors, .. } => {
            let keys = bitmap_key_colors(&colors);
            if keys.is_empty() {
                return Action::Ignore;
            }
            // One overlay per colour keeps the number of overlays small for typical frames.
            let mut by_color: Vec<(Color, Vec<KeyId>)> = Vec::new();
            for (key, color) in keys {
                match by_color.iter_mut().find(|(c, _)| *c == color) {
                    Some((_, list)) => list.push(key),
                    None => by_color.push((color, vec![key])),
                }
            }
            Action::Overlays(
                by_color
                    .into_iter()
                    .map(|(color, keys)| Overlay {
                        target: OverlayTarget::Kind(DeviceType::Keyboard),
                        zone: OverlayZone::Keys(keys),
                        color,
                        expires: now + HOLD,
                        flash_hz: None,
                        source: Some(game.clone()),
                    })
                    .collect(),
            )
        }
        GameSenseOutput::Screen {
            game,
            content,
            duration,
            ..
        } => Action::Screen {
            game,
            content: screen_content(content),
            duration,
        },
        GameSenseOutput::Clear { game } => Action::Clear { game },
        GameSenseOutput::Tactile { .. } => Action::Ignore,
    }
}

fn screen_content(content: gamesense::ScreenContent) -> ScreenContent {
    match content {
        gamesense::ScreenContent::Bitmap(data) => ScreenContent::Bitmap {
            width: 128,
            height: (data.len() as u32 * 8) / 128,
            data,
        },
        gamesense::ScreenContent::Lines { lines, .. } => {
            let mut text = Vec::new();
            let mut progress = None;
            for line in lines {
                match line {
                    ScreenLine::Text { text: t, .. } => text.push(t),
                    ScreenLine::ProgressBar { percent } => progress = Some(percent),
                }
            }
            match progress {
                Some(percent) if text.len() <= 1 => ScreenContent::Progress {
                    label: text.into_iter().next().unwrap_or_default(),
                    percent: f32::from(percent),
                },
                Some(percent) => {
                    let filled = usize::from(percent.min(100)) / 10;
                    text.push(format!("[{}{}]", "#".repeat(filled), "-".repeat(10 - filled)));
                    ScreenContent::Text { lines: text }
                }
                None => ScreenContent::Text { lines: text },
            }
        }
    }
}

/// The SDK's full-keyboard bitmap is 132 colours on a 22x6 grid, row by row from the top-left.
/// This places a standard ANSI layout on that grid. The SDK does not publish exact key
/// positions, so the columns are an approximation of the physical layout.
const BITMAP_GRID: [&[(usize, KeyId)]; 6] = {
    use KeyId::*;
    [
        &[
            (0, Escape),
            (2, F1),
            (3, F2),
            (4, F3),
            (5, F4),
            (6, F5),
            (7, F6),
            (8, F7),
            (9, F8),
            (10, F9),
            (11, F10),
            (12, F11),
            (13, F12),
        ],
        &[
            (0, Backtick),
            (1, Key1),
            (2, Key2),
            (3, Key3),
            (4, Key4),
            (5, Key5),
            (6, Key6),
            (7, Key7),
            (8, Key8),
            (9, Key9),
            (10, Key0),
            (11, Minus),
            (12, Equal),
            (13, Backspace),
            (14, Insert),
            (15, Home),
            (16, PageUp),
            (17, NumLock),
            (18, NumSlash),
            (19, NumAsterisk),
            (20, NumMinus),
        ],
        &[
            (0, Tab),
            (1, Q),
            (2, W),
            (3, E),
            (4, R),
            (5, T),
            (6, Y),
            (7, U),
            (8, I),
            (9, O),
            (10, P),
            (11, LeftBracket),
            (12, RightBracket),
            (13, Backslash),
            (14, Delete),
            (15, End),
            (16, PageDown),
            (17, Num7),
            (18, Num8),
            (19, Num9),
            (20, NumPlus),
        ],
        &[
            (0, CapsLock),
            (1, A),
            (2, S),
            (3, D),
            (4, F),
            (5, G),
            (6, H),
            (7, J),
            (8, K),
            (9, L),
            (10, Semicolon),
            (11, Quote),
            (13, Enter),
            (17, Num4),
            (18, Num5),
            (19, Num6),
        ],
        &[
            (0, LeftShift),
            (2, Z),
            (3, X),
            (4, C),
            (5, V),
            (6, B),
            (7, N),
            (8, M),
            (9, Comma),
            (10, Period),
            (11, Slash),
            (13, RightShift),
            (15, ArrowUp),
            (17, Num1),
            (18, Num2),
            (19, Num3),
            (20, NumEnter),
        ],
        &[
            (0, LeftCtrl),
            (1, LeftWin),
            (2, LeftAlt),
            (6, Space),
            (10, RightAlt),
            (11, RightWin),
            (12, Menu),
            (13, RightCtrl),
            (14, ArrowLeft),
            (15, ArrowDown),
            (16, ArrowRight),
            (17, Num0),
            (19, NumPeriod),
        ],
    ]
};

/// Colour per key for an SDK bitmap frame (132 entries, 22 per row).
pub fn bitmap_key_colors(colors: &[Color]) -> Vec<(KeyId, Color)> {
    const COLUMNS: usize = 22;
    let mut out = Vec::new();
    for (row, keys) in BITMAP_GRID.iter().enumerate() {
        for (column, key) in keys.iter() {
            if let Some(color) = colors.get(row * COLUMNS + column) {
                out.push((*key, *color));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitmap_places_keys_on_the_grid() {
        let mut colors = vec![Color::BLACK; 132];
        colors[0] = Color::RED; // row 0, col 0 = Escape
        colors[22 + 1] = Color::GREEN; // row 1, col 1 = 1
        colors[5 * 22 + 6] = Color::BLUE; // row 5, col 6 = Space
        let keys = bitmap_key_colors(&colors);
        assert!(keys.contains(&(KeyId::Escape, Color::RED)));
        assert!(keys.contains(&(KeyId::Key1, Color::GREEN)));
        assert!(keys.contains(&(KeyId::Space, Color::BLUE)));
        let unique: std::collections::HashSet<_> = keys.iter().map(|(k, _)| *k).collect();
        assert_eq!(unique.len(), keys.len(), "no key placed twice");
    }

    #[test]
    fn short_bitmaps_do_not_panic() {
        assert!(bitmap_key_colors(&[Color::RED; 5]).len() <= 5);
    }

    #[test]
    fn progress_only_screens_become_progress_bars() {
        let content = gamesense::ScreenContent::Lines {
            lines: vec![
                ScreenLine::Text {
                    text: "HP".into(),
                    bold: false,
                    wrap: 0,
                },
                ScreenLine::ProgressBar { percent: 40 },
            ],
            icon_id: None,
        };
        match screen_content(content) {
            ScreenContent::Progress { label, percent } => {
                assert_eq!(label, "HP");
                assert!((percent - 40.0).abs() < f32::EPSILON);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn lighting_targets_follow_the_device_type() {
        let action = translate(
            GameSenseOutput::Lighting {
                game: "G".into(),
                event: "HEALTH".into(),
                device_type: gamesense::DeviceType::Mouse,
                target: LightTarget::NamedZone("logo".into()),
                color: Color::RED,
                flash: None,
                duration: None,
            },
            Instant::now(),
        );
        let Action::Overlays(overlays) = action else {
            panic!("expected overlays");
        };
        assert_eq!(overlays[0].target, OverlayTarget::Kind(DeviceType::Mouse));
        assert_eq!(overlays[0].zone, OverlayZone::Named("logo".into()));
        assert_eq!(overlays[0].source.as_deref(), Some("G"));
    }
}
