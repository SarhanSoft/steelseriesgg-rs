//! Moments: instant-replay clips (GG Moments) through `gpu-screen-recorder`.
//!
//! The daemon keeps `gpu-screen-recorder` running in replay mode, holding the last N seconds
//! in memory; saving a clip sends it `SIGUSR1`, which makes it write the buffer to the output
//! folder. Nothing is written to disk until a clip is saved.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// `[moments]` in config.toml.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct MomentsConfig {
    /// Keep the replay buffer running while the daemon runs.
    pub enabled: bool,
    /// Seconds kept in the replay buffer.
    pub replay_seconds: u32,
    /// Folder for saved clips; empty = `~/Videos/SteelSeries Moments`.
    pub output_dir: String,
    /// What to capture: `screen`, `portal` (GNOME/KDE Wayland), a monitor name, or `focused`.
    pub capture: String,
    pub fps: u32,
    /// gpu-screen-recorder quality: medium, high, very_high, ultra.
    pub quality: String,
    /// Audio source: `default_output`, `default_input`, or a PulseAudio/PipeWire device name.
    pub audio: String,
    /// Container: mp4, mkv, flv...
    pub container: String,
}

impl Default for MomentsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            replay_seconds: 30,
            output_dir: String::new(),
            capture: "screen".to_string(),
            fps: 60,
            quality: "very_high".to_string(),
            audio: "default_output".to_string(),
            container: "mp4".to_string(),
        }
    }
}

impl MomentsConfig {
    pub fn output_dir(&self) -> PathBuf {
        if !self.output_dir.trim().is_empty() {
            return PathBuf::from(self.output_dir.trim());
        }
        directories::UserDirs::new()
            .and_then(|dirs| dirs.video_dir().map(|p| p.to_path_buf()))
            .or_else(|| directories::BaseDirs::new().map(|d| d.home_dir().join("Videos")))
            .unwrap_or_else(|| PathBuf::from("."))
            .join("SteelSeries Moments")
    }

