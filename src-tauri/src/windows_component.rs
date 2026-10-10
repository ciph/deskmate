//! A pinned, optional Windows UI component. Downloads never execute code.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::Duration,
};
use tauri::Manager;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Manifest {
    pub version: String,
    pub url: String,
    pub archive_bytes: u64,
    archive_sha256: String,
    binary_name: String,
    binary_bytes: u64,
    binary_sha256: String,
}
pub(crate) fn manifest() -> Manifest {
    serde_json::from_str(include_str!("../../components/windows-mcp.json"))
        .expect("compiled component manifest")
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Status {
    pub phase: String,
    pub downloaded: u64,
    pub total: u64,
    pub version: String,
    pub supported: bool,
    pub activation: String,
    pub error: Option<String>,
}
impl Default for Status {
    fn default() -> Self {
        let m = manifest();
        Self {
            phase: "unchecked".into(),
            downloaded: 0,
            total: m.archive_bytes,
            version: m.version,
            supported: cfg!(all(windows, target_arch = "x86_64")),
            activation: "idle".into(),
            error: None,
        }
    }
}
#[derive(Default)]
pub(crate) struct ComponentState {
    pub status: Mutex<Status>,
    action: Mutex<()>,
    downloading: AtomicBool,
    cancel: AtomicBool,
}
impl ComponentState {
    fn start_download(&self) -> bool {
        if self.downloading.swap(true, Ordering::AcqRel) {
            return false;
        }
        self.cancel.store(false, Ordering::Release);
        true
    }
}
fn require_settings(window: &tauri::WebviewWindow) -> Result<(), String> {
    (window.label() == "settings")
        .then_some(())
        .ok_or_else(|| "component_settings_only".into())
}
fn root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let data = app.path().app_data_dir().map_err(|_| "component_storage")?;
    let components = data.join("components");
    let root = components.join("windows-mcp");
    for path in [&components, &root] {
        reject_link(path)?;
    }
    Ok(root)
}
fn reject_link(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if meta.file_attributes() & 0x400 != 0 {
                    return Err("component_unsafe_path".into());
                }
            }
            if meta.file_type().is_symlink() {
                return Err("component_unsafe_path".into());
            }
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("component_storage".into()),
    }
}
fn verified_file(path: &Path, m: &Manifest) -> bool {
    if reject_link(path).is_err() {
        return false;
    }
    let Ok(file) = fs::File::open(path) else {
        return false;
    };
    if file.metadata().map(|v| v.len()).ok() != Some(m.binary_bytes) {
        return false;
    }
    let mut reader = file.take(m.binary_bytes + 1);
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => hash.update(&buffer[..n]),
            Err(_) => return false,
        }
    }
    format!("{:x}", hash.finalize()) == m.binary_sha256
}
pub(crate) fn resolve(app: &tauri::AppHandle) -> Option<PathBuf> {
    if !cfg!(all(windows, target_arch = "x86_64")) {
        return None;
    }
    let m = manifest();
    let directory = root(app).ok()?.join(&m.version);
    reject_link(&directory).ok()?;
    let path = directory.join(&m.binary_name);
    verified_file(&path, &m).then_some(path)
}
fn snapshot(app: &tauri::AppHandle) -> Result<Status, String> {
    let state = app.state::<ComponentState>();
    let mut status = state.status.lock().map_err(|_| "component_state")?;
    if status.phase == "unchecked" {
        status.phase = if resolve(app).is_some() {
            "ready"
        } else {
            "missing"
        }
        .into();
    }
    Ok(status.clone())
}
#[tauri::command]
pub(crate) async fn windows_component_status(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> Result<Status, String> {
    require_settings(&window)?;
    tauri::async_runtime::spawn_blocking(move || snapshot(&app))
        .await
        .map_err(|_| "component_state")?
}
fn update(app: &tauri::AppHandle, phase: &str, downloaded: u64, error: Option<String>) {
    if phase == "downloading" && app.state::<ComponentState>().cancel.load(Ordering::Acquire) {
        return;
    }
    if let Ok(mut status) = app.state::<ComponentState>().status.lock() {
        status.phase = phase.into();
        status.downloaded = downloaded;
        status.error = error;
    }
}
fn check_cancel(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Acquire) {
        Err("component_cancelled".into())
    } else {
        Ok(())
    }
}
fn download(
    mut reader: impl Read,
    mut writer: impl Write,
    m: &Manifest,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64),
) -> Result<(), String> {
    let mut hash = Sha256::new();
    let mut count = 0;
    let mut buffer = [0u8; 65536];
    loop {
        check_cancel(cancel)?;
        let n = reader.read(&mut buffer).map_err(|_| {
            if cancel.load(Ordering::Acquire) {
                "component_cancelled"
            } else {
                "component_network"
            }
        })?;
        if n == 0 {
            break;
        }
        count += n as u64;
        if count > m.archive_bytes {
            return Err("component_size".into());
        }
        writer
            .write_all(&buffer[..n])
            .map_err(|_| "component_storage")?;
        hash.update(&buffer[..n]);
        progress(count);
    }
    check_cancel(cancel)?;
    if count != m.archive_bytes || format!("{:x}", hash.finalize()) != m.archive_sha256 {
        return Err("component_checksum".into());
    }
    Ok(())
}
fn extract(archive: &Path, destination: &Path, m: &Manifest) -> Result<(), String> {
    let file = fs::File::open(archive).map_err(|_| "component_storage")?;
    let mut zip = zip::ZipArchive::new(file).map_err(|_| "component_archive")?;
    // Extract only the pinned root executable, never archive-provided paths.
    let mut entry = zip
        .by_name(&m.binary_name)
        .map_err(|_| "component_archive")?;
    if entry.size() != m.binary_bytes
        || entry.is_dir()
        || entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
    {
        return Err("component_archive".into());
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|_| "component_storage")?;
    std::io::copy(&mut entry.by_ref().take(m.binary_bytes + 1), &mut file)
        .map_err(|_| "component_archive")?;
    file.sync_all().map_err(|_| "component_storage")?;
    drop(file);
    if !verified_file(destination, m) {
        return Err("component_checksum".into());
    }
    Ok(())
}
struct Staging(PathBuf);
impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn response(m: &Manifest) -> Result<ureq::Response, String> {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(15))
        .build()
        .get(&m.url)
        .call()
        .map_err(|_| "component_network".into())
}
fn install(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<ComponentState>();
    if resolve(app).is_some() {
        return Ok(());
    }
    let m = manifest();
    let root = root(app)?;
    fs::create_dir_all(&root).map_err(|_| "component_storage")?;
    let staging = Staging(root.join(format!(".download-{}", uuid::Uuid::new_v4())));
    fs::create_dir(&staging.0).map_err(|_| "component_storage")?;
    let archive = staging.0.join("download.zip");
    let file = fs::File::create(&archive).map_err(|_| "component_storage")?;
    let response = response(&m)?;
    download(response.into_reader(), file, &m, &state.cancel, |n| {
        update(app, "downloading", n, None)
    })?;
    update(app, "verifying", m.archive_bytes, None);
    let binary = staging.0.join(&m.binary_name);
    extract(&archive, &binary, &m)?;
    check_cancel(&state.cancel)?;
    fs::remove_file(&archive).map_err(|_| "component_storage")?;
    let destination = root.join(&m.version);
    reject_link(&destination)?;
    // Leave any damaged installation recoverable; never overwrite other versions.
    if destination.exists() {
        fs::rename(
            &destination,
            root.join(format!(".invalid-{}", uuid::Uuid::new_v4())),
        )
        .map_err(|_| "component_storage")?;
    }
    fs::rename(&staging.0, &destination).map_err(|_| "component_storage")?;
    Ok(())
}
#[tauri::command]
pub(crate) fn windows_component_install(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> Result<(), String> {
    require_settings(&window)?;
    if !cfg!(all(windows, target_arch = "x86_64")) {
        return Err("component_unsupported".into());
    }
    let state = app.state::<ComponentState>();
    let action = state.action.try_lock().map_err(|_| "component_busy")?;
    if !state.start_download() {
        return Ok(());
    }
    update(&app, "downloading", 0, None);
    drop(action);
    tauri::async_runtime::spawn_blocking(move || {
        let mut result = install(&app);
        let state = app.state::<ComponentState>();
        let _action = state
            .action
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if result.is_err() && state.cancel.load(Ordering::Acquire) {
            result = Err("component_cancelled".into());
        }
        match result {
            Ok(()) => {
                update(&app, "ready", manifest().archive_bytes, None);
                if !state.cancel.load(Ordering::Acquire) && enable(&app, true).is_err() {
                    if let Ok(mut status) = state.status.lock() {
                        status.activation = "error".into();
                        status.error = Some("component_enable".into());
                    }
                }
            }
            Err(error) => update(
                &app,
                if error == "component_cancelled" {
                    "missing"
                } else {
                    "error"
                },
                0,
                Some(error),
            ),
        }
        state.downloading.store(false, Ordering::Release);
    });
    Ok(())
}
#[tauri::command]
pub(crate) fn windows_component_cancel(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> Result<(), String> {
    require_settings(&window)?;
    let state = app.state::<ComponentState>();
    let _action = state.action.try_lock().map_err(|_| "component_busy")?;
    state.cancel.store(true, Ordering::Release);
    if state.downloading.load(Ordering::Acquire) {
        if let Ok(mut status) = state.status.lock() {
            status.phase = "cancelling".into();
        }
    }
    Ok(())
}
fn enable(app: &tauri::AppHandle, enabled: bool) -> Result<(), String> {
    if enabled && resolve(app).is_none() {
        update(app, "missing", 0, None);
        return Err("component_missing".into());
    }
    let state = app.state::<crate::settings::SettingsState>();
    let mut settings = state.0.lock().map_err(|_| "component_state")?.clone();
    let changed = settings.windows_mcp_enabled != enabled;
    settings.windows_mcp_enabled = enabled;
    crate::settings::set_settings(app.clone(), state, settings)?;
    // Repairing an enabled-but-missing installation must also reload the engine.
    if !changed {
        crate::component_activation::request(app);
    }
    Ok(())
}
#[tauri::command]
pub(crate) async fn windows_component_set_enabled(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    enabled: bool,
) -> Result<(), String> {
    require_settings(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<ComponentState>();
        let _action = state.action.lock().map_err(|_| "component_state")?;
        if !enabled {
            state.cancel.store(true, Ordering::Release);
        }
        enable(&app, enabled)
    })
    .await
    .map_err(|_| "component_state")?
}
#[tauri::command]
pub(crate) fn windows_component_retry_activation(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> Result<(), String> {
    require_settings(&window)?;
    if app
        .state::<crate::settings::SettingsState>()
        .0
        .lock()
        .map_err(|_| "component_state")?
        .windows_mcp_enabled
        && resolve(&app).is_none()
    {
        update(&app, "missing", 0, None);
        return Err("component_missing".into());
    }
    crate::component_activation::request(&app);
    Ok(())
}

#[cfg(test)]
#[path = "windows_component_tests.rs"]
mod tests;
