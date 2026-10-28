use tauri::{Emitter, Manager};
use super::{catalog::CatalogStore, catalog_import, catalog_model::*, catalog_query::*, native_api::{authorize_history_window, NativeHistoryClient}, HistoryState};

pub(crate) fn store<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<CatalogStore, String> {
    // WebView2 can invoke the workspace picker before the host setup has finished.
    let state = app.try_state::<super::maintenance::HistoryMaintenance>().ok_or("history_state_unavailable")?;
    Ok(CatalogStore::open(&app.path().app_data_dir().map_err(|_| "history_catalog_unavailable")?.join("history-catalog.sqlite"))?
        .track_associations(state.associations.clone()))
}
pub(crate) fn client(app: &tauri::AppHandle) -> Result<NativeHistoryClient, String> {
    NativeHistoryClient::new(&crate::sidecar_url(app), &crate::sidecar_auth_header(app)).map_err(|error| error.to_string())
}
pub(crate) fn now() -> Result<u64, String> {
    u64::try_from(chrono::Utc::now().timestamp_millis()).map_err(|_| "history_clock_invalid".into())
}
pub(crate) fn workspace(app: &tauri::AppHandle) -> Result<String, String> {
    Ok(catalog_import::canonical_directory(&app.path().app_data_dir().map_err(|_| "history_catalog_unavailable")?.join("workspace").to_string_lossy()))
}
pub(crate) fn register_known_directory(app: &tauri::AppHandle, directory: &str) -> Result<(), String> {
    let path = std::path::Path::new(directory).canonicalize().map_err(|_| "history_directory_unavailable")?;
    if !path.is_dir() { return Err("history_directory_unavailable".into()); }
    store(app)?.register_directory(&catalog_import::canonical_directory(&path.to_string_lossy()))
}

#[tauri::command]
pub(crate) fn history_model_selection_get(window: tauri::WebviewWindow, app: tauri::AppHandle, key: String) -> Result<crate::settings::ConversationModelSelection, String> {
    crate::tool_permissions::runtime::require_chat(&window)?;
    store(&app)?.model_selection(&key)
}

#[tauri::command]
pub(crate) fn history_model_selection_set(
    window: tauri::WebviewWindow, app: tauri::AppHandle, key: String,
    selection: crate::settings::ConversationModelSelection,
) -> Result<(), String> {
    crate::tool_permissions::runtime::require_chat(&window)?;
    let catalog = store(&app)?;
    let entry = catalog.get(&key)?;
    if entry.tombstone.is_some() || entry.archived || entry.runtime != RuntimeState::Idle
        || entry.availability != Availability::Available || entry.ownership == Ownership::Workbench
        || !matches!(entry.identity, CatalogIdentity::Native { .. }) {
        return Err("history_model_selection_locked".into());
    }
    if matches!(selection, crate::settings::ConversationModelSelection::Override { .. }) {
        let settings = app.state::<crate::settings::SettingsState>().0.lock().map_err(|_| "chat_model_settings_unavailable")?.clone();
        crate::settings::resolve_chat_model(&app, &settings, &selection)?;
    }
    catalog.set_model_selection(&key, &selection)
}

#[tauri::command]
pub(crate) fn history_recent_workspaces(window: tauri::WebviewWindow, app: tauri::AppHandle) -> Result<Vec<String>, String> {
    crate::tool_permissions::runtime::require_chat(&window)?;
    store(&app)?.recent_workspaces()
}

#[tauri::command]
pub(crate) fn history_remember_workspace(window: tauri::WebviewWindow, app: tauri::AppHandle, directory: String) -> Result<(), String> {
    crate::tool_permissions::runtime::require_chat(&window)?;
    let path = std::path::Path::new(&directory).canonicalize().map_err(|_| "history_directory_unavailable")?;
    if !path.is_dir() { return Err("history_directory_unavailable".into()); }
    store(&app)?.remember_workspace(&path.to_string_lossy(), now()?)
}

#[tauri::command]
pub(crate) fn history_validate_workspace(window: tauri::WebviewWindow, directory: String) -> Result<String, String> {
    crate::tool_permissions::runtime::require_chat(&window)?;
    let path = std::path::Path::new(&directory).canonicalize().map_err(|_| "history_directory_unavailable")?;
    if !path.is_dir() { return Err("history_directory_unavailable".into()); }
    crate::history::catalog_model::canonical_directory(&path.to_string_lossy()).map_err(|_| "history_directory_unavailable".into())
}

