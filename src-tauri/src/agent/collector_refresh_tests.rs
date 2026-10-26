use super::*;
use crate::agent::{record_store::{NativePart, NativeToolState, RunOutcome}, AgentRunState, RunStore};

fn record() -> RunRecord {
    RunRecord {
        run_id: "msg_run".into(), session_id: Some("ses_run".into()),
        workspace_path: std::path::PathBuf::from(r"E:\synthetic refresh"),
        created_at: "2026-10-22T00:00:00Z".into(), ended_at: None, outcome: None,
        error_summary: None, pending_outcome: None, pending_error_summary: None,
        message_ids: Vec::new(), pending_usage_message_ids: Vec::new(),
        part_ids: Vec::new(), call_ids: Vec::new(), initial_input: None,
    }
}

fn changes(ids: &[&str]) -> RefreshChanges {
    RefreshChanges { ticket: 1, full: false, connected: true,
        message_ids: ids.iter().map(|id| (*id).into()).collect() }
}

fn message(id: &str, created: u64) -> NativeMessage {
    NativeMessage {
        usage: None, system: None, id: id.into(), role: Some("assistant".into()),
        created: Some(created), parent_id: Some("msg_run".into()), completed: false,
        finish: None, error: None, parts: vec![NativePart {
            file_name: None,
            id: format!("prt_{id}"), synthetic: false, kind: Some("text".into()),
            text: Some("original".into()), call_id: None, tool: None, state: None,
        }],
    }
}

fn primed(record: &RunRecord, now: Instant, messages: Vec<NativeMessage>) -> LiveRefresh {
    let mut refresh = LiveRefresh::default();
    let scope = LiveRefresh::scope(record, 7).unwrap();
    assert_eq!(refresh.plan(record, &scope, &changes(&[]), now, false), Some(RefreshPlan::Full));
    let fetched = refresh.fetch_with(record, &RefreshPlan::Full, || Ok(messages.clone()), |_| panic!("initial fetch must be full")).unwrap();
    refresh.confirm(fetched, now);
    refresh
}

#[test]
fn initial_then_idle_then_thirty_second_reconciliation() {
    let record = record();
    let now = Instant::now();
    let scope = LiveRefresh::scope(&record, 7).unwrap();
    let mut refresh = primed(&record, now, vec![message("msg_a", 1)]);
    assert_eq!(refresh.plan(&record, &scope, &changes(&[]), now + Duration::from_millis(29_999), false), None);
    assert_eq!(refresh.plan(&record, &scope, &changes(&[]), now + Duration::from_secs(30), false), Some(RefreshPlan::Full));
    let fetched = refresh.fetch_with(&record, &RefreshPlan::Full, || Ok(vec![]), |_| panic!("full request")).unwrap();
    refresh.confirm(fetched, now + Duration::from_secs(30));
    assert_eq!(refresh.plan(&record, &scope, &changes(&[]), now + Duration::from_secs(59), false), None);
    assert_eq!(refresh.plan(&record, &scope, &changes(&[]), now + Duration::from_secs(60), false), Some(RefreshPlan::Full));
}

#[test]
fn dirty_message_budget_and_reconnect_force_authoritative_reads() {
    let record = record();
    let now = Instant::now();
    let scope = LiveRefresh::scope(&record, 7).unwrap();
    let mut refresh = primed(&record, now, vec![]);
    let dirty = changes(&["msg_b", "msg_a"]);
    assert_eq!(refresh.plan(&record, &scope, &dirty, now, false), Some(RefreshPlan::Messages(dirty.message_ids)));
    let mut dirty = changes(&[]);
    dirty.message_ids = (0..16).map(|i| format!("msg_{i}")).collect();
    assert!(matches!(refresh.plan(&record, &scope, &dirty, now, false), Some(RefreshPlan::Messages(ids)) if ids.len() == 16));
    dirty.message_ids.push("msg_overflow".into());
    assert_eq!(refresh.plan(&record, &scope, &dirty, now, false), Some(RefreshPlan::Full));
    let mut reconnect = changes(&[]);
    reconnect.full = true;
    assert_eq!(refresh.plan(&record, &scope, &reconnect, now, false), Some(RefreshPlan::Full));
    let mut disconnected = changes(&[]);
    disconnected.connected = false;
    assert_eq!(refresh.plan(&record, &scope, &disconnected, now, false), Some(RefreshPlan::Full));
}

