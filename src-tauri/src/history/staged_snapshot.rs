use std::path::Path;

use tauri::Manager;

use super::{
    archive::merged_agent_snapshot,
    catalog::CatalogStore,
    catalog_model::{CatalogEntry, CatalogIdentity, Ownership, SIDECAR_ID},
    recovery::{self, SnapshotWrite},
    session_storage::{prepare, PreparedSessionFile},
    AgentHistorySnapshot, HistorySession, HistoryState,
};

pub(crate) struct PreparedAgentSnapshot {
    baseline: HistorySession,
    revision: Option<u64>,
    change: Option<(HistorySession, PreparedSessionFile)>,
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
    let state = app.state::<HistoryState>();
    let (baseline, revision) = {
        let list = state.0.lock().map_err(|_| "history_state_unavailable")?;
        let session = list.iter().find(|s| s.id == snapshot.session_id).ok_or("history_not_found")?.clone();
        (session, state.revision())
    };
    let session = &baseline;
    // Read only the trusted origin; merging, serialization and fsync run unlocked.
    let origin = session
        .origin_run_id
        .as_deref()
        .map(|id| app.state::<crate::agent::AgentRunState>().record(id))
        .transpose()?
        .flatten();
    let mut prepared = PreparedAgentSnapshot::prepare(
        &super::history_path(app)?,
        vec![baseline],
        origin.as_ref(),
        workspace,
        snapshot,
        super::commands::store(app)?,
    )?;
    prepared.revision = Some(revision);
    Ok(prepared)
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
        let self_session = session.clone();
        let change = if scoped {
            merged_agent_snapshot(&baseline, snapshot)?
                .map(|session| {
                    prepare(path, &self_session, &session).map(|file| (session, file))
                })
                .transpose()?
        } else {
            None
        };
        Ok(Self {
            baseline: session.clone(),
            revision: None,
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
        if self.revision.is_some_and(|revision| state.revision() != revision) { return Err("history_snapshot_superseded".into()); }
        let changed = self.scoped && self.change.is_some();
        self.commit_into(&mut list, PreparedSessionFile::commit)?;
        if changed { state.committed(); }
        Ok(())
    }

    fn commit_into(
        self,
        list: &mut Vec<HistorySession>,
        persist: impl FnOnce(PreparedSessionFile) -> Result<(), String>,
    ) -> Result<(), String> {
        // The selected row must still match; unrelated rows are never replaced.
        if list.iter().find(|s| s.id == self.session_id) != Some(&self.baseline) {
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
                    let current = list.iter_mut().find(|s| s.id == self.session_id).ok_or("history_not_found")?;
                    *current = next;
                    list.sort_by(|left,right| right.updated.cmp(&left.updated));
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
