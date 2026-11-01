//! Logical expiry is observable immediately, without a maintenance write.
use super::*;
use crate::memory::export;
use std::sync::Mutex;

const NOW: &str = "2026-10-25T00:00:00Z";
const FUTURE: &str = "2027-01-01T00:00:00Z";
struct TestClock(Mutex<String>);
impl TestClock {
    fn new() -> Self { Self(Mutex::new(NOW.into())) }
}
impl Clock for TestClock {
    fn now(&self) -> String { self.0.lock().unwrap().clone() }
    fn now_plus_hours(&self, _: i64) -> String { FUTURE.into() }
}
fn repo() -> MemoryRepository<TestClock> {
    MemoryRepository::new(MemoryStore::open_in_memory().unwrap(), TestClock::new())
}
fn seed(store: &MemoryStore, count: usize, deadline: &str) {
    store.with_transaction(|tx| {
        for i in 0..count {
            tx.execute("INSERT INTO memories(id,scope,persona_id,type,memory_key,content,status,confidence,importance,sensitivity,source_kind,valid_from,expires_at,created_at,updated_at,revision)
                VALUES(?1,'global',NULL,'preference',?1,'synthetic 甜食','active',1,3,'normal','explicit','old',?2,'old','last-edit',1)", params![format!("seed-{i}"), deadline])?;
            tx.execute("INSERT INTO memory_sources(id,memory_id,source_kind,created_at) VALUES(?1,?1,'explicit','old')", [format!("seed-{i}")])?;
            tx.execute("INSERT INTO memory_task_links(memory_id,task_id,created_at) VALUES(?1,'task','old')", [format!("seed-{i}")])?;
        }
        Ok(())
    }).unwrap();
}
fn raw_active(store: &MemoryStore) -> i64 {
    store.with_connection(|db| Ok(db.query_row("SELECT count(*) FROM memories WHERE status='active'", [], |r| r.get(0))?)).unwrap()
}
fn query(statuses: Vec<MemoryStatus>) -> MemoryQuery {
    MemoryQuery { statuses: Some(statuses), ..Default::default() }
}
fn request(key: &str, expiry: Option<&str>) -> NewMemory {
    NewMemory { scope: MemoryScope::Global, persona_id: None, memory_type: MemoryType::Preference,
        memory_key: Some(key.into()), content: "synthetic 甜食".into(), importance: None,
        expires_at: expiry.map(str::to_owned), source_kind: SourceKind::Explicit,
        conversation_id: None, message_id: None, sensitive_confirmed: false }
}

#[test]
fn logical_and_materialized_lists_search_export_and_retrieval_agree() {
    let repository = repo();
    seed(&repository.store, 24, NOW);
    repository.store.with_transaction(|tx| {
        for (id, status, expiry, scope, persona) in [
            (0,"active",Some(FUTURE),"global",None),
            (1,"active",None,"global",None),
            (2,"superseded",Some(NOW),"global",None),
            (3,"expired",Some(FUTURE),"global",None),
            (4,"active",Some(NOW),"persona",Some("viewer")),
            (5,"active",Some(NOW),"persona",Some("other")),
            // Preserve the pre-existing lexical RFC3339 TEXT contract.
            (6,"active",Some("2026-10-25T08:00:00+08:00"),"global",None),
            (7,"active",Some("2026-10-25T00:00:00.500Z"),"global",None),
        ] {
            tx.execute("UPDATE memories SET status=?2,expires_at=?3,scope=?4,persona_id=?5 WHERE id=?1", params![format!("seed-{id}"),status,expiry,scope,persona])?;
        }
        Ok(())
    }).unwrap();
    let mut queries = Vec::new();
    for statuses in [vec![MemoryStatus::Active], vec![MemoryStatus::Expired], vec![MemoryStatus::Superseded],
        vec![MemoryStatus::Active,MemoryStatus::Expired,MemoryStatus::Superseded]] {
        for persona in [None, Some("viewer"), Some("other")] {
            for search in [None, Some("synthetic"), Some("甜食")] {
                queries.push(MemoryQuery { statuses: Some(statuses.clone()), persona_id: persona.map(str::to_owned),
                    search: search.map(str::to_owned), ..Default::default() });
            }
        }
    }
    let physical_before = raw_active(&repository.store);
    let before: Vec<_> = queries.iter().map(|q| repository.list(q).unwrap()).collect();
    let exported = export::build(&repository,"fixture").unwrap();
    let retrieved = repository.retrieve("viewer", &["甜食".into()], &[MemoryType::Preference], 200).unwrap();
    assert_eq!(retrieved.len(), 3); // future, no deadline, lexically future offset
    assert_eq!(before[0].len(), 3);
    assert_eq!(before[9].len(), 20); // already expired + all logically due
    assert_eq!(before[10].len(), 0); // FTS remains active-only for explicit expired search
    assert_eq!(before[11].len(), 20); // explicit history's short LIKE preserves its contract
    assert_eq!(raw_active(&repository.store), physical_before, "reads must not materialize");
    let mut batches = 0;
    while let Some((memories, candidates)) = repository.maintain_expiry().unwrap() {
        assert!(memories <= EXPIRY_MEMORY_BATCH as usize && candidates == 0);
        batches += 1;
        assert!(batches <= 24);
    }
    assert!(batches > 1);
    for (q, expected) in queries.iter().zip(before) { assert_eq!(repository.list(q).unwrap(), expected); }
    assert_eq!(export::build(&repository,"fixture").unwrap(), exported);
    assert_eq!(repository.retrieve("viewer", &["甜食".into()], &[MemoryType::Preference], 200).unwrap(), retrieved);
}

#[test]
fn reads_work_in_query_only_mode_and_do_not_require_a_writer_reservation() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("expiry.db");
    let repository = MemoryRepository::new(MemoryStore::open(&path).unwrap(), TestClock::new());
    seed(&repository.store, 20, NOW);
    let writer = Connection::open(&path).unwrap();
    writer.execute_batch("BEGIN IMMEDIATE; UPDATE memories SET content='uncommitted' WHERE id='seed-0';").unwrap();
    repository.store.with_connection(|db| {
        db.busy_timeout(std::time::Duration::from_millis(10))?;
        db.pragma_update(None,"query_only",true)?;
        Ok(())
    }).unwrap();
    assert!(repository.list(&MemoryQuery::default()).unwrap().is_empty());
    assert_eq!(repository.list(&query(vec![MemoryStatus::Expired])).unwrap().len(),20);
    assert!(repository.retrieve("viewer", &["synthetic".into()], &[], 200).unwrap().is_empty());
    assert_eq!(export::build(&repository,"fixture").unwrap().memories.len(),20);
    writer.execute_batch("COMMIT").unwrap();
    assert_eq!(repository.list(&query(vec![MemoryStatus::Expired])).unwrap().iter().find(|m|m.memory.id=="seed-0").unwrap().memory.content,"uncommitted");
    assert_eq!(raw_active(&repository.store),20);
}

