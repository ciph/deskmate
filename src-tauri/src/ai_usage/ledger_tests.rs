use super::*;

#[test]
#[ignore = "synthetic disk benchmark; run explicitly with --ignored --nocapture"]
fn database_scale_usage() {
    use crate::database_measurements::sample;
    use chrono::TimeZone;
    use std::io::Write;
    let now = chrono::Local::now();
    let today = now.date_naive();
    let (start,end) = super::super::day_window(now).unwrap();
    for count in [10_000,100_000] {
        let old_root = tempfile::tempdir().unwrap();
        let new_root = tempfile::tempdir().unwrap();
        let entries: HashMap<String,StoredUsage> = (0..count).map(|i| (format!("id-{i:06}"),entry(
            if i%3==0 {"q"} else {"p"},&format!("model-{}",i%7),start-(i%365)*86_400_000+5000,100))).collect();
        let bytes = serde_json::to_vec(&entries).unwrap();
        let json_bytes = bytes.len();
        fs::write(old_root.path().join(LEDGER_FILE),&bytes).unwrap();
        fs::write(new_root.path().join(LEDGER_FILE),&bytes).unwrap();
        drop(entries); drop(bytes);
        let begin = std::time::Instant::now();
        drop(Ledger::open(new_root.path()).unwrap());
        let migration_ms = begin.elapsed().as_secs_f64()*1000.0;
        let old_read = || -> HashMap<String,StoredUsage> { serde_json::from_slice(&fs::read(old_root.path().join(LEDGER_FILE)).unwrap()).unwrap() };
        let old_save = |id:String,next:StoredUsage| {
            let mut entries = old_read();
            if entries.get(&id).is_none_or(|old|old.tokens<next.tokens) {
                entries.insert(id,next);
                let bytes = serde_json::to_vec(&entries).unwrap();
                let pending = old_root.path().join("pending.tmp");
                let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&pending).unwrap();
                file.write_all(&bytes).unwrap(); file.sync_all().unwrap(); drop(file);
                fs::rename(pending,old_root.path().join(LEDGER_FILE)).unwrap();
            }
        };
        let old_summary = || {
            let mut models: HashMap<String,(u64,u64)> = HashMap::new();
            for value in old_read().values() {
                let time = chrono::Local.timestamp_millis_opt(value.created_at_ms).single();
                if value.provider_id=="p" && time.is_some_and(|time|time.date_naive()==today) {
                    let model = models.entry(value.model_id.clone()).or_default();
                    model.0 = model.0.saturating_add(value.tokens); model.1 += 1;
                }
            }
            models.values().fold((0u64,0u64),|(tokens,requests),(t,r)|(tokens.saturating_add(*t),requests+r))
        };
        let expected = old_summary();
        let begin = std::time::Instant::now();
        let actual = Ledger::open(new_root.path()).unwrap().summary("p",start,end).unwrap();
        let cold_summary_ms = begin.elapsed().as_secs_f64()*1000.0;
        assert_eq!((actual.tokens,actual.requests),expected);
        let old_today = sample(|| { std::hint::black_box(old_summary()); });
        let new_today = sample(|| { std::hint::black_box(Ledger::open(new_root.path()).unwrap().summary("p",start,end).unwrap()); });
        let mut i = 0;
        let old_insert = sample(|| { i+=1; old_save(format!("new-{i}"),entry("p","model-0",start+6000,100)); });
        i = 0;
        let new_insert = sample(|| { i+=1; Ledger::open(new_root.path()).unwrap().save([(format!("new-{i}"),entry("p","model-0",start+6000,100))]).unwrap(); });
        let old_duplicate = sample(||old_save("new-1".into(),entry("p","model-0",start+6000,100)));
        let new_duplicate = sample(||Ledger::open(new_root.path()).unwrap().save([("new-1".into(),entry("p","model-0",start+6000,100))]).unwrap());
        let mut tokens = 100;
        let old_correction = sample(|| { tokens+=1; old_save("new-1".into(),entry("p","model-0",start+6000,tokens)); });
        tokens = 100;
        let new_correction = sample(|| { tokens+=1; Ledger::open(new_root.path()).unwrap().save([("new-1".into(),entry("p","model-0",start+6000,tokens))]).unwrap(); });
        let actual = Ledger::open(new_root.path()).unwrap().summary("p",start,end).unwrap();
        assert_eq!((actual.tokens,actual.requests),old_summary());
        println!("DATABASE_MEASUREMENT {}",serde_json::json!({"finding":"F04","rows":count,"legacy_bytes":json_bytes,"migration_ms":migration_ms,"cold_summary_ms":cold_summary_ms,"day_rows":expected.1,"old_today":old_today,"new_today":new_today,"old_insert":old_insert,"new_insert":new_insert,"old_duplicate":old_duplicate,"new_duplicate":new_duplicate,"old_correction":old_correction,"new_correction":new_correction}));
    }
}

