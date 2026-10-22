//! Soft-stop / restore lifecycle for resolvable VRAM supervisors.

use crate::cgroup::CgroupTarget;
use crate::profile::{VramReclaimConfig, VramReclaimPolicy};
use crate::session::process_name_matches;
use crate::vram::{vram_warn_targets, VramConsumer};
use crate::{Error, Profile, Result};
use serde::{Deserialize, Serialize};
use std::process::Command;
use tracing::{info, warn};

/// Effective action mode after combining profile policy, TTY, and CLI flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReclaimMode {
    WarnOnly,
    Prompt,
    Auto,
}

/// Actionable supervisor target that can be soft-stopped and restored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReclaimTarget {
    Docker { id: String, name: String },
    Podman { id: String, name: String },
    SystemdUser { unit: String },
}

impl ReclaimTarget {
    pub fn label(&self) -> String {
        match self {
            Self::Docker { name, .. } => format!("Docker: {name}"),
            Self::Podman { name, .. } => format!("Podman: {name}"),
            Self::SystemdUser { unit } => format!("systemd-user: {unit}"),
        }
    }

    pub fn match_name(&self) -> &str {
        match self {
            Self::Docker { name, .. } | Self::Podman { name, .. } => name,
            Self::SystemdUser { unit } => unit,
        }
    }

    pub fn from_cgroup(target: CgroupTarget, name: String) -> Self {
        match target {
            CgroupTarget::Docker { id } => Self::Docker { id, name },
            CgroupTarget::Podman { id } => Self::Podman { id, name },
            CgroupTarget::SystemdUser { unit } => Self::SystemdUser { unit },
        }
    }
}

/// One soft-stopped target recorded in the session snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReclaimedEntry {
    pub target: ReclaimTarget,
    pub was_running: bool,
}

/// Soft-stop / start operations for Docker, Podman, and systemd --user.
pub trait SupervisorOps: Send + Sync {
    fn is_running(&self, target: &ReclaimTarget) -> Result<bool>;
    fn soft_stop(&self, target: &ReclaimTarget) -> Result<()>;
    fn start(&self, target: &ReclaimTarget) -> Result<()>;
}

/// Real supervisor CLI (`docker` / `podman` / `systemctl --user`). Soft-fails via Result.
#[derive(Debug, Default)]
pub struct SystemSupervisor;

impl SupervisorOps for SystemSupervisor {
    fn is_running(&self, target: &ReclaimTarget) -> Result<bool> {
        match target {
            ReclaimTarget::Docker { id, .. } => container_running("docker", id),
            ReclaimTarget::Podman { id, .. } => container_running("podman", id),
            ReclaimTarget::SystemdUser { unit } => systemd_user_active(unit),
        }
    }

    fn soft_stop(&self, target: &ReclaimTarget) -> Result<()> {
        match target {
            ReclaimTarget::Docker { id, .. } => run_ok("docker", &["stop", "-t", "10", id]),
            ReclaimTarget::Podman { id, .. } => run_ok("podman", &["stop", "-t", "10", id]),
            ReclaimTarget::SystemdUser { unit } => {
                run_ok("systemctl", &["--user", "stop", unit])
            }
        }
    }

    fn start(&self, target: &ReclaimTarget) -> Result<()> {
        match target {
            ReclaimTarget::Docker { id, .. } => run_ok("docker", &["start", id]),
            ReclaimTarget::Podman { id, .. } => run_ok("podman", &["start", id]),
            ReclaimTarget::SystemdUser { unit } => {
                run_ok("systemctl", &["--user", "start", unit])
            }
        }
    }
}

fn container_running(engine: &str, id: &str) -> Result<bool> {
    let out = Command::new(engine)
        .args(["inspect", "--format", "{{.State.Running}}", id])
        .output()?;
    if !out.status.success() {
        return Err(Error::Boost {
            provider: "vram_reclaim".into(),
            message: format!("{engine} inspect failed for {id}"),
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim() == "true")
}

fn systemd_user_active(unit: &str) -> Result<bool> {
    let out = Command::new("systemctl")
        .args(["--user", "is-active", unit])
        .output()?;
    Ok(String::from_utf8_lossy(&out.stdout).trim() == "active")
}

fn run_ok(bin: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(bin).args(args).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::Boost {
            provider: "vram_reclaim".into(),
            message: format!("{bin} {} failed", args.join(" ")),
        })
    }
}

pub fn effective_reclaim_mode(
    policy: VramReclaimPolicy,
    force_reclaim: bool,
    is_tty: bool,
) -> ReclaimMode {
    if force_reclaim {
        return ReclaimMode::Auto;
    }
    match policy {
        VramReclaimPolicy::Auto => ReclaimMode::Auto,
        VramReclaimPolicy::Prompt if is_tty => ReclaimMode::Prompt,
        VramReclaimPolicy::Prompt | VramReclaimPolicy::Warn => ReclaimMode::WarnOnly,
    }
}

