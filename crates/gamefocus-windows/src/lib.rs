//! Windows Focus (suspend/resume), power-plan Boost, and working-set Boost.

/// GUID printed by `powercfg /getactivescheme`.
pub fn parse_active_scheme(text: &str) -> Option<String> {
    text.split_whitespace().find_map(|token| {
        let bytes = token.as_bytes();
        if bytes.len() == 36 && bytes.get(8) == Some(&b'-') && bytes.get(13) == Some(&b'-') {
            Some(token.to_string())
        } else {
            None
        }
    })
}

use gamefocus_core::{BoostHandle, BoostProvider, Error, ProcessController, ProcessInfo, Result};

/// `powercfg` High Performance scheme.
pub const HIGH_PERFORMANCE_SCHEME: &str = "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c";

pub trait SchemeStore: Send + Sync {
    fn active_scheme(&self) -> Result<String>;
    fn set_active(&self, guid: &str) -> Result<()>;
}

pub struct PowerPlanBoost<S> {
    store: S,
}

impl<S> PowerPlanBoost<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }
}

impl<S: SchemeStore> BoostProvider for PowerPlanBoost<S> {
    fn id(&self) -> &'static str {
        "power_plan"
    }

    fn apply(&self, _ctx: &gamefocus_core::BoostContext<'_>) -> Result<BoostHandle> {
        let previous = self.store.active_scheme()?;
        if previous != HIGH_PERFORMANCE_SCHEME {
            self.store.set_active(HIGH_PERFORMANCE_SCHEME)?;
        }
        Ok(BoostHandle {
            provider_id: self.id().into(),
            payload: previous,
        })
    }

    fn rollback(&self, handle: BoostHandle) -> Result<()> {
        if handle.payload != HIGH_PERFORMANCE_SCHEME {
            self.store.set_active(&handle.payload)?;
        }
        Ok(())
    }
}

/// `powercfg.exe` adapter. Calls succeed only on Windows.
pub struct PowercfgStore;

impl SchemeStore for PowercfgStore {
    fn active_scheme(&self) -> Result<String> {
        #[cfg(windows)]
        {
            let out = std::process::Command::new("powercfg")
                .arg("/getactivescheme")
                .output()?;
            let text = String::from_utf8_lossy(&out.stdout);
            parse_active_scheme(&text).ok_or_else(|| Error::Boost {
                provider: "power_plan".into(),
                message: format!("unparsed powercfg output: {text}"),
            })
        }
        #[cfg(not(windows))]
        {
            Err(Error::Boost {
                provider: "power_plan".into(),
                message: "powercfg is only available on Windows".into(),
            })
        }
    }

    fn set_active(&self, guid: &str) -> Result<()> {
        #[cfg(windows)]
        {
            let status = std::process::Command::new("powercfg")
                .args(["/setactive", guid])
                .status()?;
            if status.success() {
                return Ok(());
            }
            Err(Error::Boost {
                provider: "power_plan".into(),
                message: format!("powercfg /setactive {guid} exited {status}"),
            })
        }
        #[cfg(not(windows))]
        {
            let _ = guid;
            Err(Error::Boost {
                provider: "power_plan".into(),
                message: "powercfg is only available on Windows".into(),
            })
        }
    }
}

impl PowerPlanBoost<PowercfgStore> {
    pub fn system() -> Self {
        Self::new(PowercfgStore)
    }
}

pub trait WorkingSetOps: Send + Sync {
    fn empty_working_set(&self, pid: u32) -> Result<()>;
}

/// Trims the working set of processes Focus just suspended. Rollback is a no-op.
pub struct WorkingSetBoost<O> {
    ops: O,
}

impl<O> WorkingSetBoost<O> {
    pub fn new(ops: O) -> Self {
        Self { ops }
    }
}

impl WorkingSetBoost<SystemWorkingSetOps> {
    pub fn system() -> Self {
        Self::new(SystemWorkingSetOps)
    }
}

impl<O: WorkingSetOps> BoostProvider for WorkingSetBoost<O> {
    fn id(&self) -> &'static str {
        "working_set"
    }

    fn apply(&self, ctx: &gamefocus_core::BoostContext<'_>) -> Result<BoostHandle> {
        let mut trimmed = Vec::new();
        for proc in ctx.suspended {
            if proc.action != "suspend" {
                continue;
            }
            match self.ops.empty_working_set(proc.pid) {
                Ok(()) => trimmed.push(proc.pid.to_string()),
                Err(err) => tracing::warn!(pid = proc.pid, %err, "working_set soft-fail"),
            }
        }
        Ok(BoostHandle {
            provider_id: self.id().into(),
            payload: trimmed.join(","),
        })
    }

    fn rollback(&self, _handle: BoostHandle) -> Result<()> {
        Ok(())
    }
}

pub struct SystemWorkingSetOps;

