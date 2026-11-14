use super::{catalog::CatalogStore,catalog_import::new_entry,catalog_model::*,native_api::NativeSession};

fn row(id: &str, directory: &str) -> CatalogEntry {
    let mut row = new_entry(CatalogIdentity::Native { sidecar_id:SIDECAR_ID.into(),directory:directory.into(),session_id:id.into() },id.into(),ConversationSource::Workbench,1,2);
    row.has_records=Some(true);
    row
}
fn native(id: &str, directory: &str, updated: u64) -> NativeSession {
    serde_json::from_value(serde_json::json!({"id":id,"directory":directory,"title":"updated","time":{"created":1,"updated":updated}})).unwrap()
}

#[test]
fn discovery_decodes_only_selected_identity_or_complete_directory() {
    let root=tempfile::tempdir().unwrap();
    let store=CatalogStore::open(&root.path().join("catalog.sqlite")).unwrap();
    let target=row("ses_target","/target");
    store.update(|rows| { rows.push(target.clone()); for i in 0..1000 { rows.push(row(&format!("ses_{i}"),"/unrelated")); } Ok(()) }).unwrap();
    super::catalog::DECODED_ROWS.with(|count|count.set(0));
    super::reconcile::apply_discovery(&store,"/target",&[native("ses_target","/target",3)],Some(&Default::default()),&[],false).unwrap();
    let count=super::catalog::DECODED_ROWS.with(|count|count.get());
    assert_eq!(count,1,"single discovery decoded {count} rows");
    println!("HISTORY_SCALE unrelated_rows=1000 single_discovery_decoded={count}");
    assert_eq!(store.get(&target.key()).unwrap().updated,3);
    super::catalog::DECODED_ROWS.with(|count|count.set(0));
    super::reconcile::apply_discovery(&store,"/target",&[],Some(&Default::default()),&[],true).unwrap();
    let count=super::catalog::DECODED_ROWS.with(|count|count.get());
    assert_eq!(count,1,"directory discovery decoded {count} rows");
    assert_eq!(store.get(&target.key()).unwrap().availability,Availability::Unavailable);
    assert_eq!(store.get(&row("ses_0","/unrelated").key()).unwrap().availability,Availability::Stale);
}

#[test]
fn maintenance_keysets_visit_only_candidates_once_and_reject_dirty_projection() {
    let root=tempfile::tempdir().unwrap();
    let store=CatalogStore::open(&root.path().join("catalog.sqlite")).unwrap();
    store.update(|rows| {
        for i in 0..2000 { rows.push(row(&format!("ses_full_{i:04}"),"/other")); }
        for i in 0..257 { let mut r=row(&format!("ses_empty_{i:04}"),"/target"); r.availability=Availability::Available; r.runtime=RuntimeState::Idle; r.has_records=Some(false); rows.push(r); }
        Ok(())
    }).unwrap();
    super::catalog::DECODED_ROWS.with(|count|count.set(0));
    let mut after=String::new(); let mut keys=std::collections::HashSet::new();
    loop {
        let batch=store.maintenance_batch(1,&after).unwrap();
        assert!(batch.len()<=128);
        if batch.is_empty() { break; }
        after=batch.last().unwrap().key();
        for r in batch { assert!(keys.insert(r.key())); }
    }
    assert_eq!(keys.len(),257);
    assert_eq!(super::catalog::DECODED_ROWS.with(|count|count.get()),257);
    println!("HISTORY_SCALE catalog_rows=2257 maintenance_candidates_decoded=257 batch_limit=128");
    let conn=store.connection().unwrap();
    let plan:String=conn.query_row("EXPLAIN QUERY PLAN SELECT key FROM entry_projection WHERE maintenance=1 AND key>'x' ORDER BY key LIMIT 128",[],|r|r.get(3)).unwrap();
    assert!(plan.contains("catalog_maintenance"),"{plan}");
    conn.execute("INSERT INTO entries VALUES('legacy:invalid','broken')",[]).unwrap();
    assert!(store.maintenance_batch(1,"").is_err());
    assert!(super::reconcile::mark_unavailable(&store,"/target").is_err());
}

