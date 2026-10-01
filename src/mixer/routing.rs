//! Per-application routing rules: which channel each application plays to.

use serde::{Deserialize, Serialize};

use super::Channel;
use crate::{Error, Result};

/// Which stream property a rule compares.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchField {
    /// `application.name` or `application.process.binary`.
    #[default]
    Any,
    /// `application.name`, e.g. `"Firefox"`, `"WEBRTC VoiceEngine"`.
    ApplicationName,
    /// `application.process.binary`, e.g. `"firefox"`, `"Discord"`.
    ProcessBinary,
}

/// The identifying properties of a playback stream.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AppIdentity {
    pub application_name: Option<String>,
    pub process_binary: Option<String>,
}

/// Route streams whose property matches `pattern` to `channel`.
///
/// Matching ignores case. `*` matches any run of characters; a pattern without `*` must equal
/// the whole property value.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RoutingRule {
    #[serde(default)]
    pub field: MatchField,
    pub pattern: String,
    pub channel: Channel,
}

impl RoutingRule {
    pub fn new(field: MatchField, pattern: impl Into<String>, channel: Channel) -> Self {
        Self {
            field,
            pattern: pattern.into(),
            channel,
        }
    }

    pub fn matches(&self, app: &AppIdentity) -> bool {
        let check = |value: &Option<String>| value.as_deref().is_some_and(|v| glob_match(&self.pattern, v));
        match self.field {
            MatchField::Any => check(&app.application_name) || check(&app.process_binary),
            MatchField::ApplicationName => check(&app.application_name),
            MatchField::ProcessBinary => check(&app.process_binary),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.pattern.trim().is_empty() {
            return Err(Error::InvalidConfig("routing rule pattern is empty".into()));
        }
        if !self.channel.is_output() {
            return Err(Error::InvalidConfig(format!(
                "routing rule '{}' targets the microphone; applications can only be routed to Game, Chat, Media or Aux",
                self.pattern
            )));
        }
        Ok(())
    }

    fn same_selector(&self, other: &RoutingRule) -> bool {
        self.field == other.field && self.pattern.eq_ignore_ascii_case(&other.pattern)
    }
}

/// Ordered routing rules; the first match wins, unmatched streams go to `default_channel`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct RoutingConfig {
    pub default_channel: Channel,
    pub rules: Vec<RoutingRule>,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            default_channel: Channel::Game,
            rules: default_rules(),
        }
    }
}

impl RoutingConfig {
    pub fn channel_for(&self, app: &AppIdentity) -> Channel {
        self.rules
            .iter()
            .find(|rule| rule.matches(app))
            .map_or(self.default_channel, |rule| rule.channel)
    }

    /// Put `rule` first (so it beats the defaults), replacing any rule with the same field and
    /// pattern.
    pub fn upsert(&mut self, rule: RoutingRule) {
        self.rules.retain(|existing| !existing.same_selector(&rule));
        self.rules.insert(0, rule);
    }

    pub fn validate(&self) -> Result<()> {
        if !self.default_channel.is_output() {
            return Err(Error::InvalidConfig(
                "routing default channel must be Game, Chat, Media or Aux".into(),
            ));
        }
        self.rules.iter().try_for_each(RoutingRule::validate)
    }
}

/// Voice apps to Chat, music players and browsers to Media; everything else falls through to
/// the default channel (Game).
pub fn default_rules() -> Vec<RoutingRule> {
    use Channel::{Chat, Media};
    use MatchField::{Any, ProcessBinary};

    let rules: [(MatchField, &str, Channel); 22] = [
        // Discord's voice stream is named "WEBRTC VoiceEngine"; its binary is "Discord" (or
        // ".Discord-wrapped" on NixOS), so match the binary loosely.
        (Any, "*discord*", Chat),
        (Any, "vesktop", Chat),
        (Any, "webcord", Chat),
        (Any, "*teamspeak*", Chat),
        (ProcessBinary, "ts3client*", Chat),
        (Any, "*mumble*", Chat),
        (Any, "zoom*", Chat),
        (Any, "teams", Chat),
        (Any, "teams-for-linux", Chat),
        (Any, "msteams", Chat),
        (Any, "skype*", Chat),
        (Any, "spotify*", Media),
        (Any, "firefox*", Media),
        (Any, "librewolf*", Media),
        (Any, "*chrome*", Media),
        (Any, "chromium*", Media),
        (Any, "brave*", Media),
        (Any, "vivaldi*", Media),
        (Any, "opera*", Media),
        (Any, "*msedge*", Media),
        (Any, "vlc*", Media),
        (Any, "mpv*", Media),
    ];
    rules
        .into_iter()
        .map(|(field, pattern, channel)| RoutingRule::new(field, pattern, channel))
        .collect()
}

