use super::runtime::PermissionRequest;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    io::{BufRead, BufReader, Read},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tauri::Manager;

#[path = "events_refresh.rs"]
mod refresh;
use refresh::ActiveRefresh;
pub(crate) use refresh::{RefreshChanges, RefreshScope};

const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;
const MAX_PENDING_PERMISSIONS: usize = 4096;
const MAX_SEEN_PERMISSIONS: usize = 256;
type Generation = Arc<dyn Fn() -> Result<u64, String> + Send + Sync>;
type PermissionKey = (String, String, String);

#[derive(Default)]
struct StreamState {
    connected: bool,
    generation: Option<u64>,
    requests: BTreeMap<PermissionKey, PermissionRequest>,
    settled: BTreeSet<PermissionKey>,
    permissions_invalid: bool,
    seen_permissions: VecDeque<(String, String, String)>,
    revision: u64,
    active: Option<ActiveRefresh>,
}

impl StreamState {
    fn full(&mut self) {
        self.revision = self.revision.saturating_add(1);
        if let Some(active) = &mut self.active {
            active.full(self.revision);
        }
    }
    fn disconnect(&mut self) {
        self.connected = false;
        self.requests.clear();
        self.settled.clear();
        self.seen_permissions.clear();
        self.permissions_invalid = false;
        self.full();
    }
}

#[derive(Clone)]
pub struct PermissionEvents {
    state: Arc<Mutex<StreamState>>,
    stopped: Arc<AtomicBool>,
    current_generation: Generation,
    default_directory: Option<String>,
}

impl Default for PermissionEvents {
    fn default() -> Self {
        Self {
            state: Arc::default(),
            stopped: Arc::default(),
            current_generation: Arc::new(|| Ok(0)),
            default_directory: None,
        }
    }
}

impl PermissionEvents {
    pub fn start_managed(app: tauri::AppHandle, base: String) -> Self {
        let default_directory = app
            .path()
            .app_data_dir()
            .ok()
            .and_then(|path| path.join("workspace").canonicalize().ok())
            .and_then(|path| directory_key(&crate::agent::opencode_wire_directory(&path)));
        Self {
            current_generation: Arc::new(move || crate::sidecar_generation(&app)),
            default_directory,
            ..Self::default()
        }
        .start_worker(base)
    }

