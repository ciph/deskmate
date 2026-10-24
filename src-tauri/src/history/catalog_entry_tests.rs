use super::*;
use crate::history::{catalog_cache::Revision, catalog_import::new_entry, catalog_model::*};
use std::sync::{Arc, Barrier};

fn entry(directory: &str) -> CatalogEntry {
    new_entry(CatalogIdentity::Native { sidecar_id: SIDECAR_ID.into(), directory: directory.into(), session_id: "ses_same".into() }, "Native".into(), ConversationSource::LightChat, 1, 2)
}

fn insert(store: &CatalogStore, row: CatalogEntry) {
    store.update_entry(&row.key(), |current| { *current = Some(row); Ok(()) }).unwrap();
}

#[test]
fn single_entry_preserves_colliding_identities_preferences_and_unchanged_rows() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("catalog.sqlite");
    let store = CatalogStore::open(&path).unwrap();
    let a = entry("c:/project-a");
    let b = entry("c:/project-b");
    let mut c = a.clone();
    if let CatalogIdentity::Native { sidecar_id, .. } = &mut c.identity { *sidecar_id = "other-sidecar".into(); }
    for row in [&a, &b, &c] { insert(&store, row.clone()); }
    let selected = ConversationModelSelection::Override { configured_provider_id: "provider".into(), sidecar_id: "runtime".into(), model_id: "model".into() };
    store.set_model_selection(&a.key(), &selected).unwrap();
    let connection = Connection::open(&path).unwrap();
    connection.execute_batch("CREATE TABLE writes(kind TEXT); CREATE TRIGGER track_insert AFTER INSERT ON entries BEGIN INSERT INTO writes VALUES('insert'); END; CREATE TRIGGER track_update AFTER UPDATE ON entries BEGIN INSERT INTO writes VALUES('update'); END; CREATE TRIGGER track_delete AFTER DELETE ON entries BEGIN INSERT INTO writes VALUES('delete'); END;").unwrap();
    store.update_entry(&a.key(), |row| { row.as_mut().unwrap().pinned = true; Ok(()) }).unwrap();
    store.update_entry(&a.key(), |_| Ok(())).unwrap();
    store.update_entry("missing", |row| { assert!(row.is_none()); Ok(()) }).unwrap();
    let count: i64 = connection.query_row("SELECT COUNT(*) FROM writes", [], |row| row.get(0)).unwrap();
    assert_eq!(count, 1);
    let reopened = CatalogStore::open(&path).unwrap();
    assert!(reopened.get(&a.key()).unwrap().pinned);
    assert_eq!(reopened.get(&a.key()).unwrap().updated, a.updated);
    assert_eq!(reopened.model_selection(&a.key()).unwrap(), selected);
    assert_eq!(reopened.get(&b.key()).unwrap(), b);
    assert_eq!(reopened.get(&c.key()).unwrap(), c);
}

#[test]
fn single_entry_normalizes_directory_but_cannot_rekey_or_insert_invalid_identity() {
    let root = tempfile::tempdir().unwrap();
    let store = CatalogStore::open(&root.path().join("catalog.sqlite")).unwrap();
    let a = entry("C:\\project-a\\.");
    insert(&store, a.clone());
    let normalized = store.get(&a.key()).unwrap();
    assert!(matches!(&normalized.identity, CatalogIdentity::Native { directory, .. } if directory == "c:/project-a"));
    let b = entry("c:/project-b");
    insert(&store, b.clone());
    assert_eq!(store.update_entry(&a.key(), |row| { *row = Some(b.clone()); Ok(()) }).unwrap_err(), "history_identity_invalid");
    assert_eq!(store.update_entry(&a.key(), |row| {
        if let CatalogIdentity::Native { session_id, .. } = &mut row.as_mut().unwrap().identity { *session_id = "bad/id".into(); }
        Ok(())
    }).unwrap_err(), "history_identity_invalid");
    assert_eq!(store.update_entry("wrong-key", |row| { *row = Some(b.clone()); Ok(()) }).unwrap_err(), "history_identity_invalid");
    assert_eq!(store.get(&a.key()).unwrap(), normalized);
    assert_eq!(store.get(&b.key()).unwrap(), b);
}

