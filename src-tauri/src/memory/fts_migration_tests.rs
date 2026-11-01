use super::*;
use super::fts_tests::{consistent, mapping, rows, seed};
use sha2::{Digest,Sha256};
use crate::memory::{domain::{MemoryQuery,MemoryStatus}, repository::{MemoryRepository,SystemClock}};

fn public_views(repository: &MemoryRepository<SystemClock>) -> Vec<Vec<crate::memory::domain::MemoryRecord>> {
    [None,Some("synthetic"),Some("SYNTHETIC"),Some("不喜欢甜食"),Some("甜食"),Some("%_")].into_iter()
        .map(|search| repository.export_records(&MemoryQuery {
            statuses: Some(vec![MemoryStatus::Active,MemoryStatus::Expired,MemoryStatus::Superseded]),
            search:search.map(str::to_owned),..Default::default()
        }).unwrap()).collect()
}

fn old_store(path: &Path, count: usize) -> Connection {
    let connection = Connection::open(path).unwrap();
    for step in &MIGRATIONS[..3] { connection.execute_batch(step.sql).unwrap(); }
    connection.pragma_update(None,"user_version",3).unwrap();
    seed(&connection,count);
    connection.execute_batch("UPDATE memories SET status='expired' WHERE id='memory-000001';
        UPDATE memories SET status='superseded' WHERE id='memory-000002';
        UPDATE memories SET supersedes_id='memory-000002' WHERE id='memory-000004';
        INSERT INTO memory_sources VALUES('source','memory-000000','session','message','explicit','old');
        INSERT INTO memory_task_links VALUES('memory-000000','task','old');
        INSERT INTO memory_applicability VALUES('memory-000000','/fixture','topic','active','aliases',1);
        INSERT INTO memory_work_links VALUES('memory-000000','entry',2);
        INSERT INTO memory_events VALUES('event','memory-000000','created','old');").unwrap();
    connection
}

fn data(connection: &Connection) -> Vec<Vec<Vec<rusqlite::types::Value>>> {
    ["memories", "memory_sources", "memory_task_links", "memory_candidates", "memory_applicability", "memory_work_links",
     "memory_events", "relationship_states", "memory_jobs", "memory_consumed", "memory_suppressed", "memory_automation_state"]
        .iter().map(|table|rows(connection,&format!("SELECT * FROM {table} ORDER BY 1"))).collect()
}
fn fts(connection: &Connection) -> Vec<Vec<rusqlite::types::Value>> {
    rows(connection,"SELECT memory_id,content,memory_key FROM memory_search ORDER BY memory_id")
}
fn legacy_snapshot(connection: &Connection) -> Vec<Vec<Vec<rusqlite::types::Value>>> {
    let mut snapshot=data(connection);
    snapshot.push(rows(connection,"SELECT rowid,* FROM memory_search ORDER BY rowid"));
    snapshot.push(rows(connection,"SELECT type,name,sql FROM sqlite_master ORDER BY type,name"));
    snapshot
}

#[test]
fn fts_upgrade_preserves_records_provenance_search_and_readonly_original() {
    let root=tempfile::tempdir().unwrap();
    let path=root.path().join(DB_FILE_NAME);
    let old=old_store(&path,100);
    let before=data(&old);
    let searchable=fts(&old);
    let old_repository=MemoryRepository::new(MemoryStore {connection:Mutex::new(old)},SystemClock);
    let views=public_views(&old_repository);
    assert_eq!(views[1].len(),98);
    assert_eq!(views[4].len(),100);
    drop(old_repository);
    let backup=root.path().join("schema-3-readonly.db");
    std::fs::copy(&path,&backup).unwrap();
    let hash=Sha256::digest(std::fs::read(&backup).unwrap());
    let mut identities=None;
    for _ in 0..2 {
        let store=MemoryStore::open(&path).unwrap();
        assert_eq!(store.schema_version().unwrap(),4);
        store.with_connection(|db| {
            assert_eq!(data(db),before);
            assert_eq!(fts(db),searchable);
            if let Some(ids)=&identities { assert_eq!(&mapping(db),ids); }
            else { identities=Some(mapping(db)); }
            consistent(db);
            assert_eq!(db.query_row::<String,_,_>("PRAGMA integrity_check",[],|r|r.get(0))?,"ok");
            Ok(())
        }).unwrap();
        let repository=MemoryRepository::new(store,SystemClock);
        assert_eq!(public_views(&repository),views);
    }
    let old=Connection::open_with_flags(&backup,OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    assert_eq!(read_schema_version(&old).unwrap(),3);
    assert_eq!(data(&old),before);
    assert_eq!(fts(&old),searchable);
    assert_eq!(Sha256::digest(std::fs::read(&backup).unwrap()),hash);
}

#[test]
fn fts_migration_rebuilds_from_authoritative_rows_not_a_stale_old_index() {
    let root=tempfile::tempdir().unwrap();
    let path=root.path().join(DB_FILE_NAME);
    let old=old_store(&path,10);
    old.execute_batch("DELETE FROM memory_search WHERE memory_id='memory-000000';
        INSERT INTO memory_search(memory_id,content,memory_key) VALUES('orphan','obsolete index text','');").unwrap();
    let records=data(&old);
    drop(old);
    let store=MemoryStore::open(&path).unwrap();
    store.with_connection(|db| {
        assert_eq!(data(db),records);
        consistent(db);
        assert_eq!(db.query_row::<i64,_,_>("SELECT count(*) FROM memory_search WHERE memory_search MATCH '\"obsolete\"'",[],|r|r.get(0))?,0);
        Ok(())
    }).unwrap();
}

#[test]
fn late_fts_migration_failure_restores_old_triggers_rowids_schema_and_version() {
    let root=tempfile::tempdir().unwrap();
    let connection=old_store(&root.path().join(DB_FILE_NAME),100);
    configure(&connection).unwrap();
    let before=legacy_snapshot(&connection);
    let store=MemoryStore { connection:Mutex::new(connection) };
    let broken=[Migration {to_version:4,sql:MIGRATIONS[3].sql},Migration {to_version:5,sql:"SELECT * FROM injected_missing_table;"}];
    assert!(store.migrate_steps(&broken).is_err());
    store.with_connection(|db| {
        assert_eq!(read_schema_version(db).unwrap(),3);
        assert_eq!(legacy_snapshot(db),before);
        // The original trigger still works after the failed rebuild.
        db.execute_batch("BEGIN; UPDATE memories SET content='rollback search check' WHERE id='memory-000000'; ROLLBACK;")?;
        assert_eq!(legacy_snapshot(db),before);
        Ok(())
    }).unwrap();
    store.migrate().unwrap();
    assert_eq!(store.schema_version().unwrap(),4);
    store.with_connection(|db| { consistent(db); Ok(()) }).unwrap();
}

#[test]
fn fts_migration_disk_full_rolls_back_and_can_retry() {
    let root=tempfile::tempdir().unwrap();
    let connection=old_store(&root.path().join(DB_FILE_NAME),500);
    configure(&connection).unwrap();
    let before=legacy_snapshot(&connection);
    let pages:i64=connection.query_row("PRAGMA page_count",[],|r|r.get(0)).unwrap();
    connection.pragma_update(None,"max_page_count",pages).unwrap();
    let store=MemoryStore {connection:Mutex::new(connection)};
    let error=store.migrate().unwrap_err();
    assert!(error.message().contains("full"),"{error}");
    store.with_connection(|db| {
        assert_eq!(read_schema_version(db).unwrap(),3);
        assert_eq!(legacy_snapshot(db),before);
        db.pragma_update(None,"max_page_count",1_000_000)?;
        Ok(())
    }).unwrap();
    store.migrate().unwrap();
    store.with_connection(|db| { consistent(db); Ok(()) }).unwrap();
}

#[test]
fn fts_open_failure_preserves_wal_then_concurrent_openers_rebuild_once() {
    let root=tempfile::tempdir().unwrap();
    let path=root.path().join(DB_FILE_NAME);
    let old=old_store(&path,100);
    old.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;
        INSERT INTO memory_events VALUES('wal','memory-000000','edited','latest');
        CREATE TABLE memory_search_ids(conflict TEXT);").unwrap();
    let before=legacy_snapshot(&old);
    assert!(path.with_extension("db-wal").exists());
    assert!(MemoryStore::open(&path).is_err());
    assert_eq!(read_schema_version(&old).unwrap(),3);
    assert_eq!(legacy_snapshot(&old),before);
    old.execute_batch("DROP TABLE memory_search_ids").unwrap();
    let records=data(&old);
    let searchable=fts(&old);
    drop(old);
    let barrier=std::sync::Arc::new(std::sync::Barrier::new(4));
    let workers:Vec<_>=(0..4).map(|_| {
        let path=path.clone(); let barrier=barrier.clone();
        std::thread::spawn(move || {
            barrier.wait();
            let store=MemoryStore::open(&path).unwrap();
            assert_eq!(store.schema_version().unwrap(),4);
            store.with_connection(|db| { consistent(db); Ok((data(db),fts(db),mapping(db))) }).unwrap()
        })
    }).collect();
    let mut identities=None;
    for worker in workers {
        let (actual,search,ids)=worker.join().unwrap();
        assert_eq!(actual,records);
        assert_eq!(search,searchable);
        if let Some(expected)=&identities {assert_eq!(&ids,expected);} else {identities=Some(ids);}
    }
}
