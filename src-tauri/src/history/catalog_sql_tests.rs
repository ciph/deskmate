use super::*;
use crate::history::{catalog::DECODED_ROWS, catalog_import::new_entry, catalog_model::*, catalog_query::page as oracle};
use std::path::Path;

pub(crate) fn seed_v2(path: &Path, rows: &[CatalogEntry]) {
    let mut connection = Connection::open(path).unwrap();
    connection.execute_batch("CREATE TABLE entries(key TEXT PRIMARY KEY NOT NULL, metadata TEXT NOT NULL); CREATE TABLE directories(directory TEXT PRIMARY KEY NOT NULL); CREATE TABLE model_selections(key TEXT PRIMARY KEY NOT NULL, selection TEXT NOT NULL); CREATE TABLE recent_workspaces(directory TEXT PRIMARY KEY NOT NULL, selected_at INTEGER NOT NULL); PRAGMA user_version=2;").unwrap();
    let tx = connection.transaction().unwrap();
    {
        let mut statement = tx.prepare("INSERT INTO entries VALUES (?1,?2)").unwrap();
        for row in rows { statement.execute((row.key(), serde_json::to_string(row).unwrap())).unwrap(); }
    }
    tx.execute("INSERT INTO directories VALUES('/unrelated')", []).unwrap();
    tx.commit().unwrap();
}

pub(crate) fn fixtures(size: usize) -> Vec<CatalogEntry> {
    (0..size).map(|index| {
        let identity = if index % 11 == 0 { CatalogIdentity::Legacy { history_id: format!("old_{index}") } }
        else { CatalogIdentity::Native { sidecar_id: if index % 5 == 0 { "other" } else { SIDECAR_ID }.into(), directory: format!("/项目/{}", index % 3), session_id: format!("ses_{}", index / 3) } };
        let title = ["中文对话", "Straße", "İSTANBUL", "ΟΣ", "literal %_\\", "AB\0CD", "ÉCOLE", "same"][index % 8];
        let source = [ConversationSource::LightChat, ConversationSource::Workbench, ConversationSource::Legacy][index % 3];
        let mut row = new_entry(identity, title.into(), source, 1, (index % 41) as u64);
        if index % 97 == 0 { row.updated = u64::MAX; }
        if index % 89 == 0 { row.updated = i64::MAX as u64 + 1; }
        row.user_title = match index % 7 { 0 => Some("Override 标题".into()), 1 => Some(String::new()), _ => None };
        row.pinned = index % 4 == 0;
        row.archived = index % 5 == 0;
        row.has_records = match index % 6 { 0 => Some(false), 1 => None, _ => Some(true) };
        row.runtime = [RuntimeState::Idle, RuntimeState::Running, RuntimeState::Unknown][index % 3];
        row.ownership = if index % 9 == 0 { Ownership::Agent } else { Ownership::Unowned };
        row.availability = [Availability::Available, Availability::Stale, Availability::Unavailable][index % 3];
        if index % 13 == 0 { row.tombstone = Some(DeletionTombstone { requested_at: 50, remote_deleted: index % 2 == 0 }); }
        row
    }).collect()
}

fn queries() -> Vec<CatalogQuery> {
    let mut result = vec![CatalogQuery::default()];
    for search in [" 中文 ", "STRASSE", "straße", "İST", "ΟΣ", "ÉCOLE", "%_", "\\", "\0c", "标题", "/项目/1", "no match", "   "] {
        result.push(CatalogQuery { search: Some(search.into()), ..Default::default() });
    }
    for archived in [false, true] {
        for pinned in [None, Some(false), Some(true)] {
            for source in [None, Some(ConversationSource::LightChat), Some(ConversationSource::Workbench), Some(ConversationSource::Legacy)] {
                for directory in [None, Some("/项目/1"), Some("")] {
                    result.push(CatalogQuery { archived: Some(archived), pinned, source, directory: directory.map(str::to_owned), ..Default::default() });
                }
            }
        }
    }
    for (from, to) in [(0, 0), (10, 20), (20, 10), (i64::MAX as u64, u64::MAX), (u64::MAX, u64::MAX)] {
        result.push(CatalogQuery { from: Some(from), to: Some(to), ..Default::default() });
    }
    for (offset, limit) in [(0, 0), (0, 1), (50, 50), (999, 201), (0, usize::MAX), (usize::MAX, 50)] {
        result.push(CatalogQuery { offset: Some(offset), limit: Some(limit), ..Default::default() });
    }
    result
}

fn assert_equivalent(store: &CatalogStore, rows: &[CatalogEntry], query: CatalogQuery) -> usize {
    let expected = oracle(rows.to_vec(), query.clone(), vec!["/unrelated".into()], vec!["native_unavailable".into()]);
    DECODED_ROWS.with(|count| count.set(0));
    let actual = store.page(query, vec!["native_unavailable".into()]).unwrap();
    let decoded = DECODED_ROWS.with(|count| count.get());
    assert_eq!(decoded, actual.items.len());
    assert!(decoded <= 200);
    assert_eq!(serde_json::to_value(&actual).unwrap(), serde_json::to_value(expected).unwrap());
    decoded
}

