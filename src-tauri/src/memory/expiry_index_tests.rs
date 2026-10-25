use super::*;
use rusqlite::StatementStatus;

const NOW: &str = "2026-10-25T00:00:00Z";
const DUE_IDS: &str = "SELECT id FROM memories WHERE status='active' AND expires_at IS NOT NULL AND expires_at<=?1";
const SCAN_IDS: &str = "SELECT id FROM memories NOT INDEXED WHERE status='active' AND expires_at IS NOT NULL AND expires_at<=?1";
const INSERT: &str = "INSERT INTO memories(id,scope,persona_id,type,content,status,confidence,importance,sensitivity,source_kind,valid_from,expires_at,created_at,updated_at,revision)
    VALUES(?1,?2,?3,'preference','synthetic expiry evidence',?4,1,3,'normal','explicit','2026-01-01T00:00:00Z',?5,'2026-01-01T00:00:00Z','2026-01-01T00:00:00Z',1)";

fn ids(connection: &Connection, sql: &str) -> Vec<String> {
    let mut statement = connection.prepare(sql).unwrap();
    let mut result = statement.query_map([NOW], |row| row.get(0)).unwrap().collect::<Result<Vec<String>, _>>().unwrap();
    result.sort();
    result
}

fn update_plan(connection: &Connection) -> Vec<String> {
    connection.prepare(&format!("EXPLAIN QUERY PLAN {EXPIRE_MEMORIES_SQL}")).unwrap()
        .query_map([NOW], |row| row.get(3)).unwrap().collect::<Result<_, _>>().unwrap()
}

fn seed_boundaries(store: &MemoryStore) {
    store.with_transaction(|tx| {
        let mut insert = tx.prepare(INSERT)?;
        let times = [None, Some("2026-10-24T23:59:59Z"), Some(NOW), Some("2026-10-25T00:00:01Z"),
            Some("2026-10-25T08:00:00+08:00"), Some("2026-10-24T23:00:00-01:00"), Some("2026-10-25T00:00:00.500Z")];
        for (index, expires) in times.iter().enumerate() {
            for status in ["active", "expired", "superseded"] {
                let persona = if index % 2 == 0 { Some("other") } else { None };
                insert.execute(params![format!("{index}-{status}"), if persona.is_some() { "persona" } else { "global" }, persona, status, expires])?;
            }
        }
        for (id, decision, expires) in [("pending-due", "pending", NOW), ("pending-future", "pending", "2027-01-01T00:00:00Z"), ("accepted-due", "accepted", NOW)] {
            tx.execute("INSERT INTO memory_candidates(id,scope,type,content,confidence,importance,decision,created_at,expires_at) VALUES(?1,'global','preference','synthetic candidate',1,3,?2,'old',?3)", params![id, decision, expires])?;
        }
        Ok(())
    }).unwrap();
}

fn state(connection: &Connection) -> (Vec<(String, String, String)>, Vec<String>, Vec<String>) {
    let memories = connection.prepare("SELECT id,status,updated_at FROM memories ORDER BY id").unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).unwrap().collect::<Result<_, _>>().unwrap();
    let fts = connection.prepare("SELECT memory_id FROM memory_search ORDER BY memory_id").unwrap()
        .query_map([], |row| row.get(0)).unwrap().collect::<Result<_, _>>().unwrap();
    let candidates = connection.prepare("SELECT id FROM memory_candidates ORDER BY id").unwrap()
        .query_map([], |row| row.get(0)).unwrap().collect::<Result<_, _>>().unwrap();
    (memories, fts, candidates)
}

#[test]
fn expiry_index_preserves_boundaries_text_time_comparison_and_candidate_cleanup() {
    let indexed = MemoryStore::open_in_memory().unwrap();
    let scan = MemoryStore::open_in_memory().unwrap();
    for store in [&indexed, &scan] { seed_boundaries(store); }
    scan.with_connection(|connection| { connection.execute_batch("DROP INDEX idx_memories_active_expiry")?; Ok(()) }).unwrap();
    indexed.with_connection(|connection| {
        let plan = update_plan(connection);
        assert!(plan.iter().any(|line| line.starts_with("SEARCH memories USING ") && line.contains("INDEX idx_memories_active_expiry (expires_at>")), "{plan:?}");
        assert_eq!(ids(connection, DUE_IDS), ids(connection, SCAN_IDS));
        // Preserve existing TEXT comparison, including offset/fractional representations;
        // this index change does not normalize or reinterpret stored timestamps.
        assert_eq!(ids(connection, DUE_IDS), vec!["1-active", "2-active", "5-active", "6-active"]);
        Ok(())
    }).unwrap();
    for store in [&indexed, &scan] { store.with_transaction(|tx| expire_due(tx, NOW)).unwrap(); }
    let actual = indexed.with_connection(|connection| Ok(state(connection))).unwrap();
    assert_eq!(actual, scan.with_connection(|connection| Ok(state(connection))).unwrap());
    assert_eq!(actual.2, vec!["accepted-due", "pending-future"]);
    assert!(actual.0.iter().any(|(id, status, updated)| id == "2-active" && status == "expired" && updated == NOW));
    assert!(actual.0.iter().any(|(id, status, updated)| id == "2-superseded" && status == "superseded" && updated != NOW));
    assert!(!actual.1.contains(&"2-active".into()));
}

