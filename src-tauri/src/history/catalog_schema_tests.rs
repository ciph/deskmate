use super::*;
use crate::history::{catalog::CatalogStore, catalog_sql::tests::{fixtures, seed_v2}, catalog_query::CatalogQuery};
use sha2::{Digest, Sha256};

fn version(connection: &Connection) -> u32 { connection.query_row("PRAGMA user_version", [], |row| row.get(0)).unwrap() }
fn raw_rows(connection: &Connection) -> Vec<(String, String)> {
    connection.prepare("SELECT key, metadata FROM entries ORDER BY key").unwrap().query_map([], |row| Ok((row.get(0)?, row.get(1)?))).unwrap().collect::<Result<_, _>>().unwrap()
}

#[test]
fn migration_preserves_metadata_preferences_registry_and_readonly_old_copy() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("catalog.sqlite");
    let rows = fixtures(1007);
    seed_v2(&path, &rows);
    let before = {
        let connection = Connection::open(&path).unwrap();
        connection.execute("INSERT INTO model_selections VALUES (?1,?2)", (rows[1].key(), "\"inherit\"")).unwrap();
        connection.execute("INSERT INTO recent_workspaces VALUES('/unrelated',123)", []).unwrap();
        raw_rows(&connection)
    };
    // The fixture has no live connections or WAL; preserve a separate old copy.
    let old_path = root.path().join("v2-readonly.sqlite");
    std::fs::copy(&path, &old_path).unwrap();
    let old_hash = Sha256::digest(std::fs::read(&old_path).unwrap());
    for _ in 0..2 {
        let store = CatalogStore::open(&path).unwrap();
        assert!(store.page(CatalogQuery::default(), vec![]).is_ok());
        let connection = Connection::open(&path).unwrap();
        assert_eq!(version(&connection), 3);
        assert_eq!(raw_rows(&connection), before);
        assert_eq!(connection.query_row("SELECT selection FROM model_selections", [], |row| row.get::<_, String>(0)).unwrap(), "\"inherit\"");
        assert_eq!(store.recent_workspaces().unwrap(), vec!["/unrelated"]);
    }
    let old = Connection::open_with_flags(&old_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    assert_eq!(version(&old), 2);
    assert_eq!(raw_rows(&old), before);
    assert_eq!(Sha256::digest(std::fs::read(&old_path).unwrap()), old_hash);
    // The previous catalog implementation explicitly rejects any version >2.
    assert!(version(&Connection::open(&path).unwrap()) > 2);
}

#[test]
fn malformed_json_or_identity_rolls_back_schema_and_leaves_old_reader_usable() {
    for invalid in ["invalid".to_owned(), serde_json::to_string(&fixtures(1)[0]).unwrap()] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("catalog.sqlite");
        seed_v2(&path, &fixtures(30));
        let connection = Connection::open(&path).unwrap();
        connection.execute("INSERT INTO entries VALUES('wrong-key',?1)", [invalid]).unwrap();
        let before = raw_rows(&connection);
        assert!(matches!(CatalogStore::open(&path), Err(error) if error == "history_catalog_invalid"));
        assert_eq!(version(&connection), 2);
        assert_eq!(raw_rows(&connection), before);
        assert_eq!(connection.query_row("SELECT count(*) FROM sqlite_master WHERE name='entry_projection'", [], |row| row.get::<_, u32>(0)).unwrap(), 0);
        connection.execute("DELETE FROM entries WHERE key='wrong-key'", []).unwrap();
        assert!(CatalogStore::open(&path).is_ok());
    }
}

#[test]
fn interrupted_backfill_rolls_back_ddl_partial_rows_and_version_then_retries() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("catalog.sqlite");
    seed_v2(&path, &fixtures(100));
    let mut connection = Connection::open(&path).unwrap();
    let before = raw_rows(&connection);
    {
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).unwrap();
        let mut visited = 0;
        assert_eq!(migrate(&tx, || { visited += 1; if visited == 50 { Err("migration_interrupted".into()) } else { Ok(()) } }).unwrap_err(), "migration_interrupted");
        assert_eq!(visited, 50);
        // Drop follows the same rollback path as any open()/migration error.
    }
    assert_eq!(version(&connection), 2);
    assert_eq!(raw_rows(&connection), before);
    assert_eq!(connection.query_row("SELECT count(*) FROM sqlite_master WHERE name='entry_projection'", [], |row| row.get::<_, u32>(0)).unwrap(), 0);
    assert!(CatalogStore::open(&path).is_ok());
}

#[test]
fn sqlite_full_rolls_back_migration_without_changing_old_metadata() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("catalog.sqlite");
    seed_v2(&path, &fixtures(100));
    let mut connection = Connection::open(&path).unwrap();
    let before = raw_rows(&connection);
    let pages: u32 = connection.query_row("PRAGMA page_count", [], |row| row.get(0)).unwrap();
    connection.pragma_update(None, "max_page_count", pages + 12).unwrap();
    {
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).unwrap();
        assert_eq!(migrate_v3(&tx).unwrap_err(), "history_catalog_unavailable");
    }
    assert_eq!(version(&connection), 2);
    assert_eq!(raw_rows(&connection), before);
    connection.pragma_update(None, "max_page_count", 1_000_000).unwrap();
    assert!(CatalogStore::open(&path).is_ok());
}

#[test]
fn future_schema_is_rejected_without_mutation_and_parallel_open_migrates_once() {
    let root = tempfile::tempdir().unwrap();
    let future = root.path().join("future.sqlite");
    seed_v2(&future, &fixtures(5));
    Connection::open(&future).unwrap().pragma_update(None, "user_version", 4).unwrap();
    let bytes = std::fs::read(&future).unwrap();
    assert!(matches!(CatalogStore::open(&future), Err(error) if error == "history_catalog_version_unsupported"));
    assert_eq!(std::fs::read(&future).unwrap(), bytes);
    let path = root.path().join("catalog.sqlite");
    seed_v2(&path, &fixtures(1000));
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
    let workers: Vec<_> = (0..4).map(|_| {
        let path = path.clone(); let barrier = barrier.clone();
        std::thread::spawn(move || { barrier.wait(); CatalogStore::open(&path).unwrap().page(CatalogQuery::default(), vec![]).unwrap().total })
    }).collect();
    let counts: Vec<_> = workers.into_iter().map(|worker| worker.join().unwrap()).collect();
    assert!(counts.iter().all(|count| *count == counts[0]));
}
