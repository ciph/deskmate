#[cfg(windows)]
mod windows_process_tree;
#[cfg(windows)]
mod windows_startup;
// allow: SIZE_OK — legacy Tauri bootstrap root owns startup/resource wiring; this patch keeps the migration at that boundary.
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Condvar, Mutex, OnceLock, RwLock, RwLockReadGuard,
};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use tauri::{Emitter, Manager, RunEvent, State};

mod sidecar_supervisor;
mod sidecar_logs;
mod agent;
mod ai_usage;
#[cfg(feature = "legacy-ai-recovery")]
pub mod legacy_ai_recovery;
mod retired_tools;
mod chat_attachments;
mod chat_links;
mod component_activation;
mod windows_component;
mod file_picker;
mod history;
mod history_entry;
/// Local memory: storage, policy, retrieval, and the frontend command surface.
mod memory;
/// User-installable persona packs imported from local `.dmpack` archives.
mod packs;
mod pet_geometry;
mod pet_input;
mod pet_placement;
mod pet_recovery;
mod pet_startup;
mod pet_visibility;
mod pet_visibility_recovery;
mod pet_visibility_state;
mod pomodoro;
#[cfg(any(feature = "worklog-qa", test))]
mod qa_identity;
mod settings;
mod settings_window;
mod startup_settings;
mod tool_permissions;
mod updater;
mod window_layout;
mod workbench;
mod workbench_theme;
mod worklog;
mod yume_context;
use ai_usage::{fetch_ai_usage, record_ai_usage};
use chat_attachments::AttachmentStore;
use history::HistoryState;
use settings::{get_settings, set_settings, verify_api_key, SettingsState};

const RESOURCE_ERROR_EVENT: &str = "deskmate://resource-error";

/// Sidecar state: the spawned `opencode serve` process and its base URL.
pub(crate) struct Sidecar {
    child: Mutex<Option<Child>>,
    generation: RwLock<u64>,
    logs: Mutex<Option<sidecar_logs::SidecarLogs>>,
    pub(crate) port: u16,
    /// Per-launch random password the sidecar requires (Basic auth). Generated
    /// fresh each start, never persisted; shared only with YUME-owned windows
    /// and the host's own clients at request time.
    pub(crate) password: String,
}

impl Sidecar {
    pub(crate) fn generation_guard(&self, expected: u64) -> Result<Option<RwLockReadGuard<'_, u64>>, String> {
        let generation = self.generation.read().map_err(|_| "sidecar generation poisoned")?;
        Ok(if *generation == expected { Some(generation) } else { None })
    }
}

pub(crate) fn sidecar_generation(app: &tauri::AppHandle) -> Result<u64, String> {
    app.state::<Sidecar>().generation.read().map(|generation| *generation)
        .map_err(|_| "sidecar generation poisoned".to_owned())
}

/// Every publication/revocation invalidates results read from the previous engine.
fn mutate_sidecar_child<T>(app: &tauri::AppHandle, action: impl FnOnce(&mut Option<Child>) -> Result<T, String>) -> Result<T, String> {
    let sidecar = app.state::<Sidecar>();
    let mut generation = sidecar.generation.write().map_err(|_| "sidecar generation poisoned")?;
    let mut child = sidecar.child.lock().map_err(|_| "sidecar lock poisoned")?;
    let before = child.as_ref().map(Child::id);
    let result = action(&mut child);
    if child.as_ref().map(Child::id) != before { *generation = generation.wrapping_add(1); }
    result
}

#[derive(Default)]
struct SidecarSupervisor {
    stopping: AtomicBool,
    recovery: Mutex<()>,
    policy: Mutex<sidecar_supervisor::RetryPolicy>,
    wake_revision: Mutex<u64>,
    wake: Condvar,
}

impl SidecarSupervisor {
    fn notify(&self) {
        if let Ok(mut revision) = self.wake_revision.lock() {
            *revision = revision.wrapping_add(1);
            self.wake.notify_all();
        }
    }

    fn wait(&self, timeout: Duration) {
        let Ok(revision) = self.wake_revision.lock() else { return; };
        let observed = *revision;
        let _wait = self.wake.wait_timeout_while(revision, timeout, |revision| {
            *revision == observed && !self.stopping.load(Ordering::SeqCst)
        });
    }
}

pub(crate) fn sidecar_stopping(app: &tauri::AppHandle) -> bool {
    app.state::<SidecarSupervisor>().stopping.load(Ordering::SeqCst)
}

/// Basic-auth header value for the current managed sidecar.
pub(crate) fn sidecar_auth_header(app: &tauri::AppHandle) -> String {
    use base64::Engine;
    let password = &app.state::<Sidecar>().password;
    format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(format!("opencode:{password}"))
    )
}

/// Process-launch sidecar auth value for call sites that only carry a URL
/// (e.g. the tool-permission polling chain). Set once at startup; unset in
/// tests, where fixture servers do not require auth.
static SIDECAR_AUTH: OnceLock<String> = OnceLock::new();

pub(crate) fn sidecar_auth_value() -> Option<&'static str> {
    SIDECAR_AUTH.get().map(String::as_str)
}

struct ChatShown(Mutex<bool>);

#[derive(Default)]
struct ChatMotion {
    target: Mutex<Option<window_layout::Point>>,
    worker_running: AtomicBool,
}

#[tauri::command]
fn sidecar_base_url(sidecar: State<Sidecar>) -> String {
    format!("http://127.0.0.1:{}", sidecar.port)
}

/// Credentials for the managed sidecar, delivered in memory to YUME's own
/// chat/settings/workbench windows only. Never logged, never persisted.
fn can_access_sidecar_credentials(window_label: &str) -> bool {
    matches!(window_label, "chat" | "settings" | "workbench")
}

