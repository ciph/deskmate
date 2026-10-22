use super::{safe_id, unavailable, Launch, Request, WorklogBridge};
use crate::worklog::error::{WorklogError, WorklogResult};
use notify::{event::ModifyKind, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager};

pub fn assert_plain(path: &Path, directory: bool) -> WorklogResult<()> {
    let metadata = fs::symlink_metadata(path).map_err(|_| unavailable())?;
    if metadata.file_type().is_symlink() || metadata.is_dir() != directory {
        return Err(unavailable());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(unavailable());
        }
    }
    Ok(())
}
pub fn cleanup_stale(root: &Path) -> WorklogResult<()> {
    assert_plain(root, true)?;
    for entry in fs::read_dir(root).map_err(|_| unavailable())? {
        let entry = entry.map_err(|_| unavailable())?;
        let path = entry.path();
        if uuid::Uuid::parse_str(&entry.file_name().to_string_lossy()).is_err() {
            continue;
        }
        assert_plain(&path, true)?;
        for file in fs::read_dir(&path).map_err(|_| unavailable())? {
            let file = file.map_err(|_| unavailable())?.path();
            assert_plain(&file, false)?;
            let stem = file
                .file_stem()
                .and_then(|name| name.to_str())
                .ok_or_else(unavailable)?;
            if uuid::Uuid::parse_str(stem).is_err() {
                return Err(unavailable());
            }
            if !matches!(
                file.extension().and_then(|value| value.to_str()),
                Some("request" | "response" | "tmp" | "response-tmp")
            ) {
                return Err(unavailable());
            }
            fs::remove_file(&file).map_err(|_| unavailable())?;
        }
        fs::remove_dir(path).map_err(|_| unavailable())?;
    }
    Ok(())
}
fn fetch(launch: &Launch, directory: &str, route: &str, limit: u64) -> WorklogResult<Value> {
    use std::io::Read;
    crate::history::native_api::NativeHistoryClient::new(&launch.endpoint, &launch.auth_header)
        .map_err(|_| unavailable())?;
    let response = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(5))
        .redirects(0)
        .build()
        .get(&format!("{}{route}", launch.endpoint))
        .query("directory", directory)
        .set("Authorization", &launch.auth_header)
        .call()
        .map_err(|_| unavailable())?;
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| unavailable())?;
    if bytes.len() as u64 > limit {
        return Err(unavailable());
    }
    serde_json::from_slice(&bytes).map_err(|_| unavailable())
}
fn parent(launch: &Launch, request: &Request) -> WorklogResult<String> {
    if !safe_id(&request.session_id) || !safe_id(&request.message_id) || !safe_id(&request.call_id)
    {
        return Err(WorklogError::validation("Invalid tool context"));
    }
    let directory = super::canonical_directory(&request.directory)?;
    let session = fetch(
        launch,
        &directory,
        &format!("/session/{}", request.session_id),
        64 * 1024,
    )?;
    if session["id"].as_str() != Some(&request.session_id)
        || session["directory"]
            .as_str()
            .and_then(|path| super::canonical_directory(path).ok())
            .as_deref()
            != Some(&directory)
    {
        return Err(WorklogError::new(
            "INVALID_CONTEXT",
            "Tool session does not belong to this workspace",
        ));
    }
    // Fetch only the current assistant; unrelated attachments/history never enter this lookup.
    let message = fetch(
        launch,
        &directory,
        &format!(
            "/session/{}/message/{}",
            request.session_id, request.message_id
        ),
        8 * 1024 * 1024,
    )?;
    verify_parent(&[message], request)
}
fn native_user_text(launch: &Launch, request: &Request, user: &str) -> WorklogResult<String> {
    if !safe_id(user) {
        return Err(unavailable());
    }
    let directory = super::canonical_directory(&request.directory)?;
    let message = fetch(
        launch,
        &directory,
        &format!("/session/{}/message/{user}", request.session_id),
        96 * 1024 * 1024,
    )?;
    if message["info"]["id"].as_str() != Some(user)
        || message["info"]["role"] != "user"
        || message["info"]["sessionID"].as_str() != Some(&request.session_id)
    {
        return Err(WorklogError::new(
            "INVALID_CONTEXT",
            "Native user source is unavailable",
        ));
    }
    let text = message["parts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|part| {
            part["type"] == "text" && part["synthetic"] != true && part["ignored"] != true
        })
        .filter_map(|part| part["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n");
    if text.trim().is_empty() {
        return Err(WorklogError::new(
            "INVALID_CONTEXT",
            "Native user text is unavailable",
        ));
    }
    Ok(text)
}
fn verify_parent(messages: &[Value], request: &Request) -> WorklogResult<String> {
    let message = messages
        .iter()
        .find(|message| message["info"]["id"].as_str() == Some(&request.message_id))
        .ok_or_else(unavailable)?;
    if message["info"]["role"] != "assistant"
        || message["info"]["sessionID"].as_str() != Some(&request.session_id)
    {
        return Err(unavailable());
    }
    let tool = format!("worklog_{}", request.action);
    let valid = message["parts"].as_array().is_some_and(|parts| {
        parts.iter().any(|part| {
            part["type"] == "tool"
                && part["callID"].as_str() == Some(&request.call_id)
                && part["tool"].as_str() == Some(&tool)
        })
    });
    if !valid {
        return Err(WorklogError::new(
            "INVALID_CONTEXT",
            "Tool call does not belong to the registered assistant",
        ));
    }
    message["info"]["parentID"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(unavailable)
}
fn ensure_current(bridge: &WorklogBridge, launch: &Launch) -> WorklogResult<()> {
    let current = bridge.launch.lock().map_err(|_| unavailable())?;
    if current.as_ref().map(|value| &value.directory) != Some(&launch.directory) {
        return Err(WorklogError::new("STALE_CONTEXT", "Sidecar was restarted"));
    }
    Ok(())
}

// Production and tests share the authenticated lookup, grant lookup and repository dispatch.
fn process(
    bridge: &WorklogBridge,
    launch: &Launch,
    request: &Request,
    execute: impl FnOnce(&super::auth::Grant, &str) -> WorklogResult<Value>,
) -> WorklogResult<Value> {
    if request.version != 1 || uuid::Uuid::parse_str(&request.request_id).is_err() {
        return Err(WorklogError::validation("Unsupported bridge request"));
    }
    ensure_current(bridge, launch)?;
    let directory = super::canonical_directory(&request.directory)?;
    let user = parent(launch, request)?;
    let needs_source = {
        let grants = bridge.grants.lock().map_err(|_| unavailable())?;
        grants
            .active
            .get(&(directory.clone(), request.session_id.clone(), user.clone()))
            .is_some_and(|turn| turn.grant.is_none())
    };
    let source = if needs_source {
        Some(native_user_text(launch, request, &user)?)
    } else {
        None
    };
    // Hold the launch through commit: reset cannot race a validated old request into a write.
    let current = bridge.launch.lock().map_err(|_| unavailable())?;
    if current.as_ref().map(|value| &value.directory) != Some(&launch.directory) {
        return Err(WorklogError::new("STALE_CONTEXT", "Sidecar was restarted"));
    }
    let mut grants = bridge.grants.lock().map_err(|_| unavailable())?;
    let turn = grants
        .active
        .get_mut(&(directory, request.session_id.clone(), user.clone()))
        .filter(|turn| turn.created.elapsed().as_secs() < 1800)
        .ok_or_else(|| {
            WorklogError::new(
                "NEEDS_EXPLICIT_REQUEST",
                "This conversation turn has no work journal authorization",
            )
        })?;
    if turn.grant.is_none() {
        let mut grant = super::auth::grant(source.as_deref().ok_or_else(unavailable)?)?;
        grant.received_date = turn.received_date;
        grant.created = turn.created;
        turn.grant = Some(grant);
    }
    execute(turn.grant.as_ref().ok_or_else(unavailable)?, &user)
}

fn handle(app: &tauri::AppHandle, launch: &Launch, request: Request) -> WorklogResult<Value> {
    let model = {
        let state = app.state::<crate::settings::SettingsState>();
        let settings = state.0.lock().map_err(|_| unavailable())?;
        format!("{}/{}", settings.provider_id, settings.model_id)
    };
    let bridge = app.state::<WorklogBridge>();
    let result = process(&bridge, launch, &request, |grant, user| {
        app.state::<crate::worklog::WorklogState>()
            .with_repository(|repo| super::dispatch::execute(repo, &request, grant, user, &model))
    })?;
    if request.action != "query" {
        if app.emit("deskmate://worklog-changed", ()).is_err() {
            eprintln!("Worklog notification unavailable; committed data remains readable");
        }
    }
    Ok(result)
}
fn consume(
    launch: &Launch,
    path: &Path,
    execute: impl FnOnce(Request) -> WorklogResult<Value>,
) -> WorklogResult<()> {
    assert_plain(&launch.directory, true)?;
    assert_plain(path, false)?;
    if fs::metadata(path).map_err(|_| unavailable())?.len() > 256 * 1024 {
        fs::remove_file(path).map_err(|_| unavailable())?;
        return Err(WorklogError::validation("Bridge request exceeds 256 KiB"));
    }
    let name = path
        .file_stem()
        .and_then(|name| name.to_str())
        .ok_or_else(unavailable)?;
    uuid::Uuid::parse_str(name).map_err(|_| WorklogError::validation("Invalid request file"))?;
    let request: Request = serde_json::from_slice(&fs::read(path).map_err(|_| unavailable())?)
        .map_err(|_| WorklogError::validation("Malformed bridge request"))?;
    if request.request_id != name {
        return Err(WorklogError::validation("Request identity mismatch"));
    }
    let result = match execute(request) {
        Ok(value) => json!({"version":1,"requestId":name,"status":"completed","result":value}),
        Err(error) => json!({"version":1,"requestId":name,"status":"rejected","error":error}),
    };
    let encoded = serde_json::to_vec(&result)?;
    let encoded = if encoded.len() > 256 * 1024 {
        serde_json::to_vec(
            &json!({"version":1,"requestId":name,"status":"rejected","error":{"code":"RESULT_TOO_LARGE","message":"Narrow the requested date range"}}),
        )?
    } else {
        encoded
    };
    let temp = launch.directory.join(format!("{name}.response-tmp"));
    let target = launch.directory.join(format!("{name}.response"));
    use std::io::Write;
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|_| unavailable())?;
    output.write_all(&encoded).map_err(|_| unavailable())?;
    output.sync_all().map_err(|_| unavailable())?;
    fs::rename(temp, target).map_err(|_| unavailable())?;
    fs::remove_file(path).map_err(|_| unavailable())?;
    Ok(())
}
// Below the tool's 15 s receipt wait; slow/in-flight native calls can still return pending.
const RECONCILE_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Default)]
struct WakeState {
    pending: bool,
    stopped: bool,
    watch_failed: bool,
}

