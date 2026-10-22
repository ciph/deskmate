use super::*;
use notify::event::{CreateKind, DataChange, Flag, ModifyKind, RemoveKind, RenameMode};
use std::{
    cell::Cell,
    sync::mpsc,
    time::{SystemTime, UNIX_EPOCH},
};

fn launch(directory: &Path) -> Launch {
    Launch {
        directory: directory.to_owned(),
        endpoint: "http://127.0.0.1:1".into(),
        auth_header: "Basic isolated-file-fixture".into(),
    }
}

fn request_bytes(id: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "version": 1, "requestId": id, "directory": "/workspace/isolated",
        "sessionId": "ses_fixture", "messageId": "msg_fixture", "callId": "call_fixture",
        "action": "query", "args": {}
    }))
    .unwrap()
}

fn publish(directory: &Path) -> PathBuf {
    let id = uuid::Uuid::new_v4().to_string();
    let temporary = directory.join(format!("{id}.tmp"));
    let request = directory.join(format!("{id}.request"));
    fs::write(&temporary, request_bytes(&id)).unwrap();
    fs::rename(temporary, &request).unwrap();
    request
}

#[test]
fn notification_filter_ignores_own_outputs_and_accepts_atomic_request_publish() {
    let path = PathBuf::from("launch/id.request");
    assert!(request_event(
        &Event::new(EventKind::Create(CreateKind::File)).add_path(path.clone())
    ));
    assert!(request_event(
        &Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Both)))
            .add_path(PathBuf::from("launch/id.tmp"))
            .add_path(path.clone())
    ));
    assert!(request_event(&Event::new(EventKind::Modify(
        ModifyKind::Name(RenameMode::To)
    ))));
    assert!(request_event(&Event::new(EventKind::Any)));
    assert!(request_event(&Event::new(EventKind::Other)));
    assert!(request_event(
        &Event::new(EventKind::Access(notify::event::AccessKind::Any)).set_flag(Flag::Rescan)
    ));
    for suffix in ["tmp", "response", "response-tmp"] {
        let path = PathBuf::from(format!("launch/id.{suffix}"));
        assert!(!request_event(
            &Event::new(EventKind::Create(CreateKind::File)).add_path(path.clone())
        ));
        assert!(!request_event(
            &Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::To))).add_path(path)
        ));
    }
    assert!(!request_event(
        &Event::new(EventKind::Remove(RemoveKind::File)).add_path(path.clone())
    ));
    assert!(!request_event(
        &Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Content))).add_path(path)
    ));
}

#[test]
fn duplicate_notifications_are_coalesced_and_do_not_hide_scan_time_notifications() {
    let wake = Wake::default();
    for _ in 0..1_000 {
        wake.notify();
    }
    assert!(wake.wait_until(Instant::now() + Duration::from_secs(10)));
    assert!(!wake.state.lock().unwrap().pending);
    // An event arriving while the consumer scans is retained for the next wait.
    wake.notify();
    assert!(wake.wait_until(Instant::now() + Duration::from_secs(10)));
    let start = Instant::now();
    assert!(wake.wait_until(start + Duration::from_millis(40)));
    assert!(start.elapsed() >= Duration::from_millis(30));
}

#[test]
fn shutdown_interrupts_an_idle_ten_second_wait() {
    let wake = Arc::new(Wake::default());
    let (ready, received) = mpsc::channel();
    let waiting = wake.clone();
    let worker = std::thread::spawn(move || {
        ready.send(()).unwrap();
        waiting.wait_until(Instant::now() + RECONCILE_INTERVAL)
    });
    received.recv_timeout(Duration::from_secs(1)).unwrap();
    let stopped = Instant::now();
    wake.stop();
    assert!(!worker.join().unwrap());
    assert!(stopped.elapsed() < Duration::from_secs(1));
    wake.notify();
    assert!(
        !wake.wait_until(Instant::now() + RECONCILE_INTERVAL),
        "stop wins over pending"
    );
}

#[test]
fn real_file_notification_wakes_atomic_rename_without_waiting_for_reconciliation() {
    let root = tempfile::tempdir().unwrap();
    let launch = launch(root.path());
    let wake = Arc::new(Wake::default());
    let mut watch = DirectoryWatch::default();
    watch.refresh(Some(root.path()), &wake, false);
    assert!(
        watch.watcher.is_some(),
        "native watcher must be available for this test"
    );
    scan(&launch, |_| panic!("empty initial scan")).unwrap();
    let file = publish(root.path());
    let published = Instant::now();
    assert!(wake.wait_until(published + Duration::from_secs(3)));
    assert!(
        published.elapsed() < Duration::from_secs(2),
        "notification, not the 10 s fallback"
    );
    let calls = Cell::new(0);
    scan(&launch, |_| {
        calls.set(calls.get() + 1);
        Ok(json!({"ok": true}))
    })
    .unwrap();
    for _ in 0..1_000 {
        wake.notify();
    }
    assert!(wake.wait_until(Instant::now() + RECONCILE_INTERVAL));
    scan(&launch, |_| {
        calls.set(calls.get() + 1);
        Ok(json!({}))
    })
    .unwrap();
    assert_eq!(calls.get(), 1);
    assert!(!file.exists());
    assert!(file.with_extension("response").exists());
}

