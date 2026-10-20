use gamefocus_core::{BoostProvider, ProcessController};

pub fn platform_controller() -> Box<dyn ProcessController> {
    #[cfg(target_os = "linux")]
    {
        Box::new(gamefocus_linux::LinuxProcessController::new())
    }
    #[cfg(target_os = "windows")]
    {
        Box::new(gamefocus_windows::WindowsProcessController::new())
    }
    #[cfg(target_os = "macos")]
    {
        Box::new(gamefocus_macos::MacosProcessController::new())
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        compile_error!("unsupported OS");
        unreachable!()
    }
}

pub fn platform_boosters() -> Vec<Box<dyn BoostProvider>> {
    #[cfg(target_os = "linux")]
    {
        vec![Box::new(gamefocus_linux::GameModeBoost::new())]
    }
    #[cfg(not(target_os = "linux"))]
    {
        Vec::new()
    }
}
