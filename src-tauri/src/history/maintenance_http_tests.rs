use super::*;
use crate::agent::{AgentRunState, RunOutcome, RunStore};
use serde_json::json;
use std::{io::{Read, Write}, net::TcpListener, sync::{mpsc, RwLock}, time::{Duration, Instant}};
use tauri::test::MockRuntime;

struct Fixture {
    app: tauri::App<MockRuntime>,
    store: super::super::catalog::CatalogStore,
    entry: CatalogEntry,
    root: tempfile::TempDir,
}

impl Fixture {
    fn new(port: u16) -> Self {
        let probe = tauri::test::mock_app();
        let root = tempfile::Builder::new().prefix("yume-maintenance-").tempdir_in(probe.path().data_dir().unwrap()).unwrap();
        let mut context = tauri::test::mock_context(tauri::test::noop_assets());
        context.config_mut().identifier = root.path().file_name().unwrap().to_str().unwrap().into();
        let app = tauri::test::mock_builder()
            .manage(HistoryMaintenance::default())
            .manage(HistoryState::new(Mutex::new(Vec::new())))
            .manage(AgentRunState::new(RunStore::new(root.path().join("runs"))))
            .manage(crate::workbench::WorkbenchOwnership::default())
            .manage(crate::Sidecar { child: Mutex::new(None), generation: RwLock::new(1), logs: Mutex::new(None), port, password: "fixture".into() })
            .build(context).unwrap();
        assert_eq!(app.path().app_data_dir().unwrap(), root.path());
        let directory = super::super::catalog_model::canonical_directory(&root.path().to_string_lossy()).unwrap();
        let store = commands::store(app.handle()).unwrap();
        store.register_directory(&directory).unwrap();
        let mut entry = super::super::catalog_import::new_entry(CatalogIdentity::Native {
            sidecar_id: SIDECAR_ID.into(), directory, session_id: "ses_empty".into(),
        }, "Empty".into(), ConversationSource::LightChat, 1, 2);
        entry.availability = Availability::Available;
        entry.runtime = RuntimeState::Idle;
        entry.has_records = Some(false);
        store.update_entry(&entry.key(), |row| { *row = Some(entry.clone()); Ok(()) }).unwrap();
        Self { app, store, entry, root }
    }

    fn session(&self) -> String {
        let CatalogIdentity::Native { directory, session_id, .. } = &self.entry.identity else { unreachable!() };
        json!({"id":session_id,"title":"Discovered","directory":directory,"time":{"created":1,"updated":2}}).to_string()
    }

    fn start(&self, session: &str) {
        let runs = self.app.state::<AgentRunState>();
        runs.begin("msg_active", self.root.path(), "Synthetic task").unwrap();
        runs.bind_session("msg_active", session).unwrap();
        runs.confirm_submission("msg_active").unwrap();
    }


}

fn reconcile(app: &tauri::AppHandle<MockRuntime>) -> Result<Vec<String>, String> {
    super::super::reconcile::reconcile(&commands::store(app)?, &commands::client(app)?,
        &app.state::<AgentRunState>(), &app.state::<crate::Sidecar>())
}

// A response gate makes lock contention deterministic; all socket waits are bounded.
fn serve_gated(listener: TcpListener, responses: Vec<(u16, String)>, gate: usize) -> (mpsc::Receiver<()>, mpsc::Sender<()>, std::thread::JoinHandle<Vec<String>>) {
    listener.set_nonblocking(true).unwrap();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let worker = std::thread::spawn(move || responses.into_iter().enumerate().map(|(index, (status, body))| {
        let deadline = Instant::now() + Duration::from_secs(8);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline => std::thread::sleep(Duration::from_millis(5)),
                Err(error) => panic!("expected HTTP request {index}: {error}"),
            }
        };
        // Windows accepted sockets can inherit the listener's nonblocking mode.
        stream.set_nonblocking(false).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        let mut bytes = Vec::new();
        while !bytes.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            stream.read_exact(&mut byte).unwrap(); bytes.push(byte[0]);
            assert!(bytes.len() < 16_384);
        }
        if index == gate { entered_tx.send(()).unwrap(); release_rx.recv_timeout(Duration::from_secs(4)).unwrap(); }
        write!(stream, "HTTP/1.1 {status} Result\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        String::from_utf8(bytes).unwrap()
    }).collect());
    (entered_rx, release_tx, worker)
}