fn entry(provider: &str, model: &str, created_at_ms: i64, tokens: u64) -> StoredUsage {
    StoredUsage { provider_id: provider.into(), model_id: model.into(), created_at_ms, tokens }
}
fn summary(ledger: &mut Ledger, provider: &str) -> (u64,u64) {
    let value = ledger.summary(provider,100,200).unwrap();
    (value.tokens,value.requests)
}

#[test]
fn legacy_import_preserves_all_rows_unsigned_tokens_and_original_bytes_once() {
    let root = tempfile::tempdir().unwrap();
    let legacy = root.path().join(LEDGER_FILE);
    let data = serde_json::to_vec(&HashMap::from([
        ("one",entry("p","m",110,u64::MAX)), ("two",entry("p","n",120,7)),
        ("other",entry("q","m",110,9)), ("old",entry("p","m",90,12)),
    ])).unwrap();
    fs::write(&legacy,&data).unwrap();
    let mut ledger = Ledger::open(root.path()).unwrap();
    assert_eq!(summary(&mut ledger,"p"),(u64::MAX,2));
    assert_eq!(summary(&mut ledger,"q"),(9,1));
    assert_eq!(ledger.connection.query_row("SELECT COUNT(*) FROM usage_messages",[],|r|r.get::<_,i64>(0)).unwrap(),4);
    assert_eq!(fs::read(&legacy).unwrap(),data);
    drop(ledger);
    // The committed SQLite database is canonical; the old JSON is an unchanged recovery copy.
    fs::write(&legacy,b"not reread after committed import").unwrap();
    assert_eq!(summary(&mut Ledger::open(root.path()).unwrap(),"q"),(9,1));
}

