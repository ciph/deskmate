use super::*;
use crate::{
    agent::{NativeMessage, NativePart, RunOutcome, RunRecord},
    history::{
        catalog_import::new_entry,
        catalog_model::*,
        history_tests::{agent, ordinary},
        storage::{load_path, persist_path},
    },
};
use std::{fs, path::PathBuf, sync::mpsc, time::Duration};

struct Fixture {
    root: tempfile::TempDir,
    path: PathBuf,
    catalog_path: PathBuf,
    origin: RunRecord,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let workspace = root.path().join("workspace");
        fs::create_dir(&workspace).unwrap();
        let path = root.path().join("history.json");
        let catalog_path = root.path().join("catalog.sqlite");
        let origin = RunRecord {
            run_id: "msg_origin".into(),
            session_id: Some("ses_agent".into()),
            workspace_path: workspace.canonicalize().unwrap(),
            created_at: "2026-01-01T00:00:00Z".into(),
            ended_at: Some("2026-01-01T00:00:01Z".into()),
            outcome: Some(RunOutcome::Completed),
            error_summary: None,
            pending_outcome: None,
            pending_error_summary: None,
            message_ids: Vec::new(),
            pending_usage_message_ids: Vec::new(),
            part_ids: Vec::new(),
            call_ids: Vec::new(),
            initial_input: None,
        };
        Self {
            root,
            path,
            catalog_path,
            origin,
        }
    }
    fn catalog(&self) -> CatalogStore {
        CatalogStore::open(&self.catalog_path).unwrap()
    }
    fn prepare(
        &self,
        list: &[HistorySession],
        messages: &[NativeMessage],
    ) -> PreparedAgentSnapshot {
        PreparedAgentSnapshot::prepare(
            &self.path,
            list.to_vec(),
            Some(&self.origin),
            &self.origin.workspace_path,
            AgentHistorySnapshot {
                session_id: "ses_agent",
                messages,
            },
            self.catalog(),
        )
        .unwrap()
    }
    fn row(&self) -> CatalogEntry {
        let mut row = new_entry(
            CatalogIdentity::Native {
                sidecar_id: SIDECAR_ID.into(),
                directory: super::super::catalog_import::canonical_directory(
                    &self.origin.workspace_path.to_string_lossy(),
                ),
                session_id: "ses_agent".into(),
            },
            "Agent".into(),
            ConversationSource::Workbench,
            1,
            1,
        );
        row.ownership = Ownership::Agent;
        row
    }
    fn pending(&self) -> usize {
        fs::read_dir(self.root.path())
            .unwrap()
            .filter(|entry| {
                entry
                    .as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".history-")
            })
            .count()
    }
}

fn message() -> NativeMessage {
    NativeMessage {
        usage: None,
        system: None,
        id: "msg_reply".into(),
        role: Some("assistant".into()),
        created: Some(20),
        parent_id: Some("msg_later_run".into()),
        completed: true,
        finish: Some("stop".into()),
        error: None,
        parts: vec![NativePart {
            synthetic: false,
            id: "part_reply".into(),
            kind: Some("text".into()),
            text: Some("durable reply".into()),
            call_id: None,
            tool: None,
            state: None,
        }],
    }
}

#[test]
fn revision_follows_successful_replacement_and_rejects_failed_noop_and_late_projection() {
    let fixture = Fixture::new();
    let state = HistoryState::new(std::sync::Mutex::new(vec![agent("ses_agent")]));
    let baseline = state.0.lock().unwrap().clone();
    let messages = [message()];
    // A directory at the exact destination makes the real replacement fail.
    fs::create_dir(&fixture.path).unwrap();
    assert!(fixture.prepare(&baseline, &messages).commit_to_state(&state).is_err());
    assert_eq!(state.revision(), 0);
    assert_eq!(*state.0.lock().unwrap(), baseline);
    fs::remove_dir(&fixture.path).unwrap();
    let first = fixture.prepare(&baseline, &messages);
    let late = fixture.prepare(&baseline, &messages);
    first.commit_to_state(&state).unwrap();
    assert_eq!(state.revision(), 1);
    assert_eq!(late.commit_to_state(&state), Err("history_snapshot_superseded".into()));
    assert_eq!(state.revision(), 1);
    let current = state.0.lock().unwrap().clone();
    fixture.prepare(&current, &messages).commit_to_state(&state).unwrap();
    assert_eq!(state.revision(), 1);
    assert_eq!(load_path(&fixture.path).unwrap(), current);
}

