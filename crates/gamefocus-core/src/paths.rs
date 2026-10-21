//! Profile discovery helpers shared by CLI and UI.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Session snapshot next to the user's runtime data dir when possible.
pub fn default_snapshot_path() -> PathBuf {
    if let Ok(dir) = env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(dir).join("gamefocus-session.json");
    }
    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".local/state/gamefocus")
            .join("session.json");
    }
    PathBuf::from("gamefocus-session.json")
}

/// Resolve a profile path; must exist and be a file.
pub fn resolve_profile_path(path: impl AsRef<Path>) -> Result<PathBuf, String> {
    let path = path.as_ref();
    let candidates = [
        path.to_path_buf(),
        PathBuf::from("profiles").join(path),
        repo_root_guess().join(path),
        repo_root_guess().join("profiles").join(path),
    ];
    for c in candidates {
        if c.is_file() {
            return Ok(c);
        }
    }
    Err(format!("profile not found: {}", path.display()))
}

pub fn discover_profiles() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for dir in profile_search_dirs() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                out.push(path);
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn profile_search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    dirs.push(repo_root_guess().join("profiles/bundled"));
    dirs.push(PathBuf::from("profiles/bundled"));
    if let Ok(extra) = env::var("GAMEFOCUS_PROFILES") {
        dirs.push(PathBuf::from(extra));
    }
    if let Some(home) = env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(".config/gamefocus/profiles"));
    }
    dirs
}

/// Guess repo root from this crate's manifest (`crates/gamefocus-core` → `../..`).
pub fn repo_root_guess() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .nth(2)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| manifest.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_windows_profile_uses_power_plan() {
        let path = repo_root_guess().join("profiles/bundled/windows-gaming.toml");
        let profile = crate::Profile::load_file(&path).expect("windows profile");
        assert_eq!(profile.name, "windows-gaming");
        assert!(!profile.park.is_empty());
        assert!(profile.focus.protect_shell);
        assert_eq!(
            profile.boost.providers,
            vec!["power_plan".to_string(), "working_set".to_string()]
        );
    }

    #[test]
    fn bundled_macos_profile_has_no_boost_provider() {
        let path = repo_root_guess().join("profiles/bundled/macos-gaming.toml");
        let profile = crate::Profile::load_file(&path).expect("macos profile");
        assert_eq!(profile.name, "macos-gaming");
        assert!(!profile.park.is_empty());
        assert!(profile.focus.protect_shell);
        assert!(profile.boost.providers.is_empty());
    }

    #[test]
    fn discover_profiles_finds_bundled() {
        let found = discover_profiles();
        assert!(
            found
                .iter()
                .any(|p| { p.file_name().and_then(|n| n.to_str()) == Some("linux-gaming.toml") }),
            "expected bundled linux-gaming.toml, got {found:?}"
        );
        assert!(
            found
                .iter()
                .any(|p| { p.file_name().and_then(|n| n.to_str()) == Some("windows-gaming.toml") }),
            "expected bundled windows-gaming.toml, got {found:?}"
        );
        assert!(
            found
                .iter()
                .any(|p| { p.file_name().and_then(|n| n.to_str()) == Some("macos-gaming.toml") }),
            "expected bundled macos-gaming.toml, got {found:?}"
        );
    }

    #[test]
    fn resolve_profile_rejects_missing() {
        assert!(resolve_profile_path("/no/such/gamefocus-profile.toml").is_err());
    }

    #[test]
    fn resolve_bundled_relative_name() {
        let p = resolve_profile_path("bundled/linux-gaming.toml")
            .or_else(|_| resolve_profile_path("profiles/bundled/linux-gaming.toml"));
        assert!(p.is_ok(), "{p:?}");
    }
}
