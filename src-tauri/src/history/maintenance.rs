//! Cached history is readable while native discovery and unused-draft cleanup run.
use std::{collections::{HashMap, HashSet}, sync::{atomic::{AtomicBool, AtomicU64, Ordering}, Mutex}};
use tauri::{Emitter, Manager};
use super::{catalog_model::*, commands, HistoryState};

// A fresh native project may install plugin dependencies even after global health is ready.
// This budget applies only to background bootstrap, never interactive history reads.
const PROJECT_BOOTSTRAP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

#[derive(Default)]
pub(crate) struct HistoryMaintenance {
    pub(super) initialized: Mutex<super::catalog_cache::ImportCache>,
    pub(super) associations: std::sync::Arc<super::catalog_cache::Revision>,
    pub(super) errors: Mutex<Vec<String>>,
    pub(super) refreshing: AtomicBool,
    native_ready_generation: Mutex<Option<u64>>,
    refresh_requested: AtomicBool,
    cleanup_started: AtomicBool,
    chat_selection: AtomicU64,
    // Keep drafts protected even while their native window is hidden.
    pub(crate) used: Mutex<HashMap<String, HashSet<String>>>,
}

pub(super) fn wait_for_native_directory(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<HistoryMaintenance>();
    let generation = crate::sidecar_generation(app)?;
    if *state.native_ready_generation.lock().map_err(|_| "history_state_unavailable")? == Some(generation) { return Ok(()); }
    // Background-only bootstrap, outside the Agent operation lock; ordinary reads keep their short deadlines.
    if !crate::agent::managed_sidecar_ready(app)? { return Err("history_native_unavailable".into()); }
    let generation = crate::sidecar_generation(app)?;
    commands::client(app)?.wait_for_directory(&commands::workspace(app)?, PROJECT_BOOTSTRAP_TIMEOUT)
        .map_err(|error| error.to_string())?;
    let sidecar = app.state::<crate::Sidecar>();
    let _generation = sidecar.generation_guard(generation)?.ok_or("history_native_unavailable")?;
    *state.native_ready_generation.lock().map_err(|_| "history_state_unavailable")? = Some(generation);
    Ok(())
}

pub(crate) fn protect(app: &tauri::AppHandle, owner: &str, key: &str) -> Result<(), String> {
    let entry = commands::store(app)?.get(key)?;
    if entry.tombstone.is_some() { return Err("history_deleted".into()); }
    // Content evidence never regresses; transcript reads need not wait on draft cleanup.
    if entry.has_records == Some(true) { return Ok(()); }
    let state = app.state::<HistoryMaintenance>();
    let mut used = state.used.lock().map_err(|_| "history_state_unavailable")?;
    let entry = commands::store(app)?.get(key)?;
    if entry.tombstone.is_some() { return Err("history_deleted".into()); }
    let owner = if owner == "chat" { "chat:pending" } else { owner };
    used.entry(owner.into()).or_default().insert(key.into());
    Ok(())
}

#[tauri::command]
pub(crate) async fn history_chat_selected(window: tauri::WebviewWindow, app: tauri::AppHandle, key: String) -> Result<(), String> {
    crate::tool_permissions::runtime::require_chat(&window)?;
    let sequence = app.state::<HistoryMaintenance>().chat_selection.fetch_add(1, Ordering::AcqRel) + 1;
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<HistoryMaintenance>();
        let mut used = state.used.lock().map_err(|_| "history_state_unavailable")?;
        if state.chat_selection.load(Ordering::Acquire) != sequence { return Ok(()); }
        let entry = commands::store(&app)?.get(&key)?;
        if entry.tombstone.is_some() { return Err("history_deleted".into()); }
        if let Some(pending) = used.get_mut("chat:pending") { pending.remove(&key); }
        used.insert("chat".into(), HashSet::from([key]));
        Ok(())
    }).await.map_err(|_| "history_state_unavailable".to_owned())?
}

pub(crate) fn select_workbench(app: &tauri::AppHandle, directory: &str, session_id: &str) -> Result<(), String> {
    let key = CatalogIdentity::Native { sidecar_id: SIDECAR_ID.into(), directory: directory.into(), session_id: session_id.into() }.key();
    let state = app.state::<HistoryMaintenance>();
    let mut used = state.used.lock().map_err(|_| "history_state_unavailable")?;
    // A workbench-created session may not have reached the catalog yet.
    commands::client(app)?.get(directory, session_id).map_err(|error| error.to_string())?;
    used.insert("workbench".into(), HashSet::from([key]));
    Ok(())
}

