use clap::{Parser, Subcommand, ValueEnum};
use gamefocus_core::{
    end_session, start_session, BoostProvider, ProcessController, Profile, SessionMode,
    SessionSnapshot,
};
use std::fs;
use std::path::{Path, PathBuf};
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
        /// Where to write the session snapshot (for `end`)
        #[arg(long, default_value = "gamefocus-session.json")]
        snapshot: PathBuf,
        /// List what would be acted on without suspending/stopping
        #[arg(long)]
        dry_run: bool,
    },
    /// End a session using a previously written snapshot
    End {
        #[arg(long, default_value = "gamefocus-session.json")]
        snapshot: PathBuf,
    },
    /// Show whether a snapshot file exists and summarize it
    Status {
        #[arg(long, default_value = "gamefocus-session.json")]
        snapshot: PathBuf,
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
        } => cmd_start(profile, focus, boost, snapshot, dry_run)?,
        Commands::End { snapshot } => cmd_end(snapshot)?,
        Commands::Status { snapshot } => cmd_status(snapshot)?,
    }
    Ok(())
}

fn cmd_start(
    profile_path: PathBuf,
    focus: Option<OnOff>,
    boost: Option<OnOff>,
    snapshot_path: PathBuf,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let profile = Profile::load_file(&profile_path)?;
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

fn cmd_end(snapshot_path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
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

fn cmd_status(snapshot_path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
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
    println!("would evaluate {} processes (whitelist + shell protect applied at start)", processes.len());
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
    #[cfg(not(target_os = "linux"))]
    {
        Vec::new()
    }
}

#[allow(dead_code)]
fn resolve_profile(path: &Path) -> PathBuf {
    path.to_path_buf()
}
