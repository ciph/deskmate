use super::*;
fn request() -> RecordEntry {
    RecordEntry {
        request_id: uuid::Uuid::new_v4().to_string(),
        business_date: "2026-09-07".into(),
        project: Some(" Project ".into()),
        original_text: "Finished release checks".into(),
        text: "Finished release checks".into(),
        status: EntryStatus::Done,
        source_session_id: Some("session-a".into()),
        source_message_id: Some("message-a".into()),
    }
}
#[test]
fn source_deletion_clears_transitive_snapshots_and_cancels_generation() {
    let repo = Repository::new(WorklogStore::memory().unwrap());
    let saved = repo.record_entry(&request()).unwrap();
    let daily = SaveReport {
        request_id: uuid::Uuid::new_v4().to_string(),
        kind: ReportKind::Daily,
        period_start: "2026-09-07".into(),
        period_end: "2026-09-07".into(),
        expected_revision: None,
        body_markdown: "Source may remain in report body".into(),
    };
    let report = repo.save_report(&daily).unwrap();
    let detail = repo.get_report(&report.entity_id).unwrap();
    let version = &detail.versions[0];
    let source = SourceSnapshot {
        source: SourceRef {
            kind: "entry".into(),
            id: saved.entity_id.clone(),
            revision: 1,
        },
        business_date: "2026-09-07".into(),
        project: None,
        text: "private synthetic source".into(),
        entry_status: Some(EntryStatus::Done),
    };
    repo.store.with_transaction(|db|{
        db.execute("INSERT INTO report_sources VALUES (?1,'entry',?2,1)",params![version.id,saved.entity_id])?;
        db.execute("UPDATE report_versions SET source_revision_manifest=?1,source_snapshot=?2 WHERE id=?3",params![serde_json::to_string(&vec![&source.source])?,serde_json::to_string(&vec![&source])?,version.id])?;
        db.execute("INSERT INTO report_runs(id,kind,period_start,period_end,occurrence_key,state,source_manifest,source_snapshot,model_id,created_at,updated_at) VALUES ('run','daily','2026-09-07','2026-09-07','test','running',?1,?2,'p/m','now','now')",params![serde_json::to_string(&vec![&source.source])?,serde_json::to_string(&vec![&source])?])?;
        Ok(())
    }).unwrap();
    repo.delete_entry(&DeleteRecord {
        request_id: uuid::Uuid::new_v4().to_string(),
        id: saved.entity_id,
        expected_revision: 1,
        delete_linked_reports: false,
    })
    .unwrap();
    let after = repo.get_report(&report.entity_id).unwrap();
    assert!(after.report.stale);
    assert!(after.report.source_deleted);
    assert!(after.versions[0].source_snapshot.is_empty());
    assert_eq!(after.versions[0].body_markdown, daily.body_markdown);
    let runs = repo.list_runs().unwrap();
    assert_eq!(
        runs[0].state,
        super::super::contract_runtime::RunState::Cancelled
    );
    assert!(runs[0].source_snapshot.is_empty());
}

#[test]
fn manual_save_preserves_freshness_and_date_move_marks_old_day() {
    let repo = Repository::new(WorklogStore::memory().unwrap());
    let entry = repo.record_entry(&request()).unwrap();
    let mut daily = SaveReport {
        request_id: uuid::Uuid::new_v4().to_string(),
        kind: ReportKind::Daily,
        period_start: "2026-09-07".into(),
        period_end: "2026-09-07".into(),
        expected_revision: None,
        body_markdown: "Original notes".into(),
    };
    let saved = repo.save_report(&daily).unwrap();
    assert!(!repo.get_report(&saved.entity_id).unwrap().report.stale);
    daily.request_id = uuid::Uuid::new_v4().to_string();
    daily.expected_revision = Some(1);
    repo.save_report(&daily).unwrap();
    assert!(!repo.get_report(&saved.entity_id).unwrap().report.stale);
    repo.update_entry(&UpdateEntry {
        request_id: uuid::Uuid::new_v4().to_string(),
        id: entry.entity_id,
        expected_revision: 1,
        business_date: "2026-09-08".into(),
        project: None,
        text: "Moved work item".into(),
        status: EntryStatus::Done,
    })
    .unwrap();
    assert!(repo.get_report(&saved.entity_id).unwrap().report.stale);
}

