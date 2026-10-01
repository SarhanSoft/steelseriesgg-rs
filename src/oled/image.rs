//! Turn PNG, GIF, JPEG or BMP images into OLED frames.
//!
//! Pipeline: decode → scale to fit (aspect ratio kept, centred, black letterbox) → grey
//! (Rec. 601 luma, transparency composited over black) → 1 bit, by Floyd–Steinberg error
//! diffusion or a plain threshold.

use std::io::Cursor;
use std::path::Path;
use std::time::Duration;

use image::codecs::gif::GifDecoder;
use image::imageops::FilterType;
use image::{AnimationDecoder, DynamicImage, ImageFormat};

use super::{DEFAULT_HEIGHT, DEFAULT_WIDTH, OledFrame};
use crate::{Error, Result};

/// Most frames kept from one animated GIF; the rest are dropped.
pub const MAX_GIF_FRAMES: usize = 2000;

/// GIF delays below this are treated as "unset", as browsers do.
const MIN_GIF_DELAY: Duration = Duration::from_millis(20);
/// Delay used for frames whose GIF delay is unset or below [`MIN_GIF_DELAY`].
const DEFAULT_GIF_DELAY: Duration = Duration::from_millis(100);

/// How grey levels become on/off pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Dither {
    /// Floyd–Steinberg error diffusion (7/16 right, 3/16 below-left, 5/16 below, 1/16
    /// below-right) at a 50% threshold. Best for photos and gradients.
    #[default]
    FloydSteinberg,
    /// A pixel is lit when its luma (0-255) is at least this value. Best for logos, line art
    /// and text, which dithering would fringe.
    Threshold(u8),
}

/// Options for converting an image to a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImageOptions {
    /// Output width in pixels.
    pub width: u32,
    /// Output height in pixels.
    pub height: u32,
    pub dither: Dither,
    /// Swap lit and dark after conversion (dark-on-light artwork).
    pub invert: bool,
}

impl Default for ImageOptions {
    /// 128x40, Floyd–Steinberg, not inverted.
    fn default() -> Self {
        Self {
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            dither: Dither::FloydSteinberg,
            invert: false,
        }
    }
}

fn decode_error(e: image::ImageError) -> Error {
    Error::Other(format!("image decoding failed: {e}"))
}

/// Decode a PNG, GIF (first frame), JPEG or BMP image held in memory and convert it.
pub fn frame_from_image_bytes(bytes: &[u8], options: &ImageOptions) -> Result<OledFrame> {
    let image = image::load_from_memory(bytes).map_err(decode_error)?;
    Ok(frame_from_image(&image, options))
}

/// Read and convert an image file. See [`frame_from_image_bytes`].
pub fn frame_from_image_path(path: impl AsRef<Path>, options: &ImageOptions) -> Result<OledFrame> {
    let bytes = std::fs::read(path)?;
    frame_from_image_bytes(&bytes, options)
}

/// Decode every frame of an animated GIF, with each frame's display time.
///
/// Frames come out fully composited (GIF disposal handled by the decoder). Delays under
/// 20 ms are replaced by 100 ms, as browsers do. At most [`MAX_GIF_FRAMES`] frames are kept.
pub fn frames_from_gif_bytes(bytes: &[u8], options: &ImageOptions) -> Result<Vec<(OledFrame, Duration)>> {
    let decoder = GifDecoder::new(Cursor::new(bytes)).map_err(decode_error)?;
    let mut frames = Vec::new();
    for frame in decoder.into_frames() {
        if frames.len() >= MAX_GIF_FRAMES {
            tracing::warn!("GIF has more than {MAX_GIF_FRAMES} frames; the rest are dropped");
            break;
        }
        let frame = frame.map_err(decode_error)?;
        let (numer, denom) = frame.delay().numer_denom_ms();
        let delay = Duration::from_millis(u64::from(numer.checked_div(denom).unwrap_or(0)));
        let delay = if delay < MIN_GIF_DELAY {
            DEFAULT_GIF_DELAY
        } else {
            delay
        };
        let image = DynamicImage::ImageRgba8(frame.into_buffer());
        frames.push((frame_from_image(&image, options), delay));
    }
    if frames.is_empty() {
        return Err(Error::Other("GIF contains no frames".to_string()));
    }
    Ok(frames)
}

/// Decode any supported image as a sequence: every frame of a GIF, or a single frame (with a
/// zero duration) for PNG, JPEG and BMP.
pub fn frames_from_image_bytes(bytes: &[u8], options: &ImageOptions) -> Result<Vec<(OledFrame, Duration)>> {
    match image::guess_format(bytes) {
        Ok(ImageFormat::Gif) => frames_from_gif_bytes(bytes, options),
        _ => Ok(vec![(frame_from_image_bytes(bytes, options)?, Duration::ZERO)]),
    }
}