#[test]
fn session_migration_preserves_backup_and_targeted_commit_preserves_unrelated_edits() {
    use super::{history_tests::ordinary,storage,session_storage};
    let root=tempfile::tempdir().unwrap(); let path=root.path().join("history.json");
    let mut sessions:Vec<_>=(0..200).map(|i| { let mut s=ordinary(&format!("session_{i:04}"));s.messages[0].text="文".repeat(10_000); s }).collect();
    storage::persist_path(&path,&sessions).unwrap();
    let backup=std::fs::read(&path).unwrap();
    session_storage::ensure(&path).unwrap();
    assert_eq!(storage::load_path(&path).unwrap(),sessions);
    let before=sessions[0].clone(); let mut changed=before.clone(); changed.messages[0].text="new reply".into();changed.updated=2;
    let staged=session_storage::prepare(&path,&before,&changed).unwrap();
    sessions[1].title="concurrent unrelated title".into();
    storage::persist_path(&path,&sessions).unwrap();
    let conn=rusqlite::Connection::open(session_storage::path(&path)).unwrap();
    conn.execute_batch("CREATE TABLE writes(id TEXT); CREATE TRIGGER count_updates AFTER UPDATE ON sessions BEGIN INSERT INTO writes VALUES(NEW.id); END;").unwrap();
    staged.commit().unwrap();
    let loaded=storage::load_path(&path).unwrap();
    assert_eq!(loaded.iter().find(|s|s.id==changed.id).unwrap(),&changed);
    assert_eq!(loaded.iter().find(|s|s.id==sessions[1].id).unwrap(),&sessions[1]);
    let writes:u32=conn.query_row("SELECT count(*) FROM writes",[],|r|r.get(0)).unwrap();assert_eq!(writes,1);
    assert_eq!(std::fs::read(&path).unwrap(),backup);
    println!("HISTORY_SCALE sessions=200 legacy_bytes={} snapshot_rows_written=1 unrelated_rows_written=0",backup.len());
    let future=session_storage::prepare(&path,&changed,&before).unwrap();
    conn.pragma_update(None,"user_version",2).unwrap();
    assert_eq!(future.commit(),Err("history_version_unsupported".into()));
    assert_eq!(storage::load_path(&path),Err("history_version_unsupported".into()));
    conn.pragma_update(None,"user_version",1).unwrap();
    let stale=session_storage::prepare(&path,&changed,&before).unwrap();
    conn.execute("DELETE FROM sessions WHERE id=?1",[&changed.id]).unwrap();
    assert_eq!(stale.commit(),Err("history_snapshot_superseded".into()));
    assert!(session_storage::prepare(&path,&changed,&before).is_err());
}

#[test]
fn invalid_legacy_import_rolls_back_and_can_retry_without_touching_backup() {
    use super::{history_tests::ordinary,storage,session_storage};
    let root=tempfile::tempdir().unwrap();let path=root.path().join("history.json");
    let session=ordinary("duplicate");
    std::fs::write(&path,serde_json::to_vec(&vec![session.clone(),session.clone()]).unwrap()).unwrap();
    let bytes=std::fs::read(&path).unwrap();
    assert!(session_storage::ensure(&path).is_err());
    assert!(!session_storage::ready(&path).unwrap());
    assert_eq!(std::fs::read(&path).unwrap(),bytes);
    let trailing=format!("{} trailing",serde_json::to_string(&vec![session.clone()]).unwrap());
    std::fs::write(&path,&trailing).unwrap();
    assert!(session_storage::ensure(&path).is_err());
    assert!(!session_storage::ready(&path).unwrap());
    assert_eq!(std::fs::read_to_string(&path).unwrap(),trailing);
    storage::persist_path(&path,std::slice::from_ref(&session)).unwrap();
    session_storage::ensure(&path).unwrap();
    assert_eq!(storage::load_path(&path).unwrap(),vec![session]);
}