#[test]
fn single_entry_isolates_unrelated_corruption_but_full_validation_still_rejects_it() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("catalog.sqlite");
    let store = CatalogStore::open(&path).unwrap();
    let a = entry("/project");
    insert(&store, a.clone());
    let connection = Connection::open(&path).unwrap();
    connection.execute("INSERT INTO entries VALUES ('bad', 'invalid')", []).unwrap();
    store.update_entry(&a.key(), |row| { row.as_mut().unwrap().archived = true; Ok(()) }).unwrap();
    assert_eq!(store.update_entry::<()>("bad", |_| panic!("corrupt target reached editor")).unwrap_err(), "history_catalog_invalid");
    assert_eq!(store.all().unwrap_err(), "history_catalog_invalid");
    assert_eq!(store.update(|_| Ok(())).unwrap_err(), "history_catalog_invalid");
    let corrupt: String = connection.query_row("SELECT metadata FROM entries WHERE key='bad'", [], |row| row.get(0)).unwrap();
    assert_eq!(corrupt, "invalid");
    // A valid JSON payload stored under a different identity is also rejected.
    connection.execute("UPDATE entries SET metadata=?1 WHERE key='bad'", [serde_json::to_string(&a).unwrap()]).unwrap();
    assert_eq!(store.update_entry("bad", |_| Ok(())).unwrap_err(), "history_catalog_invalid");
}

#[test]
fn single_entry_failure_rolls_back_row_preference_and_revision() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("catalog.sqlite");
    let revision = Arc::new(Revision::default());
    let store = CatalogStore::open(&path).unwrap().track_associations(revision.clone());
    let a = entry("/project");
    insert(&store, a.clone());
    store.set_model_selection(&a.key(), &ConversationModelSelection::Inherit).unwrap();
    assert_eq!(store.update_entry(&a.key(), |row| { row.as_mut().unwrap().source = ConversationSource::Workbench; Err::<(), _>("edit_failed".into()) }).unwrap_err(), "edit_failed");
    let connection = Connection::open(&path).unwrap();
    connection.execute_batch("CREATE TRIGGER reject_preference_delete BEFORE DELETE ON model_selections BEGIN SELECT RAISE(ABORT, 'synthetic failure'); END; CREATE TRIGGER reject_update BEFORE UPDATE ON entries BEGIN SELECT RAISE(ABORT, 'synthetic failure'); END;").unwrap();
    assert_eq!(store.update_entry(&a.key(), |row| { *row = None; Ok(()) }).unwrap_err(), "history_catalog_unavailable");
    assert_eq!(store.update_entry(&a.key(), |row| { row.as_mut().unwrap().source = ConversationSource::Workbench; Ok(()) }).unwrap_err(), "history_catalog_unavailable");
    assert_eq!(store.get(&a.key()).unwrap(), a);
    assert_eq!(revision.get(), 1);
    assert_eq!(connection.query_row("SELECT COUNT(*) FROM model_selections", [], |row| row.get::<_, i64>(0)).unwrap(), 1);
    connection.execute_batch("DROP TRIGGER reject_preference_delete; DROP TRIGGER reject_update;").unwrap();
    store.update_entry(&a.key(), |row| { *row = None; Ok(()) }).unwrap();
    assert_eq!(revision.get(), 2);
    assert_eq!(store.get(&a.key()).unwrap_err(), "history_not_found");
    assert_eq!(connection.query_row("SELECT COUNT(*) FROM model_selections", [], |row| row.get::<_, i64>(0)).unwrap(), 0);
}

#[test]
fn single_entry_only_committed_native_associations_invalidate_imports() {
    let root = tempfile::tempdir().unwrap();
    let revision = Arc::new(Revision::default());
    let store = CatalogStore::open(&root.path().join("catalog.sqlite")).unwrap().track_associations(revision.clone());
    let a = entry("/project");
    insert(&store, a.clone());
    assert_eq!(revision.get(), 1);
    store.update_entry(&a.key(), |row| {
        let row = row.as_mut().unwrap();
        row.pinned = true; row.archived = true;
        row.tombstone = Some(DeletionTombstone { requested_at: 3, remote_deleted: false });
        Ok(())
    }).unwrap();
    crate::history::catalog_mutation::confirm_deleted(&store, &a.key()).unwrap();
    assert_eq!(revision.get(), 1);
    store.update_entry(&a.key(), |row| { row.as_mut().unwrap().source = ConversationSource::Workbench; Ok(()) }).unwrap();
    assert_eq!(revision.get(), 2);
    let mut legacy = a;
    legacy.identity = CatalogIdentity::Legacy { history_id: "legacy".into() };
    insert(&store, legacy.clone());
    store.update_entry(&legacy.key(), |row| { *row = None; Ok(()) }).unwrap();
    assert_eq!(revision.get(), 2);
}

