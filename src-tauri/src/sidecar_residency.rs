//! Reclaim only a completely idle engine. Persistent sessions remain in SQLite.
use std::{
    collections::{HashMap, HashSet},
    sync::{Condvar, Mutex, OnceLock},
    time::{Duration, Instant},
};
use tauri::Manager;
const IDLE: Duration = Duration::from_secs(10 * 60);
struct Gate {
    active: usize,
    retiring: bool,
    last_used: Instant,
    resident: HashSet<String>,
    retired: HashSet<String>,
}
impl Default for Gate {
    fn default() -> Self {
        Self {
            active: 0,
            retiring: false,
            last_used: Instant::now(),
            resident: HashSet::new(),
            retired: HashSet::new(),
        }
    }
}
impl Gate {
    fn begin(&mut self, now: Instant) -> bool {
        if self.active != 0
            || self.retiring
            || self.resident.is_empty()
            || now.saturating_duration_since(self.last_used) < IDLE
        {
            return false;
        }
        self.retiring = true;
        true
    }
    fn finish(&mut self, success: bool) {
        if success {
            self.retired.extend(self.resident.drain());
        }
        self.retiring = false;
    }
}
fn state() -> &'static (Mutex<Gate>, Condvar) {
    static STATE: OnceLock<(Mutex<Gate>, Condvar)> = OnceLock::new();
    STATE.get_or_init(|| (Mutex::new(Gate::default()), Condvar::new()))
}
static DEFAULT_DIRECTORY: OnceLock<String> = OnceLock::new();
pub(crate) struct Lease {
    foreground: bool,
}
impl Lease {
    pub(crate) fn for_url(url: &str, foreground: bool) -> Result<Option<Self>, String> {
        let parsed = url::Url::parse(url).map_err(|_| "residency endpoint invalid")?;
        let directory = parsed
            .query_pairs()
            .find(|(key, _)| key == "directory")
            .map(|(_, value)| value.into_owned())
            .or_else(|| DEFAULT_DIRECTORY.get().cloned());
        let lease = Self::acquire(foreground)?;
        if let Some(directory) = directory {
            if !foreground && retired(&directory) {
                return Ok(None);
            }
            lease.directory(&directory)?;
        }
        Ok(Some(lease))
    }
    pub(crate) fn acquire(foreground: bool) -> Result<Self, String> {
        let (lock, wake) = state();
        let mut gate = lock.lock().map_err(|_| "sidecar residency unavailable")?;
        while gate.retiring {
            gate = wake
                .wait(gate)
                .map_err(|_| "sidecar residency unavailable")?;
        }
        gate.active += 1;
        if foreground {
            gate.last_used = Instant::now();
        }
        Ok(Self { foreground })
    }
    pub(crate) fn directory(&self, directory: &str) -> Result<(), String> {
        let directory = crate::history::canonical_directory(directory);
        let mut gate = state()
            .0
            .lock()
            .map_err(|_| "sidecar residency unavailable")?;
        if !self.foreground && gate.retired.contains(&directory) {
            return Err("sidecar instance retired".into());
        }
        gate.retired.remove(&directory);
        gate.resident.insert(directory);
        Ok(())
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        if let Ok(mut gate) = state().0.lock() {
            gate.active = gate.active.saturating_sub(1);
            if self.foreground {
                gate.last_used = Instant::now();
            }
        }
    }
}
pub(crate) fn retired(directory: &str) -> bool {
    state()
        .0
        .lock()
        .map(|gate| {
            gate.retired
                .contains(&crate::history::canonical_directory(directory))
        })
        .unwrap_or(true)
}
pub(crate) fn restarted(directory: &str) {
    let directory = crate::history::canonical_directory(directory);
    let _ = DEFAULT_DIRECTORY.set(directory.clone());
    if let Ok(mut gate) = state().0.lock() {
        gate.retired.clear();
        gate.resident.clear();
        gate.resident.insert(directory);
        gate.last_used = Instant::now();
    }
}
struct Retirement {
    service: Option<std::sync::RwLockWriteGuard<'static, ()>>,
    success: bool,
}
impl Drop for Retirement {
    fn drop(&mut self) {
        drop(self.service.take());
        if let Ok(mut gate) = state().0.lock() {
            gate.finish(self.success);
            state().1.notify_all();
        }
    }
}

