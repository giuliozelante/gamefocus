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
    /// Soft-stopped VRAM supervisors to restore on end.
    #[serde(default)]
    pub reclaimed: Vec<crate::reclaim::ReclaimedEntry>,
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

/// `process_name` rules: exact basename (case-insensitive), or simple `*` globs.
pub(crate) fn process_name_matches(process_name: &str, pattern: &str) -> bool {
    let name = process_name.to_ascii_lowercase();
    let pat = pattern.to_ascii_lowercase();
    if pat.contains('*') {
        simple_glob_matches(&name, &pat)
    } else {
        name == pat
    }
}

fn simple_glob_matches(name: &str, pattern: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.is_empty() {
        return name.is_empty();
    }
    let mut rest = name;
    if !parts[0].is_empty() {
        if !rest.starts_with(parts[0]) {
            return false;
        }
        rest = &rest[parts[0].len()..];
    }
    for (i, part) in parts.iter().enumerate().skip(1) {
        if part.is_empty() {
            if i + 1 == parts.len() {
                return true;
            }
            continue;
        }
        if i + 1 == parts.len() {
            return rest.ends_with(part);
        }
        match rest.find(part) {
            Some(idx) => rest = &rest[idx + part.len()..],
            None => return false,
        }
    }
    true
}

fn matcher_hits(info: &ProcessInfo, m: &crate::profile::Matcher) -> bool {
    use crate::profile::Matcher::*;
    match m {
        ProcessName(s) => process_name_matches(&info.name, s),
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
        ProcessName(s) => process_name_matches(&info.name, s),
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

/// Context passed to Boost after Focus has run.
#[derive(Debug, Clone, Copy)]
pub struct BoostContext<'a> {
    pub suspended: &'a [SuspendedProcess],
}

/// Processes Focus would park for this profile (deny-list, whitelist, shell protect).
pub fn park_targets<'a>(
    profile: &Profile,
    processes: &'a [ProcessInfo],
    self_pid: u32,
) -> Vec<&'a ProcessInfo> {
    processes
        .iter()
        .filter(|info| {
            if info.pid == self_pid || info.pid == 1 {
                return false;
            }
            if profile.focus.protect_shell && is_protected_shell(info) {
                return false;
            }
            if matches_whitelist(info, profile) || matches_any_matcher(info, &profile.matchers) {
                return false;
            }
            matches_any_matcher(info, &profile.park)
        })
        .collect()
}

/// Apply Focus and/or Boost according to profile + mode overrides.
/// Focus always runs before Boost so providers can see parked pids.
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

    if mode.focus_enabled(profile) {
        if profile.park.is_empty() {
            warn!("focus enabled but profile.park is empty; nothing to suspend");
        }
        let processes = controller.list_processes()?;
        let self_pid = std::process::id();
        for info in park_targets(profile, &processes, self_pid) {
            let result = match profile.focus.action {
                FocusAction::Suspend => controller.suspend(info.pid).map(|_| "suspend"),
                FocusAction::SoftStop => controller.soft_stop(info.pid).map(|_| "soft_stop"),
            };
            match result {
                Ok(action) => snap.suspended.push(SuspendedProcess {
                    pid: info.pid,
                    name: info.name.clone(),
                    action: action.into(),
                }),
                Err(err) => warn!(
                    pid = info.pid,
                    name = %info.name,
                    %err,
                    "focus park soft-fail"
                ),
            }
        }
        snap.focus_applied = true;
        info!(count = snap.suspended.len(), "focus applied");
    }

    if mode.boost_enabled(profile) {
        let ctx = BoostContext {
            suspended: &snap.suspended,
        };
        for provider in boosters {
            if !profile.boost.providers.iter().any(|p| p == provider.id()) {
                continue;
            }
            match provider.apply(&ctx) {
                Ok(handle) => {
                    info!(provider = provider.id(), "boost applied");
                    snap.boost_handles.push(handle.into());
                    snap.boost_applied = true;
                }
                Err(err) => warn!(provider = provider.id(), %err, "boost soft-fail"),
            }
        }
    }

    Ok(snap)
}

