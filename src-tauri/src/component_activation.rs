//! Short restart boundary shared by task admission and background model work.
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    RwLock, RwLockReadGuard,
};
use std::time::Duration;
use tauri::Manager;

static SERVICE_USE: RwLock<()> = RwLock::new(());
#[derive(Default)]
pub(crate) struct ActivationState {
    running: AtomicBool,
    revision: AtomicU64,
}

pub(crate) fn service_use() -> Result<RwLockReadGuard<'static, ()>, String> {
    SERVICE_USE.try_read().map_err(|_| {
        "AI service is applying the Windows component change; please retry shortly.".into()
    })
}
// Background workers can wait for the short restart; native window commands
// must use try_read above so the UI thread remains available to the restart.
pub(crate) fn background_service_use() -> Result<RwLockReadGuard<'static, ()>, String> {
    SERVICE_USE
        .read()
        .map_err(|_| "component_activation_unavailable".into())
}
fn status(app: &tauri::AppHandle, value: &str) {
    if let Ok(mut status) = app
        .state::<crate::windows_component::ComponentState>()
        .status
        .lock()
    {
        status.activation = value.into();
        if value == "waiting" {
            status.error = None;
        }
    }
}
fn idle(app: &tauri::AppHandle) -> Result<bool, String> {
    if app
        .state::<crate::agent::AgentRunState>()
        .read()?
        .active
        .is_some()
    {
        return Ok(false);
    }
    // A visible native workbench can submit directly to the engine. Wait until
    // it is closed, then also check tasks that continue while it is hidden.
    if let Some(window) = app.get_webview_window("workbench") {
        if window.is_visible().map_err(|e| e.to_string())? {
            return Ok(false);
        }
    }
    let store = crate::history::commands::store(app)?;
    let client = crate::history::commands::client(app)?;
    let mut directories = store.directories()?;
    directories.push(crate::history::commands::workspace(app)?);
    directories.sort();
    directories.dedup();
    for directory in directories {
        // A removed workspace cannot accept new work through the host.
        if !std::path::Path::new(&directory).is_dir() {
            continue;
        }
        if client
            .statuses(&directory)
            .map_err(|e| e.to_string())?
            .values()
            .any(|s| !matches!(s, crate::history::native_api::NativeStatus::Idle))
        {
            return Ok(false);
        }
    }
    Ok(true)
}
pub(crate) fn request(app: &tauri::AppHandle) {
    let state = app.state::<ActivationState>();
    state.revision.fetch_add(1, Ordering::AcqRel);
    status(app, "waiting");
    if state.running.swap(true, Ordering::AcqRel) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || loop {
        let state = app.state::<ActivationState>();
        let revision = state.revision.load(Ordering::Acquire);
        let result = match SERVICE_USE.try_write() {
            Ok(_exclusive) => match idle(&app) {
                Ok(true) => {
                    status(&app, "applying");
                    Some(crate::restart_idle_sidecar(&app))
                }
                Ok(false) => None,
                Err(error) => Some(Err(error)),
            },
            Err(std::sync::TryLockError::WouldBlock) => None,
            Err(_) => Some(Err("component_activation_unavailable".into())),
        };
        if let Some(result) = result {
            status(&app, if result.is_ok() { "idle" } else { "error" });
            state.running.store(false, Ordering::Release);
            // A setting may change while the old request is being applied.
            if state.revision.load(Ordering::Acquire) != revision {
                request(&app);
            }
            return;
        }
        std::thread::sleep(Duration::from_millis(500));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn an_active_model_request_prevents_the_restart_boundary() {
        let active = service_use().unwrap();
        assert!(matches!(
            SERVICE_USE.try_write(),
            Err(std::sync::TryLockError::WouldBlock)
        ));
        drop(active);
    }
}
