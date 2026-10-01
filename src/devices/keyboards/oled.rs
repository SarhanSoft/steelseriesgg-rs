//! OLED screen protocol for SteelSeries Apex keyboards. `[EXPERIMENTAL]`
//!
//! None of this has been run on hardware by this project. Each model's
//! [`Verification`] says how much the byte layout can be trusted:
//! [`Verification::Reference`] means a published driver whose users report it working sends
//! exactly these bytes; [`Verification::Guess`] means no reference covers the model and the
//! layout is copied from the nearest sibling.
//!
//! # Report layouts
//!
//! Every screen is 128x40, one bit per pixel, 640 bytes of pixel data per frame. All reports
//! are HID *feature* reports; byte 0 of each [`OledReport`] is the HID report ID, as
//! `HIDIOCSFEATURE` / `hid_send_feature_report` expect.
//!
//! | Layout | Reports | Bytes |
//! |---|---|---|
//! | [`OledProtocol::RowMajor`] | 1 x 642 | `61` + 640 row-major pixel bytes + `00` |
//! | [`OledProtocol::RowMajorApex7`] | 1 x 642 | `00` (report ID) + `65` + 640 row-major pixel bytes |
//! | [`OledProtocol::PageMajor`] | 1 x 642 | `61` + 640 page-major pixel bytes + `00` |
//! | [`OledProtocol::Chunked`] | 8 x 641 | `cmd 01 off_lo off_hi 50 00` + 80 page-major bytes + zeros |
//!
//! - *Row-major*: the [`OledFrame`] packing — 16 bytes per row, bit 7 = leftmost pixel.
//! - *Page-major* (SSD1306 order): 5 pages of 8 rows; byte `page * 128 + x` holds column `x` of
//!   that page, bit 0 = top row of the page.
//!
//! Sources: apex-tux (public domain) `apex-hardware/src/usb.rs` and `src/device.rs` for
//! `RowMajor`, `PageMajor` and `Chunked`; apex7tkl_linux `device.py` / `oled.py` (facts only)
//! for `RowMajorApex7`.
//!
//! # Giving the screen back
//!
//! No reference documents a command that hands the screen back to the keyboard's own menu or
//! idle image. apex-tux sends nothing on shutdown. The last frame stays until it is replaced;
//! whether the firmware redraws its own UI on its own (for example when the on-board menu is
//! opened) is not documented anywhere we could find.

use crate::devices::DeviceInfo;
use crate::devices::product_ids;
use crate::devices::settings::Verification;
use crate::oled::OledFrame;
use crate::{Error, Result};

/// Width in pixels of every Apex OLED screen.
pub const OLED_WIDTH: u32 = 128;
/// Height in pixels of every Apex OLED screen.
pub const OLED_HEIGHT: u32 = 40;
/// Pixel bytes in one 128x40 frame.
pub const OLED_FRAME_BYTES: usize = (OLED_WIDTH as usize / 8) * OLED_HEIGHT as usize;

/// Report ID of the single-report image upload (apex-tux `FrameBuffer` header byte).
pub const OLED_IMAGE_REPORT_ID: u8 = 0x61;
/// Length of the single-report image upload, report ID included.
pub const OLED_IMAGE_REPORT_LEN: usize = 1 + OLED_FRAME_BYTES + 1;
/// Command byte apex7tkl_linux sends (after report ID 0) to draw on the Apex 7 / 7 TKL.
pub const OLED_APEX7_COMMAND: u8 = 0x65;
/// Length of the Apex 7 image upload: report ID 0, command, pixels.
pub const OLED_APEX7_REPORT_LEN: usize = 1 + 1 + OLED_FRAME_BYTES;
/// Chunked upload command for the wired path (Apex Pro TKL Wireless Gen 3 on USB cable).
pub const OLED_CHUNK_COMMAND_WIRED: u8 = 0x0C;
/// Chunked upload command through the 2.4 GHz dongle.
pub const OLED_CHUNK_COMMAND_DONGLE: u8 = 0x4C;
/// Sub-command byte of every chunk.
pub const OLED_CHUNK_SUBCOMMAND: u8 = 0x01;
/// Pixel bytes per chunk.
pub const OLED_CHUNK_DATA_LEN: usize = 80;
/// Chunks per frame.
pub const OLED_CHUNK_COUNT: usize = OLED_FRAME_BYTES / OLED_CHUNK_DATA_LEN;
/// Header bytes before a chunk's pixel data.
pub const OLED_CHUNK_HEADER_LEN: usize = 6;
/// Length of one chunk report, report ID (the command byte) included.
pub const OLED_CHUNK_REPORT_LEN: usize = 641;

