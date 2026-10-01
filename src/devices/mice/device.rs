//! [EXPERIMENTAL] The one mouse implementation: a profile plus a HID handle.

use std::time::{Duration, Instant};

use hidapi::HidDevice;
use parking_lot::Mutex;

use super::encode::{Report, ReportKind};
use super::profile::{self, EffectState};
use super::{Mouse, MouseModel};
use crate::devices::settings::{Configurable, DeviceStatus, SettingDescriptor, SettingValue};
use crate::devices::{Device, DeviceInfo, DeviceType};
use crate::rgb::Color;
use crate::{Error, Result};

/// Minimum time between two commands. rivalcfg waits 50 ms after every command and warns that
/// less can hang some mice (`Mouse.command_delay` in `rivalcfg/mouse.py`).
pub const COMMAND_SPACING: Duration = Duration::from_millis(50);

/// Minimum time between two direct-color writes. OpenRGB streams colors to the Rival 3, Rival 5
/// and Aerox mice without any delay (2 ms on the Aerox 5), so frames are not held to
/// [`COMMAND_SPACING`]. [EXPERIMENTAL] Guess: untested at animation rates.
pub const DIRECT_COLOR_SPACING: Duration = Duration::from_millis(2);

/// How long to wait for a response, as rivalcfg (`timeout_ms=200`).
pub const READ_TIMEOUT_MS: i32 = 200;

/// Byte-level access to the mouse. Exists so tests can record what would be sent.
pub(crate) trait Transport: Send {
    /// Send an output report; `wire` starts with the report ID.
    fn write_output(&mut self, wire: &[u8]) -> Result<()>;
    /// Send a feature report; `wire` starts with the report ID.
    fn write_feature(&mut self, wire: &[u8]) -> Result<()>;
    /// Read one input report, waiting at most `timeout_ms`. Returns the byte count (0 on
    /// timeout).
    fn read(&mut self, buf: &mut [u8], timeout_ms: i32) -> Result<usize>;
}

/// [`Transport`] over an opened hidapi handle.
pub(crate) struct HidTransport {
    device: HidDevice,
    /// hidraw node of the control interface, used for feature reports.
    #[cfg(target_os = "linux")]
    hidraw_path: String,
}

impl HidTransport {
    pub(crate) fn new(info: &DeviceInfo, model: &MouseModel, device: HidDevice) -> Self {
        #[cfg(target_os = "linux")]
        let hidraw_path = crate::devices::find_hidraw_for_interface(
            info.vendor_id,
            info.product_id,
            usize::try_from(model.interface_number).unwrap_or(0),
        )
        .unwrap_or_else(|| info.path.clone());
        #[cfg(not(target_os = "linux"))]
        let _ = (info, model);
        Self {
            device,
            #[cfg(target_os = "linux")]
            hidraw_path,
        }
    }
}

impl Transport for HidTransport {
    fn write_output(&mut self, wire: &[u8]) -> Result<()> {
        self.device.write(wire)?;
        Ok(())
    }

    /// hidapi's Linux backend sends `HIDIOCSFEATURE` with the wrong direction bits, so on Linux
    /// the feature report goes through the raw ioctl in `devices::send_feature_report_raw`.
    #[cfg(target_os = "linux")]
    fn write_feature(&mut self, wire: &[u8]) -> Result<()> {
        crate::devices::send_feature_report_raw(&self.hidraw_path, wire, wire.len())
    }

    #[cfg(not(target_os = "linux"))]
    fn write_feature(&mut self, wire: &[u8]) -> Result<()> {
        self.device.send_feature_report(wire)?;
        Ok(())
    }

    fn read(&mut self, buf: &mut [u8], timeout_ms: i32) -> Result<usize> {
        Ok(self.device.read_timeout(buf, timeout_ms)?)
    }
}

/// A SteelSeries mouse driven by its rivalcfg profile.
pub struct SteelSeriesMouse {
    info: DeviceInfo,
    model: &'static MouseModel,
    transport: Option<Mutex<Box<dyn Transport>>>,
    command_spacing: Duration,
    last_write: Option<Instant>,
    effects: EffectState,
}

