use super::*;

#[test]
#[ignore = "synthetic disk benchmark; run explicitly with --ignored --nocapture"]
fn database_scale_foreign_keys() {
    use crate::database_measurements::sample;
    for count in [10_000,100_000] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(DB_FILE_NAME);
        let mut db = version_four(&path);
        configure(&db).unwrap();
        {
            let tx = db.transaction().unwrap();
            {
                let mut memories = tx.prepare("INSERT INTO memories(id,scope,type,content,status,confidence,importance,sensitivity,source_kind,valid_from,created_at,updated_at,revision) VALUES(?1,'global','preference','preserved historical memory','superseded',1,3,'normal','explicit','2026-01-01','2026-01-01','2026-01-01',1)").unwrap();
                let mut candidates = tx.prepare("INSERT INTO memory_candidates(id,scope,type,content,confidence,importance,decision,created_at,expires_at) VALUES(?1,'global','preference','preserved candidate',1,3,'pending','2026-01-01','2027-01-01')").unwrap();
                for id in 0..count { memories.execute([format!("memory-{id}")]).unwrap(); candidates.execute([format!("candidate-{id}")]).unwrap(); }
            }
            tx.commit().unwrap();
        }
        let measure = |db:&Connection| sample(|| {
            db.execute_batch("BEGIN IMMEDIATE; DELETE FROM memories WHERE id='parent'; ROLLBACK;").unwrap();
        });
        let before = measure(&db);
        let plan = |db:&Connection| -> Vec<String> { ["SELECT rowid FROM memories WHERE supersedes_id='parent'","SELECT rowid FROM memory_candidates WHERE accepted_memory_id='parent'"].iter().map(|sql|db.query_row(&format!("EXPLAIN QUERY PLAN {sql}"),[],|r|r.get(3)).unwrap()).collect() };
        let old_plan = plan(&db);
        drop(db);
        let start = std::time::Instant::now();
        let store = MemoryStore::open(&path).unwrap();
        let migration_ms = start.elapsed().as_secs_f64()*1000.0;
        store.with_connection(|db| {
            let after = measure(db);
            assert_eq!(db.query_row("PRAGMA foreign_keys",[],|r|r.get::<_,i64>(0))?,1);
            assert_eq!(db.query_row("SELECT COUNT(*) FROM memories",[],|r|r.get::<_,i64>(0))?,count+2);
            assert_eq!(db.query_row("SELECT supersedes_id FROM memories WHERE id='child'",[],|r|r.get::<_,String>(0))?,"parent");
            println!("DATABASE_MEASUREMENT {}",serde_json::json!({"finding":"F12","rows_per_child_table":count,"old":before,"new":after,"migration_ms":migration_ms,"old_plan":old_plan,"new_plan":plan(db),"method":"DELETE with real FK/FTS/secure-delete, ROLLBACK each sample; excludes commit fsync"}));
            Ok(())
        }).unwrap();
    }
}

#[test]
fn foreign_key_children_use_indexes_instead_of_scanning() {
    let store = MemoryStore::open_in_memory().unwrap();
    store.with_connection(|db| {
        for (table, column, index) in [
            ("memories", "supersedes_id", "idx_memories_supersedes"),
            ("memory_candidates", "accepted_memory_id", "idx_candidates_accepted_memory"),
        ] {
            let plan: String = db.query_row(&format!("EXPLAIN QUERY PLAN SELECT rowid FROM {table} WHERE {column}=?1"), ["missing"], |r| r.get(3))?;
            assert!(plan.contains(index), "{plan}");
            assert!(!plan.contains("SCAN"), "{plan}");
        }
        Ok(())
    }).unwrap();
}

