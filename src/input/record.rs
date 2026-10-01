//! Turning captured key events into macro steps.

use std::time::Duration;

use super::binding::{MAX_DELAY_MS, MacroStep};
use super::keys::InputKey;
use super::remapper::KeyValue;

/// A key event captured while recording, timed from the start of the recording.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordedEvent {
    pub at: Duration,
    pub key: InputKey,
    pub value: KeyValue,
}

impl RecordedEvent {
    pub fn new(at: Duration, key: InputKey, value: KeyValue) -> Self {
        Self { at, key, value }
    }
}

/// Convert captured events into macro steps.
///
/// - The gap between consecutive kept events becomes a `Delay` (whole milliseconds, capped at
///   [`MAX_DELAY_MS`]); there is no delay before the first step.
/// - Auto-repeat events and every event of `stop_key` are dropped.
/// - A release whose press happened before the recording started is dropped.
/// - Keys still held at the end are released, in the order they were pressed.
pub fn steps_from_recording(events: &[RecordedEvent], stop_key: Option<InputKey>) -> Vec<MacroStep> {
    let mut steps = Vec::new();
    let mut held: Vec<InputKey> = Vec::new();
    let mut last_at: Option<Duration> = None;
    for event in events {
        if Some(event.key) == stop_key {
            continue;
        }
        let step = match event.value {
            KeyValue::Repeat => continue,
            KeyValue::Press => {
                if held.contains(&event.key) {
                    continue;
                }
                held.push(event.key);
                MacroStep::Press(event.key)
            }
            KeyValue::Release => {
                let Some(pos) = held.iter().position(|k| *k == event.key) else {
                    continue;
                };
                held.remove(pos);
                MacroStep::Release(event.key)
            }
        };
        if let Some(last) = last_at {
            let gap = event.at.saturating_sub(last).as_millis();
            let gap = u32::try_from(gap).unwrap_or(u32::MAX).min(MAX_DELAY_MS);
            if gap > 0 {
                steps.push(MacroStep::Delay(gap));
            }
        }
        last_at = Some(event.at);
        steps.push(step);
    }
    steps.extend(held.into_iter().map(MacroStep::Release));
    steps
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(name: &str) -> InputKey {
        InputKey::parse(name).unwrap()
    }

    fn ev(ms: u64, name: &str, value: KeyValue) -> RecordedEvent {
        RecordedEvent::new(Duration::from_millis(ms), key(name), value)
    }

    #[test]
    fn delays_between_events() {
        let events = [
            ev(100, "a", KeyValue::Press),
            ev(180, "a", KeyValue::Release),
            ev(180, "b", KeyValue::Press),
            ev(250, "b", KeyValue::Release),
        ];
        assert_eq!(
            steps_from_recording(&events, None),
            vec![
                MacroStep::Press(key("a")),
                MacroStep::Delay(80),
                MacroStep::Release(key("a")),
                MacroStep::Press(key("b")),
                MacroStep::Delay(70),
                MacroStep::Release(key("b")),
            ]
        );
    }

    #[test]
    fn drops_stop_key_repeats_and_orphan_releases() {
        let events = [
            ev(0, "enter", KeyValue::Release),
            ev(10, "a", KeyValue::Press),
            ev(500, "a", KeyValue::Repeat),
            ev(530, "a", KeyValue::Repeat),
            ev(600, "a", KeyValue::Release),
            ev(900, "esc", KeyValue::Press),
        ];
        assert_eq!(
            steps_from_recording(&events, Some(InputKey::KEY_ESC)),
            vec![
                MacroStep::Press(key("a")),
                MacroStep::Delay(590),
                MacroStep::Release(key("a"))
            ]
        );
    }

    #[test]
    fn releases_keys_still_held() {
        let events = [ev(0, "leftshift", KeyValue::Press), ev(5, "x", KeyValue::Press)];
        assert_eq!(
            steps_from_recording(&events, None),
            vec![
                MacroStep::Press(InputKey::KEY_LEFTSHIFT),
                MacroStep::Delay(5),
                MacroStep::Press(key("x")),
                MacroStep::Release(InputKey::KEY_LEFTSHIFT),
                MacroStep::Release(key("x")),
            ]
        );
    }

    #[test]
    fn long_gaps_are_capped() {
        let events = [ev(0, "a", KeyValue::Press), ev(3_600_000, "a", KeyValue::Release)];
        assert_eq!(
            steps_from_recording(&events, None),
            vec![
                MacroStep::Press(key("a")),
                MacroStep::Delay(MAX_DELAY_MS),
                MacroStep::Release(key("a")),
            ]
        );
    }

    #[test]
    fn empty_recording() {
        assert!(steps_from_recording(&[], Some(InputKey::KEY_ESC)).is_empty());
    }
}