#[test]
fn sql_pages_match_full_oracle_for_filters_unicode_unsigned_time_and_all_pages() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("catalog.sqlite");
    let rows = fixtures(1007);
    seed_v2(&path, &rows);
    let store = CatalogStore::open(&path).unwrap();
    for query in queries() { assert_equivalent(&store, &rows, query); }
    for archived in [false, true] {
        for offset in (0..rows.len()).step_by(50) {
            assert_equivalent(&store, &rows, CatalogQuery { archived: Some(archived), offset: Some(offset), ..Default::default() });
        }
    }
}

#[test]
fn offline_and_directories_are_global_but_empty_and_tombstoned_rows_do_not_count() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("catalog.sqlite");
    let mut rows = fixtures(3);
    for row in &mut rows { row.availability = Availability::Available; row.has_records = Some(true); row.tombstone = None; }
    rows[1].archived = true; rows[1].availability = Availability::Unavailable;
    seed_v2(&path, &rows);
    let store = CatalogStore::open(&path).unwrap();
    let query = CatalogQuery { search: Some("nonexistent".into()), ..Default::default() };
    let empty = store.page(query.clone(), vec![]).unwrap();
    assert!(empty.offline && empty.items.is_empty());
    assert_eq!(empty.directories, vec!["/unrelated"]);
    store.update_entry(&rows[1].key(), |row| {
        let row = row.as_mut().unwrap(); row.has_records = Some(false); row.runtime = RuntimeState::Idle; row.ownership = Ownership::Unowned; Ok(())
    }).unwrap();
    assert!(!store.page(query.clone(), vec![]).unwrap().offline);
    store.update_entry(&rows[1].key(), |row| { row.as_mut().unwrap().ownership = Ownership::Agent; Ok(()) }).unwrap();
    assert!(store.page(query.clone(), vec![]).unwrap().offline);
    store.update_entry(&rows[1].key(), |row| { row.as_mut().unwrap().tombstone = Some(DeletionTombstone { requested_at: 3, remote_deleted: false }); Ok(()) }).unwrap();
    assert!(!store.page(query, vec![]).unwrap().offline);
}

#[test]
fn metadata_projection_and_removal_commit_or_roll_back_together() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("catalog.sqlite");
    let rows = fixtures(50);
    seed_v2(&path, &rows);
    let store = CatalogStore::open(&path).unwrap();
    let before = serde_json::to_value(store.page(CatalogQuery::default(), vec![]).unwrap()).unwrap();
    let connection = Connection::open(&path).unwrap();
    connection.execute_batch("CREATE TRIGGER reject_projection BEFORE INSERT ON entry_projection BEGIN SELECT RAISE(ABORT, 'synthetic projection failure'); END;").unwrap();
    assert_eq!(store.update_entry(&rows[2].key(), |row| { row.as_mut().unwrap().user_title = Some("new title".into()); Ok(()) }).unwrap_err(), "history_catalog_unavailable");
    assert_eq!(store.get(&rows[2].key()).unwrap(), rows[2]);
    assert_eq!(serde_json::to_value(store.page(CatalogQuery::default(), vec![]).unwrap()).unwrap(), before);
    assert!(store.update(|rows| { rows[0].pinned = !rows[0].pinned; rows[1].archived = !rows[1].archived; Ok(()) }).is_err());
    assert_eq!(store.all().unwrap().len(), rows.len());
    connection.execute_batch("DROP TRIGGER reject_projection;").unwrap();
    let mut expected = rows.clone();
    expected[2].pinned = !expected[2].pinned;
    expected[2].user_title = Some("new title".into());
    store.update_entry(&rows[2].key(), |row| { *row = Some(expected[2].clone()); Ok(()) }).unwrap();
    assert_equivalent(&store, &expected, CatalogQuery::default());
    store.update_entry(&rows[2].key(), |row| { *row = None; Ok(()) }).unwrap();
    expected.remove(2);
    assert_equivalent(&store, &expected, CatalogQuery::default());
    assert_eq!(connection.query_row("SELECT count(*) FROM entry_projection", [], |row| row.get::<_, i64>(0)).unwrap(), expected.len() as i64);
}

