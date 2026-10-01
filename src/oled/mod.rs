//! Monochrome frames for the OLED screen on SteelSeries Apex keyboards. `[EXPERIMENTAL]`
//!
//! Everything here is pure: building a frame never touches a device. A frame is sent with
//! [`crate::devices::keyboards::Keyboard::draw_oled`], which encodes it into the keyboard's own
//! report layout (see [`crate::devices::keyboards::oled`]).
//!
//! - [`OledFrame`]: a 1-bit-per-pixel framebuffer.
//! - [`text`](self::text): an embedded bitmap font (small and large), word-wrap and alignment.
//! - [`image`](self::image): PNG/GIF/JPEG/BMP decoding, scale-to-fit, 1-bit conversion.
//! - [`screens`](self::screens): ready-made layouts (clock, system stats, now playing,
//!   notification, progress).

pub mod font;
pub mod image;
pub mod screens;
pub mod text;

pub use self::image::{
    Dither, ImageOptions, dither_luma, frame_from_image, frame_from_image_bytes, frame_from_image_path,
    frames_from_gif_bytes, frames_from_image_bytes,
};
pub use self::screens::{SystemStats, clock, notification, now_playing, progress_bar, system_stats};
pub use self::text::{Align, FontSize, TextLayout, render_lines, render_lines_with};

/// Width in pixels of the OLED screen on every Apex keyboard that has one.
pub const DEFAULT_WIDTH: u32 = 128;

/// Height in pixels of the OLED screen on every Apex keyboard that has one.
pub const DEFAULT_HEIGHT: u32 = 40;

/// Largest width or height [`OledFrame::new`] accepts; larger values are clamped to it.
///
/// The real screens are 128x40. The cap keeps a bad argument from allocating gigabytes.
pub const MAX_DIMENSION: u32 = 1024;

/// A 1-bit-per-pixel monochrome frame.
///
/// # Packing order
///
/// Row-major, most significant bit first — the order the Apex Pro / Apex 7 / Apex 5 family
/// expects on the wire (apex-tux `apex-hardware/src/device.rs`, `FrameBuffer`):
///
/// - Rows run top to bottom. Each row takes [`row_stride`](Self::row_stride) bytes,
///   `ceil(width / 8)`; for the 128x40 screen that is 16 bytes per row, 640 bytes in total.
/// - Within a row, byte `x / 8` holds pixels `x & !7 ..= x | 7`; bit 7 (`0x80`) is the
///   leftmost pixel of the byte and bit 0 the rightmost.
/// - A set bit is a lit pixel.
/// - When `width` is not a multiple of 8, the unused low bits of each row's last byte stay 0.
///
/// The Gen 3 boards want SSD1306 page-major bytes instead; the encoder in
/// [`crate::devices::keyboards::oled`] transposes for them, so callers never deal with it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OledFrame {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl Default for OledFrame {
    /// A blank 128x40 frame.
    fn default() -> Self {
        Self::new(DEFAULT_WIDTH, DEFAULT_HEIGHT)
    }
}

impl OledFrame {
    /// Create a blank (all pixels off) frame. Each dimension is clamped to [`MAX_DIMENSION`].
    pub fn new(width: u32, height: u32) -> Self {
        let width = width.min(MAX_DIMENSION);
        let height = height.min(MAX_DIMENSION);
        let len = Self::stride_for(width) * height as usize;
        Self {
            width,
            height,
            data: vec![0; len],
        }
    }

    /// Wrap already-packed bytes (see the packing order on [`OledFrame`]).
    ///
    /// Fails when `data.len()` is not exactly `ceil(width / 8) * height`, or a dimension is
    /// above [`MAX_DIMENSION`]. Unused padding bits are cleared.
    pub fn from_packed(width: u32, height: u32, data: Vec<u8>) -> crate::Result<Self> {
        if width > MAX_DIMENSION || height > MAX_DIMENSION {
            return Err(crate::Error::InvalidConfig(format!(
                "OLED frame {width}x{height} exceeds the {MAX_DIMENSION} pixel limit"
            )));
        }
        let expected = Self::stride_for(width) * height as usize;
        if data.len() != expected {
            return Err(crate::Error::InvalidConfig(format!(
                "OLED frame {width}x{height} needs {expected} packed bytes, got {}",
                data.len()
            )));
        }
        let mut frame = Self { width, height, data };
        frame.clear_padding_bits();
        Ok(frame)
    }

