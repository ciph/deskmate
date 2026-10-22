use super::*;
use crate::worklog::{contract::ReportKind, contract_runtime::SaveSchedule, storage::WorklogStore};
use std::sync::{atomic::AtomicUsize, mpsc, Barrier};

fn repository() -> Arc<Repository> {
    Arc::new(Repository::new(WorklogStore::memory().unwrap()))
}

// SQLite statement counts use a test-only trace callback, not estimated loop arithmetic.
#[derive(Default)]
struct SqlCounts {
    reads: std::sync::atomic::AtomicU64,
    mutations: std::sync::atomic::AtomicU64,
    transactions: std::sync::atomic::AtomicU64,
}
struct SqlTrace<'a> {
    repo: &'a Repository,
    counts: Box<SqlCounts>,
}
unsafe extern "C" fn count_statement(
    kind: std::ffi::c_uint,
    context: *mut std::ffi::c_void,
    statement: *mut std::ffi::c_void,
    _sql: *mut std::ffi::c_void,
) -> std::ffi::c_int {
    if kind == rusqlite::ffi::SQLITE_TRACE_STMT as std::ffi::c_uint {
        let counts = &*(context as *const SqlCounts);
        let sql = rusqlite::ffi::sqlite3_sql(statement as *mut rusqlite::ffi::sqlite3_stmt);
        if !sql.is_null() {
            let sql = std::ffi::CStr::from_ptr(sql).to_bytes();
            if sql.starts_with(b"SELECT") {
                counts.reads.fetch_add(1, Ordering::Relaxed);
            } else if sql.starts_with(b"INSERT")
                || sql.starts_with(b"UPDATE")
                || sql.starts_with(b"DELETE")
            {
                counts.mutations.fetch_add(1, Ordering::Relaxed);
            } else if sql.starts_with(b"BEGIN") {
                counts.transactions.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    0
}
impl<'a> SqlTrace<'a> {
    fn start(repo: &'a Repository) -> Self {
        let mut trace = Self {
            repo,
            counts: Box::default(),
        };
        // The boxed counters stay at a stable address until Drop unregisters the callback.
        repo.store
            .with_connection(|db| {
                unsafe {
                    assert_eq!(
                        rusqlite::ffi::sqlite3_trace_v2(
                            db.handle(),
                            rusqlite::ffi::SQLITE_TRACE_STMT as std::ffi::c_uint,
                            Some(count_statement),
                            (&mut *trace.counts as *mut SqlCounts).cast(),
                        ),
                        rusqlite::ffi::SQLITE_OK
                    );
                }
                Ok(())
            })
            .unwrap();
        trace
    }
}
impl Drop for SqlTrace<'_> {
    fn drop(&mut self) {
        self.repo
            .store
            .with_connection(|db| {
                unsafe {
                    rusqlite::ffi::sqlite3_trace_v2(db.handle(), 0, None, std::ptr::null_mut());
                }
                Ok(())
            })
            .unwrap();
    }
}
#[test]
fn ten_minute_virtual_idle_has_bounded_sql_work_and_no_row_updates() {
    for with_schedule in [false, true] {
        let repo = repository();
        // Saturday/Sunday in all timezones: the Friday occurrence is outside this 10m interval.
        let now = stamp("2026-09-12T12:00:00Z");
        if with_schedule {
            repo.store.with_connection(|db| {
                db.execute("INSERT INTO report_schedules(id,kind,weekday_set,local_time,enabled,created_at,updated_at) VALUES('idle','weekly','[5]','17:00',1,?1,?1)", [now.to_rfc3339()])?;
                Ok(())
            }).unwrap();
        }
        repo.enqueue_due(now, "fixture/test").unwrap(); // Warm the persisted next_due once.
        let before = repo
            .store
            .with_connection(|db| Ok(db.total_changes()))
            .unwrap();
        let trace = SqlTrace::start(&repo);
        let mut elapsed = Duration::ZERO;
        let mut cycles = 0;
        while elapsed < Duration::from_secs(600) {
            let current = now + ChronoDuration::from_std(elapsed).unwrap();
            assert_eq!(repo.enqueue_due(current, "fixture/test").unwrap(), 0);
            assert!(repo.claim_run(current).unwrap().is_none());
            let next = repo.next_runner_deadline(current).unwrap();
            elapsed += wait_until(next, current);
            cycles += 1;
        }
        let reads = trace.counts.reads.load(Ordering::Relaxed);
        let mutation_statements = trace.counts.mutations.load(Ordering::Relaxed);
        let transactions = trace.counts.transactions.load(Ordering::Relaxed);
        drop(trace);
        let rows = repo
            .store
            .with_connection(|db| Ok(db.total_changes() - before))
            .unwrap();
        assert_eq!(
            (cycles, reads, mutation_statements, transactions, rows),
            (10, 30, 10, 20, 0)
        );
        println!("I07B_VIRTUAL_600S schedule={with_schedule} cycles={cycles} sql_select={reads} mutation_statements={mutation_statements} begin_immediate={transactions} changed_rows={rows}");
    }
}

fn stamp(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .unwrap()
        .with_timezone(&Utc)
}
fn enqueue_empty(repo: &Repository, id: &str, now: DateTime<Utc>) {
    repo.store.with_connection(|db| {
        db.execute("INSERT INTO report_runs(id,kind,period_start,period_end,occurrence_key,state,model_id,created_at,updated_at) VALUES(?1,'daily','2026-09-07','2026-09-07',?1,'queued','fixture/test',?2,?2)", rusqlite::params![id, now.to_rfc3339()])?;
        Ok(())
    }).unwrap();
}
#[test]
fn deadlines_include_retries_and_exhausted_leases_and_exclude_disabled_or_terminal() {
    let repo = repository();
    let now = stamp("2026-09-08T10:00:00Z");
    assert_eq!(repo.next_runner_deadline(now).unwrap(), None);
    enqueue_empty(&repo, "retry", now);
    enqueue_empty(&repo, "exhausted", now);
    enqueue_empty(&repo, "done", now);
    repo.store.with_connection(|db| {
        db.execute("UPDATE report_runs SET state='retry_wait',next_retry_at=?1 WHERE id='retry'", [(now + ChronoDuration::seconds(20)).to_rfc3339()])?;
        db.execute("UPDATE report_runs SET state='running',attempt=max_attempt,lease_until=?1 WHERE id='exhausted'", [(now + ChronoDuration::seconds(10)).to_rfc3339()])?;
        db.execute("UPDATE report_runs SET state='failed',next_retry_at=?1,lease_until=?1 WHERE id='done'", [(now - ChronoDuration::seconds(10)).to_rfc3339()])?;
        db.execute("INSERT INTO report_schedules(id,kind,weekday_set,local_time,enabled,created_at,updated_at,next_due_at) VALUES('disabled','daily','[1]','17:00',0,?1,?1,?1)", [(now - ChronoDuration::seconds(20)).to_rfc3339()])?;
        Ok(())
    }).unwrap();
    assert_eq!(
        repo.next_runner_deadline(now).unwrap(),
        Some(now + ChronoDuration::seconds(10))
    );
    assert!(repo
        .claim_run(now + ChronoDuration::seconds(10))
        .unwrap()
        .is_none());
    assert_eq!(
        repo.next_runner_deadline(now).unwrap(),
        Some(now + ChronoDuration::seconds(20))
    );
    let retry = repo
        .claim_run(now + ChronoDuration::seconds(20))
        .unwrap()
        .unwrap();
    assert_eq!(retry.id, "retry");
    assert_eq!(
        repo.next_runner_deadline(now).unwrap(),
        Some(now + ChronoDuration::seconds(20) + ChronoDuration::minutes(6))
    );
}
#[test]
fn queued_retry_and_lease_recovery_keep_the_existing_attempt_fence() {
    let repo = super::tests::fixture();
    let now = super::tests::now();
    assert_eq!(repo.next_runner_deadline(now).unwrap(), Some(now));
    let first = repo.claim_run(now).unwrap().unwrap();
    assert_eq!(
        repo.next_runner_deadline(now).unwrap(),
        Some(now + ChronoDuration::minutes(6))
    );
    repo.fail_run(
        &first,
        (&WorklogError::new("MODEL_TIMEOUT", "fixture"), now),
    )
    .unwrap();
    assert_eq!(
        repo.next_runner_deadline(now).unwrap(),
        Some(now + ChronoDuration::minutes(1))
    );
    assert!(repo
        .claim_run(now + ChronoDuration::seconds(59))
        .unwrap()
        .is_none());
    let retry = repo
        .claim_run(now + ChronoDuration::minutes(1))
        .unwrap()
        .unwrap();
    let resumed = now + ChronoDuration::minutes(8);
    let recovered = repo.claim_run(resumed).unwrap().unwrap();
    assert_eq!(recovered.attempt, 3);
    assert!(repo.publish_run(&retry, ("late", resumed)).is_err());
    repo.publish_run(&recovered, ("recovered", resumed))
        .unwrap();
    assert!(repo.next_runner_deadline(resumed).unwrap().is_none());
}
#[test]
fn closest_schedule_updates_and_disabling_it_removes_the_deadline() {
    let repo = repository();
    let now = Utc::now();
    let request = SaveSchedule {
        request_id: uuid::Uuid::new_v4().to_string(),
        id: None,
        expected_revision: None,
        kind: ReportKind::Daily,
        weekday_set: vec![1, 2, 3, 4, 5],
        local_time: "17:00".into(),
        enabled: true,
    };
    let saved = repo.save_schedule(&request).unwrap();
    let first = repo.next_runner_deadline(now).unwrap().unwrap();
    let changed = SaveSchedule {
        request_id: uuid::Uuid::new_v4().to_string(),
        id: Some(saved.entity_id.clone()),
        expected_revision: Some(saved.revision),
        local_time: "18:00".into(),
        ..request
    };
    let saved = repo.save_schedule(&changed).unwrap();
    assert_ne!(repo.next_runner_deadline(now).unwrap(), Some(first));
    let future = now + ChronoDuration::days(8);
    assert_eq!(repo.enqueue_due(future, "fixture/test").unwrap(), 1);
    let running = repo.claim_run(future).unwrap().unwrap();
    let disabled = SaveSchedule {
        request_id: uuid::Uuid::new_v4().to_string(),
        expected_revision: Some(saved.revision),
        enabled: false,
        ..changed
    };
    repo.save_schedule(&disabled).unwrap();
    assert!(repo.publish_run(&running, ("late", future)).is_err());
    assert!(repo.claim_run(future).unwrap().is_none());
    assert_eq!(repo.next_runner_deadline(now).unwrap(), None);
    assert_eq!(
        repo.enqueue_due(now + ChronoDuration::days(20), "fixture/test")
            .unwrap(),
        0
    );
}
#[test]
fn wake_before_or_during_wait_is_retained_and_many_notifications_coalesce() {
    let wake = Arc::new(RunnerWake::default());
    let stop = AtomicBool::new(false);
    wake.begin_cycle();
    for _ in 0..100 {
        wake.notify();
    }
    let start = Instant::now();
    wake.wait(Duration::from_secs(2), &stop);
    assert!(start.elapsed() < Duration::from_secs(1));
    wake.begin_cycle();
    let sender = wake.clone();
    let thread = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(20));
        sender.notify();
    });
    let start = Instant::now();
    wake.wait(Duration::from_secs(2), &stop);
    assert!(start.elapsed() < Duration::from_secs(1));
    thread.join().unwrap();
    wake.begin_cycle();
    let start = Instant::now();
    wake.wait(Duration::from_millis(20), &stop);
    assert!(start.elapsed() >= Duration::from_millis(15));
}
#[test]
fn idle_stop_interrupts_a_long_wait() {
    let wake = Arc::new(RunnerWake::default());
    let stop = Arc::new(AtomicBool::new(false));
    let (done, receive) = mpsc::channel();
    let worker_wake = wake.clone();
    let worker_stop = stop.clone();
    let worker = std::thread::spawn(move || {
        worker_wake.wait(IDLE_RECHECK, &worker_stop);
        done.send(()).unwrap();
    });
    stop.store(true, Ordering::Relaxed);
    wake.notify();
    receive.recv_timeout(Duration::from_secs(1)).unwrap();
    worker.join().unwrap();
}
struct ObservedEnvironment {
    cycles: Arc<AtomicUsize>,
    changes: mpsc::Sender<()>,
    exited: mpsc::Sender<()>,
}
impl RunnerEnvironment for ObservedEnvironment {
    fn endpoint(&self) -> Option<ModelEndpoint> {
        panic!("empty work must not contact a provider");
    }
    fn model_identity(&self) -> Option<String> {
        self.cycles.fetch_add(1, Ordering::SeqCst);
        Some("fixture/test".into())
    }
    fn changed(&self) {
        let _ = self.changes.send(());
    }
}
impl Drop for ObservedEnvironment {
    fn drop(&mut self) {
        let _ = self.exited.send(());
    }
}
#[test]
fn actual_worker_wakes_for_committed_work_drains_queue_and_stops() {
    let repo = repository();
    let cycles = Arc::new(AtomicUsize::new(0));
    let (changes, receive) = mpsc::channel();
    let (exited, exit) = mpsc::channel();
    let handle = RunnerHandle::start(
        repo.clone(),
        Arc::new(ObservedEnvironment {
            cycles: cycles.clone(),
            changes,
            exited,
        }),
    );
    let started = Instant::now();
    while cycles.load(Ordering::SeqCst) == 0 {
        assert!(started.elapsed() < Duration::from_secs(2));
        std::thread::sleep(Duration::from_millis(1));
    }
    // Let the initial empty cycle enter its 60-second wait.
    std::thread::sleep(Duration::from_millis(30));
    enqueue_empty(&repo, "first", Utc::now());
    enqueue_empty(&repo, "second", Utc::now());
    for _ in 0..100 {
        handle.wake();
    }
    receive.recv_timeout(Duration::from_secs(2)).unwrap();
    receive.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(receive.recv_timeout(Duration::from_millis(50)).is_err());
    repo.store
        .with_connection(|db| {
            assert_eq!(
                db.query_row(
                    "SELECT count(*) FROM report_runs WHERE state='no_material' AND attempt=1",
                    [],
                    |row| row.get::<_, i64>(0)
                )?,
                2
            );
            Ok(())
        })
        .unwrap();
    handle.stop();
    exit.recv_timeout(Duration::from_secs(1)).unwrap();
}
#[test]
fn competing_wakes_enqueue_and_claim_a_due_cycle_only_once() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("scheduler.db");
    let repo = Arc::new(Repository::new(WorklogStore::open(&database).unwrap()));
    let other = Arc::new(Repository::new(WorklogStore::open(&database).unwrap()));
    let now = super::tests::now();
    repo.store.with_connection(|db| {
        db.execute("INSERT INTO report_schedules(id,kind,weekday_set,local_time,enabled,created_at,updated_at) VALUES('schedule','weekly','[5]','17:00',1,'2026-08-01T00:00:00+00:00','2026-08-01T00:00:00+00:00')", [])?;
        Ok(())
    }).unwrap();
    let start = Arc::new(Barrier::new(3));
    let workers: Vec<_> = [repo, other]
        .into_iter()
        .map(|repo| {
            let start = start.clone();
            std::thread::spawn(move || {
                start.wait();
                let enqueued = repo.enqueue_due(now, "fixture/test").unwrap();
                let claimed = repo.claim_run(now).unwrap().is_some();
                (enqueued, usize::from(claimed))
            })
        })
        .collect();
    start.wait();
    let totals = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .fold((0, 0), |sum, result| (sum.0 + result.0, sum.1 + result.1));
    assert_eq!(totals, (1, 1));
}
#[test]
fn wall_clock_rollback_forward_and_missed_wake_recompute_without_duplicates() {
    let repo = repository();
    let now = super::tests::now();
    repo.store.with_connection(|db| {
        db.execute("INSERT INTO report_schedules(id,kind,weekday_set,local_time,enabled,created_at,updated_at) VALUES('schedule','weekly','[5]','17:00',1,'2026-08-01T00:00:00+00:00','2026-08-01T00:00:00+00:00')", [])?;
        Ok(())
    }).unwrap();
    assert_eq!(repo.enqueue_due(now, "fixture/test").unwrap(), 1);
    let due = repo.next_runner_deadline(now).unwrap();
    assert_eq!(due, Some(now)); // Existing queued occurrence is immediately claimable.
    let first = repo.claim_run(now).unwrap().unwrap();
    repo.publish_run(&first, ("", now)).unwrap();
    let first_due = repo.next_runner_deadline(now).unwrap().unwrap();
    let backwards = now - ChronoDuration::hours(1);
    assert_eq!(repo.enqueue_due(backwards, "fixture/test").unwrap(), 0);
    assert_eq!(
        repo.next_runner_deadline(backwards).unwrap(),
        Some(first_due)
    );
    let resumed = now + ChronoDuration::weeks(3);
    assert_eq!(repo.enqueue_due(resumed, "fixture/test").unwrap(), 1);
    assert_eq!(repo.enqueue_due(resumed, "fixture/test").unwrap(), 0);
    let missed = repo.claim_run(resumed).unwrap().unwrap();
    repo.publish_run(&missed, ("", resumed)).unwrap();
    assert!(repo.next_runner_deadline(resumed).unwrap().unwrap() > resumed);
    assert_eq!(wait_until(None, resumed), IDLE_RECHECK);
    assert_eq!(
        wait_until(Some(resumed - ChronoDuration::hours(1)), resumed),
        BLOCKED_RECHECK
    );
    assert_eq!(
        wait_until(Some(resumed + ChronoDuration::milliseconds(50)), resumed),
        Duration::from_millis(50)
    );
}
