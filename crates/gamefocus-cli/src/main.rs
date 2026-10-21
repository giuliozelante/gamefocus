use clap::{Parser, Subcommand, ValueEnum};
use gamefocus_core::{
    default_snapshot_path, discover_profiles, end_session, park_targets, resolve_profile_path,
    start_session, BoostProvider, ProcessController, Profile, SessionMode, SessionSnapshot,
};
use std::fs;
use std::path::PathBuf;
use tracing::{info, Level};

#[derive(Parser, Debug)]
#[command(name = "gamefocus", version, about = "Cross-platform game focus + optional OS boosts")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start a focus/boost session from a profile
    Start {
        /// Path to a profile TOML
        #[arg(long, short)]
        profile: PathBuf,
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
    },
    /// End a session using a previously written snapshot
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
        } => cmd_start(profile, focus, boost, snapshot, dry_run)?,
        Commands::End { snapshot } => cmd_end(snapshot)?,
        Commands::Status { snapshot } => cmd_status(snapshot)?,
        Commands::Profiles => cmd_profiles()?,
    }
    Ok(())
}

fn snapshot_or_default(snapshot: Option<PathBuf>) -> PathBuf {
    snapshot.unwrap_or_else(default_snapshot_path)
}

fn cmd_start(
    profile_path: PathBuf,
    focus: Option<OnOff>,
    boost: Option<OnOff>,
    snapshot: Option<PathBuf>,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let resolved = resolve_profile_path(&profile_path)?;
    let profile = Profile::load_file(&resolved)?;
    let snapshot_path = snapshot_or_default(snapshot);
    let mode = SessionMode {
        focus: focus.map(OnOff::as_bool),
        boost: boost.map(OnOff::as_bool),
    };

    let controller = platform_controller();
    let boosters = platform_boosters();
    let booster_refs: Vec<&dyn BoostProvider> = boosters.iter().map(|b| b.as_ref()).collect();

    if dry_run {
        dry_run_report(&profile, mode, controller.as_ref())?;
        return Ok(());
    }

    if let Some(parent) = snapshot_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let snap = start_session(&profile, mode, controller.as_ref(), &booster_refs)?;
    fs::write(&snapshot_path, serde_json::to_string_pretty(&snap)?)?;
    info!(
        path = %snapshot_path.display(),
        suspended = snap.suspended.len(),
        focus = snap.focus_applied,
        boost = snap.boost_applied,
        "session started"
    );
    Ok(())
}

fn cmd_end(snapshot: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let snapshot_path = snapshot_or_default(snapshot);
    let text = fs::read_to_string(&snapshot_path)?;
    let snap: SessionSnapshot = serde_json::from_str(&text)?;
    let controller = platform_controller();
    let boosters = platform_boosters();
    let booster_refs: Vec<&dyn BoostProvider> = boosters.iter().map(|b| b.as_ref()).collect();
    end_session(&snap, controller.as_ref(), &booster_refs)?;
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