fn empty_responses(row: &str) -> Vec<(u16, String)> {
    vec![(200,row.into()), (200,"[]".into()), (200,row.into()), (200,"[]".into()), (200,"{}".into()), (200,row.into()), (200,"true".into()), (404,"{}".into())]
}

#[test]
fn discovery_releases_control_during_http_and_rechecks_new_runs() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let f = Fixture::new(listener.local_addr().unwrap().port());
    let app = f.app.handle().clone();
    let (entered, release, server) = serve_gated(listener, vec![(200,format!("[{}]",f.session())),(200,"{}".into())], 0);
    std::thread::scope(|scope| {
        let refresh = scope.spawn(|| reconcile(&app));
        entered.recv_timeout(Duration::from_secs(3)).unwrap();
        let (tx, rx) = mpsc::channel();
        let control = app.clone();
        let workspace = f.root.path().to_owned();
        let start = scope.spawn(move || {
            let runs = control.state::<AgentRunState>();
            let _operation = runs.lock_operation().unwrap();
            runs.begin("msg_active", &workspace, "Synthetic task").unwrap();
            runs.bind_session("msg_active", "ses_empty").unwrap();
            runs.confirm_submission("msg_active").unwrap(); tx.send(()).unwrap();
        });
        let responsive = rx.recv_timeout(Duration::from_secs(1)).is_ok();
        release.send(()).unwrap();
        assert!(refresh.join().unwrap().unwrap().is_empty()); start.join().unwrap();
        assert!(responsive, "discovery HTTP blocked Agent control");
    });
    let row = f.store.get(&f.entry.key()).unwrap();
    assert_eq!(row.runtime, RuntimeState::Running);
    assert_eq!(row.ownership, Ownership::Agent);
    assert_eq!(server.join().unwrap().len(),2);
}

#[test]
fn discovery_from_a_retired_generation_cannot_publish() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let f = Fixture::new(listener.local_addr().unwrap().port());
    let app = f.app.handle().clone();
    let (entered, release, server) = serve_gated(listener, vec![(200,format!("[{}]",f.session())),(200,"{}".into())], 0);
    std::thread::scope(|scope| {
        let refresh = scope.spawn(|| reconcile(&app));
        entered.recv_timeout(Duration::from_secs(3)).unwrap();
        *app.state::<crate::Sidecar>().generation.write().unwrap() += 1;
        release.send(()).unwrap();
        assert_eq!(refresh.join().unwrap(),Err("history_native_unavailable".into()));
    });
    assert_eq!(f.store.get(&f.entry.key()).unwrap().title, "Empty");
    assert_eq!(server.join().unwrap().len(),2);
}

#[test]
fn tombstone_retry_does_not_hold_agent_control_during_delete() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let f = Fixture::new(listener.local_addr().unwrap().port());
    let app = f.app.handle().clone();
    f.start("ses_other");
    f.store.update_entry(&f.entry.key(), |row| {
        row.as_mut().unwrap().tombstone = Some(DeletionTombstone { requested_at: 3, remote_deleted:false }); Ok(())
    }).unwrap();
    let (entered, release, server) = serve_gated(listener, vec![(200,format!("[{}]",f.session())),(200,"{}".into()),(200,f.session()),(200,"true".into()),(404,"{}".into())], 3);
    std::thread::scope(|scope| {
        let refresh = scope.spawn(|| reconcile(&app));
        entered.recv_timeout(Duration::from_secs(3)).unwrap();
        let (tx, rx) = mpsc::channel();
        let control = app.clone();
        let cancel = scope.spawn(move || {
            let runs = control.state::<AgentRunState>();
            let _operation = runs.lock_operation().unwrap();
            runs.request_finish("msg_active", RunOutcome::Cancelled, None).unwrap(); tx.send(()).unwrap();
        });
        let responsive = rx.recv_timeout(Duration::from_secs(1)).is_ok();
        release.send(()).unwrap();
        assert!(refresh.join().unwrap().unwrap().is_empty()); cancel.join().unwrap();
        assert!(responsive, "tombstone retry blocked cancellation control");
    });
    assert!(f.store.get(&f.entry.key()).unwrap().tombstone.unwrap().remote_deleted);
    assert!(server.join().unwrap()[3].starts_with("DELETE "));
}