#[tauri::command]
pub(crate) fn history_catalog_native_key(
    window: tauri::WebviewWindow, app: tauri::AppHandle, directory: String, session_id: String,
) -> Result<String, String> {
    crate::tool_permissions::runtime::require_chat(&window)?;
    let identity = CatalogIdentity::Native {
        sidecar_id: SIDECAR_ID.into(),
        directory: canonical_directory(&directory).map_err(|_| "history_identity_invalid")?,
        session_id,
    };
    let key = identity.key();
    initialize(&app)?;
    store(&app)?.get(&key)?;
    Ok(key)
}
pub(crate) fn authorize_directory(app: &tauri::AppHandle, directory: Option<&str>) -> Result<String, String> {
    let default = workspace(app)?;
    let directory = directory.map(catalog_import::canonical_directory).unwrap_or_else(|| default.clone());
    if directory != default && !store(app)?.directories()?.contains(&directory) { return Err("history_directory_unknown".into()); }
    Ok(directory)
}
pub(crate) fn apply_ownership(app: &tauri::AppHandle, rows: &mut [CatalogEntry]) -> Result<(), String> {
    for row in rows {
        if let CatalogIdentity::Native { directory, session_id, .. } = &row.identity {
            if row.ownership != Ownership::Agent {
                row.ownership = if app.state::<crate::workbench::WorkbenchOwnership>().is_owned(directory, session_id)? { Ownership::Workbench } else { Ownership::Unowned };
            }
        }
    }
    Ok(())
}
pub(crate) fn initialize(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.try_state::<super::maintenance::HistoryMaintenance>().ok_or("history_state_unavailable")?;
    let history = app.try_state::<HistoryState>().ok_or("history_state_unavailable")?;
    let index = app.try_state::<super::NativeSessionIndex>().ok_or("history_state_unavailable")?;
    let runs = app.try_state::<crate::agent::AgentRunState>().ok_or("history_state_unavailable")?;
    let mut initialized = state.initialized.lock().map_err(|_| "history_state_unavailable")?;
    let retry = super::catalog_cache::ImportInputs {
        history: &history, index: &index, runs: &runs, associations: &state.associations,
        history_path: &super::history_path(app)?, index_path: &super::native_index_path(app)?,
        workspace: &workspace(app)?,
    }.synchronize(&mut initialized, std::time::Instant::now(), || store(app))?;
    drop(initialized);
    if retry { super::maintenance::schedule_refresh(app.clone()); }
    Ok(())
}