#[test]
fn maintenance_is_bounded_atomic_retryable_and_skips_busy_writers() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("expiry.db");
    let repository = MemoryRepository::new(MemoryStore::open(&path).unwrap(), TestClock::new());
    seed(&repository.store, 25, NOW);
    repository.store.with_connection(|_| {
        assert_eq!(repository.maintain_expiry().unwrap(), None, "yield to an occupied local connection");
        Ok(())
    }).unwrap();
    repository.store.with_transaction(|tx| {
        for i in 0..70 {
            tx.execute("INSERT INTO memory_candidates(id,scope,type,content,confidence,importance,decision,created_at,expires_at) VALUES(?1,'global','preference','candidate',1,3,'pending','old',?2)",params![format!("candidate-{i}"), NOW])?;
        }
        tx.execute_batch("CREATE TRIGGER fail_cleanup BEFORE DELETE ON memory_candidates BEGIN SELECT RAISE(ABORT,'retry fixture'); END;")?;
        Ok(())
    }).unwrap();
    assert!(repository.maintain_expiry().is_err());
    assert_eq!(raw_active(&repository.store),25);
    repository.store.with_connection(|db| {
        assert_eq!(db.query_row::<i64,_,_>("PRAGMA busy_timeout",[],|r|r.get(0))?,5000);
        assert_eq!(db.query_row::<i64,_,_>("SELECT count(*) FROM memory_search",[],|r|r.get(0))?,25);
        db.execute_batch("DROP TRIGGER fail_cleanup")?; Ok(())
    }).unwrap();
    let writer = Connection::open(&path).unwrap();
    writer.execute_batch("BEGIN IMMEDIATE").unwrap();
    let started = std::time::Instant::now();
    assert_eq!(repository.maintain_expiry().unwrap(),None);
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
    writer.execute_batch("COMMIT").unwrap();
    let (changed, candidates) = repository.maintain_expiry().unwrap().unwrap();
    assert!((1..=8).contains(&changed));
    assert_eq!(candidates, 32);
    // A foreground forget can run between batches and erases even a due row.
    repository.forget("seed-24").unwrap();
    let mut batches = 0;
    while let Some((memories,candidates)) = repository.maintain_expiry().unwrap() {
        assert!(memories<=8 && candidates<=32);
        batches+=1; assert!(batches<=25);
    }
    assert_eq!(raw_active(&repository.store),0);
    repository.store.with_connection(|db| {
        assert_eq!(db.query_row::<i64,_,_>("SELECT count(*) FROM memory_candidates",[],|r|r.get(0))?,0);
        assert_eq!(db.query_row::<i64,_,_>("SELECT count(*) FROM memory_sources WHERE memory_id='seed-24'",[],|r|r.get(0))?,0);
        assert_eq!(db.query_row::<i64,_,_>("SELECT count(*) FROM memory_search",[],|r|r.get(0))?,0);
        Ok(())
    }).unwrap();
}