#[test]
fn failed_expiry_rolls_back_status_index_membership_fts_and_candidates() {
    let store = MemoryStore::open_in_memory().unwrap();
    seed_boundaries(&store);
    let before = store.with_connection(|connection| Ok((state(connection), ids(connection, DUE_IDS)))).unwrap();
    store.with_connection(|connection| { connection.execute_batch("CREATE TRIGGER reject_candidate_cleanup BEFORE DELETE ON memory_candidates BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;")?; Ok(()) }).unwrap();
    assert!(store.with_transaction(|tx| expire_due(tx, NOW)).is_err());
    assert_eq!(store.with_connection(|connection| Ok((state(connection), ids(connection, DUE_IDS)))).unwrap(), before);
    store.with_connection(|connection| { connection.execute_batch("DROP TRIGGER reject_candidate_cleanup;")?; Ok(()) }).unwrap();
    store.with_transaction(|tx| expire_due(tx, NOW)).unwrap();
    assert!(store.with_connection(|connection| Ok(ids(connection, DUE_IDS))).unwrap().is_empty());
}

struct FixedClock;
impl Clock for FixedClock {
    fn now(&self) -> String { NOW.into() }
    fn now_plus_hours(&self, _: i64) -> String { "2027-01-01T00:00:00Z".into() }
}

#[test]
fn persona_listing_keeps_global_expiry_and_does_not_expose_other_personas() {
    let store = MemoryStore::open_in_memory().unwrap();
    seed_boundaries(&store);
    let repository = MemoryRepository::new(store, FixedClock);
    let visible = repository.list(&MemoryQuery { persona_id: Some("viewer".into()), ..Default::default() }).unwrap();
    assert!(visible.iter().all(|record| record.memory.scope == MemoryScope::Global));
    repository.store.with_connection(|connection| {
        let status: String = connection.query_row("SELECT status FROM memories WHERE id='2-active' AND persona_id='other'", [], |row| row.get(0))?;
        assert_eq!(status, "expired", "expiry remains global even when listing another persona");
        Ok(())
    }).unwrap();
}

fn seed_scale(store: &MemoryStore, size: usize, case: &str) -> usize {
    let due = match case { "few_due" => 5, "concentrated" => size / 5, "other_persona" => 5, _ => 0 };
    store.with_transaction(|tx| {
        let mut insert = tx.prepare(INSERT)?;
        for index in 0..size {
            let expires = if index < due { Some("2026-10-24T00:00:00Z") }
                else if case == "future_only" || case == "concentrated" { Some("2027-01-01T00:00:00Z") } else { None };
            let persona = if case == "other_persona" && index < due { "other" } else { "viewer" };
            insert.execute(params![format!("fixture-{index:06}"), "persona", persona, "active", expires])?;
        }
        Ok(())
    }).unwrap();
    due
}

#[test]
#[ignore = "manual M19a 100k expiry access-path and candidate-selection measurement"]
fn expiry_index_scaling_benchmark() {
    for case in ["no_expiry", "future_only", "few_due", "concentrated", "other_persona"] {
        let root = tempfile::tempdir().unwrap();
        let store = MemoryStore::open(&root.path().join("memory.db")).unwrap();
        let size = 100_000;
        let expected = seed_scale(&store, size, case);
        store.with_connection(|connection| {
            let plan = update_plan(connection);
            assert!(plan.iter().any(|line| line.starts_with("SEARCH memories USING ") && line.contains("INDEX idx_memories_active_expiry (expires_at>")), "{plan:?}");
            assert!(!plan.iter().any(|line| line.contains("SCAN memories")), "{plan:?}");
            let selected = ids(connection, DUE_IDS);
            assert_eq!(selected.len(), expected);
            assert_eq!(selected, ids(connection, SCAN_IDS));
            for (label, sql) in [("indexed", DUE_IDS), ("scan_oracle", SCAN_IDS)] {
                let mut statement = connection.prepare(sql)?;
                let collect = |statement: &mut rusqlite::Statement<'_>| -> Vec<String> {
                    statement.query_map([NOW], |row| row.get(0)).unwrap().collect::<Result<_, _>>().unwrap()
                };
                assert_eq!(collect(&mut statement).len(), expected);
                let steps = statement.get_status(StatementStatus::VmStep);
                for _ in 0..3 { collect(&mut statement); }
                let mut timings = Vec::new();
                for _ in 0..30 {
                    let started = std::time::Instant::now();
                    assert_eq!(collect(&mut statement).len(), expected);
                    timings.push(started.elapsed());
                }
                timings.sort();
                println!("M19a rows={size} case={case} path={label} due={expected} select_vm_steps={steps} samples=30 p50_ms={:.3} p95_ms={:.3} update_plan={plan:?}", timings[14].as_secs_f64()*1000.0, timings[28].as_secs_f64()*1000.0);
            }
            if expected == 0 {
                let mut update = connection.prepare(EXPIRE_MEMORIES_SQL)?;
                assert_eq!(update.execute([NOW])?, 0);
                println!("M19a zero_due case={case} actual_update_vm_steps={}", update.get_status(StatementStatus::VmStep));
            }
            Ok(())
        }).unwrap();
    }
}
