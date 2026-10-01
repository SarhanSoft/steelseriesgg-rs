//! Key bindings and macros inside the engine: the active profile's bindings drive the input
//! engine (evdev grab + uinput), and its notices feed back into profile switching.

use tracing::{info, warn};

use crate::input::{BindingSet, EngineNotice, InputEngine};
use crate::notify::{self, Urgency};
use crate::{Error, Result};

/// Parse the `bindings` section of a profile.
pub fn bindings_from_profile(value: Option<&serde_json::Value>) -> Result<BindingSet> {
    match value {
        None | Some(serde_json::Value::Null) => Ok(BindingSet::default()),
        Some(value) => {
            let set: BindingSet = serde_json::from_value(value.clone())
                .map_err(|e| Error::Profile(format!("invalid key bindings: {e}")))?;
            set.validate()?;
            Ok(set)
        }
    }
}

/// The engine's input slot.
#[derive(Default)]
pub struct InputState {
    engine: Option<InputEngine>,
    active: BindingSet,
    last_error: Option<String>,
}

impl InputState {
    pub fn active(&self) -> &BindingSet {
        &self.active
    }

    pub fn is_running(&self) -> bool {
        self.engine.as_ref().is_some_and(InputEngine::is_running)
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// Make `bindings` the live set. Only the daemon captures input; a one-shot CLI engine
    /// just remembers the set.
    pub fn apply(&mut self, bindings: BindingSet, daemon: bool) {
        self.active = bindings;
        if !daemon {
            return;
        }
        let wanted = self.active.is_active();
        match (&self.engine, wanted) {
            (Some(engine), true) if engine.is_running() => {
                if let Err(e) = engine.update(self.active.clone()) {
                    warn!("Key bindings update failed: {e}");
                    self.last_error = Some(e.to_string());
                }
            }
            (_, true) => match InputEngine::start(self.active.clone()) {
                Ok(engine) => {
                    info!("Key bindings active ({} binding(s))", self.active.bindings.len());
                    self.engine = Some(engine);
                    self.last_error = None;
                }
                Err(e) => {
                    warn!("Key bindings unavailable: {e}");
                    self.last_error = Some(e.to_string());
                    self.engine = None;
                }
            },
            (_, false) => self.stop(),
        }
    }

    /// Release every captured device (shutdown, macro recording).
    pub fn stop(&mut self) {
        if let Some(engine) = self.engine.take()
            && let Err(e) = engine.stop()
        {
            warn!("Key bindings stop: {e}");
        }
    }

    /// Drain notices from the input thread. Returns profiles a binding asked to switch to.
    pub fn poll(&mut self) -> Vec<String> {
        let mut switches = Vec::new();
        let Some(engine) = &self.engine else {
            return switches;
        };
        let mut stopped = false;
        while let Ok(notice) = engine.notices().try_recv() {
            match notice {
                EngineNotice::SwitchProfile(name) => switches.push(name),
                EngineNotice::LaunchFailed { command, error } => {
                    warn!("Binding could not launch {command}: {error}");
                    notify::send("Key binding failed", &format!("{command}: {error}"), Urgency::Normal);
                }
                EngineNotice::DeviceGrabbed { name, .. } => info!("Key bindings now handle {name}"),
                EngineNotice::DeviceReleased { name, .. } => info!("Key bindings released {name}"),
                EngineNotice::Stopped(reason) => {
                    warn!("Key bindings stopped: {reason:?}");
                    self.last_error = Some(format!("stopped: {reason:?}"));
                    stopped = true;
                }
            }
        }
        if stopped {
            self.engine = None;
        }
        switches
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_or_null_bindings_are_empty() {
        assert!(bindings_from_profile(None).unwrap().bindings.is_empty());
        assert!(
            bindings_from_profile(Some(&serde_json::Value::Null))
                .unwrap()
                .bindings
                .is_empty()
        );
    }

    #[test]
    fn profile_bindings_parse_and_validate() {
        let value = serde_json::json!({"bindings":[{"source":"KEY_CAPSLOCK","action":{"key":"KEY_LEFTCTRL"}}]});
        let set = bindings_from_profile(Some(&value)).unwrap();
        assert_eq!(set.bindings.len(), 1);
        let bad = serde_json::json!({"bindings":[{"source":"NOT_A_KEY","action":"disabled"}]});
        assert!(bindings_from_profile(Some(&bad)).is_err());
    }

    #[test]
    fn a_cli_engine_only_remembers_the_set() {
        let mut state = InputState::default();
        let value = serde_json::json!({"bindings":[{"source":"KEY_F13","action":"disabled"}]});
        state.apply(bindings_from_profile(Some(&value)).unwrap(), false);
        assert_eq!(state.active().bindings.len(), 1);
        assert!(!state.is_running());
    }
}