#[test]
fn cache_resets_for_run_workspace_session_generation_and_creation_identity() {
    let original = record();
    let now = Instant::now();
    for case in 0..5 {
        let mut refresh = primed(&original, now, vec![message("msg_previous", 1)]);
        let mut next = original.clone();
        let mut generation = 7;
        match case {
            0 => next.run_id = "msg_next".into(),
            1 => next.workspace_path.push("another"),
            2 => next.session_id = Some("ses_next".into()),
            3 => generation += 1,
            _ => next.created_at = "2026-10-22T00:01:00Z".into(),
        }
        let scope = LiveRefresh::scope(&next, generation).unwrap();
        assert_eq!(refresh.plan(&next, &scope, &changes(&["msg_new"]), now, false), Some(RefreshPlan::Full), "identity case {case}");
        assert!(refresh.messages.is_empty(), "old identity cannot reach supervision");
        assert!(refresh.last_full.is_none());
    }
    let mut preparing = original;
    preparing.session_id = None;
    assert!(LiveRefresh::scope(&preparing, 7).is_none());
}

#[test]
fn cancellation_and_pending_failure_force_full_despite_quiet_stream() {
    let mut record = record();
    let now = Instant::now();
    let scope = LiveRefresh::scope(&record, 7).unwrap();
    let mut refresh = primed(&record, now, vec![]);
    for outcome in [RunOutcome::Cancelled, RunOutcome::Failed, RunOutcome::Interrupted] {
        record.pending_outcome = Some(outcome);
        assert_eq!(refresh.plan(&record, &scope, &changes(&[]), now, false), Some(RefreshPlan::Full));
    }
}

#[test]
fn partial_snapshot_replaces_appends_and_preserves_native_creation_order() {
    let record = record();
    let now = Instant::now();
    let mut refresh = primed(&record, now, vec![message("msg_z", 1), message("msg_b", 3)]);
    let fetched = refresh.fetch_with(&record, &RefreshPlan::Messages(vec!["msg_b".into(), "msg_a".into()]),
        || panic!("nonterminal partial fetch should not list session"), |id| {
            let mut next = message(id, if id == "msg_a" { 3 } else { 2 });
            next.parts[0].text = Some("updated".into());
            Ok(next)
        }).unwrap();
    assert_eq!(refresh.messages[1].parts[0].text.as_deref(), Some("original"), "fetch alone cannot commit");
    assert_eq!(fetched.messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["msg_z", "msg_b", "msg_a"]);
    refresh.confirm(fetched, now + Duration::from_secs(1));
    assert_eq!(refresh.messages[1].parts[0].text.as_deref(), Some("updated"));
    assert_eq!(refresh.last_full, Some(now), "a partial read must not postpone lost-event reconciliation");
}

#[test]
fn missing_or_incomplete_message_uses_full_snapshot_to_remove_stale_data() {
    let record = record();
    let now = Instant::now();
    for error in ["agent_http_404", "agent_wire_invalid", "agent_message_identity_mismatch", "agent_message_incomplete"] {
        let mut refresh = primed(&record, now, vec![message("msg_deleted", 1), message("msg_keep", 2)]);
        let fetched = refresh.fetch_with(&record, &RefreshPlan::Messages(vec!["msg_deleted".into()]),
            || Ok(vec![message("msg_keep", 2)]), |_| Err(error.into())).unwrap();
        assert_eq!(fetched.messages.len(), 1, "{error}");
        assert_eq!(fetched.messages[0].id, "msg_keep");
        refresh.confirm(fetched, now + Duration::from_secs(1));
        assert_eq!(refresh.last_full, Some(now + Duration::from_secs(1)));
    }
}