#[test]
fn prepare_does_not_publish_and_commit_keeps_title_and_first_origin() {
    let fixture = Fixture::new();
    let mut list = vec![agent("ses_agent"), ordinary("untouched")];
    list[0].title = "User title".into();
    persist_path(&fixture.path, &list).unwrap();
    let before = list.clone();
    let staged = fixture.prepare(&list, &[message()]);
    assert_eq!(fixture.pending(), 1);
    assert_eq!(load_path(&fixture.path).unwrap(), before);
    assert_eq!(list, before);
    staged
        .commit_into(&mut list, PreparedHistoryFile::commit)
        .unwrap();
    assert_eq!(list[0].title, "User title");
    assert_eq!(list[0].origin_run_id.as_deref(), Some("msg_origin"));
    assert_eq!(list[0].messages[0].text, "durable reply");
    assert_eq!(list[1], before[1]);
    assert_eq!(load_path(&fixture.path).unwrap(), list);
    assert_eq!(fixture.pending(), 0);
}

#[test]
fn concurrent_rows_titles_ownership_and_deletion_supersede_full_file_snapshot() {
    let edits: [fn(&mut Vec<HistorySession>); 7] = [
        |list| list[0].title = "New title".into(),
        |list| list[0].origin_run_id = Some("msg_new_owner".into()),
        |list| list[0].deleted = true,
        |list| {
            list[0].local_link = Some(CatalogIdentity::Legacy {
                history_id: "other".into(),
            })
        },
        |list| list.push(ordinary("new_row")),
        |list| {
            list.remove(0);
        },
        |list| list[1].messages[0].text = "Other row changed".into(),
    ];
    for edit in edits {
        let fixture = Fixture::new();
        let mut list = vec![agent("ses_agent"), ordinary("other")];
        persist_path(&fixture.path, &list).unwrap();
        let staged = fixture.prepare(&list, &[message()]);
        edit(&mut list);
        persist_path(&fixture.path, &list).unwrap();
        let latest = list.clone();
        assert_eq!(
            staged.commit_into(&mut list, |_| panic!("stale replace")),
            Err("history_snapshot_superseded".into())
        );
        assert_eq!(list, latest);
        assert_eq!(load_path(&fixture.path).unwrap(), latest);
        assert_eq!(fixture.pending(), 0);
    }
}

#[test]
fn competing_preparations_never_overwrite_first_published_snapshot() {
    let fixture = Fixture::new();
    let mut list = vec![agent("ses_agent")];
    let first = fixture.prepare(&list, &[message()]);
    let mut later_message = message();
    later_message.parts[0].text = Some("stale second copy".into());
    let second = fixture.prepare(&list, &[later_message]);
    assert_eq!(fixture.pending(), 2);
    first
        .commit_into(&mut list, PreparedHistoryFile::commit)
        .unwrap();
    assert_eq!(
        second.commit_into(&mut list, |_| panic!("stale replace")),
        Err("history_snapshot_superseded".into())
    );
    assert_eq!(list[0].messages[0].text, "durable reply");
    assert_eq!(load_path(&fixture.path).unwrap(), list);
    assert_eq!(fixture.pending(), 0);
}

#[test]
fn later_catalog_tombstone_and_workbench_owner_prevent_publish() {
    for deleted in [false, true] {
        let fixture = Fixture::new();
        let mut list = vec![agent("ses_agent")];
        persist_path(&fixture.path, &list).unwrap();
        let staged = fixture.prepare(&list, &[message()]);
        let mut row = fixture.row();
        let expected = if deleted {
            row.tombstone = Some(DeletionTombstone {
                requested_at: 1,
                remote_deleted: false,
            });
            "history_deleted"
        } else {
            row.ownership = Ownership::Workbench;
            "history_agent_owned"
        };
        fixture
            .catalog()
            .update(|rows| {
                rows.push(row);
                Ok(())
            })
            .unwrap();
        let before = list.clone();
        assert_eq!(
            staged.commit_into(&mut list, |_| panic!("forbidden replace")),
            Err(expected.into())
        );
        assert_eq!(list, before);
        assert_eq!(load_path(&fixture.path).unwrap(), before);
        assert_eq!(fixture.pending(), 0);
    }
}

