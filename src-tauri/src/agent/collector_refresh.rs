use super::record_store::{NativeMessage, RunRecord};
use crate::tool_permissions::events::{RefreshChanges, RefreshScope};
use std::time::{Duration, Instant};

const FULL_RECONCILE: Duration = Duration::from_secs(30);
const MAX_MESSAGE_FETCHES: usize = 16;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum RefreshPlan {
    Full,
    Messages(Vec<String>),
    Cached,
}

pub(super) struct FetchedSnapshot {
    pub(super) messages: Vec<NativeMessage>,
    pub(super) full: bool,
}

#[derive(Default)]
pub(super) struct LiveRefresh {
    scope: Option<RefreshScope>,
    created_at: String,
    messages: Vec<NativeMessage>,
    last_full: Option<Instant>,
}

impl LiveRefresh {
    pub(super) fn clear(&mut self) { *self = Self::default(); }

    pub(super) fn scope(record: &RunRecord, generation: u64) -> Option<RefreshScope> {
        Some(RefreshScope {
            generation,
            run_id: record.run_id.clone(),
            directory: super::opencode_wire_directory(&record.workspace_path),
            session_id: record.session_id.clone()?,
        })
    }

    pub(super) fn plan(
        &mut self,
        record: &RunRecord,
        scope: &RefreshScope,
        changes: &RefreshChanges,
        now: Instant,
        permission_waiting: bool,
    ) -> Option<RefreshPlan> {
        if self.scope.as_ref() != Some(scope) || self.created_at != record.created_at {
            self.clear();
            self.scope = Some(scope.clone());
            self.created_at = record.created_at.clone();
        }
        if self.last_full.is_none_or(|last| now.saturating_duration_since(last) >= FULL_RECONCILE)
            || !changes.connected || changes.full || record.pending_outcome.is_some()
            || changes.message_ids.len() > MAX_MESSAGE_FETCHES
        {
            return Some(RefreshPlan::Full);
        }
        if !changes.message_ids.is_empty() {
            return Some(RefreshPlan::Messages(changes.message_ids.clone()));
        }
        // Keep the existing one-second tool supervision and approval deadline
        // while avoiding a full session GET when only those timers are due.
        (permission_waiting || self.messages.iter().any(|message| {
            message.parent_id.as_deref() == Some(&record.run_id)
                && super::supervision::in_flight(message)
        })).then_some(RefreshPlan::Cached)
    }

    pub(super) fn fetch_with(
        &self,
        record: &RunRecord,
        plan: &RefreshPlan,
        mut full: impl FnMut() -> Result<Vec<NativeMessage>, String>,
        mut message: impl FnMut(&str) -> Result<NativeMessage, String>,
    ) -> Result<FetchedSnapshot, String> {
        if *plan == RefreshPlan::Full {
            return full().map(|messages| FetchedSnapshot { messages, full: true });
        }
        let mut messages = self.messages.clone();
        if let RefreshPlan::Messages(ids) = plan {
            for id in ids {
                let update = match message(id) {
                    Ok(update) => update,
                    // Deletion/compaction or an incomplete per-message response
                    // requires authoritative full reconciliation before replacing cache.
                    Err(error) if matches!(error.as_str(), "agent_http_404" | "agent_wire_invalid"
                        | "agent_message_identity_mismatch" | "agent_message_incomplete") => {
                        return full().map(|messages| FetchedSnapshot { messages, full: true });
                    }
                    Err(error) => return Err(error),
                };
                if let Some(old) = messages.iter_mut().find(|item| item.id == update.id) {
                    *old = update;
                } else {
                    messages.push(update);
                }
            }
            // The pinned native list orders by creation time, then ID, not ID alone.
            messages.sort_by(|left, right| left.created.cmp(&right.created).then_with(|| left.id.cmp(&right.id)));
        }
        if terminal_candidate(record, &messages) {
            return full().map(|messages| FetchedSnapshot { messages, full: true });
        }
        Ok(FetchedSnapshot { messages, full: false })
    }

    pub(super) fn confirm(&mut self, fetched: FetchedSnapshot, now: Instant) {
        self.messages = fetched.messages;
        if fetched.full { self.last_full = Some(now); }
    }
}

fn terminal_candidate(record: &RunRecord, messages: &[NativeMessage]) -> bool {
    let mut latest = None;
    for message in messages.iter().filter(|message| message.parent_id.as_deref() == Some(&record.run_id)) {
        if super::supervision::in_flight(message) { return false; }
        latest = Some(message);
    }
    latest.is_some_and(|message| message.error.is_some()
        || (message.completed && message.finish.as_deref() == Some("stop")))
}

#[cfg(test)]
#[path = "collector_refresh_tests.rs"]
mod tests;