#[test]
fn failed_get_does_not_commit_a_partial_prefix_or_advance_full_deadline() {
    let record = record();
    let now = Instant::now();
    let refresh = primed(&record, now, vec![message("msg_a", 1)]);
    let before = format!("{:?}", refresh.messages);
    let fetched = refresh.fetch_with(&record, &RefreshPlan::Messages(vec!["msg_a".into(), "msg_b".into()]),
        || panic!("transport errors must not silently become successful full reads"), |id| {
            if id == "msg_b" { Err("agent_transport_failure".into()) }
            else { let mut next = message(id, 1); next.parts[0].text = Some("uncommitted".into()); Ok(next) }
        });
    assert_eq!(fetched.err().as_deref(), Some("agent_transport_failure"));
    assert_eq!(format!("{:?}", refresh.messages), before);
    assert_eq!(refresh.last_full, Some(now));
    assert_eq!(refresh.fetch_with(&record, &RefreshPlan::Full, || Err("agent_http_500".into()), |_| panic!("full")).err().as_deref(), Some("agent_http_500"));
    assert_eq!(refresh.last_full, Some(now));
}

#[test]
fn terminal_partial_read_requires_full_confirmation_and_cannot_finish_a_newer_turn() {
    let record = record();
    let now = Instant::now();
    let refresh = primed(&record, now, vec![message("msg_a", 1)]);
    let mut terminal = message("msg_a", 1);
    terminal.completed = true;
    terminal.finish = Some("stop".into());
    let authoritative = vec![terminal.clone(), message("msg_newer", 2)];
    let fetched = refresh.fetch_with(&record, &RefreshPlan::Messages(vec!["msg_a".into()]),
        || Ok(authoritative.clone()), |_| Ok(terminal.clone())).unwrap();
    assert!(fetched.full);
    assert_eq!(fetched.messages.len(), 2);
    let root = std::env::temp_dir().join(format!("yume-refresh-terminal-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let state = AgentRunState::new(RunStore::new(root.join("runs")));
    state.begin("msg_run", &root, "input").unwrap();
    state.bind_session("msg_run", "ses_run").unwrap();
    state.confirm_submission("msg_run").unwrap();
    state.reconcile("msg_run", &fetched.messages).unwrap();
    assert!(state.active_record("msg_run").is_ok(), "later unfinished message must prevent finish");
    assert_eq!(refresh.fetch_with(&record, &RefreshPlan::Messages(vec!["msg_a".into()]),
        || Err("agent_transport_failure".into()), |_| Ok(terminal.clone())).err().as_deref(), Some("agent_transport_failure"));
    assert!(!refresh.messages[0].completed, "failed terminal confirmation keeps prior cache");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn owned_error_is_terminal_but_foreign_terminal_does_not_force_full() {
    let record = record();
    let now = Instant::now();
    let refresh = primed(&record, now, vec![]);
    let mut foreign = message("msg_foreign", 1);
    foreign.parent_id = Some("msg_another_run".into());
    foreign.completed = true;
    foreign.finish = Some("stop".into());
    let fetched = refresh.fetch_with(&record, &RefreshPlan::Messages(vec![foreign.id.clone()]),
        || panic!("another run cannot trigger terminal confirmation"), |_| Ok(foreign.clone())).unwrap();
    assert!(!fetched.full);
    let mut failed = message("msg_failed", 2);
    failed.error = Some("provider_failed".into());
    let fetched = refresh.fetch_with(&record, &RefreshPlan::Messages(vec![failed.id.clone()]),
        || Ok(vec![failed.clone()]), |_| Ok(failed.clone())).unwrap();
    assert!(fetched.full);
}

#[test]
fn missed_terminal_event_recovers_at_thirty_seconds_without_renderer_activity() {
    let now = Instant::now();
    let root = std::env::temp_dir().join(format!("yume-refresh-missed-event-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let state = AgentRunState::new(RunStore::new(root.join("runs")));
    state.begin("msg_run", &root, "input").unwrap();
    state.bind_session("msg_run", "ses_run").unwrap();
    state.confirm_submission("msg_run").unwrap();
    let record = state.active_record("msg_run").unwrap();
    let scope = LiveRefresh::scope(&record, 7).unwrap();
    let mut refresh = primed(&record, now, vec![message("msg_a", 1)]);
    for second in 1..30 {
        assert_eq!(refresh.plan(&record, &scope, &changes(&[]), now + Duration::from_secs(second), false), None);
    }
    let plan = refresh.plan(&record, &scope, &changes(&[]), now + Duration::from_secs(30), false).unwrap();
    let mut terminal = message("msg_a", 1);
    terminal.completed = true;
    terminal.finish = Some("stop".into());
    let fetched = refresh.fetch_with(&record, &plan, || Ok(vec![terminal.clone()]), |_| panic!("no dirty ID")).unwrap();
    state.reconcile(&record.run_id, &fetched.messages).unwrap();
    assert!(state.read().unwrap().active.is_none());
    assert_eq!(state.read().unwrap().recent[0].outcome, Some(RunOutcome::Completed));
    refresh.confirm(fetched, now + Duration::from_secs(30));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn approval_and_owned_tool_timers_reuse_cache_without_message_http() {
    let record = record();
    let now = Instant::now();
    let scope = LiveRefresh::scope(&record, 7).unwrap();
    let mut refresh = primed(&record, now, vec![message("msg_a", 1)]);
    assert_eq!(refresh.plan(&record, &scope, &changes(&[]), now, true), Some(RefreshPlan::Cached));
    for status in ["pending", "running"] {
        let mut tool = message("msg_tool", 2);
        tool.parts[0].state = Some(NativeToolState { status: status.into(), input: serde_json::Value::Null,
            output: serde_json::Value::Null, metadata: serde_json::Value::Null });
        refresh = primed(&record, now, vec![tool]);
        assert_eq!(refresh.plan(&record, &scope, &changes(&[]), now, false), Some(RefreshPlan::Cached));
        let fetched = refresh.fetch_with(&record, &RefreshPlan::Cached,
            || panic!("timer must reuse cache"), |_| panic!("timer has no dirty message")).unwrap();
        assert!(!fetched.full);
        assert_eq!(fetched.messages[0].parts[0].state.as_ref().unwrap().status, status);
        refresh.messages[0].parent_id = Some("msg_foreign".into());
        assert_eq!(refresh.plan(&record, &scope, &changes(&[]), now, false), None);
    }
    refresh.clear();
    assert!(refresh.scope.is_none() && refresh.messages.is_empty() && refresh.last_full.is_none());
}

#[test]
#[ignore = "isolated HTTP work comparison with 10000 historical run files; run with --ignored --nocapture"]
fn sixty_refresh_http_work_matches_full_reference_without_old_record_reads() {
    use crate::agent::{collector::{collect_once_with, CollectorActions, SnapshotRead},
        opencode::{AgentEndpoint, OpenCodeClient}, AgentPermissionState};
    use crate::tool_permissions::runtime::{PermissionRequest, Reply};
    use std::{fs, io::{BufRead, BufReader, Write}, net::TcpListener,
        sync::{Arc, Mutex, atomic::{AtomicBool, AtomicUsize, Ordering}}, thread};

    let root = std::env::temp_dir().join(format!("yume-refresh-work-{}", uuid::Uuid::new_v4()));
    let runs_dir = root.join("runs");
    fs::create_dir_all(&runs_dir).unwrap();
    let mut active = record();
    active.workspace_path = root.canonicalize().unwrap();
    let mut old = active.clone();
    old.outcome = Some(RunOutcome::Completed);
    old.ended_at = Some("2026-10-21T00:00:00Z".into());
    for index in 0..10_000 {
        old.run_id = format!("msg_old_{index:05}");
        fs::write(runs_dir.join(format!("{}.json", old.run_id)), serde_json::to_vec(&old).unwrap()).unwrap();
    }
    let wire: Vec<_> = (0..1000).map(|index| serde_json::json!({
        "info":{"id":format!("msg_{index:04}"),"sessionID":"ses_run","parentID":"msg_run",
            "role":"assistant","time":{"created":index}},
        "parts":[{"id":format!("prt_{index:04}"),"sessionID":"ses_run","messageID":format!("msg_{index:04}"),"type":"text","text":"version-0"}]
    })).collect();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    listener.set_nonblocking(true).unwrap();
    let stopping = Arc::new(AtomicBool::new(false));
    let current_case = Arc::new(AtomicUsize::new(0));
    let current_version = Arc::new(AtomicUsize::new(0));
    // Per case: full requests, singular requests, JSON response body bytes, decoded messages.
    let requests = Arc::new(Mutex::new([[0usize; 4]; 3]));
    let (stop, case, version, counts) = (stopping.clone(), current_case.clone(), current_version.clone(), requests.clone());
    let expected_directory = crate::agent::opencode_wire_directory(&active.workspace_path);
    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(120);
        while !stop.load(Ordering::SeqCst) {
            assert!(Instant::now() < deadline, "HTTP fixture timed out");
            let (mut stream, _) = match listener.accept() {
                Ok(connection) => connection,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(1)); continue;
                }
                Err(error) => panic!("fixture accept: {error}"),
            };
            stream.set_nonblocking(false).unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut first = String::new();
            reader.read_line(&mut first).unwrap();
            assert!(first.starts_with("GET "));
            loop {
                let mut header = String::new();
                assert!(reader.read_line(&mut header).unwrap() > 0);
                if header == "\r\n" { break; }
            }
            let target = first.split_whitespace().nth(1).unwrap();
            let url = url::Url::parse(&format!("http://127.0.0.1{target}")).unwrap();
            assert_eq!(url.query_pairs().find(|(key, _)| key == "directory").unwrap().1, expected_directory);
            let full = match url.path() {
                "/session/ses_run/message" => true,
                "/session/ses_run/message/msg_0000" => false,
                path => panic!("unexpected fixture path {path}"),
            };
            let mut current = wire[0].clone();
            current["parts"][0]["text"] = serde_json::json!(format!("version-{}", version.load(Ordering::SeqCst)));
            let body = if full {
                let mut snapshot = wire.clone(); snapshot[0] = current; serde_json::to_vec(&snapshot).unwrap()
            } else { serde_json::to_vec(&current).unwrap() };
            {
                let mut counts = counts.lock().unwrap();
                let row = &mut counts[case.load(Ordering::SeqCst)];
                row[if full { 0 } else { 1 }] += 1;
                row[2] += body.len(); row[3] += if full { 1000 } else { 1 };
            }
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n", body.len()).unwrap();
            stream.write_all(&body).unwrap();
            stream.flush().unwrap();
        }
    });
    let client = OpenCodeClient::new(AgentEndpoint { base_url: format!("http://127.0.0.1:{port}"),
        provider_id: "fixture".into(), model_id: "fixture".into(),
        workspace: active.workspace_path.clone(), auth_header: String::new() });
    let mut reference_record = None;
    let mut reference_messages = None;
    let mut evidence = Vec::new();
    for (case, mode) in ["full-reference", "streaming", "idle"].iter().enumerate() {
        current_case.store(case, Ordering::SeqCst);
        fs::write(runs_dir.join("msg_run.json"), serde_json::to_vec(&active).unwrap()).unwrap();
        let store = RunStore::new(runs_dir.clone());
        let state = AgentRunState::load(store.clone()).unwrap();
        assert_eq!(store.io_counts()[0], 10_001, "startup scan is separately counted");
        let before = store.io_counts();
        let permissions = AgentPermissionState::default();
        permissions.register_run("msg_run", "ses_run", &active.workspace_path).unwrap();
        let mut refresh = LiveRefresh::default();
        let scope = LiveRefresh::scope(&active, 7).unwrap();
        let now = Instant::now();
        let mut latest = Vec::new();
        let mut collected = 0;
        for second in 0..60 {
            current_version.store(if case == 2 { 0 } else { second }, Ordering::SeqCst);
            let clock = now + Duration::from_secs(second as u64);
            let dirty = changes(if case == 1 { &["msg_0000"] } else { &[] });
            let plan = if case == 0 { Some(RefreshPlan::Full) }
                else { refresh.plan(&active, &scope, &dirty, clock, false) };
            let Some(plan) = plan else { continue; };
            let fetched = refresh.fetch_with(&active, &plan,
                || client.snapshot("ses_run"), |id| client.message("ses_run", id)).unwrap();
            latest = fetched.messages.clone();
            collect_once_with(&state, &permissions, CollectorActions {
                snapshot: |_: &RunRecord| Ok(SnapshotRead::Messages(fetched.messages.clone())),
                pending: |_: &RunRecord| Ok(Vec::<PermissionRequest>::new()),
                archive: |_: &RunRecord, _: &[NativeMessage]| Ok(()),
                respond: |_: &RunRecord, _: &PermissionRequest, _: Reply| Ok(()),
            }).unwrap();
            refresh.confirm(fetched, clock);
            collected += 1;
        }
        let final_record = state.active_record("msg_run").unwrap();
        if case == 0 {
            reference_record = Some(final_record.clone());
            reference_messages = Some(format!("{latest:?}"));
        } else {
            assert_eq!(Some(&final_record), reference_record.as_ref());
            if case == 1 { assert_eq!(Some(format!("{latest:?}")), reference_messages); }
            else { assert_eq!(latest[0].parts[0].text.as_deref(), Some("version-0")); }
        }
        let io: Vec<_> = store.io_counts().iter().zip(before).map(|(after, before)| after - before).collect();
        assert_eq!(io, [0, 1, 1, 1], "both B4 and B5 avoid old-record reads; only initial IDs persist");
        evidence.push(serde_json::json!({"mode":mode,"logicalTicks":60,"collectedSnapshots":collected,
            "startupRunReadsExcluded":10_001,"refreshRunIo":io,"sameFinalRunRecord":true}));
    }
    stopping.store(true, Ordering::SeqCst);
    server.join().unwrap();
    let counts = requests.lock().unwrap();
    assert_eq!((counts[0][0], counts[0][1], counts[0][3]), (60, 0, 60_000));
    assert_eq!((counts[1][0], counts[1][1], counts[1][3]), (2, 58, 2058));
    assert_eq!((counts[2][0], counts[2][1], counts[2][3]), (2, 0, 2000));
    for (row, counts) in evidence.iter_mut().zip(counts.iter()) {
        row["fullRequests"] = counts[0].into(); row["singularRequests"] = counts[1].into();
        row["jsonBodyBytes"] = counts[2].into(); row["decodedMessages"] = counts[3].into();
    }
    println!("refresh_http_work={}", serde_json::json!({"messages":1000,"historicalRunFiles":10_000,
        "logicalSeconds":60,"http":"actual loopback OpenCodeClient; JSON body bytes exclude headers/SSE",
        "scope":"plan/fetch/confirm plus collect_once_with; archive is no-op; not native app end-to-end or elapsed-time benchmark",
        "cases":evidence}));
    let resolved = root.canonicalize().unwrap();
    assert!(resolved.starts_with(std::env::temp_dir().canonicalize().unwrap()));
    assert!(resolved.file_name().unwrap().to_string_lossy().starts_with("yume-refresh-work-"));
    fs::remove_dir_all(resolved).unwrap();
}

#[test]
fn concurrent_usage_revision_skips_capture_then_retries_terminal_snapshot() {
    use crate::agent::{collector::{collect_once_prepared_with, CollectorActions, SnapshotRead}, AgentPermissionState};
    use crate::tool_permissions::runtime::{PermissionRequest, Reply};
    use std::cell::Cell;
    let root = std::env::temp_dir().join(format!("yume-refresh-skipped-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let state = AgentRunState::new(RunStore::new(root.join("runs")));
    let permissions = AgentPermissionState::default();
    state.begin("msg_run", &root, "input").unwrap();
    state.bind_session_with("msg_run", "ses_run", || permissions.register_run("msg_run", "ses_run", &root)).unwrap();
    state.confirm_submission("msg_run").unwrap();
    let now = Instant::now();
    let mut refresh = LiveRefresh::default();
    let mut terminal = message("msg_a", 1);
    terminal.completed = true;
    terminal.finish = Some("stop".into());
    for concurrent_change in [true, false] {
        let record = state.active_record("msg_run").unwrap();
        let scope = LiveRefresh::scope(&record, 7).unwrap();
        let plan = refresh.plan(&record, &scope, &changes(&[]), now, false).unwrap();
        assert_eq!(plan, RefreshPlan::Full, "an unapplied first read cannot initialize the cache");
        let fetched = refresh.fetch_with(&record, &plan, || Ok(vec![terminal.clone()]), |_| panic!("full")).unwrap();
        let applied = Cell::new(false);
        let committed = Cell::new(false);
        collect_once_prepared_with(&state, &permissions, CollectorActions {
            snapshot: |_: &RunRecord| Ok(SnapshotRead::Messages(fetched.messages.clone())),
            pending: |_: &RunRecord| Ok(Vec::new()),
            archive: |record: &RunRecord, _: &[NativeMessage]| {
                if concurrent_change { state.set_usage_pending(record, vec!["msg_usage".into()])?; }
                Ok(())
            },
            respond: |_: &RunRecord, _: &PermissionRequest, _: Reply| Ok(()),
        }, || Ok(Some(())), |_, _| { applied.set(true); Ok(()) },
            |_, ()| { committed.set(true); Ok(()) }).unwrap();
        assert_eq!(applied.get(), !concurrent_change);
        assert_eq!(committed.get(), !concurrent_change);
        if applied.get() { refresh.confirm(fetched, now); }
        if concurrent_change {
            assert!(refresh.last_full.is_none());
            assert!(refresh.messages.is_empty());
            assert!(state.active_record("msg_run").is_ok());
        }
    }
    assert_eq!(state.read().unwrap().recent[0].outcome, Some(RunOutcome::Completed));
    assert_eq!(refresh.last_full, Some(now));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn missed_tool_completion_rechecks_full_before_interrupt_or_timeout() {
    use crate::agent::opencode::{AgentEndpoint, OpenCodeClient};
    use std::{io::{BufRead, BufReader, Write}, net::TcpListener};
    for (busy, native_running) in [(false, false), (true, false), (true, true)] {
        let root = std::env::temp_dir().join(format!("yume-refresh-supervision-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let state = AgentRunState::new(RunStore::new(root.join("runs")));
        state.begin("msg_run", &root, "input").unwrap();
        state.bind_session("msg_run", "ses_run").unwrap();
        state.confirm_submission("msg_run").unwrap();
        let record = state.active_record("msg_run").unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let mut native = serde_json::json!([{"info":{"id":"msg_tool","sessionID":"ses_run","parentID":"msg_run","role":"assistant","finish":"stop","time":{"created":1,"completed":2}},"parts":[{"id":"prt_tool","type":"tool","tool":"bash","callID":"call_tool","state":{"status":"completed","input":{},"output":"done"}}]}]);
        if native_running {
            native[0]["info"]["time"] = serde_json::json!({"created":chrono::Utc::now().timestamp_millis()});
            native[0]["info"]["finish"] = serde_json::Value::Null;
            native[0]["parts"][0]["state"]["status"] = serde_json::json!("running");
        }
        let server = std::thread::spawn(move || {
            for (path, body) in [
                ("/session/status", if busy { r#"{"ses_run":{"type":"busy"}}"#.to_owned() } else { "{}".to_owned() }),
                ("/session/ses_run/message", native.to_string()),
            ] {
                let (mut stream, _) = listener.accept().unwrap();
                stream.set_nonblocking(false).unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut first = String::new();
                reader.read_line(&mut first).unwrap();
                assert!(first.starts_with(&format!("GET {path}?")), "no abort or tool mutation is allowed: {first}");
                loop {
                    let mut header = String::new();
                    assert!(reader.read_line(&mut header).unwrap() > 0);
                    if header == "\r\n" { break; }
                }
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
                stream.flush().unwrap();
            }
        });
        let client = OpenCodeClient::new(AgentEndpoint { base_url: format!("http://127.0.0.1:{port}"),
            provider_id: "fixture".into(), model_id: "fixture".into(), workspace: root.clone(), auth_header: String::new() });
        let now = chrono::Utc::now().timestamp_millis() as u64;
        let mut stale = message("msg_tool", now - if busy { 17 * 60 * 1000 } else { 11_000 });
        stale.parts[0].state = Some(NativeToolState { status: "running".into(), input: serde_json::Value::Null,
            output: serde_json::Value::Null, metadata: serde_json::Value::Null });
        let latest = crate::agent::supervision::inspect_messages(&client, &state, &record, vec![stale], false).unwrap();
        server.join().unwrap();
        assert_eq!(state.active_record("msg_run").unwrap().pending_outcome, None);
        assert_eq!(latest[0].parts[0].state.as_ref().unwrap().status, if native_running { "running" } else { "completed" });
        state.reconcile("msg_run", &latest).unwrap();
        if native_running { assert!(state.active_record("msg_run").is_ok()); }
        else { assert_eq!(state.read().unwrap().recent[0].outcome, Some(RunOutcome::Completed)); }
        std::fs::remove_dir_all(root).unwrap();
    }
}