#[test]
fn tombstones_with_colliding_session_but_other_directory_or_sidecar_do_not_block() {
    let fixture = Fixture::new();
    let mut list = vec![agent("ses_agent")];
    let staged = fixture.prepare(&list, &[message()]);
    let mut directory_collision = fixture.row();
    let CatalogIdentity::Native { directory, .. } = &mut directory_collision.identity else {
        unreachable!()
    };
    *directory = "c:/unrelated".into();
    directory_collision.tombstone = Some(DeletionTombstone {
        requested_at: 1,
        remote_deleted: true,
    });
    let mut sidecar_collision = fixture.row();
    let CatalogIdentity::Native { sidecar_id, .. } = &mut sidecar_collision.identity else {
        unreachable!()
    };
    *sidecar_id = "other-sidecar".into();
    sidecar_collision.tombstone = Some(DeletionTombstone {
        requested_at: 1,
        remote_deleted: true,
    });
    fixture
        .catalog()
        .update(|rows| {
            rows.extend([directory_collision, sidecar_collision]);
            Ok(())
        })
        .unwrap();
    staged
        .commit_into(&mut list, PreparedHistoryFile::commit)
        .unwrap();
    assert_eq!(list[0].messages.len(), 1);
    assert_eq!(fixture.pending(), 0);
}

#[test]
fn legacy_tombstone_rejects_projection_before_or_during_native_coalescing() {
    for native_present in [false, true] {
        let fixture = Fixture::new();
        let mut list = vec![agent("ses_agent")];
        let staged = fixture.prepare(&list, &[message()]);
        let mut legacy = new_entry(
            CatalogIdentity::Legacy {
                history_id: "ses_agent".into(),
            },
            "Deleted legacy".into(),
            ConversationSource::Legacy,
            1,
            1,
        );
        legacy.tombstone = Some(DeletionTombstone {
            requested_at: 1,
            remote_deleted: true,
        });
        fixture
            .catalog()
            .update(|rows| {
                rows.push(legacy);
                if native_present {
                    rows.push(fixture.row());
                }
                Ok(())
            })
            .unwrap();
        assert_eq!(
            staged.commit_into(&mut list, |_| panic!("deleted replace")),
            Err("history_deleted".into())
        );
        assert!(list[0].messages.is_empty());
        assert!(!fixture.path.exists());
        assert_eq!(fixture.pending(), 0);
    }
}

#[test]
fn unrelated_legacy_owner_or_workspace_is_preserved_without_temporary_write() {
    let fixture = Fixture::new();
    let mut list = vec![ordinary("ses_agent")];
    let staged = fixture.prepare(&list, &[message()]);
    staged
        .commit_into(&mut list, |_| panic!("ordinary legacy replaced"))
        .unwrap();
    assert_eq!(list, vec![ordinary("ses_agent")]);
    let mut list = vec![agent("ses_agent")];
    let staged = PreparedAgentSnapshot::prepare(
        &fixture.path,
        list.clone(),
        Some(&fixture.origin),
        Path::new("c:/unrelated"),
        AgentHistorySnapshot {
            session_id: "ses_agent",
            messages: &[message()],
        },
        fixture.catalog(),
    )
    .unwrap();
    staged
        .commit_into(&mut list, |_| panic!("other workspace replaced"))
        .unwrap();
    assert_eq!(list, vec![agent("ses_agent")]);
    assert!(!fixture.path.exists());
    assert_eq!(fixture.pending(), 0);
}

