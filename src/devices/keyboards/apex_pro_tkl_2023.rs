//! Apex Pro TKL 2023 specific implementations.

use super::{GenericKeyboard, Keyboard};
use crate::Result;
#[cfg(feature = "experimental-apex-2023")]
use crate::devices::hid_reports::APEX_2023_PERKEY_REPORT_SIZE;
use crate::devices::hid_reports::{
    ActuationCommand, ApexDirectPacket, HidCommand, HidDeviceType, HidReportBuilder, KEYBOARD_REPORT_SIZE,
};
use crate::devices::key_mapping::{KeyAddress, KeyId, KeyMapping};
use crate::devices::product_ids;
use crate::devices::settings::{Configurable, SettingDescriptor, SettingValue, Verification};
use crate::devices::zone_mapping::{ZoneEffect, ZoneMapping};
use crate::devices::{Device, DeviceInfo, DeviceType};
use crate::rgb::{Color, PerKeyEffect};
use async_trait::async_trait;
use std::ops::{Deref, DerefMut};

/// Apex Pro TKL 2023 implementation.
///
/// Also wraps the 2023 wireless pair (`0x1630`, `0x1632`) and the Gen 3 TKL models (`0x1642`,
/// `0x1644`, `0x1646`). Their 643-byte direct protocol (0x4B init, then 0x40 / 0x61 frames) is
/// sent by the inner `GenericKeyboard`'s per-key family, with the same bytes and transport this
/// type used to send itself (see `protocol::ApexPerKeyProtocol::tkl_2023`).
pub struct ApexProTkl2023 {
    inner: GenericKeyboard,
}

impl ApexProTkl2023 {
    /// Product ID for Apex Pro TKL 2023.
    pub const PRODUCT_ID: u16 = 0x1628;

    /// Create from a generic keyboard.
    pub fn new(keyboard: GenericKeyboard) -> Self {
        Self { inner: keyboard }
    }

    /// Create for wireless model without a hidapi device handle.
    /// Uses raw hidraw feature reports only.
    pub fn new_wireless_raw(info: DeviceInfo) -> Self {
        Self {
            inner: GenericKeyboard::new_without_device(info),
        }
    }

    /// Whether this device uses the new 2023 per-key direct protocol.
    /// The 2023 wired model (`0x1628`) is excluded: it keeps its own path behind
    /// the `experimental-apex-2023` feature flag.
    fn uses_new_protocol(&self) -> bool {
        self.direct_packet_id().is_some()
    }

    /// Whether this is a wireless variant.
    fn is_wireless(&self) -> bool {
        matches!(
            self.inner.info().product_id,
            product_ids::APEX_PRO_TKL_2023_WIRELESS
                | product_ids::APEX_PRO_TKL_2023_WIRELESS_2
                | product_ids::APEX_PRO_TKL_WIRELESS_2024_DONGLE
                | product_ids::APEX_PRO_TKL_WIRELESS_2024
        )
    }

    /// Direct-mode packet ID, or `None` if the model does not speak the 643-byte
    /// feature-report protocol.
    ///
    /// Verified on Apex Pro TKL Wireless Gen 3 hardware: the dongle (`0x1644`) and
    /// the same keyboard on USB cable (`0x1646`) both answer to the wireless packet
    /// ID `0x61`, while the wired-only Gen 3 model (`0x1642`) uses `0x40`.
    fn direct_packet_id(&self) -> Option<u8> {
        Some(match self.inner.info().product_id {
            product_ids::APEX_PRO_TKL_2023_WIRELESS
            | product_ids::APEX_PRO_TKL_2023_WIRELESS_2
            | product_ids::APEX_PRO_TKL_WIRELESS_2024_DONGLE
            | product_ids::APEX_PRO_TKL_WIRELESS_2024 => ApexDirectPacket::Wireless2023.byte(),
            product_ids::APEX_PRO_TKL_2024 => ApexDirectPacket::Wired2023.byte(),
            _ => return None,
        })
    }

    /// Set actuation point for all keys (global).
    /// Value is in 0.1mm increments (e.g. 4 = 0.4mm, 36 = 3.6mm).
    /// Send actuation command and update cache
    fn send_actuation_command(&mut self, command: ActuationCommand) -> Result<()> {
        let report_builder = HidReportBuilder::new(HidDeviceType::Keyboard);
        let mut buffer = [0u8; KEYBOARD_REPORT_SIZE];
        let size = report_builder.build_report(command.clone(), &mut buffer)?;
        self.inner.send_raw(&buffer[..size])?;
        self.inner.update_cached_actuation_point(command.actuation_point);
        Ok(())
    }

