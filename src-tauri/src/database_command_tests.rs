use crate::{memory, worklog};
use serde_json::{json, Value};
use std::{
    sync::{mpsc, Arc, Mutex},
    time::Duration,
};
use tauri::{
    ipc::{InvokeBody, InvokeResponse},
    test::MockRuntime,
    Listener, Manager,
};

fn fixture() -> (tauri::App<MockRuntime>, tauri::WebviewWindow<MockRuntime>) {
    let app = tauri::test::mock_builder()
        .manage(memory::MemoryState(Mutex::new(Some(
            memory::repository::MemoryRepository::new(
                memory::storage::MemoryStore::open_in_memory().unwrap(),
                memory::repository::SystemClock,
            ),
        ))))
        .manage(worklog::WorklogState(Mutex::new(Some(Arc::new(
            worklog::repository::Repository::new(worklog::storage::WorklogStore::memory().unwrap()),
        )))))
        .invoke_handler(tauri::generate_handler![
            memory::commands::memory_list,
            memory::commands::memory_create,
            memory::commands::memory_update,
            memory::commands::memory_export,
            memory::commands::memory_forget,
            memory::commands::memory_available,
            memory::commands::memory_forget_conversation,
            worklog::commands::worklog_query,
            worklog::commands::worklog_get_entry,
            worklog::commands::worklog_record,
            worklog::commands::worklog_get_operation,
            worklog::commands::worklog_delete_entry,
            worklog::commands::worklog_available,
            worklog::commands::worklog_get_operation_states
        ])
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    let window = tauri::WebviewWindowBuilder::new(&app, "settings", Default::default())
        .build()
        .unwrap();
    (app, window)
}

fn request(cmd: &str, body: Value) -> tauri::webview::InvokeRequest {
    tauri::webview::InvokeRequest {
        cmd: cmd.into(),
        callback: tauri::ipc::CallbackFn(0),
        error: tauri::ipc::CallbackFn(1),
        url: if cfg!(windows) {
            "http://tauri.localhost"
        } else {
            "tauri://localhost"
        }
        .parse()
        .unwrap(),
        body: InvokeBody::Json(body),
        headers: Default::default(),
        invoke_key: tauri::test::INVOKE_KEY.into(),
    }
}

// Release the held database state even if dispatch regresses, so the test fails instead of hanging.
fn dispatch_while_locked(
    window: tauri::WebviewWindow<MockRuntime>,
    request: tauri::webview::InvokeRequest,
    release: impl FnOnce(),
) {
    let (tick_tx, tick_rx) = mpsc::channel();
    let (reply_tx, reply_rx) = mpsc::channel();
    let dispatcher = std::thread::spawn(move || {
        window.on_message(
            request,
            Box::new(move |_, _, reply, _, _| {
                reply_tx
                    .send(match reply {
                        InvokeResponse::Ok(body) => Ok(body.deserialize::<Value>().unwrap()),
                        InvokeResponse::Err(error) => Err(error.0),
                    })
                    .unwrap();
            }),
        );
        tick_tx.send(()).unwrap();
    });
    let responsive = tick_rx.recv_timeout(Duration::from_secs(2)).is_ok();
    let pending = matches!(reply_rx.try_recv(), Err(mpsc::TryRecvError::Empty));
    release();
    let reply = reply_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    dispatcher.join().unwrap();
    assert!(
        responsive,
        "database wait blocked the generated IPC dispatcher"
    );
    assert!(pending, "database command must wait for its store lock");
    assert_eq!(reply, Ok(json!([])));
}

#[test]
fn memory_lock_wait_does_not_block_ipc_dispatch() {
    let (app, window) = fixture();
    let state = app.state::<memory::MemoryState>();
    let guard = state.0.lock().unwrap();
    dispatch_while_locked(window, request("memory_list", json!({"query":{}})), || {
        drop(guard)
    });
}

#[test]
fn worklog_lock_wait_does_not_block_ipc_dispatch() {
    let (app, window) = fixture();
    let state = app.state::<worklog::WorklogState>();
    let guard = state.0.lock().unwrap();
    dispatch_while_locked(
        window,
        request(
            "worklog_query",
            json!({"query":{"start":"2026-11-01","end":"2026-11-09","project":null}}),
        ),
        || drop(guard),
    );
}

fn invoke(window: &tauri::WebviewWindow<MockRuntime>, cmd: &str, body: Value) -> Result<Value, Value> {
    tauri::test::get_ipc_response(window, request(cmd, body)).map(|body| body.deserialize().unwrap())
}

fn create_memory(window: &tauri::WebviewWindow<MockRuntime>) -> Value {
    invoke(window, "memory_create", json!({"memory":{
        "scope":"global","type":"preference","content":"Prefer concise explanations","sourceKind":"explicit"
    }})).unwrap()
}

#[test]
fn memory_ipc_keeps_commit_before_event_export_and_forget_semantics() {
    let (app, window) = fixture();
    let handle = app.handle().clone();
    let (tx, rx) = mpsc::channel();
    app.listen(memory::commands::MEMORY_CHANGED_EVENT, move |event| {
        let event: Value = serde_json::from_str(event.payload()).unwrap();
        let state = handle.state::<memory::MemoryState>();
        let guard = state.0.lock().unwrap();
        let count = guard.as_ref().unwrap().list(&Default::default()).unwrap().len();
        tx.send((event, count)).unwrap();
    });
    let memory = create_memory(&window);
    let (event, count) = rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(count,1);
    assert!(!event.to_string().contains("Prefer concise"), "change events must stay content-free");
    let export = invoke(&window,"memory_export",json!({})).unwrap();
    assert_eq!(export["memories"][0]["id"],memory["id"]);
    assert_eq!(export["schemaVersion"],1);
    invoke(&window,"memory_forget",json!({"id":memory["id"]})).unwrap();
    assert_eq!(rx.recv_timeout(Duration::from_secs(2)).unwrap().1,0);
    assert_eq!(invoke(&window,"memory_export",json!({})).unwrap()["memories"],json!([]));
}

