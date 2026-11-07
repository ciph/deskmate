use super::{collector::SnapshotRead, opencode::OpenCodeClient, record_store::{NativeMessage, RunRecord}};

const IDLE_GRACE_MS: u64 = 10_000;

fn unfinished(record: &RunRecord, messages: &[NativeMessage], now: u64) -> bool {
    if record.initial_input.is_some() || record.pending_outcome.is_some() { return false; }
    let Ok(created) = chrono::DateTime::parse_from_rfc3339(&record.created_at) else { return false; };
    let Ok(mut latest_activity) = u64::try_from(created.timestamp_millis()) else { return false; };
    let mut accepted = false;
    let mut latest_reply = None;
    for message in messages {
        if message.id == record.run_id && message.role.as_deref() == Some("user") { accepted = true; }
        if message.parent_id.as_deref() == Some(&record.run_id) {
            accepted = true;
            if super::supervision::in_flight(message) { return false; }
            latest_activity = latest_activity.max(message.created.unwrap_or(now));
            latest_reply = Some(message);
        }
    }
    accepted && now.saturating_sub(latest_activity) >= IDLE_GRACE_MS
        && !latest_reply.is_some_and(|message| message.error.is_some()
            || (message.completed && message.finish.as_deref() == Some("stop")))
}

pub(super) fn inspect(client: &OpenCodeClient, record: &RunRecord, messages: Vec<NativeMessage>) -> SnapshotRead {
    let now = chrono::Utc::now().timestamp_millis().max(0) as u64;
    if !unfinished(record, &messages, now) { return SnapshotRead::Messages(messages); }
    let Some(session) = record.session_id.as_deref() else { return SnapshotRead::Messages(messages); };
    let probe = || -> Result<Option<SnapshotRead>, String> {
        if !client.idle_without_interactions(session)? { return Ok(None); }
        // Re-read full messages: a late completion must win over the cached unfinished snapshot.
        let latest = client.snapshot(session)?;
        if unfinished(record, &latest, now) && client.idle_without_interactions(session)? {
            Ok(Some(SnapshotRead::IdleInterrupted(latest)))
        } else {
            Ok(Some(SnapshotRead::Messages(latest)))
        }
    };
    match probe() {
        Ok(Some(snapshot)) => snapshot,
        Ok(None) => SnapshotRead::Messages(messages),
        Err(error) => {
            // Unknown liveness must not enter the tool-failure path that can isolate the engine.
            eprintln!("agent idle recovery {} deferred: {error}", record.run_id);
            SnapshotRead::Messages(messages)
        }
    }
}

#[cfg(test)]
#[path = "idle_recovery_tests.rs"]
mod tests;
