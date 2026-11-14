//! Create settings on demand; keep its mounted draft state when hidden.
use serde::Serialize;
use std::sync::Mutex;
use tauri::{Emitter, Manager};

#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SettingsRequest {
    pub tab: &'static str,
    pub target: Option<serde_json::Value>,
}

#[derive(Default)]
pub(crate) struct SettingsWindow {
    creation: Mutex<()>,
    pending: Mutex<Option<SettingsRequest>>,
}
impl SettingsWindow {
    fn ensure<T>(
        &self,
        find: impl FnOnce() -> Option<T>,
        create: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        let _creation = self
            .creation
            .lock()
            .map_err(|_| "settings creation unavailable")?;
        match find() {
            Some(window) => Ok(window),
            None => create(),
        }
    }
    fn request(&self, request: Option<SettingsRequest>) -> Result<(), String> {
        if request.is_some() {
            *self
                .pending
                .lock()
                .map_err(|_| "settings request unavailable")? = request;
        }
        Ok(())
    }
    fn consume(&self, label: &str) -> Result<Option<SettingsRequest>, String> {
        if label != "settings" {
            return Err("only settings may consume its navigation request".into());
        }
        Ok(self
            .pending
            .lock()
            .map_err(|_| "settings request unavailable")?
            .take())
    }
}

fn create(app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, String> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|config| config.label == "settings")
        .ok_or("settings window configuration missing")?;
    let window = tauri::WebviewWindowBuilder::from_config(app, config)
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;
    let owner = window.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            hide(owner.app_handle());
        }
    });
    Ok(window)
}

fn show(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<SettingsWindow>();
    let window = state.ensure(|| app.get_webview_window("settings"), || create(app))?;
    let large = app
        .state::<crate::settings::SettingsState>()
        .0
        .lock()
        .map_err(|_| "settings unavailable")?
        .settings_large;
    crate::settings_window::apply(app, large).map_err(|e| e.to_string())?;
    window.set_always_on_top(true).map_err(|e| e.to_string())?;
    window.show().map_err(|e| e.to_string())?;
    let _ = window.emit("deskmate://settings-visibility", true);
    window.set_focus().map_err(|e| e.to_string())?;
    // The ready page consumes the queue after subscribing; a lost notification is safe.
    window
        .emit("deskmate://settings-request", ())
        .map_err(|e| e.to_string())
}

pub(crate) fn hide(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.set_always_on_top(false);
        if window.hide().is_ok() {
            let _ = window.emit("deskmate://settings-visibility", false);
        }
    }
}

pub(crate) async fn open(
    app: tauri::AppHandle,
    request: Option<SettingsRequest>,
) -> Result<(), String> {
    app.state::<SettingsWindow>().request(request)?;
    // Building WebView2 from a synchronous command can deadlock the UI thread.
    tauri::async_runtime::spawn_blocking(move || show(&app))
        .await
        .map_err(|_| "settings creation worker failed".to_owned())?
}

#[tauri::command]
pub(crate) fn consume_settings_request(
    window: tauri::WebviewWindow,
    state: tauri::State<SettingsWindow>,
) -> Result<Option<SettingsRequest>, String> {
    state.consume(window.label())
}

#[tauri::command]
pub(crate) fn get_settings_visibility(window: tauri::WebviewWindow) -> Result<bool, String> {
    if window.label() != "settings" {
        return Err("only settings may query its visibility".into());
    }
    window.is_visible().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigation_waits_for_ready_page_and_only_latest_target_is_consumed() {
        let state = SettingsWindow::default();
        state
            .request(Some(SettingsRequest {
                tab: "ai",
                target: None,
            }))
            .unwrap();
        let request = SettingsRequest {
            tab: "worklog",
            target: Some(serde_json::json!({"kind":"entry","id":"entry-1"})),
        };
        state.request(Some(request.clone())).unwrap();
        state.request(None).unwrap();
        assert!(state.consume("chat").is_err());
        assert_eq!(state.consume("settings").unwrap(), Some(request));
        assert_eq!(state.consume("settings").unwrap(), None);
    }
    #[test]
    fn creation_is_single_and_retryable_after_failure_without_losing_target() {
        use std::sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            Arc,
        };
        let state = Arc::new(SettingsWindow::default());
        state
            .request(Some(SettingsRequest {
                tab: "widget",
                target: None,
            }))
            .unwrap();
        assert!(state
            .ensure::<()>(|| None, || Err("fixture failure".into()))
            .is_err());
        let exists = Arc::new(AtomicBool::new(false));
        let creations = Arc::new(AtomicUsize::new(0));
        let threads: Vec<_> = (0..12)
            .map(|_| {
                let (state, exists, creations) = (state.clone(), exists.clone(), creations.clone());
                std::thread::spawn(move || {
                    state
                        .ensure(
                            || exists.load(Ordering::SeqCst).then_some(()),
                            || {
                                creations.fetch_add(1, Ordering::SeqCst);
                                exists.store(true, Ordering::SeqCst);
                                Ok(())
                            },
                        )
                        .unwrap()
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(creations.load(Ordering::SeqCst), 1);
        assert_eq!(state.consume("settings").unwrap().unwrap().tab, "widget");
    }
    #[test]
    fn production_config_defers_settings_but_preserves_chat_and_settings_window_properties() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let windows = config["app"]["windows"].as_array().unwrap();
        let settings = windows.iter().find(|w| w["label"] == "settings").unwrap();
        assert_eq!(settings["create"], false);
        assert_eq!(settings["visible"], false);
        assert_eq!(settings["url"], "settings.html");
        assert_eq!(settings["width"], 720);
        assert_eq!(settings["height"], 520);
        assert_eq!(settings["transparent"], true);
        assert!(windows
            .iter()
            .filter(|w| w["label"] != "settings")
            .all(|w| w["create"] != false));
    }
}