#[test]
fn pre_watch_requests_and_missed_notifications_are_found_by_the_same_scan() {
    let root = tempfile::tempdir().unwrap();
    let launch = launch(root.path());
    let early = publish(root.path());
    let wake = Arc::new(Wake::default());
    let mut watch = DirectoryWatch::default();
    watch.refresh(Some(root.path()), &wake, false);
    let calls = Cell::new(0);
    scan(&launch, |_| {
        calls.set(calls.get() + 1);
        Ok(json!({}))
    })
    .unwrap();
    assert_eq!(calls.get(), 1);
    assert!(!early.exists());
    // Deliberately disconnect the watcher to model an unavailable/fully lost notification source.
    drop(watch);
    let wake = Wake::default();
    let late = publish(root.path());
    let started = Instant::now();
    let deadline = started + RECONCILE_INTERVAL;
    assert!(wake.wait_until(deadline));
    assert!(Instant::now() >= deadline);
    assert!(started.elapsed() < RECONCILE_INTERVAL + Duration::from_secs(2));
    scan(&launch, |_| {
        calls.set(calls.get() + 1);
        Ok(json!({}))
    })
    .unwrap();
    assert_eq!(calls.get(), 2);
    assert!(!late.exists());
}

#[test]
fn half_written_temporary_file_is_not_consumed_and_response_is_published_atomically() {
    let root = tempfile::tempdir().unwrap();
    let launch = launch(root.path());
    let id = uuid::Uuid::new_v4().to_string();
    let temporary = root.path().join(format!("{id}.tmp"));
    let request = temporary.with_extension("request");
    let response = temporary.with_extension("response");
    fs::write(&temporary, b"{\"version\":").unwrap();
    scan(&launch, |_| panic!("partial tmp must not execute")).unwrap();
    assert!(temporary.exists());
    fs::write(&temporary, request_bytes(&id)).unwrap();
    fs::rename(temporary, &request).unwrap();
    scan(&launch, |parsed| {
        assert_eq!(parsed.request_id, id);
        assert!(!response.exists(), "receipt follows the committed result");
        Ok(json!({"receipt": {"status": "saved"}}))
    })
    .unwrap();
    let result: Value = serde_json::from_slice(&fs::read(&response).unwrap()).unwrap();
    assert_eq!(result["status"], "completed");
    assert_eq!(result["result"]["receipt"]["status"], "saved");
    assert!(!request.exists());
    assert!(!response.with_extension("response-tmp").exists());
}

#[test]
fn expired_files_are_removed_without_execution_and_rejections_keep_host_error() {
    let root = tempfile::tempdir().unwrap();
    let launch = launch(root.path());
    let expired = publish(root.path());
    let file = fs::OpenOptions::new().write(true).open(&expired).unwrap();
    file.set_times(
        fs::FileTimes::new().set_modified(SystemTime::now() - Duration::from_secs(1801)),
    )
    .unwrap();
    drop(file);
    scan(&launch, |_| panic!("expired request must not execute")).unwrap();
    assert!(!expired.exists());
    let current = publish(root.path());
    scan(&launch, |_| {
        Err(WorklogError::new("STALE_CONTEXT", "Sidecar was restarted"))
    })
    .unwrap();
    let receipt: Value =
        serde_json::from_slice(&fs::read(current.with_extension("response")).unwrap()).unwrap();
    assert_eq!(receipt["status"], "rejected");
    assert_eq!(receipt["error"]["code"], "STALE_CONTEXT");
}

#[test]
fn malformed_and_mismatched_request_files_never_execute() {
    let root = tempfile::tempdir().unwrap();
    let launch = launch(root.path());
    let malformed = root
        .path()
        .join(format!("{}.request", uuid::Uuid::new_v4()));
    fs::write(&malformed, b"{\"version\":").unwrap();
    let mismatch = publish(root.path());
    fs::write(&mismatch, request_bytes(&uuid::Uuid::new_v4().to_string())).unwrap();
    let oversized = publish(root.path());
    fs::write(&oversized, vec![b' '; 256 * 1024 + 1]).unwrap();
    scan(&launch, |_| {
        panic!("untrusted shape cannot reach execution")
    })
    .unwrap();
    for path in [malformed, mismatch, oversized] {
        assert!(!path.exists());
        assert!(!path.with_extension("response").exists());
    }
}

