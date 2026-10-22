use super::{active, fixture, permission_snapshot, terminal};
use crate::{agent::{collector::{collect_once_with, retry_usage_with, CollectorActions, SnapshotRead},
    opencode::{AgentEndpoint, OpenCodeClient}, record_store::{NativeMessage, RunOutcome, RunRecord}, test_support::TestResult},
    tool_permissions::runtime::{PermissionRequest, Reply}};
use std::{fs, io::{Read, Write}, net::TcpListener, sync::{mpsc, Arc}, thread, time::Duration};

#[test]
fn slow_usage_http_does_not_hold_control_or_active_collection() -> TestResult<()> {
    let (root, workspace) = fixture("slow-usage")?;
    let (runs, permissions) = active(&root, &workspace)?;
    runs.set_usage_pending(&runs.active_record("msg_run")?, vec!["msg_reply".into()])?;
    let candidate = runs.active_record("msg_run")?;
    let runs = Arc::new(runs);
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let endpoint = format!("http://{}", listener.local_addr()?);
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let server = thread::spawn(move || -> Result<(), String> {
        let (mut socket, _) = listener.accept().map_err(|e| e.to_string())?;
        socket.set_read_timeout(Some(Duration::from_secs(5))).map_err(|e| e.to_string())?;
        let mut request = [0; 4096];
        socket.read(&mut request).map_err(|e| e.to_string())?;
        started_tx.send(()).map_err(|e| e.to_string())?;
        release_rx.recv_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
        let body = r#"[{"info":{"id":"msg_reply","sessionID":"ses_run","role":"assistant","parentID":"msg_run","time":{"created":2,"completed":3},"finish":"stop"},"parts":[]}]"#;
        write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).map_err(|e| e.to_string())?;
        Ok(())
    });
    let worker_runs = Arc::clone(&runs);
    let http_workspace = workspace.clone();
    let worker = thread::spawn(move || retry_usage_with(&worker_runs, &candidate,
        || OpenCodeClient::new(AgentEndpoint { base_url: endpoint, auth_header: "Basic fixture".into(),
            provider_id: "yume".into(), model_id: "fixture".into(), workspace: http_workspace }).snapshot("ses_run"),
        || Ok(Some(())), |_, messages| { assert_eq!(messages.len(), 1); Ok(Vec::new()) }));
    started_rx.recv_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    collect_once_with(&runs, &permissions, CollectorActions {
        snapshot: |_: &RunRecord| Ok(SnapshotRead::Messages(permission_snapshot())),
        pending: |_: &RunRecord| Ok(Vec::new()),
        archive: |_: &RunRecord, _: &[NativeMessage]| Ok(()),
        respond: |_: &RunRecord, _: &PermissionRequest, _: Reply| Ok(()),
    })?;
    { let _operation = runs.lock_operation()?; runs.request_finish("msg_run", RunOutcome::Cancelled, None)?; }
    release_tx.send(()).map_err(|e| e.to_string())?;
    worker.join().map_err(|_| "usage worker panicked".to_owned())??;
    server.join().map_err(|_| "usage server panicked".to_owned())??;
    let current = runs.active_record("msg_run")?;
    assert_eq!(current.pending_outcome, Some(RunOutcome::Cancelled));
    assert!(current.pending_usage_message_ids.is_empty());
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn usage_retry_rejects_changed_generation_and_full_run_identity() -> TestResult<()> {
    for change in ["generation", "workspace", "session", "created"] {
        let (root, workspace) = fixture(change)?;
        let (runs, _) = active(&root, &workspace)?;
        runs.set_usage_pending(&runs.active_record("msg_run")?, vec!["msg_reply".into()])?;
        let candidate = runs.active_record("msg_run")?;
        let mut changed = candidate.clone();
        match change {
            "workspace" => changed.workspace_path = root.join("other"),
            "session" => changed.session_id = Some("ses_other".into()),
            "created" => changed.created_at = "replacement".into(),
            _ => {}
        }
        runs.store.write(&changed)?;
        retry_usage_with(&runs, &candidate, || Ok(terminal("msg_run")),
            || Ok((change != "generation").then_some(())),
            |_, _| panic!("stale usage response committed"))?;
        assert_eq!(runs.record("msg_run")?.unwrap().pending_usage_message_ids, ["msg_reply"]);
        fs::remove_dir_all(root)?;
    }
    Ok(())
}

#[test]
fn failed_or_missing_usage_responses_preserve_pending_and_retry() -> TestResult<()> {
    let (root, workspace) = fixture("usage-failure")?;
    let (runs, _) = active(&root, &workspace)?;
    runs.set_usage_pending(&runs.active_record("msg_run")?, vec!["msg_reply".into()])?;
    runs.finish("msg_run", RunOutcome::Completed, None)?;
    let candidate = runs.record("msg_run")?.unwrap();
    assert_eq!(retry_usage_with(&runs, &candidate, || Err("timeout".into()),
        || Ok(Some(())), |_, _| panic!("failed HTTP committed")), Err("timeout".into()));
    retry_usage_with(&runs, &candidate, || Ok(Vec::new()), || Ok(Some(())), |_, _| Ok(Vec::new()))?;
    assert_eq!(runs.record("msg_run")?.unwrap().pending_usage_message_ids, ["msg_reply"]);
    runs.store.inject_write_failure(true);
    assert_eq!(retry_usage_with(&runs, &candidate, || Ok(terminal("msg_run")), || Ok(Some(())), |_, _| Ok(Vec::new())), Err("agent_storage_unavailable".into()));
    assert_eq!(runs.read()?.recent[0].pending_usage_message_ids, ["msg_reply"]);
    runs.store.inject_write_failure(false);
    retry_usage_with(&runs, &candidate, || Ok(terminal("msg_run")), || Ok(Some(())), |_, _| Ok(Vec::new()))?;
    let completed = runs.record("msg_run")?.unwrap();
    assert!(completed.pending_usage_message_ids.is_empty());
    assert_eq!(completed.outcome, Some(RunOutcome::Completed));
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn deleted_run_record_is_not_recreated_by_usage_retry() -> TestResult<()> {
    let (root, workspace) = fixture("deleted-usage")?;
    let (runs, _) = active(&root, &workspace)?;
    runs.set_usage_pending(&runs.active_record("msg_run")?, vec!["msg_reply".into()])?;
    let candidate = runs.active_record("msg_run")?;
    fs::remove_file(root.join("runs/msg_run.json"))?;
    retry_usage_with(&runs, &candidate, || Ok(terminal("msg_run")), || Ok(Some(())), |_, _| panic!("deleted run charged"))?;
    assert!(runs.record("msg_run")?.is_none());
    assert!(runs.active_record("msg_run")?.pending_usage_message_ids.is_empty());
    assert!(runs.usage_retry_candidate(std::time::Instant::now())?.is_none());
    fs::remove_dir_all(root)?;
    Ok(())
}
