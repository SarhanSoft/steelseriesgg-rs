//! Environment detection: is PipeWire running, which tools and plugins exist.

use std::path::{Path, PathBuf};

use serde::Serialize;

use super::pactl::{PACTL, parse_info_json, parse_info_text};
use super::runner::{CommandOutput, CommandRunner, is_not_found};

/// Which external programs could be executed.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ToolAvailability {
    pub pipewire: bool,
    pub pactl: bool,
    pub wpctl: bool,
    pub pw_dump: bool,
    pub pw_cli: bool,
    pub pgrep: bool,
}

/// What [`super::Mixer::detect`] found. `problems` block [`super::Mixer::start`]; `notes`
/// describe reduced functionality.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct DetectReport {
    pub tools: ToolAvailability,
    /// The PulseAudio-protocol server answering `pactl` is PipeWire.
    pub pipewire_running: bool,
    /// `Server Name` from `pactl info`, e.g. `PulseAudio (on PipeWire 1.0.5)`.
    pub server_name: Option<String>,
    /// Version printed by `pipewire --version`.
    pub pipewire_version: Option<String>,
    /// `wpctl status` succeeded, so a session manager (WirePlumber) is linking nodes.
    pub session_manager_running: bool,
    /// `pactl -f json` works (pactl 16+).
    pub pactl_json: bool,
    pub rnnoise_plugin: Option<PathBuf>,
    pub webrtc_aec_found: bool,
    pub sofa_plugin_found: bool,
    pub problems: Vec<String>,
    pub notes: Vec<String>,
}

impl DetectReport {
    pub fn can_start(&self) -> bool {
        self.problems.is_empty()
    }

    /// One line per problem and note.
    pub fn summary(&self) -> String {
        let mut lines = Vec::new();
        if self.problems.is_empty() {
            lines.push(format!(
                "PipeWire mixer can start ({})",
                self.server_name.as_deref().unwrap_or("unknown server")
            ));
        }
        lines.extend(self.problems.iter().map(|p| format!("problem: {p}")));
        lines.extend(self.notes.iter().map(|n| format!("note: {n}")));
        lines.join("\n")
    }
}

/// Run `program args`; `None` when it is not installed.
fn probe_tool(runner: &dyn CommandRunner, program: &str, args: &[&str]) -> Option<CommandOutput> {
    let args: Vec<String> = args.iter().map(|a| (*a).to_string()).collect();
    match runner.run(program, &args) {
        Ok(out) => Some(out),
        Err(e) if is_not_found(&e) => None,
        // Present but misbehaving (e.g. timed out): counts as installed.
        Err(e) => Some(CommandOutput::failed(-1, e.to_string())),
    }
}

/// `pipewire --version` prints `Compiled with libpipewire X` and `Linked with libpipewire Y`;
/// the linked version is the one running.
fn parse_pipewire_version(text: &str) -> Option<String> {
    let find = |prefix: &str| {
        text.lines()
            .find_map(|line| line.trim().strip_prefix(prefix))
            .map(|v| v.trim().to_string())
    };
    find("Linked with libpipewire").or_else(|| find("Compiled with libpipewire"))
}

fn library_dirs(subdir: &str) -> Vec<PathBuf> {
    [
        "/usr/lib",
        "/usr/lib64",
        "/usr/lib/x86_64-linux-gnu",
        "/usr/lib/aarch64-linux-gnu",
        "/usr/local/lib",
    ]
    .iter()
    .map(|base| Path::new(base).join(subdir))
    .collect()
}

/// LADSPA search path: `$LADSPA_PATH`, then the usual distribution directories.
pub(crate) fn ladspa_dirs(env_path: Option<&str>, home: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = env_path
        .unwrap_or_default()
        .split(':')
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .collect();
    dirs.extend(library_dirs("ladspa"));
    dirs.push(PathBuf::from("/run/current-system/sw/lib/ladspa"));
    if let Some(home) = home {
        dirs.push(home.join(".ladspa"));
    }
    dirs
}

pub(crate) fn find_rnnoise(dirs: &[PathBuf], exists: &dyn Fn(&Path) -> bool) -> Option<PathBuf> {
    dirs.iter()
        .map(|dir| dir.join("librnnoise_ladspa.so"))
        .find(|path| exists(path))
}