#[test]
fn changing_launch_and_rearming_a_replaced_directory_keeps_notifications_live() {
    let root = tempfile::tempdir().unwrap();
    let old = root.path().join("old");
    let current = root.path().join("current");
    fs::create_dir(&old).unwrap();
    fs::create_dir(&current).unwrap();
    let wake = Arc::new(Wake::default());
    let mut watch = DirectoryWatch::default();
    watch.refresh(Some(&old), &wake, false);
    publish(&old);
    watch.refresh(Some(&current), &wake, false);
    let current_request = publish(&current);
    scan(&launch(&current), |_| Ok(json!({}))).unwrap();
    assert!(!current_request.exists());
    assert_eq!(
        fs::read_dir(&old).unwrap().count(),
        1,
        "late old launch notifications never scan the old directory"
    );

    fs::rename(&current, root.path().join("retired")).unwrap();
    fs::create_dir(&current).unwrap();
    watch.refresh(Some(&current), &wake, true);
    // Discard the old directory's deletion/rename notification before checking its replacement.
    wake.state.lock().unwrap().pending = false;
    let replacement_request = publish(&current);
    let start = Instant::now();
    assert!(wake.wait_until(start + Duration::from_secs(3)));
    assert!(start.elapsed() < Duration::from_secs(2));
    scan(&launch(&current), |_| Ok(json!({}))).unwrap();
    assert!(!replacement_request.exists());
    watch.refresh(None, &wake, false);
    assert!(watch.watcher.is_none());
    assert!(watch.directory.is_none());
}

#[test]
fn watcher_failure_recovers_when_directory_returns() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("missing");
    let wake = Arc::new(Wake::default());
    let mut watch = DirectoryWatch::default();
    watch.refresh(Some(&directory), &wake, false);
    assert!(watch.watcher.is_none());
    fs::create_dir(&directory).unwrap();
    watch.refresh(Some(&directory), &wake, true);
    assert!(watch.watcher.is_some());
    wake.invalidate_watch();
    assert!(wake.wait_until(Instant::now() + RECONCILE_INTERVAL));
    watch.refresh(Some(&directory), &wake, false);
    assert!(watch.watcher.is_some());
    assert!(!wake.take_watch_failure());
}

#[cfg(any(unix, windows))]
#[test]
fn scan_rejects_a_linked_launch_directory_before_expired_cleanup() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("target");
    let linked = root.path().join("linked");
    fs::create_dir(&target).unwrap();
    let sentinel = target.join("sentinel.response");
    fs::write(&sentinel, b"must survive").unwrap();
    fs::OpenOptions::new()
        .write(true)
        .open(&sentinel)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_secs(1)))
        .unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&target, &linked).unwrap();
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let result = std::process::Command::new("cmd.exe")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(&linked)
            .arg(&target)
            .creation_flags(0x08000000)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "isolated junction fixture: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    assert!(scan(&launch(&linked), |_| panic!(
        "linked launch must not dispatch"
    ))
    .is_err());
    assert!(sentinel.exists());
    fs::remove_dir(&linked).unwrap();
}

#[test]
#[ignore = "10 minute real wall-clock idle check; run separately from builds/benchmarks"]
fn idle_notifications_ten_minutes_real_wall_clock() {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    let legacy_root = tempfile::tempdir().unwrap();
    let legacy_done = Arc::new(AtomicBool::new(false));
    let legacy_scans = Arc::new(AtomicUsize::new(0));
    let done = legacy_done.clone();
    let count = legacy_scans.clone();
    // Empty-directory branch of 19630b6's loop; compare operation counts, not CPU or power.
    let legacy = std::thread::spawn(move || {
        while !done.load(Ordering::Relaxed) {
            for entry in fs::read_dir(legacy_root.path()).unwrap() {
                let _ = entry.unwrap().metadata();
            }
            count.fetch_add(1, Ordering::Relaxed);
            std::thread::sleep(Duration::from_millis(100));
        }
    });
    let root = tempfile::tempdir().unwrap();
    let launch = launch(root.path());
    let wake = Arc::new(Wake::default());
    let mut watch = DirectoryWatch::default();
    let started = Instant::now();
    let end = started + Duration::from_secs(600);
    let mut next = started;
    let mut scans = 0;
    let mut wakeups = 0;
    while Instant::now() < end {
        assert!(wake.wait_until(next.min(end)));
        if Instant::now() >= end {
            break;
        }
        wakeups += 1;
        let reconcile = Instant::now() >= next;
        if reconcile {
            next = Instant::now() + RECONCILE_INTERVAL;
        }
        watch.refresh(Some(root.path()), &wake, reconcile);
        scan(&launch, |_| panic!("idle")).unwrap();
        scans += 1;
    }
    legacy_done.store(true, Ordering::Relaxed);
    legacy.join().unwrap();
    println!(
        "I07a legacy empty-loop scans={}",
        legacy_scans.load(Ordering::Relaxed)
    );
    println!(
        "I07a real idle: wall_ms={}, scans={scans}, wakeups={wakeups}, interval_ms={}",
        started.elapsed().as_millis(),
        RECONCILE_INTERVAL.as_millis()
    );
    assert!(scans <= 61);
    assert!(wakeups <= 61);
}