fn wait_for_host_history<R: tauri::Runtime>(app: &tauri::AppHandle<R>, budget: std::time::Duration) -> Result<(), String> {
    let deadline = std::time::Instant::now() + budget;
    // AgentRunState is registered after the history and native index are loaded.
    while app.try_state::<crate::agent::AgentRunState>().is_none() {
        if std::time::Instant::now() >= deadline { return Err("history_state_unavailable".into()); }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn history_catalog_list(window: tauri::WebviewWindow, app: tauri::AppHandle, query: CatalogQuery, refresh: Option<bool>) -> Result<CatalogPage, String> {
    authorize_history_window(window.label()).map_err(|error| error.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        wait_for_host_history(&app, std::time::Duration::from_secs(10))?;
        initialize(&app)?;
        let store = store(&app)?;
        if refresh.unwrap_or(false) { super::maintenance::schedule_refresh(app.clone()); }
        let errors = app.state::<super::maintenance::HistoryMaintenance>().errors.lock().map_err(|_| "history_state_unavailable")?.clone();
        let mut page = store.page(query, errors)?;
        page.refreshing = app.state::<super::maintenance::HistoryMaintenance>().refreshing.load(std::sync::atomic::Ordering::Acquire);
        // Workbench leases only switch Workbench/Unowned, never Agent visibility.
        // Derive current capabilities for returned rows without persisting leases.
        for row in &mut page.items {
            apply_ownership(&app, std::slice::from_mut(&mut row.entry))?;
            row.capabilities = row.entry.capabilities();
        }
        Ok(page)
    }).await.map_err(|_| "history_catalog_unavailable".to_owned())?
}

pub(crate) fn register_native(app: &tauri::AppHandle, session_id: &str, source: &str, directory: Option<&str>) -> Result<CatalogRow, String> {
    let source = match source { "light_chat" => ConversationSource::LightChat, "workbench" => ConversationSource::Workbench, _ => return Err("native_session_metadata_invalid".into()) };
    let directory = directory.map(catalog_import::canonical_directory).unwrap_or(workspace(app)?);
    let native = client(app)?.get(&directory, session_id).map_err(|error| error.to_string())?;
    if native.parent_id.is_some() { return Err("history_child_session".into()); }
    let has_records = client(app)?.has_messages(&directory, session_id).ok();
    let store = store(app)?;
    store.register_directory(&directory)?;
    let identity = CatalogIdentity::Native { sidecar_id: SIDECAR_ID.into(), directory, session_id: session_id.into() };
    let result = store.update_entry(&identity.key(), |entry| {
        if let Some(row) = entry { return Ok(row.clone()); }
        let mut row = catalog_import::new_entry(identity, native.title, source, native.time.created, native.time.updated);
        row.availability = Availability::Available;
        row.has_records = has_records;
        row.runtime = RuntimeState::Unknown;
        *entry = Some(row.clone()); Ok(row)
    })?;
    if source == ConversationSource::LightChat { super::maintenance::protect(app, "chat", &result.key())?; }
    app.emit("history-catalog-changed", ()).map_err(|_| "history_event_unavailable")?;
    Ok(result.into())
}

#[tauri::command]
pub(crate) async fn history_catalog_load(window: tauri::WebviewWindow, app: tauri::AppHandle, key: String) -> Result<CatalogLoaded, String> {
    authorize_history_window(window.label()).map_err(|error| error.to_string())?;
    let owner = window.label().to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        initialize(&app)?;
        super::maintenance::protect(&app, &owner, &key)?;
        let store = store(&app)?;
        let entry = store.get(&key)?;
        if entry.tombstone.is_some() { return Err("history_deleted".into()); }
        match &entry.identity {
            CatalogIdentity::Legacy { history_id } => {
                let session = app.state::<HistoryState>().0.lock().map_err(|_| "history_state_unavailable")?.iter().find(|session| &session.id == history_id && !session.deleted).ok_or("history_not_found")?.clone();
                let agent_details = super::catalog_details::for_entry(&entry, &app.state::<crate::agent::AgentRunState>().all_records()?, session.origin_run_id.as_deref());
                Ok(CatalogLoaded { entry: entry.into(), messages: session.messages.into_iter().map(Into::into).collect(), agent_details, origin_run_id: session.origin_run_id })
            }
            CatalogIdentity::Native { directory, session_id, .. } => {
                let client = client(&app)?;
                let native = client.get(directory, session_id).map_err(|error| error.to_string())?;
                let (snapshot, statuses) = std::thread::scope(|scope| {
                    let status = scope.spawn(|| client.statuses(directory));
                    let snapshot = crate::agent::read_scoped_snapshot(&app, std::path::Path::new(directory), session_id);
                    (snapshot, status.join().unwrap_or(Err(super::native_api::NativeApiError::Unavailable)))
                });
                super::reconcile::apply_discovery(&store, directory, &[native], statuses.as_ref().ok(), &app.state::<crate::agent::AgentRunState>().all_records()?, false)?;
                let snapshot = snapshot?;
                let profiles = app.path().app_data_dir().ok().map(|path| super::persona::cached_profiles(&path)).unwrap_or_default();
                let identities = super::persona::message_personas(&snapshot, &profiles);
                let files = super::native_files::collect(&snapshot);
                let mut messages: Vec<_> = snapshot.into_iter().flat_map(|message| {
                    let persona_id = identities.get(&message.id).cloned();
                    let role = message.role.unwrap_or_default(); let time = message.created.unwrap_or(0);
                    message.parts.into_iter().filter(|part| !part.synthetic && part.kind.as_deref() == Some("text")).filter_map(move |part| Some(super::HistoryMessage { persona_id: persona_id.clone(), local_only: false, role: role.clone(), text: part.text?, time, message_id: Some(message.id.clone()), part_id: Some(part.id) }))
                }).collect();
                super::native_files::include_file_only_messages(&mut messages, &files);
                let mut rows = vec![store.get(&key)?]; apply_ownership(&app, &mut rows)?;
                let entry = rows.remove(0);
                let agent_details = super::catalog_details::for_entry(&entry, &app.state::<crate::agent::AgentRunState>().all_records()?, None);
                let origin_run_id = super::catalog_details::native_origin(&entry, &app.state::<crate::agent::AgentRunState>().all_records()?);
                let local = app.state::<HistoryState>().0.lock().map_err(|_| "history_state_unavailable")?.clone();
                let (entry, messages) = super::catalog_local::combine_messages(entry.into(), messages, &local, &workspace(&app)?);
                Ok(CatalogLoaded { entry, messages: super::native_files::attach(messages, files), agent_details, origin_run_id })
            }
        }
    }).await.map_err(|_| "history_load_unavailable".to_owned())?
}








