//! What the keyboard OLED shows: an idle screen (clock, system stats, now playing, an image)
//! interrupted by temporary content (text from the CLI, notifications, GameSense screens).

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sysinfo::System;

use crate::oled::{self, ImageOptions, OledFrame, SystemStats};
use crate::{Error, Result};

/// The screen shown when nothing temporary is on.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum IdleScreen {
    #[default]
    Clock,
    Stats,
    /// Track from any MPRIS player, through `playerctl`; falls back to the clock.
    NowPlaying,
    /// A still or animated image file (PNG, GIF, JPEG, BMP).
    Image {
        path: String,
    },
    /// Raw 1-bit pixels, row-major, MSB first (uploaded from the control panel).
    Bitmap {
        width: u32,
        height: u32,
        data: Vec<u8>,
    },
    /// Leave the keyboard's own screen alone.
    Off,
}

/// Something shown for a while on top of the idle screen.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScreenContent {
    Text {
        lines: Vec<String>,
    },
    Image {
        path: String,
    },
    Notification {
        title: String,
        body: String,
    },
    Progress {
        label: String,
        percent: f32,
    },
    /// Raw 1-bit pixels, row-major, MSB first (GameSense image frames).
    Bitmap {
        width: u32,
        height: u32,
        data: Vec<u8>,
    },
}

/// Decode `content` into frames for a `width` x `height` screen.
pub fn render_content(content: &ScreenContent, width: u32, height: u32) -> Result<Vec<(OledFrame, Duration)>> {
    let still = |frame: OledFrame| vec![(frame, Duration::from_secs(3600))];
    Ok(match content {
        ScreenContent::Text { lines } => {
            let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
            still(oled::render_lines(&refs))
        }
        ScreenContent::Image { path } => load_image(path, width, height)?,
        ScreenContent::Notification { title, body } => still(oled::notification(title, body)),
        ScreenContent::Progress { label, percent } => still(oled::progress_bar(label, *percent)),
        ScreenContent::Bitmap { width, height, data } => still(OledFrame::from_packed(*width, *height, data.clone())?),
    })
}

fn load_image(path: &str, width: u32, height: u32) -> Result<Vec<(OledFrame, Duration)>> {
    let path = Path::new(path);
    let bytes =
        std::fs::read(path).map_err(|e| Error::InvalidConfig(format!("cannot read image {}: {e}", path.display())))?;
    let options = ImageOptions {
        width,
        height,
        ..ImageOptions::default()
    };
    oled::frames_from_image_bytes(&bytes, &options)
}

struct Timed {
    frames: Vec<(OledFrame, Duration)>,
    started: Instant,
    until: Option<Instant>,
}

impl Timed {
    fn frame_at(&self, now: Instant) -> Option<&OledFrame> {
        let total: Duration = self.frames.iter().map(|(_, d)| *d).sum();
        if self.frames.is_empty() || total.is_zero() {
            return self.frames.first().map(|(f, _)| f);
        }
        let mut t = Duration::from_nanos((now.duration_since(self.started).as_nanos() % total.as_nanos()) as u64);
        for (frame, delay) in &self.frames {
            if t < *delay {
                return Some(frame);
            }
            t -= *delay;
        }
        self.frames.last().map(|(f, _)| f)
    }
}

struct NowPlaying {
    title: String,
    artist: String,
    progress: f32,
}

/// Chooses the frame to show at any moment.
pub struct ScreenManager {
    idle: IdleScreen,
    idle_image: Option<Timed>,
    temporary: Option<Timed>,
    system: System,
    stats: Option<(Instant, SystemStats)>,
    playing: Option<(Instant, Option<NowPlaying>)>,
    width: u32,
    height: u32,
}

impl ScreenManager {
    pub fn new(idle: IdleScreen) -> Self {
        let mut manager = Self {
            idle: IdleScreen::Off,
            idle_image: None,
            temporary: None,
            system: System::new(),
            stats: None,
            playing: None,
            width: oled::DEFAULT_WIDTH,
            height: oled::DEFAULT_HEIGHT,
        };
        if let Err(e) = manager.set_idle(idle) {
            tracing::warn!("OLED idle screen: {e}");
        }
        manager
    }

    pub fn idle(&self) -> &IdleScreen {
        &self.idle
    }

    pub fn set_idle(&mut self, idle: IdleScreen) -> Result<()> {
        self.idle_image = match &idle {
            IdleScreen::Image { path } => Some(Timed {
                frames: load_image(path, self.width, self.height)?,
                started: Instant::now(),
                until: None,
            }),
            IdleScreen::Bitmap { width, height, data } => Some(Timed {
                frames: vec![(
                    OledFrame::from_packed(*width, *height, data.clone())?,
                    Duration::from_secs(3600),
                )],
                started: Instant::now(),
                until: None,
            }),
            _ => None,
        };
        self.idle = idle;
        Ok(())
    }

    /// Show `content` for `duration` (`None` = until replaced).
    pub fn show(&mut self, content: &ScreenContent, duration: Option<Duration>) -> Result<()> {
        let now = Instant::now();
        self.temporary = Some(Timed {
            frames: render_content(content, self.width, self.height)?,
            started: now,
            until: duration.map(|d| now + d),
        });
        Ok(())
    }

    pub fn clear_temporary(&mut self) {
        self.temporary = None;
    }

