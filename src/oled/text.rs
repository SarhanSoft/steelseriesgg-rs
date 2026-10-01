//! Text rendering on an [`OledFrame`]: measuring, drawing, word-wrap, truncation, alignment.
//!
//! The font is monospaced (see [`FontSize`]), so widths are character counts times the advance.
//! Characters without a glyph draw as a hollow box; control characters draw as spaces.

pub use super::font::FontSize;
use super::{DEFAULT_HEIGHT, DEFAULT_WIDTH, OledFrame, font};

/// Ellipsis appended when text is cut to fit. It has its own one-glyph-wide bitmap.
pub const ELLIPSIS: char = '…';

/// Horizontal alignment inside the available width.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

/// Width in pixels that `text` occupies on one line (no trailing spacing).
pub fn text_width(text: &str, font: FontSize) -> u32 {
    let chars = u32::try_from(text.chars().count()).unwrap_or(u32::MAX);
    if chars == 0 {
        return 0;
    }
    chars.saturating_mul(font.advance()).saturating_sub(font.spacing())
}

/// How many characters fit in `max_width` pixels on one line.
pub fn chars_per_line(max_width: u32, font: FontSize) -> usize {
    ((max_width + font.spacing()) / font.advance()) as usize
}

/// How many text lines fit in `max_height` pixels.
pub fn lines_per_height(max_height: u32, font: FontSize) -> usize {
    ((max_height + font.spacing()) / font.line_height()) as usize
}

/// Draw one line of text with its top-left corner at `(x, y)`, setting inked pixels to `on`
/// and leaving the rest untouched. Returns the width drawn. Parts outside the frame are clipped.
pub fn draw_text(frame: &mut OledFrame, x: i32, y: i32, text: &str, font: FontSize, on: bool) -> u32 {
    let advance = i32::try_from(font.advance()).unwrap_or(i32::MAX);
    let mut pen = x;
    for c in text.chars() {
        let c = if c.is_control() { ' ' } else { c };
        if c != ' ' {
            let g = font::glyph(c, font);
            for gy in 0..g.height() {
                for gx in 0..g.width() {
                    if g.pixel(gx, gy) {
                        frame.put_pixel(pen.saturating_add(gx as i32), y.saturating_add(gy as i32), on);
                    }
                }
            }
        }
        pen = pen.saturating_add(advance);
    }
    text_width(text, font)
}

/// Draw one line aligned inside the horizontal span `x .. x + width`. Text wider than the span
/// is clipped by the frame, not truncated; use [`truncate`] first to add an ellipsis.
#[allow(clippy::too_many_arguments)]
pub fn draw_text_in(
    frame: &mut OledFrame,
    x: i32,
    y: i32,
    width: u32,
    text: &str,
    font: FontSize,
    align: Align,
    on: bool,
) -> u32 {
    // A negative `free` (text wider than the span) centres the overflow, or right-aligns it so
    // the end of the text stays visible.
    let free = i64::from(width) - i64::from(text_width(text, font));
    let offset = match align {
        Align::Left => 0,
        Align::Center => free / 2,
        Align::Right => free,
    };
    let start = i32::try_from(i64::from(x) + offset).unwrap_or(x);
    draw_text(frame, start, y, text, font, on)
}

/// Draw one line aligned across the whole frame width.
pub fn draw_text_aligned(frame: &mut OledFrame, y: i32, text: &str, font: FontSize, align: Align, on: bool) -> u32 {
    let width = frame.width();
    draw_text_in(frame, 0, y, width, text, font, align, on)
}

/// Cut `text` so it fits in `max_width` pixels, ending in [`ELLIPSIS`] when anything was cut.
pub fn truncate(text: &str, max_width: u32, font: FontSize) -> String {
    let max_chars = chars_per_line(max_width, font);
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    with_ellipsis(text, max_chars)
}

/// End `line` with [`ELLIPSIS`], shortening it so the result still fits in `max_width`. Used for
/// the last visible line when more lines follow that do not fit.
pub fn mark_cut(line: &str, max_width: u32, font: FontSize) -> String {
    let max_chars = chars_per_line(max_width, font);
    let mut kept: String = line.chars().take(max_chars.saturating_sub(1)).collect();
    kept.truncate(kept.trim_end().len());
    kept.push(ELLIPSIS);
    kept
}

fn with_ellipsis(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let mut out: String = text.chars().take(max_chars - 1).collect();
    let trimmed = out.trim_end().len();
    out.truncate(trimmed);
    out.push(ELLIPSIS);
    out
}