/// Case-insensitive match where `*` stands for any run of characters.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.to_lowercase().chars().collect();
    let text: Vec<char> = text.to_lowercase().chars().collect();

    let (mut p, mut t) = (0, 0);
    let mut backtrack: Option<(usize, usize)> = None;
    while t < text.len() {
        if p < pattern.len() && pattern[p] == '*' {
            backtrack = Some((p, t));
            p += 1;
        } else if p < pattern.len() && pattern[p] == text[t] {
            p += 1;
            t += 1;
        } else if let Some((star, matched)) = backtrack {
            p = star + 1;
            t = matched + 1;
            backtrack = Some((star, matched + 1));
        } else {
            return false;
        }
    }
    pattern[p..].iter().all(|&c| c == '*')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(name: Option<&str>, binary: Option<&str>) -> AppIdentity {
        AppIdentity {
            application_name: name.map(str::to_string),
            process_binary: binary.map(str::to_string),
        }
    }

    #[test]
    fn glob_matching() {
        assert!(glob_match("firefox", "Firefox"));
        assert!(!glob_match("firefox", "firefox-bin"));
        assert!(glob_match("firefox*", "firefox-bin"));
        assert!(glob_match("*discord*", ".Discord-wrapped"));
        assert!(glob_match("*", ""));
        assert!(glob_match("a*b*c", "aXXbYYc"));
        assert!(!glob_match("a*b*c", "aXXbYY"));
        assert!(glob_match("*chrome*", "Google Chrome"));
        assert!(!glob_match("teams", "steam"));
    }

    #[test]
    fn default_routes() {
        let routing = RoutingConfig::default();
        let cases = [
            (app(Some("WEBRTC VoiceEngine"), Some("Discord")), Channel::Chat),
            (app(Some("TeamSpeak 3"), Some("ts3client_linux_amd64")), Channel::Chat),
            (app(Some("Mumble"), Some("mumble")), Channel::Chat),
            (app(Some("ZOOM VoiceEngine"), Some("zoom")), Channel::Chat),
            (app(Some("teams-for-linux"), Some("teams-for-linux")), Channel::Chat),
            (app(Some("Skype"), Some("skypeforlinux")), Channel::Chat),
            (app(Some("spotify"), Some("spotify")), Channel::Media),
            (app(Some("Firefox"), Some("firefox")), Channel::Media),
            (app(Some("Google Chrome"), Some("chrome")), Channel::Media),
            (app(Some("Chromium"), Some("chromium")), Channel::Media),
            (
                app(Some("VLC media player (LibVLC 3.0.20)"), Some("vlc")),
                Channel::Media,
            ),
            (app(Some("mpv Media Player"), Some("mpv")), Channel::Media),
            (app(Some("Counter-Strike 2"), Some("cs2")), Channel::Game),
            (app(Some("steam"), Some("steam")), Channel::Game),
            (app(None, None), Channel::Game),
        ];
        for (identity, expected) in cases {
            assert_eq!(routing.channel_for(&identity), expected, "{identity:?}");
        }
    }

    #[test]
    fn field_specific_rules() {
        let rule = RoutingRule::new(MatchField::ProcessBinary, "game", Channel::Aux);
        assert!(rule.matches(&app(Some("x"), Some("GAME"))));
        assert!(!rule.matches(&app(Some("game"), Some("x"))));
        let rule = RoutingRule::new(MatchField::ApplicationName, "game", Channel::Aux);
        assert!(rule.matches(&app(Some("Game"), None)));
        assert!(!rule.matches(&app(None, Some("game"))));
    }

    #[test]
    fn upsert_puts_user_rule_first_and_replaces_duplicates() {
        let mut routing = RoutingConfig::default();
        let firefox = app(Some("Firefox"), Some("firefox"));
        assert_eq!(routing.channel_for(&firefox), Channel::Media);

        routing.upsert(RoutingRule::new(MatchField::Any, "Firefox", Channel::Game));
        assert_eq!(routing.channel_for(&firefox), Channel::Game);
        let before = routing.rules.len();
        routing.upsert(RoutingRule::new(MatchField::Any, "firefox", Channel::Aux));
        assert_eq!(routing.rules.len(), before, "same selector replaced, not added");
        assert_eq!(routing.channel_for(&firefox), Channel::Aux);
    }

    #[test]
    fn validation() {
        RoutingConfig::default().validate().unwrap();
        assert!(
            RoutingRule::new(MatchField::Any, " ", Channel::Game)
                .validate()
                .is_err()
        );
        assert!(RoutingRule::new(MatchField::Any, "x", Channel::Mic).validate().is_err());
        let routing = RoutingConfig {
            default_channel: Channel::Mic,
            rules: Vec::new(),
        };
        assert!(routing.validate().is_err());
    }
}
