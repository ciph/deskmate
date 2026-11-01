use super::{lifecycle::AgentRunState, record_store::RunRecord};
use std::{collections::VecDeque, time::{Duration, Instant}};

const WINDOW: usize = 32;

#[derive(Default)]
pub(super) struct UsageRetries {
    pending: usize,
    queue: VecDeque<String>,
    cursor: usize,
    remaining: usize,
    rounds: u32,
    due: Option<Instant>,
    in_flight: Option<String>,
}

impl UsageRetries {
    pub(super) fn new(active: Option<&RunRecord>, recent: &[RunRecord]) -> Self {
        Self {
            pending: active.into_iter().chain(recent).filter(|run| !run.pending_usage_message_ids.is_empty()).count(),
            ..Self::default()
        }
    }

    fn refill(&mut self, active: Option<&RunRecord>, recent: &[RunRecord], now: Instant) {
        if self.pending == 0 || !self.queue.is_empty() || self.in_flight.is_some() { return; }
        let count = recent.len() + usize::from(active.is_some());
        if count == 0 {
            self.pending = 0; self.queue.clear(); self.remaining = 0; self.cursor = 0;
            return;
        }
        if self.remaining == 0 {
            if self.due.is_some_and(|due| due > now) { return; }
            self.remaining = count;
            self.due = None;
        }
        while self.remaining > 0 && self.queue.len() < WINDOW {
            let index = self.cursor % count;
            self.cursor = (index + 1) % count;
            self.remaining -= 1;
            let record = if let Some(active) = active {
                if index == 0 { active } else { &recent[index - 1] }
            } else { &recent[index] };
            if !record.pending_usage_message_ids.is_empty() {
                self.queue.push_back(record.run_id.clone());
            }
        }
    }
}

pub(super) fn same_identity(left: &RunRecord, right: &RunRecord) -> bool {
    left.run_id == right.run_id && left.session_id == right.session_id
        && left.workspace_path == right.workspace_path && left.created_at == right.created_at
}

