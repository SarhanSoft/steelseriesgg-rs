//! Binding model: what a key or mouse button does in a profile.

use std::collections::BTreeSet;
use std::fmt;

use serde::de::{self, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

use super::keys::{InputKey, MediaKey, MouseButton};
use super::text::text_to_steps;
use crate::error::{Error, Result};

/// Most steps one macro may have.
pub const MAX_MACRO_STEPS: usize = 4096;
/// Highest `repeat` count of a `Once` macro.
pub const MAX_MACRO_REPEAT: u32 = 1000;
/// Longest single macro delay, in milliseconds.
pub const MAX_DELAY_MS: u32 = 60_000;
/// Longest `Text` action, in characters.
pub const MAX_TEXT_CHARS: usize = 4096;
/// Most keys in one combo.
pub const MAX_COMBO_KEYS: usize = 8;

/// When a binding fires.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerMode {
    /// Fire on key press. `Key`, `Combo`, `Media` and `MouseButton` actions follow the source key:
    /// held while it is held, auto-repeating while it repeats.
    #[default]
    Press,
    /// Fire once when the key is released. Key-like actions are tapped instead of held.
    Release,
}

impl TriggerMode {
    fn is_default(&self) -> bool {
        *self == TriggerMode::default()
    }
}

/// How a macro plays.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MacroMode {
    /// Play `repeat` times per trigger. A trigger while it is still playing is ignored.
    #[default]
    Once,
    /// Loop while the source key is held; stop at once when it is released.
    WhileHeld,
    /// The first trigger starts an endless loop; the next trigger stops it.
    Toggle,
}

/// One step of a macro.
///
/// Keys a macro still holds at the end of an iteration, or when it is stopped, are released, so a
/// macro can never leave a key stuck down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MacroStep {
    Press(InputKey),
    Release(InputKey),
    /// Press then release, held for the remapper's tap duration.
    Tap(InputKey),
    /// Wait this many milliseconds.
    Delay(u32),
}

/// What a bound key does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// Act as another key.
    Key(InputKey),
    /// Press several keys together, e.g. Ctrl+C. Pressed in order, released in reverse.
    /// Deserializes from a list or from a `+`-separated string such as `"ctrl+shift+esc"`.
    Combo(#[serde(deserialize_with = "deserialize_combo")] Vec<InputKey>),
    /// Play a recorded or written macro.
    Macro {
        steps: Vec<MacroStep>,
        /// Plays per trigger; only used by `MacroMode::Once`.
        #[serde(default = "default_repeat")]
        repeat: u32,
        #[serde(default)]
        mode: MacroMode,
    },
    /// Type a string, assuming a US keyboard layout.
    Text(String),
    /// Start a program (no shell: `command` is the executable, `args` its arguments).
    Launch {
        command: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        args: Vec<String>,
    },
    Media(MediaKey),
    MouseButton(MouseButton),
    /// Ask the owner of the engine to switch to the named profile.
    SwitchProfile(String),
    /// Swallow the key.
    Disabled,
    /// Behave as if unbound.
    Passthrough,
}

fn default_repeat() -> u32 {
    1
}

/// A key or mouse button and what it does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    pub source: InputKey,
    pub action: Action,
    #[serde(default, skip_serializing_if = "TriggerMode::is_default")]
    pub mode: TriggerMode,
}

impl Binding {
    /// A binding that fires on press.
    pub fn new(source: InputKey, action: Action) -> Self {
        Self {
            source,
            action,
            mode: TriggerMode::Press,
        }
    }

    /// Check this binding on its own (limits, text characters, mode combinations).
    pub fn validate(&self) -> Result<()> {
        let fail = |msg: String| Err(Error::InvalidConfig(format!("binding for {}: {msg}", self.source)));
        match &self.action {
            Action::Combo(keys) => {
                if keys.is_empty() || keys.len() > MAX_COMBO_KEYS {
                    return fail(format!("a combo needs 1 to {MAX_COMBO_KEYS} keys, got {}", keys.len()));
                }
            }
            Action::Macro { steps, repeat, mode } => {
                if steps.is_empty() || steps.len() > MAX_MACRO_STEPS {
                    return fail(format!(
                        "a macro needs 1 to {MAX_MACRO_STEPS} steps, got {}",
                        steps.len()
                    ));
                }
                if *repeat == 0 || *repeat > MAX_MACRO_REPEAT {
                    return fail(format!("repeat must be 1 to {MAX_MACRO_REPEAT}, got {repeat}"));
                }
                if let Some(ms) = steps.iter().find_map(|s| match s {
                    MacroStep::Delay(ms) if *ms > MAX_DELAY_MS => Some(*ms),
                    _ => None,
                }) {
                    return fail(format!("delay {ms} ms is longer than the {MAX_DELAY_MS} ms limit"));
                }
                if *mode == MacroMode::WhileHeld && self.mode == TriggerMode::Release {
                    return fail("a while-held macro cannot trigger on release".to_string());
                }
            }
            Action::Text(text) => {
                let chars = text.chars().count();
                if chars == 0 || chars > MAX_TEXT_CHARS {
                    return fail(format!("text must be 1 to {MAX_TEXT_CHARS} characters, got {chars}"));
                }
                if let Err(e) = text_to_steps(text) {
                    return fail(e.to_string());
                }
            }
            Action::Launch { command, args } => {
                if command.trim().is_empty() {
                    return fail("launch command is empty".to_string());
                }
                if command.contains('\0') || args.iter().any(|a| a.contains('\0')) {
                    return fail("launch command contains a NUL byte".to_string());
                }
            }
            Action::SwitchProfile(name) => {
                if name.trim().is_empty() {
                    return fail("profile name is empty".to_string());
                }
            }
            Action::Key(_) | Action::Media(_) | Action::MouseButton(_) | Action::Disabled | Action::Passthrough => {}
        }
        Ok(())
    }
}