#[test]
fn expiry_survives_restart_and_nearest_future_deadline_needs_no_write() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("expiry.db");
    {
        let repository = MemoryRepository::new(MemoryStore::open(&path).unwrap(), TestClock::new());
        seed(&repository.store, 1, FUTURE);
        repository.store.with_connection(|db| { db.pragma_update(None,"query_only",true)?; Ok(()) }).unwrap();
        assert_eq!(repository.maintain_expiry().unwrap(), None);
        assert_eq!(repository.list(&MemoryQuery::default()).unwrap().len(), 1);
    }
    let repository = MemoryRepository::new(MemoryStore::open(&path).unwrap(), TestClock::new());
    *repository.clock.0.lock().unwrap() = FUTURE.into();
    assert!(repository.list(&MemoryQuery::default()).unwrap().is_empty());
    assert_eq!(repository.list(&query(vec![MemoryStatus::Expired])).unwrap().len(),1);
    assert_eq!(raw_active(&repository.store),1);
    assert_eq!(repository.maintain_expiry().unwrap(), Some((1,0)));
    assert_eq!(repository.maintain_expiry().unwrap(), None);
}

#[test]
fn writes_handle_only_the_target_expiry_and_cannot_revive_expired_rows() {
    let repository = repo();
    seed(&repository.store, 20, NOW);
    let replacement = repository.create(&request("seed-0", None)).unwrap();
    assert_eq!(replacement.supersedes_id, None);
    assert_eq!(replacement.status, MemoryStatus::Active);
    assert_eq!(raw_active(&repository.store),20); // one old row expired, one new active
    let edited = repository.update(&MemoryUpdate {
        id: "seed-1".into(), expected_revision: 1, content: "edited synthetic".into(),
        importance: None, expires_at: Some(FUTURE.into()), sensitive_confirmed: false,
    }).unwrap();
    assert_eq!(edited.status, MemoryStatus::Expired);
    assert_eq!(edited.revision,2);
    let immediate = repository.create(&request("immediate",Some(NOW))).unwrap();
    assert_eq!(immediate.status,MemoryStatus::Expired);
    assert_eq!(repository.list(&MemoryQuery::default()).unwrap().len(),1);
    repository.store.with_connection(|db| {
        assert_eq!(db.query_row::<String,_,_>("SELECT status FROM memories WHERE id='seed-2'",[],|r|r.get(0))?,"active");
        assert_eq!(db.query_row::<i64,_,_>("SELECT count(*) FROM memory_search WHERE memory_id=?1",[immediate.id],|r|r.get(0))?,0);
        Ok(())
    }).unwrap();
}

#[test]
fn candidate_deadline_alone_triggers_cleanup_and_preserves_decided_or_future_rows() {
    let repository = repo();
    repository.store.with_transaction(|tx| {
        for (id, decision, expires) in [("due","pending",NOW),("future","pending",FUTURE),
            ("accepted","accepted",NOW),("dismissed","dismissed",NOW)] {
            tx.execute("INSERT INTO memory_candidates(id,scope,type,content,confidence,importance,decision,created_at,expires_at) VALUES(?1,'global','preference','candidate',1,3,?2,'old',?3)",params![id,decision,expires])?;
        }
        Ok(())
    }).unwrap();
    assert_eq!(repository.maintain_expiry().unwrap(),Some((0,1)));
    assert_eq!(repository.maintain_expiry().unwrap(),None);
    repository.store.with_connection(|db| {
        assert_eq!(db.query_row::<i64,_,_>("SELECT count(*) FROM memory_candidates",[],|r|r.get(0))?,3);
        Ok(())
    }).unwrap();
}