impl WorkingSetOps for SystemWorkingSetOps {
    fn empty_working_set(&self, pid: u32) -> Result<()> {
        #[cfg(windows)]
        {
            empty_working_set_win32(pid)
        }
        #[cfg(not(windows))]
        {
            let _ = pid;
            Err(Error::Boost {
                provider: "working_set".into(),
                message: "EmptyWorkingSet is only available on Windows".into(),
            })
        }
    }
}

#[cfg(windows)]
fn empty_working_set_win32(pid: u32) -> Result<()> {
    use std::os::raw::c_void;
    const PROCESS_QUERY_INFORMATION: u32 = 0x0400;
    const PROCESS_SET_QUOTA: u32 = 0x0100;
    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
        fn CloseHandle(handle: *mut c_void) -> i32;
        fn GetLastError() -> u32;
        fn K32EmptyWorkingSet(handle: *mut c_void) -> i32;
    }
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_SET_QUOTA, 0, pid);
        if handle.is_null() {
            return Err(Error::Boost {
                provider: "working_set".into(),
                message: format!("OpenProcess {}", GetLastError()),
            });
        }
        let ok = K32EmptyWorkingSet(handle);
        CloseHandle(handle);
        if ok == 0 {
            return Err(Error::Boost {
                provider: "working_set".into(),
                message: format!("EmptyWorkingSet {}", GetLastError()),
            });
        }
        Ok(())
    }
}

pub trait ProcessOps: Send + Sync {
    fn list(&self) -> Result<Vec<ProcessInfo>>;
    fn suspend(&self, pid: u32) -> Result<()>;
    fn resume(&self, pid: u32) -> Result<()>;
    fn soft_stop(&self, pid: u32) -> Result<()>;
}

pub struct WindowsProcessController<O = SystemProcessOps> {
    ops: O,
}

impl WindowsProcessController<SystemProcessOps> {
    pub fn new() -> Self {
        Self {
            ops: SystemProcessOps,
        }
    }
}

impl Default for WindowsProcessController<SystemProcessOps> {
    fn default() -> Self {
        Self::new()
    }
}

impl<O> WindowsProcessController<O> {
    pub fn from_ops(ops: O) -> Self {
        Self { ops }
    }
}

impl<O: ProcessOps> ProcessController for WindowsProcessController<O> {
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

/// OS process backend. Win32 calls compile only on Windows.
pub struct SystemProcessOps;

impl ProcessOps for SystemProcessOps {
    fn list(&self) -> Result<Vec<ProcessInfo>> {
        #[cfg(windows)]
        {
            let out = std::process::Command::new("tasklist")
                .args(["/FO", "CSV", "/NH"])
                .output()?;
            Ok(parse_tasklist_csv(&String::from_utf8_lossy(&out.stdout)))
        }
        #[cfg(not(windows))]
        {
            Err(Error::Other(
                "WindowsProcessController only works on Windows".into(),
            ))
        }
    }

    fn suspend(&self, pid: u32) -> Result<()> {
        #[cfg(windows)]
        {
            nt_process(pid, true)
        }
        #[cfg(not(windows))]
        {
            let _ = pid;
            Err(Error::Other(
                "WindowsProcessController only works on Windows".into(),
            ))
        }
    }

    fn resume(&self, pid: u32) -> Result<()> {
        #[cfg(windows)]
        {
            nt_process(pid, false)
        }
        #[cfg(not(windows))]
        {
            let _ = pid;
            Err(Error::Other(
                "WindowsProcessController only works on Windows".into(),
            ))
        }
    }

    fn soft_stop(&self, pid: u32) -> Result<()> {
        #[cfg(windows)]
        {
            let status = std::process::Command::new("taskkill")
                .args(["/PID", &pid.to_string()])
                .status()?;
            if status.success() {
                return Ok(());
            }
            Err(Error::Process {
                pid,
                message: format!("taskkill exited {status}"),
            })
        }
        #[cfg(not(windows))]
        {
            let _ = pid;
            Err(Error::Other(
                "WindowsProcessController only works on Windows".into(),
            ))
        }
    }
}

/// CSV from `tasklist /FO CSV /NH`.
pub fn parse_tasklist_csv(text: &str) -> Vec<ProcessInfo> {
    let mut out = Vec::new();
    for line in text.lines() {
        let cols = split_csv_line(line);
        if cols.len() < 2 {
            continue;
        }
        let Ok(pid) = cols[1].parse::<u32>() else {
            continue;
        };
        out.push(ProcessInfo {
            pid,
            name: cols[0].clone(),
            exe_path: None,
            cmdline: None,
        });
    }
    out
}

fn split_csv_line(line: &str) -> Vec<String> {
    let mut cols = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    for ch in line.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                cols.push(std::mem::take(&mut cur));
            }
            _ => cur.push(ch),
        }
    }
    if !cur.is_empty() || !cols.is_empty() {
        cols.push(cur);
    }
    cols
}