    /// The frame for this instant; `None` means leave the screen to the keyboard.
    pub fn frame(&mut self, now: Instant) -> Option<OledFrame> {
        if let Some(temp) = &self.temporary {
            if temp.until.is_some_and(|until| now >= until) {
                self.temporary = None;
            } else {
                return temp.frame_at(now).cloned();
            }
        }
        match self.idle.clone() {
            IdleScreen::Off => None,
            IdleScreen::Clock => Some(oled::clock(&chrono::Local::now())),
            IdleScreen::Stats => Some(oled::system_stats(&self.sample_stats(now))),
            IdleScreen::NowPlaying => match self.sample_playing(now) {
                Some(p) => Some(oled::now_playing(&p.title, &p.artist, p.progress)),
                None => Some(oled::clock(&chrono::Local::now())),
            },
            IdleScreen::Image { .. } | IdleScreen::Bitmap { .. } => {
                self.idle_image.as_ref().and_then(|t| t.frame_at(now).cloned())
            }
        }
    }

    fn sample_stats(&mut self, now: Instant) -> SystemStats {
        if let Some((at, stats)) = self.stats
            && now.duration_since(at) < Duration::from_secs(2)
        {
            return stats;
        }
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        let total = self.system.total_memory().max(1) as f32;
        let stats = SystemStats {
            cpu_percent: self.system.global_cpu_usage(),
            ram_percent: self.system.used_memory() as f32 / total * 100.0,
            gpu_percent: None,
            cpu_temp_celsius: None,
        };
        self.stats = Some((now, stats));
        stats
    }

    fn sample_playing(&mut self, now: Instant) -> Option<&NowPlaying> {
        let stale = self
            .playing
            .as_ref()
            .is_none_or(|(at, _)| now.duration_since(*at) >= Duration::from_secs(1));
        if stale {
            self.playing = Some((now, query_playerctl()));
        }
        self.playing.as_ref().and_then(|(_, p)| p.as_ref())
    }
}

/// Ask `playerctl` for the current track; `None` when nothing plays or it is not installed.
fn query_playerctl() -> Option<NowPlaying> {
    let output = Command::new("playerctl")
        .args([
            "metadata",
            "--format",
            "{{status}}\t{{title}}\t{{artist}}\t{{position}}\t{{mpris:length}}",
        ])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_playerctl(&String::from_utf8_lossy(&output.stdout))
}

fn parse_playerctl(line: &str) -> Option<NowPlaying> {
    let mut parts = line.trim_end_matches(['\n', '\r']).split('\t');
    let status = parts.next()?;
    if status != "Playing" && status != "Paused" {
        return None;
    }
    let title = parts.next().unwrap_or("").to_string();
    let artist = parts.next().unwrap_or("").to_string();
    let position: f64 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0.0);
    let length: f64 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0.0);
    if title.is_empty() {
        return None;
    }
    let progress = if length > 0.0 { (position / length) as f32 } else { 0.0 };
    Some(NowPlaying {
        title,
        artist,
        progress: progress.clamp(0.0, 1.0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temporary_content_expires_back_to_idle() {
        let mut screens = ScreenManager::new(IdleScreen::Off);
        let start = Instant::now();
        screens
            .show(
                &ScreenContent::Text {
                    lines: vec!["HELLO".into()],
                },
                Some(Duration::from_millis(50)),
            )
            .unwrap();
        let frame = screens.frame(start).expect("text is shown");
        assert!(frame.count_lit() > 0);
        assert!(
            screens.frame(start + Duration::from_millis(60)).is_none(),
            "idle is Off"
        );
    }

    #[test]
    fn clock_idle_always_draws() {
        let mut screens = ScreenManager::new(IdleScreen::Clock);
        assert!(screens.frame(Instant::now()).is_some_and(|f| f.count_lit() > 0));
    }

    #[test]
    fn animation_picks_frames_by_elapsed_time() {
        let a = oled::render_lines(&["A"]);
        let b = oled::render_lines(&["B"]);
        let start = Instant::now();
        let timed = Timed {
            frames: vec![
                (a.clone(), Duration::from_millis(100)),
                (b.clone(), Duration::from_millis(100)),
            ],
            started: start,
            until: None,
        };
        assert_eq!(timed.frame_at(start + Duration::from_millis(50)), Some(&a));
        assert_eq!(timed.frame_at(start + Duration::from_millis(150)), Some(&b));
        assert_eq!(timed.frame_at(start + Duration::from_millis(250)), Some(&a));
    }

    #[test]
    fn parses_playerctl_output() {
        let p = parse_playerctl("Playing\tSong\tBand\t30000000\t120000000\n").unwrap();
        assert_eq!(p.title, "Song");
        assert_eq!(p.artist, "Band");
        assert!((p.progress - 0.25).abs() < 1e-6);
        assert!(parse_playerctl("Stopped\t\t\t\t").is_none());
        assert!(parse_playerctl("Playing\t\tBand\t1\t2").is_none());
    }

    #[test]
    fn bitmap_content_must_match_its_size() {
        let ok = ScreenContent::Bitmap {
            width: 128,
            height: 40,
            data: vec![0; 640],
        };
        assert!(render_content(&ok, 128, 40).is_ok());
        let bad = ScreenContent::Bitmap {
            width: 128,
            height: 40,
            data: vec![0; 10],
        };
        assert!(render_content(&bad, 128, 40).is_err());
    }
}