#[test]
fn concurrent_memory_ipc_updates_keep_revision_conflicts() {
    let (_app, window) = fixture();
    let memory = create_memory(&window);
    let update = |content| json!({"update":{"id":memory["id"],"content":content,"expectedRevision":memory["revision"]}});
    let replies = std::thread::scope(|scope| {
        let a = scope.spawn(|| invoke(&window,"memory_update",update("Prefer short paragraphs")));
        let b = scope.spawn(|| invoke(&window,"memory_update",update("Prefer compact bullet points")));
        [a.join().unwrap(),b.join().unwrap()]
    });
    assert_eq!(replies.iter().filter(|reply| reply.is_ok()).count(),1);
    assert_eq!(replies.iter().find_map(|reply| reply.as_ref().err()).unwrap()["code"],"CONFLICT");
    assert_eq!(invoke(&window,"memory_list",json!({"query":{}})).unwrap()[0]["revision"],2);
}

#[test]
fn worklog_ipc_preserves_idempotency_receipts_and_guarded_deletion() {
    let (_app, window) = fixture();
    let request_id = uuid::Uuid::new_v4().to_string();
    let entry = json!({"request":{"requestId":request_id,"businessDate":"2026-11-09","project":null,
        "originalText":"Finished the synthetic task","text":"Finished the synthetic task","status":"done",
        "sourceSessionId":null,"sourceMessageId":null}});
    let receipt = invoke(&window,"worklog_record",entry.clone()).unwrap();
    assert_eq!(invoke(&window,"worklog_record",entry).unwrap(),receipt);
    assert_eq!(invoke(&window,"worklog_get_operation",json!({"requestId":request_id})).unwrap(),receipt);
    let query = json!({"query":{"start":"2026-11-09","end":"2026-11-09","project":null}});
    assert_eq!(invoke(&window,"worklog_query",query.clone()).unwrap().as_array().unwrap().len(),1);
    let remove = |revision| json!({"request":{"requestId":uuid::Uuid::new_v4().to_string(),"id":receipt["entityId"],"expectedRevision":revision,"deleteLinkedReports":false}});
    assert!(invoke(&window,"worklog_delete_entry",remove(0)).is_err());
    assert_eq!(invoke(&window,"worklog_query",query.clone()).unwrap().as_array().unwrap().len(),1);
    invoke(&window,"worklog_delete_entry",remove(receipt["revision"].as_i64().unwrap())).unwrap();
    assert_eq!(invoke(&window,"worklog_query",query).unwrap(),json!([]));
}

#[test]
fn database_ipc_preserves_disabled_errors_and_window_authorization() {
    let (app, window) = fixture();
    assert_eq!(invoke(&window,"worklog_get_operation_states",json!({"requestIds":[]})).unwrap_err()["code"],"FORBIDDEN");
    assert_eq!(invoke(&window,"memory_forget_conversation",json!({"conversationId":"ses_one","catalogKey":"native:invalid"})).unwrap_err()["code"],"VALIDATION_FAILED");
    *app.state::<memory::MemoryState>().0.lock().unwrap() = None;
    *app.state::<worklog::WorklogState>().0.lock().unwrap() = None;
    assert_eq!(invoke(&window,"memory_available",json!({})).unwrap(),false);
    assert_eq!(invoke(&window,"worklog_available",json!({})).unwrap(),false);
    assert_eq!(invoke(&window,"memory_list",json!({"query":{}})).unwrap_err()["code"],"MEMORY_DISABLED");
    assert_eq!(invoke(&window,"worklog_query",json!({"query":{"start":"2026-11-09","end":"2026-11-09","project":null}})).unwrap_err()["code"],"WORKLOG_DISABLED");
}

#[test]
fn worklog_entry_lookup_returns_one_complete_record_null_after_deletion_and_rejects_other_windows() {
    let (app, window) = fixture();
    let entry = json!({"request":{
        "requestId":uuid::Uuid::new_v4().to_string(),"businessDate":"2026-11-09","project":"YUME",
        "originalText":"Kept original","text":"Current entry","status":"done",
        "sourceSessionId":"session","sourceMessageId":"message"
    }});
    let receipt = invoke(&window,"worklog_record",entry).unwrap();
    let query = json!({"id":receipt["entityId"]});
    let loaded = invoke(&window,"worklog_get_entry",query.clone()).unwrap();
    assert_eq!(loaded["originalText"],"Kept original");
    assert_eq!(loaded["sourceMessageId"],"message");
    assert_eq!(loaded["revision"],1);
    assert!(invoke(&window,"worklog_get_entry",json!({"id":"' OR 1=1 --"})).unwrap().is_null());
    let chat = tauri::WebviewWindowBuilder::new(&app,"chat",Default::default()).build().unwrap();
    assert_eq!(invoke(&chat,"worklog_get_entry",query.clone()).unwrap_err()["code"],"FORBIDDEN");
    invoke(&window,"worklog_delete_entry",json!({"request":{
        "requestId":uuid::Uuid::new_v4().to_string(),"id":receipt["entityId"],"expectedRevision":1,"deleteLinkedReports":false
    }})).unwrap();
    assert!(invoke(&window,"worklog_get_entry",query).unwrap().is_null());
    *app.state::<worklog::WorklogState>().0.lock().unwrap() = None;
    assert_eq!(invoke(&window,"worklog_get_entry",json!({"id":"missing"})).unwrap_err()["code"],"WORKLOG_DISABLED");
}
