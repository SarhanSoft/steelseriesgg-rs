//! USB polling rate control for mice and keyboards.
//!
//! On Linux this sets the kernel's `usbhid` polling-interval parameters
//! (`/sys/module/usbhid/parameters/{mousepoll,kbpoll}`). Those parameters hold an interval in
//! **milliseconds** (`0` = use the device's own interval), apply to every USB mouse or keyboard
//! on the system, and only take effect when a device is bound to `usbhid` — so after writing
//! the parameter the SteelSeries interfaces are re-bound to apply it immediately.
//!
//! Because the interval is in whole milliseconds, the kernel route tops out at 1000 Hz. Higher
//! rates (2000/4000/8000 Hz) are a device setting and are changed through the device's own
//! protocol (`polling_rate` setting on supported mice), not here.
//!
//! Requires root.

use crate::{Error, Result};
#[cfg(target_os = "linux")]
use rustix::process::geteuid;

/// USB polling rate options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollRate {
    /// No kernel override: the device uses its own interval.
    DeviceDefault,
    /// 125 Hz (8ms interval)
    Hz125,
    /// 250 Hz (4ms interval)
    Hz250,
    /// 500 Hz (2ms interval)
    Hz500,
    /// 1000 Hz (1ms interval)
    Hz1000,
    /// 2000 Hz (0.5ms interval) - device-side setting only on Linux
    Hz2000,
    /// 4000 Hz (0.25ms interval) - device-side setting only on Linux
    Hz4000,
    /// Any other kernel interval, in milliseconds.
    IntervalMs(u8),
}

impl PollRate {
    /// Convert to the `usbhid` parameter value (an interval in milliseconds, `0` = default).
    pub fn to_sysfs_value(&self) -> Result<u8> {
        match self {
            PollRate::DeviceDefault => Ok(0),
            PollRate::Hz125 => Ok(8),
            PollRate::Hz250 => Ok(4),
            PollRate::Hz500 => Ok(2),
            PollRate::Hz1000 => Ok(1),
            PollRate::IntervalMs(ms) => Ok(*ms),
            PollRate::Hz2000 | PollRate::Hz4000 => Err(Error::Unsupported(format!(
                "{} Hz cannot be set through the kernel: usbhid's interval is in whole milliseconds, \
                 so 1000 Hz is its maximum. Set it on the device instead (mouse `polling_rate` setting).",
                self.to_hz()
            ))),
        }
    }

    /// Convert a `usbhid` parameter value (interval in ms) to a poll rate.
    pub fn from_sysfs_value(value: u8) -> Result<Self> {
        Ok(match value {
            0 => PollRate::DeviceDefault,
            1 => PollRate::Hz1000,
            2 => PollRate::Hz500,
            4 => PollRate::Hz250,
            8 => PollRate::Hz125,
            ms => PollRate::IntervalMs(ms),
        })
    }

    /// Convert from frequency in Hz to poll rate enum.
    pub fn from_hz(hz: u32) -> Result<Self> {
        match hz {
            0 => Ok(PollRate::DeviceDefault),
            125 => Ok(PollRate::Hz125),
            250 => Ok(PollRate::Hz250),
            500 => Ok(PollRate::Hz500),
            1000 => Ok(PollRate::Hz1000),
            2000 => Ok(PollRate::Hz2000),
            4000 => Ok(PollRate::Hz4000),
            _ => Err(Error::InvalidConfig(format!(
                "Unsupported poll rate: {} Hz (supported: 125, 250, 500, 1000, 2000, 4000; 0 = device default)",
                hz
            ))),
        }
    }

    /// Convert poll rate to frequency in Hz (`0` for the device default).
    pub fn to_hz(&self) -> u32 {
        match self {
            PollRate::DeviceDefault => 0,
            PollRate::Hz125 => 125,
            PollRate::Hz250 => 250,
            PollRate::Hz500 => 500,
            PollRate::Hz1000 => 1000,
            PollRate::Hz2000 => 2000,
            PollRate::Hz4000 => 4000,
            PollRate::IntervalMs(ms) => 1000 / u32::from((*ms).max(1)),
        }
    }

    /// Check if this poll rate requires special hardware support.
    pub fn requires_hardware_support(&self) -> bool {
        matches!(self, PollRate::Hz2000 | PollRate::Hz4000)
    }

