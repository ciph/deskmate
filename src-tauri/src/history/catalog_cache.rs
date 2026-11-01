//! Import invalidation is independent of catalog rows and live window leases.
use std::{sync::atomic::{AtomicU64, Ordering}, time::{Duration, Instant}};
use super::{catalog::CatalogStore, HistoryState, NativeSessionIndex};

#[derive(Default)]
pub(crate) struct Revision(AtomicU64);

impl Revision {
    pub(crate) fn get(&self) -> u64 { self.0.load(Ordering::Acquire) }
    pub(crate) fn committed(&self) { self.0.fetch_add(1, Ordering::Release); }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Sources {
    pub(super) history: u64,
    pub(super) index: u64,
    pub(super) runs: u64,
    pub(super) associations: u64,
}

#[derive(Default)]
pub(super) struct ImportCache {
    completed: Option<Sources>,
    reconciled_at: Option<Instant>,
}

impl ImportCache {
    pub(super) fn reconcile_due(&self, now: Instant) -> bool {
        self.reconciled_at.is_none_or(|last| now.saturating_duration_since(last) >= Duration::from_secs(30))
    }

    /// The caller serializes imports, but source writers never take this lock.
    /// A changing generation remains dirty and must receive another pass.
    pub(super) fn refresh(
        &mut self,
        now: Instant,
        revision: impl Fn() -> Sources,
        import: impl FnOnce(bool) -> Result<(), String>,
    ) -> Result<bool, String> {
        let reconcile = self.reconcile_due(now);
        let before = revision();
        if !reconcile && self.completed == Some(before) { return Ok(false); }
        // Invalidate before starting: an unsuccessful or superseded import must
        // not acknowledge either its previous or its newly observed sources.
        self.completed = None;
        import(reconcile)?;
        if revision() != before { return Ok(true); }
        self.completed = Some(before);
        if reconcile { self.reconciled_at = Some(now); }
        Ok(false)
    }
}

pub(super) struct ImportInputs<'a> {
    pub(super) history: &'a HistoryState,
    pub(super) index: &'a NativeSessionIndex,
    pub(super) runs: &'a crate::agent::AgentRunState,
    pub(super) associations: &'a Revision,
    pub(super) history_path: &'a std::path::Path,
    pub(super) index_path: &'a std::path::Path,
    pub(super) workspace: &'a str,
}

impl ImportInputs<'_> {
    pub(super) fn synchronize(
        &self, cache: &mut ImportCache, now: Instant,
        open: impl FnOnce() -> Result<CatalogStore, String>,
    ) -> Result<bool, String> {
        cache.refresh(now, || Sources {
            history: self.history.revision(), index: self.index.revision(),
            runs: self.runs.catalog_revision(), associations: self.associations.get(),
        }, |reconcile| {
            if reconcile {
                self.history.reload(self.history_path)?;
                super::native_index::load_into(self.index_path, self.index)?;
            }
            let store = open()?;
            let sessions = self.history.0.lock().map_err(|_| "history_state_unavailable")?.clone();
            let index = self.index.0.lock().map_err(|_| "native_session_index_unavailable")?.clone();
            let records = self.runs.all_records()?;
            store.register_directory(self.workspace)?;
            for row in &index { store.register_directory(&super::catalog_import::canonical_directory(&row.workspace_path))?; }
            for record in &records { store.register_directory(&super::catalog_import::canonical_directory(&record.workspace_path.to_string_lossy()))?; }
            super::catalog_import::register_sources(&store, &sessions, &index, self.workspace, &records)
        })
    }
}

#[cfg(test)]
#[path = "catalog_cache_tests.rs"]
mod tests;