/// How a keyboard wants its OLED frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OledProtocol {
    /// One report, row-major pixels (apex-tux `DeviceProtocol::Legacy`). Interface 1.
    RowMajor,
    /// One report, report ID 0, command `0x65`, row-major pixels (apex7tkl_linux). Interface 1.
    RowMajorApex7,
    /// One report, page-major pixels (apex-tux `DeviceProtocol::ApexProGen3`). Interface 1.
    PageMajor,
    /// Eight 80-byte page-major chunks (apex-tux `DeviceProtocol::ChunkedGen3`). Interface 3.
    /// `command` is [`OLED_CHUNK_COMMAND_WIRED`] or [`OLED_CHUNK_COMMAND_DONGLE`].
    Chunked { command: u8 },
}

impl OledProtocol {
    /// USB interface that accepts the OLED reports (apex-tux `oled_interface`).
    pub const fn interface(self) -> u8 {
        match self {
            OledProtocol::Chunked { .. } => 3,
            OledProtocol::RowMajor | OledProtocol::RowMajorApex7 | OledProtocol::PageMajor => 1,
        }
    }
}

/// One keyboard with an OLED screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OledModel {
    pub product_id: u16,
    pub name: &'static str,
    pub width: u32,
    pub height: u32,
    pub protocol: OledProtocol,
    pub verification: Verification,
    /// Where the layout comes from, or what it was extrapolated from.
    pub source: &'static str,
}

impl OledModel {
    /// `(width, height)` in pixels.
    pub const fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// USB interface that accepts the OLED reports.
    pub const fn interface(&self) -> u8 {
        self.protocol.interface()
    }
}

const fn model(
    product_id: u16,
    name: &'static str,
    protocol: OledProtocol,
    verification: Verification,
    source: &'static str,
) -> OledModel {
    OledModel {
        product_id,
        name,
        width: OLED_WIDTH,
        height: OLED_HEIGHT,
        protocol,
        verification,
        source,
    }
}

const CHUNKED_WIRED: OledProtocol = OledProtocol::Chunked {
    command: OLED_CHUNK_COMMAND_WIRED,
};
const CHUNKED_DONGLE: OledProtocol = OledProtocol::Chunked {
    command: OLED_CHUNK_COMMAND_DONGLE,
};

