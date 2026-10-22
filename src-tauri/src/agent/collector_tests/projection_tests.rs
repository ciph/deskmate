use super::{active, fixture, terminal};
use crate::{agent::{collector::{collect_once_prepared_with, collect_once_with, CollectorActions, SnapshotRead},
    record_store::{NativeMessage, RunOutcome, RunRecord}, test_support::TestResult},
    tool_permissions::runtime::{PermissionRequest, Reply}};
use std::{fs, sync::{mpsc, atomic::{AtomicBool, AtomicUsize, Ordering}}, thread, time::Duration};

#[test]
fn projection_barrier_releases_control_and_rejects_every_changed_revision() -> TestResult<()> {
    for change in ["cancel", "new_run", "generation", "workspace", "session", "created", "usage"] {
        let (root, workspace) = fixture(change)?;
        let (runs, permissions) = active(&root, &workspace)?;
        let generation_current = AtomicBool::new(true);
        let commits = AtomicUsize::new(0);
        let captures = AtomicUsize::new(0);
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        thread::scope(|scope| -> TestResult<()> {
            let (runs, permissions, generation_current, commits, captures, root, workspace) =
                (&runs, &permissions, &generation_current, &commits, &captures, &root, &workspace);
            let worker = scope.spawn(move || collect_once_prepared_with(runs, permissions, CollectorActions {
                snapshot: |_: &RunRecord| Ok(SnapshotRead::Messages(terminal("msg_run"))),
                pending: |_: &RunRecord| Ok(Vec::new()),
                archive: |_: &RunRecord, _: &[NativeMessage]| {
                    started_tx.send(()).unwrap();
                    release_rx.recv_timeout(Duration::from_secs(10)).unwrap();
                    Ok(())
                },
                respond: |_: &RunRecord, _: &PermissionRequest, _: Reply| panic!("stale permission response"),
            }, || Ok(generation_current.load(Ordering::SeqCst).then_some(())),
                |_, _| { captures.fetch_add(1, Ordering::SeqCst); Ok(()) },
                |_, ()| { commits.fetch_add(1, Ordering::SeqCst); Ok(()) }));
            started_rx.recv_timeout(Duration::from_secs(10)).map_err(|e| e.to_string())?;
            let (controlled_tx, controlled_rx) = mpsc::channel();
            let controller = scope.spawn(move || -> Result<(), String> {
                let _operation = runs.lock_operation()?;
                match change {
                    "cancel" => runs.request_finish("msg_run", RunOutcome::Cancelled, None)?,
                    "new_run" => {
                        runs.finish("msg_run", RunOutcome::Cancelled, None)?;
                        runs.begin("msg_new", &workspace, "new")?;
                        runs.bind_session("msg_new", "ses_new")?;
                        runs.confirm_submission("msg_new")?;
                    },
                    "generation" => { generation_current.store(false, Ordering::SeqCst); },
                    "usage" => runs.set_usage_pending(&runs.active_record("msg_run")?, vec!["missing".into()])?,
                    _ => {
                        let mut data = runs.data.lock().unwrap();
                        let record = data.active.as_mut().unwrap();
                        match change {
                            "workspace" => record.workspace_path = root.join("other-workspace"),
                            "session" => record.session_id = Some("ses_replacement".into()),
                            "created" => record.created_at = "replacement-revision".into(),
                            _ => unreachable!(),
                        }
                    }
                }
                controlled_tx.send(()).unwrap();
                Ok(())
            });
            // Release preparation even on failure, so a regression reports instead of hanging.
            let acquired_before_release = controlled_rx.recv_timeout(Duration::from_secs(5)).is_ok();
            release_tx.send(()).map_err(|e| e.to_string())?;
            controller.join().unwrap()?;
            worker.join().unwrap()?;
            assert!(acquired_before_release, "control waited for {change} projection release");
            Ok(())
        })?;
        assert_eq!(commits.load(Ordering::SeqCst), 0, "{change} projection committed");
        assert_eq!(captures.load(Ordering::SeqCst), 0, "{change} usage committed");
        if change == "cancel" {
            assert_eq!(runs.active_record("msg_run")?.pending_outcome, Some(RunOutcome::Cancelled));
            collect_once_with(&runs, &permissions, CollectorActions {
                snapshot: |_: &RunRecord| Ok(SnapshotRead::Messages(terminal("msg_run"))),
                pending: |_: &RunRecord| panic!("cancelled run requested permissions"),
                archive: |_: &RunRecord, _: &[NativeMessage]| Ok(()),
                respond: |_: &RunRecord, _: &PermissionRequest, _: Reply| Ok(()),
            })?;
            assert_eq!(runs.read()?.recent[0].outcome, Some(RunOutcome::Cancelled));
        }
        fs::remove_dir_all(root)?;
    }
    Ok(())
}

