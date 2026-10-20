//! Windows backends — stubs until Phase 4.

use gamefocus_core::{Error, ProcessController, ProcessInfo, Result};

pub struct WindowsProcessController;

impl WindowsProcessController {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowsProcessController {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessController for WindowsProcessController {
    fn list_processes(&self) -> Result<Vec<ProcessInfo>> {
        Err(Error::Other(
            "WindowsProcessController not implemented yet (Phase 4)".into(),
        ))
    }

    fn suspend(&self, pid: u32) -> Result<()> {
        let _ = pid;
        Err(Error::Other(
            "WindowsProcessController not implemented yet (Phase 4)".into(),
        ))
    }

    fn resume(&self, pid: u32) -> Result<()> {
        let _ = pid;
        Err(Error::Other(
            "WindowsProcessController not implemented yet (Phase 4)".into(),
        ))
    }

    fn soft_stop(&self, pid: u32) -> Result<()> {
        let _ = pid;
        Err(Error::Other(
            "WindowsProcessController not implemented yet (Phase 4)".into(),
        ))
    }
}
