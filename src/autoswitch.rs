//! Automatic profile switching while a program runs (GG's "link profile to application").
//!
//! Detection is by running process, which works the same under X11 and every Wayland
//! compositor. Names are compared case-insensitively without a trailing `.exe`, against the
//! process name, its executable file name, and any `*.exe` path in its command line — the
//! last one catches Windows games under Proton/Wine, whose process name the kernel truncates
//! to 15 characters.

use std::collections::{BTreeSet, HashSet};

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

/// Normalise a program name for comparison: lowercase, no directory, no `.exe`.
pub fn normalize(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name).trim();
    let lower = base.to_ascii_lowercase();
    lower.strip_suffix(".exe").map(str::to_string).unwrap_or(lower)
}

/// Snapshot of running program names (normalised).
pub struct ProcessWatcher {
    system: System,
}

impl Default for ProcessWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessWatcher {
    pub fn new() -> Self {
        Self { system: System::new() }
    }

    pub fn running(&mut self) -> HashSet<String> {
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing()
                .with_exe(UpdateKind::OnlyIfNotSet)
                .with_cmd(UpdateKind::OnlyIfNotSet),
        );
        let mut names = HashSet::new();
        for process in self.system.processes().values() {
            names.insert(normalize(&process.name().to_string_lossy()));
            if let Some(exe) = process.exe() {
                names.insert(normalize(&exe.to_string_lossy()));
            }
            for arg in process.cmd() {
                let arg = arg.to_string_lossy();
                if arg.to_ascii_lowercase().ends_with(".exe") {
                    names.insert(normalize(&arg));
                }
            }
        }
        names.remove("");
        names
    }
}

/// One profile and the programs that activate it.
#[derive(Clone, Debug)]
pub struct Rule {
    pub profile: String,
    pub apps: Vec<String>,
}

/// Decides when to switch profiles. Pure logic, driven by [`AutoSwitch::decide`].
#[derive(Debug, Default)]
pub struct AutoSwitch {
    /// Profile that was active before an automatic switch, restored when the program exits.
    base: Option<String>,
    /// Profile we switched to automatically.
    auto_active: Option<String>,
    /// Profiles the user switched away from by hand while their program kept running; they
    /// are not re-applied until that program exits.
    suppressed: BTreeSet<String>,
}

impl AutoSwitch {
    /// Given the active profile, the rules and the running programs, return the profile to
    /// load now, if any. `fallback` is used when a program exits and nothing was active before.
    pub fn decide(
        &mut self,
        current: Option<&str>,
        rules: &[Rule],
        running: &HashSet<String>,
        fallback: Option<&str>,
    ) -> Option<String> {
        let matched: Vec<&str> = rules
            .iter()
            .filter(|rule| rule.apps.iter().any(|app| running.contains(&normalize(app))))
            .map(|rule| rule.profile.as_str())
            .collect();

        // Forget suppressions whose program has exited.
        self.suppressed.retain(|p| matched.contains(&p.as_str()));

        // The user loaded something else by hand while an automatic profile was active.
        if let Some(auto) = &self.auto_active
            && current != Some(auto.as_str())
        {
            self.suppressed.insert(auto.clone());
            self.auto_active = None;
            self.base = None;
        }

        if let Some(auto) = &self.auto_active
            && matched.contains(&auto.as_str())
        {
            return None;
        }

        let candidate = matched.iter().find(|p| !self.suppressed.contains(**p)).copied();
        match (candidate, self.auto_active.take()) {
            (Some(profile), previous) => {
                if previous.is_none() {
                    self.base = current.map(str::to_string);
                }
                self.auto_active = Some(profile.to_string());
                (current != Some(profile)).then(|| profile.to_string())
            }
            (None, Some(_)) => {
                let restore = self.base.take().or_else(|| fallback.map(str::to_string));
                restore.filter(|r| current != Some(r.as_str()))
            }
            (None, None) => None,
        }
    }

    pub fn auto_active(&self) -> Option<&str> {
        self.auto_active.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> Vec<Rule> {
        vec![
            Rule {
                profile: "fps".into(),
                apps: vec!["cs2".into(), "Cyberpunk2077.exe".into()],
            },
            Rule {
                profile: "work".into(),
                apps: vec!["code".into()],
            },
        ]
    }

    fn running(names: &[&str]) -> HashSet<String> {
        names.iter().map(|n| normalize(n)).collect()
    }

    #[test]
    fn normalizes_paths_case_and_exe_suffix() {
        assert_eq!(normalize("Z:\\Games\\Cyberpunk2077.EXE"), "cyberpunk2077");
        assert_eq!(normalize("/usr/bin/code"), "code");
        assert_eq!(normalize(" cs2 "), "cs2");
    }

    #[test]
    fn switches_in_and_back_out() {
        let mut auto = AutoSwitch::default();
        let rules = rules();
        assert_eq!(auto.decide(Some("default"), &rules, &running(&["bash"]), None), None);
        assert_eq!(
            auto.decide(Some("default"), &rules, &running(&["cs2"]), None),
            Some("fps".into())
        );
        assert_eq!(auto.decide(Some("fps"), &rules, &running(&["cs2"]), None), None);
        assert_eq!(
            auto.decide(Some("fps"), &rules, &running(&["bash"]), None),
            Some("default".into())
        );
        assert_eq!(auto.decide(Some("default"), &rules, &running(&["bash"]), None), None);
    }

    #[test]
    fn falls_back_when_nothing_was_active() {
        let mut auto = AutoSwitch::default();
        let rules = rules();
        assert_eq!(
            auto.decide(None, &rules, &running(&["code"]), Some("base")),
            Some("work".into())
        );
        assert_eq!(
            auto.decide(Some("work"), &rules, &running(&[]), Some("base")),
            Some("base".into())
        );
    }

    #[test]
    fn manual_change_suppresses_until_the_program_exits() {
        let mut auto = AutoSwitch::default();
        let rules = rules();
        assert_eq!(
            auto.decide(Some("default"), &rules, &running(&["cs2"]), None),
            Some("fps".into())
        );
        // User picks "rgb-party" by hand while cs2 still runs: leave it alone.
        assert_eq!(auto.decide(Some("rgb-party"), &rules, &running(&["cs2"]), None), None);
        assert_eq!(auto.decide(Some("rgb-party"), &rules, &running(&["cs2"]), None), None);
        // cs2 exits, then starts again: switching resumes.
        assert_eq!(auto.decide(Some("rgb-party"), &rules, &running(&[]), None), None);
        assert_eq!(
            auto.decide(Some("rgb-party"), &rules, &running(&["cs2"]), None),
            Some("fps".into())
        );
    }

    #[test]
    fn moves_between_matching_programs() {
        let mut auto = AutoSwitch::default();
        let rules = rules();
        assert_eq!(
            auto.decide(Some("default"), &rules, &running(&["code"]), None),
            Some("work".into())
        );
        assert_eq!(
            auto.decide(Some("work"), &rules, &running(&["cs2"]), None),
            Some("fps".into())
        );
        assert_eq!(
            auto.decide(Some("fps"), &rules, &running(&[]), None),
            Some("default".into())
        );
    }
}
