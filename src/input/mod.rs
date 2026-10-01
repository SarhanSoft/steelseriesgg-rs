//! Key bindings and macros (SteelSeries GG "Key Bindings" and "Macro Editor").
//!
//! - [`BindingSet`] / [`Binding`] / [`Action`]: the per-profile model, serde-ready.
//! - [`InputKey`]: evdev key and mouse-button codes with names and aliases.
//! - [`Remapper`]: a pure, clock-injected state machine that turns physical key events into
//!   virtual key events and side-effect requests. Fully testable off Linux.
//! - [`InputEngine`]: the Linux I/O layer (evdev capture with `EVIOCGRAB`, uinput re-emission) on
//!   a dedicated thread. Other platforms get the same API returning `Error::Unsupported`.
//!
//! See `docs/development/input.md` for permissions, safety and limitations.

mod binding;
mod engine;
mod keys;
#[cfg(target_os = "linux")]
mod linux;
mod record;
mod remapper;
mod text;

pub use binding::{
    Action, Binding, BindingSet, MAX_COMBO_KEYS, MAX_DELAY_MS, MAX_MACRO_REPEAT, MAX_MACRO_STEPS, MAX_TEXT_CHARS,
    MacroMode, MacroStep, TriggerMode,
};
pub use engine::{DeviceFilter, EngineNotice, EngineOptions, InputEngine, StopReason, VIRTUAL_DEVICE_NAME};
pub use keys::{InputKey, KEY_MAX, MediaKey, MouseButton};
pub use record::{RecordedEvent, steps_from_recording};
pub use remapper::{ESCAPE_CHORD, Effect, KeyEvent, KeyValue, Remapper, RemapperConfig};
pub use text::text_to_steps;