fn idle_payload(route: &str, value: &serde_json::Value) -> bool {
    if route == "/session/status" {
        value.as_object().is_some_and(|statuses| {
            statuses
                .values()
                .all(|status| status.get("type").and_then(|kind| kind.as_str()) == Some("idle"))
        })
    } else {
        value.as_array().is_some_and(Vec::is_empty)
    }
}
fn dispose_idle(base: &str, auth: &str, directories: &[String]) -> Result<bool, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(5))
        .redirects(0)
        .build();
    for directory in directories {
        for route in ["/session/status", "/pty", "/permission", "/question"] {
            let mut url = url::Url::parse(&format!("{base}{route}"))
                .map_err(|_| "residency endpoint invalid")?;
            url.query_pairs_mut().append_pair("directory", directory);
            let value: serde_json::Value = agent
                .get(url.as_str())
                .set("Authorization", auth)
                .call()
                .map_err(|_| "residency idle check failed")?
                .into_json()
                .map_err(|_| "residency idle response invalid")?;
            if !idle_payload(route, &value) {
                return Ok(false);
            }
        }
    }
    // Unlike /instance/dispose, the pinned /global/dispose awaits teardown before replying.
    let disposed: bool = agent
        .post(&format!("{base}/global/dispose"))
        .set("Authorization", auth)
        .timeout(Duration::from_secs(30))
        .call()
        .map_err(|_| "residency disposal failed")?
        .into_json()
        .map_err(|_| "residency disposal response invalid")?;
    Ok(disposed)
}
fn collect(app: &tauri::AppHandle) -> Result<(), String> {
    if app
        .state::<crate::agent::AgentRunState>()
        .read()?
        .active
        .is_some()
    {
        return Ok(());
    }
    for label in ["chat", "workbench"] {
        if let Some(window) = app.get_webview_window(label) {
            if window.is_visible().map_err(|e| e.to_string())? {
                return Ok(());
            }
        }
    }
    // Take admission and the existing restart boundary atomically. Once retiring,
    // no UI calls occur: a new foreground request may safely wait for disposal.
    let (mut directories, service) = {
        let mut gate = state()
            .0
            .lock()
            .map_err(|_| "sidecar residency unavailable")?;
        let Some(service) = crate::component_activation::idle_boundary()? else {
            return Ok(());
        };
        if !gate.begin(Instant::now()) {
            return Ok(());
        }
        (gate.resident.iter().cloned().collect::<Vec<_>>(), service)
    };
    let mut retirement = Retirement {
        service: Some(service),
        success: false,
    };
    directories.extend(
        crate::history::commands::store(app)?
            .directories()?
            .into_iter()
            .filter(|directory| !retired(directory) && std::path::Path::new(directory).is_dir()),
    );
    directories.sort();
    directories.dedup();
    state()
        .0
        .lock()
        .map_err(|_| "sidecar residency unavailable")?
        .resident
        .extend(directories.iter().cloned());
    let disposed = dispose_idle(
        &crate::sidecar_url(app),
        &crate::sidecar_auth_header(app),
        &directories,
    )?;
    retirement.success = disposed;
    Ok(())
}
pub(crate) fn start(app: tauri::AppHandle) {
    if let Ok(directory) = crate::history::commands::workspace(&app) {
        restarted(&directory);
    }
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(60));
        if let Err(error) = collect(&app) {
            eprintln!("sidecar idle collection deferred: {error}");
        }
    });
}