/// Word-wrap `text` to lines no wider than `max_width` pixels.
///
/// `\n` starts a new line (an empty paragraph yields an empty line). Runs of whitespace
/// collapse to one space. A word longer than a whole line is broken across lines.
pub fn wrap(text: &str, max_width: u32, font: FontSize) -> Vec<String> {
    let max_chars = chars_per_line(max_width, font).max(1);
    let mut lines = Vec::new();

    for paragraph in text.split('\n') {
        let mut current = String::new();
        let mut current_len = 0usize;

        for word in paragraph.split_whitespace() {
            let word_len = word.chars().count();
            if current_len > 0 && current_len + 1 + word_len <= max_chars {
                current.push(' ');
                current.push_str(word);
                current_len += 1 + word_len;
                continue;
            }
            if current_len > 0 {
                lines.push(std::mem::take(&mut current));
                current_len = 0;
            }
            if word_len <= max_chars {
                current.push_str(word);
                current_len = word_len;
                continue;
            }
            let chars: Vec<char> = word.chars().collect();
            let mut pieces = chars.chunks(max_chars).peekable();
            while let Some(piece) = pieces.next() {
                if pieces.peek().is_some() {
                    lines.push(piece.iter().collect());
                } else {
                    current = piece.iter().collect();
                    current_len = piece.len();
                }
            }
        }

        lines.push(current);
    }

    lines
}

/// How [`render_lines_with`] lays text out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextLayout {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    pub font: FontSize,
    pub align: Align,
    /// Word-wrap long lines (`true`) or cut them with an ellipsis (`false`).
    pub wrap: bool,
    /// Centre the block of lines vertically instead of starting at the top.
    pub center_vertically: bool,
}

impl Default for TextLayout {
    /// 128x40, small font, left-aligned, wrapped, top-aligned.
    fn default() -> Self {
        Self {
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            font: FontSize::Small,
            align: Align::Left,
            wrap: true,
            center_vertically: false,
        }
    }
}

/// Render lines of text on a blank 128x40 frame: small font, left-aligned, word-wrapped, from
/// the top. Up to 5 lines fit; when there is more, the last visible line ends in an ellipsis.
pub fn render_lines(lines: &[&str]) -> OledFrame {
    render_lines_with(lines, &TextLayout::default())
}

