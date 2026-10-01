//! Desktop notifications through `notify-send` (libnotify), when it is installed.
//!
//! Failures are logged and swallowed: a missing notification daemon must never break device
//! control.

use std::process::{Command, Stdio};

#[derive(Clone, Copy, Debug)]
pub enum Urgency {
    Low,
    Normal,
    Critical,
}

impl Urgency {
    fn as_arg(self) -> &'static str {
        match self {
            Urgency::Low => "low",
            Urgency::Normal => "normal",
            Urgency::Critical => "critical",
        }
    }
}

/// Show a desktop notification. Returns immediately; the child process is not awaited.
pub fn send(summary: &str, body: &str, urgency: Urgency) {
    let result = Command::new("notify-send")
        .args(["--app-name=SteelSeries GG", "--icon=input-gaming"])
        .arg(format!("--urgency={}", urgency.as_arg()))
        .arg(summary)
        .arg(body)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    match result {
        Ok(mut child) => {
            // Reap in the background so no zombie is left behind.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
        Err(e) => tracing::debug!("notify-send unavailable ({e}); notification dropped: {summary}"),
    }
}

/// Tracks a battery reading and decides when to warn, so a level hovering around the
/// threshold does not produce a stream of notifications.
#[derive(Clone, Debug, Default)]
pub struct BatteryWatch {
    warned_low: bool,
    warned_critical: bool,
}

impl BatteryWatch {
    pub const LOW: u8 = 20;
    pub const CRITICAL: u8 = 10;

    /// Returns the urgency of the warning to show for this reading, if any.
    pub fn observe(&mut self, percent: u8, charging: bool) -> Option<Urgency> {
        if charging || percent > Self::LOW + 5 {
            self.warned_low = false;
            self.warned_critical = false;
            return None;
        }
        if percent <= Self::CRITICAL && !self.warned_critical {
            self.warned_critical = true;
            self.warned_low = true;
            return Some(Urgency::Critical);
        }
        if percent <= Self::LOW && !self.warned_low {
            self.warned_low = true;
            return Some(Urgency::Normal);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn battery_warns_once_per_threshold_with_hysteresis() {
        let mut watch = BatteryWatch::default();
        assert!(watch.observe(80, false).is_none());
        assert!(matches!(watch.observe(19, false), Some(Urgency::Normal)));
        assert!(watch.observe(18, false).is_none());
        assert!(watch.observe(21, false).is_none());
        assert!(watch.observe(19, false).is_none());
        assert!(matches!(watch.observe(9, false), Some(Urgency::Critical)));
        assert!(watch.observe(8, false).is_none());
        assert!(watch.observe(8, true).is_none());
        assert!(watch.observe(30, false).is_none());
        assert!(matches!(watch.observe(15, false), Some(Urgency::Normal)));
    }
}