pub fn name_matches_reclaimable(name: &str, patterns: &[String]) -> bool {
    if patterns.is_empty() {
        return false;
    }
    patterns
        .iter()
        .any(|pat| process_name_matches(name, pat))
}

/// Map a heavy VRAM consumer to a reclaimable supervisor target when resolvable + patterned.
pub fn reclaim_candidate(
    consumer: &VramConsumer,
    config: &VramReclaimConfig,
    resolved: Option<ReclaimTarget>,
) -> Option<ReclaimTarget> {
    let target = resolved?;
    if consumer.used_mib < config.threshold_mib {
        return None;
    }
    if !name_matches_reclaimable(target.match_name(), &config.reclaimable_patterns) {
        return None;
    }
    Some(target)
}

/// Soft-stop approved targets. Soft-fails per target; never aborts the session.
pub fn soft_stop_targets(
    targets: &[ReclaimTarget],
    mode: ReclaimMode,
    mut ask: impl FnMut(&ReclaimTarget) -> bool,
    ops: &dyn SupervisorOps,
) -> Vec<ReclaimedEntry> {
    let mut out = Vec::new();
    for target in targets {
        let approve = match mode {
            ReclaimMode::WarnOnly => {
                warn!(
                    target = %target.label(),
                    "VRAM reclaim candidate; not stopping (warn-only)"
                );
                false
            }
            ReclaimMode::Auto => true,
            ReclaimMode::Prompt => ask(target),
        };
        if !approve {
            continue;
        }
        let was_running = match ops.is_running(target) {
            Ok(v) => v,
            Err(err) => {
                warn!(target = %target.label(), %err, "reclaim is_running soft-fail");
                continue;
            }
        };
        if !was_running {
            info!(target = %target.label(), "reclaim skip; already stopped");
            continue;
        }
        match ops.soft_stop(target) {
            Ok(()) => {
                info!(target = %target.label(), "reclaim soft-stopped");
                out.push(ReclaimedEntry {
                    target: target.clone(),
                    was_running: true,
                });
            }
            Err(err) => warn!(target = %target.label(), %err, "reclaim soft-stop soft-fail"),
        }
    }
    out
}

/// Restart targets that were running before soft-stop. Soft-fails per entry.
pub fn restore_reclaimed(entries: &[ReclaimedEntry], ops: &dyn SupervisorOps) {
    for entry in entries.iter().rev() {
        if !entry.was_running {
            continue;
        }
        match ops.start(&entry.target) {
            Ok(()) => info!(target = %entry.target.label(), "reclaim restored"),
            Err(err) => warn!(
                target = %entry.target.label(),
                %err,
                "reclaim restore failed; start manually if needed"
            ),
        }
    }
}

/// Probe + resolve + filter reclaimable VRAM targets for this profile.
pub fn collect_reclaim_candidates(profile: &Profile) -> Vec<ReclaimTarget> {
    let config = &profile.gpu.vram_reclaim;
    let consumers = crate::vram::probe_nvidia_compute_apps();
    let heavy = vram_warn_targets(&consumers, profile, config.threshold_mib);
    let mut out = Vec::new();
    for c in heavy {
        let resolved = resolve_pid_reclaim_target(c.pid);
        if let Some(t) = reclaim_candidate(c, config, resolved) {
            if !out.iter().any(|e: &ReclaimTarget| e == &t) {
                out.push(t);
            }
        }
    }
    out
}

/// Resolve `/proc/<pid>/cgroup` into a named reclaim target (Linux).
pub fn resolve_pid_reclaim_target(pid: u32) -> Option<ReclaimTarget> {
    #[cfg(target_os = "linux")]
    {
        let text = std::fs::read_to_string(format!("/proc/{pid}/cgroup")).ok()?;
        resolve_from_cgroup_text(&text)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        None
    }
}

