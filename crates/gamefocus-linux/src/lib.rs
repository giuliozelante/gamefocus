//! Linux process control (SIGSTOP/SIGCONT) and optional GameMode boost.

mod pageout;

use gamefocus_core::{
    BoostHandle, BoostProvider, Error, ProcessController, ProcessInfo, Result,
};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
#[cfg(target_os = "linux")]
use tracing::debug;
use tracing::{info, warn};

pub struct LinuxProcessController;

impl LinuxProcessController {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LinuxProcessController {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessController for LinuxProcessController {
    fn list_processes(&self) -> Result<Vec<ProcessInfo>> {
        let mut out = Vec::new();
        for entry in fs::read_dir("/proc")? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let Ok(pid) = name.parse::<u32>() else {
                continue;
            };
            let base = entry.path();
            let comm = fs::read_to_string(base.join("comm"))
                .unwrap_or_default()
                .trim()
                .to_string();
            let cmdline = fs::read(base.join("cmdline")).ok().map(|bytes| {
                bytes
                    .split(|b| *b == 0)
                    .filter(|p| !p.is_empty())
                    .map(|p| String::from_utf8_lossy(p).into_owned())
                    .collect::<Vec<_>>()
                    .join(" ")
            });
            let exe_path = fs::read_link(base.join("exe"))
                .ok()
                .map(|p| p.to_string_lossy().into_owned());
            out.push(ProcessInfo {
                pid,
                name: comm,
                exe_path,
                cmdline,
            });
        }
        Ok(out)
    }

    fn suspend(&self, pid: u32) -> Result<()> {
        signal_pid(pid, SignalKind::Stop)?;
        // Soft-fail: reclaim anonymous RSS so SIGSTOP is not a no-op for RAM pressure.
        pageout::page_out_after_stop(pid);
        Ok(())
    }

    fn resume(&self, pid: u32) -> Result<()> {
        signal_pid(pid, SignalKind::Cont)
    }

    fn soft_stop(&self, pid: u32) -> Result<()> {
        signal_pid(pid, SignalKind::Term)
    }
}

enum SignalKind {
    Stop,
    Cont,
    Term,
}

fn signal_pid(pid: u32, kind: SignalKind) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid;
        let sig = match kind {
            SignalKind::Stop => Signal::SIGSTOP,
            SignalKind::Cont => Signal::SIGCONT,
            SignalKind::Term => Signal::SIGTERM,
        };
        kill(Pid::from_raw(pid as i32), sig).map_err(|e| Error::Process {
            pid,
            message: e.to_string(),
        })?;
        debug!(pid, ?sig, "signaled");
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (pid, kind);
        Err(Error::Other(
            "LinuxProcessController only works on Linux".into(),
        ))
    }
}

/// Soft-fail GameMode provider: registers this process with `gamemoded` over D-Bus.
pub struct GameModeBoost {
    active: bool,
}

/// `busctl --user call …` argv for GameMode RegisterGame / UnregisterGame.
fn gamemode_busctl_args(method: &str, pid: u32) -> Vec<String> {
    vec![
        "--user".into(),
        "call".into(),
        "com.feralinteractive.GameMode".into(),
        "/com/feralinteractive/GameMode".into(),
        "com.feralinteractive.GameMode".into(),
        method.into(),
        "i".into(),
        pid.to_string(),
    ]
}

impl GameModeBoost {
    pub fn new() -> Self {
        Self { active: false }
    }

    fn available() -> bool {
        which("gamemoded").is_some() || which("busctl").is_some()
    }
}

impl Default for GameModeBoost {
    fn default() -> Self {
        Self::new()
    }
}

impl BoostProvider for GameModeBoost {
    fn id(&self) -> &'static str {
        "gamemode"
    }

    fn apply(&self, _ctx: &gamefocus_core::BoostContext<'_>) -> Result<BoostHandle> {
        if !Self::available() {
            return Err(Error::Boost {
                provider: "gamemode".into(),
                message: "gamemoded not found; install Feral GameMode".into(),
            });
        }
        let pid = std::process::id();
        let status = Command::new("busctl")
            .args(gamemode_busctl_args("RegisterGame", pid))
            .output();
        match status {
            Ok(out) if out.status.success() => {
                info!("GameMode RegisterGame succeeded");
                Ok(BoostHandle {
                    provider_id: "gamemode".into(),
                    payload: pid.to_string(),
                })
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
                warn!(%stderr, "GameMode register failed");
                Err(Error::Boost {
                    provider: "gamemode".into(),
                    message: stderr,
                })
            }
            Err(err) => Err(Error::Boost {
                provider: "gamemode".into(),
                message: err.to_string(),
            }),
        }
    }

    fn rollback(&self, handle: BoostHandle) -> Result<()> {
        let _ = self.active;
        let Ok(pid) = handle.payload.parse::<u32>() else {
            return Ok(());
        };
        let _ = Command::new("busctl")
            .args(gamemode_busctl_args("UnregisterGame", pid))
            .output();
        Ok(())
    }
}

fn which(bin: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(bin);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::gamemode_busctl_args;

    #[test]
    fn register_game_uses_gamemode_interface_not_client() {
        let args = gamemode_busctl_args("RegisterGame", 42);
        assert_eq!(
            args[4], "com.feralinteractive.GameMode",
            "GameMode 1.x exposes RegisterGame on the main interface, not .Client"
        );
        assert!(
            !args.iter().any(|a| a.contains(".Client")),
            "stale .Client iface: {args:?}"
        );
    }
}
