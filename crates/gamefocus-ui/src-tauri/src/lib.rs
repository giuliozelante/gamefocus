//! Tauri tray + control window for gamefocus sessions.

mod host;
mod paths;

use gamefocus_core::{
    collect_reclaim_candidates, effective_reclaim_mode, end_session, load_session_profile,
    restore_reclaimed, soft_stop_targets, start_session, warn_heavy_vram_users, Profile,
    ReclaimTarget, SessionMode, SessionSnapshot, SystemSupervisor,
};
use host::{platform_boosters, platform_controller};
use paths::{default_snapshot_path, discover_profiles, resolve_profile_path};
use rfd::{MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, State, WindowEvent,
};
use tracing::{info, warn};

struct AppState {
    inner: Mutex<UiState>,
}

#[derive(Debug, Clone)]
struct UiState {
    /// `None` means resolve user default.toml or the embedded OS profile.
    profile: Option<PathBuf>,
    focus: bool,
    boost: bool,
    snapshot: PathBuf,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            profile: None,
            focus: true,
            boost: !cfg!(target_os = "macos"),
            snapshot: default_snapshot_path(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusDto {
    active: bool,
    profile_name: Option<String>,
    focus_applied: bool,
    boost_applied: bool,
    suspended: usize,
    profile_path: String,
    focus: bool,
    boost: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileOption {
    path: String,
    name: String,
}

#[tauri::command]
fn list_profiles() -> Vec<ProfileOption> {
    let mut out = vec![ProfileOption {
        path: String::new(),
        name: "Default (auto)".into(),
    }];
    for path in discover_profiles() {
        let name = Profile::load_file(&path)
            .map(|p| p.name)
            .unwrap_or_else(|_| {
                path.file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string())
            });
        out.push(ProfileOption {
            path: path.display().to_string(),
            name,
        });
    }
    out
}

#[tauri::command]
fn get_status(state: State<'_, AppState>) -> Result<StatusDto, String> {
    let ui = state.inner.lock().map_err(|e| e.to_string())?;
    Ok(status_from_ui(&ui))
}

#[tauri::command]
fn set_toggles(state: State<'_, AppState>, focus: bool, boost: bool) -> Result<StatusDto, String> {
    let mut ui = state.inner.lock().map_err(|e| e.to_string())?;
    ui.focus = focus;
    ui.boost = boost;
    Ok(status_from_ui(&ui))
}

#[tauri::command]
fn set_profile(state: State<'_, AppState>, path: String) -> Result<StatusDto, String> {
    let mut ui = state.inner.lock().map_err(|e| e.to_string())?;
    if path.trim().is_empty() {
        ui.profile = None;
    } else {
        let resolved = resolve_profile_path(&path).map_err(|e| e.to_string())?;
        Profile::load_file(&resolved).map_err(|e| e.to_string())?;
        ui.profile = Some(resolved);
    }
    Ok(status_from_ui(&ui))
}

#[tauri::command]
fn start_focus_session(state: State<'_, AppState>) -> Result<StatusDto, String> {
    let ui = {
        let guard = state.inner.lock().map_err(|e| e.to_string())?;
        guard.clone()
    };
    run_start(&ui)?;
    let ui = state.inner.lock().map_err(|e| e.to_string())?;
    Ok(status_from_ui(&ui))
}

#[tauri::command]
fn end_focus_session(state: State<'_, AppState>) -> Result<StatusDto, String> {
    let snapshot = {
        let ui = state.inner.lock().map_err(|e| e.to_string())?;
        ui.snapshot.clone()
    };
    run_end(&snapshot)?;
    let ui = state.inner.lock().map_err(|e| e.to_string())?;
    Ok(status_from_ui(&ui))
}

fn status_from_ui(ui: &UiState) -> StatusDto {
    if ui.snapshot.exists() {
        match fs::read_to_string(&ui.snapshot)
            .ok()
            .and_then(|t| serde_json::from_str::<SessionSnapshot>(&t).ok())
        {
            Some(snap) => StatusDto {
                active: true,
                profile_name: Some(snap.profile_name),
                focus_applied: snap.focus_applied,
                boost_applied: snap.boost_applied,
                suspended: snap.suspended.len(),
                profile_path: profile_label(ui),
                focus: ui.focus,
                boost: ui.boost,
            },
            None => StatusDto {
                active: true,
                profile_name: None,
                focus_applied: false,
                boost_applied: false,
                suspended: 0,
                profile_path: profile_label(ui),
                focus: ui.focus,
                boost: ui.boost,
            },
        }
    } else {
        StatusDto {
            active: false,
            profile_name: None,
            focus_applied: false,
            boost_applied: false,
            suspended: 0,
            profile_path: profile_label(ui),
            focus: ui.focus,
            boost: ui.boost,
        }
    }
}

fn profile_label(ui: &UiState) -> String {
    match &ui.profile {
        Some(p) => p.display().to_string(),
        None => "default (auto)".into(),
    }
}

fn run_start(ui: &UiState) -> Result<(), String> {
    if ui.snapshot.exists() {
        return Err("session already active; end it first".into());
    }
    let (profile, source) = load_session_profile(ui.profile.as_deref())?;
    info!(%source, "profile loaded for tray session");
    warn_heavy_vram_users(&profile);
    let mode = SessionMode {
        focus: Some(ui.focus),
        boost: Some(ui.boost),
    };
    let reclaim_mode =
        effective_reclaim_mode(profile.gpu.vram_reclaim.policy, false, true);
    let candidates = collect_reclaim_candidates(&profile);
    let reclaimed = soft_stop_targets(
        &candidates,
        reclaim_mode,
        prompt_tray_reclaim,
        &SystemSupervisor,
    );
    let controller = platform_controller();
    let boosters = platform_boosters();
    let booster_refs: Vec<&dyn gamefocus_core::BoostProvider> =
        boosters.iter().map(|b| b.as_ref()).collect();
    let mut snap = start_session(&profile, mode, controller.as_ref(), &booster_refs)
        .map_err(|e| e.to_string())?;
    snap.reclaimed = reclaimed;
    if let Some(parent) = ui.snapshot.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(
        &ui.snapshot,
        serde_json::to_string_pretty(&snap).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    info!(
        path = %ui.snapshot.display(),
        suspended = snap.suspended.len(),
        reclaimed = snap.reclaimed.len(),
        "session started from UI"
    );
    Ok(())
}

fn prompt_tray_reclaim(target: &ReclaimTarget) -> bool {
    let result = MessageDialog::new()
        .set_level(MessageLevel::Warning)
        .set_title("High GPU memory in use")
        .set_description(format!(
            "{} is holding VRAM. Suspending it will not free GPU memory.\n\nStop it for this session and restore when you stop gaming?",
            target.label()
        ))
        .set_buttons(MessageButtons::YesNo)
        .show();
    matches!(result, MessageDialogResult::Yes)
}

fn run_end(snapshot: &PathBuf) -> Result<(), String> {
    if !snapshot.exists() {
        return Err("no active session".into());
    }
    let text = fs::read_to_string(snapshot).map_err(|e| e.to_string())?;
    let snap: SessionSnapshot = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let controller = platform_controller();
    let boosters = platform_boosters();
    let booster_refs: Vec<&dyn gamefocus_core::BoostProvider> =
        boosters.iter().map(|b| b.as_ref()).collect();
    end_session(&snap, controller.as_ref(), &booster_refs).map_err(|e| e.to_string())?;
    if !snap.reclaimed.is_empty() {
        restore_reclaimed(&snap.reclaimed, &SystemSupervisor);
        info!(count = snap.reclaimed.len(), "vram reclaim restored from UI");
    }
    let _ = fs::remove_file(snapshot);
    info!("session ended from UI");
    Ok(())
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let focus_item = CheckMenuItem::with_id(app, "focus", "Focus", true, true, None::<&str>)?;
    let boost_enabled = !cfg!(target_os = "macos");
    let boost_item = CheckMenuItem::with_id(
        app,
        "boost",
        if boost_enabled {
            "Boost"
        } else {
            "Boost (not on macOS)"
        },
        boost_enabled,
        boost_enabled,
        None::<&str>,
    )?;
    let start_item = MenuItem::with_id(app, "start", "Start gaming", true, None::<&str>)?;
    let end_item = MenuItem::with_id(app, "end", "Stop gaming", true, None::<&str>)?;
    let open_item = MenuItem::with_id(app, "open", "Open controls…", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;

    let menu = Menu::with_items(
        app,
        &[
            &open_item,
            &sep,
            &focus_item,
            &boost_item,
            &sep,
            &start_item,
            &end_item,
            &sep,
            &quit_item,
        ],
    )?;

    let icon = app
        .default_window_icon()
        .cloned()
        .expect("bundled window icon required for tray");
    let _tray = TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("gamefocus")
        .on_menu_event(|app, event| {
            let state = app.state::<AppState>();
            match event.id.as_ref() {
                "open" => show_main_window(app),
                "focus" => {
                    if let Ok(mut ui) = state.inner.lock() {
                        ui.focus = !ui.focus;
                    }
                }
                "boost" => {
                    if let Ok(mut ui) = state.inner.lock() {
                        ui.boost = !ui.boost;
                    }
                }
                "start" => {
                    let ui = match state.inner.lock() {
                        Ok(g) => g.clone(),
                        Err(e) => {
                            warn!("lock poisoned: {e}");
                            return;
                        }
                    };
                    if let Err(e) = run_start(&ui) {
                        warn!("start failed: {e}");
                    }
                }
                "end" => {
                    let snapshot = match state.inner.lock() {
                        Ok(g) => g.snapshot.clone(),
                        Err(e) => {
                            warn!("lock poisoned: {e}");
                            return;
                        }
                    };
                    if let Err(e) = run_end(&snapshot) {
                        warn!("end failed: {e}");
                    }
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            inner: Mutex::new(UiState::default()),
        })
        .invoke_handler(tauri::generate_handler![
            list_profiles,
            get_status,
            set_toggles,
            set_profile,
            start_focus_session,
            end_focus_session
        ])
        .setup(|app| {
            build_tray(app.handle())?;
            if let Some(window) = app.get_webview_window("main") {
                let handle = app.handle().clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        if let Some(w) = handle.get_webview_window("main") {
                            let _ = w.hide();
                        }
                    }
                });
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running gamefocus UI");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_inactive_when_no_snapshot() {
        let ui = UiState {
            profile: None,
            focus: true,
            boost: false,
            snapshot: PathBuf::from("/tmp/gamefocus-ui-test-missing-snapshot.json"),
        };
        let s = status_from_ui(&ui);
        assert!(!s.active);
        assert!(s.focus);
        assert!(!s.boost);
        assert_eq!(s.suspended, 0);
    }
}