/// [`render_lines`] with explicit size, font, alignment and wrapping.
pub fn render_lines_with(lines: &[&str], layout: &TextLayout) -> OledFrame {
    let mut frame = OledFrame::new(layout.width, layout.height);
    let font = layout.font;
    let width = frame.width();

    let mut laid_out: Vec<String> = Vec::new();
    for line in lines {
        if layout.wrap {
            laid_out.extend(wrap(line, width, font));
        } else {
            for part in line.split('\n') {
                laid_out.push(truncate(part, width, font));
            }
        }
    }

    let max_lines = lines_per_height(frame.height(), font);
    if laid_out.len() > max_lines {
        laid_out.truncate(max_lines);
        if let Some(last) = laid_out.last_mut() {
            *last = mark_cut(last, width, font);
        }
    }

    let line_height = font.line_height();
    let block_height = (laid_out.len() as u32 * line_height).saturating_sub(font.spacing());
    let top = if layout.center_vertically {
        frame.height().saturating_sub(block_height) / 2
    } else {
        0
    };

    for (i, line) in laid_out.iter().enumerate() {
        let y = top + i as u32 * line_height;
        draw_text_in(&mut frame, 0, y as i32, width, line, font, layout.align, true);
    }
    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_and_capacity() {
        assert_eq!(text_width("", FontSize::Small), 0);
        assert_eq!(text_width("A", FontSize::Small), 5);
        assert_eq!(text_width("AB", FontSize::Small), 11);
        assert_eq!(text_width("12:34", FontSize::Large), 5 * 12 - 2);
        assert_eq!(chars_per_line(128, FontSize::Small), 21);
        assert_eq!(chars_per_line(128, FontSize::Large), 10);
        assert_eq!(lines_per_height(40, FontSize::Small), 5);
        assert_eq!(lines_per_height(40, FontSize::Large), 2);
    }

    #[test]
    fn draws_glyph_bits_at_the_pen_position() {
        let mut frame = OledFrame::new(16, 8);
        let drawn = draw_text(&mut frame, 1, 0, "A", FontSize::Small, true);
        assert_eq!(drawn, 5);
        let art = frame.to_ascii();
        let rows: Vec<&str> = art.lines().collect();
        assert_eq!(rows[0], "..###...........");
        assert_eq!(rows[3], ".#####..........");
        assert_eq!(rows[6], ".#...#..........");
        assert_eq!(rows[7], "................");
    }

    #[test]
    fn second_glyph_starts_one_advance_later() {
        let mut frame = OledFrame::new(16, 8);
        draw_text(&mut frame, 0, 0, "|", FontSize::Small, true);
        draw_text(&mut frame, 0, 0, " |", FontSize::Small, true);
        let row: String = frame.to_ascii().lines().next().unwrap_or_default().to_string();
        assert_eq!(row, "..#.....#.......");
    }

    #[test]
    fn negative_origin_is_clipped_not_panicking() {
        let mut frame = OledFrame::new(8, 8);
        draw_text(&mut frame, -3, -2, "W", FontSize::Large, true);
        assert!(frame.count_lit() > 0);
    }

    #[test]
    fn alignment() {
        let mut left = OledFrame::new(20, 7);
        draw_text_aligned(&mut left, 0, "I", FontSize::Small, Align::Left, true);
        let mut right = OledFrame::new(20, 7);
        draw_text_aligned(&mut right, 0, "I", FontSize::Small, Align::Right, true);
        let mut center = OledFrame::new(20, 7);
        draw_text_aligned(&mut center, 0, "I", FontSize::Small, Align::Center, true);

        // 'I' top row is .###. — ink columns 1..=3 of the 5-wide glyph.
        assert!(left.get_pixel(1, 0) && left.get_pixel(3, 0));
        assert!(right.get_pixel(16, 0) && right.get_pixel(18, 0) && !right.get_pixel(19, 0));
        // (20 - 5) / 2 = 7: glyph spans 7..12, ink 8..=10.
        assert!(center.get_pixel(8, 0) && center.get_pixel(10, 0) && !center.get_pixel(7, 0));
    }

    #[test]
    fn wrap_breaks_on_words_and_splits_long_words() {
        let lines = wrap("the quick brown fox jumps over the lazy dog", 60, FontSize::Small);
        // 60 px fits 10 characters.
        assert_eq!(lines, ["the quick", "brown fox", "jumps over", "the lazy", "dog"]);
        for line in &lines {
            assert!(text_width(line, FontSize::Small) <= 60);
        }

        let long = wrap("abcdefghijklmnopqrstuvwxyz", 30, FontSize::Small);
        assert_eq!(long, ["abcde", "fghij", "klmno", "pqrst", "uvwxy", "z"]);

        assert_eq!(wrap("one\n\ntwo", 128, FontSize::Small), ["one", "", "two"]);
        assert_eq!(wrap("", 128, FontSize::Small), [""]);
    }

    #[test]
    fn truncate_adds_an_ellipsis_only_when_needed() {
        assert_eq!(truncate("short", 128, FontSize::Small), "short");
        let cut = truncate("this title is far too long to fit", 60, FontSize::Small);
        assert_eq!(cut, "this titl…");
        assert!(text_width(&cut, FontSize::Small) <= 60);
        assert_eq!(truncate("ab cdefgh", 24, FontSize::Small), "ab…");
        assert_eq!(truncate("abc", 0, FontSize::Small), "");
    }

    #[test]
    fn render_lines_fills_top_down_and_marks_overflow() {
        let frame = render_lines(&["Hello", "World"]);
        assert_eq!(frame.size(), (128, 40));
        assert!(frame.get_pixel(0, 1), "H stem on line 1");
        assert!(frame.get_pixel(0, 9), "W stem on line 2 (y = 8 + 1)");
        assert!(
            (16..40).all(|y| (0..128).all(|x| !frame.get_pixel(x, y))),
            "lines 3-5 are empty"
        );

        let many = render_lines(&["1", "2", "3", "4", "5", "6"]);
        // The fifth line starts at y = 32 and must end in an ellipsis (dots on its bottom row).
        let bottom: String = many.to_ascii().lines().nth(38).unwrap_or_default().to_string();
        assert!(bottom.contains("#.#.#"), "ellipsis on the last visible line: {bottom}");
    }

    #[test]
    fn render_lines_with_large_centered() {
        let layout = TextLayout {
            font: FontSize::Large,
            align: Align::Center,
            center_vertically: true,
            ..TextLayout::default()
        };
        let frame = render_lines_with(&["OK"], &layout);
        // One 14 px line in 40 px: top at (40 - 14) / 2 = 13.
        assert!((0..13).all(|y| (0..128).all(|x| !frame.get_pixel(x, y))));
        assert!((13..27).any(|y| (0..128).any(|x| frame.get_pixel(x, y))));
    }
}