    #[cfg(test)]
    pub fn start(base: String) -> Self {
        let default_directory = std::env::var_os("YUME_PERMISSION_TEST_DIRECTORY")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::current_dir().ok())
            .and_then(|path| path.canonicalize().ok())
            .and_then(|path| directory_key(&crate::agent::opencode_wire_directory(&path)));
        Self {
            default_directory,
            ..Self::default()
        }
        .start_worker(base)
    }

    fn start_worker(self, base: String) -> Self {
        let worker = self.clone();
        std::thread::spawn(move || {
            while !worker.stopped.load(Ordering::Relaxed) {
                if let Err(error) = worker.read(&base) {
                    eprintln!("permission stream: {error}");
                }
                if let Ok(mut state) = worker.state.lock() {
                    state.disconnect();
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        });
        self
    }

    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Relaxed);
        if let Ok(mut state) = self.state.lock() {
            state.disconnect();
        }
    }

    fn begin_connection(&self, generation: u64) -> Result<(), String> {
        if (self.current_generation)()? != generation {
            return Err("permission_generation_changed".into());
        }
        let mut state = self.state.lock().map_err(|_| "permission_stream_lock")?;
        state.disconnect();
        state.generation = Some(generation);
        state.connected = true;
        Ok(())
    }

    fn read(&self, base: &str) -> Result<(), String> {
        let generation = (self.current_generation)()?;
        let request = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(3))
            .timeout_read(Duration::from_secs(45))
            .build()
            .get(&format!("{base}/global/event"));
        let request = match crate::sidecar_auth_value() {
            Some(value) => request.set("Authorization", value),
            None => request,
        };
        let response = request.call().map_err(|error| error.to_string())?;
        self.begin_connection(generation)?;
        self.read_frames(BufReader::new(response.into_reader()), generation)
    }

    fn read_frames(&self, mut reader: impl BufRead, generation: u64) -> Result<(), String> {
        let mut data = Vec::new();
        let mut line = Vec::new();
        loop {
            if self.stopped.load(Ordering::Relaxed) {
                return Ok(());
            }
            line.clear();
            let count = (&mut reader)
                .take((MAX_FRAME_BYTES + 1) as u64)
                .read_until(b'\n', &mut line)
                .map_err(|error| error.to_string())?;
            if count == 0 {
                return Ok(());
            }
            if (self.current_generation)()? != generation {
                return Err("permission_generation_changed".into());
            }
            if line.len() > MAX_FRAME_BYTES {
                self.malformed(generation);
                return Err("permission_event_too_large".into());
            }
            while matches!(line.last(), Some(b'\n' | b'\r')) {
                line.pop();
            }
            if line.is_empty() {
                if !data.is_empty() {
                    match serde_json::from_slice::<Value>(&data) {
                        Ok(event) => self.accept(generation, event),
                        Err(_) => {
                            self.malformed(generation);
                            return Err("permission_invalid_event".into());
                        }
                    }
                    if self
                        .state
                        .lock()
                        .map_err(|_| "permission_stream_lock")?
                        .permissions_invalid
                    {
                        return Err("permission_invalid_event".into());
                    }
                    data.clear();
                }
            } else if let Some(value) = line.strip_prefix(b"data:") {
                let value = value.strip_prefix(b" ").unwrap_or(value);
                if data.len() + value.len() + 1 > MAX_FRAME_BYTES {
                    self.malformed(generation);
                    return Err("permission_event_too_large".into());
                }
                if !data.is_empty() {
                    data.push(b'\n');
                }
                data.extend_from_slice(value);
            }
        }
    }

    fn malformed(&self, generation: u64) {
        if (self.current_generation)().ok() != Some(generation) {
            return;
        }
        if let Ok(mut state) = self.state.lock() {
            if state.generation == Some(generation) && state.connected {
                state.requests.clear();
                state.permissions_invalid = true;
                state.full();
            }
        }
    }

    fn accept(&self, generation: u64, envelope: Value) {
        if (self.current_generation)().ok() != Some(generation) {
            return;
        }
        if !envelope["workspace"].is_null() {
            return;
        }
        let event = &envelope["payload"];
        let Some(kind) = event["type"].as_str() else {
            self.malformed(generation);
            return;
        };
        // Durable sync mirrors reuse the normal event ID and must not consume it.
        if kind == "sync" || kind == "server.heartbeat" || kind == "server.connected" {
            return;
        }
        if kind == "global.disposed" {
            if let Ok(mut state) = self.state.lock() {
                if state.connected && state.generation == Some(generation) {
                    state.requests.clear();
                    state.settled.clear();
                    state.seen_permissions.clear();
                    state.full();
                }
            }
            return;
        }
        let relevant = kind.starts_with("message.")
            || kind.starts_with("session.")
            || kind.starts_with("permission.")
            || kind.starts_with("question.")
            || kind == "server.instance.disposed";
        if !relevant {
            return;
        }
        let Some(raw_directory) = envelope["directory"].as_str() else {
            self.malformed(generation);
            return;
        };
        if directory_key(raw_directory).is_none() {
            self.malformed(generation);
            return;
        }
        // Resolve Windows short-path aliases only for sparse permission/lifecycle events.
        let canonical_directory = if matches!(
            kind,
            "permission.asked" | "permission.replied" | "server.instance.disposed"
        ) {
            Path::new(raw_directory)
                .canonicalize()
                .ok()
                .map(|path| crate::agent::opencode_wire_directory(&path))
        } else {
            None
        };
        let Some(directory) =
            directory_key(canonical_directory.as_deref().unwrap_or(raw_directory))
        else {
            self.malformed(generation);
            return;
        };
        let properties = &event["properties"];
        let session = properties["sessionID"]
            .as_str()
            .or_else(|| properties["info"]["sessionID"].as_str())
            .or_else(|| properties["part"]["sessionID"].as_str());
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(_) => return,
        };
        if !state.connected || state.generation != Some(generation) {
            return;
        }
        if kind == "server.instance.disposed" {
            state.requests.retain(|(dir, _, _), _| dir != &directory);
            state.settled.retain(|(dir, _, _)| dir != &directory);
            state
                .seen_permissions
                .retain(|(dir, _, _)| dir != &directory);
            if state
                .active
                .as_ref()
                .is_some_and(|active| active.directory == directory)
            {
                state.full();
            }
            return;
        }
        if kind == "session.deleted" {
            if let Some(session) = session {
                state.requests.retain(|(dir, request_session, _), _| {
                    dir != &directory || request_session != session
                });
            }
        }
        if matches!(kind, "permission.asked" | "permission.replied") {
            Self::permission(&mut state, &directory, kind, event, session);
        }
        let revision = state.revision.saturating_add(1);
        let Some(active) = &mut state.active else {
            return;
        };
        if active.scope.generation != generation || active.directory != directory {
            return;
        }
        if session.is_some_and(|session| session != active.scope.session_id) {
            return;
        }
        if active.duplicate(kind, event["id"].as_str().filter(|id| valid_id(id))) {
            return;
        }
        let contradictory_session = [
            properties["info"]["sessionID"].as_str(),
            properties["part"]["sessionID"].as_str(),
        ]
        .into_iter()
        .flatten()
        .any(|nested| Some(nested) != session);
        if session.is_none() || contradictory_session {
            active.full(revision);
        } else {
            match kind {
                "message.updated" => {
                    match properties["info"]["id"].as_str().filter(|id| valid_id(id)) {
                        Some(id) => active.dirty(id, revision),
                        None => active.full(revision),
                    }
                }
                "message.part.updated" => match properties["part"]["messageID"]
                    .as_str()
                    .filter(|id| valid_id(id))
                {
                    Some(id) => active.dirty(id, revision),
                    None => active.full(revision),
                },
                "message.part.delta" => {
                    match properties["messageID"].as_str().filter(|id| valid_id(id)) {
                        Some(id) => active.dirty(id, revision),
                        None => active.full(revision),
                    }
                }
                "session.status" if properties["status"]["type"] == "busy" => {}
                "message.removed"
                | "message.part.removed"
                | "session.status"
                | "session.idle"
                | "session.error"
                | "session.deleted" => active.full(revision),
                kind if kind.starts_with("permission.") || kind.starts_with("question.") => {
                    active.full(revision);
                    if let Some(id) = properties["tool"]["messageID"]
                        .as_str()
                        .filter(|id| valid_id(id))
                    {
                        active.dirty(id, revision);
                    }
                }
                _ => {}
            }
        }
        state.revision = revision;
    }

    fn permission(
        state: &mut StreamState,
        directory: &str,
        kind: &str,
        event: &Value,
        session: Option<&str>,
    ) {
        if state.permissions_invalid {
            return;
        }
        if let Some(id) = event["id"].as_str().filter(|id| valid_id(id)) {
            let key = (directory.to_owned(), kind.to_owned(), id.to_owned());
            if state.seen_permissions.contains(&key) {
                return;
            }
            state.seen_permissions.push_back(key);
            if state.seen_permissions.len() > MAX_SEEN_PERMISSIONS {
                state.seen_permissions.pop_front();
            }
        }
        let properties = &event["properties"];
        match kind {
            "permission.asked" => {
                if let Ok(request) = serde_json::from_value::<PermissionRequest>(properties.clone())
                {
                    if valid_id(&request.id)
                        && valid_id(&request.session_id)
                        && request.metadata.is_object()
                    {
                        let key = (
                            directory.to_owned(),
                            request.session_id.clone(),
                            request.id.clone(),
                        );
                        if state.settled.contains(&key) {
                            return;
                        }
                        if state.requests.contains_key(&key)
                            || state.requests.len() < MAX_PENDING_PERMISSIONS
                        {
                            state.requests.insert(key, request);
                            return;
                        }
                    }
                }
            }
            "permission.replied" => {
                if let (Some(session), Some(id)) = (session, properties["requestID"].as_str()) {
                    if valid_id(session) && valid_id(id) {
                        let key = (directory.to_owned(), session.to_owned(), id.to_owned());
                        if state.settled.contains(&key)
                            || state.settled.len() < MAX_PENDING_PERMISSIONS
                        {
                            state.requests.remove(&key);
                            state.settled.insert(key);
                            return;
                        }
                    }
                }
            }
            _ => return,
        }
        state.requests.clear();
        state.permissions_invalid = true;
    }

    pub(crate) fn changes(&self, scope: &RefreshScope) -> Result<RefreshChanges, String> {
        let generation = (self.current_generation)()?;
        let directory = directory_key(&scope.directory).ok_or("permission_invalid_directory")?;
        let mut state = self.state.lock().map_err(|_| "permission_stream_lock")?;
        if state.generation != Some(generation) && state.connected {
            state.disconnect();
        }
        if state
            .active
            .as_ref()
            .is_none_or(|active| active.scope != *scope)
        {
            state.revision = state.revision.saturating_add(1);
            state.active = Some(ActiveRefresh::new(scope.clone(), directory, state.revision));
        }
        let active = state.active.as_ref().expect("active scope initialized");
        let connected = state.connected
            && state.generation == Some(scope.generation)
            && generation == scope.generation;
        Ok(RefreshChanges {
            ticket: state.revision,
            full: !connected || active.full_at.is_some(),
            connected,
            message_ids: active.dirty.keys().cloned().collect(),
        })
    }

    pub(crate) fn ack(&self, scope: &RefreshScope, ticket: u64) -> Result<(), String> {
        let generation = (self.current_generation)()?;
        let mut state = self.state.lock().map_err(|_| "permission_stream_lock")?;
        if generation != scope.generation || state.generation != Some(generation) {
            return Ok(());
        }
        if let Some(active) = state
            .active
            .as_mut()
            .filter(|active| active.scope == *scope)
        {
            active.ack(ticket);
        }
        Ok(())
    }

    pub(crate) fn deactivate(&self) -> Result<(), String> {
        self.state
            .lock()
            .map_err(|_| "permission_stream_lock")?
            .active = None;
        Ok(())
    }

    // The pinned list encoder rejects some omitted metadata. Fallback data must
    // originate from this live generation and the exact requested directory.
    pub(super) fn snapshot(&self) -> Result<Vec<PermissionRequest>, String> {
        self.snapshot_directory(
            self.default_directory
                .as_deref()
                .ok_or("permission_unavailable")?,
            None,
        )
    }

    fn snapshot_directory(
        &self,
        directory: &str,
        session: Option<&str>,
    ) -> Result<Vec<PermissionRequest>, String> {
        let generation = (self.current_generation)().map_err(|_| "permission_unavailable")?;
        let state = self.state.lock().map_err(|_| "permission_unavailable")?;
        if !state.connected || state.generation != Some(generation) || state.permissions_invalid {
            return Err("permission_unavailable".into());
        }
        let requests: Vec<_> = state
            .requests
            .iter()
            .filter(|((dir, request_session, _), _)| {
                dir == directory && session.is_none_or(|session| session == request_session)
            })
            .map(|(_, request)| request.clone())
            .collect();
        if requests.is_empty() {
            return Err("permission_unavailable".into());
        }
        Ok(requests)
    }

    pub(super) fn pending_scoped(
        &self,
        base: &str,
        directory: &Path,
        session: &str,
    ) -> Result<Vec<PermissionRequest>, String> {
        let key = directory_key(&crate::agent::opencode_wire_directory(directory))
            .ok_or("permission_invalid_directory")?;
        let requests = super::scoped::pending_scoped(base, directory).or_else(|error| {
            if error == "permission_invalid_metadata" {
                self.snapshot_directory(&key, Some(session))
                    .map_err(|_| error)
            } else {
                Err(error)
            }
        })?;
        requests
            .into_iter()
            .filter(|request| request.session_id == session)
            .map(|mut request| {
                if matches!(
                    request.permission.as_str(),
                    "read" | "glob" | "grep" | "list"
                ) {
                    let metadata = request
                        .metadata
                        .as_object_mut()
                        .ok_or_else(|| "permission_invalid_metadata".to_owned())?;
                    match metadata.get("path") {
                        None => {
                            metadata.insert(
                                "path".into(),
                                Value::String(crate::agent::opencode_wire_directory(directory)),
                            );
                        }
                        Some(Value::String(path)) if !path.is_empty() => {}
                        Some(_) => return Err("permission_invalid_metadata".into()),
                    }
                }
                Ok(request)
            })
            .collect()
    }
}

fn valid_id(id: &str) -> bool {
    id.len() <= 512 && crate::worklog::bridge::safe_id(id)
}

fn directory_key(value: &str) -> Option<String> {
    if value.len() > 32_768 || value.contains('\0') || !Path::new(value).is_absolute() {
        return None;
    }
    let value = crate::agent::opencode_wire_directory(Path::new(value));
    #[cfg(windows)]
    let value = value.replace('/', "\\").to_ascii_lowercase();
    Some(value)
}

#[cfg(test)]
#[path = "events_scoped_tests.rs"]
mod scoped_tests;

#[cfg(test)]
#[path = "events_refresh_tests.rs"]
mod tests;