#[test]
fn simultaneous_single_and_bulk_updates_preserve_independent_fields() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("catalog.sqlite");
    let store = CatalogStore::open(&path).unwrap();
    let a = entry("/project");
    insert(&store, a.clone());
    let barrier = Arc::new(Barrier::new(3));
    let handles: Vec<_> = (0..3).map(|index| {
        let path = path.clone(); let key = a.key(); let barrier = barrier.clone();
        std::thread::spawn(move || {
            let store = CatalogStore::open(&path).unwrap();
            barrier.wait();
            if index == 2 {
                store.update(|rows| { rows[0].updated = 42; rows.push(entry("/other")); Ok(()) }).unwrap();
            } else {
                store.update_entry(&key, |row| {
                    if index == 0 { row.as_mut().unwrap().pinned = true; }
                    else { row.as_mut().unwrap().archived = true; }
                    Ok(())
                }).unwrap();
            }
        })
    }).collect();
    for handle in handles { handle.join().unwrap(); }
    let result = store.get(&a.key()).unwrap();
    assert!(result.pinned && result.archived);
    assert_eq!(result.updated, 42);
    assert_eq!(store.all().unwrap().len(), 2);
}

#[test]
fn preference_write_cannot_leave_an_orphan_when_entry_removal_overlaps() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("catalog.sqlite");
    let store = CatalogStore::open(&path).unwrap();
    let a = entry("/project");
    insert(&store, a.clone());
    store.set_model_selection(&a.key(), &ConversationModelSelection::Inherit).unwrap();
    let (held, acquired) = std::sync::mpsc::channel();
    let (release, released) = std::sync::mpsc::channel();
    let delete_path = path.clone(); let delete_key = a.key();
    let deletion = std::thread::spawn(move || CatalogStore::open(&delete_path).unwrap().update_entry(&delete_key, |row| {
        *row = None;
        held.send(()).unwrap();
        released.recv().unwrap();
        Ok(())
    }));
    acquired.recv().unwrap();
    let write_path = path.clone(); let write_key = a.key();
    let writer = std::thread::spawn(move || CatalogStore::open(&write_path).unwrap().set_model_selection(&write_key, &ConversationModelSelection::Inherit));
    release.send(()).unwrap();
    deletion.join().unwrap().unwrap();
    assert_eq!(writer.join().unwrap().unwrap_err(), "history_not_found");
    assert_eq!(Connection::open(&path).unwrap().query_row("SELECT COUNT(*) FROM model_selections", [], |row| row.get::<_, i64>(0)).unwrap(), 0);
}

#[test]
#[ignore = "manual synthetic 1k/10k/100k H18b operation-count measurement"]
fn single_entry_scaling_benchmark() {
    for size in [1_000, 10_000, 100_000] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("catalog.sqlite");
        let store = CatalogStore::open(&path).unwrap();
        let mut connection = Connection::open(&path).unwrap();
        let tx = connection.transaction().unwrap();
        {
            let mut insert = tx.prepare("INSERT INTO entries VALUES (?1, ?2)").unwrap();
            for index in 0..size {
                let row = entry(&format!("/project-{index}"));
                insert.execute((row.key(), serde_json::to_string(&row).unwrap())).unwrap();
            }
        }
        tx.commit().unwrap();
        connection.execute_batch("CREATE TABLE writes(n INTEGER); INSERT INTO writes VALUES(0); CREATE TRIGGER count_updates AFTER UPDATE ON entries BEGIN UPDATE writes SET n=n+1; END;").unwrap();
        let key = entry(&format!("/project-{}", size / 2)).key();
        let plan: String = connection.query_row("EXPLAIN QUERY PLAN SELECT metadata FROM entries WHERE key=?1", [&key], |row| row.get(3)).unwrap();
        assert!(plan.contains("SEARCH") && plan.contains("INDEX"), "{plan}");
        DECODED_ROWS.with(|count| count.set(0));
        let started = std::time::Instant::now();
        for index in 0..100 {
            store.update_entry(&key, |row| { row.as_mut().unwrap().pinned = index % 2 == 0; Ok(()) }).unwrap();
        }
        let elapsed = started.elapsed();
        let decoded = DECODED_ROWS.with(|count| count.get());
        let writes: i64 = connection.query_row("SELECT n FROM writes", [], |row| row.get(0)).unwrap();
        assert_eq!(decoded, 100);
        assert_eq!(writes, 100);
        println!("H18b rows={size} mutations=100 decoded={decoded} writes={writes} elapsed={elapsed:?} plan={plan}");
    }
}