#[derive(Default)]
pub(super) struct Wake {
    state: Mutex<WakeState>,
    changed: Condvar,
}
impl Wake {
    pub(super) fn notify(&self) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.pending = true;
        self.changed.notify_one();
    }
    fn invalidate_watch(&self) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.watch_failed = true;
        state.pending = true;
        self.changed.notify_one();
    }
    fn take_watch_failure(&self) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        std::mem::take(&mut state.watch_failed)
    }
    pub(super) fn stop(&self) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.stopped = true;
        self.changed.notify_one();
    }
    // Consume pending before scanning; notifications during the scan remain set for the next pass.
    fn wait_until(&self, deadline: Instant) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        while !state.pending && !state.stopped {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            state = self
                .changed
                .wait_timeout(state, deadline - now)
                .unwrap_or_else(|error| error.into_inner())
                .0;
        }
        state.pending = false;
        !state.stopped
    }
}

fn request_event(event: &Event) -> bool {
    if event.need_rescan() || matches!(event.kind, EventKind::Any | EventKind::Other) {
        return true;
    }
    matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(ModifyKind::Name(_) | ModifyKind::Any)
    ) && (event.paths.is_empty()
        || event.paths.iter().any(|path| {
            path.extension().and_then(|extension| extension.to_str()) == Some("request")
        }))
}

