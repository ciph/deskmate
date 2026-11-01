use super::*;
use super::fts_tests::{consistent,seed};
use rusqlite::StatementStatus;

// Extract the actual production DELETE from each schema's update trigger.
// Only replace the trigger-bound UUID with the same prepared-statement input.
fn trigger_delete(db: &Connection) -> String {
    let trigger: String=db.query_row("SELECT sql FROM sqlite_master WHERE name='trg_memories_search_update'",[],|r|r.get(0)).unwrap();
    let start=trigger.find("DELETE FROM memory_search").unwrap();
    trigger[start..].split(';').next().unwrap().replace("NEW.id","?1").replace("OLD.id","?1")
}

#[test]
#[ignore = "manual M19c 1k/10k/100k actual trigger and mutation scaling comparison"]
fn precise_fts_scaling_benchmark() {
    let mut precise_probe_steps=Vec::new();
    for size in [1_000,10_000,100_000] {
        for schema in [3,4] {
            let root=tempfile::tempdir().unwrap();
            let store=MemoryStore {connection: std::sync::Mutex::new(open_connection(&root.path().join(DB_FILE_NAME)).unwrap())};
            store.migrate_steps(&MIGRATIONS[..schema]).unwrap();
            store.with_connection(|db| {
                seed(db,size);
                let sql=trigger_delete(db);
                let plan:Vec<String>=db.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap()
                    .query_map(["memory-000000"],|r|r.get(3)).unwrap().collect::<Result<_,_>>().unwrap();
                if schema==4 {
                    assert!(plan.iter().any(|line|line.contains("VIRTUAL TABLE INDEX 0:=")),"{plan:?}");
                    assert!(plan.iter().any(|line|line.contains("SEARCH memory_search_ids USING COVERING INDEX")),"{plan:?}");
                }
                db.execute_batch("SAVEPOINT probe")?;
                let mut statement=db.prepare(&sql)?;
                let start=std::time::Instant::now();
                assert_eq!(statement.execute(["memory-000000"])?,1);
                let elapsed=start.elapsed();
                let steps=statement.get_status(StatementStatus::VmStep);
                if schema==4 { precise_probe_steps.push(steps); }
                drop(statement);
                db.execute_batch("ROLLBACK TO probe; RELEASE probe")?;
                println!("M19c rows={size} schema={schema} fts_delete_vm_steps={steps} fts_delete_ms={:.3} plan={plan:?}",elapsed.as_secs_f64()*1000.0);
                for (label,sql) in [
                    ("metadata","UPDATE memories SET importance=4,updated_at='edited',revision=revision+1 WHERE id=?1"),
                    ("content","UPDATE memories SET content='changed synthetic 不喜欢甜食' WHERE id=?1"),
                    ("delete","DELETE FROM memories WHERE id=?1"),
                ] {
                    db.execute_batch("BEGIN IMMEDIATE")?;
                    let mut statement=db.prepare(sql)?;
                    let mut times=Vec::new();
                    for i in 0..20 {
                        let start=std::time::Instant::now();
                        assert_eq!(statement.execute([format!("memory-{i:06}")])?,1);
                        times.push(start.elapsed());
                    }
                    let steps=statement.get_status(StatementStatus::VmStep);
                    let scans=statement.get_status(StatementStatus::FullscanStep);
                    drop(statement);
                    let commit=std::time::Instant::now();
                    db.execute_batch("COMMIT")?;
                    let commit=commit.elapsed();
                    times.sort();
                    println!("M19c rows={size} schema={schema} operation={label} count=20 vm_steps={steps} fullscan_steps={scans} p50_ms={:.3} p95_ms={:.3} commit_ms={:.3}",
                        times[9].as_secs_f64()*1000.0,times[18].as_secs_f64()*1000.0,commit.as_secs_f64()*1000.0);
                }
                if schema==4 { consistent(db); }
                Ok(())
            }).unwrap();
        }
    }
    // FTS positioning stays bounded; full DELETE also performs existing FK and
    // privacy work, reported separately instead of attributing all work to FTS.
    assert!(precise_probe_steps.iter().all(|steps|*steps<200));
    assert!(precise_probe_steps.iter().max().unwrap()-precise_probe_steps.iter().min().unwrap()<=10);
}