#[test]
fn unprojected_or_corrupt_metadata_fails_listing_even_outside_returned_page() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("catalog.sqlite");
    let rows = fixtures(70);
    seed_v2(&path, &rows);
    let store = CatalogStore::open(&path).unwrap();
    let connection = Connection::open(&path).unwrap();
    connection.execute("UPDATE entries SET metadata='invalid' WHERE key=?1", [rows[0].key()]).unwrap();
    assert!(matches!(store.page(CatalogQuery { search: Some("none".into()), ..Default::default() }, vec![]), Err(error) if error == "history_catalog_invalid"));
    assert!(store.all().is_err());
    assert_eq!(store.get(&rows[1].key()).unwrap(), rows[1]);
    connection.execute("UPDATE entries SET metadata=?1 WHERE key=?2", (serde_json::to_string(&rows[0]).unwrap(), rows[0].key())).unwrap();
    // Direct SQL did not publish a validated projection; even valid JSON must not serve stale counts.
    assert!(store.page(CatalogQuery::default(), vec![]).is_err());
    store.update_entry(&rows[0].key(), |row| { row.as_mut().unwrap().pinned = !rows[0].pinned; Ok(()) }).unwrap();
    assert!(store.page(CatalogQuery::default(), vec![]).is_ok());
}

#[test]
fn page_count_and_global_metadata_share_a_wal_read_snapshot() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("catalog.sqlite");
    let rows = fixtures(80);
    seed_v2(&path, &rows);
    let store = CatalogStore::open(&path).unwrap();
    let mut reader = store.connection().unwrap();
    reader.pragma_update(None, "journal_mode", "WAL").unwrap();
    let tx = reader.transaction().unwrap();
    let before = serde_json::to_value(read_page(&tx, CatalogQuery::default(), vec![]).unwrap()).unwrap();
    // A writer can commit while the page's DEFERRED transaction remains active.
    store.update(|rows| {
        rows.remove(1);
        rows[0].tombstone = Some(DeletionTombstone { requested_at: 123, remote_deleted: true });
        for row in rows { row.archived = !row.archived; }
        Ok(())
    }).unwrap();
    store.register_directory("/new-directory").unwrap();
    assert_eq!(serde_json::to_value(read_page(&tx, CatalogQuery::default(), vec![]).unwrap()).unwrap(), before);
    tx.commit().unwrap();
    assert_ne!(serde_json::to_value(store.page(CatalogQuery::default(), vec![]).unwrap()).unwrap(), before);
}

#[test]
#[ignore = "manual H18c 1k/10k/100k migration, query equivalence and work measurement"]
fn sql_paging_scaling_benchmark() {
    for size in [1_000, 10_000, 100_000] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("catalog.sqlite");
        let rows = fixtures(size);
        seed_v2(&path, &rows);
        let started = std::time::Instant::now();
        let store = CatalogStore::open(&path).unwrap();
        println!("H18c migration rows={size} elapsed={:?}", started.elapsed());
        for query in queries() { assert_equivalent(&store, &rows, query); }
        for (label, query) in [("first", CatalogQuery::default()),
            ("deep", CatalogQuery { offset: Some(size / 2), ..Default::default() }),
            ("search", CatalogQuery { search: Some("标题".into()), ..Default::default() }),
            ("directory", CatalogQuery { directory: Some("/项目/1".into()), ..Default::default() }),
            ("source", CatalogQuery { source: Some(ConversationSource::Workbench), ..Default::default() })] {
            let decoded = assert_equivalent(&store, &rows, query.clone());
            let filter = Filter::new(&query);
            let sql = filter.page_sql();
            let connection = store.connection().unwrap();
            let mut count = connection.prepare(&format!("SELECT count(*) FROM entry_projection WHERE {}", filter.sql)).unwrap();
            count.query_row(params_from_iter(&filter.values), |row| row.get::<_, i64>(0)).unwrap();
            let count_steps = count.get_status(rusqlite::StatementStatus::VmStep);
            let mut values = filter.values;
            values.extend([Value::Integer(50), Value::Integer(query.offset.unwrap_or(0) as i64)]);
            let mut explain = connection.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
            let plan: Vec<String> = explain.query_map(params_from_iter(&values), |row| row.get(3)).unwrap().collect::<Result<_, _>>().unwrap();
            let expected_index = match label { "directory" => "catalog_directory_order", "source" => "catalog_source_order", _ => "catalog_page_order" };
            assert!(plan.iter().any(|line| line.contains(expected_index)), "{plan:?}");
            let mut statement = connection.prepare(&sql).unwrap();
            for _ in 0..3 { store.page(query.clone(), vec![]).unwrap(); }
            let mut elapsed = Vec::new();
            for _ in 0..30 {
                let started = std::time::Instant::now();
                store.page(query.clone(), vec![]).unwrap();
                elapsed.push(started.elapsed());
            }
            elapsed.sort();
            let mut selected = statement.query(params_from_iter(&values)).unwrap();
            while selected.next().unwrap().is_some() {}
            drop(selected);
            let steps = statement.get_status(rusqlite::StatementStatus::VmStep);
            println!("H18c rows={size} case={label} decoded={decoded} page_vm_steps={steps} count_vm_steps={count_steps} samples=30 p50_ms={:.3} p95_ms={:.3} plan={plan:?}", elapsed[14].as_secs_f64() * 1000.0, elapsed[28].as_secs_f64() * 1000.0);
        }
    }
}
