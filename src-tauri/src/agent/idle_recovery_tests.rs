use super::*;
use crate::agent::{AgentRunState, AgentPermissionState, RunStore, RunOutcome,
    collector::{collect_once_guarded_with, CollectorActions}, opencode::AgentEndpoint};
use std::{io::{BufRead, BufReader, Write}, net::TcpListener, sync::{Arc, atomic::{AtomicBool, Ordering}}, time::Duration};
use serde_json::json;

fn user() -> NativeMessage {
    NativeMessage { usage: None, system: None, id: "msg_run".into(), role: Some("user".into()),
        created: Some(1), parent_id: None, completed: false, finish: None, error: None, parts: vec![] }
}

struct Fixture { root: std::path::PathBuf, runs: AgentRunState, permissions: AgentPermissionState }
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("yume-idle-recovery-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let store = RunStore::new(root.join("runs"));
        let runs = AgentRunState::new(store.clone());
        runs.begin("msg_run", &root, "input").unwrap();
        runs.bind_session("msg_run", "ses_run").unwrap();
        runs.confirm_submission("msg_run").unwrap();
        let mut old = runs.read().unwrap().active.unwrap();
        old.created_at = "2026-01-01T00:00:00Z".into();
        store.write(&old).unwrap();
        let permissions = AgentPermissionState::default();
        permissions.register_run("msg_run", "ses_run", &root).unwrap();
        Self { root, runs: AgentRunState::load(store).unwrap(), permissions }
    }
    fn collect(&self, snapshot: SnapshotRead, generation_current: bool) {
        collect_once_guarded_with(&self.runs, &self.permissions, CollectorActions {
            snapshot: |_: &RunRecord| Ok(snapshot), pending: |_: &RunRecord| Ok(vec![]), archive: |_: &RunRecord, _: &[NativeMessage]| Ok(()), respond: |_: &RunRecord, _: &crate::tool_permissions::runtime::PermissionRequest, _: crate::tool_permissions::runtime::Reply| Ok(()),
        }, || Ok(generation_current.then_some(())), |_, _| Ok(())).unwrap();
    }
}
impl Drop for Fixture { fn drop(&mut self) { std::fs::remove_dir_all(&self.root).unwrap(); } }