/// The bindings of one profile. Keys without a binding pass through unchanged.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingSet {
    #[serde(default)]
    pub bindings: Vec<Binding>,
}

impl BindingSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// No bindings at all.
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// At least one binding changes behaviour (anything but `Passthrough`). Devices are only
    /// grabbed while this is true.
    pub fn is_active(&self) -> bool {
        self.bindings.iter().any(|b| b.action != Action::Passthrough)
    }

    /// Source keys whose behaviour is changed by this set.
    pub fn active_sources(&self) -> impl Iterator<Item = InputKey> + '_ {
        self.bindings
            .iter()
            .filter(|b| b.action != Action::Passthrough)
            .map(|b| b.source)
    }

    pub fn get(&self, source: InputKey) -> Option<&Binding> {
        self.bindings.iter().find(|b| b.source == source)
    }

    /// Add a binding, replacing (and returning) any existing binding of the same source.
    pub fn insert(&mut self, binding: Binding) -> Option<Binding> {
        match self.bindings.iter_mut().find(|b| b.source == binding.source) {
            Some(existing) => Some(std::mem::replace(existing, binding)),
            None => {
                self.bindings.push(binding);
                None
            }
        }
    }

    pub fn remove(&mut self, source: InputKey) -> Option<Binding> {
        let index = self.bindings.iter().position(|b| b.source == source)?;
        Some(self.bindings.remove(index))
    }

    /// Check every binding and reject duplicate sources.
    pub fn validate(&self) -> Result<()> {
        let mut seen = BTreeSet::new();
        for binding in &self.bindings {
            if !seen.insert(binding.source) {
                return Err(Error::InvalidConfig(format!(
                    "{} is bound more than once",
                    binding.source
                )));
            }
            binding.validate()?;
        }
        Ok(())
    }
}