pub(crate) fn schedule_refresh(app: tauri::AppHandle) {
    let state = app.state::<HistoryMaintenance>();
    state.refresh_requested.store(true, Ordering::Release);
    if state.refreshing.swap(true, Ordering::AcqRel) { return; }
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<HistoryMaintenance>();
        while state.refresh_requested.swap(false, Ordering::AcqRel) {
            let result = commands::refresh_history(&app);
            if let Ok(mut errors) = state.errors.lock() { *errors = result.unwrap_or_else(|error| vec![error]); }
            let _ = app.emit("history-catalog-changed", ());
        }
        state.refreshing.store(false, Ordering::Release);
        let _ = app.emit("history-catalog-changed", ());
        // A request arriving between the last check and release still gets a pass.
        if state.refresh_requested.load(Ordering::Acquire) { schedule_refresh(app.clone()); }
    });
}

pub(crate) fn start_cleanup(app: tauri::AppHandle) {
    if app.state::<HistoryMaintenance>().cleanup_started.swap(true, Ordering::AcqRel) { return; }
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(30));
        if !app.state::<HistoryMaintenance>().refreshing.load(Ordering::Acquire)
            && crate::agent::managed_sidecar_ready(&app).unwrap_or(false) {
            let _ = prune_empty(&app);
        }
    });
}

fn eligible(row: &CatalogEntry, used: &HashMap<String, HashSet<String>>, now: u64) -> bool {
    row.tombstone.is_none() && row.has_records == Some(false)
        && row.availability == Availability::Available && row.runtime == RuntimeState::Idle
        && row.ownership == Ownership::Unowned
        && match &row.identity { CatalogIdentity::Native { sidecar_id, .. } => sidecar_id == SIDECAR_ID, CatalogIdentity::Legacy { .. } => true }
        // Registration, route claims and resource staging get time to finish.
        && now.saturating_sub(row.created.max(row.updated)) >= 30_000
        && !used.values().any(|keys| keys.contains(&row.key()))
}

pub(super) fn prune_empty(app: &tauri::AppHandle) -> Result<Vec<String>, String> {
    let store = commands::store(app)?;
    let client = commands::client(app)?;
    let state = app.state::<HistoryMaintenance>();
    let runs = app.state::<crate::agent::AgentRunState>();
    let mut changed = false;
    let mut errors = Vec::new();
    for candidate in store.all()?.into_iter().filter(|row| row.has_records == Some(false) && row.tombstone.is_none()) {
        // Same lock order as launch/save. Failed checks retain the record.
        let _operation = runs.lock_operation()?;
        let used = state.used.lock().map_err(|_| "history_state_unavailable")?;
        let mut row = store.get(&candidate.key())?;
        if !eligible(&row, &used, commands::now()?) { continue; }
        if let CatalogIdentity::Legacy { history_id } = &row.identity {
            if runs.all_records()?.iter().any(|run| run.session_id.as_ref() == Some(history_id)) { continue; }
            let local = app.state::<HistoryState>();
            let mut sessions = local.0.lock().map_err(|_| "history_state_unavailable")?;
            if delete_empty_legacy(&super::history_path(app)?, &mut sessions, history_id, row.updated)? {
                local.committed();
                mark_removed(&store, &row)?;
                changed = true;
            }
            continue;
        }
        let CatalogIdentity::Native { directory, session_id, .. } = &row.identity else { continue; };
        // The workbench routes and sends directly through the native SDK. Do not
        // sweep while it is visible (including the interval before route claim).
        if app.state::<crate::workbench::WorkbenchOwnership>().host_visible()? { continue; }
        if app.state::<crate::workbench::WorkbenchOwnership>().is_owned(directory, session_id)? { continue; }
        if runs.all_records()?.iter().any(|run| run.session_id.as_deref() == Some(session_id)
            && super::catalog_import::canonical_directory(&run.workspace_path.to_string_lossy()) == *directory) { continue; }
        let local = app.state::<HistoryState>().0.lock().map_err(|_| "history_state_unavailable")?.clone();
        super::catalog_content::apply_local_evidence(std::slice::from_mut(&mut row), &local, &commands::workspace(app)?);
        if row.has_records != Some(false) { continue; }
        let CatalogIdentity::Native { directory, session_id, .. } = &row.identity else { continue; };
        match client.delete_empty(directory, session_id, row.updated) {
            Ok(true) => {
                mark_removed(&store, &row)?;
                if let Some(resources) = app.try_state::<crate::chat_attachments::resources::ResourceStore>() {
                    if let Err(error) = resources.revoke_session(directory, session_id) { errors.push(error); }
                }
                changed = true;
            }
            Ok(false) => {},
            Err(error) => { errors.push(error.to_string()); break; }
        }
    }
    if changed { let _ = app.emit("history-catalog-changed", ()); }
    Ok(errors)
}

fn mark_removed(store: &super::catalog::CatalogStore, row: &CatalogEntry) -> Result<(), String> {
    store.update_entry(&row.key(), |entry| {
        if let Some(current) = entry {
            if let Some(tombstone) = &mut current.tombstone { tombstone.remote_deleted = true; }
            else { current.tombstone = Some(DeletionTombstone { requested_at: commands::now()?, remote_deleted: true }); }
        }
        Ok(())
    })?;
    super::catalog_preview::invalidate(&row.key());
    Ok(())
}