#[tauri::command]
fn sidecar_auth(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<serde_json::Value, String> {
    if !can_access_sidecar_credentials(window.label()) {
        return Err("sidecar credentials are only available to YUME windows".to_string());
    }
    let password = &app.state::<Sidecar>().password;
    Ok(serde_json::json!({
        "username": "opencode",
        "password": password,
    }))
}

pub(crate) fn sidecar_url(app: &tauri::AppHandle) -> String {
    format!("http://127.0.0.1:{}", app.state::<Sidecar>().port)
}

pub(crate) fn sidecar_owned_running(app: &tauri::AppHandle) -> Result<bool, String> {
    let sidecar = app.state::<Sidecar>();
    let mut child = sidecar.child.lock().map_err(|_| "sidecar lock poisoned")?;
    match child.as_mut() {
        Some(process) => process
            .try_wait()
            .map(|status| status.is_none())
            .map_err(|error| error.to_string()),
        None => Ok(false),
    }
}

/// Load the persona system prompt + placeholders, preferring an installed pack.
#[tauri::command]
fn load_persona(app: tauri::AppHandle, id: String) -> Result<serde_json::Value, String> {
    let (persona, placeholders_raw, skills) = packs::persona_files(&app, &id)?;
    let placeholders = placeholders_raw
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .unwrap_or(serde_json::Value::Null);
    // Skills come from the owning pack's manifest, so a new pack can grant
    // abilities without changing this command.
    let skills = (!skills.is_empty()).then(|| skills.join("\n\n"));
    Ok(serde_json::json!({
        "persona": persona,
        "placeholders": placeholders,
        "skills": skills,
    }))
}

#[tauri::command]
fn toggle_chat(app: tauri::AppHandle) -> Result<bool, String> {
    toggle_chat_impl(&app)
}

pub(crate) fn toggle_chat_impl(app: &tauri::AppHandle) -> Result<bool, String> {
    let chat = app
        .get_webview_window("chat")
        .ok_or("chat window missing")?;
    let shown = app.state::<ChatShown>();

    let explicitly_shown = *shown
        .0
        .lock()
        .map_err(|_| "chat state poisoned".to_string())?;
    let window_visible = chat.is_visible().map_err(|e| e.to_string())?;
    if should_hide_chat(explicitly_shown, window_visible) {
        hide_chat_impl(app)?;
        return Ok(false);
    }
    show_chat(app)?;
    Ok(true)
}

fn should_hide_chat(explicitly_shown: bool, window_visible: bool) -> bool {
    explicitly_shown && window_visible
}

fn should_restore_settings_focus(chat_was_shown: bool, settings_visible: bool) -> bool {
    chat_was_shown && settings_visible
}

fn clear_chat_motion(app: &tauri::AppHandle) {
    if let Some(motion) = app.try_state::<Arc<ChatMotion>>() {
        if let Ok(mut target) = motion.target.lock() {
            *target = None;
        }
    }
}

#[tauri::command]
fn hide_chat(app: tauri::AppHandle) -> Result<(), String> {
    hide_chat_impl(&app)
}

#[tauri::command]
fn show_chat_window(app: tauri::AppHandle) -> Result<(), String> {
    show_chat(&app)
}

#[tauri::command]
fn preview_pet_scale(app: tauri::AppHandle, scale: f64) {
    if let Some(pet) = app.get_webview_window("pet") {
        settings::apply_pet_scale(&pet, scale);
    }
}

pub(crate) fn hide_chat_impl(app: &tauri::AppHandle) -> Result<(), String> {
    let chat = app
        .get_webview_window("chat")
        .ok_or("chat window missing")?;
    chat.hide().map_err(|e| e.to_string())?;
    clear_chat_motion(app);
    if let Some(shown) = app.try_state::<ChatShown>() {
        *shown
            .0
            .lock()
            .map_err(|_| "chat state poisoned".to_string())? = false;
    }
    Ok(())
}

#[cfg(test)]
mod worklog_tool_tests;

#[cfg(test)]
mod tests {
    use super::{
        can_access_sidecar_credentials, configure_sidecar_command, configure_sidecar_environment,
        migrate_legacy_xiaozhu_intro, overwrite_builtin_xiaozhu_persona,
        should_follow_chat_on_window_event,
        should_hide_chat, should_restore_settings_focus,
    };
    use std::ffi::OsStr;
    use std::path::Path;
    use std::process::Command;
    use tauri::{PhysicalPosition, WindowEvent};

    #[test]
    fn settings_can_authenticate_model_requests_without_exposing_credentials_to_other_windows() {
        // Settings fetches /config/providers after verifying an upstream API key.
        // Denying its credentials makes that successful verification appear to fail.
        for label in ["settings", "chat", "workbench"] {
            assert!(can_access_sidecar_credentials(label), "{label}");
        }
        for label in ["pet", "", "settings-external", "external", "Settings"] {
            assert!(!can_access_sidecar_credentials(label), "{label}");
        }
    }

    #[test]
    fn stale_shown_state_does_not_hide_an_already_hidden_chat_window() {
        assert!(!should_hide_chat(true, false));
        assert!(should_hide_chat(true, true));
        assert!(!should_hide_chat(false, true));
    }

    #[test]
    fn visible_settings_window_reclaims_focus_after_chat_is_shown() {
        assert!(should_restore_settings_focus(true, true));
        assert!(!should_restore_settings_focus(true, false));
        assert!(!should_restore_settings_focus(false, true));
    }

    #[test]
    fn moving_the_pet_repositions_the_visible_chat_window() {
        assert!(should_follow_chat_on_window_event(&WindowEvent::Moved(
            PhysicalPosition::new(1500, 700),
        )));
        assert!(!should_follow_chat_on_window_event(&WindowEvent::Focused(
            true,
        )));
    }

    #[test]
    fn yume_sidecar_disables_external_plugins() {
        let mut command = Command::new("opencode");
        configure_sidecar_command(&mut command, 47_891, Path::new("."));

        // Plugin loading stays enabled for the host-managed yume-context
        // plugin; isolation comes from the private HOME, not `--pure`.
        assert_eq!(command.get_args().next(), Some(OsStr::new("serve")));
        assert!(!command.get_args().any(|arg| arg == OsStr::new("--pure")));
    }

    #[test]
    fn yume_sidecar_environment_isolated_from_global_home_and_xdg_locations() {
        let root = std::env::temp_dir().join(format!("yume-sidecar-env-{}", uuid::Uuid::new_v4()));
        let data_dir = root.join("data");
        std::fs::create_dir_all(&data_dir).expect("create data directory");

        let mut command = Command::new("opencode");
        configure_sidecar_environment(&mut command, &data_dir);

        let sidecar_home = data_dir.join("opencode-home");
        let appdata = sidecar_home.join("AppData").join("Roaming");
        let localappdata = sidecar_home.join("AppData").join("Local");
        let xdg_config = sidecar_home.join("xdg-config");
        let xdg_data = sidecar_home.join("xdg-data");
        let xdg_cache = sidecar_home.join("xdg-cache");
        let envs = command.get_envs().collect::<Vec<_>>();

        assert!(envs.iter().any(|(key, value)| {
            *key == OsStr::new("HOME") && *value == Some(sidecar_home.as_os_str())
        }));
        assert!(envs.iter().any(|(key, value)| {
            *key == OsStr::new("USERPROFILE") && *value == Some(sidecar_home.as_os_str())
        }));
        assert!(envs.iter().any(|(key, value)| {
            *key == OsStr::new("APPDATA") && *value == Some(appdata.as_os_str())
        }));
        assert!(envs.iter().any(|(key, value)| {
            *key == OsStr::new("LOCALAPPDATA") && *value == Some(localappdata.as_os_str())
        }));
        assert!(envs.iter().any(|(key, value)| {
            *key == OsStr::new("XDG_CONFIG_HOME") && *value == Some(xdg_config.as_os_str())
        }));
        assert!(envs.iter().any(|(key, value)| {
            *key == OsStr::new("XDG_DATA_HOME") && *value == Some(xdg_data.as_os_str())
        }));
        assert!(envs.iter().any(|(key, value)| {
            *key == OsStr::new("XDG_CACHE_HOME") && *value == Some(xdg_cache.as_os_str())
        }));
        assert!(envs.iter().any(|(key, value)| {
            *key == OsStr::new("OPENCODE_CONFIG_CONTENT") && value.is_none()
        }));
        assert!(envs.iter().any(|(key, value)| {
            *key == OsStr::new("OPENCODE_AUTH_CONTENT") && value.is_none()
        }));
        assert!(envs.iter().any(|(key, value)| {
            *key == OsStr::new("OPENCODE_ENABLE_EXA") && *value == Some(OsStr::new("1"))
        }));
        assert!(envs.iter().any(|(key, value)| {
            *key == OsStr::new("OPENCODE_WEBSEARCH_PROVIDER") && *value == Some(OsStr::new("exa"))
        }));
        assert!(envs
            .iter()
            .any(|(key, value)| { *key == OsStr::new("EXA_API_KEY") && value.is_none() }));
        assert!(envs
            .iter()
            .any(|(key, value)| { *key == OsStr::new("PARALLEL_API_KEY") && value.is_none() }));
        assert!(envs.iter().any(|(key, value)| {
            *key == OsStr::new("OPENCODE_ENABLE_PARALLEL") && value.is_none()
        }));

        std::fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn migrates_only_the_legacy_xiaozhu_intro() {
        let legacy_mid =
            "你好！我是**著名**当代游戏电子游戏音乐先锋——霄太郎，当然\\~您叫我**小著**就行。";
        let legacy_runtime =
            "你好！我是著名当代游戏电子游戏音乐先锋——霄太郎，当然~您叫我小著就行。";
        let legacy = "你好，栋梁！我是小著，一名游戏配乐师，我会把游戏里的音乐布置到游戏中，让它们自然地流动起来，并随着游戏的状态有机地连续播放。";

        let prompt = format!("---\nid: xiaozhu\n---\n\n# 我是谁\n\n{legacy_mid}\n\n其他自定义内容");
        let migrated = migrate_legacy_xiaozhu_intro(&prompt);

        assert_eq!(
            migrated.as_deref(),
            Some("---\nid: xiaozhu\n---\n\n# 我是谁\n\n你好！我是当代游戏电子游戏音乐先锋——小著。\n\n其他自定义内容")
        );

        let prompt =
            format!("---\nid: xiaozhu\n---\n\n# 我是谁\n\n{legacy_runtime}\n\n其他自定义内容");
        let migrated = migrate_legacy_xiaozhu_intro(&prompt);

        assert_eq!(
            migrated.as_deref(),
            Some("---\nid: xiaozhu\n---\n\n# 我是谁\n\n你好！我是当代游戏电子游戏音乐先锋——小著。\n\n其他自定义内容")
        );

        let prompt = format!("---\nid: xiaozhu\n---\n\n# 我是谁\n\n{legacy}\n\n其他自定义内容");
        let migrated = migrate_legacy_xiaozhu_intro(&prompt);

        assert_eq!(
            migrated.as_deref(),
            Some("---\nid: xiaozhu\n---\n\n# 我是谁\n\n你好！我是当代游戏电子游戏音乐先锋——小著。\n\n其他自定义内容")
        );
        assert!(migrate_legacy_xiaozhu_intro("没有旧文案").is_none());
        assert!(migrate_legacy_xiaozhu_intro(&format!("# 其他内容\n\n{legacy}")).is_none());
    }

    #[test]
    fn overwrites_builtin_xiaozhu_persona_when_runtime_copy_is_stale() {
        let root = std::env::temp_dir().join(format!("yume-xiaozhu-sync-{}", uuid::Uuid::new_v4()));
        let shipped_personas = root.join("shipped-personas");
        let data_dir = root.join("data");
        let shipped_persona = shipped_personas.join("xiaozhu").join("persona.md");
        let runtime_persona = data_dir.join("personas").join("xiaozhu").join("persona.md");

        std::fs::create_dir_all(shipped_persona.parent().expect("shipped parent"))
            .expect("create shipped directory");
        std::fs::create_dir_all(runtime_persona.parent().expect("runtime parent"))
            .expect("create runtime directory");
        std::fs::write(&shipped_persona, "new built-in persona").expect("write shipped persona");
        std::fs::write(&runtime_persona, "stale runtime persona").expect("write stale persona");

        overwrite_builtin_xiaozhu_persona(&shipped_personas, &data_dir)
            .expect("sync built-in persona");

        assert_eq!(
            std::fs::read_to_string(&runtime_persona).expect("read synchronized persona"),
            "new built-in persona"
        );
        std::fs::remove_dir_all(root).expect("remove test directory");
    }

}

fn should_follow_chat_on_window_event(event: &tauri::WindowEvent) -> bool {
    matches!(event, tauri::WindowEvent::Moved(_))
}

fn chat_target(app: &tauri::AppHandle) -> Result<tauri::PhysicalPosition<i32>, String> {
    let pet = app.get_webview_window("pet").ok_or("pet window missing")?;
    let chat = app
        .get_webview_window("chat")
        .ok_or("chat window missing")?;

    let pet_pos = pet.outer_position().map_err(|e| e.to_string())?;
    let chat_scale = chat.scale_factor().map_err(|e| e.to_string())?;
    let chat_size = chat.outer_size().unwrap_or_else(|_| {
        tauri::PhysicalSize::new(
            (420.0 * chat_scale).round() as u32,
            (560.0 * chat_scale).round() as u32,
        )
    });
    let monitor = pet.current_monitor().ok().flatten();
    let (work_area, gap) = if let Some(monitor) = monitor {
        let area = monitor.work_area();
        (
            window_layout::Rect {
                x: i64::from(area.position.x),
                y: i64::from(area.position.y),
                width: i64::from(area.size.width),
                height: i64::from(area.size.height),
            },
            (8.0 * monitor.scale_factor()).round() as i64,
        )
    } else {
        (
            window_layout::Rect {
                x: i64::from(pet_pos.x) - 4096,
                y: i64::from(pet_pos.y) - 4096,
                width: 8192,
                height: 8192,
            },
            (8.0 * chat_scale).round() as i64,
        )
    };
    let point = window_layout::position_chat(
        pet_geometry::chat_bounds(&pet)?,
        window_layout::Size {
            width: i64::from(chat_size.width),
            height: i64::from(chat_size.height),
        },
        work_area,
        gap,
    );
    Ok(tauri::PhysicalPosition::new(
        point.x.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        point.y.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
    ))
}

fn reposition_chat(app: &tauri::AppHandle) -> Result<(), String> {
    let chat = app
        .get_webview_window("chat")
        .ok_or("chat window missing")?;
    chat.set_position(chat_target(app)?)
        .map_err(|e| e.to_string())
}

fn request_chat_reposition(app: &tauri::AppHandle) -> Result<(), String> {
    let chat = app
        .get_webview_window("chat")
        .ok_or("chat window missing")?;
    if !chat.is_visible().map_err(|e| e.to_string())? {
        return Ok(());
    }
    let target = chat_target(app)?;
    let motion = app.state::<Arc<ChatMotion>>().inner().clone();
    *motion
        .target
        .lock()
        .map_err(|_| "chat motion state poisoned".to_string())? = Some(window_layout::Point {
        x: i64::from(target.x),
        y: i64::from(target.y),
    });
    if !motion.worker_running.swap(true, Ordering::AcqRel) {
        spawn_chat_motion_worker(chat, motion);
    }
    Ok(())
}

fn spawn_chat_motion_worker(chat: tauri::WebviewWindow, motion: Arc<ChatMotion>) {
    std::thread::spawn(move || loop {
        let target = motion.target.lock().ok().and_then(|guard| *guard);
        let Some(target) = target else {
            motion.worker_running.store(false, Ordering::Release);
            if motion
                .target
                .lock()
                .map(|guard| guard.is_some())
                .unwrap_or(false)
                && !motion.worker_running.swap(true, Ordering::AcqRel)
            {
                continue;
            }
            break;
        };

        let current = chat
            .outer_position()
            .unwrap_or(tauri::PhysicalPosition::new(
                target.x.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
                target.y.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
            ));
        let next = window_layout::smooth_step(
            window_layout::Point {
                x: i64::from(current.x),
                y: i64::from(current.y),
            },
            target,
            0.28,
        );
        let next_position = tauri::PhysicalPosition::new(
            next.x.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
            next.y.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        );
        let _ = chat.set_position(next_position);
        if next == target {
            if let Ok(mut guard) = motion.target.lock() {
                if *guard == Some(target) {
                    *guard = None;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(8));
    });
}

fn reposition_visible_chat(app: &tauri::AppHandle) {
    let Some(chat) = app.get_webview_window("chat") else {
        return;
    };
    if chat.is_visible().unwrap_or(false) {
        let _ = request_chat_reposition(app);
    }
}

fn place_pet_bottom_right(pet: &tauri::WebviewWindow) {
    let (Ok(size), Ok(Some(monitor))) = (pet.outer_size(), pet.current_monitor()) else {
        return;
    };
    let area = monitor.work_area();
    let point = window_layout::position_pet_bottom_right(
        window_layout::Size {
            width: i64::from(size.width),
            height: i64::from(size.height),
        },
        window_layout::Rect {
            x: i64::from(area.position.x),
            y: i64::from(area.position.y),
            width: i64::from(area.size.width),
            height: i64::from(area.size.height),
        },
        16,
    );
    let _ = pet.set_position(tauri::PhysicalPosition::new(
        point.x.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        point.y.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
    ));
}

/// Anchor the chat window next to the pet and bring it up (idempotent).
pub(crate) fn show_chat(app: &tauri::AppHandle) -> Result<(), String> {
    let chat = app
        .get_webview_window("chat")
        .ok_or("chat window missing")?;
    let shown = app.state::<ChatShown>();

    clear_chat_motion(app);
    let large = app
        .state::<SettingsState>()
        .0
        .lock()
        .map(|s| s.chat_large)
        .map_err(|error| error.to_string())?;
    settings_window::apply_chat(app, large).map_err(|error| error.to_string())?;
    reposition_chat(app)?;
    chat.show().map_err(|e| e.to_string())?;
    chat.set_focus().map_err(|e| e.to_string())?;
    *shown
        .0
        .lock()
        .map_err(|_| "chat state poisoned".to_string())? = true;
    restore_settings_focus_if_visible(app);
    Ok(())
}

/// Resolve the opencode binary: bundled resources first, then platform-specific
/// package-manager locations, then PATH.
fn resolve_opencode(app: &tauri::AppHandle) -> PathBuf {
    let executable = if cfg!(windows) {
        "opencode.exe"
    } else {
        "opencode"
    };

    if let Ok(dir) = app.path().resource_dir() {
        let bundled = dir.join("resources").join("opencode").join(executable);
        if bundled.exists() {
            return bundled;
        }
    }

    // npm global install ships a real .exe next to the shim.
    if let Ok(appdata) = std::env::var("APPDATA") {
        let npm = PathBuf::from(appdata)
            .join("npm")
            .join("node_modules")
            .join("opencode-ai")
            .join("bin")
            .join("opencode.exe");
        if npm.exists() {
            return npm;
        }
    }

    // Apps opened from Finder do not inherit the interactive shell PATH, so
    // probe the standard Homebrew and user-level install locations on macOS.
    #[cfg(target_os = "macos")]
    {
        for path in [
            PathBuf::from("/opt/homebrew/bin/opencode"),
            PathBuf::from("/usr/local/bin/opencode"),
        ] {
            if path.exists() {
                return path;
            }
        }

        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            for relative in [
                ".opencode/bin/opencode",
                ".local/bin/opencode",
                ".bun/bin/opencode",
                "bin/opencode",
            ] {
                let path = home.join(relative);
                if path.exists() {
                    return path;
                }
            }
        }
    }

    PathBuf::from("opencode")
}

fn pick_free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        // SAFE-EXPECT: binding port 0 asks the OS for an available localhost port at startup.
        .expect("no free port available")
}

const CURRENT_XIAOZHU_INTRO: &str = "你好！我是当代游戏电子游戏音乐先锋——小著。";
const LEGACY_XIAOZHU_INTROS: &[&str] = &[
    "你好！我是**著名**当代游戏电子游戏音乐先锋——霄太郎，当然\\~您叫我**小著**就行。",
    "你好！我是著名当代游戏电子游戏音乐先锋——霄太郎，当然~您叫我小著就行。",
    "你好，栋梁！我是小著，一名游戏配乐师，我会把游戏里的音乐布置到游戏中，让它们自然地流动起来，并随着游戏的状态有机地连续播放。",
];

fn migrate_legacy_xiaozhu_intro(prompt: &str) -> Option<String> {
    let identity_start = prompt.find("# 我是谁")?;
    let identity_heading_len = "# 我是谁".len();
    let identity_end = prompt[identity_start + identity_heading_len..]
        .find("\n# ")
        .map(|offset| identity_start + identity_heading_len + offset)
        .unwrap_or(prompt.len());
    let identity_section = &prompt[identity_start..identity_end];
    let updated_section = LEGACY_XIAOZHU_INTROS
        .iter()
        .fold(identity_section.to_owned(), |current, legacy| {
            current.replace(legacy, CURRENT_XIAOZHU_INTRO)
        });
    if updated_section == identity_section {
        return None;
    }
    let mut updated = String::with_capacity(prompt.len() + CURRENT_XIAOZHU_INTRO.len());
    updated.push_str(&prompt[..identity_start]);
    updated.push_str(&updated_section);
    updated.push_str(&prompt[identity_end..]);
    Some(updated)
}

/// Copy ship resources (personas) into the app data dir on first run,
/// so users can edit them without touching the install dir.
/// Returns a human-readable reason for the first failure, if any.
fn sync_ship_resources(app: &tauri::AppHandle, data_dir: &Path) -> Result<(), String> {
    let res_dir = app
        .path()
        .resource_dir()
        .map_err(|e| format!("无法定位安装资源目录: {e}"))?;
    let mut shipped_personas_dir = None;
    for name in ["personas", "skills"] {
        let candidates = [res_dir.join("resources").join(name), res_dir.join(name)];
        let dst = data_dir.join(name);
        for src in candidates {
            if src.exists() {
                copy_missing_dir_recursive(&src, &dst)
                    .map_err(|e| format!("无法复制 {name} 资源到 {}: {e}", dst.display()))?;
                if name == "personas" {
                    shipped_personas_dir = Some(src);
                }
                break;
            }
        }
    }

    if let Some(shipped_personas_dir) = shipped_personas_dir {
        overwrite_builtin_xiaozhu_persona(&shipped_personas_dir, data_dir)
            .map_err(|error| format!("无法覆盖小著内置人设: {error}"))?;
    }

    let xiaozhu_persona = data_dir.join("personas").join("xiaozhu").join("persona.md");
    if let Ok(prompt) = std::fs::read_to_string(&xiaozhu_persona) {
        if let Some(updated) = migrate_legacy_xiaozhu_intro(&prompt) {
            std::fs::write(&xiaozhu_persona, updated).map_err(|error| {
                format!(
                    "无法更新小著默认人设 {}: {error}",
                    xiaozhu_persona.display()
                )
            })?;
        }
    }

    let shipped_opencode_tools = [
        res_dir.join("resources").join("opencode-tools"),
        res_dir.join("opencode-tools"),
    ]
    .into_iter()
    .find(|path| path.is_dir())
    .ok_or_else(|| "找不到内置 OpenCode 工具资源".to_string())?;
    overwrite_worklog_tools(&shipped_opencode_tools, data_dir)
        .map_err(|error| format!("无法刷新工作记录工具: {error}"))?;

    Ok(())
}

fn overwrite_builtin_xiaozhu_persona(
    shipped_personas_dir: &Path,
    data_dir: &Path,
) -> std::io::Result<()> {
    let source = shipped_personas_dir.join("xiaozhu").join("persona.md");
    let target_dir = data_dir.join("personas").join("xiaozhu");
    std::fs::create_dir_all(&target_dir)?;
    std::fs::copy(source, target_dir.join("persona.md"))?;
    Ok(())
}

fn overwrite_worklog_tools(shipped_tools_dir: &Path, data_dir: &Path) -> std::io::Result<()> {
    const FILES: &[(&str, &[u8])] = &[
        (
            "worklog_record.ts",
            include_bytes!("../resources/opencode-tools/worklog_record.ts"),
        ),
        (
            "memory_manage.ts",
            include_bytes!("../resources/opencode-tools/memory_manage.ts"),
        ),
        (
            "worklog_query.ts",
            include_bytes!("../resources/opencode-tools/worklog_query.ts"),
        ),
        (
            "worklog_update.ts",
            include_bytes!("../resources/opencode-tools/worklog_update.ts"),
        ),
        (
            "worklog_generate_report.ts",
            include_bytes!("../resources/opencode-tools/worklog_generate_report.ts"),
        ),
        (
            "worklog_schedule_report.ts",
            include_bytes!("../resources/opencode-tools/worklog_schedule_report.ts"),
        ),
        (
            "../worklog-bridge.ts",
            include_bytes!("../resources/worklog-bridge.ts"),
        ),
    ];
    let config_dir = data_dir.join("workspace").join(".opencode");
    let global_config = data_dir
        .join("opencode-home")
        .join("xdg-config")
        .join("opencode");
    let config_dirs = [&config_dir, &global_config];
    let validated = FILES
        .iter()
        .map(|(name, expected)| {
            let source = std::fs::read(shipped_tools_dir.join(name))?;
            if Sha256::digest(&source) != Sha256::digest(expected) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Work journal tool integrity check failed",
                ));
            }
            Ok((*name, source))
        })
        .collect::<std::io::Result<Vec<_>>>();
    let validated = match validated {
        Ok(files) => files,
        Err(error) => {
            for config in config_dirs {
                for (name, _) in FILES {
                    let target = if *name == "../worklog-bridge.ts" {
                        config.join("worklog-bridge.ts")
                    } else {
                        config.join("tools").join(name)
                    };
                    if target.is_file() {
                        std::fs::remove_file(target)?;
                    }
                }
            }
            return Err(error);
        }
    };
    for config in config_dirs {
        let target_dir = config.join("tools");
        std::fs::create_dir_all(&target_dir)?;
        for (name, source) in &validated {
            let target = if *name == "../worklog-bridge.ts" {
                config.join("worklog-bridge.ts")
            } else {
                target_dir.join(name)
            };
            std::fs::write(target, source)?;
        }
    }
    Ok(())
}