// An HTTP fixture checks that recovery only reads native facts; no abort, prompt or tool mutation.
fn inspect_fixture(f: &Fixture, mode: &str) -> SnapshotRead {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let stop = Arc::new(AtomicBool::new(false));
    let done = stop.clone();
    let directory = crate::agent::opencode_wire_directory(&f.root);
    let server_directory = directory.clone();
    let mode = mode.to_owned();
    let server = std::thread::spawn(move || {
        let mut status_reads = 0;
        while !done.load(Ordering::SeqCst) {
            let (mut stream, _) = match listener.accept() {
                Ok(value) => value,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => { std::thread::sleep(Duration::from_millis(2)); continue; }
                Err(error) => panic!("{error}"),
            };
            stream.set_nonblocking(false).unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut first = String::new(); reader.read_line(&mut first).unwrap();
            assert!(first.starts_with("GET "), "recovery cannot mutate native execution: {first}");
            let url = url::Url::parse(&format!("http://localhost{}", first.split_whitespace().nth(1).unwrap())).unwrap();
            assert_eq!(url.query_pairs().find(|(k,_)| k == "directory").unwrap().1, server_directory);
            loop { let mut line = String::new(); assert!(reader.read_line(&mut line).unwrap() > 0); if line == "\r\n" { break; } }
            let mut code = 200;
            let body = match url.path() {
                "/session/ses_run" => json!({"id":"ses_run","title":"fixture","directory":if mode == "foreign" { "/different" } else { &server_directory },"time":{"created":1,"updated":1}}),
                "/session/status" => {
                    status_reads += 1;
                    if mode == "network" { code = 503; }
                    if mode == "malformed" { json!({"ses_run":{"type":"unknown"}}) }
                    else if mode == "busy" || (mode == "late-busy" && status_reads > 1) { json!({"ses_run":{"type":"busy"}}) }
                    else if mode == "retry" { json!({"ses_run":{"type":"retry"}}) }
                    else { json!({}) }
                }
                "/permission" | "/question" => {
                    if url.path() == format!("/{mode}") { json!([{"id":"req_pending","sessionID":"ses_run"}]) } else { json!([]) }
                }
                "/session/ses_run/message" => {
                    let mut messages = json!([{"info":{"id":"msg_run","sessionID":"ses_run","role":"user","time":{"created":1}},"parts":[]}]);
                    if mode == "completed" { messages.as_array_mut().unwrap().push(json!({"info":{"id":"msg_reply","sessionID":"ses_run","role":"assistant","parentID":"msg_run","time":{"created":2,"completed":3},"finish":"stop"},"parts":[]})); }
                    messages
                }
                other => panic!("unexpected request: {other}"),
            }.to_string();
            write!(stream,"HTTP/1.1 {code} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        }
    });
    let client = OpenCodeClient::new(AgentEndpoint { base_url:format!("http://127.0.0.1:{port}"), provider_id:String::new(), model_id:String::new(),workspace:f.root.clone(),auth_header:"Basic fixture".into() });
    let snapshot = inspect(&client, &f.runs.read().unwrap().active.unwrap(), vec![user()]);
    stop.store(true, Ordering::SeqCst); server.join().unwrap(); snapshot
}

#[test]
fn submitted_user_only_idle_run_is_interrupted_and_stays_terminal_after_restart() {
    let f = Fixture::new();
    let snapshot = inspect_fixture(&f, "idle");
    assert!(matches!(&snapshot, SnapshotRead::IdleInterrupted(_)));
    f.collect(snapshot, true);
    let reloaded = AgentRunState::load(RunStore::new(f.root.join("runs"))).unwrap().read().unwrap();
    assert!(reloaded.active.is_none());
    assert_eq!(reloaded.recent[0].outcome, Some(RunOutcome::Interrupted));
    assert_eq!(reloaded.recent[0].error_summary.as_deref(), Some("agent_execution_lost"));
    assert_eq!(reloaded.recent[0].session_id.as_deref(), Some("ses_run"));
}

#[test]
fn busy_retry_interactions_foreign_scope_and_unknown_network_remain_active() {
    for mode in ["busy","retry","permission","question","network","malformed","foreign","late-busy"] {
        let f = Fixture::new(); let snapshot = inspect_fixture(&f, mode);
        assert!(matches!(&snapshot, SnapshotRead::Messages(_)), "{mode}");
        f.collect(snapshot, true); assert!(f.runs.read().unwrap().active.is_some(), "{mode}");
    }
}

#[test]
fn late_native_completion_wins_over_cached_user_only_snapshot() {
    let f = Fixture::new(); let snapshot = inspect_fixture(&f, "completed");
    f.collect(snapshot, true);
    assert_eq!(f.runs.read().unwrap().recent[0].outcome, Some(RunOutcome::Completed));
}

#[test]
fn stale_engine_or_run_cannot_commit_idle_recovery() {
    let f = Fixture::new(); f.collect(inspect_fixture(&f, "idle"), false);
    assert!(f.runs.read().unwrap().active.is_some());
    collect_once_guarded_with(&f.runs, &f.permissions, CollectorActions {
        snapshot: |_: &RunRecord| { f.runs.request_finish("msg_run", RunOutcome::Cancelled, None)?; Ok(SnapshotRead::IdleInterrupted(vec![user()])) },
        pending: |_: &RunRecord| Ok(vec![]), archive: |_: &RunRecord, _: &[NativeMessage]| Ok(()), respond: |_: &RunRecord, _: &crate::tool_permissions::runtime::PermissionRequest, _: crate::tool_permissions::runtime::Reply| Ok(()),
    }, || Ok(Some(())), |_,_| Ok(())).unwrap();
    assert_eq!(f.runs.read().unwrap().active.unwrap().pending_outcome, Some(RunOutcome::Cancelled));
}

#[test]
fn recent_unconfirmed_missing_and_in_flight_snapshots_are_not_idle_candidates() {
    let f = Fixture::new(); let mut record = f.runs.read().unwrap().active.unwrap();
    let now = chrono::Utc::now().timestamp_millis() as u64;
    assert!(!unfinished(&record, &[], now));
    record.initial_input = Some("preparing".into()); assert!(!unfinished(&record, &[user()], now));
    record.initial_input = None; record.created_at = chrono::Utc::now().to_rfc3339();
    assert!(!unfinished(&record, &[user()], now));
    record.created_at = "2026-01-01T00:00:00Z".into();
    let mut reply = user(); reply.id = "msg_reply".into(); reply.role = Some("assistant".into()); reply.parent_id = Some(record.run_id.clone());
    reply.parts.push(crate::agent::record_store::NativePart {
        file_name: None, synthetic: false, id: "prt_tool".into(), kind: Some("tool".into()), text: None,
        call_id: Some("call_tool".into()), tool: Some("bash".into()),
        state: Some(crate::agent::record_store::NativeToolState { status: "running".into(), input:json!({}), output:json!(null), metadata:json!({}) }),
    });
    assert!(!unfinished(&record, &[user(), reply], now));
}

#[test]
fn host_task_keys_collapse_windows_aliases_but_keep_full_identity() {
    let f = Fixture::new(); let mut run = f.runs.read().unwrap().active.unwrap();
    for (a,b) in [(r"\\?\C:\Work\project\", "c:/work/project"), (r"\\?\UNC\Host\Share\a\..\b", "//host/share/b")] {
        run.workspace_path = a.into(); let expected = run.catalog_key();
        run.workspace_path = b.into(); assert_eq!(run.catalog_key(), expected);
        run.workspace_path = "C:/different".into(); assert_ne!(run.catalog_key(), expected);
    }
    run.workspace_path = "/Project".into(); let expected = run.catalog_key();
    run.workspace_path = "/project".into(); assert_ne!(run.catalog_key(), expected);
    run.workspace_path = "/Project".into(); run.session_id = Some("ses_other".into()); assert_ne!(run.catalog_key(), expected);
}