    pub fn set_actuation_point(&mut self, value: u8) -> Result<()> {
        let command = ActuationCommand::new(value);
        command.validate()?;
        self.send_actuation_command(command)
    }

    /// Set actuation point in millimeters.
    /// Precision is limited to 0.1mm increments.
    pub fn set_actuation_point_mm(&mut self, mm: f32) -> Result<()> {
        let command = ActuationCommand::from_mm(mm);
        self.set_actuation_point(command.actuation_point)
    }

    #[cfg(feature = "experimental-apex-2023")]
    const fn experimental_direct_key_id(key_id: KeyId) -> Option<u8> {
        Some(match key_id {
            KeyId::A => 0x04,
            KeyId::B => 0x05,
            KeyId::C => 0x06,
            KeyId::D => 0x07,
            KeyId::E => 0x08,
            KeyId::F => 0x09,
            KeyId::G => 0x0A,
            KeyId::H => 0x0B,
            KeyId::I => 0x0C,
            KeyId::J => 0x0D,
            KeyId::K => 0x0E,
            KeyId::L => 0x0F,
            KeyId::M => 0x10,
            KeyId::N => 0x11,
            KeyId::O => 0x12,
            KeyId::P => 0x13,
            KeyId::Q => 0x14,
            KeyId::R => 0x15,
            KeyId::S => 0x16,
            KeyId::T => 0x17,
            KeyId::U => 0x18,
            KeyId::V => 0x19,
            KeyId::W => 0x1A,
            KeyId::X => 0x1B,
            KeyId::Y => 0x1C,
            KeyId::Z => 0x1D,
            KeyId::Key1 => 0x1E,
            KeyId::Key2 => 0x1F,
            KeyId::Key3 => 0x20,
            KeyId::Key4 => 0x21,
            KeyId::Key5 => 0x22,
            KeyId::Key6 => 0x23,
            KeyId::Key7 => 0x24,
            KeyId::Key8 => 0x25,
            KeyId::Key9 => 0x26,
            KeyId::Key0 => 0x27,
            KeyId::Enter => 0x28,
            KeyId::Escape => 0x29,
            KeyId::Backspace => 0x2A,
            KeyId::Tab => 0x2B,
            KeyId::Space => 0x2C,
            KeyId::Minus => 0x2D,
            KeyId::Equal => 0x2E,
            KeyId::LeftBracket => 0x2F,
            KeyId::RightBracket => 0x30,
            KeyId::Backslash => 0x31,
            KeyId::Semicolon => 0x33,
            KeyId::Quote => 0x34,
            KeyId::Backtick => 0x35,
            KeyId::Comma => 0x36,
            KeyId::Period => 0x37,
            KeyId::Slash => 0x38,
            KeyId::CapsLock => 0x39,
            KeyId::F1 => 0x3A,
            KeyId::F2 => 0x3B,
            KeyId::F3 => 0x3C,
            KeyId::F4 => 0x3D,
            KeyId::F5 => 0x3E,
            KeyId::F6 => 0x3F,
            KeyId::F7 => 0x40,
            KeyId::F8 => 0x41,
            KeyId::F9 => 0x42,
            KeyId::F10 => 0x43,
            KeyId::F11 => 0x44,
            KeyId::F12 => 0x45,
            KeyId::Insert => 0x49,
            KeyId::Home => 0x4A,
            KeyId::PageUp => 0x4B,
            KeyId::Delete => 0x4C,
            KeyId::End => 0x4D,
            KeyId::PageDown => 0x4E,
            KeyId::ArrowRight => 0x4F,
            KeyId::ArrowLeft => 0x50,
            KeyId::ArrowDown => 0x51,
            KeyId::ArrowUp => 0x52,
            KeyId::Menu => 0x65,
            KeyId::LeftCtrl => 0xE0,
            KeyId::LeftShift => 0xE1,
            KeyId::LeftAlt => 0xE2,
            KeyId::LeftWin => 0xE3,
            KeyId::RightCtrl => 0xE4,
            KeyId::RightShift => 0xE5,
            KeyId::RightAlt => 0xE6,
            KeyId::RightWin => 0xE7,
            KeyId::SteelSeriesKey | KeyId::VolumeWheel => return None,
            // Not part of the 0x1628 capture's 84-key list.
            KeyId::PrintScreen
            | KeyId::ScrollLock
            | KeyId::Pause
            | KeyId::NonUsHash
            | KeyId::NonUsBackslash
            | KeyId::JpRo
            | KeyId::JpKana
            | KeyId::JpYen
            | KeyId::JpHenkan
            | KeyId::JpMuhenkan
            | KeyId::MediaPlayPause => return None,
            KeyId::NumLock
            | KeyId::NumSlash
            | KeyId::NumAsterisk
            | KeyId::NumMinus
            | KeyId::Num7
            | KeyId::Num8
            | KeyId::Num9
            | KeyId::NumPlus
            | KeyId::Num4
            | KeyId::Num5
            | KeyId::Num6
            | KeyId::Num1
            | KeyId::Num2
            | KeyId::Num3
            | KeyId::NumEnter
            | KeyId::Num0
            | KeyId::NumPeriod => return None,
        })
    }