#[test]
fn explicit_retry_rebinds_model_and_retains_attempt_fence() {
    use super::super::contract_runtime::*;
    let repo = Repository::new(WorklogStore::memory().unwrap());
    let saved = repo
        .generate_report(&GenerateReport {
            request_id: uuid::Uuid::new_v4().to_string(),
            kind: ReportKind::Daily,
            period_start: "2026-09-07".into(),
            period_end: "2026-09-07".into(),
            model_id: "old/model".into(),
        })
        .unwrap();
    repo.store
        .with_transaction(|db| {
            db.execute(
                "UPDATE report_runs SET state='failed',attempt=3 WHERE id=?1",
                [&saved.entity_id],
            )?;
            Ok(())
        })
        .unwrap();
    let request = RetryRun {
        request_id: uuid::Uuid::new_v4().to_string(),
        id: saved.entity_id,
    };
    repo.retry_run(&request, "new/model").unwrap();
    let run = repo.list_runs().unwrap().remove(0);
    assert_eq!(run.attempt, 3);
    assert_eq!(run.model_id, "new/model");
    assert_eq!(run.state, RunState::Queued);
    assert_eq!(
        repo.retry_run(&request, "other/model").unwrap().entity_id,
        run.id
    );
}

#[test]
#[ignore = "Prints synthetic SQLite reopen evidence"]
fn manual_sqlite_reopen_evidence() {
    let dir = std::env::temp_dir().join(format!("worklog-manual-{}", uuid::Uuid::new_v4()));
    let path = dir.join("journal.db");
    println!("SURFACE: independent SQLite file in temporary directory; user data never accessed");
    {
        let repo = Repository::new(WorklogStore::open(&path).unwrap());
        let saved = repo.record_entry(&request()).unwrap();
        println!(
            "COMMITTED operation={} entry={} revision={}",
            saved.operation_id, saved.entity_id, saved.revision
        );
    }
    {
        let repo = Repository::new(WorklogStore::open(&path).unwrap());
        let entries = repo
            .query_entries(&DateQuery {
                start: "2026-09-07".into(),
                end: "2026-09-08".into(),
                project: None,
            })
            .unwrap();
        assert_eq!(entries.len(), 1);
        let item = &entries[0];
        assert_eq!(item.text, "Finished release checks");
        println!(
            "REOPEN date={} project={:?} sourceSession={:?} sourceMessage={:?} body={}",
            item.business_date,
            item.project,
            item.source_session_id,
            item.source_message_id,
            item.text
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
    println!("CLEANUP: temporary SQLite directory removed; 2 persistence assertions passed");
}

fn deletion_snapshot(kind: &str, id: &str) -> SourceSnapshot {
    SourceSnapshot {
        source: SourceRef {
            kind: kind.into(),
            id: id.into(),
            revision: 1,
        },
        business_date: "2026-09-07".into(),
        project: None,
        text: "synthetic private source".into(),
        entry_status: Some(EntryStatus::Done),
    }
}
fn deletion_report(db: &Connection, id: &str) -> WorklogResult<()> {
    db.execute("INSERT INTO reports(id,kind,period_start,period_end,updated_at) VALUES(?1,'daily',?1,?1,'now')", [id])?;
    Ok(())
}
fn deletion_version(
    db: &Connection,
    report: &str,
    id: &str,
    number: i64,
    manifest: &[SourceRef],
    snapshots: &[SourceSnapshot],
) -> WorklogResult<()> {
    db.execute("INSERT INTO report_versions(id,report_id,version,body_markdown,origin,source_revision_manifest,source_snapshot,coverage_dates,generated_at) VALUES(?1,?2,?3,'retained body','manual',?4,?5,'[\"2026-09-07\"]','now')", params![id,report,number,serde_json::to_string(manifest)?,serde_json::to_string(snapshots)?])?;
    Ok(())
}
fn deletion_request(id: &str, linked: bool) -> DeleteRecord {
    DeleteRecord {
        request_id: uuid::Uuid::new_v4().to_string(),
        id: id.into(),
        expected_revision: 1,
        delete_linked_reports: linked,
    }
}
#[test]
fn d20a_unchanged_versions_receive_no_update() {
    let repo = Repository::new(WorklogStore::memory().unwrap());
    let entry = repo.record_entry(&request()).unwrap();
    repo.store.with_transaction(|db| {
        deletion_report(db, "related")?;
        deletion_report(db, "unrelated")?;
        let source = deletion_snapshot("entry", &entry.entity_id);
        deletion_version(db, "related", "related-v", 1, &[source.source.clone()], &[source])?;
        deletion_version(db, "unrelated", "unrelated-v", 1, &[], &[])?;
        db.execute("INSERT INTO report_sources VALUES('related-v','entry',?1,1)", [&entry.entity_id])?;
        db.execute_batch("CREATE TEMP TABLE deletion_writes(id TEXT); CREATE TEMP TRIGGER deletion_version_write AFTER UPDATE ON report_versions BEGIN INSERT INTO deletion_writes VALUES(old.id); END;")?;
        Ok(())
    }).unwrap();
    repo.delete_entry(&deletion_request(&entry.entity_id, false))
        .unwrap();
    let writes = repo
        .store
        .with_connection(|db| {
            Ok(db.query_row(
                "SELECT count(*) FROM deletion_writes WHERE id='unrelated-v'",
                [],
                |r| r.get::<_, i64>(0),
            )?)
        })
        .unwrap();
    println!("D20A unrelated_version_updates={writes}");
    assert_eq!(writes, 0, "unchanged version must not be written");
}

use std::cell::Cell;

#[derive(Default)]
struct DeletionSqlCounts {
    version_scans: Cell<usize>,
    run_scans: Cell<usize>,
    version_rows: Cell<usize>,
    run_rows: Cell<usize>,
    version_updates: Cell<usize>,
    unrelated_updates: Cell<usize>,
    run_updates: Cell<usize>,
    transaction_start: Cell<Option<std::time::Instant>>,
    transaction_ns: Cell<u64>,
}
unsafe extern "C" fn deletion_trace(
    kind: std::ffi::c_uint,
    context: *mut std::ffi::c_void,
    statement: *mut std::ffi::c_void,
    detail: *mut std::ffi::c_void,
) -> std::ffi::c_int {
    let counts = &*(context as *const DeletionSqlCounts);
    let statement = statement as *mut rusqlite::ffi::sqlite3_stmt;
    let sql = rusqlite::ffi::sqlite3_sql(statement);
    if sql.is_null() {
        return 0;
    }
    let sql = std::ffi::CStr::from_ptr(sql).to_string_lossy();
    if kind == rusqlite::ffi::SQLITE_TRACE_STMT as u32 && sql.starts_with("BEGIN IMMEDIATE") {
        counts
            .transaction_start
            .set(Some(std::time::Instant::now()));
    }
    if kind == rusqlite::ffi::SQLITE_TRACE_PROFILE as u32 && sql == "COMMIT" {
        if let Some(started) = counts.transaction_start.take() {
            counts
                .transaction_ns
                .set(started.elapsed().as_nanos() as u64);
        }
    }
    let versions = sql.starts_with("SELECT")
        && sql.contains("source_revision_manifest")
        && sql.contains("FROM report_versions")
        && !sql.contains("WHERE");
    let runs = sql.starts_with("SELECT")
        && sql.contains("source_manifest")
        && sql.contains("FROM report_runs")
        && !sql.contains("WHERE");
    if kind == rusqlite::ffi::SQLITE_TRACE_ROW as u32 {
        if versions {
            counts.version_rows.set(counts.version_rows.get() + 1);
        }
        if runs {
            counts.run_rows.set(counts.run_rows.get() + 1);
        }
    } else if kind == rusqlite::ffi::SQLITE_TRACE_STMT as u32 {
        if !detail.is_null()
            && std::ffi::CStr::from_ptr(detail.cast())
                .to_bytes()
                .starts_with(b"--")
        {
            return 0;
        }
        if versions {
            counts.version_scans.set(counts.version_scans.get() + 1);
        }
        if runs {
            counts.run_scans.set(counts.run_scans.get() + 1);
        }
        if sql.starts_with("UPDATE report_versions") {
            counts.version_updates.set(counts.version_updates.get() + 1);
            let expanded = rusqlite::ffi::sqlite3_expanded_sql(statement);
            if !expanded.is_null() {
                if std::ffi::CStr::from_ptr(expanded)
                    .to_string_lossy()
                    .contains("'u-")
                {
                    counts
                        .unrelated_updates
                        .set(counts.unrelated_updates.get() + 1);
                }
                rusqlite::ffi::sqlite3_free(expanded.cast());
            }
        }
        if sql.starts_with("UPDATE report_runs") {
            counts.run_updates.set(counts.run_updates.get() + 1);
        }
    }
    0
}
struct DeletionTrace<'a> {
    repo: &'a Repository,
    counts: Box<DeletionSqlCounts>,
}
impl<'a> DeletionTrace<'a> {
    fn new(repo: &'a Repository) -> Self {
        let mut trace = Self {
            repo,
            counts: Box::default(),
        };
        repo.store
            .with_connection(|db| {
                // The box stays at one address until Drop unregisters the connection callback.
                assert_eq!(
                    unsafe {
                        rusqlite::ffi::sqlite3_trace_v2(
                            db.handle(),
                            (rusqlite::ffi::SQLITE_TRACE_STMT
                                | rusqlite::ffi::SQLITE_TRACE_ROW
                                | rusqlite::ffi::SQLITE_TRACE_PROFILE)
                                as u32,
                            Some(deletion_trace),
                            (&mut *trace.counts as *mut DeletionSqlCounts).cast(),
                        )
                    },
                    rusqlite::ffi::SQLITE_OK
                );
                Ok(())
            })
            .unwrap();
        trace
    }
}
impl Drop for DeletionTrace<'_> {
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
fn deletion_run(
    db: &Connection,
    id: &str,
    state: &str,
    manifest: &[SourceRef],
    snapshots: &[SourceSnapshot],
) -> WorklogResult<()> {
    db.execute("INSERT INTO report_runs(id,kind,period_start,period_end,occurrence_key,state,source_manifest,source_snapshot,model_id,created_at,updated_at,attempt,lease_until) VALUES(?1,'daily','2026-09-07','2026-09-07',?1,?2,?3,?4,'p/m','now','now',1,'2099-01-01T00:00:00Z')",params![id,state,serde_json::to_string(manifest)?,serde_json::to_string(snapshots)?])?;
    Ok(())
}

#[test]
fn d20b_scans_once_with_ten_thousand_unrelated_and_twenty_related_versions() {
    let mut measurements = Vec::new();
    let mut preserved_json = Vec::new();
    for related in [1, 20] {
        let repo = Repository::new(WorklogStore::memory().unwrap());
        let entry = repo.record_entry(&request()).unwrap();
        repo.store.with_transaction(|db| {
            deletion_report(db,"unrelated")?;
            deletion_report(db,"related")?;
            let source = deletion_snapshot("entry",&entry.entity_id);
            for n in 0..10_000 { deletion_version(db,"unrelated",&format!("u-{n}"),n + 1,&[],&[])?; }
            db.execute("UPDATE report_versions SET source_revision_manifest='[ ]',source_snapshot='[  ]' WHERE id='u-0'",[])?;
            for n in 0..related {
                let id = format!("r-{n}");
                deletion_version(db,"related",&id,n + 1,&[source.source.clone()],&[source.clone()])?;
                db.execute("INSERT INTO report_sources VALUES(?1,'entry',?2,1)",params![id,entry.entity_id])?;
            }
            let indirect = deletion_snapshot("daily_version", "r-0");
            deletion_run(db,"orphan","running",&[],&[indirect])?;
            deletion_run(db,"untouched","running",&[],&[])?;
            Ok(())
        }).unwrap();
        let trace = DeletionTrace::new(&repo);
        let started = std::time::Instant::now();
        repo.delete_entry(&deletion_request(&entry.entity_id, false))
            .unwrap();
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        let c = &trace.counts;
        println!(
            "D20_METRICS {}",
            serde_json::json!({"unrelatedVersions":10000,"relatedVersions":related,"versionScans":c.version_scans.get(),"runScans":c.run_scans.get(),"versionRows":c.version_rows.get(),"runRows":c.run_rows.get(),"jsonDocumentsParsed":2*(c.version_rows.get()+c.run_rows.get()),"versionUpdates":c.version_updates.get(),"unrelatedVersionUpdates":c.unrelated_updates.get(),"runUpdates":c.run_updates.get(),"deleteTransactionMs":elapsed,"sqliteBeginThroughCommitMs":c.transaction_ns.get() as f64/1_000_000.0})
        );
        assert!(
            c.transaction_ns.get() > 0,
            "SQLite BEGIN through completed COMMIT was traced"
        );
        measurements.push((
            related as usize,
            c.version_scans.get(),
            c.run_scans.get(),
            c.version_rows.get(),
            c.run_rows.get(),
            c.version_updates.get(),
            c.unrelated_updates.get(),
            c.run_updates.get(),
        ));
        drop(trace);
        preserved_json.push(repo.store.with_connection(|db| {
            let preserved:(String,String)=db.query_row("SELECT source_revision_manifest,source_snapshot FROM report_versions WHERE id='u-0'",[],|r|Ok((r.get(0)?,r.get(1)?)))?;
            Ok(preserved)
        }).unwrap());
    }
    for preserved in preserved_json {
        assert_eq!(preserved, ("[ ]".into(), "[  ]".into()));
    }
    for (
        related,
        version_scans,
        run_scans,
        version_rows,
        run_rows,
        updates,
        unrelated,
        run_updates,
    ) in measurements
    {
        assert_eq!(unrelated, 0);
        assert_eq!(
            version_scans, 1,
            "full version scan must not multiply by related versions"
        );
        assert_eq!(run_scans, 1);
        assert_eq!(version_rows, 10_000 + related);
        assert_eq!(run_rows, 2);
        assert_eq!(updates, related);
        assert_eq!(run_updates, 1);
    }
}

#[test]
fn d20b_closure_clears_deep_duplicate_and_snapshot_only_edges_preserving_bodies() {
    let repo = Repository::new(WorklogStore::memory().unwrap());
    let entry_request = request();
    let entry = repo.record_entry(&entry_request).unwrap();
    let source = deletion_snapshot("entry", &entry.entity_id);
    let other_kind = deletion_snapshot("different_kind", &entry.entity_id);
    repo.store
        .with_transaction(|db| {
            for id in ["daily", "weekly", "monthly", "deep", "unrelated"] {
                deletion_report(db, id)?;
            }
            deletion_version(
                db,
                "daily",
                "daily-v1",
                1,
                &[source.source.clone(), source.source.clone()],
                &[source.clone(), other_kind.clone()],
            )?;
            deletion_version(db, "daily", "daily-v2", 2, &[], &[])?;
            let daily = deletion_snapshot("daily_version", "daily-v2");
            deletion_version(db, "weekly", "weekly-v", 1, &[], &[daily])?;
            let weekly = deletion_snapshot("daily_version", "weekly-v");
            deletion_version(
                db,
                "monthly",
                "monthly-v",
                1,
                &[weekly.source.clone()],
                &[weekly],
            )?;
            deletion_version(db, "deep", "deep-v", 1, &[], &[])?;
            db.execute(
                "INSERT INTO report_sources VALUES('deep-v','daily_version','monthly-v',1)",
                [],
            )?;
            // A cycle and a duplicate diamond must not repeat work or recurse forever.
            db.execute(
                "INSERT INTO report_sources VALUES('daily-v2','daily_version','deep-v',1)",
                [],
            )?;
            db.execute(
                "INSERT INTO report_sources VALUES('deep-v','daily_version','daily-v1',1)",
                [],
            )?;
            deletion_version(
                db,
                "unrelated",
                "untouched-v",
                1,
                &[other_kind.source.clone()],
                &[other_kind.clone()],
            )?;
            for (id, state) in [
                ("queued", "queued"),
                ("running", "running"),
                ("retry", "retry_wait"),
                ("completed", "succeeded"),
            ] {
                deletion_run(
                    db,
                    id,
                    state,
                    &[],
                    &[deletion_snapshot("daily_version", "deep-v")],
                )?;
            }
            Ok(())
        })
        .unwrap();
    let deletion = deletion_request(&entry.entity_id, false);
    repo.delete_entry(&deletion).unwrap();
    for id in ["daily", "weekly", "monthly", "deep"] {
        let detail = repo.get_report(id).unwrap();
        assert!(detail.report.stale && detail.report.source_deleted);
        for version in detail.versions {
            assert_eq!(version.body_markdown, "retained body");
            assert_eq!(version.coverage_dates, vec!["2026-09-07"]);
            assert!(version.source_revision_manifest.is_empty());
            assert!(version
                .source_snapshot
                .iter()
                .all(|s| s.source.kind == "different_kind"));
        }
    }
    let unrelated = repo.get_report("unrelated").unwrap();
    assert!(!unrelated.report.stale && !unrelated.report.source_deleted);
    assert_eq!(unrelated.versions[0].source_snapshot.len(), 1);
    repo.store.with_connection(|db| {
        assert_eq!(db.query_row("SELECT count(*) FROM report_sources",[],|r|r.get::<_,i64>(0))?,0);
        assert_eq!(db.query_row("SELECT count(*) FROM report_runs WHERE source_snapshot='[]' AND source_manifest='[]'",[],|r|r.get::<_,i64>(0))?,4);
        assert_eq!(db.query_row("SELECT count(*) FROM report_runs WHERE state='cancelled'",[],|r|r.get::<_,i64>(0))?,3);
        assert_eq!(db.query_row("SELECT state FROM report_runs WHERE id='completed'",[],|r|r.get::<_,String>(0))?,"succeeded");
        Ok(())
    }).unwrap();
    let trace = DeletionTrace::new(&repo);
    assert_eq!(repo.delete_entry(&deletion).unwrap().status, "deleted");
    assert_eq!(repo.record_entry(&entry_request).unwrap().status, "deleted");
    assert_eq!(trace.counts.version_scans.get(), 0);
}

#[test]
fn d20b_entry_linked_delete_removes_closure_but_direct_report_delete_keeps_dependents() {
    for delete_entry in [false, true] {
        let repo = Repository::new(WorklogStore::memory().unwrap());
        let entry = repo.record_entry(&request()).unwrap();
        repo.store.with_transaction(|db| {
            for id in ["daily","weekly","monthly","unrelated"] { deletion_report(db,id)?; }
            let source=deletion_snapshot("entry",&entry.entity_id);
            deletion_version(db,"daily","daily-v",1,&[],&[source])?;
            deletion_version(db,"weekly","weekly-v",1,&[],&[deletion_snapshot("daily_version","daily-v")])?;
            deletion_version(db,"monthly","monthly-v",1,&[],&[deletion_snapshot("daily_version","weekly-v")])?;
            deletion_version(db,"unrelated","untouched-v",1,&[],&[])?;
            db.execute("INSERT INTO worklog_operations VALUES('synthetic-report-operation','hash','report','daily',1,NULL,'committed')",[])?;
            Ok(())
        }).unwrap();
        if delete_entry {
            repo.delete_entry(&deletion_request(&entry.entity_id, true))
                .unwrap();
        } else {
            repo.delete_report(&deletion_request("daily", true))
                .unwrap();
        }
        assert!(repo.get_report("daily").is_err());
        assert!(repo.get_report("unrelated").is_ok());
        for id in ["weekly", "monthly"] {
            let report = repo.get_report(id);
            if delete_entry {
                assert!(report.is_err());
            } else {
                let report = report.unwrap();
                assert!(report.report.stale && report.report.source_deleted);
                assert_eq!(report.versions[0].body_markdown, "retained body");
                assert!(report.versions[0].source_snapshot.is_empty());
            }
        }
        assert_eq!(
            repo.operation("synthetic-report-operation")
                .unwrap()
                .unwrap()
                .status,
            "deleted"
        );
    }
}

#[test]
fn d20b_snapshot_only_run_cannot_publish_or_renew_after_erasure() {
    use super::super::repository_runtime::ClaimedRun;
    let repo = Repository::new(WorklogStore::memory().unwrap());
    let entry = repo.record_entry(&request()).unwrap();
    let source = deletion_snapshot("entry", &entry.entity_id);
    repo.store
        .with_transaction(|db| deletion_run(db, "in-flight", "running", &[], &[source.clone()]))
        .unwrap();
    let run = ClaimedRun {
        id: "in-flight".into(),
        attempt: 1,
        max_attempt: 3,
        kind: ReportKind::Daily,
        period_start: "2026-09-07".into(),
        period_end: "2026-09-07".into(),
        model_id: "p/m".into(),
        base_revision: None,
        sources: vec![source],
    };
    repo.delete_entry(&deletion_request(&entry.entity_id, false))
        .unwrap();
    let now = chrono::Utc::now();
    assert_eq!(
        repo.publish_run(&run, ("late source-containing text", now))
            .unwrap_err()
            .code,
        "RUN_CANCELLED"
    );
    assert_eq!(repo.renew_run(&run, now).unwrap_err().code, "RUN_CANCELLED");
    repo.store
        .with_connection(|db| {
            assert_eq!(
                db.query_row("SELECT count(*) FROM reports", [], |r| r.get::<_, i64>(0))?,
                0
            );
            assert_eq!(
                db.query_row(
                    "SELECT source_snapshot FROM report_runs WHERE id='in-flight'",
                    [],
                    |r| r.get::<_, String>(0)
                )?,
                "[]"
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn d20b_parse_and_write_failure_roll_back_all_erasure_changes() {
    for malformed in [true, false] {
        let repo = Repository::new(WorklogStore::memory().unwrap());
        let entry = repo.record_entry(&request()).unwrap();
        let source = deletion_snapshot("entry", &entry.entity_id);
        repo.store.with_transaction(|db| {
            deletion_report(db,"daily")?;
            deletion_version(db,"daily","daily-v",1,&[source.source.clone()],&[source.clone()])?;
            db.execute("INSERT INTO report_sources VALUES('daily-v','entry',?1,1)",[&entry.entity_id])?;
            deletion_run(db,"running","running",&[source.source.clone()],&[source.clone()])?;
            if malformed { db.execute("UPDATE report_runs SET source_snapshot='not JSON' WHERE id='running'",[])?; }
            else { db.execute_batch("CREATE TEMP TRIGGER reject_erasure BEFORE UPDATE ON report_runs BEGIN SELECT RAISE(ABORT,'injected write failure'); END;")?; }
            Ok(())
        }).unwrap();
        let deletion = deletion_request(&entry.entity_id, false);
        assert!(repo.delete_entry(&deletion).is_err());
        assert!(repo.operation(&deletion.request_id).unwrap().is_none());
        let detail = repo.get_report("daily").unwrap();
        assert!(!detail.report.stale && !detail.report.source_deleted);
        assert_eq!(
            detail.versions[0].source_snapshot[0].source.id,
            entry.entity_id
        );
        assert_eq!(
            repo.operation(&entry.operation_id).unwrap().unwrap().status,
            "committed"
        );
        repo.store
            .with_connection(|db| {
                assert_eq!(
                    db.query_row(
                        "SELECT count(*) FROM work_entries WHERE id=?1",
                        [&entry.entity_id],
                        |r| r.get::<_, i64>(0)
                    )?,
                    1
                );
                assert_eq!(
                    db.query_row("SELECT count(*) FROM report_sources", [], |r| r
                        .get::<_, i64>(0))?,
                    1
                );
                Ok(())
            })
            .unwrap();
    }
}

#[test]
fn d20b_deleting_report_cancels_existing_regeneration_without_blocking_new_intent() {
    use super::super::contract_runtime::GenerateReport;
    for delete_linked in [false, true] {
        let repo = Repository::new(WorklogStore::memory().unwrap());
        let entry = repo.record_entry(&request()).unwrap();
        let generate = || GenerateReport {
            request_id: uuid::Uuid::new_v4().to_string(),
            kind: ReportKind::Daily,
            period_start: "2026-09-07".into(),
            period_end: "2026-09-07".into(),
            model_id: "p/m".into(),
        };
        repo.generate_report(&generate()).unwrap();
        let first = repo.claim_run(chrono::Utc::now()).unwrap().unwrap();
        let report = repo
            .publish_run(&first, ("initial archive", chrono::Utc::now()))
            .unwrap()
            .unwrap();
        // Moving the old entry gives a real regeneration entirely different source material.
        repo.update_entry(&UpdateEntry {
            request_id: uuid::Uuid::new_v4().to_string(),
            id: entry.entity_id.clone(),
            expected_revision: 1,
            business_date: "2026-09-08".into(),
            project: None,
            text: "Moved original work".into(),
            status: EntryStatus::Done,
        })
        .unwrap();
        let mut newer_entry = request();
        newer_entry.text = "Independent later work".into();
        newer_entry.original_text = newer_entry.text.clone();
        let newer = repo.record_entry(&newer_entry).unwrap();
        repo.generate_report(&generate()).unwrap();
        let in_flight = repo.claim_run(chrono::Utc::now()).unwrap().unwrap();
        assert_eq!(in_flight.sources.len(), 1);
        assert_eq!(in_flight.sources[0].source.id, newer.entity_id);
        let queued = repo.generate_report(&generate()).unwrap();
        let retry = repo.generate_report(&generate()).unwrap();
        repo.store.with_transaction(|db| {
            db.execute("UPDATE report_runs SET state='retry_wait' WHERE id=?1",[&retry.entity_id])?;
            deletion_run(db,"other-kind","running",&[],&[])?;
            db.execute("UPDATE report_runs SET kind='weekly' WHERE id='other-kind'",[])?;
            deletion_run(db,"other-period","running",&[],&[])?;
            db.execute("UPDATE report_runs SET period_start='2026-09-08',period_end='2026-09-08' WHERE id='other-period'",[])?;
            Ok(())
        }).unwrap();
        if delete_linked {
            let mut deletion = deletion_request(&entry.entity_id, true);
            deletion.expected_revision = 2;
            repo.delete_entry(&deletion).unwrap();
        } else {
            repo.delete_report(&deletion_request(&report, false))
                .unwrap();
        }
        assert!(repo.get_report(&report).is_err());
        assert_eq!(
            repo.publish_run(&in_flight, ("late regenerated archive", chrono::Utc::now()))
                .unwrap_err()
                .code,
            "RUN_CANCELLED"
        );
        assert_eq!(
            repo.renew_run(&in_flight, chrono::Utc::now())
                .unwrap_err()
                .code,
            "RUN_CANCELLED"
        );
        repo.store
            .with_connection(|db| {
                for id in [&in_flight.id, &queued.entity_id, &retry.entity_id] {
                    assert_eq!(
                        db.query_row("SELECT state FROM report_runs WHERE id=?1", [id], |r| {
                            r.get::<_, String>(0)
                        })?,
                        "cancelled"
                    );
                }
                assert_eq!(
                    db.query_row(
                        "SELECT state FROM report_runs WHERE id=?1",
                        [&first.id],
                        |r| r.get::<_, String>(0)
                    )?,
                    "succeeded"
                );
                for id in ["other-kind", "other-period"] {
                    assert_eq!(
                        db.query_row("SELECT state FROM report_runs WHERE id=?1", [id], |r| {
                            r.get::<_, String>(0)
                        })?,
                        "running"
                    );
                }
                Ok(())
            })
            .unwrap();
        repo.generate_report(&generate()).unwrap();
        let fresh = repo.claim_run(chrono::Utc::now()).unwrap().unwrap();
        let fresh_report = repo
            .publish_run(&fresh, ("new explicit request", chrono::Utc::now()))
            .unwrap()
            .unwrap();
        assert_ne!(fresh_report, report);
    }
}

#[test]
fn d20b_target_cancellation_failure_rolls_back_report_deletion() {
    let repo = Repository::new(WorklogStore::memory().unwrap());
    let report = repo
        .save_report(&SaveReport {
            request_id: uuid::Uuid::new_v4().to_string(),
            kind: ReportKind::Daily,
            period_start: "2026-09-07".into(),
            period_end: "2026-09-07".into(),
            expected_revision: None,
            body_markdown: "Original archive".into(),
        })
        .unwrap();
    repo.store.with_transaction(|db| {
        deletion_run(db,"queued","queued",&[],&[])?;
        db.execute_batch("CREATE TEMP TRIGGER reject_cancellation BEFORE UPDATE ON report_runs BEGIN SELECT RAISE(ABORT,'injected cancellation failure'); END;")?;
        Ok(())
    }).unwrap();
    let deletion = deletion_request(&report.entity_id, false);
    assert!(repo.delete_report(&deletion).is_err());
    assert!(repo.operation(&deletion.request_id).unwrap().is_none());
    assert_eq!(
        repo.get_report(&report.entity_id).unwrap().versions[0].body_markdown,
        "Original archive"
    );
    assert_eq!(
        repo.operation(&report.operation_id)
            .unwrap()
            .unwrap()
            .status,
        "committed"
    );
    repo.store
        .with_connection(|db| {
            assert_eq!(
                db.query_row("SELECT state FROM report_runs WHERE id='queued'", [], |r| r
                    .get::<_, String>(0))?,
                "queued"
            );
            Ok(())
        })
        .unwrap();
}