#[derive(Default)]
pub(crate) struct SidecarRequests(Mutex<HashMap<String, (String, Lease)>>);
pub(crate) fn release_page<R: tauri::Runtime>(app: &tauri::AppHandle<R>, label: &str) {
    if let Some(requests) = app.try_state::<SidecarRequests>() {
        if let Ok(mut requests) = requests.0.lock() {
            requests.retain(|_, (owner, _)| owner != label);
        }
    }
}
#[tauri::command]
pub(crate) async fn sidecar_request_begin(
    window: tauri::WebviewWindow,
    directory: Option<String>,
    background: bool,
) -> Result<String, String> {
    if !matches!(window.label(), "workbench" | "chat" | "settings") {
        return Err("forbidden".into());
    }
    let app = window.app_handle().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let directory = directory.unwrap_or(crate::history::commands::workspace(&app)?);
        if background && !window.is_visible().map_err(|e| e.to_string())? && retired(&directory) {
            return Err("sidecar_instance_retired".into());
        }
        let lease = Lease::acquire(true)?;
        if background && !window.is_visible().map_err(|e| e.to_string())? && retired(&directory) {
            return Err("sidecar_instance_retired".into());
        }
        lease.directory(&directory)?;
        let token = uuid::Uuid::new_v4().to_string();
        app.state::<SidecarRequests>()
            .0
            .lock()
            .map_err(|_| "sidecar residency unavailable")?
            .insert(token.clone(), (window.label().to_owned(), lease));
        Ok(token)
    })
    .await
    .map_err(|_| "sidecar residency unavailable".to_string())?
}
#[tauri::command]
pub(crate) fn sidecar_request_end(
    window: tauri::WebviewWindow,
    token: String,
) -> Result<(), String> {
    if !matches!(window.label(), "workbench" | "chat" | "settings") {
        return Err("forbidden".into());
    }
    let state = window.state::<SidecarRequests>();
    let mut requests = state
        .0
        .lock()
        .map_err(|_| "sidecar residency unavailable")?;
    if requests
        .get(&token)
        .is_some_and(|(owner, _)| owner != window.label())
    {
        return Err("forbidden".into());
    }
    requests.remove(&token);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn server(bodies: Vec<&str>) -> (String, std::thread::JoinHandle<Vec<String>>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let bodies = bodies.into_iter().map(str::to_owned).collect::<Vec<_>>();
        let worker = std::thread::spawn(move || {
            bodies.into_iter().map(|body| {
            let deadline=Instant::now()+Duration::from_secs(5);
            let mut stream=loop {
                match listener.accept() {
                    Ok((stream,_))=>break stream,
                    Err(error) if error.kind()==std::io::ErrorKind::WouldBlock && Instant::now()<deadline=>std::thread::sleep(Duration::from_millis(2)),
                    Err(error)=>panic!("fixture accept failed: {error}"),
                }
            };
            stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            let mut request=Vec::new();
            while !request.ends_with(b"\r\n\r\n") { let mut byte=[0];stream.read_exact(&mut byte).unwrap();request.push(byte[0]); }
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",body.len()).unwrap();
            String::from_utf8(request).unwrap()
        }).collect()
        });
        (base, worker)
    }
    #[test]
    fn http_idle_checks_are_scoped_and_disposal_only_follows_all_four_checks() {
        let (base, server) = server(vec!["{}", "[]", "[]", "[]", "true"]);
        assert!(dispose_idle(&base, "Basic fixture", &["C:/synthetic space".into()]).unwrap());
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 5);
        for (request, route) in
            requests
                .iter()
                .zip(["/session/status", "/pty", "/permission", "/question"])
        {
            assert!(request.starts_with(&format!("GET {route}?directory=C%3A%2Fsynthetic+space ")));
            assert!(request.contains("Authorization: Basic fixture"));
        }
        assert!(requests[4].starts_with("POST /global/dispose "));
    }
    #[test]
    fn http_busy_and_pending_work_never_send_dispose() {
        for responses in [
            vec!["{\"s\":{\"type\":\"busy\"}}"],
            vec!["{}", "[{\"id\":\"terminal\"}]"],
            vec!["{}", "[]", "[{\"id\":\"approval\"}]"],
            vec!["{}", "[]", "[]", "[{\"id\":\"question\"}]"],
        ] {
            let expected = responses.len();
            let (base, server) = server(responses);
            assert!(!dispose_idle(&base, "Basic fixture", &["C:/fixture".into()]).unwrap());
            assert_eq!(server.join().unwrap().len(), expected);
        }
        let (base, server) = server(vec!["not-json"]);
        assert!(dispose_idle(&base, "Basic fixture", &["C:/fixture".into()]).is_err());
        assert_eq!(server.join().unwrap().len(), 1);
    }
    #[test]
    fn one_ten_thirty_directories_retire_and_reopen_without_growing_live_sets() {
        for count in [1, 10, 30] {
            let now = Instant::now();
            let mut gate = Gate::default();
            gate.last_used = now;
            for n in 0..count {
                gate.resident.insert(format!("dir-{n}"));
            }
            assert!(!gate.begin(now + IDLE - Duration::from_secs(1)));
            gate.active = 1;
            assert!(!gate.begin(now + IDLE));
            gate.active = 0;
            assert!(gate.begin(now + IDLE));
            assert!(!gate.begin(now + IDLE));
            gate.finish(true);
            assert!(gate.resident.is_empty());
            assert_eq!(gate.retired.len(), count);
            gate.retired.remove("dir-0");
            gate.resident.insert("dir-0".into());
            assert_eq!(gate.resident.len(), 1);
        }
    }
    #[test]
    fn malformed_busy_retry_pty_approval_and_questions_prevent_disposal() {
        for payload in [
            serde_json::json!(null),
            serde_json::json!([]),
            serde_json::json!({"s":{"type":"busy"}}),
            serde_json::json!({"s":{"type":"retry"}}),
            serde_json::json!({"s":{}}),
        ] {
            assert!(!idle_payload("/session/status", &payload));
        }
        assert!(idle_payload(
            "/session/status",
            &serde_json::json!({"s":{"type":"idle"}})
        ));
        for route in ["/pty", "/permission", "/question"] {
            assert!(!idle_payload(route, &serde_json::json!([{"id":"active"}])));
            assert!(!idle_payload(route, &serde_json::json!({})));
            assert!(idle_payload(route, &serde_json::json!([])));
        }
    }
}