/// Every keyboard known to have an OLED screen. Models absent from this table have none
/// (Apex 3, Apex 3 TKL, Apex 150, Apex 9 Mini / TKL, Apex Pro Mini in all variants).
const MODELS: &[OledModel] = &[
    model(
        product_ids::APEX_PRO,
        "Apex Pro",
        OledProtocol::RowMajor,
        Verification::Reference,
        "apex-tux README lists it as supported; usb.rs Legacy path",
    ),
    model(
        product_ids::APEX_7,
        "Apex 7",
        OledProtocol::RowMajor,
        Verification::Reference,
        "apex-tux README lists it as supported (apex7tkl_linux drives the same PID with command 0x65)",
    ),
    model(
        product_ids::APEX_PRO_TKL,
        "Apex Pro TKL",
        OledProtocol::RowMajor,
        Verification::Reference,
        "apex-tux usb.rs: the one entry above its 'Never tested' marker",
    ),
    model(
        product_ids::APEX_7_TKL,
        "Apex 7 TKL",
        OledProtocol::RowMajorApex7,
        Verification::Reference,
        "apex7tkl_linux device.py: SET_REPORT feature, wValue 0x0300, interface 1, 0x65 + 640 bytes",
    ),
    model(
        product_ids::APEX_5,
        "Apex 5",
        OledProtocol::RowMajor,
        Verification::Reference,
        "apex-tux README lists it as supported; usb.rs Legacy path",
    ),
    model(
        product_ids::APEX_PRO_TKL_2023,
        "Apex Pro TKL (2023)",
        OledProtocol::PageMajor,
        Verification::Guess,
        "no working reference; apex-tux issue #52 reports the Legacy layout failing on this PID, \
         so the wired-only Gen 3 layout (0x1640) is assumed",
    ),
    model(
        product_ids::APEX_PRO_TKL_2023_WIRELESS_2,
        "Apex Pro TKL Wireless (2023), dongle",
        CHUNKED_DONGLE,
        Verification::Guess,
        "no reference; copied from the Gen 3 dongle 0x1644, which shares its RGB direct packet (0x61)",
    ),
    model(
        product_ids::APEX_PRO_TKL_2023_WIRELESS,
        "Apex Pro TKL Wireless (2023), USB cable",
        CHUNKED_WIRED,
        Verification::Guess,
        "no reference; copied from the Gen 3 board on cable 0x1646",
    ),
    model(
        product_ids::APEX_PRO_2024,
        "Apex Pro (Gen 3)",
        OledProtocol::PageMajor,
        Verification::Reference,
        "apex-tux PR #78, 'support for apex pro gen 3 (wired full size)'",
    ),
    model(
        product_ids::APEX_PRO_TKL_2024,
        "Apex Pro TKL (Gen 3)",
        OledProtocol::PageMajor,
        Verification::Guess,
        "no reference; copied from the wired-only Gen 3 full-size board 0x1640",
    ),
    model(
        product_ids::APEX_PRO_TKL_WIRELESS_2024_DONGLE,
        "Apex Pro TKL Wireless (Gen 3), dongle",
        CHUNKED_DONGLE,
        Verification::Reference,
        "apex-tux PR #74: captured from GG with Wireshark, tested through the dongle",
    ),
    model(
        product_ids::APEX_PRO_TKL_WIRELESS_2024,
        "Apex Pro TKL Wireless (Gen 3), USB cable",
        CHUNKED_WIRED,
        Verification::Reference,
        "apex-tux PR #74: captured from GG with Wireshark, tested on USB cable",
    ),
    model(
        product_ids::APEX_5_2024,
        "Apex 5 (2024)",
        OledProtocol::PageMajor,
        Verification::Guess,
        "no reference; assumes the 128x40 screen of the Apex 5 and the wired Gen 3 layout of 0x1640",
    ),
    model(
        product_ids::APEX_7_2024,
        "Apex 7 (2024)",
        OledProtocol::PageMajor,
        Verification::Guess,
        "no reference; assumes the 128x40 screen of the Apex 7 and the wired Gen 3 layout of 0x1640",
    ),
];

/// Every keyboard known to have an OLED screen.
pub fn oled_models() -> &'static [OledModel] {
    MODELS
}

/// The OLED description of a keyboard, or `None` when it has no screen.
pub fn oled_model_for_product_id(product_id: u16) -> Option<&'static OledModel> {
    MODELS.iter().find(|m| m.product_id == product_id)
}

/// `(width, height)` of a keyboard's OLED screen, or `None` when it has no screen.
pub fn oled_size_for_product_id(product_id: u16) -> Option<(u32, u32)> {
    oled_model_for_product_id(product_id).map(OledModel::size)
}

/// How far the OLED layout of a keyboard can be trusted, or `None` when it has no screen.
pub fn oled_verification_for_product_id(product_id: u16) -> Option<Verification> {
    oled_model_for_product_id(product_id).map(|m| m.verification)
}

/// One OLED feature report. Byte 0 is the HID report ID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OledReport {
    bytes: Vec<u8>,
}

impl OledReport {
    /// The whole report, report ID first.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Report length, report ID included.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Always `false`; every OLED report has a fixed, non-zero length.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// The HID report ID (byte 0).
    pub fn report_id(&self) -> u8 {
        self.bytes.first().copied().unwrap_or(0)
    }
}

