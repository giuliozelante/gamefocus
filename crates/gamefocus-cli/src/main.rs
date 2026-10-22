use clap::{Parser, Subcommand, ValueEnum};
use gamefocus_core::{
    collect_reclaim_candidates, default_snapshot_path, discover_profiles, effective_reclaim_mode,
    end_session, format_session_impact, load_session_profile, mem_available_bytes, park_targets,
    restore_reclaimed, soft_stop_targets, start_session, warn_heavy_vram_users, BoostProvider,
    ProcessController, Profile, ReclaimTarget, SessionMode, SessionSnapshot, SystemSupervisor,
};
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::process::{Command, ExitStatus};
use tracing::{info, warn, Level};

#[derive(Parser, Debug)]
#[command(name = "gamefocus", version, about = "Cross-platform game focus + optional OS boosts")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start a focus/boost session (embedded OS default if no --profile)
    Start {
        /// Path to a profile TOML. Omit to use ~/.config/gamefocus/default.toml or the embedded default.
        #[arg(long, short)]
        profile: Option<PathBuf>,
        /// Override Focus on/off
        #[arg(long, value_enum)]
        focus: Option<OnOff>,
        /// Override Boost on/off
        #[arg(long, value_enum)]
        boost: Option<OnOff>,
        /// Where to write the session snapshot (for `end`). Default: XDG/runtime path.
        #[arg(long)]
        snapshot: Option<PathBuf>,
        /// List what would be acted on without suspending/stopping
        #[arg(long)]
        dry_run: bool,
        /// Soft-stop reclaimable VRAM targets (Docker/Podman/systemd-user) without prompting
        #[arg(long)]
        reclaim_vram: bool,
    },
    /// End a session using a previously written snapshot
    #[command(visible_alias = "stop")]
    End {
        #[arg(long)]
        snapshot: Option<PathBuf>,
    },
    /// Show whether a snapshot file exists and summarize it
    Status {
        #[arg(long)]
        snapshot: Option<PathBuf>,
    },
    /// List discoverable profile TOML files
    Profiles,
    /// Start a session, run a command, end the session when it exits (Steam: gamefocus run -- %command%)
    Run {
        /// Path to a profile TOML. Omit to use ~/.config/gamefocus/default.toml or the embedded default.
        #[arg(long, short)]
        profile: Option<PathBuf>,
        /// Override Focus on/off
        #[arg(long, value_enum)]
        focus: Option<OnOff>,
        /// Override Boost on/off
        #[arg(long, value_enum)]
        boost: Option<OnOff>,
        /// Where to write the session snapshot. Default: XDG/runtime path.
        #[arg(long)]
        snapshot: Option<PathBuf>,
        /// Soft-stop reclaimable VRAM targets without prompting
        #[arg(long)]
        reclaim_vram: bool,
        /// Command and args (use `--` before the game command)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
        command: Vec<String>,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum OnOff {
    On,
    Off,
}

impl OnOff {
    fn as_bool(self) -> bool {
        matches!(self, OnOff::On)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_target(false)
        .init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Start {
            profile,
            focus,
            boost,
            snapshot,
            dry_run,
            reclaim_vram,
        } => cmd_start(profile, focus, boost, snapshot, dry_run, reclaim_vram)?,
        Commands::End { snapshot } => cmd_end(snapshot)?,
        Commands::Status { snapshot } => cmd_status(snapshot)?,
        Commands::Profiles => cmd_profiles()?,
        Commands::Run {
            profile,
            focus,
            boost,
            snapshot,
            reclaim_vram,
            command,
        } => {
            let code = cmd_run(profile, focus, boost, snapshot, reclaim_vram, command)?;
            if code != 0 {
                std::process::exit(code);
            }
        }
    }
    Ok(())
}

/// Map a child `ExitStatus` to a process exit code (Unix signals → 128+n).
fn exit_code_from_status(status: ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = status.signal() {
            return 128 + sig;
        }
    }
    1
}

fn snapshot_or_default(snapshot: Option<PathBuf>) -> PathBuf {
    snapshot.unwrap_or_else(default_snapshot_path)
}

