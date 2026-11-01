use super::*;
use crate::memory::{domain::*, repository::{MemoryRepository, SystemClock}};
use rusqlite::params;

pub(super) const INSERT: &str = "INSERT INTO memories(id,scope,type,memory_key,content,status,confidence,importance,sensitivity,source_kind,valid_from,created_at,updated_at,revision)
    VALUES(?1,'global','preference',?1,?2,?3,1,3,'normal','explicit','old','old','unchanged',1)";

pub(super) fn seed(connection: &Connection, count: usize) {
    connection.execute_batch("BEGIN IMMEDIATE").unwrap();
    {
        let mut insert = connection.prepare(INSERT).unwrap();
        for i in 0..count {
            insert.execute(params![format!("memory-{i:06}"), format!("synthetic 不喜欢甜食 {i}"), "active"]).unwrap();
        }
    }
    connection.execute_batch("COMMIT").unwrap();
}

pub(super) fn rows(connection: &Connection, sql: &str) -> Vec<Vec<rusqlite::types::Value>> {
    let mut statement = connection.prepare(sql).unwrap();
    let columns = statement.column_count();
    statement.query_map([], |row| (0..columns).map(|i| row.get(i)).collect()).unwrap()
        .collect::<Result<_, _>>().unwrap()
}

pub(super) fn mapping(connection: &Connection) -> Vec<Vec<rusqlite::types::Value>> {
    rows(connection, "SELECT search_id,memory_id FROM memory_search_ids ORDER BY memory_id")
}

fn shadows(connection: &Connection) -> Vec<Vec<Vec<rusqlite::types::Value>>> {
    ["memory_search_data", "memory_search_idx", "memory_search_content", "memory_search_docsize", "memory_search_config"]
        .iter().map(|table| rows(connection, &format!("SELECT * FROM {table} ORDER BY 1"))).collect()
}

pub(super) fn consistent(connection: &Connection) {
    assert_eq!(rows(connection, "SELECT rowid,memory_id,content,memory_key FROM memory_search ORDER BY memory_id"),
        rows(connection, "SELECT s.search_id,m.id,m.content,IFNULL(m.memory_key,'') FROM memories m JOIN memory_search_ids s ON s.memory_id=m.id WHERE m.status='active' ORDER BY m.id"));
    assert_eq!(rows(connection,"SELECT id FROM memories ORDER BY id"), rows(connection,"SELECT memory_id FROM memory_search_ids ORDER BY memory_id"));
    assert!(rows(connection,"PRAGMA foreign_key_check").is_empty());
    connection.execute("INSERT INTO memory_search(memory_search) VALUES('integrity-check')", []).unwrap();
}