fn check_size(frame: &OledFrame) -> Result<()> {
    if frame.size() == (OLED_WIDTH, OLED_HEIGHT) && frame.data().len() == OLED_FRAME_BYTES {
        Ok(())
    } else {
        Err(Error::InvalidConfig(format!(
            "OLED frame is {}x{}, the screen is {OLED_WIDTH}x{OLED_HEIGHT}",
            frame.width(),
            frame.height()
        )))
    }
}

/// The frame's pixels in SSD1306 page-major order: byte `page * 128 + x`, bit `y % 8`, with
/// `page = y / 8` (apex-tux `encode_apex_pro_gen3_report`).
pub fn page_major_bytes(frame: &OledFrame) -> Result<Vec<u8>> {
    check_size(frame)?;
    let mut out = vec![0u8; OLED_FRAME_BYTES];
    for y in 0..OLED_HEIGHT {
        let page = (y / 8) as usize;
        let mask = 1u8 << (y % 8);
        for x in 0..OLED_WIDTH {
            if frame.get_pixel(x, y) {
                out[page * OLED_WIDTH as usize + x as usize] |= mask;
            }
        }
    }
    Ok(out)
}

/// [`OledProtocol::RowMajor`]: `61` + 640 row-major bytes + `00`.
pub fn encode_row_major(frame: &OledFrame) -> Result<OledReport> {
    check_size(frame)?;
    let mut bytes = Vec::with_capacity(OLED_IMAGE_REPORT_LEN);
    bytes.push(OLED_IMAGE_REPORT_ID);
    bytes.extend_from_slice(frame.data());
    bytes.push(0x00);
    Ok(OledReport { bytes })
}

/// [`OledProtocol::RowMajorApex7`]: `00` (report ID) + `65` + 640 row-major bytes.
pub fn encode_row_major_apex7(frame: &OledFrame) -> Result<OledReport> {
    check_size(frame)?;
    let mut bytes = Vec::with_capacity(OLED_APEX7_REPORT_LEN);
    bytes.push(0x00);
    bytes.push(OLED_APEX7_COMMAND);
    bytes.extend_from_slice(frame.data());
    Ok(OledReport { bytes })
}

/// [`OledProtocol::PageMajor`]: `61` + 640 page-major bytes + `00`.
pub fn encode_page_major(frame: &OledFrame) -> Result<OledReport> {
    let pixels = page_major_bytes(frame)?;
    let mut bytes = Vec::with_capacity(OLED_IMAGE_REPORT_LEN);
    bytes.push(OLED_IMAGE_REPORT_ID);
    bytes.extend_from_slice(&pixels);
    bytes.push(0x00);
    Ok(OledReport { bytes })
}

/// [`OledProtocol::Chunked`]: eight 641-byte reports. Report `i` is
/// `command 01 off_lo off_hi 50 00` + page-major bytes `i * 80 .. i * 80 + 80` + zero padding,
/// where `off = i * 80` little-endian.
pub fn encode_chunked(frame: &OledFrame, command: u8) -> Result<Vec<OledReport>> {
    let pixels = page_major_bytes(frame)?;
    let mut reports = Vec::with_capacity(OLED_CHUNK_COUNT);
    for (i, chunk) in pixels.as_chunks::<OLED_CHUNK_DATA_LEN>().0.iter().enumerate() {
        let offset = u16::try_from(i * OLED_CHUNK_DATA_LEN).unwrap_or(u16::MAX).to_le_bytes();
        let mut bytes = vec![0u8; OLED_CHUNK_REPORT_LEN];
        bytes[..OLED_CHUNK_HEADER_LEN].copy_from_slice(&[
            command,
            OLED_CHUNK_SUBCOMMAND,
            offset[0],
            offset[1],
            OLED_CHUNK_DATA_LEN as u8,
            0x00,
        ]);
        bytes[OLED_CHUNK_HEADER_LEN..OLED_CHUNK_HEADER_LEN + OLED_CHUNK_DATA_LEN].copy_from_slice(chunk);
        reports.push(OledReport { bytes });
    }
    Ok(reports)
}

