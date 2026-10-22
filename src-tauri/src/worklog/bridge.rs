use super::error::{WorklogError, WorklogResult};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tauri::Manager;

#[path = "bridge_auth.rs"]
pub mod auth;
#[path = "bridge_dispatch.rs"]
mod dispatch;
#[path = "bridge_worker.rs"]
mod worker;

#[derive(Clone)]
pub struct Launch {
    pub directory: PathBuf,
    pub endpoint: String,
    // Host memory only; never serialized into tool requests or responses.
    auth_header: String,
}
type TurnKey = (String, String, String);
struct RegisteredTurn {
    grant: Option<auth::Grant>,
    created: std::time::Instant,
    received_date: chrono::NaiveDate,
}
#[derive(Default)]
struct Registrations {
    active: HashMap<TurnKey, RegisteredTurn>,
    // Fixed-size identity tombstones survive revocation/expiry until this launch ends.
    seen: HashSet<[u8; 32]>,
}
fn turn_fingerprint(key: &TurnKey) -> [u8; 32] {
    use sha2::Digest;
    let mut hash = sha2::Sha256::new();
    for value in [&key.0, &key.1, &key.2] {
        hash.update((value.len() as u64).to_le_bytes());
        hash.update(value.as_bytes());
    }
    hash.finalize().into()
}
pub struct WorklogBridge {
    root: Option<PathBuf>,
    launch: Mutex<Option<Launch>>,
    grants: Mutex<Registrations>,
    wake: Arc<worker::Wake>,
}
impl WorklogBridge {
    pub fn initialize(app: &tauri::AppHandle) -> Self {
        let root = app
            .path()
            .app_data_dir()
            .ok()
            .map(|path| path.join("worklog-ipc"));
        if let Some(root) = &root {
            if root.exists() && worker::cleanup_stale(root).is_err() {
                eprintln!("Worklog IPC cleanup skipped unsafe or unavailable files");
            }
        }
        Self {
            root,
            launch: Mutex::new(None),
            grants: Mutex::new(Registrations::default()),
            wake: Arc::new(worker::Wake::default()),
        }
    }
    pub fn reset_launch(&self, endpoint: String, auth_header: String) -> WorklogResult<PathBuf> {
        // A restart attempt retires its predecessor even if validation or IPC setup fails.
        let mut launch = self.launch.lock().map_err(|_| unavailable())?;
        *launch = None;
        self.wake.notify();
        *self.grants.lock().map_err(|_| unavailable())? = Registrations::default();
        crate::history::native_api::NativeHistoryClient::new(&endpoint, &auth_header)
            .map_err(|_| WorklogError::validation("Invalid managed sidecar connection"))?;
        let root = self.root.as_ref().ok_or_else(unavailable)?;
        std::fs::create_dir_all(root).map_err(|_| unavailable())?;
        worker::assert_plain(root, true)?;
        let directory = root.join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir(&directory).map_err(|_| unavailable())?;
        *launch = Some(Launch {
            directory: directory.clone(),
            endpoint,
            auth_header,
        });
        Ok(directory)
    }
    pub fn shutdown(&self) {
        self.wake.stop();
    }
    pub fn register(
        &self,
        directory: String,
        session: String,
        message: String,
        text: String,
    ) -> WorklogResult<()> {
        self.register_turn(directory, session, message, Some(auth::grant(&text)?))
    }
    pub(crate) fn launch_id(&self) -> WorklogResult<PathBuf> {
        self.launch
            .lock()
            .map_err(|_| unavailable())?
            .as_ref()
            .map(|launch| launch.directory.clone())
            .ok_or_else(unavailable)
    }
    pub(crate) fn register_native(
        &self,
        launch_id: &std::path::Path,
        directory: String,
        session: String,
        message: String,
    ) -> WorklogResult<()> {
        self.register_turn_for_launch(Some(launch_id), directory, session, message, None)
    }
    fn register_turn(
        &self,
        directory: String,
        session: String,
        message: String,
        grant: Option<auth::Grant>,
    ) -> WorklogResult<()> {
        self.register_turn_for_launch(None, directory, session, message, grant)
    }
    fn register_turn_for_launch(
        &self,
        launch_id: Option<&std::path::Path>,
        directory: String,
        session: String,
        message: String,
        grant: Option<auth::Grant>,
    ) -> WorklogResult<()> {
        let directory = canonical_directory(&directory)?;
        if !safe_id(&session) || !safe_id(&message) {
            return Err(WorklogError::validation("Invalid conversation identity"));
        }
        let _launch = self.launch.lock().map_err(|_| unavailable())?;
        if _launch.is_none() {
            return Err(unavailable());
        }
        if launch_id
            .is_some_and(|id| _launch.as_ref().map(|launch| launch.directory.as_path()) != Some(id))
        {
            return Err(WorklogError::new("STALE_CONTEXT", "Sidecar was restarted"));
        }
        let mut grants = self.grants.lock().map_err(|_| unavailable())?;
        let key = (directory.clone(), session.clone(), message);
        // Plugin/host duplicate registration cannot reset a failed query or revive an old turn.
        if !grants.seen.insert(turn_fingerprint(&key)) {
            return Ok(());
        }
        grants
            .active
            .retain(|(registered_directory, registered_session, _), turn| {
                (registered_directory != &directory || registered_session != &session)
                    && turn.created.elapsed().as_secs() < 1800
            });
        grants.active.insert(
            key,
            RegisteredTurn {
                received_date: grant
                    .as_ref()
                    .map(|grant| grant.received_date)
                    .unwrap_or_else(|| {
                        crate::worklog::calendar::business_date(chrono::Local::now().naive_local())
                    }),
                grant,
                created: std::time::Instant::now(),
            },
        );
        Ok(())
    }
}
fn canonical_directory(directory: &str) -> WorklogResult<String> {
    crate::history::catalog_model::canonical_directory(directory)
        .map_err(|_| WorklogError::validation("Invalid workspace identity"))?;
    let path = std::path::Path::new(directory);
    let path = path.canonicalize().unwrap_or_else(|_| path.to_owned());
    crate::history::catalog_model::canonical_directory(&path.to_string_lossy())
        .map_err(|_| WorklogError::validation("Invalid workspace identity"))
}