#[derive(Default)]
struct DirectoryWatch {
    directory: Option<PathBuf>,
    watcher: Option<RecommendedWatcher>,
}
impl DirectoryWatch {
    fn refresh(&mut self, directory: Option<&Path>, wake: &Arc<Wake>, reconcile: bool) {
        let failed = wake.take_watch_failure();
        let changed = self.directory.as_deref() != directory;
        if !changed && !failed && !reconcile && self.watcher.is_some() {
            return;
        }
        if changed || failed {
            self.watcher = None;
        }
        self.directory = directory.map(Path::to_path_buf);
        let Some(directory) = directory else { return };
        if assert_plain(directory, true).is_err() {
            self.watcher = None;
            return;
        }
        // Rearm on reconciliation too: a replaced directory may have lost its delete notification.
        if let Some(watcher) = &mut self.watcher {
            let _ = watcher.unwatch(directory);
            if watcher
                .watch(directory, RecursiveMode::NonRecursive)
                .is_ok()
            {
                return;
            }
            self.watcher = None;
        }
        let signal = wake.clone();
        let watched = directory.to_path_buf();
        let result =
            notify::recommended_watcher(move |result: notify::Result<Event>| match result {
                Ok(event) => {
                    if event.paths.iter().any(|path| path == &watched)
                        && matches!(
                            event.kind,
                            EventKind::Remove(_) | EventKind::Modify(ModifyKind::Name(_))
                        )
                    {
                        signal.invalidate_watch();
                    } else if request_event(&event) {
                        signal.notify();
                    }
                }
                Err(_) => signal.invalidate_watch(),
            })
            .and_then(|mut watcher| {
                watcher.watch(directory, RecursiveMode::NonRecursive)?;
                Ok(watcher)
            });
        match result {
            Ok(watcher) => self.watcher = Some(watcher),
            Err(_) => eprintln!(
                "Worklog file notifications unavailable; periodic reconciliation remains active"
            ),
        }
    }
}