    #[cfg(feature = "experimental-apex-2023")]
    fn build_experimental_direct_command(
        &self,
        key_colors: &[(KeyId, Color)],
    ) -> Option<crate::devices::hid_reports::Apex2023DirectCommand> {
        use crate::devices::hid_reports::Apex2023DirectCommand;

        if key_colors.is_empty() {
            return None;
        }

        let mut command = Apex2023DirectCommand::new();
        for (key_id, color) in key_colors {
            let Some(direct_key_id) = Self::experimental_direct_key_id(*key_id) else {
                // Key not addressable via the 0x40 protocol (e.g. SteelSeries key, volume wheel)
                continue;
            };
            command.set_key_color(direct_key_id, *color);
        }

        if command.is_empty() {
            return None;
        }

        Some(command)
    }
}

// Delegate Device trait
impl Device for ApexProTkl2023 {
    fn info(&self) -> &DeviceInfo {
        self.inner.info()
    }

    fn device_type(&self) -> DeviceType {
        self.inner.device_type()
    }

    fn initialize(&mut self) -> Result<()> {
        // Wireless variant uses feature reports for init, not output reports.
        // The 0x4B init is sent lazily in ensure_new_protocol_init().
        if self.is_wireless() {
            return Ok(());
        }
        self.inner.initialize()
    }

    fn close(&mut self) -> Result<()> {
        self.inner.close()
    }

    fn is_connected(&self) -> bool {
        self.inner.is_connected()
    }

    fn send_raw(&mut self, data: &[u8]) -> Result<()> {
        self.inner.send_raw(data)
    }

    fn receive_raw(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.inner.receive_raw(buf)
    }
}

// Delegate Keyboard trait
crate::impl_keyboard_with_delegation!(ApexProTkl2023, {
    // The inner keyboard's family decides the encoding: legacy 0x21 zones on 0x1628, the
    // whole-keyboard direct frame (first colour on every key) on the new-protocol models.
    async fn set_color(&mut self, color: Color) -> Result<()> {
        self.inner.set_color(color).await
    }

    async fn set_zone_colors(&mut self, colors: &[Color]) -> Result<()> {
        self.inner.set_zone_colors(colors).await
    }

    async fn apply(&mut self) -> Result<()> {
        // Direct mode is live-streamed. The APPLY/save command (0x09) makes the
        // controller fall back to its stored profile, discarding the colours we
        // just sent, so the board visibly ignores every RGB command.
        if self.uses_new_protocol() {
            return Ok(());
        }
        self.inner.apply().await
    }

    async fn set_key_color(&mut self, key_id: KeyId, color: Color) -> Result<()> {
        self.set_key_colors(&[(key_id, color)]).await
    }

    async fn set_key_colors(&mut self, key_colors: &[(KeyId, Color)]) -> Result<()> {
        // The 0x40 command of the feature-gated path is the 0x1628 capture; the new-protocol
        // models get their own direct frame from the inner keyboard.
        #[cfg(feature = "experimental-apex-2023")]
        if !self.uses_new_protocol()
            && let Some(command) = self.build_experimental_direct_command(key_colors)
        {
            let report_builder = HidReportBuilder::new(HidDeviceType::Keyboard);
            let mut buffer = [0u8; APEX_2023_PERKEY_REPORT_SIZE];
            let size = report_builder.build_report(command, &mut buffer)?;
            return self.inner.send_raw(&buffer[..size]);
        }

        self.inner.set_key_colors(key_colors).await
    }

    async fn set_all_key_colors(&mut self, colors: &[(KeyId, Color)]) -> Result<()> {
        if self.uses_new_protocol() {
            return self.inner.set_all_key_colors(colors).await;
        }
        self.set_key_colors(colors).await
    }

    fn read_actuation_point(&mut self) -> Result<u8> {
        self.inner.read_actuation_point()
    }

    fn set_actuation_point(&mut self, value: u8) -> Result<()> {
        self.set_actuation_point(value)
    }

    fn set_actuation_point_mm(&mut self, mm: f32) -> Result<()> {
        self.set_actuation_point_mm(mm)
    }
});