fn cmd_start(
    profile_path: Option<PathBuf>,
    focus: Option<OnOff>,
    boost: Option<OnOff>,
    snapshot: Option<PathBuf>,
    dry_run: bool,
    reclaim_vram: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let (profile, source) = load_session_profile(profile_path.as_deref())?;
    info!(%source, "profile loaded");
    let snapshot_path = snapshot_or_default(snapshot);
    let mode = SessionMode {
        focus: focus.map(OnOff::as_bool),
        boost: boost.map(OnOff::as_bool),
    };

    let controller = platform_controller();
    let boosters = platform_boosters();
    let booster_refs: Vec<&dyn BoostProvider> = boosters.iter().map(|b| b.as_ref()).collect();

    warn_heavy_vram_users(&profile);

    if dry_run {
        dry_run_report(&profile, mode, controller.as_ref())?;
        return Ok(());
    }

    let reclaim_mode = effective_reclaim_mode(
        profile.gpu.vram_reclaim.policy,
        reclaim_vram,
        io::stdin().is_terminal(),
    );
    let candidates = collect_reclaim_candidates(&profile);
    let supervisor = SystemSupervisor;
    let reclaimed = soft_stop_targets(
        &candidates,
        reclaim_mode,
        prompt_reclaim,
        &supervisor,
    );

    if let Some(parent) = snapshot_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let before_mem = mem_available_bytes();
    let mut snap = start_session(&profile, mode, controller.as_ref(), &booster_refs)?;
    snap.reclaimed = reclaimed;
    // Brief settle so PAGEOUT / working-set trim can show up in MemAvailable.
    std::thread::sleep(std::time::Duration::from_millis(400));
    let after_mem = mem_available_bytes();
    fs::write(&snapshot_path, serde_json::to_string_pretty(&snap)?)?;
    info!(
        path = %snapshot_path.display(),
        suspended = snap.suspended.len(),
        reclaimed = snap.reclaimed.len(),
        focus = snap.focus_applied,
        boost = snap.boost_applied,
        "session started"
    );
    println!(
        "{}",
        format_session_impact(
            snap.suspended.len(),
            snap.boost_applied,
            before_mem,
            after_mem
        )
    );
    if !snap.reclaimed.is_empty() {
        println!(
            "reclaimed {} VRAM target(s); will restore on end",
            snap.reclaimed.len()
        );
    }
    Ok(())
}

fn prompt_reclaim(target: &ReclaimTarget) -> bool {
    eprint!(
        "Stop {} for this session and restore on end? [Y/n]: ",
        target.label()
    );
    let _ = io::stderr().flush();
    let mut line = String::new();
    if io::stdin().read_line(&mut line).is_err() {
        return false;
    }
    let answer = line.trim().to_ascii_lowercase();
    answer.is_empty() || answer == "y" || answer == "yes"
}

fn cmd_run(
    profile_path: Option<PathBuf>,
    focus: Option<OnOff>,
    boost: Option<OnOff>,
    snapshot: Option<PathBuf>,
    reclaim_vram: bool,
    command: Vec<String>,
) -> Result<i32, Box<dyn std::error::Error>> {
    if command.is_empty() {
        return Err("run requires a command (e.g. gamefocus run -- %command%)".into());
    }
    let snapshot_path = snapshot_or_default(snapshot.clone());
    cmd_start(profile_path, focus, boost, snapshot.clone(), false, reclaim_vram)?;
    let mut child = Command::new(&command[0]);
    if command.len() > 1 {
        child.args(&command[1..]);
    }
    info!(cmd = %command.join(" "), "running wrapped command");
    let status = match child.status() {
        Ok(s) => s,
        Err(err) => {
            warn!(%err, "wrapped command failed to spawn; ending session");
            let _ = cmd_end(Some(snapshot_path));
            return Err(err.into());
        }
    };
    if let Err(err) = cmd_end(Some(snapshot_path)) {
        warn!(%err, "session end after run failed");
    }
    Ok(exit_code_from_status(status))
}

