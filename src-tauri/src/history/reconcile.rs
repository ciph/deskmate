use std::collections::HashMap;
use super::{catalog::CatalogStore, catalog_import::{canonical_directory, new_entry}, catalog_model::*, native_api::{NativeHistoryClient, NativeSession, NativeStatus}};

pub(crate) fn reconcile(store: &CatalogStore, client: &NativeHistoryClient, runs: &crate::agent::AgentRunState, sidecar: &crate::Sidecar) -> Result<Vec<String>, String> {
    let generation = *sidecar.generation.read().map_err(|_| "history_native_unavailable")?;
    let mut errors = Vec::new();
    let mut current_statuses = HashMap::new();
    for directory in store.directories()? {
        if crate::sidecar_residency::retired(&directory) { continue; }
        let fetched = client.list_all(&directory);
        let statuses = client.statuses(&directory);
        // Slow discovery must not own the Agent control lock. Re-read runs at commit.
        let _operation = runs.lock_operation()?;
        let _generation = sidecar.generation_guard(generation)?.ok_or("history_native_unavailable")?;
        if let Err(error) = &statuses { errors.push(error.to_string()); }
        match fetched {
            Ok(sessions) => {
                apply_discovery(store, &directory, &sessions, statuses.as_ref().ok(), &runs.all_records()?, true)?;
                if let Ok(statuses) = statuses { current_statuses.insert(directory.clone(), statuses); }
            }
            Err(error) => {
                errors.push(error.to_string());
                mark_unavailable(store, &directory)?;
            }
        }
    }
    // Tombstones commit before remote deletion; every successful refresh retries unconfirmed requests.
    let mut after = String::new();
    loop {
        let batch = store.maintenance_batch(3, &after)?;
        let Some(last) = batch.last() else { break; };
        after = last.key();
        for row in batch {
        if row.tombstone.as_ref().is_none_or(|tombstone| tombstone.remote_deleted) { continue; }
        let CatalogIdentity::Native { directory, session_id, .. } = &row.identity else { continue; };
        let Some(statuses) = current_statuses.get(directory) else { continue; };
        if matches!(statuses.get(session_id), Some(NativeStatus::Busy | NativeStatus::Retry { .. })) { continue; }
        {
            let _operation = runs.lock_operation()?;
            let _generation = sidecar.generation_guard(generation)?.ok_or("history_native_unavailable")?;
            let current = store.get(&row.key())?;
            if current.tombstone.as_ref().is_none_or(|tombstone| tombstone.remote_deleted) { continue; }
            if runs.all_records()?.iter().any(|record| record.outcome.is_none() && record.session_id.as_deref() == Some(session_id) && canonical_directory(&record.workspace_path.to_string_lossy()) == *directory) { continue; }
        }
        // The durable tombstone rejects new launches; remote retry needs no Agent lock.
        match client.delete(directory, session_id) {
            Ok(()) => {
                let _generation = sidecar.generation_guard(generation)?.ok_or("history_native_unavailable")?;
                super::catalog_mutation::confirm_deleted(store, &row.key())?;
            }
            Err(error) => errors.push(error.to_string()),
        }
    }
    }
    errors.sort(); errors.dedup(); Ok(errors)
}

pub(crate) fn apply_discovery(store: &CatalogStore, directory: &str, sessions: &[NativeSession], statuses: Option<&HashMap<String, NativeStatus>>, records: &[crate::agent::RunRecord], complete: bool) -> Result<(), String> {
    let keys = sessions.iter().filter(|session| session.parent_id.is_none() && canonical_directory(&session.directory) == directory).map(|session| {
        let identity = CatalogIdentity::Native { sidecar_id: SIDECAR_ID.into(), directory: directory.into(), session_id: session.id.clone() };
        identity.validate().map_err(|_| "history_identity_invalid")?;
        Ok(identity.key())
    }).collect::<Result<Vec<_>, String>>()?;
    let mut run_status = HashMap::new();
    for record in records.iter().filter(|record| canonical_directory(&record.workspace_path.to_string_lossy()) == directory) {
        if let Some(id) = &record.session_id { *run_status.entry(id.as_str()).or_insert(false) |= record.outcome.is_none(); }
    }
    let changed = store.update_scoped(directory, if complete { None } else { Some(&keys) }, |rows| {
        let mut changed = Vec::new();
        if complete { for row in rows.iter_mut().filter(|row| matches!(&row.identity, CatalogIdentity::Native { directory: project, .. } if project == directory)) { row.availability = Availability::Unavailable; row.runtime = RuntimeState::Unknown; } }
        let mut indexes: HashMap<_, _> = rows.iter().enumerate().map(|(index, row)| (row.key(), index)).collect();
        for session in sessions {
            if session.parent_id.is_some() || canonical_directory(&session.directory) != directory { continue; }
            let identity = CatalogIdentity::Native { sidecar_id: SIDECAR_ID.into(), directory: directory.into(), session_id: session.id.clone() };
            identity.validate().map_err(|_| "history_identity_invalid")?;
            let index = *indexes.entry(identity.key()).or_insert_with(|| {
                let index = rows.len();
                rows.push(new_entry(identity, session.title.clone(), ConversationSource::Workbench, session.time.created, session.time.updated));
                index
            });
            let row = &mut rows[index];
            if row.tombstone.is_some() { continue; }
            if row.updated != session.time.updated { changed.push(row.key()); }
            if row.has_records == Some(false) && row.updated != session.time.updated {
                row.has_records = None;
            }
            row.title = session.title.clone(); row.created = session.time.created; row.updated = session.time.updated;
            row.availability = Availability::Available;
            // The pinned native server cannot restore an archived session; user archive is catalog-owned.

            row.runtime = match statuses.and_then(|statuses| statuses.get(&session.id)) {
                Some(NativeStatus::Busy | NativeStatus::Retry { .. }) => RuntimeState::Running,
                Some(NativeStatus::Idle) => RuntimeState::Idle,
                None if statuses.is_some() => RuntimeState::Idle,
                None => RuntimeState::Unknown,
            };
            if let Some(active) = run_status.get(session.id.as_str()) { row.ownership = Ownership::Agent; if *active { row.runtime = RuntimeState::Running; } }
        }
        Ok(changed)
    })?;
    for key in changed { super::catalog_preview::invalidate(&key); }
    Ok(())
}

pub(crate) fn mark_unavailable(store: &CatalogStore, directory: &str) -> Result<(), String> {
    store.update_scoped(directory, None, |rows| { for row in rows { if matches!(&row.identity, CatalogIdentity::Native { directory: project, .. } if project == directory) { row.availability = Availability::Unavailable; row.runtime = RuntimeState::Unknown; } } Ok(()) })
}



