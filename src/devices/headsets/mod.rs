//! Headset support (Arctis and Arctis Nova series).
//!
//! Data-driven: [`models::MODELS`] lists every known headset with its control interface, the
//! settings it offers ([`capability::Capability`], each carrying that model's byte encoding)
//! and its status protocol ([`status::StatusProtocol`]). One implementation,
//! [`SteelSeriesHeadset`], serves every model through the uniform settings model
//! ([`Configurable`]).
//!
//! [EXPERIMENTAL] Every protocol here comes from published reference drivers (HeadsetControl,
//! OpenRGB, nova-chatmix-linux). None of it has been tested on hardware by this project.

pub mod capability;
pub mod lighting;
pub mod models;
pub mod report;
pub mod status;

use std::time::{Duration, Instant};

use hidapi::HidDevice;
use parking_lot::Mutex;
use tracing::debug;

use self::capability::Capability;
use self::lighting::LightingState;
use self::report::{Report, ReportKind};
use self::status::{Parsed, Query, StatusProtocol};
use super::settings::{ChatMix, Configurable, DeviceStatus, SettingDescriptor, SettingValue};
use super::{Device, DeviceInfo, DeviceType};
use crate::{Error, Result};

pub use self::models::MODELS;

/// How long to wait for a status answer. Status is polled, so a missing answer must never block.
pub const READ_TIMEOUT: Duration = Duration::from_millis(100);
/// Largest answer any reference reads (HeadsetControl `STATUS_BUF_SIZE`).
const READ_BUFFER_LEN: usize = 128;
/// Upper bound on stale reports discarded before a request, so a chatty device cannot stall us.
const MAX_DRAINED_REPORTS: usize = 32;

/// Static description of one headset model.
#[derive(Debug)]
pub struct HeadsetModel {
    pub product_id: u16,
    pub name: &'static str,
    /// USB interface that carries the control endpoint.
    pub interface_number: i32,
    /// Usage page of the control collection on that interface, when the reference names one.
    pub usage_page: Option<u16>,
    /// Settings this model offers, in display order.
    pub capabilities: &'static [Capability],
    /// How to read battery, ChatMix and link state; `None` when the reference reads nothing.
    pub status: Option<StatusProtocol>,
    /// Where the protocol facts come from.
    pub source: &'static str,
}

impl HeadsetModel {
    /// The uniform settings descriptors for this model.
    pub fn descriptors(&self) -> Vec<SettingDescriptor> {
        self.capabilities.iter().map(Capability::descriptor).collect()
    }

    pub fn capability(&self, id: &str) -> Option<&'static Capability> {
        self.capabilities.iter().find(|c| c.id() == id)
    }
}

/// Look up a headset model by USB product ID.
pub fn model_for_product_id(product_id: u16) -> Option<&'static HeadsetModel> {
    MODELS.iter().find(|m| m.product_id == product_id)
}

/// Headset-specific functionality on top of the uniform settings model.
pub trait Headset: Device + Configurable {
    fn model(&self) -> &'static HeadsetModel;
}

/// The three HID operations a headset needs. Implemented for [`HidDevice`]; tests substitute a
/// recording fake.
pub trait HidTransport: Send {
    fn write_output(&self, data: &[u8]) -> Result<()>;
    fn write_feature(&self, data: &[u8]) -> Result<()>;
    /// Read one input report, waiting at most `timeout_ms`. Returns 0 when nothing arrived.
    fn read_timeout(&self, buf: &mut [u8], timeout_ms: i32) -> Result<usize>;
}

impl HidTransport for HidDevice {
    fn write_output(&self, data: &[u8]) -> Result<()> {
        self.write(data).map(|_| ()).map_err(Error::from)
    }

    fn write_feature(&self, data: &[u8]) -> Result<()> {
        self.send_feature_report(data).map_err(Error::from)
    }

    fn read_timeout(&self, buf: &mut [u8], timeout_ms: i32) -> Result<usize> {
        HidDevice::read_timeout(self, buf, timeout_ms).map_err(Error::from)
    }
}

/// Open the right headset implementation for an opened HID handle.
pub fn open(info: DeviceInfo, device: HidDevice) -> Result<Box<dyn Headset>> {
    let model = model_for_product_id(info.product_id).ok_or(Error::UnsupportedDevice {
        vendor_id: info.vendor_id,
        product_id: info.product_id,
    })?;
    Ok(Box::new(SteelSeriesHeadset::new(info, model, Box::new(device))))
}

/// The one headset implementation; everything model-specific comes from its [`HeadsetModel`].
pub struct SteelSeriesHeadset {
    info: DeviceInfo,
    model: &'static HeadsetModel,
    transport: Option<Mutex<Box<dyn HidTransport>>>,
    lighting: LightingState,
    /// Last ChatMix position the device pushed on its own (Nova Pro Wireless).
    last_chatmix: Option<ChatMix>,
}