fn delete_empty_legacy(path: &std::path::Path, sessions: &mut Vec<super::HistorySession>, id: &str, updated: u64) -> Result<bool, String> {
    // The caller holds HistoryState through verification and durable removal.
    if !sessions.iter().any(|session| session.id == id && !session.deleted && session.updated == updated
        && session.messages.is_empty() && session.origin_run_id.is_none() && session.local_link.is_none()) { return Ok(false); }
    super::delete_from_path(path, sessions, super::DeleteHistory { id, trusted_origin_run_id: None, active: false })?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_cleanup_requires_unchanged_empty_unlinked_local_record() {
        let root = std::env::temp_dir().join(format!("yume-empty-legacy-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("history.json");
        let base = super::super::HistorySession { id: "legacy_one".into(), title: "Draft".into(), created: 1, updated: 2, messages: vec![], origin_run_id: None, local_link: None, deleted: false };
        for mutate in [|s: &mut super::super::HistorySession| s.updated = 3,
            |s: &mut super::super::HistorySession| s.origin_run_id = Some("run_one".into()),
            |s: &mut super::super::HistorySession| s.local_link = Some(row().identity),
            |s: &mut super::super::HistorySession| s.messages.push(super::super::HistoryMessage { role: "user".into(), text: String::new(), time: 1, message_id: None, part_id: None, persona_id: None, local_only: false })] {
            let mut session = base.clone(); mutate(&mut session);
            let mut sessions = vec![session];
            assert!(!delete_empty_legacy(&path, &mut sessions, "legacy_one", 2).unwrap());
            assert_eq!(sessions.len(), 1);
        }
        let mut sessions = vec![base];
        assert!(delete_empty_legacy(&path, &mut sessions, "legacy_one", 2).unwrap());
        assert!(sessions.is_empty());
        assert!(super::super::load_path(&path).unwrap().is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
    fn row() -> CatalogEntry {
        let mut row = super::super::catalog_import::new_entry(CatalogIdentity::Native { sidecar_id: SIDECAR_ID.into(), directory: "c:/a".into(), session_id: "ses_old".into() }, "Draft".into(), ConversationSource::LightChat, 1, 2);
        row.has_records = Some(false); row.runtime = RuntimeState::Idle; row.availability = Availability::Available; row
    }
    #[test]
    fn cleanup_receipt_preserves_current_organization_and_existing_deletion_time() {
        let root = tempfile::tempdir().unwrap();
        let store = super::super::catalog::CatalogStore::open(&root.path().join("catalog.sqlite")).unwrap();
        let snapshot = row();
        let mut current = snapshot.clone();
        current.pinned = true;
        current.tombstone = Some(DeletionTombstone { requested_at: 7, remote_deleted: false });
        let mut other = snapshot.clone();
        if let CatalogIdentity::Native { directory, .. } = &mut other.identity { *directory = "c:/b".into(); }
        store.update(|rows| { rows.extend([current.clone(), other.clone()]); Ok(()) }).unwrap();
        mark_removed(&store, &snapshot).unwrap();
        current.tombstone.as_mut().unwrap().remote_deleted = true;
        assert_eq!(store.get(&snapshot.key()).unwrap(), current);
        assert_eq!(store.get(&other.key()).unwrap(), other);
        mark_removed(&store, &other).unwrap();
        assert!(store.get(&other.key()).unwrap().tombstone.unwrap().remote_deleted);
        store.update_entry(&snapshot.key(), |row| { *row = None; Ok(()) }).unwrap();
        mark_removed(&store, &snapshot).unwrap();
        assert_eq!(store.get(&snapshot.key()).unwrap_err(), "history_not_found");
    }
    #[test]
    fn cleanup_excludes_live_unknown_recent_and_used_scopes() {
        let base = row(); let mut used = HashMap::new();
        assert!(eligible(&base, &used, 40_000));
        assert!(!eligible(&base, &used, 20_000));
        for mutate in [|r: &mut CatalogEntry| r.has_records = None, |r: &mut CatalogEntry| r.has_records = Some(true),
            |r: &mut CatalogEntry| r.runtime = RuntimeState::Running, |r: &mut CatalogEntry| r.runtime = RuntimeState::Unknown,
            |r: &mut CatalogEntry| r.availability = Availability::Unavailable, |r: &mut CatalogEntry| r.ownership = Ownership::Agent] {
            let mut other = base.clone(); mutate(&mut other); assert!(!eligible(&other, &used, 40_000));
        }
        used.insert("chat".into(), HashSet::from([base.key()]));
        assert!(!eligible(&base, &used, 40_000));
        let mut other = base; if let CatalogIdentity::Native { directory, .. } = &mut other.identity { *directory = "c:/b".into(); }
        assert!(eligible(&other, &used, 40_000));
    }
}