/// Encode a frame with a model's protocol. Fails when the frame is not 128x40.
pub fn encode_frame(protocol: OledProtocol, frame: &OledFrame) -> Result<Vec<OledReport>> {
    match protocol {
        OledProtocol::RowMajor => Ok(vec![encode_row_major(frame)?]),
        OledProtocol::RowMajorApex7 => Ok(vec![encode_row_major_apex7(frame)?]),
        OledProtocol::PageMajor => Ok(vec![encode_page_major(frame)?]),
        OledProtocol::Chunked { command } => encode_chunked(frame, command),
    }
}

/// Encode `frame` for the keyboard described by `info` and send it. `[EXPERIMENTAL]`
///
/// The reports go to the model's OLED interface through the raw feature-report path
/// ([`crate::devices::send_feature_report_raw`]), so this works with or without an open
/// hidapi handle (the wireless raw-only mode included).
pub async fn draw_frame(info: &DeviceInfo, frame: &OledFrame) -> Result<()> {
    let model = oled_model_for_product_id(info.product_id).ok_or_else(|| {
        Error::Unsupported(format!(
            "keyboard {:04x}:{:04x} has no OLED screen",
            info.vendor_id, info.product_id
        ))
    })?;
    let reports = encode_frame(model.protocol, frame)?;
    tracing::debug!(
        "Drawing OLED frame on {} ({:?} layout, verification {:?}): {} report(s)",
        model.name,
        model.protocol,
        model.verification,
        reports.len()
    );

    let info = info.clone();
    tokio::task::spawn_blocking(move || send_reports_blocking(&info, model, &reports))
        .await
        .map_err(|e| Error::DeviceCommunication(format!("OLED write task failed: {e}")))?
}

/// Send already-encoded reports to the model's OLED interface. Blocking.
pub fn send_reports_blocking(info: &DeviceInfo, model: &OledModel, reports: &[OledReport]) -> Result<()> {
    let path = oled_interface_path(info, model.interface())?;
    for report in reports {
        crate::devices::send_feature_report_raw(&path, report.as_bytes(), report.len())?;
    }
    Ok(())
}

#[cfg(unix)]
fn oled_interface_path(info: &DeviceInfo, interface: u8) -> Result<String> {
    if let Some(path) = crate::devices::find_hidraw_for_interface(info.vendor_id, info.product_id, interface.into()) {
        return Ok(path);
    }
    if info.interface_number == i32::from(interface) {
        return Ok(info.path.clone());
    }
    Err(Error::DeviceNotFound(format!(
        "OLED interface {interface} of {:04x}:{:04x} has no hidraw node",
        info.vendor_id, info.product_id
    )))
}