impl SteelSeriesMouse {
    pub(crate) fn new(info: DeviceInfo, model: &'static MouseModel, transport: Box<dyn Transport>) -> Self {
        Self {
            info,
            model,
            transport: Some(Mutex::new(transport)),
            command_spacing: COMMAND_SPACING,
            last_write: None,
            effects: EffectState::default(),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_command_spacing(mut self, spacing: Duration) -> Self {
        self.command_spacing = spacing;
        self
    }

    fn transport(&mut self) -> Result<&mut Box<dyn Transport>> {
        self.transport
            .as_mut()
            .map(Mutex::get_mut)
            .ok_or_else(|| Error::DeviceCommunication(format!("{} is not connected", self.model.name)))
    }

    /// Send one report, keeping at least `spacing` since the previous one.
    fn send(&mut self, report: &Report, spacing: Duration) -> Result<()> {
        if let Some(last) = self.last_write {
            let elapsed = last.elapsed();
            if elapsed < spacing {
                std::thread::sleep(spacing - elapsed);
            }
        }
        let wire = report.wire_bytes();
        let result = match report.kind {
            ReportKind::Output => self.transport()?.write_output(&wire),
            ReportKind::Feature => self.transport()?.write_feature(&wire),
        };
        self.last_write = Some(Instant::now());
        result
    }

    /// Send a setting or save command, then drain the 2.4 GHz acknowledgement like rivalcfg.
    fn send_command(&mut self, report: &Report, spacing: Duration) -> Result<()> {
        self.send(report, spacing)?;
        let readback = self.model.profile.readback_len();
        if readback > 0 {
            let mut buf = vec![0u8; readback];
            self.transport()?.read(&mut buf, READ_TIMEOUT_MS)?;
        }
        Ok(())
    }

    /// Send a query and return the response bytes.
    fn query(&mut self, report: &Report, response_len: usize) -> Result<Vec<u8>> {
        let spacing = self.command_spacing;
        self.send(report, spacing)?;
        let mut buf = vec![0u8; response_len];
        let len = self.transport()?.read(&mut buf, READ_TIMEOUT_MS)?;
        buf.truncate(len);
        Ok(buf)
    }
}

impl Device for SteelSeriesMouse {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn device_type(&self) -> DeviceType {
        DeviceType::Mouse
    }

    fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    fn close(&mut self) -> Result<()> {
        self.transport = None;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.transport.is_some()
    }

    fn send_raw(&mut self, data: &[u8]) -> Result<()> {
        self.transport()?.write_output(data)
    }

    fn receive_raw(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.transport()?.read(buf, READ_TIMEOUT_MS)
    }
}

impl Configurable for SteelSeriesMouse {
    fn setting_descriptors(&self) -> Vec<SettingDescriptor> {
        profile::descriptors(self.model.profile)
    }

    /// Values are sent as they are given and are not saved to the mouse: rivalcfg's CLI sends
    /// its save command after every change, which here is the explicit `save` action instead,
    /// so the mouse's flash is written only when asked.
    fn apply_setting(&mut self, id: &str, value: &SettingValue) -> Result<()> {
        let reports = profile::reports_for(self.model.profile, &mut self.effects, id, value)?;
        let spacing = self.command_spacing;
        for report in &reports {
            self.send_command(report, spacing)?;
        }
        Ok(())
    }

    fn read_status(&mut self) -> Result<DeviceStatus> {
        let profile = self.model.profile;
        let mut status = DeviceStatus::default();
        if let Some(query) = profile.battery {
            let response = self.query(&profile.encode_command(&query.command), query.response_len)?;
            let (level, charging) = profile::parse_battery(query.format, &response);
            status.battery_percent = level;
            status.charging = charging;
        }
        if let Some(query) = profile.firmware {
            let response = self.query(&profile.encode_command(&query.command), query.response_len)?;
            if let Some(version) = profile::parse_firmware(&response) {
                status.extra.insert("firmware".to_string(), version);
            }
        }
        Ok(status)
    }
}

impl Mouse for SteelSeriesMouse {
    fn model(&self) -> &'static MouseModel {
        self.model
    }

    fn color_zone_names(&self) -> Vec<String> {
        self.model.profile.zone_names()
    }

    fn set_zone_colors_direct(&mut self, colors: &[Color]) -> Result<()> {
        let reports = profile::direct_color_reports(self.model.profile, colors)?;
        self.effects.clear_gradients();
        let spacing = self.command_spacing.min(DIRECT_COLOR_SPACING);
        for report in &reports {
            self.send_command(report, spacing)?;
        }
        Ok(())
    }
}
