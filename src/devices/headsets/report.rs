//! Report framing shared by every headset family.
//!
//! A headset command is always "two header bytes, then a payload", zero-padded to the frame
//! length the reference driver uses. The bytes are exactly what the reference hands to
//! `hid_write` / `hid_send_feature_report`: the first byte is the HID report ID (or `0x00` when
//! the interface has none).
//!
//! [EXPERIMENTAL] Every frame size here comes from a published reference driver. None has been
//! confirmed on hardware by this project.

/// How a report reaches the device.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReportKind {
    /// Interrupt OUT transfer (`hid_write`).
    Output,
    /// Feature report (`hid_send_feature_report`).
    Feature,
}

/// Transfer kind plus the length a report is zero-padded to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Frame {
    pub kind: ReportKind,
    /// Minimum report length. Shorter reports are zero-padded; `0` sends the bytes unpadded.
    pub len: usize,
}

impl Frame {
    /// Legacy Arctis packets (HeadsetControl `PACKET_SIZE_31`).
    pub const LEGACY: Self = Self::output(31);
    /// Nova-family packets (HeadsetControl `MSG_SIZE`).
    pub const NOVA: Self = Self::output(64);
    /// Arctis Nova 3 settings, sent as 64-byte feature reports.
    pub const NOVA_FEATURE: Self = Self {
        kind: ReportKind::Feature,
        len: 64,
    };
    /// Nova Pro Wireless base-station messages (nova-chatmix-linux `MSGLEN`).
    pub const NOVA_PRO_MESSAGE: Self = Self::output(63);
    /// Arctis 5 lighting packets (OpenRGB `ARCTIS_5_REPORT_SIZE`).
    pub const ARCTIS_5_LIGHTING: Self = Self::output(37);
    /// Arctis Nova 3 lighting effect packet (OpenRGB `ARCTIS_NOVA3_REPORT_SIZE`).
    pub const NOVA_3_EFFECT: Self = Self::output(521);
    /// Requests the references write without padding, e.g. a two-byte status query.
    pub const UNPADDED: Self = Self::output(0);

    pub const fn output(len: usize) -> Self {
        Self {
            kind: ReportKind::Output,
            len,
        }
    }
}

/// A command: a frame and its two leading bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Command {
    pub frame: Frame,
    pub header: [u8; 2],
}

impl Command {
    pub const fn new(frame: Frame, first: u8, second: u8) -> Self {
        Self {
            frame,
            header: [first, second],
        }
    }

    /// Build the report for this command carrying `payload` right after the header.
    pub fn report(self, payload: &[u8]) -> Report {
        let mut bytes = Vec::with_capacity(self.frame.len.max(self.header.len() + payload.len()));
        bytes.extend_from_slice(&self.header);
        bytes.extend_from_slice(payload);
        if bytes.len() < self.frame.len {
            bytes.resize(self.frame.len, 0);
        }
        Report {
            kind: self.frame.kind,
            bytes,
        }
    }
}

/// A fixed command with a fixed payload, e.g. a save command. Usable in `static` tables.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Packet {
    pub command: Command,
    pub payload: &'static [u8],
}

impl Packet {
    pub const fn new(command: Command, payload: &'static [u8]) -> Self {
        Self { command, payload }
    }

    pub fn report(&self) -> Report {
        self.command.report(self.payload)
    }
}

/// One fully built report, ready to hand to the HID layer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Report {
    pub kind: ReportKind,
    pub bytes: Vec<u8>,
}

impl Report {
    /// Write `values` starting at `offset`, growing the report if needed.
    pub fn put(&mut self, offset: usize, values: &[u8]) {
        let end = offset + values.len();
        if self.bytes.len() < end {
            self.bytes.resize(end, 0);
        }
        self.bytes[offset..end].copy_from_slice(values);
    }

    /// Builder form of [`Report::put`].
    pub fn with(mut self, offset: usize, values: &[u8]) -> Self {
        self.put(offset, values);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_pads_to_frame_length() {
        let report = Command::new(Frame::LEGACY, 0x06, 0x35).report(&[0x01, 0x00, 0x12]);
        assert_eq!(report.kind, ReportKind::Output);
        assert_eq!(report.bytes.len(), 31);
        assert_eq!(&report.bytes[..5], &[0x06, 0x35, 0x01, 0x00, 0x12]);
        assert!(report.bytes[5..].iter().all(|b| *b == 0));
    }

    #[test]
    fn unpadded_command_keeps_exact_length() {
        let report = Command::new(Frame::UNPADDED, 0x00, 0xb0).report(&[]);
        assert_eq!(report.bytes, vec![0x00, 0xb0]);
    }

    #[test]
    fn feature_frame_marks_report_kind() {
        let report = Packet::new(Command::new(Frame::NOVA_FEATURE, 0x06, 0x09), &[]).report();
        assert_eq!(report.kind, ReportKind::Feature);
        assert_eq!(report.bytes.len(), 64);
    }

    #[test]
    fn put_grows_and_overwrites() {
        let report = Command::new(Frame::UNPADDED, 0x06, 0x8a)
            .report(&[])
            .with(4, &[0xaa, 0xbb]);
        assert_eq!(report.bytes, vec![0x06, 0x8a, 0x00, 0x00, 0xaa, 0xbb]);
    }
}
