//! Mouse support (Rival, Sensei, Aerox, Prime series).
//!
//! Skeleton: the model table and protocol live here; `discovery.rs` only asks
//! [`model_for_product_id`] whether a PID is a mouse and which interface to open, then hands the
//! opened HID handle to [`open`].

use hidapi::HidDevice;

use super::settings::Configurable;
use super::{Device, DeviceInfo};
use crate::{Error, Result};

/// Static description of one mouse model.
#[derive(Clone, Copy, Debug)]
pub struct MouseModel {
    pub product_id: u16,
    pub name: &'static str,
    /// USB interface number that carries the vendor control endpoint.
    pub interface_number: i32,
}

/// Every known mouse model.
pub static MODELS: &[MouseModel] = &[];

/// Look up a mouse model by USB product ID.
pub fn model_for_product_id(product_id: u16) -> Option<&'static MouseModel> {
    MODELS.iter().find(|m| m.product_id == product_id)
}

/// Mouse-specific functionality on top of the uniform settings model.
pub trait Mouse: Device + Configurable {
    fn model(&self) -> &'static MouseModel;
}

/// Wrap an opened HID handle in the right mouse implementation.
pub fn open(info: DeviceInfo, _device: HidDevice) -> Result<Box<dyn Mouse>> {
    Err(Error::UnsupportedDevice {
        vendor_id: info.vendor_id,
        product_id: info.product_id,
    })
}