    /// Command-line arguments for gpu-screen-recorder in replay mode.
    pub fn recorder_args(&self) -> Vec<String> {
        vec![
            "-w".to_string(),
            self.capture.clone(),
            "-f".to_string(),
            self.fps.to_string(),
            "-a".to_string(),
            self.audio.clone(),
            "-q".to_string(),
            self.quality.clone(),
            "-c".to_string(),
            self.container.clone(),
            "-r".to_string(),
            self.replay_seconds.clamp(5, 1200).to_string(),
            "-o".to_string(),
            self.output_dir().to_string_lossy().into_owned(),
        ]
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MomentsStatus {
    pub enabled: bool,
    pub recording: bool,
    pub replay_seconds: u32,
    pub output_dir: String,
    pub last_error: Option<String>,
}

/// Owns the gpu-screen-recorder child process.
pub struct MomentsRecorder {
    config: MomentsConfig,
    child: Option<Child>,
    last_error: Option<String>,
    next_restart: Option<Instant>,
    failures: u32,
}

impl MomentsRecorder {
    pub fn new(config: MomentsConfig) -> Self {
        Self {
            config,
            child: None,
            last_error: None,
            next_restart: None,
            failures: 0,
        }
    }

    pub fn config(&self) -> &MomentsConfig {
        &self.config
    }

    pub fn set_config(&mut self, config: MomentsConfig) {
        let restart = self.is_running();
        self.stop();
        self.config = config;
        if restart || self.config.enabled {
            self.next_restart = Some(Instant::now());
        }
    }

    pub fn is_running(&mut self) -> bool {
        match self.child.as_mut().map(|c| c.try_wait()) {
            Some(Ok(None)) => true,
            Some(Ok(Some(status))) => {
                self.child = None;
                self.last_error = Some(format!("gpu-screen-recorder exited ({status})"));
                false
            }
            Some(Err(e)) => {
                self.child = None;
                self.last_error = Some(format!("gpu-screen-recorder state unknown: {e}"));
                false
            }
            None => false,
        }
    }

    /// Start the replay buffer.
    pub fn start(&mut self) -> Result<()> {
        if self.is_running() {
            return Ok(());
        }
        let dir = self.config.output_dir();
        std::fs::create_dir_all(&dir)?;
        let child = Command::new("gpu-screen-recorder")
            .args(self.config.recorder_args())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| {
                let message = if e.kind() == std::io::ErrorKind::NotFound {
                    "gpu-screen-recorder is not installed (Arch: gpu-screen-recorder; Flatpak: \
                     com.dec05eba.gpu_screen_recorder)"
                        .to_string()
                } else {
                    format!("could not start gpu-screen-recorder: {e}")
                };
                self.last_error = Some(message.clone());
                Error::Unsupported(message)
            })?;
        self.child = Some(child);
        self.last_error = None;
        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            #[cfg(unix)]
            {
                if let Ok(pid) = libc::pid_t::try_from(child.id()) {
                    // SAFETY: `kill` takes a plain pid and signal number and touches no memory;
                    // `pid` is our own live child, so the signal cannot reach another process.
                    unsafe {
                        libc::kill(pid, libc::SIGINT);
                    }
                }
                let deadline = Instant::now() + Duration::from_secs(3);
                while Instant::now() < deadline {
                    if matches!(child.try_wait(), Ok(Some(_))) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
            let _ = child.kill();
            let _ = child.wait();
        }
        self.next_restart = None;
    }

    /// Save the replay buffer to a clip.
    pub fn save_clip(&mut self) -> Result<PathBuf> {
        if !self.is_running() {
            return Err(Error::Unsupported(self.last_error.clone().unwrap_or_else(|| {
                "the replay buffer is not running (enable [moments] in config.toml)".to_string()
            })));
        }
        #[cfg(unix)]
        {
            let child = self
                .child
                .as_ref()
                .ok_or_else(|| Error::Other("replay buffer vanished".to_string()))?;
            let pid =
                libc::pid_t::try_from(child.id()).map_err(|_| Error::Other("recorder pid out of range".to_string()))?;
            // SAFETY: as in `stop`: no memory is touched and `pid` is our own live child.
            let rc = unsafe { libc::kill(pid, libc::SIGUSR1) };
            if rc != 0 {
                return Err(Error::Io(std::io::Error::last_os_error()));
            }
            Ok(self.config.output_dir())
        }
        #[cfg(not(unix))]
        {
            Err(Error::Unsupported("Moments needs Linux".to_string()))
        }
    }

    /// Keep the recorder alive when enabled, restarting with backoff after a crash.
    pub fn supervise(&mut self) {
        if !self.config.enabled || self.is_running() {
            return;
        }
        let now = Instant::now();
        if self.next_restart.is_some_and(|t| now < t) {
            return;
        }
        match self.start() {
            Ok(()) => self.failures = 0,
            Err(e) => {
                self.failures = self.failures.saturating_add(1);
                let backoff = Duration::from_secs(5u64.saturating_mul(1 << self.failures.min(6)));
                if self.failures == 1 {
                    tracing::warn!("Moments: {e}");
                }
                self.next_restart = Some(now + backoff);
            }
        }
    }

    pub fn status(&mut self) -> MomentsStatus {
        MomentsStatus {
            enabled: self.config.enabled,
            recording: self.is_running(),
            replay_seconds: self.config.replay_seconds,
            output_dir: self.config.output_dir().to_string_lossy().into_owned(),
            last_error: self.last_error.clone(),
        }
    }
}

impl Drop for MomentsRecorder {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recorder_args_use_replay_mode_with_a_folder() {
        let config = MomentsConfig {
            output_dir: "/tmp/clips".into(),
            replay_seconds: 2,
            ..MomentsConfig::default()
        };
        let args = config.recorder_args();
        let pos = |flag: &str| args.iter().position(|a| a == flag).unwrap();
        assert_eq!(args[pos("-r") + 1], "5", "replay length is clamped to at least 5 s");
        assert_eq!(args[pos("-o") + 1], "/tmp/clips");
        assert_eq!(args[pos("-w") + 1], "screen");
        assert_eq!(args[pos("-a") + 1], "default_output");
    }

    #[test]
    fn default_output_dir_is_under_videos() {
        let dir = MomentsConfig::default().output_dir();
        assert!(dir.ends_with("SteelSeries Moments"));
    }

    #[test]
    fn saving_without_a_recorder_explains_why() {
        let mut recorder = MomentsRecorder::new(MomentsConfig::default());
        let err = recorder.save_clip().unwrap_err().to_string();
        assert!(err.contains("not running") || err.contains("Linux"), "{err}");
    }
}