#[test]
fn overlapping_collector_skips_without_losing_pending_recovery() -> TestResult<()> {
    let (root, workspace) = fixture("projection-single-flight")?;
    let (runs, permissions) = active(&root, &workspace)?;
    let single_flight = runs.try_collection()?.unwrap();
    runs.request_finish("msg_run", RunOutcome::Cancelled, None)?;
    collect_once_with(&runs, &permissions, CollectorActions {
        snapshot: |_: &RunRecord| panic!("overlapping collector fetched old text"),
        pending: |_: &RunRecord| panic!("overlapping permissions"),
        archive: |_: &RunRecord, _: &[NativeMessage]| panic!("overlapping archive"),
        respond: |_: &RunRecord, _: &PermissionRequest, _: Reply| panic!("overlapping reply"),
    })?;
    let persisted = runs.record("msg_run")?.unwrap();
    assert_eq!(persisted.pending_outcome, Some(RunOutcome::Cancelled));
    assert!(persisted.outcome.is_none());
    drop(single_flight);
    let recovered = crate::agent::AgentRunState::load(crate::agent::RunStore::new(root.join("runs")))?;
    collect_once_with(&recovered, &permissions, CollectorActions {
        snapshot: |_: &RunRecord| Ok(SnapshotRead::Messages(terminal("msg_run"))),
        pending: |_: &RunRecord| panic!("pending recovery requested permissions"),
        archive: |_: &RunRecord, _: &[NativeMessage]| Ok(()),
        respond: |_: &RunRecord, _: &PermissionRequest, _: Reply| Ok(()),
    })?;
    assert_eq!(recovered.read()?.recent[0].outcome, Some(RunOutcome::Cancelled));
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn usage_commit_does_not_invalidate_its_own_prepared_projection() -> TestResult<()> {
    let (root, workspace) = fixture("projection-usage")?;
    let (runs, permissions) = active(&root, &workspace)?;
    let committed = AtomicBool::new(false);
    collect_once_prepared_with(&runs, &permissions, CollectorActions {
        snapshot: |_: &RunRecord| Ok(SnapshotRead::Messages(terminal("msg_run"))),
        pending: |_: &RunRecord| Ok(Vec::new()),
        archive: |_: &RunRecord, _: &[NativeMessage]| Ok(()),
        respond: |_: &RunRecord, _: &PermissionRequest, _: Reply| Ok(()),
    }, || Ok(Some(())), |record, _| runs.set_usage_pending(record, vec!["msg_reply".into()]),
        |_, ()| { committed.store(true, Ordering::SeqCst); Ok(()) })?;
    assert!(committed.load(Ordering::SeqCst));
    let final_record = runs.record("msg_run")?.unwrap();
    assert_eq!(final_record.outcome, Some(RunOutcome::Completed));
    assert_eq!(final_record.pending_usage_message_ids, ["msg_reply"]);
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn critical_run_write_failure_never_publishes_terminal_state() -> TestResult<()> {
    let (root, workspace) = fixture("projection-critical-write")?;
    let (runs, permissions) = active(&root, &workspace)?;
    let before = runs.active_record("msg_run")?;
    let result = collect_once_prepared_with(&runs, &permissions, CollectorActions {
        snapshot: |_: &RunRecord| Ok(SnapshotRead::Messages(terminal("msg_run"))),
        pending: |_: &RunRecord| Ok(Vec::new()),
        archive: |_: &RunRecord, _: &[NativeMessage]| { runs.store.inject_write_failure(true); Ok(()) },
        respond: |_: &RunRecord, _: &PermissionRequest, _: Reply| Ok(()),
    }, || Ok(Some(())), |_, _| Ok(()), |_, ()| Ok(()));
    assert_eq!(result, Err("agent_storage_unavailable".into()));
    assert_eq!(runs.active_record("msg_run")?, before);
    assert_eq!(runs.record("msg_run")?, Some(before));
    assert!(runs.read()?.recent.is_empty());
    runs.store.inject_write_failure(false);
    fs::remove_dir_all(root)?;
    Ok(())
}


#[test]
#[ignore = "isolated 30-sample lock experiment; run serially with --ignored --nocapture"]
fn projection_control_wait_measurement() -> TestResult<()> {
    use std::time::Instant;
    let mut rows = Vec::new();
    for mode in ["previous_lock_scope_reconstruction", "production_prepared_collector"] {
        for sample in 0..33 {
            let (root, workspace) = fixture("projection-lock-sample")?;
            let (runs, permissions) = active(&root, &workspace)?;
            let (started_tx, started_rx) = mpsc::channel();
            let timings = thread::scope(|scope| -> TestResult<(f64, f64)> {
                let (runs, permissions) = (&runs, &permissions);
                let worker = scope.spawn(move || -> Result<(), String> {
                    if mode == "previous_lock_scope_reconstruction" {
                        let _operation = runs.lock_operation()?;
                        started_tx.send(()).unwrap();
                        thread::sleep(Duration::from_millis(25));
                    } else {
                        collect_once_prepared_with(runs, permissions, CollectorActions {
                            snapshot: |_: &RunRecord| Ok(SnapshotRead::Messages(terminal("msg_run"))),
                            pending: |_: &RunRecord| Ok(Vec::new()),
                            archive: |_: &RunRecord, _: &[NativeMessage]| {
                                started_tx.send(()).unwrap();
                                thread::sleep(Duration::from_millis(25));
                                Ok(())
                            },
                            respond: |_: &RunRecord, _: &PermissionRequest, _: Reply| Ok(()),
                        }, || Ok(Some(())), |_, _| Ok(()), |_, ()| Ok(()))?;
                    }
                    Ok(())
                });
                started_rx.recv_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
                let requested = Instant::now();
                let operation = runs.lock_operation()?;
                let acquired = Instant::now();
                runs.request_finish("msg_run", RunOutcome::Cancelled, None)?;
                drop(operation);
                let released = Instant::now();
                worker.join().unwrap()?;
                Ok(((acquired-requested).as_secs_f64()*1000.0,
                    (released-acquired).as_secs_f64()*1000.0))
            })?;
            assert_eq!(runs.record("msg_run")?.unwrap().pending_outcome, Some(RunOutcome::Cancelled));
            if sample >= 3 {
                rows.push(serde_json::json!({"mode":mode,"sample":sample-3,
                    "controlAcquireMs":timings.0,"controlHeldMs":timings.1,
                    "projectionDelayMs":25,"projectionOperationsUnderControl":if mode == "production_prepared_collector" {0} else {1}}));
            }
            fs::remove_dir_all(root)?;
        }
    }
    println!("R06C_LOCK_SAMPLES {}", serde_json::to_string(&rows).map_err(|e| e.to_string())?);
    Ok(())
}
