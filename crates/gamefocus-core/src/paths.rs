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

/// Where a session profile came from (for logs and UI).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileSource {
    /// Explicit `--profile` or tray selection.
    Path(PathBuf),
    /// `default.toml` under the user config dir.
    UserDefault(PathBuf),
    /// Compile-time embedded OS preset.
    Embedded(&'static str),
}

impl std::fmt::Display for ProfileSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Path(p) | Self::UserDefault(p) => write!(f, "{}", p.display()),
            Self::Embedded(os) => write!(f, "embedded:{os}"),
        }
    }
}

/// User-editable default: `$XDG_CONFIG_HOME/gamefocus/default.toml`, else
/// `~/.config/…`, else `%APPDATA%\gamefocus\default.toml` on Windows.
pub fn user_default_profile_path() -> PathBuf {
    user_default_profile_path_from_env(|key| env::var_os(key))
}

fn user_default_profile_path_from_env(
    mut get: impl FnMut(&str) -> Option<std::ffi::OsString>,
) -> PathBuf {
    if let Some(xdg) = get("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("gamefocus").join("default.toml");
    }
    if let Some(appdata) = get("APPDATA") {
        return PathBuf::from(appdata).join("gamefocus").join("default.toml");
    }
    if let Some(home) = get("HOME") {
        return PathBuf::from(home)
            .join(".config")
            .join("gamefocus")
            .join("default.toml");
    }
    PathBuf::from("gamefocus-default.toml")
}

/// Embedded TOML for the compile-time target OS.
pub fn embedded_default_toml() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        include_str!("../../../profiles/embedded/linux.toml")
    }
    #[cfg(target_os = "windows")]
    {
        include_str!("../../../profiles/embedded/windows.toml")
    }
    #[cfg(target_os = "macos")]
    {
        include_str!("../../../profiles/embedded/macos.toml")
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        include_str!("../../../profiles/embedded/linux.toml")
    }
}

fn embedded_os_label() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "linux"
    }
    #[cfg(target_os = "windows")]
    {
        "windows"
    }
    #[cfg(target_os = "macos")]
    {
        "macos"
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        "linux"
    }
}

/// Load a session profile.
///
/// Order: explicit path → user `default.toml` if present → embedded OS default.
pub fn load_session_profile(
    explicit: Option<&Path>,
) -> Result<(crate::Profile, ProfileSource), String> {
    load_session_profile_with(explicit, &user_default_profile_path())
}

fn load_session_profile_with(
    explicit: Option<&Path>,
    user_default: &Path,
) -> Result<(crate::Profile, ProfileSource), String> {
    if let Some(path) = explicit {
        let resolved = resolve_profile_path(path)?;
        let profile = crate::Profile::load_file(&resolved).map_err(|e| e.to_string())?;
        return Ok((profile, ProfileSource::Path(resolved)));
    }
    if user_default.is_file() {
        let profile = crate::Profile::load_file(user_default).map_err(|e| e.to_string())?;
        return Ok((
            profile,
            ProfileSource::UserDefault(user_default.to_path_buf()),
        ));
    }
    let profile = crate::Profile::parse_toml(embedded_default_toml()).map_err(|e| e.to_string())?;
    Ok((profile, ProfileSource::Embedded(embedded_os_label())))
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
            vec![
                "power_plan".to_string(),
                "working_set".to_string(),
                "cpu_affinity".to_string(),
            ]
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

    #[test]
    fn user_default_prefers_xdg_config_home() {
        let path = user_default_profile_path_from_env(|key| match key {
            "XDG_CONFIG_HOME" => Some(std::ffi::OsString::from("/tmp/xdg-config")),
            _ => None,
        });
        assert_eq!(
            path,
            PathBuf::from("/tmp/xdg-config/gamefocus/default.toml")
        );
    }

    #[test]
    fn user_default_uses_appdata_when_xdg_unset() {
        let path = user_default_profile_path_from_env(|key| match key {
            "XDG_CONFIG_HOME" => None,
            "APPDATA" => Some(std::ffi::OsString::from(r"C:\Users\me\AppData\Roaming")),
            _ => None,
        });
        assert_eq!(
            path,
            PathBuf::from(r"C:\Users\me\AppData\Roaming")
                .join("gamefocus")
                .join("default.toml")
        );
    }

    #[test]
    fn embedded_default_parses_for_host_os() {
        let profile = crate::Profile::parse_toml(embedded_default_toml()).expect("embedded");
        assert!(!profile.name.is_empty());
        assert!(!profile.park.is_empty());
        assert!(profile.focus.protect_shell);
    }

    #[test]
    fn load_session_profile_uses_explicit_path() {
        let path = repo_root_guess().join("profiles/bundled/linux-gaming.toml");
        let (profile, source) = load_session_profile(Some(&path)).expect("load");
        assert_eq!(profile.name, "linux-gaming");
        assert!(matches!(source, ProfileSource::Path(_)));
    }

    #[test]
    fn load_session_profile_falls_back_to_embedded_without_user_default() {
        // Relies on the test environment not having a real user default.toml that
        // shadows embedded. CI runners are clean; local override would change this.
        let user = user_default_profile_path();
        if user.is_file() {
            return;
        }
        let (profile, source) = load_session_profile(None).expect("load");
        assert!(profile.name.starts_with("default-"));
        assert!(matches!(source, ProfileSource::Embedded(_)));
    }

    #[test]
    fn load_session_profile_prefers_existing_user_default_file() {
        let dir = std::env::temp_dir().join(format!(
            "gamefocus-profile-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("mkdir");
        let path = dir.join("default.toml");
        fs::write(
            &path,
            r#"
name = "user-default"
[[park]]
process_name = "chrome"
"#,
        )
        .expect("write");
        let (profile, source) = load_session_profile_with(None, &path).expect("load");
        assert_eq!(profile.name, "user-default");
        assert!(matches!(source, ProfileSource::UserDefault(_)));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_session_profile_embedded_when_user_default_missing() {
        let missing = std::env::temp_dir().join("gamefocus-no-such-default.toml");
        let _ = fs::remove_file(&missing);
        let (profile, source) = load_session_profile_with(None, &missing).expect("load");
        assert!(profile.name.starts_with("default-"));
        assert!(matches!(source, ProfileSource::Embedded(_)));
    }
}