    /// Get a human-readable description of the poll rate.
    pub fn description(&self) -> String {
        match self {
            PollRate::DeviceDefault => "device default (no kernel override)".to_string(),
            PollRate::Hz125 => "125 Hz (8ms) - Power saving".to_string(),
            PollRate::Hz250 => "250 Hz (4ms)".to_string(),
            PollRate::Hz500 => "500 Hz (2ms) - Standard".to_string(),
            PollRate::Hz1000 => "1000 Hz (1ms) - Gaming".to_string(),
            PollRate::Hz2000 => "2000 Hz (0.5ms) - High-end gaming (requires hardware support)".to_string(),
            PollRate::Hz4000 => "4000 Hz (0.25ms) - Enthusiast (requires hardware support)".to_string(),
            PollRate::IntervalMs(ms) => format!("{} Hz ({ms}ms)", self.to_hz()),
        }
    }
}

impl std::fmt::Display for PollRate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PollRate::DeviceDefault => write!(f, "device default"),
            other => write!(f, "{} Hz", other.to_hz()),
        }
    }
}

/// Device type for poll rate control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    /// Mouse device
    Mouse,
    /// Keyboard device
    Keyboard,
}

impl DeviceType {
    #[cfg(target_os = "linux")]
    fn sysfs_path(&self) -> &'static str {
        match self {
            DeviceType::Mouse => "/sys/module/usbhid/parameters/mousepoll",
            DeviceType::Keyboard => "/sys/module/usbhid/parameters/kbpoll",
        }
    }

    #[cfg(target_os = "linux")]
    fn module_param(&self) -> &'static str {
        match self {
            DeviceType::Mouse => "mousepoll",
            DeviceType::Keyboard => "kbpoll",
        }
    }

    /// Get the display name for this device type.
    pub fn name(&self) -> &'static str {
        match self {
            DeviceType::Mouse => "mouse",
            DeviceType::Keyboard => "keyboard",
        }
    }
}

/// Check if the current process is running as root (Linux only).
#[cfg(target_os = "linux")]
pub fn is_root() -> bool {
    geteuid().as_raw() == 0
}

