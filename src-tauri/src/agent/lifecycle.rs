use super::record_store::{NativeMessage, RunOutcome, RunRecord, RunStore};
use std::{
    collections::BTreeSet,
    path::Path,
    sync::{Mutex, MutexGuard, TryLockError},
};

pub(super) struct RunData {
    pub(super) active: Option<RunRecord>,
    pub(super) recent: Vec<RunRecord>,
    pub(super) artifacts: Vec<super::artifacts::AgentArtifact>,
}

pub(crate) struct AgentRunState {
    pub(super) data: Mutex<RunData>,
    pub(super) store: RunStore,
    operation: Mutex<()>,
    collection: Mutex<()>,
    pub(super) usage_retries: Mutex<super::usage_retry::UsageRetries>,
    archive_failure: Mutex<Option<(String, String)>>,
    pub(super) collection_failure: Mutex<Option<(String, std::time::Instant)>>,
    catalog_revision: crate::history::catalog_cache::Revision,
}

impl AgentRunState {
    #[cfg(test)]
    pub(crate) fn new(store: RunStore) -> Self {
        Self {
            data: Mutex::new(RunData {
                active: None,
                recent: Vec::new(),
                artifacts: Vec::new(),
            }),
            store,
            operation: Mutex::new(()),
            collection: Mutex::new(()),
            usage_retries: Mutex::new(super::usage_retry::UsageRetries::default()),
            archive_failure: Mutex::new(None),
            collection_failure: Mutex::new(None),
            catalog_revision: crate::history::catalog_cache::Revision::default(),
        }
    }

    pub(crate) fn load(store: RunStore) -> Result<Self, String> {
        let records = store.load()?;
        let active = records
            .iter()
            .find(|record| record.outcome.is_none())
            .cloned();
        let recent: Vec<_> = records
            .into_iter()
            .filter(|record| record.outcome.is_some())
            .collect();
        let usage_retries = super::usage_retry::UsageRetries::new(active.as_ref(), &recent);
        Ok(Self {
            data: Mutex::new(RunData {
                active,
                recent,
                artifacts: Vec::new(),
            }),
            store,
            operation: Mutex::new(()),
            collection: Mutex::new(()),
            usage_retries: Mutex::new(usage_retries),
            archive_failure: Mutex::new(None),
            collection_failure: Mutex::new(None),
            catalog_revision: crate::history::catalog_cache::Revision::default(),
        })
    }

    pub(super) fn report_archive_error(&self, run_id: &str, error: Option<&str>) {
        if let Ok(mut last) = self.archive_failure.lock() {
            let next = error.map(|error| (run_id.to_owned(), error.to_owned()));
            if *last != next {
                if let Some((run_id, error)) = &next {
                    eprintln!("agent archive {run_id}: {error}");
                }
                *last = next;
            }
        }
    }

    pub(crate) fn catalog_revision(&self) -> u64 { self.catalog_revision.get() }

    /// The catalog uses identity and active/terminal evidence, not streaming
    /// part IDs, pending usage, error text or tool outcomes. Publish invalidation
    /// only after the same durable write that gates the in-memory RunRecord.
    pub(super) fn persist_record(&self, previous: Option<&RunRecord>, next: &RunRecord) -> Result<(), String> {
        let changed = previous.is_none_or(|old| old.run_id != next.run_id
            || old.session_id != next.session_id || old.workspace_path != next.workspace_path
            || old.outcome.is_none() != next.outcome.is_none());
        self.store.write(next)?;
        if changed { self.catalog_revision.committed(); }
        Ok(())
    }