#[test]
fn missing_or_mismatched_origin_is_not_silently_adopted() {
    let fixture = Fixture::new();
    let mut wrong = fixture.origin.clone();
    wrong.session_id = Some("ses_other".into());
    for origin in [None, Some(&wrong)] {
        let result = PreparedAgentSnapshot::prepare(
            &fixture.path,
            vec![agent("ses_agent")],
            origin,
            &fixture.origin.workspace_path,
            AgentHistorySnapshot {
                session_id: "ses_agent",
                messages: &[message()],
            },
            fixture.catalog(),
        );
        assert_eq!(
            result.err().as_deref(),
            Some("history_agent_origin_unknown")
        );
        assert_eq!(fixture.pending(), 0);
    }
}

#[test]
fn unchanged_projection_never_touches_history_storage() {
    let fixture = Fixture::new();
    let mut list = vec![agent("ses_agent")];
    fixture
        .prepare(&list, &[message()])
        .commit_into(&mut list, PreparedHistoryFile::commit)
        .unwrap();
    fs::remove_file(&fixture.path).unwrap();
    fs::create_dir(&fixture.path).unwrap();
    let before = list.clone();
    let staged = fixture.prepare(&list, &[message()]);
    assert_eq!(fixture.pending(), 0);
    staged
        .commit_into(&mut list, |_| panic!("unchanged replace"))
        .unwrap();
    assert_eq!(list, before);
    assert!(fixture.path.is_dir());
}

#[test]
fn replacement_failure_cleans_pending_and_never_publishes_ram() {
    let fixture = Fixture::new();
    let mut list = vec![agent("ses_agent")];
    let before = list.clone();
    let staged = fixture.prepare(&list, &[message()]);
    fs::create_dir(&fixture.path).unwrap();
    assert_eq!(
        staged.commit_into(&mut list, PreparedHistoryFile::commit),
        Err("history_storage_unavailable".into())
    );
    assert_eq!(list, before);
    assert!(fixture.path.is_dir());
    assert_eq!(fixture.pending(), 0);
}

#[test]
fn dropped_preparation_and_invalid_snapshot_leave_no_pending_files() {
    let fixture = Fixture::new();
    let list = vec![agent("ses_agent")];
    drop(fixture.prepare(&list, &[message()]));
    assert_eq!(fixture.pending(), 0);
    assert!(!fixture.path.exists());
    let mut invalid = message();
    invalid.created = None;
    let result = PreparedAgentSnapshot::prepare(
        &fixture.path,
        list.clone(),
        Some(&fixture.origin),
        &fixture.origin.workspace_path,
        AgentHistorySnapshot {
            session_id: "ses_agent",
            messages: &[invalid],
        },
        fixture.catalog(),
    );
    assert_eq!(result.err().as_deref(), Some("history_snapshot_invalid"));
    assert_eq!(fixture.pending(), 0);
    assert_eq!(list, vec![agent("ses_agent")]);
}

#[test]
fn catalog_read_transaction_serializes_tombstone_commit_after_atomic_replace() {
    let fixture = Fixture::new();
    fixture
        .catalog()
        .update(|rows| {
            rows.push(fixture.row());
            Ok(())
        })
        .unwrap();
    let mut list = vec![agent("ses_agent")];
    let staged = fixture.prepare(&list, &[message()]);
    let writer = fixture.catalog();
    let (edited_tx, edited_rx) = mpsc::channel();
    let (committed_tx, committed_rx) = mpsc::channel();
    let mut writer_thread = None;
    staged
        .commit_into(&mut list, |file| {
            writer_thread = Some(std::thread::spawn(move || {
                edited_tx.send(()).unwrap();
                let result = writer.update(|rows| {
                    rows[0].tombstone = Some(DeletionTombstone {
                        requested_at: 2,
                        remote_deleted: false,
                    });
                    Ok(())
                });
                committed_tx.send(result).unwrap();
            }));
            edited_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            assert!(matches!(
                committed_rx.recv_timeout(Duration::from_millis(50)),
                Err(mpsc::RecvTimeoutError::Timeout)
            ));
            file.commit()
        })
        .unwrap();
    committed_rx
        .recv_timeout(Duration::from_secs(2))
        .unwrap()
        .unwrap();
    writer_thread.unwrap().join().unwrap();
    assert!(fixture
        .catalog()
        .get(&fixture.row().key())
        .unwrap()
        .tombstone
        .is_some());
    assert_eq!(load_path(&fixture.path).unwrap(), list);
    assert_eq!(fixture.pending(), 0);
    // A subsequent archived snapshot sees the committed tombstone and cannot resurrect it.
    let subsequent = fixture.prepare(&list, &[message()]);
    assert_eq!(
        subsequent.commit_into(&mut list, |_| panic!("resurrection")),
        Err("history_deleted".into())
    );
}

