//! Profile discovery helpers shared by CLI and UI.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Session snapshot next to the user's runtime data dir when possible.
pub fn default_snapshot_path() -> PathBuf {
    resolve_snapshot_path(|key| env::var_os(key))
}

fn resolve_snapshot_path(
    mut get: impl FnMut(&str) -> Option<std::ffi::OsString>,
) -> PathBuf {
    if let Some(dir) = get("XDG_RUNTIME_DIR") {
        return PathBuf::from(dir).join("gamefocus-session.json");
    }
    if let Some(local) = get("LOCALAPPDATA") {
        return PathBuf::from(local).join("gamefocus").join("session.json");
    }
    if let Some(home) = get("HOME") {
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
    let mut seen = std::collections::HashSet::new();
    for dir in profile_search_dirs() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            let key = path.canonicalize().unwrap_or_else(|_| path.clone());
            if seen.insert(key) {
                out.push(path);
            }
        }
    }
    out.sort();
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
    fn snapshot_path_prefers_localappdata_when_xdg_unset() {
        let local = PathBuf::from("C:").join("Users").join("me").join("AppData").join("Local");
        let path = resolve_snapshot_path(|key| match key {
            "XDG_RUNTIME_DIR" => None,
            "LOCALAPPDATA" => Some(local.as_os_str().to_owned()),
            _ => None,
        });
        assert_eq!(path, local.join("gamefocus").join("session.json"));
    }

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
