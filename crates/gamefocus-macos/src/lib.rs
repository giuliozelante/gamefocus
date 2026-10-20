//! macOS backends — stubs until Phase 5.

use gamefocus_core::{Error, ProcessController, ProcessInfo, Result};

pub struct MacosProcessController;

impl MacosProcessController {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MacosProcessController {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessController for MacosProcessController {
    fn list_processes(&self) -> Result<Vec<ProcessInfo>> {
        Err(Error::Other(
            "MacosProcessController not implemented yet (Phase 5)".into(),
        ))
    }

    fn suspend(&self, pid: u32) -> Result<()> {
        let _ = pid;
        Err(Error::Other(
            "MacosProcessController not implemented yet (Phase 5)".into(),
        ))
    }

    fn resume(&self, pid: u32) -> Result<()> {
        let _ = pid;
        Err(Error::Other(
            "MacosProcessController not implemented yet (Phase 5)".into(),
        ))
    }

    fn soft_stop(&self, pid: u32) -> Result<()> {
        let _ = pid;
        Err(Error::Other(
            "MacosProcessController not implemented yet (Phase 5)".into(),
        ))
    }
}
