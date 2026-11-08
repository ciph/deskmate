use super::*;
use crate::{agent::{AgentRunState, RunStore}, settings::ConversationModelSelection};
use serde_json::{json, Value};
use std::{sync::{mpsc, Mutex}, time::Duration};
use tauri::{ipc::{InvokeBody, InvokeResponse}, test::MockRuntime};

struct Fixture {
    app: tauri::App<MockRuntime>,
    chat: tauri::WebviewWindow<MockRuntime>,
    catalog: CatalogStore,
    entry: CatalogEntry,
    root: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let probe = tauri::test::mock_app();
        let root = tempfile::Builder::new().prefix("yume-model-ipc-")
            .tempdir_in(probe.path().data_dir().unwrap()).unwrap();
        let mut context = tauri::test::mock_context(tauri::test::noop_assets());
        context.config_mut().identifier = root.path().file_name().unwrap().to_str().unwrap().into();
        let app = tauri::test::mock_builder()
            .manage(AgentRunState::new(RunStore::new(root.path().join("runs"))))
            .manage(crate::settings::SettingsState(Mutex::new(Default::default())))
            .manage(crate::history::maintenance::HistoryMaintenance::default())
            .invoke_handler(tauri::generate_handler![history_model_selection_set])
            .build(context).unwrap();
        assert_eq!(app.path().app_data_dir().unwrap(), root.path());
        let chat = tauri::WebviewWindowBuilder::new(&app, "chat", Default::default()).build().unwrap();
        let catalog = store(app.handle()).unwrap();
        let mut entry = catalog_import::new_entry(CatalogIdentity::Native {
            sidecar_id: SIDECAR_ID.into(),
            directory: canonical_directory(&root.path().to_string_lossy()).unwrap(),
            session_id: "ses_target".into(),
        }, "Target".into(), ConversationSource::LightChat, 1, 2);
        entry.availability = Availability::Available;
        entry.runtime = RuntimeState::Idle;
        catalog.update_entry(&entry.key(), |row| { *row = Some(entry.clone()); Ok(()) }).unwrap();
        catalog.set_model_selection(&entry.key(), &old_selection()).unwrap();
        Self { app, chat, catalog, entry, root }
    }

    fn start(&self, session: &str) {
        let runs = self.app.state::<AgentRunState>();
        runs.begin("msg_run", self.root.path(), "Task").unwrap();
        runs.bind_session("msg_run", session).unwrap();
        runs.confirm_submission("msg_run").unwrap();
    }

    fn request(&self, selection: ConversationModelSelection) -> tauri::webview::InvokeRequest {
        tauri::webview::InvokeRequest {
            cmd: "history_model_selection_set".into(),
            callback: tauri::ipc::CallbackFn(0), error: tauri::ipc::CallbackFn(1),
            url: if cfg!(windows) { "http://tauri.localhost" } else { "tauri://localhost" }.parse().unwrap(),
            body: InvokeBody::Json(json!({ "key": self.entry.key(), "selection": selection })),
            headers: Default::default(), invoke_key: tauri::test::INVOKE_KEY.into(),
        }
    }
}

fn old_selection() -> ConversationModelSelection {
    ConversationModelSelection::Override {
        configured_provider_id: "old-provider".into(), sidecar_id: "old-runtime".into(), model_id: "old-model".into(),
    }
}

// Exercise the generated Tauri IPC wrapper, not a helper that could hide a synchronous dispatch regression.
fn selection_while_locked(start_target_before_release: bool) {
    let f = Fixture::new();
    if !start_target_before_release { f.start("ses_other"); }
    let runs = f.app.state::<AgentRunState>();
    let operation = runs.lock_operation().unwrap();
    let (tick_tx, tick_rx) = mpsc::channel();
    let (reply_tx, reply_rx) = mpsc::channel();
    let request = f.request(ConversationModelSelection::Inherit);
    let chat = f.chat.clone();
    let dispatcher = std::thread::spawn(move || {
        chat.on_message(request, Box::new(move |_, _, response, _, _| {
            let response = match response {
                InvokeResponse::Ok(body) => Ok(body.deserialize::<Value>().unwrap()),
                InvokeResponse::Err(error) => Err(error.0),
            };
            reply_tx.send(response).unwrap();
        }));
        // The next UI-dispatch operation must run while the Agent still owns its lock.
        tick_tx.send(()).unwrap();
    });
    let responsive = tick_rx.recv_timeout(Duration::from_secs(2)).is_ok();
    let pending = matches!(reply_rx.try_recv(), Err(mpsc::TryRecvError::Empty));
    let before_release = f.catalog.model_selection(&f.entry.key()).unwrap();
    if start_target_before_release { f.start("ses_target"); }
    // Always release before assertions so a synchronous regression fails instead of hanging the suite.
    drop(operation);
    let response = reply_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    dispatcher.join().unwrap();
    assert!(responsive, "model selection blocked the IPC dispatch thread while waiting for the Agent lock");
    assert!(pending, "selection must remain serialized with Agent operations");
    assert_eq!(before_release, old_selection());
    if start_target_before_release {
        assert_eq!(response, Err(json!("history_model_selection_locked")));
        assert_eq!(f.catalog.model_selection(&f.entry.key()).unwrap(), old_selection());
    } else {
        assert_eq!(response, Ok(Value::Null));
        assert_eq!(f.catalog.model_selection(&f.entry.key()).unwrap(), ConversationModelSelection::Inherit);
        assert_eq!(runs.read().unwrap().active.unwrap().session_id.as_deref(), Some("ses_other"));
    }
}

#[test]
fn waiting_for_an_unrelated_task_does_not_block_ipc_dispatch() {
    selection_while_locked(false);
}

#[test]
fn task_started_while_selection_waits_is_rechecked_before_writing() {
    selection_while_locked(true);
}

#[test]
fn model_selection_ipc_rejects_other_windows_and_unverified_models() {
    let f = Fixture::new();
    let pet = tauri::WebviewWindowBuilder::new(&f.app, "pet", Default::default()).build().unwrap();
    assert!(matches!(tauri::test::get_ipc_response(&pet, f.request(ConversationModelSelection::Inherit)),
        Err(error) if error == json!("permission_chat_only")));
    assert!(tauri::test::get_ipc_response(&f.chat, f.request(old_selection())).is_err());
    assert_eq!(f.catalog.model_selection(&f.entry.key()).unwrap(), old_selection());
}
