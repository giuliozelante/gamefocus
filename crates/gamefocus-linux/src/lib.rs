//! Linux process control (SIGSTOP/SIGCONT) and optional GameMode boost.

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
        signal_pid(pid, SignalKind::Stop)
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

/// Soft-fail GameMode provider: uses `gamemoded` request via `gamemoderun`-style
/// dbus helper if `busctl` / `gamemoded` is present. v1 records intent only when
/// the `gamemoded --status` binary exists; full D-Bus client comes next.
pub struct GameModeBoost {
    active: bool,
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

    fn apply(&self) -> Result<BoostHandle> {
        if !Self::available() {
            return Err(Error::Boost {
                provider: "gamemode".into(),
                message: "gamemoded not found; install Feral GameMode".into(),
            });
        }
        // Best-effort: register a client via gamemoded CLI if present.
        let status = Command::new("busctl")
            .args([
                "--user",
                "call",
                "com.feralinteractive.GameMode",
                "/com/feralinteractive/GameMode",
                "com.feralinteractive.GameMode.Client",
                "RegisterGame",
                "i",
                &std::process::id().to_string(),
            ])
            .output();
        match status {
            Ok(out) if out.status.success() => {
                info!("GameMode RegisterGame succeeded");
                Ok(BoostHandle {
                    provider_id: "gamemode".into(),
                    payload: std::process::id().to_string(),
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
        let _ = Command::new("busctl")
            .args([
                "--user",
                "call",
                "com.feralinteractive.GameMode",
                "/com/feralinteractive/GameMode",
                "com.feralinteractive.GameMode.Client",
                "UnregisterGame",
                "i",
                &handle.payload,
            ])
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