/// Roll back boosts, then resume suspended processes.
pub fn end_session(
    snapshot: &SessionSnapshot,
    controller: &dyn ProcessController,
    boosters: &[&dyn BoostProvider],
) -> Result<()> {
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

    for proc in snapshot.suspended.iter().rev() {
        if proc.action == "suspend" {
            if let Err(err) = controller.resume(proc.pid) {
                if resume_failure_is_benign(&err) {
                    tracing::debug!(pid = proc.pid, %err, "resume skipped; process already gone");
                } else {
                    warn!(pid = proc.pid, %err, "resume failed");
                }
            }
        }
    }
    Ok(())
}

/// Gone PIDs (exited while parked) are expected after a long session — not a real failure.
pub(crate) fn resume_failure_is_benign(err: &Error) -> bool {
    match err {
        Error::Process { message, .. } => {
            let m = message.to_ascii_lowercase();
            m.contains("esrch") || m.contains("no such process")
        }
        _ => false,
    }
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
        fn apply(&self, _ctx: &BoostContext<'_>) -> Result<BoostHandle> {
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
            gpu: Default::default(),
        }
    }

    #[test]
    fn focus_runs_before_boost() {
        let order = Mutex::new(Vec::new());
        let profile = Profile {
            name: "order".into(),
            description: String::new(),
            matchers: Vec::new(),
            park: vec![crate::profile::Matcher::ProcessName("chrome".into())],
            whitelist: Vec::new(),
            focus: crate::profile::FocusPolicy::default(),
            boost: crate::profile::BoostPolicy {
                enabled: true,
                providers: vec!["power_plan".into()],
            },
            gpu: Default::default(),
        };
        struct OrderController<'a> {
            order: &'a Mutex<Vec<&'static str>>,
        }
        impl ProcessController for OrderController<'_> {
            fn list_processes(&self) -> Result<Vec<ProcessInfo>> {
                Ok(vec![ProcessInfo {
                    pid: 42,
                    name: "chrome".into(),
                    exe_path: None,
                    cmdline: None,
                }])
            }
            fn suspend(&self, _pid: u32) -> Result<()> {
                self.order.lock().expect("order").push("focus");
                Ok(())
            }
            fn resume(&self, _pid: u32) -> Result<()> {
                Ok(())
            }
            fn soft_stop(&self, _pid: u32) -> Result<()> {
                Ok(())
            }
        }
        struct OrderBoost<'a> {
            order: &'a Mutex<Vec<&'static str>>,
        }
        impl BoostProvider for OrderBoost<'_> {
            fn id(&self) -> &'static str {
                "power_plan"
            }
            fn apply(&self, _ctx: &BoostContext<'_>) -> Result<BoostHandle> {
                self.order.lock().expect("order").push("boost");
                Ok(BoostHandle {
                    provider_id: "power_plan".into(),
                    payload: String::new(),
                })
            }
            fn rollback(&self, _handle: BoostHandle) -> Result<()> {
                Ok(())
            }
        }
        let controller = OrderController { order: &order };
        let boost = OrderBoost { order: &order };
        start_session(&profile, SessionMode::default(), &controller, &[&boost]).expect("start");
        assert_eq!(*order.lock().expect("order"), vec!["focus", "boost"]);
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

    #[test]
    fn end_rolls_boost_back_before_resume() {
        let order = Mutex::new(Vec::new());
        struct OrderEndController<'a> {
            order: &'a Mutex<Vec<&'static str>>,
        }
        impl ProcessController for OrderEndController<'_> {
            fn list_processes(&self) -> Result<Vec<ProcessInfo>> {
                Ok(Vec::new())
            }
            fn suspend(&self, _pid: u32) -> Result<()> {
                Ok(())
            }
            fn resume(&self, _pid: u32) -> Result<()> {
                self.order.lock().expect("order").push("resume");
                Ok(())
            }
            fn soft_stop(&self, _pid: u32) -> Result<()> {
                Ok(())
            }
        }
        struct OrderEndBoost<'a> {
            order: &'a Mutex<Vec<&'static str>>,
        }
        impl BoostProvider for OrderEndBoost<'_> {
            fn id(&self) -> &'static str {
                "power_plan"
            }
            fn apply(&self, _ctx: &BoostContext<'_>) -> Result<BoostHandle> {
                Ok(BoostHandle {
                    provider_id: "power_plan".into(),
                    payload: String::new(),
                })
            }
            fn rollback(&self, _handle: BoostHandle) -> Result<()> {
                self.order.lock().expect("order").push("rollback");
                Ok(())
            }
        }
        let snap = SessionSnapshot {
            profile_name: "end-order".into(),
            focus_applied: true,
            boost_applied: true,
            suspended: vec![SuspendedProcess {
                pid: 7,
                name: "chrome".into(),
                action: "suspend".into(),
            }],
            boost_handles: vec![BoostHandleDto {
                provider_id: "power_plan".into(),
                payload: String::new(),
            }],
            reclaimed: Vec::new(),
        };
        let controller = OrderEndController { order: &order };
        let boost = OrderEndBoost { order: &order };
        end_session(&snap, &controller, &[&boost]).expect("end");
        assert_eq!(*order.lock().expect("order"), vec!["rollback", "resume"]);
    }

    #[test]
    fn park_eperm_soft_fails_and_continues() {
        struct MixedController;
        impl ProcessController for MixedController {
            fn list_processes(&self) -> Result<Vec<ProcessInfo>> {
                Ok(vec![
                    ProcessInfo {
                        pid: 10,
                        name: "chrome".into(),
                        exe_path: None,
                        cmdline: None,
                    },
                    ProcessInfo {
                        pid: 20,
                        name: "llama-server".into(),
                        exe_path: None,
                        cmdline: None,
                    },
                ])
            }
            fn suspend(&self, pid: u32) -> Result<()> {
                if pid == 20 {
                    return Err(Error::Process {
                        pid,
                        message: "EPERM: Operation not permitted".into(),
                    });
                }
                Ok(())
            }
            fn resume(&self, _pid: u32) -> Result<()> {
                Ok(())
            }
            fn soft_stop(&self, _pid: u32) -> Result<()> {
                Ok(())
            }
        }
        let profile = Profile {
            name: "soft".into(),
            description: String::new(),
            matchers: Vec::new(),
            park: vec![
                crate::profile::Matcher::ProcessName("chrome".into()),
                crate::profile::Matcher::ProcessName("llama-server".into()),
            ],
            whitelist: Vec::new(),
            focus: crate::profile::FocusPolicy::default(),
            boost: crate::profile::BoostPolicy {
                enabled: false,
                providers: Vec::new(),
            },
            gpu: Default::default(),
        };
        let snap = start_session(&profile, SessionMode::default(), &MixedController, &[])
            .expect("start must soft-fail EPERM");
        assert_eq!(snap.suspended.len(), 1);
        assert_eq!(snap.suspended[0].pid, 10);
    }

    #[test]
    fn park_targets_lists_deny_list_hits() {
        let profile = Profile {
            name: "park".into(),
            description: String::new(),
            matchers: Vec::new(),
            park: vec![crate::profile::Matcher::ProcessName("chrome".into())],
            whitelist: Vec::new(),
            focus: crate::profile::FocusPolicy::default(),
            boost: crate::profile::BoostPolicy::default(),
            gpu: Default::default(),
        };
        let processes = vec![
            ProcessInfo {
                pid: 1,
                name: "init".into(),
                exe_path: None,
                cmdline: None,
            },
            ProcessInfo {
                pid: 42,
                name: "chrome".into(),
                exe_path: None,
                cmdline: None,
            },
            ProcessInfo {
                pid: 99,
                name: "safe".into(),
                exe_path: None,
                cmdline: None,
            },
        ];
        let hits = park_targets(&profile, &processes, 7);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].pid, 42);
    }

    #[test]
    fn process_name_park_is_exact_not_substring() {
        let profile = Profile {
            name: "exact".into(),
            description: String::new(),
            matchers: Vec::new(),
            park: vec![crate::profile::Matcher::ProcessName("chrome".into())],
            whitelist: Vec::new(),
            focus: crate::profile::FocusPolicy::default(),
            boost: crate::profile::BoostPolicy::default(),
            gpu: Default::default(),
        };
        let processes = vec![
            ProcessInfo {
                pid: 10,
                name: "chrome".into(),
                exe_path: None,
                cmdline: None,
            },
            ProcessInfo {
                pid: 11,
                name: "chrome_crashpad".into(),
                exe_path: None,
                cmdline: None,
            },
            ProcessInfo {
                pid: 12,
                name: "Chrome".into(),
                exe_path: None,
                cmdline: None,
            },
        ];
        let hits = park_targets(&profile, &processes, 1);
        let pids: Vec<u32> = hits.iter().map(|p| p.pid).collect();
        assert_eq!(pids, vec![10, 12], "chrome_crashpad must not match chrome");
    }

    #[test]
    fn process_name_glob_star_matches_prefix() {
        let profile = Profile {
            name: "glob".into(),
            description: String::new(),
            matchers: Vec::new(),
            park: vec![crate::profile::Matcher::ProcessName("code-*".into())],
            whitelist: Vec::new(),
            focus: crate::profile::FocusPolicy::default(),
            boost: crate::profile::BoostPolicy::default(),
            gpu: Default::default(),
        };
        let processes = vec![
            ProcessInfo {
                pid: 1,
                name: "code".into(),
                exe_path: None,
                cmdline: None,
            },
            ProcessInfo {
                pid: 2,
                name: "code-helper".into(),
                exe_path: None,
                cmdline: None,
            },
            ProcessInfo {
                pid: 3,
                name: "encode-helper".into(),
                exe_path: None,
                cmdline: None,
            },
        ];
        let hits = park_targets(&profile, &processes, 99);
        let pids: Vec<u32> = hits.iter().map(|p| p.pid).collect();
        assert_eq!(pids, vec![2]);
    }

    #[test]
    fn soft_stop_entries_are_not_resumed() {
        let resumes = Mutex::new(Vec::new());
        struct ResumeLog<'a> {
            resumes: &'a Mutex<Vec<u32>>,
        }
        impl ProcessController for ResumeLog<'_> {
            fn list_processes(&self) -> Result<Vec<ProcessInfo>> {
                Ok(Vec::new())
            }
            fn suspend(&self, _pid: u32) -> Result<()> {
                Ok(())
            }
            fn resume(&self, pid: u32) -> Result<()> {
                self.resumes.lock().expect("resumes").push(pid);
                Ok(())
            }
            fn soft_stop(&self, _pid: u32) -> Result<()> {
                Ok(())
            }
        }
        let snap = SessionSnapshot {
            profile_name: "soft".into(),
            focus_applied: true,
            boost_applied: false,
            suspended: vec![SuspendedProcess {
                pid: 9,
                name: "chrome".into(),
                action: "soft_stop".into(),
            }],
            boost_handles: Vec::new(),
            reclaimed: Vec::new(),
        };
        let controller = ResumeLog { resumes: &resumes };
        end_session(&snap, &controller, &[]).expect("end");
        assert!(resumes.lock().expect("resumes").is_empty());
    }

    #[test]
    fn esrch_resume_errors_are_benign() {
        let err = Error::Process {
            pid: 1,
            message: "process 1: ESRCH: No such process".into(),
        };
        assert!(resume_failure_is_benign(&err));
        let other = Error::Process {
            pid: 1,
            message: "EPERM: Operation not permitted".into(),
        };
        assert!(!resume_failure_is_benign(&other));
    }
}
