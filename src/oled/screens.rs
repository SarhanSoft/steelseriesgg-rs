//! Ready-made 128x40 screens for the daemon.
//!
//! Every function takes the values to show as arguments and samples nothing itself, so the
//! caller decides where numbers come from and how often a screen is redrawn.

use chrono::{Datelike, Timelike};

use super::text::{Align, FontSize, draw_text_in, lines_per_height, mark_cut, truncate, wrap};
use super::{DEFAULT_HEIGHT, DEFAULT_WIDTH, OledFrame};

const DAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

fn blank() -> OledFrame {
    OledFrame::new(DEFAULT_WIDTH, DEFAULT_HEIGHT)
}

/// Clamp to `0.0..=1.0`; NaN becomes 0.
fn unit(value: f32) -> f32 {
    if value.is_nan() { 0.0 } else { value.clamp(0.0, 1.0) }
}

/// Clamp a percentage to `0.0..=100.0`; NaN becomes 0.
fn percent(value: f32) -> f32 {
    unit(value / 100.0) * 100.0
}

/// A horizontal bar: one-pixel outline, filled from the left by `fraction` (0..=1).
fn draw_bar(frame: &mut OledFrame, x: i32, y: i32, width: u32, height: u32, fraction: f32) {
    frame.draw_rect(x, y, width, height, true);
    let inset: u32 = if height >= 8 { 2 } else { 1 };
    let inner_w = width.saturating_sub(inset * 2);
    let inner_h = height.saturating_sub(inset * 2);
    let filled = (inner_w as f32 * unit(fraction)).round() as u32;
    let inset = inset as i32;
    frame.fill_rect(x + inset, y + inset, filled.min(inner_w), inner_h, true);
}

/// Time as `HH:MM:SS` in the large font, with the date (`Wed 01 Oct 2026`) below it.
///
/// Accepts any chrono date-time, e.g. `chrono::Local::now()` or a `NaiveDateTime`.
pub fn clock<T: Datelike + Timelike>(now: &T) -> OledFrame {
    let mut frame = blank();
    let time = format!("{:02}:{:02}:{:02}", now.hour(), now.minute(), now.second());
    let day = DAYS[now.weekday().num_days_from_monday() as usize % 7];
    let month = MONTHS[now.month0() as usize % 12];
    let date = format!("{day} {:02} {month} {}", now.day(), now.year());

    draw_text_in(
        &mut frame,
        0,
        6,
        DEFAULT_WIDTH,
        &time,
        FontSize::Large,
        Align::Center,
        true,
    );
    draw_text_in(
        &mut frame,
        0,
        26,
        DEFAULT_WIDTH,
        &date,
        FontSize::Small,
        Align::Center,
        true,
    );
    frame
}

/// Numbers for [`system_stats`]. Percentages are 0-100; out-of-range values are clamped.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SystemStats {
    pub cpu_percent: f32,
    pub ram_percent: f32,
    /// Shown as a third row when present.
    pub gpu_percent: Option<f32>,
    /// Shown as a last row when present; the bar spans 0-100 °C.
    pub cpu_temp_celsius: Option<f32>,
}