#[test]
fn malformed_import_rolls_back_schema_and_rows_and_can_retry() {
    let root = tempfile::tempdir().unwrap();
    let legacy = root.path().join(LEDGER_FILE);
    let valid = serde_json::to_string(&HashMap::from([("one",entry("p","m",110,20))])).unwrap();
    let broken = format!("{} trailing garbage",valid);
    fs::write(&legacy,&broken).unwrap();
    assert_eq!(Ledger::open(root.path()).err().unwrap(),"usage_storage_invalid");
    assert_eq!(fs::read_to_string(&legacy).unwrap(),broken);
    let db = Connection::open(root.path().join(DB_FILE)).unwrap();
    assert_eq!(db.query_row("PRAGMA user_version",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    assert_eq!(db.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    drop(db);
    fs::write(&legacy,&valid).unwrap();
    assert_eq!(summary(&mut Ledger::open(root.path()).unwrap(),"p"),(20,1));
}

#[test]
fn unreadable_import_and_future_schema_fail_without_erasing_source() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join(LEDGER_FILE)).unwrap();
    assert_eq!(Ledger::open(root.path()).err().unwrap(),"usage_storage_unavailable");
    assert!(root.path().join(LEDGER_FILE).is_dir());
    let future = tempfile::tempdir().unwrap();
    let db = Connection::open(future.path().join(DB_FILE)).unwrap();
    db.execute_batch("PRAGMA user_version=2; CREATE TABLE preserved(value TEXT); INSERT INTO preserved VALUES('sentinel');").unwrap();
    assert_eq!(Ledger::open(future.path()).err().unwrap(),"usage_storage_invalid");
    assert_eq!(db.query_row("PRAGMA journal_mode",[],|r|r.get::<_,String>(0)).unwrap(),"delete");
    assert_eq!(db.query_row("SELECT value FROM preserved",[],|r|r.get::<_,String>(0)).unwrap(),"sentinel");
}

#[test]
fn duplicate_json_keys_keep_the_legacy_last_value() {
    let root = tempfile::tempdir().unwrap();
    let a = serde_json::to_string(&entry("p","m",110,20)).unwrap();
    let b = serde_json::to_string(&entry("q","n",110,30)).unwrap();
    fs::write(root.path().join(LEDGER_FILE),format!("{{\"same\":{a},\"same\":{b}}}")).unwrap();
    let mut ledger = Ledger::open(root.path()).unwrap();
    assert_eq!(summary(&mut ledger,"p"),(0,0));
    assert_eq!(summary(&mut ledger,"q"),(30,1));
}

#[test]
fn replay_corrections_and_failed_batches_keep_message_and_summary_atomic() {
    let root = tempfile::tempdir().unwrap();
    let mut ledger = Ledger::open(root.path()).unwrap();
    assert_eq!(summary(&mut ledger,"p"),(0,0));
    let value = entry("p","m",110,20);
    ledger.save([("a".into(),value.clone()),("a".into(),value.clone())]).unwrap();
    ledger.save([("a".into(),entry("p","m",110,30)),("a".into(),entry("p","m",110,10))]).unwrap();
    assert_eq!(summary(&mut ledger,"p"),(30,1));
    for conflict in [entry("q","m",110,40),entry("p","other",110,40),entry("p","m",111,40)] {
        assert_eq!(ledger.save([("b".into(),value.clone()),("a".into(),conflict)]).unwrap_err(),"usage_message_identity_mismatch");
        assert_eq!(summary(&mut ledger,"p"),(30,1));
        assert_eq!(ledger.connection.query_row("SELECT COUNT(*) FROM usage_messages",[],|r|r.get::<_,i64>(0)).unwrap(),1);
    }
    ledger.connection.execute_batch("CREATE TRIGGER reject_summary BEFORE UPDATE ON usage_summary_models BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;").unwrap();
    assert!(ledger.save([("a".into(),entry("p","m",110,50))]).is_err());
    assert_eq!(summary(&mut ledger,"p"),(30,1));
    assert_eq!(ledger.connection.query_row("SELECT tokens FROM usage_messages WHERE message_id='a'",[],|r|r.get::<_,String>(0)).unwrap(),"30");
    ledger.connection.execute_batch("DROP TRIGGER reject_summary;").unwrap();
    ledger.save([("a".into(),entry("p","m",110,50))]).unwrap();
    assert_eq!(summary(&mut ledger,"p"),(50,1));
    assert_eq!(ledger.connection.query_row("PRAGMA synchronous",[],|r|r.get::<_,i64>(0)).unwrap(),2);
}

#[test]
fn concurrent_connections_preserve_maximum_tokens_and_one_request() {
    let root = tempfile::tempdir().unwrap();
    let mut ledger = Ledger::open(root.path()).unwrap();
    summary(&mut ledger,"p");
    std::thread::scope(|scope| {
        for tokens in [90,10,100,50] {
            let path = root.path();
            scope.spawn(move || Ledger::open(path).unwrap().save([("one".into(),entry("p","m",110,tokens))]).unwrap());
        }
    });
    assert_eq!(summary(&mut ledger,"p"),(100,1));
}

#[test]
fn day_boundary_timezone_change_and_top_models_match_the_raw_ledger() {
    use chrono::{FixedOffset,TimeZone};
    let root = tempfile::tempdir().unwrap();
    let mut ledger = Ledger::open(root.path()).unwrap();
    let utc = FixedOffset::east_opt(0).unwrap();
    let now = utc.with_ymd_and_hms(2026,11,10,12,0,0).unwrap();
    let (start,end) = super::super::day_window(now).unwrap();
    let observations = [
        ("before",entry("p","before",start-1,7)), ("start",entry("p","a",start,10)),
        ("middle",entry("p","b",start+36_000_000,20)), ("last",entry("p","c",end-1,30)),
        ("end",entry("p","after",end,90)), ("other",entry("q","d",start,100)),
    ];
    ledger.save(observations.iter().map(|(id,value)|(id.to_string(),value.clone()))).unwrap();
    for zone in [utc,FixedOffset::east_opt(8*3600).unwrap(),FixedOffset::east_opt(5*3600+45*60).unwrap(),FixedOffset::west_opt(5*3600).unwrap()] {
        let now = now.with_timezone(&zone);
        let (a,b) = super::super::day_window(now).unwrap();
        let result = ledger.summary("p",a,b).unwrap();
        let expected: Vec<_> = observations.iter().filter(|(_,v)|v.provider_id=="p" && zone.timestamp_millis_opt(v.created_at_ms).unwrap().date_naive()==now.date_naive()).collect();
        assert_eq!(result.tokens,expected.iter().map(|(_,v)|v.tokens).sum::<u64>());
        assert_eq!(result.requests,expected.len() as u64);
        assert!(result.top_models.len()<=3);
        assert!(result.top_models.windows(2).all(|pair|pair[0].tokens>=pair[1].tokens));
    }
    let next = ledger.summary("p",end,end+86_400_000).unwrap();
    assert_eq!((next.tokens,next.requests),(90,1));
    ledger.save([("next".into(),entry("p","after",end+1,5))]).unwrap();
    assert_eq!(ledger.summary("p",end,end+86_400_000).unwrap().tokens,95);
}

#[test]
fn day_queries_use_the_provider_time_index_and_cache_only_selected_provider() {
    let root = tempfile::tempdir().unwrap();
    let mut ledger = Ledger::open(root.path()).unwrap();
    ledger.save([("a".into(),entry("p","m",110,1)),("b".into(),entry("q","m",110,2))]).unwrap();
    assert_eq!(summary(&mut ledger,"p"),(1,1));
    let plan: String = ledger.connection.query_row("EXPLAIN QUERY PLAN SELECT model_id,tokens FROM usage_messages WHERE provider_id=?1 AND created_at_ms>=?2 AND created_at_ms<?3",params!["p",100,200],|r|r.get(3)).unwrap();
    assert!(plan.contains("idx_usage_provider_created") && !plan.contains("SCAN"),"{plan}");
    assert_eq!(ledger.connection.query_row("SELECT COUNT(*) FROM usage_summary_windows",[],|r|r.get::<_,i64>(0)).unwrap(),1);
    assert_eq!(summary(&mut ledger,"q"),(2,1));
    assert_eq!(ledger.connection.query_row("PRAGMA foreign_keys",[],|r|r.get::<_,i64>(0)).unwrap(),1);
}