/// Convert an already-decoded image: scale to fit, centre, convert to 1 bit.
pub fn frame_from_image(image: &DynamicImage, options: &ImageOptions) -> OledFrame {
    let target = OledFrame::new(options.width, options.height);
    let (out_w, out_h) = target.size();
    let (src_w, src_h) = (image.width(), image.height());
    if src_w == 0 || src_h == 0 || out_w == 0 || out_h == 0 {
        return target;
    }

    let scale = f64::min(f64::from(out_w) / f64::from(src_w), f64::from(out_h) / f64::from(src_h));
    let fit_w = ((f64::from(src_w) * scale).round() as u32).clamp(1, out_w);
    let fit_h = ((f64::from(src_h) * scale).round() as u32).clamp(1, out_h);
    // Nearest keeps small icons crisp when enlarged; Triangle averages when shrinking.
    let filter = if scale >= 1.0 {
        FilterType::Nearest
    } else {
        FilterType::Triangle
    };
    let scaled = image.resize_exact(fit_w, fit_h, filter).to_rgba8();

    let mut luma = vec![0u8; out_w as usize * out_h as usize];
    let left = (out_w - fit_w) / 2;
    let top = (out_h - fit_h) / 2;
    for (x, y, pixel) in scaled.enumerate_pixels() {
        let [r, g, b, a] = pixel.0;
        let grey = (u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114) / 1000;
        let grey = grey * u32::from(a) / 255;
        let index = (top + y) as usize * out_w as usize + (left + x) as usize;
        if let Some(slot) = luma.get_mut(index) {
            *slot = grey as u8;
        }
    }

    let mut frame = dither_luma_unchecked(&luma, out_w, out_h, options.dither);
    if options.invert {
        frame.invert();
    }
    frame
}

/// Convert a row-major 8-bit grey buffer (`width * height` bytes) to a 1-bit frame.
pub fn dither_luma(luma: &[u8], width: u32, height: u32, dither: Dither) -> Result<OledFrame> {
    let frame = OledFrame::new(width, height);
    if frame.size() != (width, height) || luma.len() != width as usize * height as usize {
        return Err(Error::InvalidConfig(format!(
            "grey buffer of {} bytes does not match {width}x{height}",
            luma.len()
        )));
    }
    Ok(dither_luma_unchecked(luma, width, height, dither))
}