/// One row per metric: a three-letter label, a bar and the value. Two to four rows, spread
/// evenly over the screen height.
pub fn system_stats(stats: &SystemStats) -> OledFrame {
    let mut rows: Vec<(&str, f32, String)> = vec![
        (
            "CPU",
            percent(stats.cpu_percent),
            format!("{:.0}%", percent(stats.cpu_percent)),
        ),
        (
            "RAM",
            percent(stats.ram_percent),
            format!("{:.0}%", percent(stats.ram_percent)),
        ),
    ];
    if let Some(gpu) = stats.gpu_percent {
        rows.push(("GPU", percent(gpu), format!("{:.0}%", percent(gpu))));
    }
    if let Some(temp) = stats.cpu_temp_celsius {
        let shown = if temp.is_nan() { 0.0 } else { temp.clamp(-99.0, 999.0) };
        rows.push(("TMP", percent(shown), format!("{shown:.0}°C")));
    }

    let mut frame = blank();
    let pitch = DEFAULT_HEIGHT / rows.len() as u32;
    let glyph_h = FontSize::Small.glyph_height();
    // Label: 3 chars (17 px) at x = 0. Value: up to 5 chars (29 px), right-aligned.
    let bar_x = 21;
    let value_w = 29;
    let bar_w = DEFAULT_WIDTH - bar_x - value_w - 3;

    for (i, (label, level, value)) in rows.iter().enumerate() {
        let y = (i as u32 * pitch + (pitch - glyph_h) / 2) as i32;
        draw_text_in(&mut frame, 0, y, 18, label, FontSize::Small, Align::Left, true);
        draw_bar(&mut frame, bar_x as i32, y + 1, bar_w, glyph_h - 2, level / 100.0);
        draw_text_in(
            &mut frame,
            (DEFAULT_WIDTH - value_w) as i32,
            y,
            value_w,
            value,
            FontSize::Small,
            Align::Right,
            true,
        );
    }
    frame
}

/// Track title and artist (cut with an ellipsis when too long) above a progress bar.
///
/// `progress` is the played fraction, `0.0..=1.0`; out-of-range values are clamped.
pub fn now_playing(title: &str, artist: &str, progress: f32) -> OledFrame {
    let mut frame = blank();
    let title = truncate(title, DEFAULT_WIDTH, FontSize::Small);
    let artist = truncate(artist, DEFAULT_WIDTH, FontSize::Small);
    draw_text_in(
        &mut frame,
        0,
        2,
        DEFAULT_WIDTH,
        &title,
        FontSize::Small,
        Align::Left,
        true,
    );
    draw_text_in(
        &mut frame,
        0,
        13,
        DEFAULT_WIDTH,
        &artist,
        FontSize::Small,
        Align::Left,
        true,
    );
    draw_bar(&mut frame, 0, 29, DEFAULT_WIDTH, 8, progress);
    frame
}

/// A title on an inverted header bar, with the body word-wrapped below it (three lines; an
/// ellipsis marks cut text).
pub fn notification(title: &str, body: &str) -> OledFrame {
    let mut frame = blank();
    let header_h = FontSize::Small.line_height() + 1;
    frame.fill_rect(0, 0, DEFAULT_WIDTH, header_h, true);
    let title = truncate(title, DEFAULT_WIDTH - 4, FontSize::Small);
    draw_text_in(
        &mut frame,
        2,
        1,
        DEFAULT_WIDTH - 4,
        &title,
        FontSize::Small,
        Align::Left,
        false,
    );

    let body_top = header_h + 2;
    let max_lines = lines_per_height(DEFAULT_HEIGHT - body_top, FontSize::Small);
    let mut lines = wrap(body, DEFAULT_WIDTH, FontSize::Small);
    if lines.len() > max_lines {
        lines.truncate(max_lines);
        if let Some(last) = lines.last_mut() {
            *last = mark_cut(last, DEFAULT_WIDTH, FontSize::Small);
        }
    }
    for (i, line) in lines.iter().enumerate() {
        let y = (body_top + i as u32 * FontSize::Small.line_height()) as i32;
        draw_text_in(
            &mut frame,
            0,
            y,
            DEFAULT_WIDTH,
            line,
            FontSize::Small,
            Align::Left,
            true,
        );
    }
    frame
}