fn copy_missing_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        if target.exists() {
            continue;
        }
        if entry.file_type()?.is_dir() {
            copy_missing_dir_recursive(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// Apply the non-user-specific OpenCode launch settings for YUME's sidecar.
///
/// Plugin loading stays enabled (no `--pure`) so the host-managed
/// `yume-context` plugin can inject persona/memory on the native
/// `experimental.chat.system.transform` hook; the isolated HOME keeps foreign
/// global plugins out, and the plugin allowlist is exactly what
/// OPENCODE_CONFIG_CONTENT declares.
fn configure_sidecar_command(cmd: &mut Command, port: u16, workspace: &Path) {
    cmd.arg("serve")
        .arg("--port")
        .arg(port.to_string())
        .arg("--hostname")
        .arg("127.0.0.1")
        // Production WebView origin on Windows is http://tauri.localhost;
        // localhost dev origins are allowed by default.
        .arg("--cors")
        .arg("http://tauri.localhost")
        .arg("--cors")
        .arg("tauri://localhost")
        .arg("--print-logs")
        .current_dir(workspace);
}

fn configure_sidecar_environment(cmd: &mut Command, data_dir: &Path) {
    let sidecar_home = data_dir.join("opencode-home");
    let appdata = sidecar_home.join("AppData").join("Roaming");
    let localappdata = sidecar_home.join("AppData").join("Local");

    cmd.env("HOME", &sidecar_home)
        .env("USERPROFILE", &sidecar_home)
        .env("APPDATA", &appdata)
        .env("LOCALAPPDATA", &localappdata)
        .env("XDG_CONFIG_HOME", sidecar_home.join("xdg-config"))
        .env("XDG_DATA_HOME", sidecar_home.join("xdg-data"))
        .env("XDG_CACHE_HOME", sidecar_home.join("xdg-cache"))
        .env("OPENCODE_ENABLE_EXA", "1")
        .env("OPENCODE_WEBSEARCH_PROVIDER", "exa")
        .env_remove("OPENCODE_CONFIG_CONTENT")
        .env_remove("OPENCODE_AUTH_CONTENT")
        .env_remove("EXA_API_KEY")
        .env_remove("PARALLEL_API_KEY")
        .env_remove("OPENCODE_ENABLE_PARALLEL");
}

fn resource_error_event(reason: String) -> (&'static str, String) {
    (RESOURCE_ERROR_EVENT, reason)
}

fn spawn_sidecar(app: &tauri::AppHandle, port: u16) -> std::io::Result<Child> {
    // SAFE-EXPECT: Tauri provides an app data directory after app setup.
    let data_dir = app.path().app_data_dir().expect("app data dir unavailable");
    std::fs::create_dir_all(&data_dir)?;
    // Fail closed if a retired tool cannot be removed from the discovery path.
    retired_tools::retire_external_deploy_tool(&data_dir)?;
    // A resource-copy failure means personas/skills will be missing, which
    // otherwise shows up as an unexplained empty persona list. Report it.
    if let Err(reason) = sync_ship_resources(app, &data_dir) {
        eprintln!("ship resource sync failed: {reason}");
        let (event, payload) = resource_error_event(reason);
        let _ = app.emit(event, &payload);
    }

    // Sessions live in a dedicated workspace dir under app data.
    let workspace = data_dir.join("workspace");
    std::fs::create_dir_all(&workspace)?;

    let bin = resolve_opencode(app);
    let mut cmd = Command::new(&bin);
    #[cfg(windows)]
    {
        let tools = bin
            .parent()
            .ok_or_else(|| std::io::Error::other("OpenCode resource directory unavailable"))?
            .join("process-tools");
        if !tools.join("taskkill.exe").is_file() {
            return Err(std::io::Error::other(
                "YUME process terminator missing; run prepare:sidecar",
            ));
        }
        let inherited = std::env::var_os("PATH").unwrap_or_default();
        let path =
            std::env::join_paths(std::iter::once(tools).chain(std::env::split_paths(&inherited)))
                .map_err(std::io::Error::other)?;
        cmd.env("PATH", path)
            .env("NoDefaultCurrentDirectoryInExePath", "1");
    }
    configure_sidecar_command(&mut cmd, port, &workspace);
    configure_sidecar_environment(&mut cmd, &data_dir);
    cmd.env_remove("YUME_WORKLOG_IPC_DIR");
    if let Some(bridge) = app.try_state::<worklog::bridge::WorklogBridge>() {
        match bridge.reset_launch(format!("http://127.0.0.1:{port}"), crate::sidecar_auth_header(app)) {
            Ok(directory) => {
                cmd.env("YUME_WORKLOG_IPC_DIR", directory);
            }
            Err(error) => eprintln!("work journal bridge unavailable: {}", error.code),
        }
    }
    let password = app.state::<Sidecar>().password.clone();
    let mut log_secrets = vec![password.clone(), sidecar_auth_header(app)];
    cmd.env("OPENCODE_SERVER_PASSWORD", password);
    // Username stays the server default ("opencode"); sidecar_auth_header matches it.

    // Deploy the host-managed context plugin and its data file before launch;
    // the plugin reads the file per request, so later persona/memory updates
    // only need a context rewrite, not a restart.
    if let Err(error) = yume_context::ensure_plugin(app) {
        eprintln!("yume context plugin deploy failed: {error}");
    }
    if let Err(error) = yume_context::write_context(app) {
        eprintln!("yume context write failed: {error}");
    }
    if let Ok(context_path) = yume_context::context_file_path(app) {
        cmd.env("YUME_CONTEXT_FILE", context_path);
    }

    let (config, auth) = match settings::sidecar_environment(app) {
        Some((config, auth)) => (config, Some(auth)),
        None => (
            serde_json::json!({
                "permission": settings::sidecar_permission_policy(),
                "experimental": { "continue_loop_on_deny": true }
            })
            .to_string(),
            None,
        ),
    };
    let config = yume_context::with_plugin(config, app);
    sidecar_logs::collect_json_secrets(&config, &mut log_secrets);
    cmd.env("OPENCODE_CONFIG_CONTENT", config);
    if let Some(auth) = auth {
        sidecar_logs::collect_json_secrets(&auth, &mut log_secrets);
        cmd.env("OPENCODE_AUTH_CONTENT", auth);
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let logs = {
        let sidecar = app.state::<Sidecar>();
        let mut logs = sidecar.logs.lock().map_err(|_| std::io::Error::other("sidecar log state poisoned"))?;
        if logs.is_none() { *logs = Some(sidecar_logs::SidecarLogs::open(data_dir.join("sidecar.log"), &log_secrets)?); }
        logs.as_ref().expect("log sink just initialized").clone()
    };
    sidecar_logs::configure(&mut cmd);
    let mut child = cmd.spawn()?;
    #[cfg(windows)]
    if let Err(error) = windows_startup::record_child(&app.config().identifier, &data_dir, child.id()) {
        // The owned Child handle remains exact even if registration failed.
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    if let Err(error) = logs.attach(&mut child, log_secrets) {
        stop_sidecar_child(app, child);
        return Err(error);
    }
    Ok(child)
}

fn forget_sidecar_child(app: &tauri::AppHandle, pid: u32) {
    #[cfg(windows)]
    if let Ok(data_dir) = app.path().app_data_dir() {
        windows_startup::forget_child(&data_dir, pid);
    }
    #[cfg(not(windows))]
    let _ = (app, pid);
}

fn stop_sidecar_child(app: &tauri::AppHandle, mut child: Child) {
    let _ = child.kill();
    if child.wait().is_ok() {
        forget_sidecar_child(app, child.id());
    }
}

pub(crate) fn restart_sidecar(app: &tauri::AppHandle) -> Result<(), String> {
    let supervisor = app.state::<SidecarSupervisor>();
    let _recovery = supervisor.recovery.lock().map_err(|_| "sidecar recovery lock poisoned")?;
    if supervisor.stopping.load(Ordering::SeqCst) { return Err("sidecar_stopping".into()); }
    if app.try_state::<agent::AgentRunState>().is_some() {
        agent::interrupt_owned_run(app, "sidecar_restarted");
    }
    supervisor.policy.lock().map_err(|_| "sidecar policy poisoned")?.manual_retry();
    let result = replace_sidecar(app);
    supervisor.notify();
    result
}

/// Isolate only the engine observed by the failed task, then join automatic backoff.
/// Isolation and scheduling share recovery, so an old incident cannot revoke a replacement.
pub(crate) fn isolate_sidecar_for_recovery(app: &tauri::AppHandle, expected: u64, isolate: impl FnOnce(&mut Option<Child>) -> Result<(), String>) -> Result<(), String> {
    let supervisor = app.state::<SidecarSupervisor>();
    let _recovery = supervisor.recovery.lock().map_err(|_| "sidecar recovery lock poisoned")?;
    if supervisor.stopping.load(Ordering::SeqCst) { return Err("sidecar_stopping".into()); }
    if sidecar_generation(app)? != expected { return Ok(()); }
    mutate_sidecar_child(app, isolate)?;
    supervisor.policy.lock().map_err(|_| "sidecar policy poisoned")?.record_failure(Instant::now());
    supervisor.notify();
    Ok(())
}

// Caller holds the component task-admission boundary and has checked all work.
pub(crate) fn restart_idle_sidecar(app: &tauri::AppHandle) -> Result<(), String> {
    let supervisor = app.state::<SidecarSupervisor>();
    let generation = {
        let _recovery = supervisor.recovery.lock().map_err(|_| "sidecar recovery lock poisoned")?;
        if supervisor.stopping.load(Ordering::SeqCst) { return Err("sidecar_stopping".into()); }
        supervisor.policy.lock().map_err(|_| "sidecar policy poisoned")?.manual_retry();
        replace_sidecar(app)?;
        sidecar_generation(app)?
    };
    supervisor.notify();
    // Readiness waits outside recovery, while the caller retains task admission.
    let deadline = Instant::now() + sidecar_supervisor::STARTUP_TIMEOUT;
    while Instant::now() < deadline {
        if supervisor.stopping.load(Ordering::SeqCst) || sidecar_generation(app)? != generation {
            return Err("component_service_replaced".into());
        }
        if sidecar_supervisor::probe_current_generation(generation, || sidecar_healthy(app), || {
            let _recovery = supervisor.recovery.lock().map_err(|_| "sidecar recovery lock poisoned")?;
            if supervisor.stopping.load(Ordering::SeqCst) || !sidecar_owned_running(app)? {
                return Err("component_service_replaced".into());
            }
            sidecar_generation(app)
        })? { return Ok(()); }
        supervisor.wait(Duration::from_millis(200));
    }
    Err("component_service_not_ready".into())
}

fn replace_sidecar(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(child) = mutate_sidecar_child(app, |slot| Ok(slot.take()))? {
        stop_sidecar_child(app, child);
    }
    spawn_supervised_sidecar(app)
}

fn record_sidecar_diagnostic(app: &tauri::AppHandle, line: &str) {
    let sidecar = app.state::<Sidecar>();
    let mut secrets = vec![sidecar.password.clone(), sidecar_auth_header(app)];
    if let Some((config, auth)) = settings::sidecar_environment(app) {
        sidecar_logs::collect_json_secrets(&config, &mut secrets);
        sidecar_logs::collect_json_secrets(&auth, &mut secrets);
    }
    let result = (|| -> std::io::Result<()> {
        let mut logs = sidecar.logs.lock().map_err(|_| std::io::Error::other("sidecar log state poisoned"))?;
        if logs.is_none() {
            let data_dir = app.path().app_data_dir().map_err(std::io::Error::other)?;
            std::fs::create_dir_all(&data_dir)?;
            *logs = Some(sidecar_logs::SidecarLogs::open(data_dir.join("sidecar.log"), &secrets)?);
        }
        logs.as_ref().expect("log sink just initialized").record(line, &secrets)
    })();
    if result.is_err() { eprintln!("sidecar diagnostic capture unavailable"); }
}

fn spawn_supervised_sidecar(app: &tauri::AppHandle) -> Result<(), String> {
    let result = spawn_sidecar(app, app.state::<Sidecar>().port);
    let supervisor = app.state::<SidecarSupervisor>();
    let mut policy = supervisor.policy.lock().map_err(|_| "sidecar policy poisoned")?;
    match result {
        Ok(child) => {
            mutate_sidecar_child(app, |slot| { *slot = Some(child); Ok(()) })?;
            policy.record_spawn(Instant::now());
            Ok(())
        }
        Err(error) => {
            policy.record_failure(Instant::now());
            record_sidecar_diagnostic(app, &format!("spawn_failed kind={:?} os_code={:?} automatic_attempts={} exhausted={}", error.kind(), error.raw_os_error(), policy.retries_started(), policy.exhausted()));
            Err("sidecar_spawn_failed".into())
        }
    }
}

fn sidecar_healthy(app: &tauri::AppHandle) -> bool {
    ureq::get(&format!("{}/global/health", sidecar_url(app)))
        .set("Authorization", &sidecar_auth_header(app))
        .timeout(Duration::from_millis(500)).call().is_ok()
}

fn record_sidecar_failure(app: &tauri::AppHandle) {
    let exhausted = app.state::<SidecarSupervisor>().policy.lock().map(|mut policy| {
        policy.record_failure(Instant::now());
        policy.exhausted()
    }).unwrap_or(false);
    if exhausted { record_sidecar_diagnostic(app, "automatic_retry_budget_exhausted"); }
}

fn sidecar_failed(app: &tauri::AppHandle) {
    if app.try_state::<agent::AgentRunState>().is_some() {
        agent::interrupt_owned_run(app, "sidecar_crashed");
    }
    workbench::release_ownership(app);
    let _ = app.emit("sidecar://restarting", ());
}

fn start_sidecar_supervisor(app: tauri::AppHandle) {
    std::thread::spawn(move || {
      let mut next_health_probe = Instant::now();
      let mut health_generation = None;
      let mut observed_wake = 0;
      loop {
        let supervisor = app.state::<SidecarSupervisor>();
        supervisor.wait(sidecar_supervisor::POLL_INTERVAL);
        if let Ok(revision) = supervisor.wake_revision.lock() {
            if *revision != observed_wake {
                observed_wake = *revision;
                next_health_probe = Instant::now();
            }
        }
        if supervisor.stopping.load(Ordering::SeqCst) { break; }
        let generation = {
            let Ok(_recovery) = supervisor.recovery.lock() else { break; };
            if supervisor.stopping.load(Ordering::SeqCst) { break; }
            // Keep uncertain Child handles: a failed status read never authorizes a duplicate engine.
            let observed = mutate_sidecar_child(&app, |slot| {
                match slot.as_mut() {
                    Some(child) => match child.try_wait() {
                        Ok(Some(status)) => {
                            record_sidecar_diagnostic(&app, &format!("process_exited code={:?}", status.code()));
                            forget_sidecar_child(&app, child.id());
                            *slot = None;
                            Ok((false, true))
                        }
                        Ok(None) => Ok((true, false)),
                        Err(_) => Err("sidecar process status unavailable".into()),
                    },
                    None => Ok((false, false)),
                }
            });
            let Ok((running, crashed)) = observed else { continue; };
            if crashed {
                record_sidecar_failure(&app);
                sidecar_failed(&app);
            }
            if !running {
                let retry = supervisor.policy.lock().map(|mut policy| policy.take_retry(Instant::now())).unwrap_or(false);
                if retry { let _ = spawn_supervised_sidecar(&app); }
                continue;
            }
            if !supervisor.policy.lock().map(|policy| policy.needs_health_probe()).unwrap_or(false) { continue; }
            let Ok(generation) = sidecar_generation(&app) else { break; };
            if health_generation == Some(generation) && Instant::now() < next_health_probe { continue; }
            health_generation = Some(generation);
            generation
        };
        // HTTP and all backoff waits release recovery, so manual retry and shutdown remain available.
        let healthy = sidecar_healthy(&app);
        let Ok(_recovery) = supervisor.recovery.lock() else { break; };
        if supervisor.stopping.load(Ordering::SeqCst) { break; }
        if sidecar_generation(&app).ok() != Some(generation) { continue; }
        let observation = match supervisor.policy.lock() {
            Ok(mut policy) => {
                let observation = policy.observe_health(Instant::now(), healthy);
                next_health_probe = Instant::now() + policy.health_probe_interval();
                observation
            },
            Err(_) => break,
        };
        match observation {
            sidecar_supervisor::HealthObservation::TimedOut => {
                record_sidecar_diagnostic(&app, "health_timeout");
                if let Ok(Some(child)) = mutate_sidecar_child(&app, |slot| Ok(slot.take())) { stop_sidecar_child(&app, child); }
                record_sidecar_failure(&app);
                sidecar_failed(&app);
            }
            sidecar_supervisor::HealthObservation::Healthy { became_healthy, budget_reset } => {
                if became_healthy {
                    record_sidecar_diagnostic(&app, "health_ready");
                    history::commands::refresh_after_ready(app.clone());
                    let _ = app.emit("sidecar://restarted", ());
                }
                if budget_reset { record_sidecar_diagnostic(&app, "stable_health_retry_budget_reset"); }
            }
            _ => {}
        }
      }
    });
}

/// Build the system tray, including an escape hatch for lost pet positions.
fn setup_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let show_hide = MenuItem::with_id(app, "toggle_pet", "显示/隐藏桌宠", true, None::<&str>)?;
    let find_pet = MenuItem::with_id(app, "find_pet", "找回桌宠", true, None::<&str>)?;
    let open_history = MenuItem::with_id(app, "history", "全部对话记录", true, None::<&str>)?;
    let open_settings = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&show_hide, &find_pet, &open_history, &open_settings, &quit],
    )?;

    TrayIconBuilder::with_id("main")
        // SAFE-EXPECT: the bundled app icon is generated by Tauri at compile time.
        .icon(app.default_window_icon().expect("bundled icon").clone())
        .tooltip("YUME - 小著")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "toggle_pet" => toggle_pet_visibility(app),
            "find_pet" => pet_recovery::find_pet(app),
            "history" => {
                if let Err(error) = history_entry::open(app) {
                    eprintln!("history organizer could not open: {error}");
                }
            }
            "settings" => show_settings_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_pet_visibility(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// Show/hide the pet; hiding the pet also hides the chat window.
pub(crate) fn toggle_pet_visibility(app: &tauri::AppHandle) {
    pet_visibility::request(app, None);
}

fn show_settings_window(app: &tauri::AppHandle) {
    let Some(win) = app.get_webview_window("settings") else {
        return;
    };
    let _ = win.set_always_on_top(true);
    let large = app
        .state::<SettingsState>()
        .0
        .lock()
        .map(|s| s.settings_large)
        .map_err(|error| error.to_string());
    if let Ok(large) = large {
        if let Err(error) = settings_window::apply(app, large) {
            eprintln!("settings window resize failed: {error}");
        }
    }
    let _ = win.show();
    let _ = win.set_focus();
}

fn restore_settings_focus_if_visible(app: &tauri::AppHandle) {
    let Some(win) = app.get_webview_window("settings") else {
        return;
    };
    let Ok(settings_visible) = win.is_visible() else {
        return;
    };
    if !should_restore_settings_focus(true, settings_visible) {
        return;
    }
    let _ = win.set_always_on_top(true);
    let _ = win.show();
    let _ = win.set_focus();
}

/// Frontend close button: move offscreen instead of hide, so IPC stays alive.
#[tauri::command]
fn hide_settings(app: tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.set_always_on_top(false);
        let _ = win.hide();
    }
}

#[tauri::command]
fn open_settings(app: tauri::AppHandle) {
    show_settings_window(&app);
}

#[tauri::command]
fn open_widget_settings(app: tauri::AppHandle) {
    show_settings_window(&app);
    let _ = app.emit("deskmate://settings-tab", "widget");
}

#[tauri::command]
fn open_ai_model_settings(app: tauri::AppHandle) {
    show_settings_window(&app);
    let _ = app.emit("deskmate://settings-tab", "ai");
}

#[tauri::command]
fn open_worklog_settings(app: tauri::AppHandle, target: Option<serde_json::Value>) {
    show_settings_window(&app);
    let _ = app.emit("deskmate://settings-tab", "worklog");
    let _ = app.emit("deskmate://worklog-target", target);
}

#[cfg(feature = "worklog-qa")]
fn validate_worklog_qa_identity(app: &tauri::AppHandle) -> Result<(), String> {
    let identifier = &app.config().identifier;
    if !qa_identity::is_allowed(identifier) {
        return Err("QA build requires an isolated QA application identity".into());
    }
    for path in [
        app.path().app_data_dir(),
        app.path().app_config_dir(),
        app.path().app_local_data_dir(),
    ] {
        let path = path.map_err(|error| error.to_string())?;
        if path.file_name().and_then(|name| name.to_str()) != Some(identifier.as_str()) {
            return Err("QA application directory must end in isolated QA identity".into());
        }
        eprintln!("worklog QA isolated directory: {}", path.display());
    }
    qa_identity::initialize_keyring(identifier)
}

#[tauri::command]
fn app_version(app: tauri::AppHandle) -> String {
    app.package_info().version.to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let context = tauri::generate_context!();
    #[cfg(windows)]
    let primary = match windows_startup::PrimaryGuard::acquire(&context.config().identifier) {
        Ok(Some(primary)) => primary,
        Ok(None) => {
            windows_startup::notify_primary(&context.config().identifier);
            return;
        }
        Err(error) => {
            eprintln!("cannot establish primary application ownership: {error}");
            return;
        }
    };
    let port = pick_free_port();
    let mut builder = tauri::Builder::default();
    #[cfg(windows)]
    { builder = builder.manage(primary); }

    // Keep the plugin's normal second-launch notification on every desktop.
    // Windows also holds an earlier atomic guard across the plugin's window gap.
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let _ = show_chat(app);
        }));
        #[cfg(windows)]
        {
            builder = builder.plugin(tauri::plugin::Builder::<_, ()>::new("windows-primary-owner")
                .setup(|app, _| {
                    if !app.state::<windows_startup::PrimaryGuard>().owns_plugin_window() {
                        // An older binary may hold only the plugin mutex. Its
                        // absent/still-starting window must never authorize spawn.
                        windows_startup::notify_primary(&app.config().identifier);
                        app.cleanup_before_exit();
                        std::process::exit(0);
                    }
                    Ok(())
                }).build());
        }
    }

    let sidecar_password = uuid::Uuid::new_v4().to_string();
    let _ = SIDECAR_AUTH.set({
        use base64::Engine;
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD
                .encode(format!("opencode:{sidecar_password}"))
        )
    });

    builder
        .register_asynchronous_uri_scheme_protocol("chat-resource", |context, request, responder| {
            let app = context.app_handle().clone();
            let is_chat = context.webview_label() == "chat";
            tauri::async_runtime::spawn_blocking(move || {
                let response = if is_chat {
                    app.try_state::<chat_attachments::resources::ResourceStore>()
                        .map(|store| chat_attachments::resources::respond_preview(&store, request))
                        .unwrap_or_else(|| tauri::http::Response::builder().status(503).body(Vec::new()).expect("static response"))
                } else {
                    tauri::http::Response::builder().status(403).body(Vec::new()).expect("static response")
                };
                // WKURLSchemeTask cancellation runs on the UI thread. Wry checks
                // whether the task is still alive while responding, so perform
                // that check and delivery on the same thread as cancellation.
                // File I/O stays above, on the blocking worker.
                if let Err(error) = app.run_on_main_thread(move || responder.respond(response)) {
                    eprintln!("local resource preview dispatch failed: {error}");
                }
            });
        })
        .on_window_event(|window, event| {
            // Configured WebviewWindows dispatch native drops as WindowEvents.
            if window.label() == "chat" {
                // QA can distinguish native delivery from frontend-only event mocks,
                // without putting file paths into the window title.
                #[cfg(feature = "worklog-qa")]
                if let tauri::WindowEvent::DragDrop(event) = event {
                    let detail = match event {
                        tauri::DragDropEvent::Enter { paths, .. } => format!("enter: {}", paths.len()),
                        tauri::DragDropEvent::Drop { paths, .. } => format!("drop: {}", paths.len()),
                        tauri::DragDropEvent::Leave => "leave".into(),
                        _ => String::new(),
                    };
                    if !detail.is_empty() {
                        let _ = window.set_title(&format!("YUME QA chat [{detail}]"));
                    }
                }
                if let tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) = event {
                    chat_attachments::resources::native_drop(window.app_handle(), paths.clone());
                }
            }
        })
        .plugin(startup_settings::plugin(|app| {
            #[cfg(feature = "worklog-qa")]
            validate_worklog_qa_identity(app)?;
            Ok(settings::load(app))
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        // Native file picker: importing a pack needs a real filesystem path,
        // which a WebView `input[type=file]` never exposes.
        .plugin(tauri_plugin_dialog::init())
        .manage(Sidecar {
            child: Mutex::new(None),
            generation: RwLock::new(0),
            logs: Mutex::new(None),
            port,
            password: sidecar_password,
        })
        .manage(SidecarSupervisor::default())
        .manage(windows_component::ComponentState::default())
        .manage(component_activation::ActivationState::default())
        .manage(workbench::WorkbenchHandoff::default())
        .manage(workbench::WorkbenchWindow::default())
        .manage(workbench::WorkbenchOwnership::default())
        .manage(pomodoro::PomodoroState::default())
        .manage(AttachmentStore::default())
        .manage(agent::AgentPermissionState::default())
        .manage(ChatShown(Mutex::new(false)))
        .manage(history_entry::HistoryOrganizerRequest::default())
        .manage(pet_visibility::PetVisibility::default())
        .manage(pet_visibility_recovery::VisibilityError::default())
        .manage(pet_geometry::PetGeometryState::default())
        .manage(pet_recovery::RecoveryState::default())
        .manage(Arc::new(ChatMotion::default()))
        .invoke_handler(tauri::generate_handler![
            windows_component::windows_component_status,
            windows_component::windows_component_install,
            windows_component::windows_component_cancel,
            windows_component::windows_component_set_enabled,
            windows_component::windows_component_retry_activation,
            chat_links::open_chat_link,
            agent::agent_permission_pending,
            agent::agent_permission_reply,
            agent::run_commands::agent_run_start,
            agent::run_commands::agent_run_read,
            agent::artifacts::agent_artifact_locate,
            agent::run_commands::agent_run_cancel,
            tool_permissions::runtime::tool_permission_pending,
            tool_permissions::runtime::tool_permission_reply,
            tool_permissions::runtime::tool_permission_cancel,
            settings::agent_permission_approval_remove,
            pet_visibility::acknowledge_pet_visibility,
            pet_visibility::register_pet_visibility,
            pet_visibility::get_pet_visibility,
            pet_visibility_recovery::get_pet_visibility_error,
            pet_geometry::configure_pet_geometry,
            pet_input::pet_primary_button_down,
            open_worklog_settings,
            worklog::bridge::worklog_register_turn,
            worklog::commands::worklog_available,
            worklog::commands::worklog_record,
            worklog::commands::worklog_update,
            worklog::commands::worklog_query,
            worklog::commands::worklog_delete_entry,
            worklog::commands::worklog_list_reports,
            worklog::commands::worklog_get_report,
            worklog::commands::worklog_save_report,
            worklog::commands::worklog_apply_version,
            worklog::commands::worklog_delete_report,
            worklog::commands::worklog_list_schedules,
            worklog::commands::worklog_save_schedule,
            worklog::commands::worklog_delete_schedule,
            worklog::commands::worklog_generate_report,
            worklog::commands::worklog_list_runs,
            worklog::commands::worklog_retry_run,
            worklog::commands::worklog_get_operation,
            worklog::export::worklog_export_report,
            sidecar_base_url,
            sidecar_auth,
            workbench::workbench_connection,
            workbench::workbench_take_handoff,
            workbench_theme::workbench_theme,
            workbench::workbench_open_external,
            workbench::workbench_open_session,
            workbench::workbench_session_owned,
            workbench::workbench_claim_session,
            workbench::workbench_resolve_session,
            workbench::workbench_heartbeat,
            workbench::workbench_release_session,
            workbench::workbench_session_status,
            workbench::workbench_abort_session,
            workbench::chat_abort_session,
            workbench::workbench_show,
            workbench::workbench_hide,
            workbench::workbench_pick_directory,
            workbench::workbench_pick_files,
            workbench::workbench_read_file,
            workbench::workbench_save_file,
            workbench::workbench_open_path,
            workbench::workbench_reveal_path,
            load_persona,
            chat_attachments::picker::pick_chat_attachment_files,
            chat_attachments::resources::pick_chat_resources,
            chat_attachments::resources::read_chat_resource_attachment,
            chat_attachments::resources::stage_chat_resource_upload_binary,
            chat_attachments::resources::discard_chat_resources,
            chat_attachments::resources::list_chat_resource_directory,
            chat_attachments::resources::prepare_chat_resources,
            chat_attachments::resources::get_chat_message_resources,
            chat_attachments::stage_chat_attachment_binary,
            chat_attachments::read_chat_attachment,
            chat_attachments::discard_chat_attachment,
            chat_attachments::convert_staged_ncm,
            chat_attachments::export_chat_artifact,
            chat_attachments::cleanup_chat_session,
            toggle_chat,
            hide_chat,
            show_chat_window,
            history_entry::show_history_organizer,
            history_entry::consume_history_organizer_request,
            preview_pet_scale,
            get_settings,
            set_settings,
            settings::chat_model_catalog,
            settings::chat_model_resolve,
            verify_api_key,
            pomodoro::pomodoro_get,
            pomodoro::pomodoro_start,
            pomodoro::pomodoro_pause,
            pomodoro::pomodoro_reset,
            pomodoro::pomodoro_select_phase,
            fetch_ai_usage,
            record_ai_usage,
            open_settings,
            open_widget_settings,
            open_ai_model_settings,
            hide_settings,
            app_version,
            history::commands::history_catalog_list,
            history::commands::history_model_selection_get,
            history::commands::history_model_selection_set,
            history::commands::history_recent_workspaces,
            history::commands::history_remember_workspace,
            history::commands::history_validate_workspace,
            history::commands::history_catalog_native_key,
            history::commands::history_catalog_load,
            history::maintenance::history_chat_selected,
            history::commands::history_catalog_access,
            history::catalog_preview::history_catalog_previews,
            history::catalog_mutation::history_catalog_mutate,
            history::catalog_local::history_save_local_messages,
            history::view::history_list,
            history::view::history_load,
            history::history_register_native_session,
            history::history_save,
            history::history_delete,
            packs::installed_packs,
            packs::import_pack,
            packs::uninstall_pack,
            memory::commands::memory_available,
            memory::automatic::bridge::memory_register_turn,
            memory::automatic::bridge::memory_automation_status,
            memory::automatic::bridge::memory_automation_retry,
            memory::commands::memory_create,
            memory::commands::memory_update,
            memory::commands::memory_list,
            memory::commands::memory_forget,
            memory::commands::memory_clear,
            memory::commands::memory_forget_conversation,
            memory::commands::memory_context,
            memory::commands::memory_export,
            memory::commands::memory_relationship,
            memory::commands::memory_set_relationship_summary,
            memory::commands::memory_link_task,
            memory::commands::memory_unlink_task,
            memory::commands::memory_unlink_deleted_task,
            updater::update_app,
            updater::check_update,
            updater::update_status,
            updater::cancel_update
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            #[cfg(feature = "worklog-qa")]
            validate_worklog_qa_identity(&handle)?;
            #[cfg(windows)]
            {
                let data_dir = handle.path().app_data_dir()?;
                if let Err(error) = app.state::<windows_startup::PrimaryGuard>().cleanup(&data_dir) {
                    eprintln!("recorded sidecar recovery unavailable: {error}");
                }
            }
            chat_attachments::start_stale_sweep(&handle);
            chat_attachments::resources::initialize(&handle).map_err(std::io::Error::other)?;

            // Settings are hydrated by the startup plugin before any window exists.
            // SAFE-UNWRAP: a poisoned settings mutex means an earlier setup command panicked.
            let loaded = app.state::<SettingsState>().0.lock().unwrap().clone();
            if let Err(error) = settings_window::apply(&handle, loaded.settings_large) {
                eprintln!("settings window resize failed: {error}");
            }
            if let Err(error) = settings_window::apply_chat(&handle, loaded.chat_large) {
                eprintln!("chat window resize failed: {error}");
            }
            pet_visibility::initialize(&handle, loaded.pet_visible);
            pomodoro::apply_preferences(&handle, loaded.pomodoro)?;
            pomodoro::start_checker(handle.clone())?;
            settings::register_shortcuts(&handle, &loaded);
            if let Some(pet) = app.get_webview_window("pet") {
                let window_event_handle = handle.clone();
                pet.on_window_event(move |event| {
                    if should_follow_chat_on_window_event(event) {
                        reposition_visible_chat(&window_event_handle);
                    }
                    if let tauri::WindowEvent::Moved(position) = event {
                        settings::persist_pet_position(&window_event_handle, *position);
                    }
                });
                let _ = pet_visibility::set_native_visible(&pet, loaded.pet_visible);
                if !loaded.always_on_top {
                    let _ = pet.set_always_on_top(false);
                }
                pet_geometry::apply(&pet, loaded.pet_scale, &loaded.persona_id);
                pet_startup::place(&pet, &loaded.persona_id, loaded.pet_position);
            }
            let history = history::load(&handle).map_err(std::io::Error::other)?;
            app.manage(HistoryState(Mutex::new(history)));
            app.manage(history::maintenance::HistoryMaintenance::default());
            app.manage(history::NativeSessionIndex::default());
            history::load_native_index(&handle, &app.state::<history::NativeSessionIndex>())?;
            // Memory is optional infrastructure: if the database cannot open,
            // `MemoryState` records that and every memory command answers
            // MEMORY_DISABLED while chat and the pet keep working.
            app.manage(memory::MemoryState::initialize(&handle));
            let runs = handle
                .path()
                .app_data_dir()
                .map_err(|error| error.to_string())?
                .join("agent-runs");
            let agent_runs = agent::AgentRunState::load(agent::RunStore::new(runs))?;
            history::index_agent_records(
                &handle,
                &app.state::<HistoryState>(),
                &agent_runs.all_records()?,
            )?;
            agent_runs.restore_permission_ownership(
                &app.state::<agent::AgentPermissionState>(),
                &loaded.agent_permission_approvals,
            )?;
            app.manage(agent_runs);
            history::commands::initialize(&handle)?;
            app.manage(worklog::commands::WorklogState::initialize(&handle));
            match memory::automatic::start(&handle) {
                Ok(bridge) => {
                    app.manage(bridge);
                }
                Err(code) => eprintln!("automatic memory unavailable: {code}"),
            }
            app.manage(worklog::bridge::WorklogBridge::initialize(&handle));
            worklog::bridge::start_worker(handle.clone());
            settings::start_scheduler(handle.clone());

            setup_tray(&handle)?;
            if spawn_supervised_sidecar(&handle).is_ok() {
                agent::recover_after_start(handle.clone());
            }
            start_sidecar_supervisor(handle.clone());
            app.manage(tool_permissions::events::PermissionEvents::start_managed(handle.clone(), format!(
                "http://127.0.0.1:{port}"
            )));
            agent::start_collector(handle.clone());
            worklog::runner_runtime::start(handle.clone());
            pet_recovery::start(handle.clone())?;
            // Only remove the pre-update backup after the new process has
            // completed its normal initialization path.
            updater::confirm_installed_update(&handle);
            Ok(())
        })
        .build(context)
        // SAFE-EXPECT: building the Tauri app is the final startup boundary; failures are fatal.
        .expect("error while building tauri application")
        .run(|app, event| {
            if let RunEvent::Resumed = event {
                worklog::runner_runtime::notify(app);
                app.state::<SidecarSupervisor>().notify();
            }
            if let RunEvent::Exit = event {
                app.state::<SidecarSupervisor>()
                    .stopping
                    .store(true, Ordering::SeqCst);
                app.state::<SidecarSupervisor>().notify();
                if let Some(bridge) = app.try_state::<worklog::bridge::WorklogBridge>() { bridge.shutdown(); }
                if app.try_state::<agent::AgentRunState>().is_some() {
                    agent::interrupt_owned_run(app, "app_exited");
                }
                app.state::<tool_permissions::events::PermissionEvents>()
                    .stop();
                pet_recovery::stop(app);
                worklog::runner_runtime::stop(app);
                pomodoro::stop_checker(app);
                settings::flush_pet_position(app);
                let supervisor = app.state::<SidecarSupervisor>();
                if let Ok(_recovery) = supervisor.recovery.lock() {
                    if let Ok(Some(child)) = mutate_sidecar_child(app, |slot| Ok(slot.take())) { stop_sidecar_child(app, child); }
                };
            }
        });
}


#[cfg(test)]
mod sidecar_wait_tests {
    use super::*;

    #[test]
    fn shutdown_interrupts_wait_without_acquiring_recovery() {
        let supervisor = Arc::new(SidecarSupervisor::default());
        let recovery = supervisor.recovery.lock().unwrap();
        let (send, receive) = std::sync::mpsc::channel();
        let waiting = supervisor.clone();
        let thread = std::thread::spawn(move || {
            waiting.wait(Duration::from_secs(3600));
            send.send(()).unwrap();
        });
        supervisor.stopping.store(true, Ordering::SeqCst);
        supervisor.notify();
        receive.recv_timeout(Duration::from_secs(2)).expect("shutdown must not wait for recovery/backoff");
        drop(recovery);
        thread.join().unwrap();
    }

    #[test]
    fn manual_retry_wakes_a_backoff_wait() {
        let supervisor = Arc::new(SidecarSupervisor::default());
        let (send, receive) = std::sync::mpsc::channel();
        let waiting = supervisor.clone();
        let thread = std::thread::spawn(move || {
            let revision = waiting.wake_revision.lock().unwrap();
            send.send(()).unwrap();
            let observed = *revision;
            let _guard = waiting.wake.wait_timeout_while(revision, Duration::from_secs(3600), |revision| *revision == observed).unwrap();
        });
        receive.recv_timeout(Duration::from_secs(2)).unwrap();
        supervisor.notify();
        thread.join().unwrap();
    }
}