#[cfg(windows)]
fn nt_process(pid: u32, suspend: bool) -> Result<()> {
    use std::os::raw::c_void;
    const PROCESS_SUSPEND_RESUME: u32 = 0x0800;
    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
        fn CloseHandle(handle: *mut c_void) -> i32;
        fn GetLastError() -> u32;
    }
    #[link(name = "ntdll")]
    extern "system" {
        fn NtSuspendProcess(handle: *mut c_void) -> i32;
        fn NtResumeProcess(handle: *mut c_void) -> i32;
    }
    unsafe {
        let handle = OpenProcess(PROCESS_SUSPEND_RESUME, 0, pid);
        if handle.is_null() {
            return Err(Error::Process {
                pid,
                message: format!("OpenProcess {}", GetLastError()),
            });
        }
        let status = if suspend {
            NtSuspendProcess(handle)
        } else {
            NtResumeProcess(handle)
        };
        CloseHandle(handle);
        if status < 0 {
            return Err(Error::Process {
                pid,
                message: format!("ntstatus {status:#x}"),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct MemScheme {
        active: Mutex<String>,
        sets: Mutex<Vec<String>>,
    }

    impl MemScheme {
        fn new(active: &str) -> Self {
            Self {
                active: Mutex::new(active.to_string()),
                sets: Mutex::new(Vec::new()),
            }
        }

        fn last_set(&self) -> Option<String> {
            self.sets.lock().expect("sets").last().cloned()
        }
    }

    #[derive(Default)]
    struct LogOps {
        calls: Mutex<Vec<(&'static str, u32)>>,
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

    impl SchemeStore for MemScheme {
        fn active_scheme(&self) -> Result<String> {
            Ok(self.active.lock().expect("active").clone())
        }

        fn set_active(&self, guid: &str) -> Result<()> {
            *self.active.lock().expect("active") = guid.to_string();
            self.sets.lock().expect("sets").push(guid.to_string());
            Ok(())
        }
    }

    #[test]
    fn parse_active_scheme_reads_powercfg_guid() {
        let text = "Power Scheme GUID: 381b4222-f694-41f0-9685-ff5bb260df2e  (Balanced)\r\n";
        assert_eq!(
            parse_active_scheme(text).as_deref(),
            Some("381b4222-f694-41f0-9685-ff5bb260df2e")
        );
    }

    #[test]
    fn boost_apply_saves_previous_scheme_and_rollback_restores_it() {
        let previous = "381b4222-f694-41f0-9685-ff5bb260df2e";
        let store = MemScheme::new(previous);
        let boost = PowerPlanBoost::new(store);
        let ctx = gamefocus_core::BoostContext { suspended: &[] };
        let handle = boost.apply(&ctx).expect("apply");
        assert_eq!(handle.payload, previous);
        assert_eq!(boost.store.last_set(), Some(HIGH_PERFORMANCE_SCHEME.to_string()));
        boost.rollback(handle).expect("rollback");
        assert_eq!(boost.store.last_set(), Some(previous.to_string()));
    }

    #[derive(Default)]
    struct LogWorkingSet {
        pids: Mutex<Vec<u32>>,
    }

    impl WorkingSetOps for LogWorkingSet {
        fn empty_working_set(&self, pid: u32) -> Result<()> {
            self.pids.lock().expect("pids").push(pid);
            Ok(())
        }
    }

    #[test]
    fn working_set_trims_suspended_pids() {
        let ops = LogWorkingSet::default();
        let boost = WorkingSetBoost::new(ops);
        let suspended = [gamefocus_core::SuspendedProcess {
            pid: 42,
            name: "chrome.exe".into(),
            action: "suspend".into(),
        }];
        let ctx = gamefocus_core::BoostContext {
            suspended: &suspended,
        };
        let handle = boost.apply(&ctx).expect("apply");
        assert_eq!(handle.payload, "42");
        assert_eq!(*boost.ops.pids.lock().expect("pids"), vec![42]);
        boost.rollback(handle).expect("rollback");
    }

    #[test]
    fn suspend_and_resume_record_the_pid() {
        let ops = LogOps::default();
        let controller = WindowsProcessController::from_ops(ops);
        controller.suspend(42).expect("suspend");
        controller.resume(42).expect("resume");
        assert_eq!(controller.ops.calls(), vec![("suspend", 42), ("resume", 42)]);
    }

    #[test]
    fn parse_tasklist_csv_reads_name_and_pid() {
        let text = "\"chrome.exe\",\"4321\",\"Console\",\"1\",\"1 K\"\r\n";
        let procs = parse_tasklist_csv(text);
        assert_eq!(procs.len(), 1);
        assert_eq!(procs[0].name, "chrome.exe");
        assert_eq!(procs[0].pid, 4321);
    }
}
