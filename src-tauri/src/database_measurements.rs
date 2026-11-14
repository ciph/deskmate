//! Opt-in measurements use temporary, on-disk stores. No timing threshold is a correctness test.
use serde_json::{json, Value};

pub(crate) fn sample(mut operation: impl FnMut()) -> Value {
    let mut times = Vec::new();
    for _ in 0..7 {
        let start = std::time::Instant::now();
        operation();
        times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    times.sort_by(f64::total_cmp);
    json!({"p50_ms":times[3],"p95_ms":times[6],"samples_ms":times})
}

#[test]
#[ignore = "synthetic disk benchmark; run explicitly with --ignored --nocapture"]
fn database_scale_worklog() {
    use crate::worklog::{contract::DateQuery, repository::Repository, storage::WorklogStore};
    for count in [10_000,100_000] {
        let root = tempfile::tempdir().unwrap();
        let store = WorklogStore::open(&root.path().join("worklog.db")).unwrap();
        store.with_transaction(|db| {
            let mut insert = db.prepare("INSERT INTO work_entries VALUES(?1,'2026-11-01','project','original complete text','summary text','done','session','message',1,'2026-11-01','2026-11-01')")?;
            for id in 0..count { insert.execute([format!("entry-{id:06}")])?; }
            Ok(())
        }).unwrap();
        let repo = Repository::new(store);
        let id = format!("entry-{:06}",count-1);
        let query = DateQuery {start:"0001-01-01".into(),end:"9999-12-31".into(),project:None};
        let all = repo.query_entries(&query).unwrap();
        let one = repo.get_entry(&id).unwrap().unwrap();
        assert_eq!(serde_json::to_value(all.last().unwrap()).unwrap(),serde_json::to_value(&one).unwrap());
        let all_bytes = serde_json::to_vec(&all).unwrap().len();
        let one_bytes = serde_json::to_vec(&one).unwrap().len();
        drop(all);
        let old = sample(|| { std::hint::black_box(repo.query_entries(&query).unwrap()); });
        let new = sample(|| { std::hint::black_box(repo.get_entry(&id).unwrap()); });
        let plan = repo.store.with_connection(|db| Ok(db.query_row("EXPLAIN QUERY PLAN SELECT * FROM work_entries WHERE id=?1",[&id],|r|r.get::<_,String>(3))?)).unwrap();
        assert!(plan.contains("INDEX"),"{plan}");
        println!("DATABASE_MEASUREMENT {}",json!({"finding":"F09","rows":count,"old":old,"new":new,"old_payload_bytes":all_bytes,"new_payload_bytes":one_bytes,"plan":plan}));
    }
}