#[test]
fn busy_catalog_fails_without_waiting_or_publishing_history() {
    for behavior in [
        rusqlite::TransactionBehavior::Immediate,
        rusqlite::TransactionBehavior::Exclusive,
    ] {
        let fixture = Fixture::new();
        let mut list = vec![agent("ses_agent")];
        persist_path(&fixture.path, &list).unwrap();
        let staged = fixture.prepare(&list, &[message()]);
        let mut blocker = rusqlite::Connection::open(&fixture.catalog_path).unwrap();
        let _transaction = blocker.transaction_with_behavior(behavior).unwrap();
        let started = std::time::Instant::now();
        let error = staged
            .commit_into(&mut list, |_| panic!("busy catalog replaced history"))
            .unwrap_err();
        assert_eq!(error, "history_catalog_unavailable");
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "busy catalog must not wait for the default 5 seconds"
        );
        assert_eq!(list, vec![agent("ses_agent")]);
        assert_eq!(load_path(&fixture.path).unwrap(), list);
        assert_eq!(fixture.pending(), 0);
    }
}

#[test]
fn wal_catalog_guard_still_excludes_concurrent_tombstone_writers() {
    let fixture = Fixture::new();
    fixture.catalog();
    let writer = rusqlite::Connection::open(&fixture.catalog_path).unwrap();
    writer.pragma_update(None, "journal_mode", "WAL").unwrap();
    writer.busy_timeout(Duration::ZERO).unwrap();
    let mut list = vec![agent("ses_agent")];
    let staged = fixture.prepare(&list, &[message()]);
    staged.commit_into(&mut list, |file| {
        let error = writer.execute_batch("BEGIN IMMEDIATE").unwrap_err();
        assert!(matches!(error, rusqlite::Error::SqliteFailure(code, _) if code.code == rusqlite::ErrorCode::DatabaseBusy));
        file.commit()
    }).unwrap();
    writer.execute_batch("BEGIN IMMEDIATE; ROLLBACK;").unwrap();
    assert_eq!(fixture.pending(), 0);
}

#[test]
fn malformed_catalog_metadata_fails_closed_without_becoming_a_tombstone() {
    let fixture = Fixture::new();
    let mut list = vec![agent("ses_agent")];
    persist_path(&fixture.path, &list).unwrap();
    let staged = fixture.prepare(&list, &[message()]);
    let connection = rusqlite::Connection::open(&fixture.catalog_path).unwrap();
    connection
        .execute(
            "INSERT INTO entries(key, metadata) VALUES (?1, ?2)",
            [fixture.row().key(), "{broken".into()],
        )
        .unwrap();
    assert_eq!(
        staged.commit_into(&mut list, |_| panic!("invalid metadata replaced history")),
        Err("history_catalog_invalid".into())
    );
    assert_eq!(list, vec![agent("ses_agent")]);
    assert_eq!(load_path(&fixture.path).unwrap(), list);
    assert_eq!(fixture.pending(), 0);
}

#[test]
fn busy_history_writer_does_not_block_control_or_publish_prepared_snapshot() {
    let fixture = Fixture::new();
    let state = HistoryState::new(std::sync::Mutex::new(vec![agent("ses_agent")]));
    let staged = fixture.prepare(&state.0.lock().unwrap(), &[message()]);
    let busy_writer = state.0.lock().unwrap();
    assert_eq!(
        staged.commit_to_state(&state),
        Err("history_state_busy".into())
    );
    assert_eq!(*busy_writer, vec![agent("ses_agent")]);
    assert!(!fixture.path.exists());
    assert_eq!(fixture.pending(), 0);
}