fn cmd_end(snapshot: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let snapshot_path = snapshot_or_default(snapshot);
    let text = fs::read_to_string(&snapshot_path)?;
    let snap: SessionSnapshot = serde_json::from_str(&text)?;
    let controller = platform_controller();
    let boosters = platform_boosters();
    let booster_refs: Vec<&dyn BoostProvider> = boosters.iter().map(|b| b.as_ref()).collect();
    end_session(&snap, controller.as_ref(), &booster_refs)?;
    if !snap.reclaimed.is_empty() {
        restore_reclaimed(&snap.reclaimed, &SystemSupervisor);
        info!(count = snap.reclaimed.len(), "vram reclaim restored");
    }
    let _ = fs::remove_file(&snapshot_path);
    info!("session ended");
    Ok(())
}

fn cmd_status(snapshot: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let snapshot_path = snapshot_or_default(snapshot);
    if !snapshot_path.exists() {
        println!("no active session ({})", snapshot_path.display());
        return Ok(());
    }
    let snap: SessionSnapshot = serde_json::from_str(&fs::read_to_string(&snapshot_path)?)?;
    println!(
        "profile={} focus={} boost={} suspended={}",
        snap.profile_name,
        snap.focus_applied,
        snap.boost_applied,
        snap.suspended.len()
    );
    for p in &snap.suspended {
        println!("  {} pid={} action={}", p.name, p.pid, p.action);
    }
    Ok(())
}

fn cmd_profiles() -> Result<(), Box<dyn std::error::Error>> {
    let found = discover_profiles();
    if found.is_empty() {
        println!("no profiles found");
        return Ok(());
    }
    for path in found {
        println!("{}", path.display());
    }
    Ok(())
}

fn dry_run_report(
    profile: &Profile,
    mode: SessionMode,
    controller: &dyn ProcessController,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("profile: {}", profile.name);
    println!(
        "focus: {}  boost: {}",
        mode.focus_enabled(profile),
        mode.boost_enabled(profile)
    );
    if !mode.focus_enabled(profile) {
        return Ok(());
    }
    let processes = controller.list_processes()?;
    let targets = park_targets(profile, &processes, std::process::id());
    println!(
        "would park {} of {} processes:",
        targets.len(),
        processes.len()
    );
    for info in targets {
        println!("  {} pid={}", info.name, info.pid);
    }
    Ok(())
}

fn platform_controller() -> Box<dyn ProcessController> {
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

fn platform_boosters() -> Vec<Box<dyn BoostProvider>> {
    #[cfg(target_os = "linux")]
    {
        vec![Box::new(gamefocus_linux::GameModeBoost::new())]
    }
    #[cfg(target_os = "windows")]
    {
        vec![
            Box::new(gamefocus_windows::PowerPlanBoost::system()),
            Box::new(gamefocus_windows::WorkingSetBoost::system()),
            Box::new(gamefocus_windows::CpuAffinityBoost::system()),
        ]
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn run_parses_trailing_command_after_double_dash() {
        let cli = Cli::try_parse_from([
            "gamefocus",
            "run",
            "--reclaim-vram",
            "--",
            "sleep",
            "1",
        ])
        .expect("parse");
        match cli.command {
            Commands::Run {
                reclaim_vram,
                command,
                ..
            } => {
                assert!(reclaim_vram);
                assert_eq!(command, vec!["sleep", "1"]);
            }
            other => panic!("expected Run, got {other:?}"),
        }
    }

    #[test]
    fn run_requires_a_command() {
        assert!(Cli::try_parse_from(["gamefocus", "run"]).is_err());
    }

    #[test]
    fn exit_code_from_success_status() {
        #[cfg(unix)]
        {
            let status = Command::new("true").status().expect("true");
            assert_eq!(exit_code_from_status(status), 0);
            let status = Command::new("false").status().expect("false");
            assert_eq!(exit_code_from_status(status), 1);
        }
        #[cfg(windows)]
        {
            let status = Command::new("cmd")
                .args(["/C", "exit /B 0"])
                .status()
                .expect("cmd");
            assert_eq!(exit_code_from_status(status), 0);
            let status = Command::new("cmd")
                .args(["/C", "exit /B 7"])
                .status()
                .expect("cmd");
            assert_eq!(exit_code_from_status(status), 7);
        }
    }
}