impl AgentRunState {
    /// Pending IDs share the run's atomic persistence boundary, including terminal records.
    pub(super) fn set_usage_pending(&self, expected: &RunRecord, mut ids: Vec<String>) -> Result<(), String> {
        ids.sort(); ids.dedup();
        let mut data = self.data.lock().map_err(|_| "agent_state_unavailable")?;
        let current = data.active.as_ref().filter(|run| run.run_id == expected.run_id)
            .or_else(|| data.recent.iter().find(|run| run.run_id == expected.run_id))
            .ok_or("agent_run_unknown")?;
        if !same_identity(current, expected) { return Err("agent_usage_stale".into()); }
        if current.pending_usage_message_ids == ids { return Ok(()); }
        let was_pending = !current.pending_usage_message_ids.is_empty();
        let mut next = current.clone();
        next.pending_usage_message_ids = ids;
        self.persist_record(Some(current), &next)?;
        let is_pending = !next.pending_usage_message_ids.is_empty();
        if data.active.as_ref().is_some_and(|run| run.run_id == next.run_id) {
            data.active = Some(next);
        } else if let Some(record) = data.recent.iter_mut().find(|run| run.run_id == next.run_id) {
            *record = next;
        }
        let mut retries = self.usage_retries.lock().map_err(|_| "agent_state_unavailable")?;
        match (was_pending, is_pending) {
            (false, true) => {
                if retries.pending == 0 { retries.due = None; retries.remaining = 0; retries.rounds = 0; }
                retries.pending += 1;
            },
            (true, false) => {
                retries.pending = retries.pending.saturating_sub(1);
                retries.queue.retain(|id| id != &expected.run_id);
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn discard_missing_usage(&self, expected: &RunRecord) -> Result<(), String> {
        if self.store.get(&expected.run_id)?.is_some() { return Err("agent_usage_stale".into()); }
        let mut data = self.data.lock().map_err(|_| "agent_state_unavailable")?;
        let record = if data.active.as_ref().is_some_and(|run| run.run_id == expected.run_id) {
            data.active.as_mut()
        } else { data.recent.iter_mut().find(|run| run.run_id == expected.run_id) };
        if let Some(record) = record.filter(|record| same_identity(record, expected)) {
            if !record.pending_usage_message_ids.is_empty() {
                record.pending_usage_message_ids.clear();
                let mut retries = self.usage_retries.lock().map_err(|_| "agent_state_unavailable")?;
                retries.pending = retries.pending.saturating_sub(1);
                retries.queue.retain(|id| id != &expected.run_id);
            }
        }
        Ok(())
    }

    pub(super) fn usage_retry_candidate(&self, now: Instant) -> Result<Option<RunRecord>, String> {
        let data = self.data.lock().map_err(|_| "agent_state_unavailable")?;
        let mut retries = self.usage_retries.lock().map_err(|_| "agent_state_unavailable")?;
        retries.refill(data.active.as_ref(), &data.recent, now);
        if retries.in_flight.is_some() { return Ok(None); }
        while let Some(run_id) = retries.queue.pop_front() {
            let candidate = data.active.as_ref().filter(|run| run.run_id == run_id)
                .or_else(|| data.recent.iter().find(|run| run.run_id == run_id))
                .filter(|run| !run.pending_usage_message_ids.is_empty()).cloned();
            if candidate.is_some() {
                retries.in_flight = Some(run_id);
                return Ok(candidate);
            }
        }
        Ok(None)
    }

    pub(super) fn finish_usage_retry(&self, run_id: &str, now: Instant) -> Result<(), String> {
        let mut retries = self.usage_retries.lock().map_err(|_| "agent_state_unavailable")?;
        if retries.in_flight.as_deref() == Some(run_id) { retries.in_flight = None; }
        if retries.queue.is_empty() && retries.remaining == 0 {
            retries.rounds = retries.rounds.saturating_add(1);
            retries.due = Some(now + Duration::from_secs((5_u64 << retries.rounds.saturating_sub(1).min(6)).min(300)));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{record_store::{RunOutcome, RunStore}, test_support::TestResult};
    use std::{collections::HashSet, fs};

    #[test]
    fn terminal_pending_recovery_is_fair_beyond_the_listing_and_retry_window() -> TestResult<()> {
        let root = std::env::temp_dir().join(format!("yume-usage-window-{}", uuid::Uuid::new_v4()));
        let workspace = root.join("workspace");
        fs::create_dir_all(&workspace)?;
        let store = RunStore::new(root.join("runs"));
        let state = AgentRunState::new(store.clone());
        for index in 0..40 {
            let id = format!("run_{index}");
            state.begin(&id, &workspace, "fixture")?;
            state.bind_session(&id, "session")?;
            state.confirm_submission(&id)?;
            state.set_usage_pending(&state.active_record(&id)?, vec![format!("message_{index}")])?;
            state.finish(&id, RunOutcome::Completed, None)?;
        }
        let state = AgentRunState::load(store)?;
        assert_eq!(state.read()?.recent.len(), 20);
        let now = Instant::now();
        let mut seen = HashSet::new();
        for index in 0..40 {
            let candidate = state.usage_retry_candidate(now)?.expect("every durable pending record is visited");
            assert!(seen.insert(candidate.run_id.clone()));
            assert!(state.usage_retries.lock().unwrap().queue.len() < WINDOW);
            assert!(state.usage_retry_candidate(now)?.is_none(), "one inflight request");
            if index == 32 { state.set_usage_pending(&candidate, Vec::new())?; }
            state.finish_usage_retry(&candidate.run_id, now)?;
        }
        assert_eq!(seen.len(), 40, "first 32 permanent failures must not starve later records");
        assert!(state.usage_retry_candidate(now + Duration::from_secs(4))?.is_none());
        assert!(state.usage_retry_candidate(now + Duration::from_secs(5))?.is_some());
        println!("usage_retry loaded_terminal_pending=40 visible_recent=20 window=32 visited=40 max_inflight=1");
        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn empty_state_and_stale_queue_items_do_not_panic_or_claim_inflight() {
        let state = AgentRunState::new(RunStore::new(std::env::temp_dir().join("unused-usage-empty-fixture")));
        {
            let mut retries = state.usage_retries.lock().unwrap();
            retries.pending = 1;
            retries.queue.push_back("deleted".into());
        }
        assert!(state.usage_retry_candidate(Instant::now()).unwrap().is_none());
        assert!(state.usage_retries.lock().unwrap().in_flight.is_none());
        assert!(state.usage_retry_candidate(Instant::now()).unwrap().is_none());
        assert_eq!(state.usage_retries.lock().unwrap().pending, 0);
    }

    #[test]
    fn pending_marker_failure_preserves_ram_disk_and_restart() -> TestResult<()> {
        let root = std::env::temp_dir().join(format!("yume-usage-marker-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root)?;
        let store = RunStore::new(root.join("runs"));
        let state = AgentRunState::new(store.clone());
        state.begin("run", &root, "fixture")?;
        let before = state.active_record("run")?;
        store.inject_write_failure(true);
        assert_eq!(state.set_usage_pending(&before, vec!["message".into()]), Err("agent_storage_unavailable".into()));
        assert_eq!(state.active_record("run")?, before);
        store.inject_write_failure(false);
        assert_eq!(AgentRunState::load(store.clone())?.active_record("run")?, before);
        state.set_usage_pending(&before, vec!["message".into()])?;
        state.finish("run", RunOutcome::Failed, Some("fixture".into()))?;
        let recovered = AgentRunState::load(store)?;
        assert_eq!(recovered.usage_retry_candidate(Instant::now())?.unwrap().pending_usage_message_ids, ["message"]);
        fs::remove_dir_all(root)?;
        Ok(())
    }
}
