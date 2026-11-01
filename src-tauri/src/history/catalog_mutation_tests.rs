use super::*;
use crate::history::{catalog::CatalogStore, catalog_import::new_entry};

fn fixture() -> (tempfile::TempDir, CatalogStore, CatalogEntry) {
    let root = tempfile::tempdir().unwrap();
    let store = CatalogStore::open(&root.path().join("catalog.sqlite")).unwrap();
    let mut row = new_entry(CatalogIdentity::Native { sidecar_id: SIDECAR_ID.into(), directory: "/fixture".into(), session_id: "ses_same".into() }, "Native".into(), ConversationSource::LightChat, 1, 2);
    row.availability = Availability::Available;
    row.runtime = RuntimeState::Idle;
    store.update_entry(&row.key(), |current| { *current = Some(row.clone()); Ok(()) }).unwrap();
    (root, store, row)
}

#[test]
fn mutation_revalidates_current_runtime_availability_and_tombstone() {
    for (state, error) in [(0, "history_agent_running"), (1, "history_agent_running"), (2, "history_deleted")] {
        let (_root, store, observed) = fixture();
        store.update_entry(&observed.key(), |row| {
            let row = row.as_mut().unwrap();
            match state {
                0 => { row.runtime = RuntimeState::Running; row.ownership = Ownership::Agent; }
                1 => { row.availability = Availability::Unavailable; row.runtime = RuntimeState::Unknown; }
                _ => row.tombstone = Some(DeletionTombstone { requested_at: 9, remote_deleted: false }),
            }
            Ok(())
        }).unwrap();
        let current = store.get(&observed.key()).unwrap();
        assert_eq!(commit_local(&store, &observed, &observed, &CatalogMutation::Delete { confirmed: true }, 10).unwrap_err(), error);
        assert_eq!(store.get(&observed.key()).unwrap(), current);
        if state == 1 {
            assert_eq!(commit_local(&store, &observed, &observed, &CatalogMutation::Rename { title: "Late rename".into() }, 10).unwrap_err(), "history_mutation_denied");
        }
    }
}

#[test]
fn organization_merges_with_discovery_and_live_guard_without_losing_fields() {
    let (_root, store, observed) = fixture();
    let mut guarded = observed.clone();
    guarded.ownership = Ownership::Agent;
    guarded.runtime = RuntimeState::Running;
    commit_local(&store, &observed, &guarded, &CatalogMutation::Pin { pinned: true }, 10).unwrap();
    store.update_entry(&observed.key(), |row| {
        let row = row.as_mut().unwrap();
        row.title = "Discovered title".into(); row.updated = 50;
        row.archived = true;
        Ok(())
    }).unwrap();
    // A later rename using an older observation cannot erase the live state.
    commit_local(&store, &observed, &observed, &CatalogMutation::Rename { title: " User title ".into() }, 11).unwrap();
    let current = store.get(&observed.key()).unwrap();
    assert!(current.pinned && current.archived);
    assert_eq!((current.runtime, current.ownership), (RuntimeState::Running, Ownership::Agent));
    assert_eq!(current.user_title.as_deref(), Some("User title"));
    assert_eq!((current.title.as_str(), current.updated), ("Discovered title", 50));
}

#[test]
fn newer_activity_timestamp_keeps_running_evidence_even_when_status_value_matches_baseline() {
    let (_root, store, mut observed) = fixture();
    observed.runtime = RuntimeState::Running;
    store.update_entry(&observed.key(), |row| { *row = Some(observed.clone()); Ok(()) }).unwrap();
    let mut guarded = observed.clone();
    guarded.runtime = RuntimeState::Idle;
    // The status request saw idle, then discovery reported newer activity that
    // has the same runtime enum as the original snapshot.
    store.update_entry(&observed.key(), |row| { row.as_mut().unwrap().updated = 50; Ok(()) }).unwrap();
    assert_eq!(commit_local(&store, &observed, &guarded, &CatalogMutation::Delete { confirmed: true }, 10).unwrap_err(), "history_agent_running");
    let current = store.get(&observed.key()).unwrap();
    assert_eq!(current.runtime, RuntimeState::Running);
    assert!(current.tombstone.is_none());
}

#[test]
fn delete_requires_confirmation_and_current_native_status_then_receipt_is_idempotent() {
    let (_root, store, mut observed) = fixture();
    store.update_entry(&observed.key(), |row| { row.as_mut().unwrap().runtime = RuntimeState::Unknown; Ok(()) }).unwrap();
    observed.runtime = RuntimeState::Unknown;
    let mut guarded = observed.clone();
    guarded.runtime = RuntimeState::Idle;
    assert_eq!(commit_local(&store, &observed, &guarded, &CatalogMutation::Delete { confirmed: false }, 10).unwrap_err(), "history_delete_confirmation_required");
    assert_eq!(store.get(&observed.key()).unwrap().runtime, RuntimeState::Unknown);
    commit_local(&store, &observed, &guarded, &CatalogMutation::Delete { confirmed: true }, 10).unwrap();
    let tombstoned = store.get(&observed.key()).unwrap();
    assert!(!tombstoned.tombstone.unwrap().remote_deleted);
    assert_eq!(commit_local(&store, &observed, &guarded, &CatalogMutation::Pin { pinned: true }, 11).unwrap_err(), "history_deleted");
    for _ in 0..2 { confirm_deleted(&store, &observed.key()).unwrap(); }
    assert_eq!(store.get(&observed.key()).unwrap().tombstone, Some(DeletionTombstone { requested_at: 10, remote_deleted: true }));
    store.update_entry(&observed.key(), |row| { *row = None; Ok(()) }).unwrap();
    confirm_deleted(&store, &observed.key()).unwrap();
    assert_eq!(commit_local(&store, &observed, &guarded, &CatalogMutation::Archive { archived: true }, 12).unwrap_err(), "history_not_found");
    assert!(store.all().unwrap().is_empty());
}

#[test]
fn deletion_receipt_never_creates_a_tombstone_or_changes_another_directory() {
    let (_root, store, row) = fixture();
    let mut other = row.clone();
    if let CatalogIdentity::Native { directory, .. } = &mut other.identity { *directory = "/other".into(); }
    other.tombstone = Some(DeletionTombstone { requested_at: 4, remote_deleted: false });
    store.update_entry(&other.key(), |current| { *current = Some(other.clone()); Ok(()) }).unwrap();
    confirm_deleted(&store, &row.key()).unwrap();
    assert_eq!(store.get(&row.key()).unwrap(), row);
    assert_eq!(store.get(&other.key()).unwrap(), other);
}
