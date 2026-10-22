//! Shared domain for gamefocus: profiles, sessions, focus/boost policies.

mod cgroup;
mod error;
mod paths;
mod profile;
mod resources;
mod session;
mod vram;

pub use error::{Error, Result};
pub use paths::{
    default_snapshot_path, discover_profiles, embedded_default_toml, load_session_profile,
    repo_root_guess, resolve_profile_path, user_default_profile_path, ProfileSource,
};
pub use profile::{
    BoostPolicy, FocusAction, FocusPolicy, Matcher, Profile, WhitelistEntry,
};
pub use session::{
    end_session, park_targets, start_session, BoostContext, SessionMode, SessionSnapshot,
    SuspendedProcess,
};
pub use cgroup::{
    format_cgroup_label, parse_cgroup_target, resolve_pid_supervisor_label, CgroupTarget,
};
pub use resources::{format_session_impact, mem_available_bytes};
pub use vram::{
    format_vram_warning, parse_nvidia_smi_compute_apps, probe_nvidia_compute_apps,
    vram_warn_targets, warn_heavy_vram_users, VramConsumer, VRAM_WARN_MIB,
};

/// Platform capability used by the CLI to pick a backend.
pub trait ProcessController: Send + Sync {
    /// List candidate user processes (pid, executable path or name, cmdline).
    fn list_processes(&self) -> Result<Vec<ProcessInfo>>;

    /// Suspend a process (SIGSTOP / NtSuspendProcess / etc.).
    fn suspend(&self, pid: u32) -> Result<()>;

    /// Resume a previously suspended process.
    fn resume(&self, pid: u32) -> Result<()>;

    /// Soft-stop (SIGTERM / graceful close) when suspend is unsupported.
    fn soft_stop(&self, pid: u32) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub exe_path: Option<String>,
    pub cmdline: Option<String>,
}

/// Optional OS boost adapters (GameMode, power plan, working set, …). Soft-fail per provider.
pub trait BoostProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn apply(&self, ctx: &BoostContext<'_>) -> Result<BoostHandle>;
    fn rollback(&self, handle: BoostHandle) -> Result<()>;
}

/// Opaque per-provider state needed to undo a boost.
#[derive(Debug, Clone)]
pub struct BoostHandle {
    pub provider_id: String,
    pub payload: String,
}
