use super::*;
use crate::{agent::{AgentRunState, RunStore}, history::{catalog_model::*, history_tests::ordinary}};
use std::{cell::Cell, sync::{Arc, Mutex}};

struct Fixture {
    root: tempfile::TempDir,
    history: HistoryState,
    index: NativeSessionIndex,
    store: RunStore,
    runs: AgentRunState,
    associations: Arc<Revision>,
    opens: Cell<usize>,
}

impl Fixture {
    fn new(body: usize) -> Self {
        let root = tempfile::tempdir().unwrap();
        let mut session = ordinary("legacy");
        session.messages[0].text = "文".repeat(body);
        super::super::storage::persist_path(&root.path().join("history.json"), &[session.clone()]).unwrap();
        let store = RunStore::new(root.path().join("runs"));
        let runs = AgentRunState::new(store.clone());
        runs.begin("msg_cache", root.path(), "fixture").unwrap();
        Self { root, history: HistoryState::new(Mutex::new(vec![session])), index: NativeSessionIndex::default(),
            store, runs, associations: Arc::default(), opens: Cell::new(0) }
    }
    fn catalog(&self) -> CatalogStore {
        CatalogStore::open(&self.root.path().join("catalog.sqlite")).unwrap().track_associations(self.associations.clone())
    }
    fn sync(&self, cache: &mut ImportCache, now: Instant) -> Result<bool, String> {
        ImportInputs { history: &self.history, index: &self.index, runs: &self.runs, associations: &self.associations,
            history_path: &self.root.path().join("history.json"), index_path: &self.root.path().join("index.json"),
            workspace: self.root.path().to_str().unwrap(),
        }.synchronize(cache, now, || { self.opens.set(self.opens.get() + 1); Ok(self.catalog()) })
    }
}

#[test]
fn warm_import_does_no_source_reads_or_catalog_opens_when_bodies_grow_tenfold() {
    for body in [10_000, 100_000] {
        let fixture = Fixture::new(body);
        let now = Instant::now();
        let mut cache = ImportCache::default();
        assert!(!fixture.sync(&mut cache, now).unwrap());
        let io = fixture.store.io_counts();
        let opens = fixture.opens.get();
        // Corrupt external bytes: a warm import cannot read, decode or hash them.
        std::fs::write(fixture.root.path().join("history.json"), b"external partial write").unwrap();
        for _ in 0..100 {
            assert!(!fixture.sync(&mut cache, now + Duration::from_secs(1)).unwrap());
        }
        assert_eq!(fixture.store.io_counts(), io);
        assert_eq!(fixture.opens.get(), opens);
        println!("H18a body_characters={body} warm_reads=100 run_file_reads=0 catalog_imports=0");
        assert!(fixture.sync(&mut cache, now + Duration::from_secs(30)).is_err());
        assert_eq!(fixture.history.0.lock().unwrap()[0].messages[0].text, "文".repeat(body));
    }
}

#[test]
fn failed_import_is_retried_and_concurrent_source_commit_is_never_acknowledged() {
    let revision = Revision::default();
    let current = || Sources { history: revision.get(), index: 0, runs: 0, associations: 0 };
    let now = Instant::now();
    let mut cache = ImportCache::default();
    assert!(cache.refresh(now, current, |_| Err("disk_full".into())).is_err());
    let entered = std::sync::Barrier::new(2);
    std::thread::scope(|scope| {
        scope.spawn(|| { entered.wait(); revision.committed(); entered.wait(); });
        assert!(cache.refresh(now, current, |_| { entered.wait(); entered.wait(); Ok(()) }).unwrap());
    });
    assert!(cache.completed.is_none());
    assert!(!cache.refresh(now, current, |_| Ok(())).unwrap());
    assert_eq!(cache.completed, Some(current()));
    assert!(!cache.refresh(now, current, |_| panic!("unchanged source was imported")).unwrap());
}

#[test]
fn recovery_reloads_external_history_and_index_and_survives_failed_import_and_restart() {
    let fixture = Fixture::new(10);
    let now = Instant::now();
    let mut cache = ImportCache::default();
    fixture.sync(&mut cache, now).unwrap();
    let mut external = ordinary("external");
    external.title = "External write".into();
    super::super::storage::persist_path(&fixture.root.path().join("history.json"), &[external]).unwrap();
    std::fs::write(fixture.root.path().join("index.json"), serde_json::to_vec(&vec![
        super::super::native_index::NativeSessionMetadata { stable_id: "ses_external".into(), session_id: "ses_external".into(),
            persona_id: "fixture".into(), workspace_path: fixture.root.path().to_string_lossy().into(),
            source: "light_chat".into(), created_at: 1, updated_at: 2 }
    ]).unwrap()).unwrap();
    assert!(fixture.sync(&mut cache, now + Duration::from_secs(30)).unwrap());
    assert!(!fixture.sync(&mut cache, now + Duration::from_secs(30)).unwrap());
    assert_eq!(fixture.catalog().get("legacy:external").unwrap().title, "External write");
    assert_eq!(fixture.history.revision(), 1);
    assert_eq!(fixture.index.revision(), 1);
    let mut restarted = ImportCache::default();
    assert!(!fixture.sync(&mut restarted, now + Duration::from_secs(31)).unwrap());
    assert_eq!(fixture.catalog().get("legacy:external").unwrap().title, "External write");
}

#[test]
fn catalog_associations_advance_only_on_success_while_mutations_and_tombstones_remain_live() {
    let fixture = Fixture::new(10);
    let now = Instant::now();
    let mut cache = ImportCache::default();
    fixture.sync(&mut cache, now).unwrap();
    let catalog = fixture.catalog();
    let row = super::super::catalog_import::new_entry(CatalogIdentity::Native { sidecar_id: SIDECAR_ID.into(),
        directory: super::super::catalog_import::canonical_directory(&fixture.root.path().to_string_lossy()),
        session_id: "legacy".into() }, "Native".into(), ConversationSource::LightChat, 1, 2);
    let failed: Result<(), String> = catalog.update(|rows| { rows.push(row.clone()); Err("rollback".into()) });
    assert!(failed.is_err());
    assert_eq!(fixture.associations.get(), 0);
    catalog.update(|rows| { rows.push(row.clone()); Ok(()) }).unwrap();
    assert_eq!(fixture.associations.get(), 1);
    fixture.sync(&mut cache, now).unwrap();
    assert!(catalog.get("legacy:legacy").is_err());
    assert_eq!(catalog.get(&row.key()).unwrap().has_records, Some(true));
    let opens = fixture.opens.get();
    catalog.update(|rows| { let row = rows.iter_mut().find(|entry| entry.key() == row.key()).unwrap();
        row.pinned = true; row.user_title = Some("User title".into());
        row.tombstone = Some(DeletionTombstone { requested_at: 3, remote_deleted: true }); Ok(()) }).unwrap();
    assert!(!fixture.sync(&mut cache, now).unwrap());
    assert_eq!(fixture.opens.get(), opens);
    assert!(catalog.get(&row.key()).unwrap().tombstone.is_some());
    fixture.sync(&mut cache, now + Duration::from_secs(30)).unwrap();
    let stored = catalog.get(&row.key()).unwrap();
    assert!(stored.pinned && stored.tombstone.is_some());
    assert_eq!(stored.user_title.as_deref(), Some("User title"));
}