#[tauri::command]
pub(crate) async fn history_catalog_access(window: tauri::WebviewWindow, app: tauri::AppHandle, key: String) -> Result<CatalogRow, String> {
    authorize_history_window(window.label()).map_err(|error| error.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        let store = store(&app)?;
        let entry = store.get(&key)?;
        if entry.tombstone.is_some() { return Err("history_deleted".into()); }
        if let CatalogIdentity::Native { directory, session_id, .. } = &entry.identity {
            let client = client(&app)?;
            let native = client.get(directory, session_id).map_err(|error| error.to_string())?;
            let statuses = client.statuses(directory);
            super::reconcile::apply_discovery(&store, directory, &[native], statuses.as_ref().ok(), &app.state::<crate::agent::AgentRunState>().all_records()?, false)?;
        }
        let mut rows = vec![store.get(&key)?]; apply_ownership(&app, &mut rows)?;
        Ok(rows.remove(0).into())
    }).await.map_err(|_| "history_load_unavailable".to_owned())?
}

pub(super) fn refresh_history(app: &tauri::AppHandle) -> Result<Vec<String>, String> {
    if let Err(error) = super::maintenance::wait_for_native_directory(app) {
        let store = store(app)?;
        for directory in store.directories()? { super::reconcile::mark_unavailable(&store, &directory)?; }
        return Err(error);
    }
    let runs = app.state::<crate::agent::AgentRunState>();
    let mut errors = {
        let _operation = runs.lock_operation()?;
        super::reconcile::reconcile(&store(app)?, &client(app)?, &runs.all_records()?)?
    };
    // Preview entries retain their bounded TTL instead of flashing on every refresh.
    initialize(app)?;
    errors.extend(super::catalog_content::refresh(&store(app)?, &client(app)?)?);
    errors.extend(super::maintenance::prune_empty(app)?);
    Ok(errors)
}

pub(crate) fn refresh_after_ready(app: tauri::AppHandle) {
    super::maintenance::start_cleanup(app.clone());
    tauri::async_runtime::spawn_blocking(move || {
        let refreshed = (|| -> Result<(), String> {
            if crate::agent::managed_sidecar_ready(&app)? {
                super::maintenance::schedule_refresh(app.clone());
            } else {
                let store = store(&app)?;
                for directory in store.directories()? { super::reconcile::mark_unavailable(&store, &directory)?; }
            }
            app.emit("history-catalog-changed", ()).map_err(|_| "history_event_unavailable".to_owned())
        })();
        if let Err(error) = refreshed { eprintln!("history startup refresh: {error}"); }
    });
}

#[cfg(test)]
mod startup_tests {
    use tauri::Manager;

    #[test]
    fn early_catalog_access_returns_not_ready_instead_of_panicking() {
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        assert_eq!(super::store(app.handle()).err().as_deref(), Some("history_state_unavailable"));
    }

    #[test]
    fn early_list_waits_for_host_state_and_has_a_bounded_failure() {
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        assert_eq!(super::wait_for_host_history(app.handle(), std::time::Duration::ZERO), Err("history_state_unavailable".into()));
        let handle = app.handle().clone();
        let waiter = std::thread::spawn(move || super::wait_for_host_history(&handle, std::time::Duration::from_secs(2)));
        let root = tempfile::tempdir().unwrap();
        let runs = crate::agent::AgentRunState::load(crate::agent::RunStore::new(root.path().to_owned())).unwrap();
        app.manage(runs);
        assert_eq!(waiter.join().unwrap(), Ok(()));
    }
}