#[cfg(target_os = "linux")]
fn require_root(device_type: DeviceType, rate: PollRate) -> Result<()> {
    if is_root() {
        return Ok(());
    }
    let bin_name = std::env::current_exe()
        .ok()
        .and_then(|path| path.file_name().map(|name| name.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "ssgg".to_string());
    Err(Error::PermissionDenied(format!(
        "Poll rate changes require root privileges. Try: sudo {} pollrate {} {}",
        bin_name,
        device_type.name(),
        rate.to_hz()
    )))
}

/// Set USB polling rate for a device type.
///
/// On Linux this writes the kernel usbhid module parameter via sysfs (root required), then
/// re-binds SteelSeries USB HID interfaces so the new interval applies without a replug.
#[cfg(target_os = "linux")]
pub async fn set_poll_rate(device_type: DeviceType, rate: PollRate) -> Result<()> {
    let value = rate.to_sysfs_value()?;
    require_root(device_type, rate)?;

    let path = device_type.sysfs_path();
    tokio::fs::write(path, value.to_string()).await.map_err(|e| {
        Error::Io(std::io::Error::new(
            e.kind(),
            format!(
                "Failed to write poll rate to {}: {}. Is the usbhid module loaded?",
                path, e
            ),
        ))
    })?;

    match tokio::task::spawn_blocking(rebind_steelseries_usbhid).await {
        Ok(Ok(count)) => tracing::info!("Re-bound {count} SteelSeries HID interface(s) to apply the new interval"),
        Ok(Err(e)) => tracing::warn!("Poll rate written, but re-binding devices failed ({e}); replug them to apply"),
        Err(e) => tracing::warn!("Poll rate written, but the re-bind task failed ({e}); replug devices to apply"),
    }
    Ok(())
}

/// Make the kernel polling interval permanent across reboots by writing
/// `/etc/modprobe.d/ssgg-usbhid.conf` (root required).
///
/// When `usbhid` is built into the kernel rather than loaded as a module, modprobe options are
/// ignored; the returned note then says to use the `usbhid.<param>=` kernel command-line option.
#[cfg(target_os = "linux")]
pub async fn persist_poll_rate(device_type: DeviceType, rate: PollRate) -> Result<String> {
    const CONF: &str = "/etc/modprobe.d/ssgg-usbhid.conf";
    let value = rate.to_sysfs_value()?;
    require_root(device_type, rate)?;

    let existing = tokio::fs::read_to_string(CONF).await.unwrap_or_default();
    let param = device_type.module_param();
    let mut params: Vec<(String, String)> = existing
        .split_whitespace()
        .filter_map(|token| token.split_once('='))
        .filter(|(k, _)| *k != param)
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    params.push((param.to_string(), value.to_string()));
    let options: Vec<String> = params.iter().map(|(k, v)| format!("{k}={v}")).collect();
    let content = format!(
        "# Written by ssgg pollrate --persistent\noptions usbhid {}\n",
        options.join(" ")
    );
    tokio::fs::write(CONF, content).await?;

    let builtin = !std::path::Path::new("/sys/module/usbhid/initstate").exists();
    Ok(if builtin {
        format!(
            "usbhid is built into this kernel, so {CONF} is ignored at boot. Add `usbhid.{param}={value}` \
             to the kernel command line instead."
        )
    } else {
        format!("Saved to {CONF}; applies on every boot.")
    })
}

#[cfg(not(target_os = "linux"))]
pub async fn persist_poll_rate(_device_type: DeviceType, _rate: PollRate) -> Result<String> {
    Err(Error::Unsupported(
        "persistent kernel poll rate is Linux-only".to_string(),
    ))
}

/// Unbind and re-bind every SteelSeries interface owned by `usbhid` so it re-reads the
/// polling-interval parameters. Returns the number of interfaces re-bound.
#[cfg(target_os = "linux")]
fn rebind_steelseries_usbhid() -> Result<usize> {
    const DRIVER: &str = "/sys/bus/usb/drivers/usbhid";
    let mut interfaces = Vec::new();
    for entry in std::fs::read_dir(DRIVER)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        // Interface entries look like "1-2:1.0"; the parent device holds idVendor.
        let Some((device, _)) = name.split_once(':') else {
            continue;
        };
        let vendor_path = format!("/sys/bus/usb/devices/{device}/idVendor");
        let is_steelseries = std::fs::read_to_string(vendor_path)
            .map(|v| v.trim().eq_ignore_ascii_case("1038"))
            .unwrap_or(false);
        if is_steelseries {
            interfaces.push(name);
        }
    }
    for interface in &interfaces {
        std::fs::write(format!("{DRIVER}/unbind"), interface)?;
        std::fs::write(format!("{DRIVER}/bind"), interface)?;
    }
    Ok(interfaces.len())
}

#[cfg(not(target_os = "linux"))]
#[allow(clippy::needless_return)]
pub async fn set_poll_rate(device_type: DeviceType, rate: PollRate) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        let ms = 1000 / rate.to_hz().max(1);
        return tokio::task::spawn_blocking(move || {
            windows_hid_poll_ioctl(device_type, Some(ms))?;
            Ok(())
        })
        .await
        .map_err(|e| Error::Other(format!("Blocking task error: {e}")))?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (device_type, rate);
        Err(Error::Other(
            "Poll rate control is not supported on this platform".to_string(),
        ))
    }
}

/// Get the current USB polling rate for a device type.
///
/// On Linux this reads the kernel usbhid module parameter via sysfs. A value of 0 means no
/// kernel override is active and is reported as [`PollRate::DeviceDefault`].
/// On Windows this queries the HID class driver via IOCTL_HID_GET_POLL_FREQUENCY_MSEC.
#[cfg(target_os = "linux")]
pub async fn get_poll_rate(device_type: DeviceType) -> Result<PollRate> {
    let path = device_type.sysfs_path();

    let content = tokio::fs::read_to_string(path).await.map_err(|e| {
        Error::Io(std::io::Error::new(
            e.kind(),
            format!(
                "Failed to read poll rate from {}: {}. Is the usbhid module loaded?",
                path, e
            ),
        ))
    })?;

    let value: u8 = content
        .trim()
        .parse()
        .map_err(|e| Error::InvalidConfig(format!("Invalid poll rate value in {}: {}", path, e)))?;

    PollRate::from_sysfs_value(value)
}