    fn stride_for(width: u32) -> usize {
        (width as usize).div_ceil(8)
    }

    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// `(width, height)` in pixels.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Bytes per row: `ceil(width / 8)`.
    pub fn row_stride(&self) -> usize {
        Self::stride_for(self.width)
    }

    /// The packed pixel bytes, in the order documented on [`OledFrame`].
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    fn index(&self, x: u32, y: u32) -> Option<(usize, u8)> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let byte = y as usize * self.row_stride() + (x / 8) as usize;
        Some((byte, 0x80 >> (x % 8)))
    }

    /// Turn one pixel on or off. Coordinates outside the frame are ignored.
    pub fn set_pixel(&mut self, x: u32, y: u32, on: bool) {
        if let Some((byte, mask)) = self.index(x, y) {
            if on {
                self.data[byte] |= mask;
            } else {
                self.data[byte] &= !mask;
            }
        }
    }

    /// Whether a pixel is lit. Coordinates outside the frame read as off.
    pub fn get_pixel(&self, x: u32, y: u32) -> bool {
        self.index(x, y).is_some_and(|(byte, mask)| self.data[byte] & mask != 0)
    }

    /// Signed-coordinate variant of [`set_pixel`](Self::set_pixel), for drawing that may start
    /// off-screen (scrolling text, centred content wider than the frame).
    pub fn put_pixel(&mut self, x: i32, y: i32, on: bool) {
        if let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) {
            self.set_pixel(x, y, on);
        }
    }

    /// Flip every pixel.
    pub fn invert(&mut self) {
        for byte in &mut self.data {
            *byte = !*byte;
        }
        self.clear_padding_bits();
    }

    /// Turn every pixel off.
    pub fn clear(&mut self) {
        self.data.fill(0);
    }

    /// Set every pixel to `on`.
    pub fn fill(&mut self, on: bool) {
        self.data.fill(if on { 0xFF } else { 0x00 });
        self.clear_padding_bits();
    }

    /// Set every pixel of a rectangle to `on`; the part outside the frame is ignored.
    pub fn fill_rect(&mut self, x: i32, y: i32, width: u32, height: u32, on: bool) {
        let x_end = x.saturating_add(i32::try_from(width).unwrap_or(i32::MAX));
        let y_end = y.saturating_add(i32::try_from(height).unwrap_or(i32::MAX));
        let x_range = x.max(0)..x_end.min(i32::try_from(self.width).unwrap_or(i32::MAX));
        for py in y.max(0)..y_end.min(i32::try_from(self.height).unwrap_or(i32::MAX)) {
            for px in x_range.clone() {
                self.put_pixel(px, py, on);
            }
        }
    }

    /// Draw a one-pixel rectangle outline.
    pub fn draw_rect(&mut self, x: i32, y: i32, width: u32, height: u32, on: bool) {
        if width == 0 || height == 0 {
            return;
        }
        let right = x.saturating_add(i32::try_from(width - 1).unwrap_or(i32::MAX));
        let bottom = y.saturating_add(i32::try_from(height - 1).unwrap_or(i32::MAX));
        self.fill_rect(x, y, width, 1, on);
        self.fill_rect(x, bottom, width, 1, on);
        self.fill_rect(x, y, 1, height, on);
        self.fill_rect(right, y, 1, height, on);
    }

    /// Number of lit pixels.
    pub fn count_lit(&self) -> usize {
        self.data.iter().map(|b| b.count_ones() as usize).sum()
    }

    /// Render as text, one line per row, `#` for lit and `.` for dark. Meant for tests, logs and
    /// a terminal preview of what the screen will show.
    pub fn to_ascii(&self) -> String {
        let mut out = String::with_capacity((self.width as usize + 1) * self.height as usize);
        for y in 0..self.height {
            for x in 0..self.width {
                out.push(if self.get_pixel(x, y) { '#' } else { '.' });
            }
            out.push('\n');
        }
        out
    }

    fn clear_padding_bits(&mut self) {
        let used = self.width % 8;
        if used == 0 {
            return;
        }
        let keep = 0xFFu8 << (8 - used);
        let stride = self.row_stride();
        for row in self.data.chunks_exact_mut(stride) {
            if let Some(last) = row.last_mut() {
                *last &= keep;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_frame_is_blank_and_sized_for_the_screen() {
        let frame = OledFrame::default();
        assert_eq!(frame.size(), (128, 40));
        assert_eq!(frame.row_stride(), 16);
        assert_eq!(frame.data().len(), 640);
        assert_eq!(frame.count_lit(), 0);
    }

    #[test]
    fn packing_is_row_major_msb_first() {
        let mut frame = OledFrame::new(128, 40);
        frame.set_pixel(0, 0, true);
        assert_eq!(frame.data()[0], 0x80, "pixel (0,0) is bit 7 of byte 0");

        frame.clear();
        frame.set_pixel(7, 0, true);
        assert_eq!(frame.data()[0], 0x01, "pixel (7,0) is bit 0 of byte 0");

        frame.clear();
        frame.set_pixel(8, 0, true);
        assert_eq!(frame.data()[1], 0x80, "pixel (8,0) starts byte 1");

        frame.clear();
        frame.set_pixel(0, 1, true);
        assert_eq!(frame.data()[16], 0x80, "row 1 starts at byte 16");

        frame.clear();
        frame.set_pixel(127, 39, true);
        assert_eq!(frame.data()[639], 0x01, "last pixel is bit 0 of the last byte");
        assert_eq!(frame.count_lit(), 1);
    }

    #[test]
    fn set_get_round_trip_and_out_of_bounds_is_ignored() {
        let mut frame = OledFrame::new(10, 3);
        assert_eq!(frame.row_stride(), 2);
        frame.set_pixel(9, 2, true);
        assert!(frame.get_pixel(9, 2));
        frame.set_pixel(9, 2, false);
        assert!(!frame.get_pixel(9, 2));

        frame.set_pixel(10, 0, true);
        frame.set_pixel(0, 3, true);
        frame.put_pixel(-1, 0, true);
        assert_eq!(frame.count_lit(), 0);
        assert!(!frame.get_pixel(100, 100));
    }

    #[test]
    fn invert_and_fill_keep_padding_bits_clear() {
        let mut frame = OledFrame::new(10, 2);
        frame.invert();
        assert_eq!(frame.count_lit(), 20);
        assert_eq!(frame.data(), &[0xFF, 0xC0, 0xFF, 0xC0]);
        frame.invert();
        assert_eq!(frame.count_lit(), 0);

        frame.fill(true);
        assert_eq!(frame.count_lit(), 20);
        frame.clear();
        assert_eq!(frame.count_lit(), 0);
    }

    #[test]
    fn from_packed_validates_length() {
        assert!(OledFrame::from_packed(128, 40, vec![0; 640]).is_ok());
        assert!(OledFrame::from_packed(128, 40, vec![0; 641]).is_err());
        assert!(OledFrame::from_packed(MAX_DIMENSION + 1, 1, vec![0; 129]).is_err());

        let frame = OledFrame::from_packed(4, 1, vec![0xFF]).unwrap();
        assert_eq!(frame.data(), &[0xF0], "padding bits are cleared");
    }

    #[test]
    fn dimensions_are_clamped() {
        let frame = OledFrame::new(u32::MAX, 2);
        assert_eq!(frame.width(), MAX_DIMENSION);
    }

    #[test]
    fn rect_helpers_clip_to_the_frame() {
        let mut frame = OledFrame::new(8, 4);
        frame.fill_rect(-2, -2, 4, 4, true);
        assert_eq!(frame.count_lit(), 4);

        frame.clear();
        frame.draw_rect(0, 0, 8, 4, true);
        assert_eq!(frame.count_lit(), 8 + 8 + 2 + 2);
        assert!(!frame.get_pixel(1, 1));
    }
}