/// Windows splits an interface into one node per top-level collection; prefer the
/// vendor-defined one (usage page `>= 0xFF00`) on the OLED interface.
#[cfg(not(unix))]
fn oled_interface_path(info: &DeviceInfo, interface: u8) -> Result<String> {
    let api = hidapi::HidApi::new()?;
    let mut fallback = None;
    for dev in api.device_list() {
        if dev.vendor_id() != info.vendor_id
            || dev.product_id() != info.product_id
            || dev.interface_number() != i32::from(interface)
        {
            continue;
        }
        let Ok(path) = dev.path().to_str() else {
            continue;
        };
        if dev.usage_page() >= 0xFF00 {
            return Ok(path.to_owned());
        }
        fallback.get_or_insert_with(|| path.to_owned());
    }
    fallback.ok_or_else(|| {
        Error::DeviceNotFound(format!(
            "OLED interface {interface} of {:04x}:{:04x} not found",
            info.vendor_id, info.product_id
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::keyboards::apex::Apex3Tkl;
    use crate::devices::keyboards::apex_pro_tkl_2023::ApexProTkl2023;
    use crate::devices::keyboards::{GenericKeyboard, Keyboard};
    use crate::devices::{DeviceType, product_ids::*};

    /// Deterministic pseudo-random frame (LCG), so comparisons cover every bit position.
    fn noise_frame(seed: u32) -> OledFrame {
        let mut state = seed;
        let mut frame = OledFrame::new(OLED_WIDTH, OLED_HEIGHT);
        for y in 0..OLED_HEIGHT {
            for x in 0..OLED_WIDTH {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                frame.set_pixel(x, y, state >> 31 == 1);
            }
        }
        frame
    }

    /// apex-tux `apex-hardware/src/device.rs`: `FrameBuffer` is a `BitArray<[u8; 642], Msb0>`
    /// whose byte 0 is `0x61`; `draw_iter` sets bit index `x + y * 128 + 8`.
    fn reference_legacy(frame: &OledFrame) -> Vec<u8> {
        let mut raw = vec![0u8; 642];
        raw[0] = 0x61;
        for y in 0..40u32 {
            for x in 0..128u32 {
                if frame.get_pixel(x, y) {
                    let index = (x + y * 128 + 8) as usize;
                    raw[index / 8] |= 0x80 >> (index % 8);
                }
            }
        }
        raw
    }

    /// apex-tux `apex-hardware/src/usb.rs` `draw_gen3`: transposes the row-major framebuffer in
    /// 8x8 blocks into the 640-byte page-major buffer that the chunks are cut from.
    fn reference_gen3_transpose(raw: &[u8]) -> Vec<u8> {
        let mut fb = vec![0u8; 640];
        for page in 0..5usize {
            for col_group in 0..16usize {
                let base = 1 + page * 128 + col_group;
                let rows: Vec<u8> = (0..8).map(|r| raw[base + r * 16]).collect();
                let dst = page * 128 + col_group * 8;
                for bit_pos in 0..8usize {
                    let s = 7 - bit_pos;
                    let mut byte = 0u8;
                    for (r, row) in rows.iter().enumerate() {
                        byte |= ((row >> s) & 1) << r;
                    }
                    fb[dst + bit_pos] = byte;
                }
            }
        }
        fb
    }

    fn info_for(product_id: u16) -> DeviceInfo {
        DeviceInfo {
            name: std::borrow::Cow::Borrowed("Test keyboard"),
            device_type: DeviceType::Keyboard,
            vendor_id: crate::STEELSERIES_VENDOR_ID,
            product_id,
            interface_number: 1,
            usage_page: 0xFFC0,
            usage: 0x01,
            serial_number: None,
            manufacturer: Some("SteelSeries".to_string()),
            path: "/nonexistent/hidraw".to_string(),
        }
    }

    #[test]
    fn screen_size_per_product_id() {
        let with_screen = [
            APEX_PRO,
            APEX_7,
            APEX_PRO_TKL,
            APEX_7_TKL,
            APEX_5,
            APEX_PRO_TKL_2023,
            APEX_PRO_TKL_2023_WIRELESS,
            APEX_PRO_TKL_2023_WIRELESS_2,
            APEX_PRO_2024,
            APEX_PRO_TKL_2024,
            APEX_PRO_TKL_WIRELESS_2024_DONGLE,
            APEX_PRO_TKL_WIRELESS_2024,
            APEX_5_2024,
            APEX_7_2024,
        ];
        for pid in with_screen {
            assert_eq!(oled_size_for_product_id(pid), Some((128, 40)), "PID {pid:#06x}");
        }

        let without_screen = [
            APEX_3,
            APEX_3_TKL,
            APEX_150,
            APEX_9_MINI,
            APEX_9_TKL,
            APEX_PRO_MINI,
            APEX_PRO_MINI_WIRELESS,
            APEX_PRO_MINI_WIRELESS_DONGLE,
            APEX_PRO_MINI_2024,
            ARCTIS_NOVA_PRO,
            0xFFFF,
        ];
        for pid in without_screen {
            assert_eq!(oled_size_for_product_id(pid), None, "PID {pid:#06x}");
        }

        assert_eq!(oled_models().len(), with_screen.len());
    }

    #[test]
    fn verification_and_protocol_per_product_id() {
        use OledProtocol::*;
        use Verification::*;
        let expected = [
            (APEX_PRO, RowMajor, Reference),
            (APEX_7, RowMajor, Reference),
            (APEX_PRO_TKL, RowMajor, Reference),
            (APEX_7_TKL, RowMajorApex7, Reference),
            (APEX_5, RowMajor, Reference),
            (APEX_PRO_TKL_2023, PageMajor, Guess),
            (APEX_PRO_TKL_2023_WIRELESS_2, CHUNKED_DONGLE, Guess),
            (APEX_PRO_TKL_2023_WIRELESS, CHUNKED_WIRED, Guess),
            (APEX_PRO_2024, PageMajor, Reference),
            (APEX_PRO_TKL_2024, PageMajor, Guess),
            (APEX_PRO_TKL_WIRELESS_2024_DONGLE, CHUNKED_DONGLE, Reference),
            (APEX_PRO_TKL_WIRELESS_2024, CHUNKED_WIRED, Reference),
            (APEX_5_2024, PageMajor, Guess),
            (APEX_7_2024, PageMajor, Guess),
        ];
        for (pid, protocol, verification) in expected {
            let model = oled_model_for_product_id(pid).unwrap();
            assert_eq!(model.protocol, protocol, "PID {pid:#06x}");
            assert_eq!(
                oled_verification_for_product_id(pid),
                Some(verification),
                "PID {pid:#06x}"
            );
            assert!(!model.source.is_empty());
        }
        // apex-tux `oled_interface`: chunked models on 3, the rest on 1.
        assert_eq!(
            oled_model_for_product_id(APEX_PRO_TKL_WIRELESS_2024)
                .unwrap()
                .interface(),
            3
        );
        assert_eq!(oled_model_for_product_id(APEX_PRO).unwrap().interface(), 1);
        assert_eq!(oled_model_for_product_id(APEX_PRO_2024).unwrap().interface(), 1);
    }

    #[test]
    fn row_major_report_layout() {
        let mut frame = OledFrame::new(128, 40);
        frame.set_pixel(0, 0, true);
        frame.set_pixel(9, 10, true);
        frame.set_pixel(127, 39, true);
        let report = encode_row_major(&frame).unwrap();
        let bytes = report.as_bytes();

        assert_eq!(report.len(), 642);
        assert_eq!(report.report_id(), 0x61);
        assert_eq!(bytes[1], 0x80, "(0,0): first pixel byte, MSB");
        assert_eq!(bytes[1 + 10 * 16 + 1], 0x40, "(9,10): row 10, byte 1, bit 6");
        assert_eq!(bytes[640], 0x01, "(127,39): last pixel byte, LSB");
        assert_eq!(bytes[641], 0x00, "trailing zero byte");
        assert_eq!(
            bytes.iter().map(|b| b.count_ones()).sum::<u32>(),
            3 + 0x61u8.count_ones()
        );
    }

    #[test]
    fn row_major_matches_apex_tux_framebuffer_bit_for_bit() {
        for seed in [1, 7, 0xDEAD_BEEF] {
            let frame = noise_frame(seed);
            assert_eq!(
                encode_row_major(&frame).unwrap().as_bytes(),
                reference_legacy(&frame).as_slice()
            );
        }
    }

    #[test]
    fn apex7_report_layout() {
        let mut frame = OledFrame::new(128, 40);
        frame.set_pixel(0, 0, true);
        let report = encode_row_major_apex7(&frame).unwrap();
        let bytes = report.as_bytes();
        assert_eq!(report.len(), 642);
        assert_eq!(&bytes[..3], &[0x00, 0x65, 0x80]);
        assert_eq!(&bytes[2..], frame.data(), "pixels follow the command byte unchanged");
    }

    #[test]
    fn page_major_report_layout() {
        let mut frame = OledFrame::new(128, 40);
        frame.set_pixel(0, 0, true);
        frame.set_pixel(9, 10, true);
        frame.set_pixel(127, 39, true);
        let report = encode_page_major(&frame).unwrap();
        let bytes = report.as_bytes();

        assert_eq!(report.len(), 642);
        assert_eq!(report.report_id(), 0x61);
        assert_eq!(bytes[1], 0x01, "(0,0): page 0, column 0, bit 0");
        assert_eq!(bytes[1 + 128 + 9], 0x04, "(9,10): page 1, column 9, bit 2");
        assert_eq!(bytes[1 + 4 * 128 + 127], 0x80, "(127,39): page 4, column 127, bit 7");
        assert_eq!(bytes[641], 0x00);
    }

    #[test]
    fn page_major_matches_apex_tux_gen3_transpose() {
        for seed in [2, 99, 0x1234_5678] {
            let frame = noise_frame(seed);
            let expected = reference_gen3_transpose(&reference_legacy(&frame));
            assert_eq!(page_major_bytes(&frame).unwrap(), expected);
            assert_eq!(
                &encode_page_major(&frame).unwrap().as_bytes()[1..641],
                expected.as_slice()
            );
        }
    }

    #[test]
    fn chunked_reports_match_apex_tux_draw_gen3() {
        let frame = noise_frame(42);
        let fb = reference_gen3_transpose(&reference_legacy(&frame));
        // apex-tux CHUNK_OFFSETS.
        let offsets: [u16; 8] = [0x0000, 0x0050, 0x00A0, 0x00F0, 0x0140, 0x0190, 0x01E0, 0x0230];

        for command in [OLED_CHUNK_COMMAND_WIRED, OLED_CHUNK_COMMAND_DONGLE] {
            let reports = encode_chunked(&frame, command).unwrap();
            assert_eq!(reports.len(), 8);
            for (i, report) in reports.iter().enumerate() {
                let mut expected = vec![0u8; 641];
                expected[0] = command;
                expected[1] = 0x01;
                expected[2..4].copy_from_slice(&offsets[i].to_le_bytes());
                expected[4] = 80;
                expected[6..86].copy_from_slice(&fb[i * 80..i * 80 + 80]);
                assert_eq!(
                    report.as_bytes(),
                    expected.as_slice(),
                    "chunk {i}, command {command:#04x}"
                );
            }
        }

        let last = &encode_chunked(&frame, 0x0C).unwrap()[7];
        assert_eq!(&last.as_bytes()[..6], &[0x0C, 0x01, 0x30, 0x02, 0x50, 0x00]);
        assert!(last.as_bytes()[86..].iter().all(|&b| b == 0), "tail is zero padding");
    }

    #[test]
    fn encode_frame_dispatches_and_rejects_wrong_sizes() {
        let frame = OledFrame::new(128, 40);
        assert_eq!(encode_frame(OledProtocol::RowMajor, &frame).unwrap().len(), 1);
        assert_eq!(encode_frame(OledProtocol::PageMajor, &frame).unwrap().len(), 1);
        assert_eq!(encode_frame(OledProtocol::RowMajorApex7, &frame).unwrap().len(), 1);
        assert_eq!(encode_frame(CHUNKED_WIRED, &frame).unwrap().len(), 8);

        let small = OledFrame::new(64, 32);
        for protocol in [OledProtocol::RowMajor, OledProtocol::PageMajor, CHUNKED_DONGLE] {
            assert!(matches!(encode_frame(protocol, &small), Err(Error::InvalidConfig(_))));
        }
    }

    #[test]
    fn keyboards_report_their_screen() {
        let generic = GenericKeyboard::new_without_device(info_for(APEX_PRO));
        assert_eq!(generic.oled_size(), Some((128, 40)));

        let apex3 = Apex3Tkl::new(GenericKeyboard::new_without_device(info_for(APEX_3_TKL)));
        assert_eq!(apex3.oled_size(), None);

        let gen3 = ApexProTkl2023::new_wireless_raw(info_for(APEX_PRO_TKL_WIRELESS_2024_DONGLE));
        assert_eq!(gen3.oled_size(), Some((128, 40)));
    }

    #[tokio::test]
    async fn draw_oled_without_a_screen_is_unsupported() {
        let mut apex3 = Apex3Tkl::new(GenericKeyboard::new_without_device(info_for(APEX_3_TKL)));
        let result = apex3.draw_oled(&OledFrame::default()).await;
        assert!(matches!(result, Err(Error::Unsupported(_))), "{result:?}");
    }

    #[tokio::test]
    async fn draw_oled_rejects_a_wrong_size_before_any_io() {
        let mut keyboard = ApexProTkl2023::new_wireless_raw(info_for(APEX_PRO_TKL_WIRELESS_2024));
        let result = keyboard.draw_oled(&OledFrame::new(64, 32)).await;
        assert!(matches!(result, Err(Error::InvalidConfig(_))), "{result:?}");
    }
}