impl Configurable for ApexProTkl2023 {
    /// The inner keyboard's settings. On `0x1628` the brightness setting is the legacy `0x22`
    /// command, which the protocol notes record as tested on this model.
    fn setting_descriptors(&self) -> Vec<SettingDescriptor> {
        let mut descriptors = self.inner.setting_descriptors();
        if !self.uses_new_protocol() {
            for descriptor in &mut descriptors {
                if descriptor.id == super::setting_ids::BRIGHTNESS {
                    descriptor.verification = Verification::Hardware;
                    descriptor.description = "Backlight brightness, 0x22 command, tested on this model.".to_string();
                }
            }
        }
        descriptors
    }

    fn apply_setting(&mut self, id: &str, value: &SettingValue) -> Result<()> {
        self.inner.apply_setting(id, value)
    }
}

// Deref allows access to Device trait methods like send_raw/receive_raw
impl Deref for ApexProTkl2023 {
    type Target = GenericKeyboard;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for ApexProTkl2023 {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::keyboards::protocol::KeyboardFamily;

    fn keyboard_for(product_id: u16) -> ApexProTkl2023 {
        let info = DeviceInfo {
            name: std::borrow::Cow::Borrowed("Test Apex Pro TKL"),
            device_type: DeviceType::Keyboard,
            vendor_id: crate::STEELSERIES_VENDOR_ID,
            product_id,
            interface_number: 3,
            usage_page: 0xFFC0,
            usage: 0x01,
            serial_number: None,
            manufacturer: Some("SteelSeries".to_string()),
            path: "/test/device/path".to_string(),
        };
        ApexProTkl2023::new_wireless_raw(info)
    }

    #[test]
    fn gen3_models_use_the_direct_protocol() {
        for pid in [
            product_ids::APEX_PRO_TKL_2024,
            product_ids::APEX_PRO_TKL_WIRELESS_2024_DONGLE,
            product_ids::APEX_PRO_TKL_WIRELESS_2024,
        ] {
            assert!(
                keyboard_for(pid).uses_new_protocol(),
                "PID {pid:#06x} should use the 643-byte direct protocol"
            );
        }
    }

    #[test]
    fn gen3_packet_ids_match_hardware() {
        // Verified on hardware: the dongle and the same board on cable both answer
        // to the wireless packet ID; the wired-only model uses 0x40.
        assert_eq!(
            keyboard_for(product_ids::APEX_PRO_TKL_WIRELESS_2024_DONGLE).direct_packet_id(),
            Some(0x61)
        );
        assert_eq!(
            keyboard_for(product_ids::APEX_PRO_TKL_WIRELESS_2024).direct_packet_id(),
            Some(0x61)
        );
        assert_eq!(
            keyboard_for(product_ids::APEX_PRO_TKL_2024).direct_packet_id(),
            Some(0x40)
        );
    }

    #[test]
    fn wired_2023_keeps_its_own_path() {
        let keyboard = keyboard_for(product_ids::APEX_PRO_TKL_2023);
        assert!(!keyboard.uses_new_protocol());
        assert_eq!(keyboard.direct_packet_id(), None);
        assert_eq!(keyboard.inner.family(), KeyboardFamily::Legacy);
    }

    /// The key list this type sent before the direct protocol moved into the inner keyboard
    /// (from OpenRGB `keys[]`; `0xFB` is the logo LED, omitting it leaves the logo lit).
    const PR_290_TKL_KEYS: [u8; 112] = [
        0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15,
        0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27,
        0x28, 0x29, 0x2A, 0x2B, 0x2C, 0x2D, 0x2E, 0x2F, 0x30, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A,
        0x3B, 0x3C, 0x3D, 0x3E, 0x3F, 0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A, 0x4B, 0x4C,
        0x4D, 0x4E, 0x4F, 0x50, 0x51, 0x52, 0x64, 0xE0, 0xE1, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xF0, 0x31, 0x87,
        0x88, 0x89, 0x8A, 0x8B, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x5B, 0x5C, 0x5D, 0x5E, 0x5F, 0x60,
        0x61, 0x62, 0x63, 0xFB,
    ];

    /// The 643-byte single-colour frame this type built by hand before.
    fn pr_290_frame(packet_id: u8, color: Color) -> Vec<u8> {
        let mut buf = vec![0u8; 643];
        buf[1] = packet_id;
        buf[2] = PR_290_TKL_KEYS.len() as u8;
        for (i, &hid_code) in PR_290_TKL_KEYS.iter().enumerate() {
            let offset = 3 + i * 4;
            buf[offset] = hid_code;
            buf[offset + 1] = color.r;
            buf[offset + 2] = color.g;
            buf[offset + 3] = color.b;
        }
        buf
    }

    #[test]
    fn new_protocol_bytes_are_unchanged() {
        use crate::devices::hid_reports::ApexInitCommand;
        use crate::devices::key_mapping::APEX_LED_TABLE;
        use crate::devices::keyboards::protocol::{ApexFrame, FeatureTarget};

        let led_hid: Vec<u8> = APEX_LED_TABLE.iter().map(|(_, hid)| *hid).collect();
        assert_eq!(led_hid, PR_290_TKL_KEYS, "shared LED table must keep the PR #290 order");

        let builder = HidReportBuilder::new(HidDeviceType::Keyboard);
        let color = Color::new(0x12, 0x34, 0x56);
        for pid in [
            product_ids::APEX_PRO_TKL_2023_WIRELESS,
            product_ids::APEX_PRO_TKL_2023_WIRELESS_2,
            product_ids::APEX_PRO_TKL_2024,
            product_ids::APEX_PRO_TKL_WIRELESS_2024_DONGLE,
            product_ids::APEX_PRO_TKL_WIRELESS_2024,
        ] {
            let keyboard = keyboard_for(pid);
            let KeyboardFamily::ApexPerKey(protocol) = keyboard.inner.family() else {
                panic!("PID {pid:#06x} must use the per-key family");
            };
            assert_eq!(Some(protocol.direct_packet().byte()), keyboard.direct_packet_id());
            assert_eq!(protocol.feature_target, FeatureTarget::Interface3);
            assert_eq!(protocol.init_settle_ms, 50);

            let mut frame = ApexFrame::new();
            frame.fill(color);
            let mut buffer = vec![0u8; protocol.report_len];
            let size = builder
                .build_report(
                    frame.direct_command(protocol.direct_packet(), protocol.report_len),
                    &mut buffer,
                )
                .unwrap();
            assert_eq!(buffer[..size], pr_290_frame(protocol.direct_packet().byte(), color)[..]);

            let mut init = vec![0u8; protocol.init_report_len];
            let size = builder
                .build_report(ApexInitCommand::new(protocol.init_report_len), &mut init)
                .unwrap();
            let mut expected = vec![0u8; 643];
            expected[1] = 0x4B;
            assert_eq!(init[..size], expected[..]);
        }
    }

    #[test]
    fn brightness_is_hardware_only_on_0x1628() {
        let wired = keyboard_for(product_ids::APEX_PRO_TKL_2023);
        let brightness = wired
            .setting_descriptors()
            .into_iter()
            .find(|d| d.id == "brightness")
            .unwrap();
        assert_eq!(brightness.verification, Verification::Hardware);

        // Without a device the new-protocol models cannot answer the read-back request, so
        // brightness is not offered.
        let gen3 = keyboard_for(product_ids::APEX_PRO_TKL_2024);
        assert!(gen3.setting_descriptors().iter().all(|d| d.id != "brightness"));
    }

    #[test]
    fn actuation_settings_per_model() {
        let ids = |pid| -> Vec<String> {
            keyboard_for(pid)
                .setting_descriptors()
                .into_iter()
                .filter(|d| d.id.starts_with("actuation"))
                .map(|d| d.id)
                .collect()
        };
        assert_eq!(ids(product_ids::APEX_PRO_TKL_2023), ["actuation", "actuation_live"]);
        assert_eq!(ids(product_ids::APEX_PRO_TKL_2024), ["actuation"]);
    }
}