#[test]
fn metadata_and_noop_edits_leave_fts_shadows_and_identity_untouched() {
    let store = MemoryStore::open_in_memory().unwrap();
    store.with_connection(|db| {
        seed(db, 3);
        let before = shadows(db);
        let ids = mapping(db);
        let changes = db.total_changes();
        db.execute_batch("UPDATE memories SET importance=5,confidence=0.8,updated_at='edited',revision=revision+1,expires_at='2027-01-01T00:00:00Z',sensitivity='sensitive' WHERE id='memory-000000';
            UPDATE memories SET content=content,memory_key=memory_key,status=status WHERE id='memory-000001';")?;
        assert_eq!(db.total_changes()-changes, 2, "only base rows should be written");
        assert_eq!(shadows(db), before);
        assert_eq!(mapping(db), ids);
        consistent(db);
        Ok(())
    }).unwrap();
}

#[test]
fn content_keys_and_status_transitions_keep_exact_search_identity() {
    let store = MemoryStore::open_in_memory().unwrap();
    store.with_connection(|db| {
        seed(db, 3);
        let ids = mapping(db);
        for sql in [
            "UPDATE memories SET content='replacement 改喝清茶',memory_key=NULL WHERE id='memory-000000'",
            "UPDATE memories SET memory_key='new.key' WHERE id='memory-000000'",
            "UPDATE memories SET status='expired' WHERE id='memory-000000'",
            "UPDATE memories SET content='inactive edit',status='superseded' WHERE id='memory-000000'",
            "UPDATE memories SET status='active' WHERE id='memory-000000'",
        ] {
            db.execute_batch(sql)?;
            consistent(db);
            assert_eq!(mapping(db),ids);
        }
        assert_eq!(db.query_row::<i64,_,_>("SELECT count(*) FROM memory_search WHERE memory_search MATCH '\"inactive\"'",[],|r|r.get(0))?,1);
        // UUID is normally immutable; a legal rename still uses the same integer identity.
        let old_id: i64 = db.query_row("SELECT search_id FROM memory_search_ids WHERE memory_id='memory-000000'",[],|r|r.get(0))?;
        db.execute_batch("UPDATE memories SET id='renamed' WHERE id='memory-000000'")?;
        consistent(db);
        assert_eq!(db.query_row::<i64,_,_>("SELECT rowid FROM memory_search WHERE memory_id='renamed'",[],|r|r.get(0))?,old_id);
        Ok(())
    }).unwrap();
}

#[test]
fn conflicts_replacement_and_rollback_never_leave_orphan_search_rows() {
    let store = MemoryStore::open_in_memory().unwrap();
    store.with_connection(|db| { seed(db,3); Ok(()) }).unwrap();
    let before = store.with_connection(|db| Ok((mapping(db), shadows(db)))).unwrap();
    assert!(store.with_transaction(|tx| {
        tx.execute_batch("UPDATE memories SET content='rolled back content' WHERE id='memory-000000'; DELETE FROM memories WHERE id='memory-000001';")?;
        tx.execute(INSERT,params!["new-row","rolled back insert","active"])?;
        Err::<(),_>(MemoryError::validation_failed("synthetic late rollback"))
    }).is_err());
    store.with_connection(|db| {
        assert_eq!((mapping(db), shadows(db)),before);
        assert!(db.execute(INSERT,params!["memory-000000","duplicate","active"]).is_err());
        assert!(db.execute_batch("UPDATE memories SET memory_key='memory-000000' WHERE id='memory-000001'").is_err());
        assert_eq!((mapping(db), shadows(db)),before);
        // FK cascade, rather than a parent DELETE trigger, also cleans REPLACE
        // with SQLite's default recursive_triggers=OFF.
        db.pragma_update(None,"recursive_triggers",false)?;
        let old: i64 = db.query_row("SELECT search_id FROM memory_search_ids WHERE memory_id='memory-000000'",[],|r|r.get(0))?;
        db.execute(&INSERT.replacen("INSERT INTO","INSERT OR REPLACE INTO",1),params!["memory-000000","replacement","active"])?;
        consistent(db);
        let new: i64 = db.query_row("SELECT search_id FROM memory_search_ids WHERE memory_id='memory-000000'",[],|r|r.get(0))?;
        assert!(new>old);
        assert_eq!(db.query_row::<i64,_,_>("SELECT count(*) FROM memory_search WHERE rowid=?1",[old],|r|r.get(0))?,0);
        Ok(())
    }).unwrap();
}

#[test]
fn vacuum_reopen_and_reactivation_preserve_mapping_without_id_reuse() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join(DB_FILE_NAME);
    let store = MemoryStore::open(&path).unwrap();
    store.with_connection(|db| {
        seed(db,12);
        db.execute_batch("UPDATE memories SET supersedes_id='memory-000005' WHERE id='memory-000006';
            DELETE FROM memories WHERE id IN ('memory-000000','memory-000005','memory-000011'); UPDATE memories SET status='expired' WHERE id='memory-000009';")?;
        assert_eq!(db.query_row::<Option<String>,_,_>("SELECT supersedes_id FROM memories WHERE id='memory-000006'",[],|r|r.get(0))?,None);
        Ok(())
    }).unwrap();
    let before = store.with_connection(|db| Ok(mapping(db))).unwrap();
    store.compact().unwrap();
    drop(store);
    let store = MemoryStore::open(&path).unwrap();
    store.with_connection(|db| {
        assert_eq!(mapping(db),before);
        db.execute_batch("UPDATE memories SET status='active' WHERE id='memory-000009'")?;
        assert_eq!(mapping(db),before);
        db.execute(INSERT,params!["after-vacuum","fresh row","active"])?;
        assert!(db.query_row::<i64,_,_>("SELECT search_id FROM memory_search_ids WHERE memory_id='after-vacuum'",[],|r|r.get(0))?>12);
        consistent(db);
        Ok(())
    }).unwrap();
}

fn request(content: &str, persona: Option<&str>) -> NewMemory {
    NewMemory { scope: if persona.is_some() {MemoryScope::Persona} else {MemoryScope::Global},
        persona_id: persona.map(str::to_owned), memory_type: MemoryType::Preference, memory_key: None,
        content: content.into(), importance: None, expires_at: None, source_kind: SourceKind::Explicit,
        conversation_id: Some("synthetic-conversation".into()), message_id: Some("synthetic-message".into()), sensitive_confirmed: true }
}

fn contains(haystack: &[u8], needle: &str) -> bool { haystack.windows(needle.len()).any(|bytes|bytes==needle.as_bytes()) }

#[test]
fn forget_and_clear_erase_owned_fts_content_postings_sources_and_file_bytes() {
    const SENTINEL: &str = "M19C_ERASE_SENTINEL_龘靐齉";
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join(DB_FILE_NAME);
    let repository = MemoryRepository::new(MemoryStore::open(&path).unwrap(),SystemClock);
    let victim = repository.create(&request(SENTINEL,None)).unwrap();
    repository.create(&request("surviving unrelated memory",None)).unwrap();
    repository.store().checkpoint().unwrap();
    assert!(contains(&std::fs::read(&path).unwrap(),SENTINEL),"positive file-content control");
    repository.forget(&victim.id).unwrap();
    repository.store().with_connection(|db| {
        let plan:Vec<String>=db.prepare("EXPLAIN QUERY PLAN DELETE FROM memories WHERE id=?1")?
            .query_map([&victim.id],|r|r.get(3))?.collect::<Result<_,_>>()?;
        println!("M19c full_delete_plan={plan:?}");
        assert_eq!(db.query_row::<i64,_,_>("SELECT v FROM memory_search_config WHERE k='secure-delete'",[],|r|r.get(0))?,1);
        assert_eq!(db.query_row::<i64,_,_>("PRAGMA secure_delete",[],|r|r.get(0))?,1);
        for table in ["memory_search", "memory_search_ids", "memory_sources", "memory_task_links", "memory_applicability", "memory_work_links"] {
            assert_eq!(db.query_row::<i64,_,_>(&format!("SELECT count(*) FROM {table} WHERE memory_id=?1"),[&victim.id],|r|r.get(0))?,0,"{table}");
        }
        assert_eq!(db.query_row::<i64,_,_>("SELECT count(*) FROM memory_search WHERE memory_search MATCH '\"龘靐齉\"'",[],|r|r.get(0))?,0);
        consistent(db);
        Ok(())
    }).unwrap();
    for file in [&path, &path.with_extension("db-wal")] {
        if let Ok(bytes) = std::fs::read(file) {
            assert!(!contains(&bytes,SENTINEL),"{}",file.display());
            assert!(!contains(&bytes,"龘靐齉"),"FTS token remains in {}",file.display());
        }
    }
    for _ in 0..3 { repository.create(&request(SENTINEL,Some("erase-persona"))).unwrap(); }
    repository.clear(Some(MemoryScope::Persona),Some("erase-persona")).unwrap();
    assert_eq!(repository.list(&MemoryQuery::default()).unwrap().len(),1);
    assert!(!contains(&std::fs::read(&path).unwrap(),SENTINEL));
    assert!(!contains(&std::fs::read(&path).unwrap(),"龘靐齉"));
    repository.store().with_connection(|db| { consistent(db); Ok(()) }).unwrap();
}
