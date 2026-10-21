//! macOS Focus: suspend and resume via SIGSTOP / SIGCONT.
//! Boost has no provider on macOS yet.

use gamefocus_core::{Error, ProcessController, ProcessInfo, Result};

pub trait ProcessOps: Send + Sync {
    fn list(&self) -> Result<Vec<ProcessInfo>>;
    fn suspend(&self, pid: u32) -> Result<()>;
    fn resume(&self, pid: u32) -> Result<()>;
    fn soft_stop(&self, pid: u32) -> Result<()>;
}

pub struct MacosProcessController<O = SystemProcessOps> {
    ops: O,
}

impl MacosProcessController<SystemProcessOps> {
    pub fn new() -> Self {
        Self {
            ops: SystemProcessOps,
        }
    }
}

impl Default for MacosProcessController<SystemProcessOps> {
    fn default() -> Self {
        Self::new()
    }
}

impl<O> MacosProcessController<O> {
    pub fn from_ops(ops: O) -> Self {
        Self { ops }
    }
}

impl<O: ProcessOps> ProcessController for MacosProcessController<O> {
    fn list_processes(&self) -> Result<Vec<ProcessInfo>> {
        self.ops.list()
    }

    fn suspend(&self, pid: u32) -> Result<()> {
        self.ops.suspend(pid)
    }

    fn resume(&self, pid: u32) -> Result<()> {
        self.ops.resume(pid)
    }

    fn soft_stop(&self, pid: u32) -> Result<()> {
        self.ops.soft_stop(pid)
    }
}

/// OS process backend. `kill` compiles only on macOS.
pub struct SystemProcessOps;

impl ProcessOps for SystemProcessOps {
    fn list(&self) -> Result<Vec<ProcessInfo>> {
        #[cfg(target_os = "macos")]
        {
            let out = std::process::Command::new("ps")
                .args(["-ax", "-o", "pid=,ucomm="])
                .output()?;
            Ok(parse_ps(&String::from_utf8_lossy(&out.stdout)))
        }
        #[cfg(not(target_os = "macos"))]
        {
            Err(Error::Other(
                "MacosProcessController only works on macOS".into(),
            ))
        }
    }

    fn suspend(&self, pid: u32) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            signal_pid(pid, SignalKind::Stop)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = pid;
            Err(Error::Other(
                "MacosProcessController only works on macOS".into(),
            ))
        }
    }

    fn resume(&self, pid: u32) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            signal_pid(pid, SignalKind::Cont)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = pid;
            Err(Error::Other(
                "MacosProcessController only works on macOS".into(),
            ))
        }
    }

    fn soft_stop(&self, pid: u32) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            signal_pid(pid, SignalKind::Term)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = pid;
            Err(Error::Other(
                "MacosProcessController only works on macOS".into(),
            ))
        }
    }
}

#[cfg(target_os = "macos")]
enum SignalKind {
    Stop,
    Cont,
    Term,
}

#[cfg(target_os = "macos")]
fn signal_pid(pid: u32, kind: SignalKind) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid;
        let sig = match kind {
            SignalKind::Stop => Signal::SIGSTOP,
            SignalKind::Cont => Signal::SIGCONT,
            SignalKind::Term => Signal::SIGTERM,
        };
        kill(Pid::from_raw(pid as i32), sig).map_err(|err| Error::Process {
            pid,
            message: err.to_string(),
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (pid, kind);
        Err(Error::Other(
            "MacosProcessController only works on macOS".into(),
        ))
    }
}

/// `ps -ax -o pid=,ucomm=` lines. A non-numeric first field is skipped.
pub fn parse_ps(text: &str) -> Vec<ProcessInfo> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.splitn(2, char::is_whitespace);
        let Some(pid_str) = parts.next() else {
            continue;
        };
        let Ok(pid) = pid_str.parse::<u32>() else {
            continue;
        };
        let name = parts.next().unwrap_or("").trim().to_string();
        if name.is_empty() {
            continue;
        }
        out.push(ProcessInfo {
            pid,
            name,
            exe_path: None,
            cmdline: None,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ps_reads_pid_and_name() {
        let text = "  4321 Google Chrome\n";
        let procs = parse_ps(text);
        assert_eq!(procs.len(), 1);
        assert_eq!(procs[0].pid, 4321);
        assert_eq!(procs[0].name, "Google Chrome");
    }

    #[derive(Default)]
    struct LogOps {
        calls: std::sync::Mutex<Vec<(&'static str, u32)>>,
    }

    impl LogOps {
        fn calls(&self) -> Vec<(&'static str, u32)> {
            self.calls.lock().expect("calls").clone()
        }
    }

    impl ProcessOps for LogOps {
        fn list(&self) -> Result<Vec<ProcessInfo>> {
            Ok(Vec::new())
        }

        fn suspend(&self, pid: u32) -> Result<()> {
            self.calls.lock().expect("calls").push(("suspend", pid));
            Ok(())
        }

        fn resume(&self, pid: u32) -> Result<()> {
            self.calls.lock().expect("calls").push(("resume", pid));
            Ok(())
        }

        fn soft_stop(&self, pid: u32) -> Result<()> {
            self.calls.lock().expect("calls").push(("soft_stop", pid));
            Ok(())
        }
    }

    #[test]
    fn suspend_and_resume_record_the_pid() {
        let ops = LogOps::default();
        let controller = MacosProcessController::from_ops(ops);
        controller.suspend(42).expect("suspend");
        controller.resume(42).expect("resume");
        assert_eq!(
            controller.ops.calls(),
            vec![("suspend", 42), ("resume", 42)]
        );
    }
}
