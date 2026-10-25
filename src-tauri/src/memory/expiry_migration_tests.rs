use super::*;
use sha2::{Digest, Sha256};

fn old_store(path: &Path, rows: usize) -> Connection {
    let mut connection = Connection::open(path).unwrap();
    for step in &MIGRATIONS[..2] { connection.execute_batch(step.sql).unwrap(); }
    connection.pragma_update(None, "user_version", 2).unwrap();
    let tx = connection.transaction().unwrap();
    {
        let mut insert = tx.prepare("INSERT INTO memories(id,scope,type,content,status,confidence,importance,sensitivity,source_kind,valid_from,expires_at,created_at,updated_at,revision)
            VALUES(?1,'global','preference','synthetic migration evidence','active',1,3,'normal','explicit','2026-01-01T00:00:00Z','2026-12-01T00:00:00Z','created','unchanged',1)").unwrap();
        for index in 0..rows { insert.execute([format!("memory-{index}")]).unwrap(); }
    }
    tx.execute_batch("INSERT INTO memory_events VALUES('e','memory-0','created','old'); INSERT INTO memory_sources VALUES('source','memory-0','session','message','explicit','old'); INSERT INTO memory_applicability VALUES('memory-0','/fixture','topic','active','aliases',1); INSERT INTO memory_task_links VALUES('memory-0','task','old'); INSERT INTO memory_work_links VALUES('memory-0','entry',2);").unwrap();
    tx.commit().unwrap();
    connection
}

fn snapshot(connection: &Connection) -> Vec<Vec<Vec<rusqlite::types::Value>>> {
    // Include FTS rowids and provenance: adding an index must not rewrite any data.
    ["memories", "memory_sources", "memory_applicability", "memory_task_links", "memory_work_links", "memory_events", "memory_search", "memory_automation_state"].iter().map(|table| {
        let mut statement = connection.prepare(&format!("SELECT rowid,* FROM {table} ORDER BY rowid")).unwrap();
        let columns = statement.column_count();
        statement.query_map([], |row| (0..columns).map(|index| row.get(index)).collect()).unwrap().collect::<Result<_, _>>().unwrap()
    }).collect()
}

fn index_count(connection: &Connection) -> i64 {
    connection.query_row("SELECT count(*) FROM sqlite_master WHERE type='index' AND name='idx_memories_active_expiry'", [], |row| row.get(0)).unwrap()
}

#[test]
fn expiry_index_upgrade_preserves_data_and_old_readonly_copy_across_reopen() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join(DB_FILE_NAME);
    let old = old_store(&path, 500);
    let before = snapshot(&old);
    drop(old);
    // No live connections or WAL: the old copy is complete and kept separate.
    let old_path = root.path().join("schema-2-readonly.db");
    std::fs::copy(&path, &old_path).unwrap();
    let hash = Sha256::digest(std::fs::read(&old_path).unwrap());
    for _ in 0..2 {
        let store = MemoryStore::open(&path).unwrap();
        assert_eq!(store.schema_version().unwrap(), 3);
        store.with_connection(|connection| {
            assert_eq!(snapshot(connection), before);
            assert_eq!(index_count(connection), 1);
            assert_eq!(connection.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0)).unwrap(), "ok");
            Ok(())
        }).unwrap();
    }
    let old = Connection::open_with_flags(&old_path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    assert_eq!(read_schema_version(&old).unwrap(), 2);
    assert_eq!(snapshot(&old), before);
    assert_eq!(index_count(&old), 0);
    assert_eq!(Sha256::digest(std::fs::read(&old_path).unwrap()), hash);
}

#[test]
fn expiry_index_late_failure_rolls_back_index_and_version_then_can_retry() {
    let root = tempfile::tempdir().unwrap();
    let connection = old_store(&root.path().join(DB_FILE_NAME), 100);
    configure(&connection).unwrap();
    let before = snapshot(&connection);
    let store = MemoryStore { connection: Mutex::new(connection) };
    let broken = [
        Migration { to_version: 3, sql: MIGRATIONS[2].sql },
        Migration { to_version: 4, sql: "SELECT * FROM injected_missing_table;" },
    ];
    assert!(store.migrate_steps(&broken).is_err());
    store.with_connection(|connection| {
        assert_eq!(read_schema_version(connection).unwrap(), 2);
        assert_eq!(index_count(connection), 0);
        assert_eq!(snapshot(connection), before);
        Ok(())
    }).unwrap();
    store.migrate().unwrap();
    assert_eq!(store.schema_version().unwrap(), 3);
}

#[test]
fn expiry_index_full_database_keeps_schema_two_and_all_committed_rows() {
    let root = tempfile::tempdir().unwrap();
    let connection = old_store(&root.path().join(DB_FILE_NAME), 500);
    configure(&connection).unwrap();
    let before = snapshot(&connection);
    let pages: i64 = connection.query_row("PRAGMA page_count", [], |row| row.get(0)).unwrap();
    connection.pragma_update(None, "max_page_count", pages).unwrap();
    let store = MemoryStore { connection: Mutex::new(connection) };
    let error = store.migrate().unwrap_err();
    assert_eq!(error.code(), crate::memory::error::MemoryErrorCode::MigrationFailed);
    assert!(error.message().contains("full"), "{error}");
    store.with_connection(|connection| {
        assert_eq!(read_schema_version(connection).unwrap(), 2);
        assert_eq!(index_count(connection), 0);
        assert_eq!(snapshot(connection), before);
        connection.pragma_update(None, "max_page_count", 1_000_000)?;
        Ok(())
    }).unwrap();
    store.migrate().unwrap();
    assert_eq!(store.schema_version().unwrap(), 3);
}

#[test]
fn expiry_index_open_failure_preserves_uncheckpointed_v2_wal_and_parallel_retry() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join(DB_FILE_NAME);
    let old = old_store(&path, 100);
    old.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; INSERT INTO memory_events VALUES('wal','memory-0','edited','latest'); CREATE TABLE idx_memories_active_expiry(conflict TEXT);").unwrap();
    let before = snapshot(&old);
    assert!(path.with_extension("db-wal").exists());
    assert!(MemoryStore::open(&path).is_err());
    assert_eq!(read_schema_version(&old).unwrap(), 2);
    assert_eq!(snapshot(&old), before);
    old.execute_batch("DROP TABLE idx_memories_active_expiry;").unwrap();
    drop(old);
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
    let workers: Vec<_> = (0..4).map(|_| {
        let path = path.clone(); let barrier = barrier.clone();
        std::thread::spawn(move || {
            barrier.wait();
            let store = MemoryStore::open(&path).unwrap();
            assert_eq!(store.schema_version().unwrap(), 3);
            store.with_connection(|connection| { assert_eq!(index_count(connection), 1); Ok(snapshot(connection)) }).unwrap()
        })
    }).collect();
    for worker in workers { assert_eq!(worker.join().unwrap(), before); }
}