/// A label, the percentage in the large font, and a bar. `percent` is 0-100, clamped.
pub fn progress_bar(label: &str, percent_done: f32) -> OledFrame {
    let mut frame = blank();
    let value = percent(percent_done);
    let label = truncate(label, DEFAULT_WIDTH, FontSize::Small);
    draw_text_in(
        &mut frame,
        0,
        0,
        DEFAULT_WIDTH,
        &label,
        FontSize::Small,
        Align::Center,
        true,
    );
    draw_text_in(
        &mut frame,
        0,
        9,
        DEFAULT_WIDTH,
        &format!("{value:.0}%"),
        FontSize::Large,
        Align::Center,
        true,
    );
    draw_bar(&mut frame, 0, 30, DEFAULT_WIDTH, 10, value / 100.0);
    frame
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn rows(frame: &OledFrame) -> Vec<String> {
        frame.to_ascii().lines().map(str::to_string).collect()
    }

    fn any_lit(frame: &OledFrame, ys: std::ops::Range<u32>) -> bool {
        ys.into_iter()
            .any(|y| (0..frame.width()).any(|x| frame.get_pixel(x, y)))
    }

    #[test]
    fn clock_draws_time_and_date() {
        let now = NaiveDate::from_ymd_opt(2026, 10, 1)
            .and_then(|d| d.and_hms_opt(9, 5, 7))
            .unwrap();
        let frame = clock(&now);
        assert_eq!(frame.size(), (128, 40));
        assert!(any_lit(&frame, 6..20), "time row");
        assert!(any_lit(&frame, 26..33), "date row");
        assert!(!any_lit(&frame, 0..6) && !any_lit(&frame, 33..40));

        let expected = {
            let mut f = blank();
            draw_text_in(&mut f, 0, 6, 128, "09:05:07", FontSize::Large, Align::Center, true);
            draw_text_in(
                &mut f,
                0,
                26,
                128,
                "Thu 01 Oct 2026",
                FontSize::Small,
                Align::Center,
                true,
            );
            f
        };
        assert_eq!(frame, expected);
    }

    #[test]
    fn system_stats_bars_follow_the_numbers() {
        let frame = system_stats(&SystemStats {
            cpu_percent: 0.0,
            ram_percent: 100.0,
            ..SystemStats::default()
        });
        // Two rows: pitch 20, text at y = 6 and y = 26; bars one pixel lower, 5 px tall.
        let cpu_bar_mid = 6 + 1 + 2;
        let ram_bar_mid = 26 + 1 + 2;
        assert!(!frame.get_pixel(40, cpu_bar_mid), "empty CPU bar");
        assert!(frame.get_pixel(40, ram_bar_mid), "full RAM bar");

        let four = system_stats(&SystemStats {
            cpu_percent: 50.0,
            ram_percent: 150.0,
            gpu_percent: Some(f32::NAN),
            cpu_temp_celsius: Some(65.0),
        });
        assert!(any_lit(&four, 30..40), "fourth row is drawn");
    }

    #[test]
    fn now_playing_truncates_and_fills_progress() {
        let frame = now_playing("A very long song title that cannot fit", "Artist", 0.5);
        let art = rows(&frame);
        // Bar from y = 29 to 36 with a 2 px inset: inner row 31, filled to about half.
        assert_eq!(&art[31][..3], "#.#");
        assert!(frame.get_pixel(60, 33) && !frame.get_pixel(70, 33));
        // The title line ends in an ellipsis (dots on the glyph's bottom row, y = 2 + 6).
        assert!(art[8].ends_with("#.#.#..."), "{}", art[8]);
    }

    #[test]
    fn notification_has_an_inverted_header_and_wrapped_body() {
        let frame = notification("Mail", "one two three four five six seven eight nine ten eleven twelve");
        // Header row 0 is fully lit; title text is cut out of it (dark pixels on row 1+).
        assert!((0..128).all(|x| frame.get_pixel(x, 0)));
        assert!((0..128).any(|x| !frame.get_pixel(x, 2)));
        assert!(any_lit(&frame, 11..40), "body text");
    }

    #[test]
    fn progress_bar_clamps() {
        let empty = progress_bar("Copying", -5.0);
        let full = progress_bar("Copying", 250.0);
        assert!(!empty.get_pixel(64, 35));
        assert!(full.get_pixel(64, 35));
        assert!(any_lit(&full, 9..23), "large percentage");
    }
}