#[cfg(not(target_os = "linux"))]
#[allow(clippy::needless_return)]
pub async fn get_poll_rate(device_type: DeviceType) -> Result<PollRate> {
    #[cfg(target_os = "windows")]
    {
        return tokio::task::spawn_blocking(move || {
            let ms = windows_hid_poll_ioctl(device_type, None)?;
            // 0 ms means the driver is using the device's native hardware interval
            // (typically 1 ms = 1000 Hz for SteelSeries keyboards).
            if ms == 0 {
                return Ok(PollRate::Hz1000);
            }
            let hz = 1000u32 / ms;
            PollRate::from_hz(hz)
        })
        .await
        .map_err(|e| Error::Other(format!("Blocking task error: {e}")))?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = device_type;
        Err(Error::Other(
            "Poll rate detection is not supported on this platform".to_string(),
        ))
    }
}

/// Send a Windows HID poll-frequency IOCTL.
///
/// `set_ms = None`      → GET; returns the current interval in milliseconds.
/// `set_ms = Some(ms)`  → SET; programs the interval and returns ms on success.
///
/// Uses `IOCTL_HID_GET_POLL_FREQUENCY_MSEC` / `IOCTL_HID_SET_POLL_FREQUENCY_MSEC`
/// (hidclass.sys, FILE_ANY_ACCESS — no elevated privileges required for GET;
///  SET may be restricted depending on the Windows version and driver).
#[cfg(target_os = "windows")]
fn windows_hid_poll_ioctl(device_type: DeviceType, set_ms: Option<u32>) -> Result<u32> {
    use hidapi::HidApi;
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING};
    use windows_sys::Win32::System::IO::DeviceIoControl;

    // HID_CTL_CODE(n) = CTL_CODE(FILE_DEVICE_KEYBOARD=0x0B, n, METHOD_BUFFERED=0, FILE_ANY_ACCESS=0)
    const IOCTL_HID_GET_POLL_FREQUENCY_MSEC: u32 = 0x000B_0024; // HID_CTL_CODE(9)
    const IOCTL_HID_SET_POLL_FREQUENCY_MSEC: u32 = 0x000B_0028; // HID_CTL_CODE(10)

    let (usage_page, usage): (u16, u16) = match device_type {
        DeviceType::Keyboard => (0x0001, 0x0006), // Generic Desktop / Keyboard
        DeviceType::Mouse => (0x0001, 0x0002),    // Generic Desktop / Mouse
    };

    let api = HidApi::new().map_err(|e| Error::DeviceCommunication(format!("HID API init failed: {e}")))?;

    let path = api
        .device_list()
        .find(|d| d.vendor_id() == crate::STEELSERIES_VENDOR_ID && d.usage_page() == usage_page && d.usage() == usage)
        .ok_or_else(|| {
            Error::DeviceNotFound(format!(
                "No SteelSeries {} found for poll-rate query",
                device_type.name()
            ))
        })?
        .path()
        .to_string_lossy()
        .into_owned();

    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();

    // FILE_ANY_ACCESS (dwDesiredAccess = 0) is sufficient for both GET and SET IOCTLs.
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        )
    };

    if handle == INVALID_HANDLE_VALUE {
        return Err(Error::DeviceCommunication(format!(
            "Failed to open HID device '{}': {}",
            path,
            std::io::Error::last_os_error()
        )));
    }

    let result: Result<u32> = if let Some(ms) = set_ms {
        let mut bytes_ret: u32 = 0;
        let ok = unsafe {
            DeviceIoControl(
                handle,
                IOCTL_HID_SET_POLL_FREQUENCY_MSEC,
                (&ms as *const u32).cast(),
                std::mem::size_of::<u32>() as u32,
                std::ptr::null_mut(),
                0,
                &mut bytes_ret,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            Err(Error::DeviceCommunication(format!(
                "IOCTL_HID_SET_POLL_FREQUENCY_MSEC failed: {}",
                std::io::Error::last_os_error()
            )))
        } else {
            Ok(ms)
        }
    } else {
        let mut poll_ms: u32 = 0;
        let mut bytes_ret: u32 = 0;
        let ok = unsafe {
            DeviceIoControl(
                handle,
                IOCTL_HID_GET_POLL_FREQUENCY_MSEC,
                std::ptr::null(),
                0,
                (&mut poll_ms as *mut u32).cast(),
                std::mem::size_of::<u32>() as u32,
                &mut bytes_ret,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            let os_err = std::io::Error::last_os_error();
            // ERROR_INVALID_FUNCTION (1): driver doesn't implement this IOCTL.
            // hidusb.sys (USB HID) never supports it; only PS/2 kbdhid does.
            if os_err.raw_os_error() == Some(1) {
                Err(Error::DeviceCommunication(
                    "Poll rate query is not supported by this device's HID driver. \
                     USB HID devices (including SteelSeries keyboards) do not expose \
                     poll rate via the Windows HID class IOCTL interface."
                        .into(),
                ))
            } else {
                Err(Error::DeviceCommunication(format!(
                    "IOCTL_HID_GET_POLL_FREQUENCY_MSEC failed: {os_err}"
                )))
            }
        } else {
            Ok(poll_ms)
        }
    };

    unsafe { CloseHandle(handle) };
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_poll_rate_conversion() {
        assert_eq!(PollRate::DeviceDefault.to_sysfs_value().unwrap(), 0);
        assert_eq!(PollRate::Hz125.to_sysfs_value().unwrap(), 8);
        assert_eq!(PollRate::Hz250.to_sysfs_value().unwrap(), 4);
        assert_eq!(PollRate::Hz500.to_sysfs_value().unwrap(), 2);
        assert_eq!(PollRate::Hz1000.to_sysfs_value().unwrap(), 1);
        assert!(PollRate::Hz2000.to_sysfs_value().is_err());
        assert!(PollRate::Hz4000.to_sysfs_value().is_err());

        assert_eq!(PollRate::from_sysfs_value(0).unwrap(), PollRate::DeviceDefault);
        assert_eq!(PollRate::from_sysfs_value(1).unwrap(), PollRate::Hz1000);
        assert_eq!(PollRate::from_sysfs_value(2).unwrap(), PollRate::Hz500);
        assert_eq!(PollRate::from_sysfs_value(4).unwrap(), PollRate::Hz250);
        assert_eq!(PollRate::from_sysfs_value(8).unwrap(), PollRate::Hz125);
        assert_eq!(PollRate::from_sysfs_value(3).unwrap(), PollRate::IntervalMs(3));
        assert_eq!(PollRate::IntervalMs(3).to_hz(), 333);
    }

    #[test]
    fn test_hz_conversion() {
        assert_eq!(PollRate::from_hz(0).unwrap(), PollRate::DeviceDefault);
        assert_eq!(PollRate::from_hz(125).unwrap(), PollRate::Hz125);
        assert_eq!(PollRate::from_hz(250).unwrap(), PollRate::Hz250);
        assert_eq!(PollRate::from_hz(500).unwrap(), PollRate::Hz500);
        assert_eq!(PollRate::from_hz(1000).unwrap(), PollRate::Hz1000);
        assert_eq!(PollRate::from_hz(2000).unwrap(), PollRate::Hz2000);
        assert_eq!(PollRate::from_hz(4000).unwrap(), PollRate::Hz4000);
        assert!(PollRate::from_hz(8000).is_err());
        assert!(PollRate::from_hz(300).is_err());

        for rate in [PollRate::Hz125, PollRate::Hz250, PollRate::Hz500, PollRate::Hz1000] {
            let back = PollRate::from_sysfs_value(rate.to_sysfs_value().unwrap()).unwrap();
            assert_eq!(back, rate);
        }
    }

    #[test]
    fn test_hardware_support_check() {
        assert!(!PollRate::Hz125.requires_hardware_support());
        assert!(!PollRate::Hz500.requires_hardware_support());
        assert!(!PollRate::Hz1000.requires_hardware_support());
        assert!(PollRate::Hz2000.requires_hardware_support());
        assert!(PollRate::Hz4000.requires_hardware_support());
    }

    #[test]
    fn test_descriptions() {
        assert!(PollRate::Hz125.description().contains("125 Hz"));
        assert!(PollRate::Hz4000.description().contains("hardware support"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_device_type_paths() {
        assert_eq!(
            DeviceType::Mouse.sysfs_path(),
            "/sys/module/usbhid/parameters/mousepoll"
        );
        assert_eq!(
            DeviceType::Keyboard.sysfs_path(),
            "/sys/module/usbhid/parameters/kbpoll"
        );
    }
}
