use crate::profile::{FocusAction, Profile};
use crate::{BoostHandle, BoostProvider, Error, ProcessController, ProcessInfo, Result};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

/// Runtime overrides for Focus / Boost independently of the profile defaults.
#[derive(Debug, Clone, Copy, Default)]
pub struct SessionMode {
    pub focus: Option<bool>,
    pub boost: Option<bool>,
}

impl SessionMode {
    pub fn focus_enabled(&self, profile: &Profile) -> bool {
        self.focus.unwrap_or(profile.focus.enabled)
    }

    pub fn boost_enabled(&self, profile: &Profile) -> bool {
        self.boost.unwrap_or(profile.boost.enabled)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuspendedProcess {
    pub pid: u32,
    pub name: String,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionSnapshot {
    pub profile_name: String,
    pub focus_applied: bool,
    pub boost_applied: bool,
    pub suspended: Vec<SuspendedProcess>,
    pub boost_handles: Vec<BoostHandleDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoostHandleDto {
    pub provider_id: String,
    pub payload: String,
}

impl From<BoostHandle> for BoostHandleDto {
    fn from(h: BoostHandle) -> Self {
        Self {
            provider_id: h.provider_id,
            payload: h.payload,
        }
    }
}

impl From<BoostHandleDto> for BoostHandle {
    fn from(h: BoostHandleDto) -> Self {
        BoostHandle {
            provider_id: h.provider_id,
            payload: h.payload,
        }
    }
}

/// Hard-coded shell / compositor names that Focus must never touch when protect_shell is on.
fn is_protected_shell(info: &ProcessInfo) -> bool {
    let name = info.name.to_lowercase();
    let path = info.exe_path.as_deref().unwrap_or("").to_lowercase();
    const NAMES: &[&str] = &[
        "systemd",
        "init",
        "kwin_wayland",
        "kwin_x11",
        "plasmashell",
        "gnome-shell",
        "xwayland",
        "xorg",
        "wayland",
        "explorer.exe",
        "dwm.exe",
        "winlogon.exe",
        "csrss.exe",
        "windowserver",
        "dock",
        "finder",
        "gamefocus",
        "gamefocus-cli",
    ];
    NAMES.iter().any(|n| name.contains(n) || path.contains(n))
}

fn matcher_hits(info: &ProcessInfo, m: &crate::profile::Matcher) -> bool {
    use crate::profile::Matcher::*;
    match m {
        ProcessName(s) => info.name.to_lowercase().contains(&s.to_lowercase()),
        ExePath(s) => info
            .exe_path
            .as_deref()
            .unwrap_or("")
            .to_lowercase()
            .contains(&s.to_lowercase()),
        FlatpakId(s) | Cmdline(s) => info
            .cmdline
            .as_deref()
            .unwrap_or("")
            .to_lowercase()
            .contains(&s.to_lowercase()),
    }
}

fn matches_any_matcher(info: &ProcessInfo, matchers: &[crate::profile::Matcher]) -> bool {
    matchers.iter().any(|m| matcher_hits(info, m))
}

fn matches_whitelist(info: &ProcessInfo, profile: &Profile) -> bool {
    use crate::profile::WhitelistEntry::*;
    profile.whitelist.iter().any(|entry| match entry {
        ProcessName(s) => info.name.to_lowercase().contains(&s.to_lowercase()),
        ExePath(s) => info
            .exe_path
            .as_deref()
            .unwrap_or("")
            .to_lowercase()
            .contains(&s.to_lowercase()),
        FlatpakId(s) | Cmdline(s) => info
            .cmdline
            .as_deref()
            .unwrap_or("")
            .to_lowercase()
            .contains(&s.to_lowercase()),
    })
}

/// Apply Focus and/or Boost according to profile + mode overrides.
pub fn start_session(
    profile: &Profile,
    mode: SessionMode,
    controller: &dyn ProcessController,
    boosters: &[&dyn BoostProvider],
) -> Result<SessionSnapshot> {
    let mut snap = SessionSnapshot {
        profile_name: profile.name.clone(),
        ..Default::default()
    };

    if mode.boost_enabled(profile) {
        for provider in boosters {
            if !profile.boost.providers.iter().any(|p| p == provider.id()) {
                continue;
            }
            match provider.apply() {
                Ok(handle) => {
                    info!(provider = provider.id(), "boost applied");
                    snap.boost_handles.push(handle.into());
                    snap.boost_applied = true;
                }
                Err(err) => warn!(provider = provider.id(), %err, "boost soft-fail"),
            }
        }
    }

    if mode.focus_enabled(profile) {
        if profile.park.is_empty() {
            warn!("focus enabled but profile.park is empty; nothing to suspend");
        }
        let processes = controller.list_processes()?;
        let self_pid = std::process::id();
        for info in processes {
            if info.pid == self_pid || info.pid == 1 {
                continue;
            }
            if profile.focus.protect_shell && is_protected_shell(&info) {
                continue;
            }
            if matches_whitelist(&info, profile) || matches_any_matcher(&info, &profile.matchers) {
                continue;
            }
            if !matches_any_matcher(&info, &profile.park) {
                continue;
            }

            let action = match profile.focus.action {
                FocusAction::Suspend => {
                    controller.suspend(info.pid)?;
                    "suspend"
                }
                FocusAction::SoftStop => {
                    controller.soft_stop(info.pid)?;
                    "soft_stop"
                }
            };
            snap.suspended.push(SuspendedProcess {
                pid: info.pid,
                name: info.name.clone(),
                action: action.into(),
            });
        }
        snap.focus_applied = true;
        info!(count = snap.suspended.len(), "focus applied");
    }

    Ok(snap)
}

/// Restore suspended processes and roll back boosts.
pub fn end_session(
    snapshot: &SessionSnapshot,
    controller: &dyn ProcessController,
    boosters: &[&dyn BoostProvider],
) -> Result<()> {
    for proc in snapshot.suspended.iter().rev() {
        if proc.action == "suspend" {
            if let Err(err) = controller.resume(proc.pid) {
                warn!(pid = proc.pid, %err, "resume failed");
            }
        }
    }

    for dto in snapshot.boost_handles.iter().rev() {
        let handle: BoostHandle = dto.clone().into();
        if let Some(provider) = boosters.iter().find(|p| p.id() == handle.provider_id) {
            if let Err(err) = provider.rollback(handle) {
                warn!(%err, "boost rollback failed");
            }
        } else {
            return Err(Error::Boost {
                provider: dto.provider_id.clone(),
                message: "provider not registered at end_session".into(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct Noop;

    impl ProcessController for Noop {
        fn list_processes(&self) -> Result<Vec<ProcessInfo>> {
            Ok(Vec::new())
        }
        fn suspend(&self, _pid: u32) -> Result<()> {
            Ok(())
        }
        fn resume(&self, _pid: u32) -> Result<()> {
            Ok(())
        }
        fn soft_stop(&self, _pid: u32) -> Result<()> {
            Ok(())
        }
    }

    struct CountingBoost {
        hits: Mutex<u32>,
    }

    impl BoostProvider for CountingBoost {
        fn id(&self) -> &'static str {
            "power_plan"
        }
        fn apply(&self) -> Result<BoostHandle> {
            *self.hits.lock().expect("hits") += 1;
            Ok(BoostHandle {
                provider_id: "power_plan".into(),
                payload: String::new(),
            })
        }
        fn rollback(&self, _handle: BoostHandle) -> Result<()> {
            Ok(())
        }
    }

    fn profile_with_providers(name: &str, providers: Vec<String>) -> Profile {
        Profile {
            name: name.into(),
            description: String::new(),
            matchers: Vec::new(),
            park: Vec::new(),
            whitelist: Vec::new(),
            focus: crate::profile::FocusPolicy {
                enabled: false,
                ..crate::profile::FocusPolicy::default()
            },
            boost: crate::profile::BoostPolicy {
                enabled: true,
                providers,
            },
        }
    }

    #[test]
    fn empty_provider_list_applies_no_booster() {
        let profile = profile_with_providers("empty-boost", Vec::new());
        let boost = CountingBoost {
            hits: Mutex::new(0),
        };
        start_session(&profile, SessionMode::default(), &Noop, &[&boost]).expect("start");
        assert_eq!(*boost.hits.lock().expect("hits"), 0);
    }

    #[test]
    fn listed_provider_is_applied() {
        let profile = profile_with_providers("listed-boost", vec!["power_plan".into()]);
        let boost = CountingBoost {
            hits: Mutex::new(0),
        };
        start_session(&profile, SessionMode::default(), &Noop, &[&boost]).expect("start");
        assert_eq!(*boost.hits.lock().expect("hits"), 1);
    }
}