pub fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}
pub fn unavailable() -> WorklogError {
    WorklogError::new("BRIDGE_UNAVAILABLE", "Work journal bridge is unavailable")
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub version: u32,
    pub request_id: String,
    pub directory: String,
    pub session_id: String,
    pub message_id: String,
    pub call_id: String,
    pub action: String,
    pub args: serde_json::Value,
}

#[tauri::command]
pub fn worklog_register_turn(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: tauri::State<'_, WorklogBridge>,
    session_id: String,
    message_id: String,
    user_text: String,
    directory: String,
) -> WorklogResult<()> {
    crate::tool_permissions::runtime::require_chat(&window)
        .map_err(|_| WorklogError::new("FORBIDDEN", "Only the chat window can register a turn"))?;
    let directory = canonical_directory(&directory)?;
    let directory = crate::history::commands::authorize_directory(&app, Some(&directory))
        .map_err(|_| WorklogError::new("INVALID_CONTEXT", "Unknown workspace"))?;
    state.register(directory, session_id, message_id, user_text)
}
pub fn start_worker(app: tauri::AppHandle) {
    worker::start(app);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn registered_launch() -> WorklogBridge {
        WorklogBridge {
            root: None,
            launch: Mutex::new(Some(Launch {
                directory: PathBuf::from("fixture-launch"),
                endpoint: "http://127.0.0.1:1".into(),
                auth_header: "Basic fixture".into(),
            })),
            grants: Mutex::new(Registrations::default()),
            wake: Arc::new(worker::Wake::default()),
        }
    }
    #[test]
    fn followup_keeps_current_user_source_separate_from_context() {
        let state = registered_launch();
        state
            .register(
                "/workspace/a".into(),
                "ses_test".into(),
                "msg_first".into(),
                "昨天完成了登录联调".into(),
            )
            .expect("context");
        state
            .register(
                "/workspace/a".into(),
                "ses_test".into(),
                "msg_followup".into(),
                "我应该是记过了，你看看，没记过就记一下".into(),
            )
            .expect("followup");
        let grants = state.grants.lock().expect("grants");
        let grant = grants.active[&(
            "/workspace/a".into(),
            "ses_test".into(),
            "msg_followup".into(),
        )]
            .grant
            .as_ref()
            .unwrap();
        assert!(!grant.text.contains("昨天"));
        assert_eq!(grant.original, "我应该是记过了，你看看，没记过就记一下");
        assert!(!grant.query_failed.get());
        assert!(grant.last_query.borrow().is_none());
    }

    #[test]
    fn explicit_followup_date_overrides_previous_context() {
        let state = registered_launch();
        state
            .register(
                "/workspace/a".into(),
                "ses_test".into(),
                "msg_first".into(),
                "昨天完成了登录联调".into(),
            )
            .expect("context");
        state
            .register(
                "/workspace/a".into(),
                "ses_test".into(),
                "msg_followup".into(),
                "看看今天的工作日志，没记过就记一下".into(),
            )
            .expect("followup");
        let grants = state.grants.lock().expect("grants");
        assert!(!grants.active[&(
            "/workspace/a".into(),
            "ses_test".into(),
            "msg_followup".into()
        )]
            .grant
            .as_ref()
            .unwrap()
            .text
            .contains("昨天"));
    }

    #[test]
    fn new_turn_revokes_previous_session_grant() {
        let state = registered_launch();
        state
            .register(
                "/workspace/a".into(),
                "ses_test".into(),
                "msg_save".into(),
                "保存到工作记录".into(),
            )
            .expect("register");
        state
            .register(
                "/workspace/a".into(),
                "ses_test".into(),
                "msg_chat".into(),
                "你好".into(),
            )
            .expect("register");
        let grants = state.grants.lock().expect("grants");
        assert!(!grants.active.contains_key(&(
            "/workspace/a".into(),
            "ses_test".into(),
            "msg_save".into()
        )));
        assert!(
            grants.active[&("/workspace/a".into(), "ses_test".into(), "msg_chat".into())]
                .grant
                .as_ref()
                .unwrap()
                .actions
                .is_empty()
        );
    }
}
