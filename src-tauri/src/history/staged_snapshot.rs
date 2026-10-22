use std::path::Path;

use tauri::Manager;

use super::{
    archive::merged_agent_snapshot,
    catalog::CatalogStore,
    catalog_model::{CatalogEntry, CatalogIdentity, Ownership, SIDECAR_ID},
    recovery::{self, SnapshotWrite},
    storage::{prepare_path, PreparedHistoryFile},
    AgentHistorySnapshot, HistorySession, HistoryState,
};

pub(crate) struct PreparedAgentSnapshot {
    baseline: Vec<HistorySession>,
    change: Option<(Vec<HistorySession>, PreparedHistoryFile)>,
    catalog: CatalogStore,
    native_key: String,
    directory: String,
    session_id: String,
    legacy_key: String,
    scoped: bool,
}

pub(crate) fn prepare_agent_snapshot(
    app: &tauri::AppHandle,
    workspace: &Path,
    snapshot: AgentHistorySnapshot<'_>,
) -> Result<PreparedAgentSnapshot, String> {
    let baseline = app
        .state::<HistoryState>()
        .0
        .lock()
        .map_err(|_| "history_state_unavailable")?
        .clone();
    let session = baseline
        .iter()
        .find(|session| session.id == snapshot.session_id)
        .ok_or("history_not_found")?;
    // Read only the trusted origin; merging, serialization and fsync run unlocked.
    let origin = session
        .origin_run_id
        .as_deref()
        .map(|id| app.state::<crate::agent::AgentRunState>().record(id))
        .transpose()?
        .flatten();
    PreparedAgentSnapshot::prepare(
        &super::history_path(app)?,
        baseline,
        origin.as_ref(),
        workspace,
        snapshot,
        super::commands::store(app)?,
    )
}

impl PreparedAgentSnapshot {
    fn prepare(
        path: &Path,
        baseline: Vec<HistorySession>,
        origin: Option<&crate::agent::RunRecord>,
        workspace: &Path,
        snapshot: AgentHistorySnapshot<'_>,
        catalog: CatalogStore,
    ) -> Result<Self, String> {
        let session = baseline
            .iter()
            .find(|session| session.id == snapshot.session_id)
            .ok_or("history_not_found")?;
        let scoped = recovery::snapshot_write(
            session,
            origin.map(std::slice::from_ref).unwrap_or_default(),
            workspace,
        )? == SnapshotWrite::Allowed;
        let directory = super::catalog_import::canonical_directory(&workspace.to_string_lossy());
        let session_id = snapshot.session_id.to_owned();
        let identity = CatalogIdentity::Native {
            sidecar_id: SIDECAR_ID.into(),
            directory: directory.clone(),
            session_id: snapshot.session_id.into(),
        };
        identity
            .validate()
            .map_err(|_| "history_identity_invalid")?;
        let native_key = identity.key();
        let legacy_key = CatalogIdentity::Legacy {
            history_id: snapshot.session_id.into(),
        }
        .key();
        let change = if scoped {
            merged_agent_snapshot(&baseline, snapshot)?
                .map(|session| {
                    let mut next = baseline.clone();
                    next.retain(|old| old.id != session.id);
                    next.push(session);
                    next.sort_by(|left, right| right.updated.cmp(&left.updated));
                    prepare_path(path, &next).map(|file| (next, file))
                })
                .transpose()?
        } else {
            None
        };
        Ok(Self {
            baseline,
            change,
            catalog,
            native_key,
            directory,
            session_id,
            legacy_key,
            scoped,
        })
    }

    /// The caller holds the run operation and sidecar generation guards.
    pub(crate) fn commit(self, app: &tauri::AppHandle) -> Result<(), String> {
        let directory = self.directory.clone();
        let session_id = self.session_id.clone();
        app.state::<crate::workbench::WorkbenchOwnership>()
            .while_not_owned(&directory, &session_id, || {
                let state = app.state::<HistoryState>();
                self.commit_to_state(&state)
            })
    }

    fn commit_to_state(self, state: &HistoryState) -> Result<(), String> {
        let mut list = state.0.try_lock().map_err(|error| match error {
            std::sync::TryLockError::WouldBlock => "history_state_busy",
            std::sync::TryLockError::Poisoned(_) => "history_state_unavailable",
        })?;
        self.commit_into(&mut list, PreparedHistoryFile::commit)
    }

    fn commit_into(
        self,
        list: &mut Vec<HistorySession>,
        persist: impl FnOnce(PreparedHistoryFile) -> Result<(), String>,
    ) -> Result<(), String> {
        // Whole-file replacement must not overwrite any concurrent row or metadata edit.
        if *list != self.baseline {
            return Err("history_snapshot_superseded".into());
        }
        if !self.scoped {
            return Ok(());
        }
        self.catalog
            .with_snapshot_entries(&self.native_key, &self.legacy_key, |native, legacy| {
                validate_catalog(native.as_ref(), legacy.as_ref())?;
                if let Some((next, file)) = self.change {
                    persist(file)?;
                    *list = next;
                }
                Ok(())
            })
    }
}

fn validate_catalog(
    native: Option<&CatalogEntry>,
    legacy: Option<&CatalogEntry>,
) -> Result<(), String> {
    if native.is_some_and(|entry| entry.tombstone.is_some())
        || legacy.is_some_and(|entry| entry.tombstone.is_some())
    {
        return Err("history_deleted".into());
    }
    if native.is_some_and(|entry| entry.ownership == Ownership::Workbench) {
        return Err("history_agent_owned".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "staged_snapshot_tests.rs"]
mod tests;
