use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// How to detect the game / session target.
    #[serde(default)]
    pub matchers: Vec<Matcher>,
    /// Processes Focus may suspend/stop (deny-list). Empty = do nothing (safe default).
    #[serde(default)]
    pub park: Vec<Matcher>,
    /// Processes that must never be acted on, even if they match `park`.
    #[serde(default)]
    pub whitelist: Vec<WhitelistEntry>,
    #[serde(default)]
    pub focus: FocusPolicy,
    #[serde(default)]
    pub boost: BoostPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Matcher {
    /// Substring match against process name (case-insensitive).
    ProcessName(String),
    /// Substring match against executable path.
    ExePath(String),
    /// Flatpak application ID substring (Linux).
    FlatpakId(String),
    /// Substring match against full command line.
    Cmdline(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WhitelistEntry {
    ProcessName(String),
    ExePath(String),
    FlatpakId(String),
    Cmdline(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusPolicy {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub action: FocusAction,
    #[serde(default = "default_true")]
    pub protect_shell: bool,
}

impl Default for FocusPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            action: FocusAction::Suspend,
            protect_shell: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FocusAction {
    #[default]
    Suspend,
    SoftStop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoostPolicy {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Provider ids to run when boost is on (e.g. "gamemode", "power_plan").
    #[serde(default)]
    pub providers: Vec<String>,
}

impl Default for BoostPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            providers: vec!["gamemode".into()],
        }
    }
}

fn default_true() -> bool {
    true
}

impl Profile {
    pub fn load_file(path: impl AsRef<Path>) -> Result<Self> {
        let text = fs::read_to_string(path)?;
        let profile: Profile = toml::from_str(&text)?;
        if profile.name.trim().is_empty() {
            return Err(Error::Profile("profile.name must not be empty".into()));
        }
        Ok(profile)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_profile() {
        let toml = r#"
name = "test"
description = "unit"
"#;
        let p: Profile = toml::from_str(toml).unwrap();
        assert!(p.focus.enabled);
        assert!(p.boost.enabled);
    }
}