impl SteelSeriesHeadset {
    pub fn new(info: DeviceInfo, model: &'static HeadsetModel, transport: Box<dyn HidTransport>) -> Self {
        Self {
            info,
            model,
            transport: Some(Mutex::new(transport)),
            lighting: LightingState::default(),
            last_chatmix: None,
        }
    }

    fn transport(&self) -> Result<parking_lot::MutexGuard<'_, Box<dyn HidTransport>>> {
        self.transport
            .as_ref()
            .map(Mutex::lock)
            .ok_or_else(|| Error::DeviceCommunication("Headset not connected".to_string()))
    }

    /// Write reports in order. Written directly rather than through `write_padded_report`, whose
    /// duplicate filter would drop a repeated save or status request sent within 50 ms.
    fn send(&self, reports: &[Report]) -> Result<()> {
        let transport = self.transport()?;
        for report in reports {
            debug!(
                "{}: sending {:?} report, {} bytes: {:02x?}",
                self.model.name,
                report.kind,
                report.bytes.len(),
                &report.bytes[..report.bytes.len().min(16)]
            );
            match report.kind {
                ReportKind::Output => transport.write_output(&report.bytes)?,
                ReportKind::Feature => transport.write_feature(&report.bytes)?,
            }
        }
        Ok(())
    }

    /// Note an unsolicited report if this model's protocol defines it.
    fn observe(&mut self, protocol: StatusProtocol, report: &[u8]) {
        if let Some(chatmix) = status::parse_event(protocol, report) {
            self.last_chatmix = Some(chatmix);
        }
    }

    /// Send `query` and wait up to [`READ_TIMEOUT`] for its answer, skipping unsolicited
    /// reports. Returns `None` when nothing acceptable arrived in time.
    fn run_query(&mut self, protocol: StatusProtocol, query: &Query) -> Result<Option<Vec<u8>>> {
        let mut buf = [0u8; READ_BUFFER_LEN];
        let mut seen = Vec::new();

        {
            let transport = self.transport()?;
            for _ in 0..MAX_DRAINED_REPORTS {
                let n = transport.read_timeout(&mut buf, 0)?;
                if n == 0 {
                    break;
                }
                seen.push(buf[..n.min(buf.len())].to_vec());
            }
        }
        for report in std::mem::take(&mut seen) {
            self.observe(protocol, &report);
        }

        self.send(std::slice::from_ref(&query.request))?;

        let deadline = Instant::now() + READ_TIMEOUT;
        let mut fallback: Option<Vec<u8>> = None;
        let mut answer: Option<Vec<u8>> = None;
        {
            let transport = self.transport()?;
            loop {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    break;
                }
                let timeout_ms = i32::try_from(remaining.as_millis()).unwrap_or(i32::MAX).max(1);
                let n = transport.read_timeout(&mut buf, timeout_ms)?;
                if n == 0 {
                    break;
                }
                let report = buf[..n.min(buf.len())].to_vec();
                seen.push(report.clone());
                if protocol.is_event(&report) {
                    continue;
                }
                if query.accept.matches(&report) {
                    answer = Some(report);
                    break;
                }
                if query.accept.allows_fallback() && fallback.is_none() {
                    fallback = Some(report);
                }
            }
        }
        for report in seen {
            self.observe(protocol, &report);
        }
        Ok(answer.or(fallback))
    }
}

impl Device for SteelSeriesHeadset {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn device_type(&self) -> DeviceType {
        DeviceType::Headset
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
        let timeout_ms = i32::try_from(READ_TIMEOUT.as_millis()).unwrap_or(i32::MAX);
        self.transport()?.read_timeout(buf, timeout_ms)
    }
}

impl Configurable for SteelSeriesHeadset {
    fn setting_descriptors(&self) -> Vec<SettingDescriptor> {
        self.model.descriptors()
    }

    fn apply_setting(&mut self, id: &str, value: &SettingValue) -> Result<()> {
        let capability = self
            .model
            .capability(id)
            .ok_or_else(|| Error::Unsupported(format!("setting '{id}' is not supported by the {}", self.model.name)))?;
        capability.descriptor().validate(value)?;
        let mut lighting = self.lighting;
        let reports = capability.encode(value, &mut lighting)?;
        self.send(&reports)?;
        self.lighting = lighting;
        Ok(())
    }

    fn read_status(&mut self) -> Result<DeviceStatus> {
        let mut status = DeviceStatus::default();
        if let Some(protocol) = self.model.status {
            for query in protocol.queries() {
                let Some(answer) = self.run_query(protocol, &query)? else {
                    debug!("{}: no answer to {:02x?}", self.model.name, &query.request.bytes[..2]);
                    continue;
                };
                if query.parser.parse(&answer, &mut status) == Parsed::Stop {
                    break;
                }
            }
        }
        if status.chatmix.is_none() {
            status.chatmix = self.last_chatmix;
        }
        Ok(status)
    }
}

impl Headset for SteelSeriesHeadset {
    fn model(&self) -> &'static HeadsetModel {
        self.model
    }
}

#[cfg(test)]
mod tests;