#[test]
fn empty_cleanup_keeps_selection_guard_but_releases_agent_control() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let f = Fixture::new(listener.local_addr().unwrap().port());
    let app = f.app.handle().clone();
    f.start("ses_other");
    let (entered, release, server) = serve_gated(listener, empty_responses(&f.session()), 3);
    std::thread::scope(|scope| {
        let cleanup = scope.spawn(|| prune_empty(&app));
        entered.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(matches!(app.state::<HistoryMaintenance>().used.try_lock(), Err(std::sync::TryLockError::WouldBlock)));
        let (tx, rx) = mpsc::channel();
        let control = app.clone();
        let cancel = scope.spawn(move || {
            let runs = control.state::<AgentRunState>();
            let _operation = runs.lock_operation().unwrap();
            runs.request_finish("msg_active", RunOutcome::Cancelled, None).unwrap(); tx.send(()).unwrap();
        });
        let responsive = rx.recv_timeout(Duration::from_secs(1)).is_ok();
        release.send(()).unwrap();
        assert!(cleanup.join().unwrap().unwrap().is_empty()); cancel.join().unwrap();
        assert!(responsive, "empty cleanup blocked cancellation control");
    });
    assert!(f.store.get(&f.entry.key()).unwrap().tombstone.unwrap().remote_deleted);
    assert_eq!(server.join().unwrap().len(),8);
}

#[test]
fn newly_arrived_messages_are_not_deleted() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let f = Fixture::new(listener.local_addr().unwrap().port());
    let app = f.app.handle().clone();
    let (entered, release, server) = serve_gated(listener, vec![(200,f.session()),(200,"[]".into()),(200,f.session()),(200,r#"[{"info":{"id":"msg_new"},"parts":[]}]"#.into())], 3);
    std::thread::scope(|scope| {
        let cleanup = scope.spawn(|| prune_empty(&app));
        entered.recv_timeout(Duration::from_secs(3)).unwrap(); release.send(()).unwrap();
        assert!(cleanup.join().unwrap().unwrap().is_empty());
    });
    assert!(f.store.get(&f.entry.key()).unwrap().tombstone.is_none());
    assert!(server.join().unwrap().iter().all(|request| !request.starts_with("DELETE ")));
}

#[test]
fn generation_change_during_empty_checks_prevents_deletion() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let f = Fixture::new(listener.local_addr().unwrap().port());
    let app = f.app.handle().clone();
    let (entered, release, server) = serve_gated(listener, empty_responses(&f.session()).into_iter().take(6).collect(), 5);
    std::thread::scope(|scope| {
        let cleanup = scope.spawn(|| prune_empty(&app));
        entered.recv_timeout(Duration::from_secs(3)).unwrap();
        *app.state::<crate::Sidecar>().generation.write().unwrap() += 1;
        release.send(()).unwrap();
        assert!(cleanup.join().unwrap().unwrap().is_empty());
    });
    assert!(f.store.get(&f.entry.key()).unwrap().tombstone.is_none());
    assert_eq!(server.join().unwrap().len(),6);
}

#[test]
fn selected_and_agent_sessions_are_never_cleanup_candidates() {
    // No server: even a single network request would fail this assertion.
    let f = Fixture::new(0);
    let app = f.app.handle().clone();
    app.state::<HistoryMaintenance>().used.lock().unwrap().insert("chat".into(), HashSet::from([f.entry.key()]));
    assert!(prune_empty(&app).unwrap().is_empty());
    app.state::<HistoryMaintenance>().used.lock().unwrap().clear();
    f.start("ses_empty");
    assert!(prune_empty(&app).unwrap().is_empty());
    assert!(f.store.get(&f.entry.key()).unwrap().tombstone.is_none());
}

#[test]
fn cleanup_waits_until_preparing_run_has_a_bound_session() {
    let f = Fixture::new(0);
    let app = f.app.handle().clone();
    app.state::<AgentRunState>().begin("msg_preparing", f.root.path(), "Synthetic task").unwrap();
    assert!(prune_empty(&app).unwrap().is_empty());
    assert!(f.store.get(&f.entry.key()).unwrap().tombstone.is_none());
}
