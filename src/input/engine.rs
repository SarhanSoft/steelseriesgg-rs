//! Public engine API. The work happens in `linux.rs`; other platforms get the same API returning
//! [`Error::Unsupported`].

use std::sync::mpsc::Receiver;
use std::time::Duration;

use super::binding::{BindingSet, MacroStep};
use super::keys::{InputKey, KEY_MAX};
use super::remapper::RemapperConfig;
use crate::STEELSERIES_VENDOR_ID;
#[cfg(not(target_os = "linux"))]
use crate::error::Error;
use crate::error::Result;

/// Name of the uinput device the engine creates. Devices with this name are never captured, so the
/// engine cannot read its own output.
pub const VIRTUAL_DEVICE_NAME: &str = "ssgg virtual input";

/// Which input devices to capture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceFilter {
    /// USB vendor ID; `None` matches any vendor.
    pub vendor_id: Option<u16>,
    /// USB product ID; `None` matches any product.
    pub product_id: Option<u16>,
    /// Case-insensitive substring of the kernel device name.
    pub name_contains: Option<String>,
}

impl DeviceFilter {
    /// Every SteelSeries device (vendor `0x1038`).
    pub fn steelseries() -> Self {
        Self {
            vendor_id: Some(STEELSERIES_VENDOR_ID),
            product_id: None,
            name_contains: None,
        }
    }

    pub fn matches(&self, vendor_id: u16, product_id: u16, name: &str) -> bool {
        if name == VIRTUAL_DEVICE_NAME {
            return false;
        }
        self.vendor_id.is_none_or(|v| v == vendor_id)
            && self.product_id.is_none_or(|p| p == product_id)
            && self
                .name_contains
                .as_ref()
                .is_none_or(|needle| name.to_lowercase().contains(&needle.to_lowercase()))
    }
}

impl Default for DeviceFilter {
    fn default() -> Self {
        Self::steelseries()
    }
}

/// Engine settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineOptions {
    pub devices: DeviceFilter,
    pub remapper: RemapperConfig,
    /// How often to look for newly connected devices while a binding set is active.
    pub rescan_interval: Duration,
}

impl Default for EngineOptions {
    fn default() -> Self {
        Self {
            devices: DeviceFilter::steelseries(),
            remapper: RemapperConfig::default(),
            rescan_interval: Duration::from_secs(2),
        }
    }
}

/// Why the engine stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    /// `stop()` was called or the [`InputEngine`] was dropped.
    Requested,
    /// The emergency escape chord (both Ctrl + both Shift held) was used.
    EmergencyEscape,
    /// An I/O error; every device was released before stopping.
    Error(String),
}

/// Messages from the engine thread, read through [`InputEngine::notices`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineNotice {
    /// A `SwitchProfile` binding fired. The owner should load that profile and call
    /// [`InputEngine::update`].
    SwitchProfile(String),
    /// A `Launch` binding could not start its program.
    LaunchFailed { command: String, error: String },
    /// A device is now captured.
    DeviceGrabbed { path: String, name: String },
    /// A device was released (no longer needed, or disconnected).
    DeviceReleased { path: String, name: String },
    /// The engine thread has ended; all devices are released.
    Stopped(StopReason),
}

/// True when a device that can produce the keys `supports` reports must be captured for
/// `bindings`: at least one of its keys has a binding that changes behaviour.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn device_needs_grab(bindings: &BindingSet, supports: impl Fn(InputKey) -> bool) -> bool {
    bindings.active_sources().any(supports)
}

/// Key codes the virtual device may advertise. Joystick, gamepad, digitizer and "trigger happy"
/// button ranges are left out: advertising them makes udev and libinput treat the virtual device as
/// a joystick or tablet.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn virtual_key_allowed(code: u16) -> bool {
    code != 0 && code <= KEY_MAX && !(0x118..=0x15f).contains(&code) && !(0x2c0..=0x2e7).contains(&code)
}

#[cfg(target_os = "linux")]
type Inner = super::linux::LinuxEngine;

#[cfg(not(target_os = "linux"))]
#[derive(Debug)]
enum Inner {}

#[cfg(not(target_os = "linux"))]
fn unsupported() -> Error {
    Error::Unsupported("key bindings and macros need Linux evdev/uinput".to_string())
}

/// Runs key bindings and macros on a dedicated thread.
///
/// While the binding set is active (see [`BindingSet::is_active`]), every SteelSeries input device
/// that has at least one bound key is captured with `EVIOCGRAB`, and all of its events are
/// re-emitted through one uinput virtual keyboard+mouse after remapping. When the set is empty,
/// nothing is captured. Dropping the engine stops it.
///
/// If the process dies (panic, `kill -9`, abort), the kernel closes its file descriptors, which
/// releases every grab and destroys the virtual device (releasing any key held on it), so input
/// is never left captured. Holding both Ctrl keys and both Shift keys for 2 seconds also stops
/// the engine.
#[derive(Debug)]
pub struct InputEngine {
    inner: Inner,
}

impl InputEngine {
    /// Start with default options (all SteelSeries devices).
    pub fn start(bindings: BindingSet) -> Result<InputEngine> {
        Self::start_with(bindings, EngineOptions::default())
    }

    /// Start with explicit options. Fails if the set is invalid, `/dev/uinput` cannot be opened,
    /// or no `/dev/input/event*` device can be opened at all.
    pub fn start_with(bindings: BindingSet, options: EngineOptions) -> Result<InputEngine> {
        #[cfg(target_os = "linux")]
        {
            Inner::start(bindings, options).map(|inner| InputEngine { inner })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (bindings, options);
            Err(unsupported())
        }
    }