fn deserialize_combo<'de, D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Vec<InputKey>, D::Error> {
    struct ComboVisitor;

    impl<'de> Visitor<'de> for ComboVisitor {
        type Value = Vec<InputKey>;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a list of keys or a string such as \"ctrl+c\"")
        }

        fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<Self::Value, E> {
            v.split('+')
                .map(|part| InputKey::parse(part).map_err(E::custom))
                .collect()
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<Self::Value, A::Error> {
            let mut keys = Vec::new();
            while let Some(key) = seq.next_element::<InputKey>()? {
                keys.push(key);
            }
            Ok(keys)
        }
    }

    deserializer.deserialize_any(ComboVisitor)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(name: &str) -> InputKey {
        InputKey::parse(name).unwrap()
    }

    fn sample_set() -> BindingSet {
        BindingSet {
            bindings: vec![
                Binding::new(key("capslock"), Action::Key(key("leftctrl"))),
                Binding::new(key("mouse4"), Action::Combo(vec![key("ctrl"), key("c")])),
                Binding::new(
                    key("f13"),
                    Action::Macro {
                        steps: vec![
                            MacroStep::Tap(key("h")),
                            MacroStep::Delay(50),
                            MacroStep::Press(key("shift")),
                            MacroStep::Tap(key("i")),
                            MacroStep::Release(key("shift")),
                        ],
                        repeat: 1,
                        mode: MacroMode::Once,
                    },
                ),
                Binding::new(key("f14"), Action::Text("Hello!\n".to_string())),
                Binding {
                    source: key("f15"),
                    action: Action::Launch {
                        command: "firefox".to_string(),
                        args: vec!["--new-window".to_string()],
                    },
                    mode: TriggerMode::Release,
                },
                Binding::new(key("f16"), Action::Media(MediaKey::PlayPause)),
                Binding::new(key("mouse5"), Action::MouseButton(MouseButton::Middle)),
                Binding::new(key("f17"), Action::SwitchProfile("gaming".to_string())),
                Binding::new(key("insert"), Action::Disabled),
                Binding::new(key("f18"), Action::Passthrough),
            ],
        }
    }

    #[test]
    fn json_round_trip() {
        let set = sample_set();
        set.validate().unwrap();
        let json = serde_json::to_string_pretty(&set).unwrap();
        let back: BindingSet = serde_json::from_str(&json).unwrap();
        assert_eq!(back, set);
        assert!(json.contains("\"source\": \"KEY_CAPSLOCK\""));
        assert!(json.contains("\"source\": \"BTN_SIDE\""));
        assert!(json.contains("\"mode\": \"release\""));
        assert!(json.contains("\"disabled\""));
    }

    #[test]
    fn toml_round_trip() {
        let set = sample_set();
        let text = toml::to_string(&set).unwrap();
        let back: BindingSet = toml::from_str(&text).unwrap();
        assert_eq!(back, set);
    }

    #[test]
    fn hand_written_json_with_aliases() {
        let json = r#"{
            "bindings": [
                { "source": "capslock", "action": { "key": "ctrl" } },
                { "source": "mouse4", "action": { "combo": "ctrl+shift+t" } },
                { "source": "f13", "action": { "macro": { "steps": [ { "tap": "a" }, { "delay": 20 } ] } } },
                { "source": "KEY_F14", "action": { "mouse_button": "mouse4" } },
                { "source": "KEY_F15", "action": { "media": "next_track" } },
                { "source": "pause", "action": "disabled" }
            ]
        }"#;
        let set: BindingSet = serde_json::from_str(json).unwrap();
        set.validate().unwrap();
        assert_eq!(set.bindings[0].source, key("KEY_CAPSLOCK"));
        assert_eq!(set.bindings[0].action, Action::Key(InputKey::KEY_LEFTCTRL));
        assert_eq!(
            set.bindings[1].action,
            Action::Combo(vec![InputKey::KEY_LEFTCTRL, InputKey::KEY_LEFTSHIFT, key("t")])
        );
        assert_eq!(
            set.bindings[2].action,
            Action::Macro {
                steps: vec![MacroStep::Tap(key("a")), MacroStep::Delay(20)],
                repeat: 1,
                mode: MacroMode::Once
            }
        );
        assert_eq!(set.bindings[3].action, Action::MouseButton(MouseButton::Side));
        assert_eq!(set.bindings[4].action, Action::Media(MediaKey::Next));
        assert_eq!(set.bindings[5].action, Action::Disabled);
        assert_eq!(set.bindings[0].mode, TriggerMode::Press);
    }

    #[test]
    fn unknown_key_in_json_is_an_error() {
        let json = r#"{ "bindings": [ { "source": "KEY_NOPE", "action": "disabled" } ] }"#;
        assert!(serde_json::from_str::<BindingSet>(json).is_err());
        let json = r#"{ "bindings": [ { "source": "a", "action": { "combo": "ctrl+nope" } } ] }"#;
        assert!(serde_json::from_str::<BindingSet>(json).is_err());
    }

    #[test]
    fn validation_rejects_bad_bindings() {
        let a = key("a");
        let bad = [
            Action::Combo(vec![]),
            Action::Macro {
                steps: vec![],
                repeat: 1,
                mode: MacroMode::Once,
            },
            Action::Macro {
                steps: vec![MacroStep::Tap(a)],
                repeat: 0,
                mode: MacroMode::Once,
            },
            Action::Macro {
                steps: vec![MacroStep::Delay(MAX_DELAY_MS + 1)],
                repeat: 1,
                mode: MacroMode::Once,
            },
            Action::Text(String::new()),
            Action::Text("caf\u{e9}".to_string()),
            Action::Launch {
                command: " ".to_string(),
                args: vec![],
            },
            Action::SwitchProfile(String::new()),
        ];
        for action in bad {
            assert!(
                Binding::new(a, action.clone()).validate().is_err(),
                "{action:?} should be rejected"
            );
        }

        let while_held_on_release = Binding {
            source: a,
            action: Action::Macro {
                steps: vec![MacroStep::Tap(a)],
                repeat: 1,
                mode: MacroMode::WhileHeld,
            },
            mode: TriggerMode::Release,
        };
        assert!(while_held_on_release.validate().is_err());
    }

    #[test]
    fn duplicate_sources_are_rejected() {
        let mut set = BindingSet::new();
        set.bindings.push(Binding::new(key("a"), Action::Disabled));
        set.bindings.push(Binding::new(key("a"), Action::Passthrough));
        assert!(set.validate().is_err());
    }

    #[test]
    fn insert_replaces_and_activity() {
        let mut set = BindingSet::new();
        assert!(set.is_empty());
        assert!(!set.is_active());
        assert!(set.insert(Binding::new(key("a"), Action::Passthrough)).is_none());
        assert!(!set.is_empty());
        assert!(!set.is_active(), "passthrough-only sets change nothing");
        let old = set.insert(Binding::new(key("a"), Action::Disabled));
        assert_eq!(old.map(|b| b.action), Some(Action::Passthrough));
        assert!(set.is_active());
        assert_eq!(set.active_sources().collect::<Vec<_>>(), vec![key("a")]);
        assert_eq!(set.get(key("a")).map(|b| &b.action), Some(&Action::Disabled));
        assert!(set.remove(key("a")).is_some());
        assert!(set.is_empty());
    }
}