fn scan(
    launch: &Launch,
    mut execute: impl FnMut(Request) -> WorklogResult<Value>,
) -> WorklogResult<()> {
    // Never enumerate through a replaced symlink/reparse directory, including expired-file cleanup.
    assert_plain(&launch.directory, true)?;
    for entry in fs::read_dir(&launch.directory)
        .map_err(|_| unavailable())?
        .flatten()
    {
        let path = entry.path();
        let expired = entry
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|modified| modified.elapsed().ok())
            .is_some_and(|age| age.as_secs() > 1800);
        if expired && assert_plain(&path, false).is_ok() {
            if let Err(error) = fs::remove_file(&path) {
                eprintln!("Worklog expired file cleanup: {}", error.kind());
            }
            continue;
        }
        if path.extension().and_then(|extension| extension.to_str()) == Some("request") {
            if let Err(error) = consume(launch, &path, &mut execute) {
                eprintln!("Worklog bridge: {}", error.code);
                if assert_plain(&path, false).is_ok() {
                    if let Err(error) = fs::remove_file(&path) {
                        eprintln!("Worklog request cleanup: {}", error.kind());
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn start(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let bridge = app.state::<WorklogBridge>();
        let mut watch = DirectoryWatch::default();
        let mut next_reconcile = Instant::now();
        while bridge.wake.wait_until(next_reconcile) {
            let now = Instant::now();
            let reconcile = now >= next_reconcile;
            if reconcile {
                next_reconcile = now + RECONCILE_INTERVAL;
                if let Ok(mut grants) = bridge.grants.lock() {
                    grants
                        .active
                        .retain(|_, turn| turn.created.elapsed().as_secs() < 1800);
                }
            }
            let launch = bridge.launch.lock().ok().and_then(|value| value.clone());
            // Install before scanning: pre-watch requests are found by the scan, later ones wake it.
            watch.refresh(
                launch.as_ref().map(|value| value.directory.as_path()),
                &bridge.wake,
                reconcile,
            );
            if let Some(launch) = launch {
                if let Err(error) = scan(&launch, |request| handle(&app, &launch, request)) {
                    eprintln!("Worklog bridge scan: {}", error.code);
                }
            }
        }
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn assistant_identity_requires_matching_tool_call() {
        let request = Request {
            version: 1,
            request_id: uuid::Uuid::new_v4().to_string(),
            directory: "/workspace/a".into(),
            session_id: "ses_test".into(),
            message_id: "msg_assistant".into(),
            call_id: "call_true".into(),
            action: "record".into(),
            args: json!({}),
        };
        let valid = json!({"info":{"id":"msg_assistant","role":"assistant","sessionID":"ses_test","parentID":"msg_host"},"parts":[{"type":"tool","tool":"worklog_record","callID":"call_true"}]});
        assert_eq!(
            verify_parent(&[valid.clone()], &request).expect("verified"),
            "msg_host"
        );
        let mut spoof = valid;
        spoof["parts"][0]["callID"] = json!("call_other");
        assert_eq!(
            verify_parent(&[spoof], &request).expect_err("reject").code,
            "INVALID_CONTEXT"
        );
    }
}

#[cfg(test)]
#[path = "bridge_http_tests.rs"]
mod http_tests;

#[cfg(test)]
#[path = "bridge_worker_tests.rs"]
mod worker_tests;