fn resolve_from_cgroup_text(cgroup: &str) -> Option<ReclaimTarget> {
    let cg = crate::cgroup::parse_cgroup_target(cgroup)?;
    match cg {
        CgroupTarget::Docker { id } => {
            let name = crate::cgroup::container_display_name("docker", &id);
            Some(ReclaimTarget::Docker { id, name })
        }
        CgroupTarget::Podman { id } => {
            let name = crate::cgroup::container_display_name("podman", &id);
            Some(ReclaimTarget::Podman { id, name })
        }
        CgroupTarget::SystemdUser { unit } => Some(ReclaimTarget::SystemdUser { unit }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct LogOps {
        running: bool,
        stops: Mutex<Vec<String>>,
        starts: Mutex<Vec<String>>,
        fail_stop: bool,
    }

    impl SupervisorOps for LogOps {
        fn is_running(&self, _target: &ReclaimTarget) -> Result<bool> {
            Ok(self.running)
        }
        fn soft_stop(&self, target: &ReclaimTarget) -> Result<()> {
            if self.fail_stop {
                return Err(Error::Boost {
                    provider: "vram_reclaim".into(),
                    message: "boom".into(),
                });
            }
            self.stops.lock().unwrap().push(target.label());
            Ok(())
        }
        fn start(&self, target: &ReclaimTarget) -> Result<()> {
            self.starts.lock().unwrap().push(target.label());
            Ok(())
        }
    }

    #[test]
    fn llama_pattern_matches_container_name() {
        let patterns = vec!["*llama*".into(), "*ollama*".into()];
        assert!(name_matches_reclaimable(
            "life-copilot-llama-server",
            &patterns
        ));
        assert!(!name_matches_reclaimable("kwin_wayland", &patterns));
        assert!(!name_matches_reclaimable("anything", &[]));
    }

    #[test]
    fn force_flag_makes_auto_even_when_policy_warn() {
        assert_eq!(
            effective_reclaim_mode(VramReclaimPolicy::Warn, true, false),
            ReclaimMode::Auto
        );
        assert_eq!(
            effective_reclaim_mode(VramReclaimPolicy::Prompt, false, true),
            ReclaimMode::Prompt
        );
        assert_eq!(
            effective_reclaim_mode(VramReclaimPolicy::Prompt, false, false),
            ReclaimMode::WarnOnly
        );
    }

    #[test]
    fn auto_stops_running_target_and_restore_restarts() {
        let target = ReclaimTarget::Docker {
            id: "abc".into(),
            name: "life-copilot-llama-server".into(),
        };
        let ops = LogOps {
            running: true,
            stops: Mutex::new(Vec::new()),
            starts: Mutex::new(Vec::new()),
            fail_stop: false,
        };
        let stopped = soft_stop_targets(&[target.clone()], ReclaimMode::Auto, |_| false, &ops);
        assert_eq!(stopped.len(), 1);
        assert!(stopped[0].was_running);
        assert_eq!(ops.stops.lock().unwrap().len(), 1);
        restore_reclaimed(&stopped, &ops);
        assert_eq!(
            ops.starts.lock().unwrap().as_slice(),
            &["Docker: life-copilot-llama-server".to_string()]
        );
    }

    #[test]
    fn warn_only_does_not_stop() {
        let target = ReclaimTarget::SystemdUser {
            unit: "ollama.service".into(),
        };
        let ops = LogOps {
            running: true,
            stops: Mutex::new(Vec::new()),
            starts: Mutex::new(Vec::new()),
            fail_stop: false,
        };
        let stopped = soft_stop_targets(&[target], ReclaimMode::WarnOnly, |_| true, &ops);
        assert!(stopped.is_empty());
        assert!(ops.stops.lock().unwrap().is_empty());
    }

    #[test]
    fn prompt_uses_ask_callback() {
        let target = ReclaimTarget::Podman {
            id: "x".into(),
            name: "ollama".into(),
        };
        let ops = LogOps {
            running: true,
            stops: Mutex::new(Vec::new()),
            starts: Mutex::new(Vec::new()),
            fail_stop: false,
        };
        let stopped = soft_stop_targets(&[target], ReclaimMode::Prompt, |_| true, &ops);
        assert_eq!(stopped.len(), 1);
    }

    #[test]
    fn reclaim_candidate_requires_pattern_and_threshold() {
        let config = VramReclaimConfig {
            policy: VramReclaimPolicy::Auto,
            threshold_mib: 500,
            reclaimable_patterns: vec!["*llama*".into()],
        };
        let consumer = VramConsumer {
            pid: 1,
            process_name: "/app/llama-server".into(),
            used_mib: 6526,
        };
        let resolved = ReclaimTarget::Docker {
            id: "d".into(),
            name: "life-copilot-llama-server".into(),
        };
        assert!(reclaim_candidate(&consumer, &config, Some(resolved.clone())).is_some());
        let low = VramConsumer {
            used_mib: 10,
            ..consumer.clone()
        };
        assert!(reclaim_candidate(&low, &config, Some(resolved.clone())).is_none());
        let other = ReclaimTarget::Docker {
            id: "d".into(),
            name: "redis".into(),
        };
        assert!(reclaim_candidate(&consumer, &config, Some(other)).is_none());
    }

    #[test]
    fn soft_stop_failure_is_soft() {
        let target = ReclaimTarget::Docker {
            id: "x".into(),
            name: "llama".into(),
        };
        let ops = LogOps {
            running: true,
            stops: Mutex::new(Vec::new()),
            starts: Mutex::new(Vec::new()),
            fail_stop: true,
        };
        let stopped = soft_stop_targets(&[target], ReclaimMode::Auto, |_| true, &ops);
        assert!(stopped.is_empty());
    }

    #[test]
    fn resolve_from_docker_cgroup() {
        let cgroup =
            "0::/system.slice/docker-d54564359db3dddb713a289251c5c6679ccbd74bcf0e6ba338fd83340b95b200.scope\n";
        let t = resolve_from_cgroup_text(cgroup).expect("target");
        match t {
            ReclaimTarget::Docker { id, name } => {
                assert!(id.starts_with("d54564359db3"));
                assert!(!name.is_empty());
            }
            other => panic!("expected docker, got {other:?}"),
        }
    }
}