fn any_exists(paths: &[PathBuf], exists: &dyn Fn(&Path) -> bool) -> bool {
    paths.iter().any(|p| exists(p))
}

pub(crate) fn detect(runner: &dyn CommandRunner, exists: &dyn Fn(&Path) -> bool) -> DetectReport {
    let mut report = DetectReport::default();

    let pipewire = probe_tool(runner, "pipewire", &["--version"]);
    report.tools.pipewire = pipewire.is_some();
    report.pipewire_version = pipewire.as_ref().and_then(|o| parse_pipewire_version(&o.stdout));
    report.tools.pactl = probe_tool(runner, PACTL, &["--version"]).is_some();
    let wpctl = probe_tool(runner, "wpctl", &["status"]);
    report.tools.wpctl = wpctl.is_some();
    report.session_manager_running = wpctl.as_ref().is_some_and(CommandOutput::success);
    report.tools.pw_dump = probe_tool(runner, "pw-dump", &["--version"]).is_some();
    report.tools.pw_cli = probe_tool(runner, "pw-cli", &["--version"]).is_some();
    report.tools.pgrep = probe_tool(runner, "pgrep", &["--version"]).is_some();

    if !report.tools.pipewire {
        report.problems.push("the `pipewire` program is not installed".into());
    }
    if report.tools.pactl {
        match probe_tool(runner, PACTL, &["info"]) {
            Some(out) if out.success() => {
                let info = parse_info_text(&out.stdout);
                report.pipewire_running = info.is_pipewire();
                report.server_name = info.server_name;
                if !report.pipewire_running {
                    report.problems.push(format!(
                        "the sound server is not PipeWire ({}); install and enable pipewire-pulse",
                        report.server_name.as_deref().unwrap_or("unknown")
                    ));
                }
            }
            _ => report
                .problems
                .push("cannot reach a PulseAudio-compatible server; is pipewire-pulse running?".into()),
        }
        report.pactl_json = probe_tool(runner, PACTL, &["-f", "json", "info"])
            .is_some_and(|out| out.success() && parse_info_json(&out.stdout).is_some());
    } else {
        report
            .problems
            .push("`pactl` is not installed (package pulseaudio-utils or libpulse)".into());
    }

    if !report.tools.wpctl {
        report
            .notes
            .push("`wpctl` not found; cannot confirm a session manager (WirePlumber) is running".into());
    } else if !report.session_manager_running {
        report
            .problems
            .push("WirePlumber is not running (`wpctl status` failed); nothing would be linked".into());
    }
    if !(report.tools.pw_dump && report.tools.pw_cli) {
        report
            .notes
            .push("`pw-dump`/`pw-cli` not found; every EQ change restarts the mixer".into());
    }
    if !report.tools.pgrep {
        report
            .notes
            .push("`pgrep` not found; a mixer process left by a crash is found only through its nodes".into());
    }

    let home = std::env::var_os("HOME").map(PathBuf::from);
    let ladspa = std::env::var("LADSPA_PATH").ok();
    report.rnnoise_plugin = find_rnnoise(&ladspa_dirs(ladspa.as_deref(), home.as_deref()), exists);
    if report.rnnoise_plugin.is_none() {
        report
            .notes
            .push("RNNoise (librnnoise_ladspa.so) not found; noise suppression uses WebRTC".into());
    }

    let spa = library_dirs("spa-0.2");
    let modules = library_dirs("pipewire-0.3");
    let join = |dirs: &[PathBuf], file: &str| dirs.iter().map(|d| d.join(file)).collect::<Vec<_>>();
    report.webrtc_aec_found = any_exists(&join(&spa, "aec/libspa-aec-webrtc.so"), exists)
        && any_exists(&join(&modules, "libpipewire-module-echo-cancel.so"), exists);
    if !report.webrtc_aec_found {
        report
            .notes
            .push("WebRTC echo-cancel module not found in the usual paths; WebRTC noise suppression may fail".into());
    }
    // The sofa plugin moved from a filter-chain module (PipeWire < 1.4) to a filter-graph SPA
    // plugin (1.4+).
    let mut sofa = join(&spa, "filter-graph/libspa-filter-graph-plugin-sofa.so");
    sofa.extend(join(&modules, "filter-chain/libpipewire-module-filter-chain-sofa.so"));
    report.sofa_plugin_found = any_exists(&sofa, exists);
    if !report.sofa_plugin_found {
        report
            .notes
            .push("PipeWire sofa plugin (libmysofa) not found in the usual paths; spatial audio may fail".into());
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mixer::pactl::samples::{INFO_JSON, INFO_TEXT};
    use crate::mixer::runner::FakeRunner;

    fn healthy_fake() -> FakeRunner {
        let fake = FakeRunner::new();
        fake.on(
            "pipewire",
            &["--version"],
            CommandOutput::ok("pipewire\nCompiled with libpipewire 1.0.5\nLinked with libpipewire 1.0.7\n"),
        );
        fake.on("pactl", &["info"], CommandOutput::ok(INFO_TEXT));
        fake.on("pactl", &["-f", "json", "info"], CommandOutput::ok(INFO_JSON));
        fake
    }

    #[test]
    fn healthy_system_can_start() {
        let fake = healthy_fake();
        let report = detect(&fake, &|p: &Path| {
            p == Path::new("/usr/lib/ladspa/librnnoise_ladspa.so")
                || p.ends_with("aec/libspa-aec-webrtc.so")
                || p.ends_with("libpipewire-module-echo-cancel.so")
        });
        assert!(report.can_start(), "{}", report.summary());
        assert!(report.pipewire_running);
        assert_eq!(report.pipewire_version.as_deref(), Some("1.0.7"));
        assert!(report.pactl_json);
        assert!(report.session_manager_running);
        assert!(report.tools.pw_dump && report.tools.pw_cli && report.tools.pgrep);
        assert_eq!(
            report.rnnoise_plugin.as_deref(),
            Some(Path::new("/usr/lib/ladspa/librnnoise_ladspa.so"))
        );
        assert!(report.webrtc_aec_found);
        assert!(!report.sofa_plugin_found);
        assert!(report.notes.iter().any(|n| n.contains("sofa")));
    }

    #[test]
    fn missing_pipewire_and_pactl_are_problems_not_panics() {
        let fake = FakeRunner::new();
        for tool in ["pipewire", "pactl", "wpctl", "pw-dump", "pw-cli", "pgrep"] {
            fake.set_missing(tool);
        }
        let report = detect(&fake, &|_: &Path| false);
        assert!(!report.can_start());
        assert!(!report.pipewire_running);
        assert_eq!(report.tools, ToolAvailability::default());
        assert!(report.problems.iter().any(|p| p.contains("pipewire")));
        assert!(report.problems.iter().any(|p| p.contains("pactl")));
        assert!(report.summary().contains("problem:"));
    }

    #[test]
    fn plain_pulseaudio_is_a_problem() {
        let fake = healthy_fake();
        fake.on("pactl", &["info"], CommandOutput::ok("Server Name: pulseaudio\n"));
        let report = detect(&fake, &|_: &Path| false);
        assert!(!report.pipewire_running);
        assert!(report.problems.iter().any(|p| p.contains("not PipeWire")));
    }

    #[test]
    fn unreachable_server_and_dead_session_manager() {
        let fake = healthy_fake();
        fake.on(
            "pactl",
            &["info"],
            CommandOutput::failed(1, "Connection failure: Connection refused"),
        );
        fake.on("pactl", &["-f", "json", "info"], CommandOutput::failed(1, ""));
        fake.on(
            "wpctl",
            &["status"],
            CommandOutput::failed(1, "Could not connect to PipeWire"),
        );
        let report = detect(&fake, &|_: &Path| false);
        assert!(!report.pactl_json);
        assert!(report.problems.iter().any(|p| p.contains("pipewire-pulse")));
        assert!(report.problems.iter().any(|p| p.contains("WirePlumber")));
    }

    #[test]
    fn ladspa_search_path_order() {
        let dirs = ladspa_dirs(Some("/opt/a::/opt/b"), Some(Path::new("/home/u")));
        assert_eq!(dirs[0], PathBuf::from("/opt/a"));
        assert_eq!(dirs[1], PathBuf::from("/opt/b"));
        assert!(dirs.contains(&PathBuf::from("/usr/lib/ladspa")));
        assert_eq!(dirs.last(), Some(&PathBuf::from("/home/u/.ladspa")));
        let found = find_rnnoise(&dirs, &|p: &Path| p.starts_with("/opt/b"));
        assert_eq!(found, Some(PathBuf::from("/opt/b/librnnoise_ladspa.so")));
    }
}