    /// Replace the binding set (e.g. on profile switch). Validated before it is sent; captures are
    /// adjusted to the new set, and an empty set releases every device.
    pub fn update(&self, bindings: BindingSet) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            self.inner.update(bindings)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = bindings;
            match self.inner {}
        }
    }

    /// Stop the engine, release every key and device, and wait for the thread. Returns the error
    /// that stopped the engine earlier, if any.
    pub fn stop(self) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            self.inner.stop()
        }
        #[cfg(not(target_os = "linux"))]
        {
            match self.inner {}
        }
    }

    /// False once the engine thread has ended (stop, escape chord, or error).
    pub fn is_running(&self) -> bool {
        #[cfg(target_os = "linux")]
        {
            self.inner.is_running()
        }
        #[cfg(not(target_os = "linux"))]
        {
            match self.inner {}
        }
    }

    /// Profile-switch requests, launch failures, device changes and the final stop reason.
    pub fn notices(&self) -> &Receiver<EngineNotice> {
        #[cfg(target_os = "linux")]
        {
            self.inner.notices()
        }
        #[cfg(not(target_os = "linux"))]
        {
            match self.inner {}
        }
    }

    /// Record a macro from the real keyboard/mouse.
    ///
    /// Captures (grabs) every device matching `device_filter` once no key is held on it, so the
    /// keys typed while recording reach no application. Recording ends when `stop_key` is pressed
    /// (not recorded) or `timeout` passes; both return the steps captured so far, with real
    /// delays. The emergency escape chord aborts with an error. Fails if another program (such as
    /// a running [`InputEngine`]) already grabs a matching device.
    pub fn record_macro(device_filter: &DeviceFilter, stop_key: InputKey, timeout: Duration) -> Result<Vec<MacroStep>> {
        #[cfg(target_os = "linux")]
        {
            super::linux::record_macro(device_filter, stop_key, timeout)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (device_filter, stop_key, timeout);
            Err(unsupported())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::binding::{Action, Binding};

    fn key(name: &str) -> InputKey {
        InputKey::parse(name).unwrap()
    }

    #[test]
    fn steelseries_filter_matches_vendor_only() {
        let f = DeviceFilter::steelseries();
        assert!(f.matches(0x1038, 0x1610, "SteelSeries Apex Pro"));
        assert!(!f.matches(0x046d, 0xc52b, "Logitech USB Receiver"));
        assert!(!f.matches(0x1038, 0x0000, VIRTUAL_DEVICE_NAME), "never our own device");
        assert_eq!(DeviceFilter::default(), f);
    }

    #[test]
    fn filter_by_product_and_name() {
        let f = DeviceFilter {
            vendor_id: None,
            product_id: Some(0x1610),
            name_contains: Some("apex".to_string()),
        };
        assert!(f.matches(0x1038, 0x1610, "SteelSeries APEX Pro Keyboard"));
        assert!(!f.matches(0x1038, 0x1610, "SteelSeries Rival 3"));
        assert!(!f.matches(0x1038, 0x1611, "SteelSeries Apex Pro"));
        let any = DeviceFilter {
            vendor_id: None,
            product_id: None,
            name_contains: None,
        };
        assert!(any.matches(0x1234, 0x5678, "Anything"));
        assert!(!any.matches(0x0000, 0x0000, VIRTUAL_DEVICE_NAME));
    }

    #[test]
    fn grab_only_devices_with_a_bound_key() {
        let keyboard = |k: InputKey| !k.is_mouse_button();
        let mouse = |k: InputKey| k.is_mouse_button();

        let empty = BindingSet::new();
        assert!(!device_needs_grab(&empty, keyboard), "never grab for an empty set");

        let mut passthrough_only = BindingSet::new();
        passthrough_only.insert(Binding::new(key("a"), Action::Passthrough));
        assert!(!device_needs_grab(&passthrough_only, keyboard));

        let mut keys_only = BindingSet::new();
        keys_only.insert(Binding::new(key("capslock"), Action::Key(key("esc"))));
        assert!(device_needs_grab(&keys_only, keyboard));
        assert!(!device_needs_grab(&keys_only, mouse));

        let mut mouse_only = BindingSet::new();
        mouse_only.insert(Binding::new(key("mouse4"), Action::Disabled));
        assert!(device_needs_grab(&mouse_only, mouse));
        assert!(!device_needs_grab(&mouse_only, keyboard));
    }

    #[test]
    fn virtual_device_key_ranges() {
        assert!(virtual_key_allowed(1));
        assert!(virtual_key_allowed(248));
        assert!(virtual_key_allowed(0x110));
        assert!(virtual_key_allowed(0x117));
        assert!(virtual_key_allowed(0x100), "BTN_0 extra mouse buttons are allowed");
        assert!(!virtual_key_allowed(0), "KEY_RESERVED");
        assert!(!virtual_key_allowed(0x120), "BTN_JOYSTICK");
        assert!(!virtual_key_allowed(0x130), "BTN_GAMEPAD");
        assert!(!virtual_key_allowed(0x14a), "BTN_TOUCH");
        assert!(!virtual_key_allowed(0x2c0), "BTN_TRIGGER_HAPPY1");
        assert!(!virtual_key_allowed(0x300));
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn non_linux_reports_unsupported() {
        assert!(matches!(
            InputEngine::start(BindingSet::new()),
            Err(Error::Unsupported(_))
        ));
        assert!(matches!(
            InputEngine::record_macro(&DeviceFilter::steelseries(), InputKey::KEY_ESC, Duration::from_secs(1)),
            Err(Error::Unsupported(_))
        ));
    }
}