fn version_four(path: &Path) -> Connection {
    let db = Connection::open(path).unwrap();
    db.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    for step in MIGRATIONS.iter().filter(|step|step.to_version<=4) { db.execute_batch(step.sql).unwrap(); }
    db.execute_batch("PRAGMA user_version=4;
        INSERT INTO memories(id,scope,type,content,status,confidence,importance,sensitivity,source_kind,valid_from,created_at,updated_at,revision)
        VALUES('parent','global','preference','erase sentinel text','active',1,3,'normal','explicit','2026-01-01','2026-01-01','2026-01-01',1);
        INSERT INTO memories(id,scope,type,content,status,confidence,importance,sensitivity,source_kind,valid_from,created_at,updated_at,revision,supersedes_id)
        VALUES('child','global','preference','preserved child text','active',1,3,'normal','explicit','2026-01-02','2026-01-02','2026-01-02',1,'parent');
        INSERT INTO memory_candidates(id,scope,type,content,confidence,importance,decision,created_at,expires_at,accepted_memory_id)
        VALUES('candidate','global','preference','preserved candidate',1,3,'accepted','2026-01-01','2027-01-01','parent');
        INSERT INTO memory_sources(id,memory_id,source_kind,created_at) VALUES('source','parent','explicit','2026-01-01');").unwrap();
    db
}

#[test]
fn v4_upgrade_is_repeatable_and_preserves_rows_fts_and_delete_contracts() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(DB_FILE_NAME);
    drop(version_four(&path));
    for _ in 0..2 {
        let store = MemoryStore::open(&path).unwrap();
        assert_eq!(store.schema_version().unwrap(),5);
        store.with_connection(|db| {
            assert_eq!(db.query_row("SELECT content FROM memories WHERE id='parent'",[],|r|r.get::<_,String>(0))?,"erase sentinel text");
            assert_eq!(db.query_row("SELECT accepted_memory_id FROM memory_candidates WHERE id='candidate'",[],|r|r.get::<_,String>(0))?,"parent");
            assert_eq!(db.query_row("SELECT COUNT(*) FROM memory_search WHERE memory_search MATCH 'sentinel'",[],|r|r.get::<_,i64>(0))?,1);
            assert_eq!(db.query_row("PRAGMA secure_delete",[],|r|r.get::<_,i64>(0))?,1);
            assert_eq!(db.query_row("SELECT v FROM memory_search_config WHERE k='secure-delete'",[],|r|r.get::<_,i64>(0))?,1);
            Ok(())
        }).unwrap();
    }
    let store = MemoryStore::open(&path).unwrap();
    store.with_transaction(|db| { db.execute("DELETE FROM memories WHERE id='parent'",[])?; Ok(()) }).unwrap();
    store.with_connection(|db| {
        assert!(db.query_row("SELECT supersedes_id FROM memories WHERE id='child'",[],|r|r.get::<_,Option<String>>(0))?.is_none());
        assert!(db.query_row("SELECT accepted_memory_id FROM memory_candidates WHERE id='candidate'",[],|r|r.get::<_,Option<String>>(0))?.is_none());
        assert_eq!(db.query_row("SELECT COUNT(*) FROM memory_sources",[],|r|r.get::<_,i64>(0))?,0);
        assert_eq!(db.query_row("SELECT COUNT(*) FROM memory_search WHERE memory_search MATCH 'sentinel'",[],|r|r.get::<_,i64>(0))?,0);
        assert_eq!(db.query_row("SELECT COUNT(*) FROM memory_search WHERE memory_search MATCH 'preserved'",[],|r|r.get::<_,i64>(0))?,1);
        assert_eq!(db.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check",[],|r|r.get::<_,i64>(0))?,0);
        Ok(())
    }).unwrap();
    store.checkpoint().unwrap();
}

#[test]
fn failed_second_index_rolls_back_first_index_and_preserves_v4_data() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(DB_FILE_NAME);
    let db = version_four(&path);
    db.execute_batch("CREATE TABLE idx_candidates_accepted_memory(sentinel TEXT);").unwrap();
    assert!(MemoryStore::open(&path).is_err());
    assert_eq!(read_schema_version(&db).unwrap(),4);
    assert_eq!(db.query_row("SELECT COUNT(*) FROM sqlite_master WHERE name='idx_memories_supersedes'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    assert_eq!(db.query_row("SELECT content FROM memories WHERE id='parent'",[],|r|r.get::<_,String>(0)).unwrap(),"erase sentinel text");
    db.execute_batch("DROP TABLE idx_candidates_accepted_memory;").unwrap();
    drop(db);
    assert_eq!(MemoryStore::open(&path).unwrap().schema_version().unwrap(),5);
}