fn dither_luma_unchecked(luma: &[u8], width: u32, height: u32, dither: Dither) -> OledFrame {
    let mut frame = OledFrame::new(width, height);
    let w = width as usize;
    let h = height as usize;

    match dither {
        Dither::Threshold(level) => {
            for y in 0..h {
                for x in 0..w {
                    if luma.get(y * w + x).is_some_and(|&v| v >= level) {
                        frame.set_pixel(x as u32, y as u32, true);
                    }
                }
            }
        }
        Dither::FloydSteinberg => {
            // Error rows padded by one cell on each side so x - 1 and x + 1 never go out of range.
            let mut current = vec![0i32; w + 2];
            let mut next = vec![0i32; w + 2];
            for y in 0..h {
                for x in 0..w {
                    let source = i32::from(luma.get(y * w + x).copied().unwrap_or(0));
                    let value = source + current[x + 1];
                    let lit = value >= 128;
                    if lit {
                        frame.set_pixel(x as u32, y as u32, true);
                    }
                    let error = value - if lit { 255 } else { 0 };
                    current[x + 2] += error * 7 / 16;
                    next[x] += error * 3 / 16;
                    next[x + 1] += error * 5 / 16;
                    next[x + 2] += error / 16;
                }
                std::mem::swap(&mut current, &mut next);
                next.fill(0);
            }
        }
    }
    frame
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn horizontal_gradient(width: u32, height: u32) -> Vec<u8> {
        let mut luma = Vec::with_capacity((width * height) as usize);
        for _ in 0..height {
            for x in 0..width {
                luma.push((x * 255 / (width - 1)) as u8);
            }
        }
        luma
    }

    fn lit_in_columns(frame: &OledFrame, columns: std::ops::Range<u32>) -> f64 {
        let mut lit = 0u32;
        for y in 0..frame.height() {
            for x in columns.clone() {
                lit += u32::from(frame.get_pixel(x, y));
            }
        }
        f64::from(lit) / f64::from(columns.len() as u32 * frame.height())
    }

    #[test]
    fn floyd_steinberg_tracks_a_gradient() {
        let luma = horizontal_gradient(128, 40);
        let frame = dither_luma(&luma, 128, 40, Dither::FloydSteinberg).unwrap();

        let overall = frame.count_lit() as f64 / (128.0 * 40.0);
        assert!((overall - 0.5).abs() < 0.03, "overall density {overall}");

        assert_eq!(lit_in_columns(&frame, 0..2), 0.0, "black end stays dark");
        assert_eq!(lit_in_columns(&frame, 126..128), 1.0, "white end stays lit");

        let mut previous = -1.0;
        for band in 0..8u32 {
            let columns = band * 16..band * 16 + 16;
            let density = lit_in_columns(&frame, columns.clone());
            let expected = columns.clone().map(|x| f64::from(x) / 127.0).sum::<f64>() / 16.0;
            assert!(
                (density - expected).abs() < 0.08,
                "band {band}: density {density:.3}, expected {expected:.3}"
            );
            assert!(density >= previous - 0.02, "band {band} is darker than the band before");
            previous = density;
        }

        // Error diffusion mixes lit and dark pixels inside a mid-grey band, unlike a threshold.
        let mid = lit_in_columns(&frame, 56..72);
        assert!(mid > 0.3 && mid < 0.7);
    }

    #[test]
    fn threshold_splits_a_gradient_at_the_level() {
        let luma = horizontal_gradient(128, 40);
        let frame = dither_luma(&luma, 128, 40, Dither::Threshold(128)).unwrap();
        // x * 255 / 127 >= 128  <=>  x >= 64
        assert_eq!(lit_in_columns(&frame, 0..64), 0.0);
        assert_eq!(lit_in_columns(&frame, 64..128), 1.0);
        assert_eq!(frame.count_lit(), 64 * 40);
    }

    #[test]
    fn dither_luma_rejects_mismatched_buffers() {
        assert!(dither_luma(&[0; 10], 4, 4, Dither::FloydSteinberg).is_err());
    }

    #[test]
    fn image_is_scaled_to_fit_and_centred() {
        // A 20x20 white square becomes 40x40 (height-limited), centred: columns 44..84.
        let img = DynamicImage::ImageRgba8(RgbaImage::from_pixel(20, 20, Rgba([255, 255, 255, 255])));
        let frame = frame_from_image(&img, &ImageOptions::default());
        assert_eq!(frame.size(), (128, 40));
        assert_eq!(frame.count_lit(), 40 * 40);
        assert!(frame.get_pixel(44, 0) && frame.get_pixel(83, 39));
        assert!(!frame.get_pixel(43, 20) && !frame.get_pixel(84, 20));
    }

    #[test]
    fn transparency_is_dark_and_invert_flips() {
        let img = DynamicImage::ImageRgba8(RgbaImage::from_pixel(128, 40, Rgba([255, 255, 255, 0])));
        let frame = frame_from_image(&img, &ImageOptions::default());
        assert_eq!(frame.count_lit(), 0);

        let options = ImageOptions {
            invert: true,
            ..ImageOptions::default()
        };
        assert_eq!(frame_from_image(&img, &options).count_lit(), 128 * 40);
    }

    #[test]
    fn png_bytes_round_trip() {
        let mut img = RgbaImage::from_pixel(128, 40, Rgba([0, 0, 0, 255]));
        for y in 0..40 {
            for x in 0..64 {
                img.put_pixel(x, y, Rgba([255, 255, 255, 255]));
            }
        }
        let mut png = Vec::new();
        DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
            .unwrap();

        let options = ImageOptions {
            dither: Dither::Threshold(128),
            ..ImageOptions::default()
        };
        let frame = frame_from_image_bytes(&png, &options).unwrap();
        assert_eq!(frame.count_lit(), 64 * 40);
        assert!(frame.get_pixel(0, 0) && !frame.get_pixel(64, 0));

        let frames = frames_from_image_bytes(&png, &options).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].1, Duration::ZERO);
    }

    #[test]
    fn animated_gif_yields_frames_with_delays() {
        use image::codecs::gif::{GifEncoder, Repeat};
        use image::{Delay, Frame};

        let mut gif = Vec::new();
        {
            let mut encoder = GifEncoder::new(&mut gif);
            encoder.set_repeat(Repeat::Infinite).unwrap();
            let white = RgbaImage::from_pixel(16, 8, Rgba([255, 255, 255, 255]));
            let black = RgbaImage::from_pixel(16, 8, Rgba([0, 0, 0, 255]));
            encoder
                .encode_frame(Frame::from_parts(white, 0, 0, Delay::from_numer_denom_ms(250, 1)))
                .unwrap();
            encoder
                .encode_frame(Frame::from_parts(black, 0, 0, Delay::from_numer_denom_ms(0, 1)))
                .unwrap();
        }

        let options = ImageOptions {
            width: 16,
            height: 8,
            dither: Dither::Threshold(128),
            invert: false,
        };
        let frames = frames_from_gif_bytes(&gif, &options).unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].0.count_lit(), 16 * 8);
        assert_eq!(frames[0].1, Duration::from_millis(250));
        assert_eq!(frames[1].0.count_lit(), 0);
        assert_eq!(frames[1].1, DEFAULT_GIF_DELAY, "unset delay falls back to 100 ms");

        assert_eq!(frames_from_image_bytes(&gif, &options).unwrap().len(), 2);
    }

    #[test]
    fn garbage_is_an_error_not_a_panic() {
        let options = ImageOptions::default();
        assert!(frame_from_image_bytes(b"not an image", &options).is_err());
        assert!(frames_from_gif_bytes(b"GIF89a", &options).is_err());
    }
}