    pub(crate) fn lock_operation(&self) -> Result<MutexGuard<'_, ()>, String> {
        self.operation
            .lock()
            .map_err(|_| "agent_state_unavailable".to_owned())
    }

    // Do not let overlapping live/recovery ticks publish older text with the
    // same message/part IDs. Control commands never acquire this mutex.
    pub(super) fn try_collection(&self) -> Result<Option<MutexGuard<'_, ()>>, String> {
        match self.collection.try_lock() {
            Ok(guard) => Ok(Some(guard)),
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Poisoned(_)) => Err("agent_state_unavailable".into()),
        }
    }

    pub(super) fn matches_snapshot(&self, candidate: &RunRecord) -> Result<bool, String> {
        let data = self.data.lock().map_err(|_| "agent_state_unavailable")?;
        // Full value comparison is the revision token. Cancellation, ownership,
        // recovered input and concurrent usage updates all invalidate it.
        Ok(data.active.as_ref() == Some(candidate))
    }

    pub(crate) fn begin(&self, run_id: &str, workspace: &Path, input: &str) -> Result<(), String> {
        let _service = crate::component_activation::service_use()?;
        if !crate::worklog::bridge::safe_id(run_id) || input.trim().is_empty() {
            return Err("agent_start_invalid".into());
        }
        let workspace_path = workspace
            .canonicalize()
            .map_err(|_| "workspace_invalid_path")?;
        if !workspace_path.is_dir() {
            return Err("workspace_not_directory".into());
        }
        let mut data = self.data.lock().map_err(|_| "agent_state_unavailable")?;
        if data.active.is_some() {
            return Err("agent_run_busy".into());
        }
        let record = RunRecord {
            run_id: run_id.into(),
            session_id: None,
            workspace_path,
            created_at: chrono::Utc::now().to_rfc3339(),
            ended_at: None,
            outcome: None,
            error_summary: None,
            pending_outcome: None,
            pending_error_summary: None,
            message_ids: Vec::new(),
            pending_usage_message_ids: Vec::new(),
            part_ids: Vec::new(),
            call_ids: Vec::new(),
            initial_input: Some(input.into()),
        };
        self.persist_record(None, &record)?;
        data.active = Some(record);
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn bind_session(&self, run_id: &str, session_id: &str) -> Result<(), String> {
        self.bind_session_with(run_id, session_id, || Ok(()))
    }

    pub(crate) fn bind_session_with(
        &self,
        run_id: &str,
        session_id: &str,
        register: impl FnOnce() -> Result<(), String>,
    ) -> Result<(), String> {
        let mut data = self.data.lock().map_err(|_| "agent_state_unavailable")?;
        let record = data
            .active
            .as_ref()
            .filter(|record| record.run_id == run_id)
            .ok_or_else(|| "agent_run_unknown".to_owned())?;
        register()?;
        let mut next = record.clone();
        next.session_id = Some(session_id.into());
        if next == *record {
            return Ok(());
        }
        self.persist_record(Some(record), &next)?;
        data.active = Some(next);
        Ok(())
    }

    pub(crate) fn confirm_submission(&self, run_id: &str) -> Result<(), String> {
        self.update_active(run_id, |record| record.initial_input = None)
    }

    #[cfg(test)]
    pub(crate) fn cancel_with(
        &self,
        run_id: &str,
        abort: impl FnOnce(&str) -> Result<bool, String>,
    ) -> Result<(), String> {
        let session = self.active_session(run_id)?;
        if !abort(&session)? {
            self.update_active(run_id, |record| {
                record.error_summary = Some("abort_not_confirmed".into())
            })?;
            return Err("agent_abort_unconfirmed".into());
        }
        self.finish(run_id, RunOutcome::Cancelled, None)
    }

    pub(crate) fn reconcile(&self, run_id: &str, messages: &[NativeMessage]) -> Result<(), String> {
        let mut terminal: Option<(RunOutcome, Option<String>)> = None;
        self.update_active(run_id, |record| {
            let mut message_ids: BTreeSet<String> = record.message_ids.iter().cloned().collect();
            let mut part_ids: BTreeSet<String> = record.part_ids.iter().cloned().collect();
            let mut call_ids: BTreeSet<String> = record.call_ids.iter().cloned().collect();
            let mut latest = None;
            let mut has_in_flight_tool = false;
            for message in messages
                .iter()
                .filter(|message| message.parent_id.as_deref() == Some(run_id))
            {
                latest = Some(message);
                message_ids.insert(message.id.clone());
                for part in &message.parts {
                    has_in_flight_tool |= matches!(
                        part.state.as_ref().map(|state| state.status.as_str()),
                        Some("pending" | "running")
                    );
                    part_ids.insert(part.id.clone());
                    if let Some(call_id) = &part.call_id {
                        call_ids.insert(call_id.clone());
                    }
                }
            }
            if let Some(message) = latest {
                terminal = if has_in_flight_tool {
                    None
                } else if let Some(error) = &message.error {
                    Some((RunOutcome::Failed, Some(error.clone())))
                } else if message.completed && message.finish.as_deref() == Some("stop") {
                    Some((RunOutcome::Completed, None))
                } else {
                    None
                };
            }
            record.message_ids = message_ids.into_iter().collect();
            record.part_ids = part_ids.into_iter().collect();
            record.call_ids = call_ids.into_iter().collect();
        })?;
        let mut data = self.data.lock().map_err(|_| "agent_state_unavailable")?;
        if let Some(record) = data
            .active
            .as_ref()
            .filter(|record| record.run_id == run_id)
        {
            data.artifacts = super::artifacts::collect(record, messages);
        }
        drop(data);
        if let Some((outcome, error)) = terminal {
            self.finish(run_id, outcome, error)?;
        }
        Ok(())
    }

    pub(crate) fn interrupt_active(&self, summary: &str) -> Result<(), String> {
        let run_id = self
            .data
            .lock()
            .map_err(|_| "agent_state_unavailable")?
            .active
            .as_ref()
            .map(|record| record.run_id.clone());
        if let Some(run_id) = run_id {
            self.finish(&run_id, RunOutcome::Interrupted, Some(summary.into()))?;
        }
        Ok(())
    }

    pub(crate) fn request_finish(
        &self,
        run_id: &str,
        outcome: RunOutcome,
        error: Option<String>,
    ) -> Result<(), String> {
        self.update_active(run_id, |record| {
            record.pending_outcome = Some(outcome);
            record.pending_error_summary = error;
        })
    }

    pub(super) fn set_collection_error(
        &self,
        run_id: &str,
        error: Option<&str>,
    ) -> Result<(), String> {
        let next = error.map(str::to_owned);
        if self.active_record(run_id)?.error_summary == next {
            return Ok(());
        }
        self.update_active(run_id, |record| record.error_summary = next)
    }

    fn update_active(
        &self,
        run_id: &str,
        update: impl FnOnce(&mut RunRecord),
    ) -> Result<(), String> {
        let mut data = self.data.lock().map_err(|_| "agent_state_unavailable")?;
        let record = data
            .active
            .as_ref()
            .filter(|record| record.run_id == run_id)
            .ok_or_else(|| "agent_run_unknown".to_owned())?;
        let mut next = record.clone();
        update(&mut next);
        if next == *record {
            return Ok(());
        }
        self.persist_record(Some(record), &next)?;
        data.active = Some(next);
        Ok(())
    }
}
