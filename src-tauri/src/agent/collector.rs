use super::{
    lifecycle::AgentRunState,
    permissions::AgentPermissionState,
    record_store::{NativeMessage, RunOutcome, RunRecord},
    AgentPendingPermission,
};
use crate::tool_permissions::runtime::{PermissionRequest, Reply};
use tauri::Manager;

pub(super) enum SnapshotRead {
    Messages(Vec<NativeMessage>),
    SidecarLost,
}

pub(super) struct CollectorActions<S, P, A, R> {
    pub(super) snapshot: S,
    pub(super) pending: P,
    pub(super) archive: A,
    pub(super) respond: R,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum CollectionMode {
    Live,
    Recovery,
}

#[cfg(test)]
pub(super) fn collect_once_with<S, P, A, R>(
    state: &AgentRunState,
    permissions: &AgentPermissionState,
    actions: CollectorActions<S, P, A, R>,
) -> Result<(), String>
where
    S: FnOnce(&RunRecord) -> Result<SnapshotRead, String>,
    P: FnOnce(&RunRecord) -> Result<Vec<PermissionRequest>, String>,
    A: FnOnce(&RunRecord, &[NativeMessage]) -> Result<(), String>,
    R: FnMut(&RunRecord, &PermissionRequest, Reply) -> Result<(), String>,
{
    collect_once_guarded_with(state, permissions, actions, || Ok(Some(())), |_, _| Ok(()))
}

#[cfg(test)]
pub(super) fn collect_once_guarded_with<S, P, A, R, G>(
    state: &AgentRunState,
    permissions: &AgentPermissionState,
    actions: CollectorActions<S, P, A, R>,
    guard: impl FnOnce() -> Result<Option<G>, String>,
    capture: impl FnOnce(&RunRecord, &[NativeMessage]) -> Result<(), String>,
) -> Result<(), String>
where
    S: FnOnce(&RunRecord) -> Result<SnapshotRead, String>,
    P: FnOnce(&RunRecord) -> Result<Vec<PermissionRequest>, String>,
    A: FnOnce(&RunRecord, &[NativeMessage]) -> Result<(), String>,
    R: FnMut(&RunRecord, &PermissionRequest, Reply) -> Result<(), String>,
{
    collect_once_prepared_with(state, permissions, actions, guard, capture, |_, ()| Ok(()))
}

pub(super) fn collect_once_prepared_with<S, P, A, R, G, T>(
    state: &AgentRunState,
    permissions: &AgentPermissionState,
    mut actions: CollectorActions<S, P, A, R>,
    guard: impl FnOnce() -> Result<Option<G>, String>,
    capture: impl FnOnce(&RunRecord, &[NativeMessage]) -> Result<(), String>,
    commit: impl FnOnce(&RunRecord, T) -> Result<(), String>,
) -> Result<(), String>
where
    S: FnOnce(&RunRecord) -> Result<SnapshotRead, String>,
    P: FnOnce(&RunRecord) -> Result<Vec<PermissionRequest>, String>,
    A: FnOnce(&RunRecord, &[NativeMessage]) -> Result<T, String>,
    R: FnMut(&RunRecord, &PermissionRequest, Reply) -> Result<(), String>,
{
    let Some(_collection) = state.try_collection()? else {
        // A skipped tick does not publish a terminal result or clear durable
        // recovery work. The next background pass retries the current run.
        return Ok(());
    };
    let Some(candidate) = state.read()?.active else {
        return Ok(());
    };
    if candidate.initial_input.is_some() || candidate.session_id.is_none() {
        return Ok(());
    }

    let snapshot = (actions.snapshot)(&candidate);
    let pending = if matches!(snapshot, Ok(SnapshotRead::SidecarLost))
        || candidate.pending_outcome.is_some()
    {
        None
    } else {
        Some((actions.pending)(&candidate))
    };
    // Legacy projection construction, serialization and temporary-file sync
    // may be expensive. They must not hold the run control or engine locks.
    // Check before preparing as well as before committing; a stale preparation
    // is harmless and its staged file is removed on drop.
    if !state.matches_snapshot(&candidate)? {
        return Ok(());
    }
    let prepared = match &snapshot {
        Ok(SnapshotRead::Messages(messages)) => {
            if candidate.pending_outcome.is_some()
                && messages.iter().any(|message| {
                    message.parent_id.as_deref() == Some(&candidate.run_id)
                        && super::supervision::in_flight(message)
                })
            {
                return Err("agent_tools_unsettled".into());
            }
            Some((actions.archive)(&candidate, messages))
        }
        _ => None,
    };
    let mut automatic = Vec::new();
    let mut reported_error = None;
    {
        let _operation = state.lock_operation()?;
        let Some(_connection) = guard()? else { return Ok(()); };
        if !state.matches_snapshot(&candidate)? {
            return Ok(());
        }
        let current = state.active_record(&candidate.run_id)?;
        if current.pending_outcome != candidate.pending_outcome {
            return Ok(());
        }
        if matches!(snapshot, Ok(SnapshotRead::SidecarLost)) {
            let (outcome, error) = current
                .pending_outcome
                .clone()
                .map(|outcome| (outcome, current.pending_error_summary.clone()))
                .unwrap_or_else(|| {
                    (
                        RunOutcome::Interrupted,
                        Some("sidecar_process_lost".to_owned()),
                    )
                });
            state.finish(&candidate.run_id, outcome, error)?;
            let _ = permissions.cancel_run(&candidate.run_id);
            return Ok(());
        }

        match snapshot {
            Err(error) => {
                eprintln!("agent snapshot {}: {error}", candidate.run_id);
                let summary = match error.as_str() {
                    "agent_tool_timeout" | "agent_tools_unsettled" => error.as_str(),
                    _ => "agent_read_failed",
                };
                state.set_collection_error(&candidate.run_id, Some(summary))?;
                reported_error = Some(summary.to_owned());
            }
            Ok(SnapshotRead::Messages(messages)) => {
                if current.pending_outcome.is_some()
                    && messages.iter().any(|message| {
                        message.parent_id.as_deref() == Some(&current.run_id)
                            && super::supervision::in_flight(message)
                    })
                {
                    return Err("agent_tools_unsettled".into());
                }
                capture(&current, &messages)?;
                let archive_error = prepared.expect("message snapshot was prepared")
                    .and_then(|prepared| commit(&current, prepared)).err();
                state.report_archive_error(&candidate.run_id, archive_error.as_deref());
                state.set_collection_error(&candidate.run_id, None)?;
                if let Some(outcome) = current.pending_outcome.clone() {
                    state.finish(
                        &candidate.run_id,
                        outcome,
                        current.pending_error_summary.clone(),
                    )?;
                } else {
                    state.reconcile(&candidate.run_id, &messages)?;
                }
                if state.active_record(&candidate.run_id).is_err() {
                    let _ = permissions.cancel_run(&candidate.run_id);
                } else if let Some(pending) = pending {
                    let current = state.active_record(&candidate.run_id)?;
                    if current.pending_outcome != candidate.pending_outcome {
                        return Ok(());
                    }
                    match pending.and_then(|requests| {
                        super::process_pending(
                            permissions,
                            &current.run_id,
                            &current.message_ids,
                            &current.call_ids,
                            requests,
                        )
                    }) {
                        Ok(replies) => automatic = replies,
                        Err(error) => {
                            state.set_collection_error(&candidate.run_id, Some(&error))?;
                            reported_error = Some(error);
                        }
                    }
                }
            }
            Ok(SnapshotRead::SidecarLost) => return Ok(()),
        }
    }

    if state.matches_active(&candidate)? {
        for (request, reply) in automatic {
            if !state.matches_active(&candidate)? {
                break;
            }
            if let Err(error) = (actions.respond)(&candidate, &request, reply) {
                if error != "permission_expired" {
                    return Err(error);
                }
                eprintln!("agent permission {} already settled", request.id);
            }
        }
    }
    match reported_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

pub(crate) fn collect_active_run_once(
    app: &tauri::AppHandle,
    mode: CollectionMode,
) -> Result<(), String> {
    collect_active_run_with_snapshot(app, mode, None, |record, client| {
        super::supervision::snapshot(client, &app.state::<AgentRunState>(), record)
    }).map(|_| ())
}

fn collect_active_run_with_snapshot(
    app: &tauri::AppHandle,
    mode: CollectionMode,
    expected_generation: Option<u64>,
    snapshot: impl FnOnce(&RunRecord, &super::opencode::OpenCodeClient) -> Result<Vec<NativeMessage>, String>,
) -> Result<bool, String> {
    let state = app.state::<AgentRunState>();
    let permissions = app.state::<AgentPermissionState>();
    let base = crate::tool_permissions::runtime::endpoint(app);
    let candidate = state.read()?.active;
    if let Some(record) = candidate
        .as_ref()
        .filter(|record| record.initial_input.is_some())
    {
        let Some(session) = record.session_id.as_deref() else {
            return Ok(false);
        };
        let settings = super::run_commands::current_start_settings(app)?;
        let client = super::run_commands::lifecycle_client(app, &settings, &record.workspace_path);
        // A failed confirmation remains a preparation barrier. Do not feed
        // metadata failures to execution supervision or submit another prompt.
        if !super::recovery::confirm_from_native_with(&state, record, || client.snapshot(session))?
        {
            return Ok(false);
        }
    }
    if let Some(record) = state
        .read()?
        .active
        .filter(|record| record.initial_input.is_none())
    {
        let _operation = state.lock_operation()?;
        if state.matches_active(&record)? {
            let approvals = app
                .state::<crate::settings::SettingsState>()
                .0
                .lock()
                .map_err(|_| "agent_settings_unavailable")?
                .agent_permission_approvals
                .clone();
            permissions.ensure_run_registered(&record, &approvals)?;
        }
    }
    let sidecar = app.state::<crate::Sidecar>();
    let generation = crate::sidecar_generation(app)?;
    if expected_generation.is_some_and(|expected| expected != generation) { return Ok(false); }
    let applied = std::cell::Cell::new(false);
    let result = collect_once_prepared_with(
        &state,
        &permissions,
        CollectorActions {
            snapshot: |record: &RunRecord| {
                if !crate::sidecar_owned_running(app)? {
                    let _operation = state.lock_operation()?;
                    if !state.matches_active(record)? {
                        return Ok(SnapshotRead::Messages(Vec::new()));
                    }
                    let current = state.active_record(&record.run_id)?;
                    let (outcome, error) = current
                        .pending_outcome
                        .clone()
                        .map(|outcome| (outcome, current.pending_error_summary.clone()))
                        .unwrap_or_else(|| {
                            (
                                RunOutcome::Interrupted,
                                Some("sidecar_process_lost".to_owned()),
                            )
                        });
                    let messages = super::supervision::settle(
                        app,
                        &current,
                        error.as_deref().unwrap_or("agent_cancelled"),
                    )?;
                    state.request_finish(&current.run_id, outcome, error)?;
                    return Ok(SnapshotRead::Messages(messages));
                }
                let settings = super::run_commands::current_start_settings(app)?;
                let client = super::run_commands::lifecycle_client(app, &settings, &record.workspace_path);
                snapshot(record, &client).map(SnapshotRead::Messages)
            },
            pending: |record: &RunRecord| {
                if mode == CollectionMode::Recovery {
                    Ok(Vec::new())
                } else {
                    let session = record
                        .session_id
                        .as_deref()
                        .ok_or_else(|| "agent_session_unknown".to_owned())?;
                    if let Some(resources) =
                        app.try_state::<crate::chat_attachments::resources::ResourceStore>()
                    {
                        permissions.set_resource_scopes(
                            &record.run_id,
                            resources.scopes(
                                &crate::agent::opencode_wire_directory(&record.workspace_path),
                                session,
                            )?,
                        )?;
                    }
                    crate::tool_permissions::runtime::pending_scoped_live(
                        app,
                        &record.workspace_path,
                        session,
                    )
                }
            },
            archive: |record: &RunRecord, messages: &[NativeMessage]| {
                let session = record
                    .session_id
                    .as_deref()
                    .ok_or_else(|| "agent_session_unknown".to_owned())?;
                let archived = crate::history::prepare_agent_snapshot(
                    app,
                    &record.workspace_path,
                    crate::history::AgentHistorySnapshot {
                        session_id: session,
                        messages,
                    },
                );
                archived
            },
            respond: |record: &RunRecord, request: &PermissionRequest, reply: Reply| {
                crate::tool_permissions::runtime::respond_scoped(
                    &base,
                    request,
                    reply,
                    &record.workspace_path,
                )
            },
        },
        || sidecar.generation_guard(generation),
        |record, messages| {
            let missing = crate::ai_usage::capture_agent_usage(app, record, messages).unwrap_or_else(|error| {
                eprintln!("agent usage capture {}: {error}", record.run_id);
                messages.iter().filter(|message| message.parent_id.as_deref() == Some(&record.run_id)
                    && message.role.as_deref() == Some("assistant") && message.completed)
                    .map(|message| message.id.clone()).collect()
            });
            state.set_usage_pending(record, retain_unobserved_usage(record, messages, missing))?;
            // This callback runs only after the full revision/generation fence.
            applied.set(true);
            Ok(())
        },
        |_, prepared: crate::history::PreparedAgentSnapshot| prepared.commit(app),
    );
    if let Err(ref error) = result {
        if let Some(candidate) = candidate {
            super::supervision::collection_failed(app, &candidate, error)?;
        }
    } else {
        *state
            .collection_failure
            .lock()
            .map_err(|_| "agent_state_unavailable")? = None;
    }
    result.map(|_| applied.get())
}


pub(super) fn retain_unobserved_usage(record: &RunRecord, messages: &[NativeMessage], mut missing: Vec<String>) -> Vec<String> {
    missing.extend(record.pending_usage_message_ids.iter().filter(|id| {
        !messages.iter().any(|message| &message.id == *id
            && message.parent_id.as_deref() == Some(&record.run_id)
            && message.role.as_deref() == Some("assistant") && message.completed)
    }).cloned());
    missing
}

pub(super) fn retry_usage_with<G>(
    state: &AgentRunState,
    candidate: &RunRecord,
    snapshot: impl FnOnce() -> Result<Vec<NativeMessage>, String>,
    guard: impl FnOnce() -> Result<Option<G>, String>,
    capture: impl FnOnce(&RunRecord, &[NativeMessage]) -> Result<Vec<String>, String>,
) -> Result<(), String> {
    let messages = snapshot()?;
    let _operation = state.lock_operation()?;
    let Some(_connection) = guard()? else { return Ok(()); };
    let Some(current) = state.record(&candidate.run_id)? else { return state.discard_missing_usage(candidate); };
    if !super::usage_retry::same_identity(candidate, &current) { return Ok(()); }
    let requested = &candidate.pending_usage_message_ids;
    let selected: Vec<_> = messages.into_iter().filter(|message| requested.contains(&message.id)
        && current.pending_usage_message_ids.contains(&message.id)).collect();
    let missing = capture(&current, &selected)?;
    state.set_usage_pending(&current, retain_unobserved_usage(&current, &selected, missing))
}

fn retry_usage_once(app: &tauri::AppHandle, state: &AgentRunState) {
    let now = std::time::Instant::now();
    let candidate = match state.usage_retry_candidate(now) {
        Ok(Some(candidate)) => candidate,
        Ok(None) => return,
        Err(error) => { eprintln!("agent usage retry: {error}"); return; }
    };
    let result = (|| -> Result<(), String> {
        let generation = crate::sidecar_generation(app)?;
        let session = candidate.session_id.as_deref().ok_or("agent_session_unknown")?;
        let settings = super::run_commands::current_start_settings(app)?;
        let sidecar = app.state::<crate::Sidecar>();
        retry_usage_with(state, &candidate,
            || super::run_commands::lifecycle_client(app, &settings, &candidate.workspace_path).snapshot(session),
            || sidecar.generation_guard(generation),
            |record, messages| crate::ai_usage::capture_agent_usage(app, record, messages))
    })();
    if let Err(error) = result { eprintln!("agent usage retry {}: {error}", candidate.run_id); }
    if let Err(error) = state.finish_usage_retry(&candidate.run_id, std::time::Instant::now()) {
        eprintln!("agent usage retry scheduling: {error}");
    }
}


fn collect_live_run_once(
    app: &tauri::AppHandle,
    refresh: &mut super::collector_refresh::LiveRefresh,
) -> Result<(), String> {
    use super::collector_refresh::LiveRefresh;
    let state = app.state::<AgentRunState>();
    let events = app.try_state::<crate::tool_permissions::events::PermissionEvents>();
    let Some(record) = state.read()?.active else {
        refresh.clear();
        if let Some(events) = events { events.deactivate()?; }
        return Ok(());
    };
    // Preparation/recovery and process loss retain their explicit full path.
    let Some(events) = events.filter(|_| record.initial_input.is_none()) else {
        refresh.clear();
        return collect_active_run_once(app, CollectionMode::Live);
    };
    if !crate::sidecar_owned_running(app)? {
        refresh.clear();
        return collect_active_run_once(app, CollectionMode::Live);
    }
    let generation = crate::sidecar_generation(app)?;
    let Some(scope) = LiveRefresh::scope(&record, generation) else { return Ok(()); };
    let changes = events.changes(&scope)?;
    let permission_waiting = app.state::<AgentPermissionState>()
        .waiting(&record.run_id).is_ok_and(|requests| !requests.is_empty());
    let Some(plan) = refresh.plan(&record, &scope, &changes, std::time::Instant::now(), permission_waiting) else {
        return Ok(());
    };
    let mut fetched = None;
    let result = collect_active_run_with_snapshot(app, CollectionMode::Live, Some(generation), |candidate, client| {
        let session = candidate.session_id.as_deref().ok_or("agent_session_unknown")?;
        if LiveRefresh::scope(candidate, generation).as_ref() != Some(&scope)
            || candidate.created_at != record.created_at
            || candidate.pending_outcome != record.pending_outcome
        {
            return super::supervision::snapshot(client, &state, candidate);
        }
        let mut next = refresh.fetch_with(candidate, &plan,
            || client.snapshot(session), |id| client.message(session, id))?;
        let messages = super::supervision::inspect_messages(client, &state, candidate, next.messages, next.full)?;
        next.messages = messages.clone();
        fetched = Some(next);
        Ok(messages)
    });
    // Busy single-flight skips never call the snapshot closure, so they cannot
    // acknowledge dirty IDs. Events received during HTTP survive this ticket.
    if matches!(result, Ok(true)) && crate::sidecar_generation(app)? == generation {
        if let Some(fetched) = fetched {
            refresh.confirm(fetched, std::time::Instant::now());
            events.ack(&scope, changes.ticket)?;
        }
    }
    result.map(|_| ())
}

pub(crate) fn start_collector(app: tauri::AppHandle) {
    let usage_app = app.clone();
    std::thread::spawn(move || loop {
        if crate::sidecar_stopping(&usage_app) { return; }
        std::thread::sleep(std::time::Duration::from_secs(1));
        if crate::sidecar_stopping(&usage_app) { return; }
        retry_usage_once(&usage_app, &usage_app.state::<AgentRunState>());
    });
    std::thread::spawn(move || {
        let mut refresh = super::collector_refresh::LiveRefresh::default();
        loop {
            if crate::sidecar_stopping(&app) { return; }
            std::thread::sleep(std::time::Duration::from_secs(1));
            if crate::sidecar_stopping(&app) { return; }
            if let Err(error) = collect_live_run_once(&app, &mut refresh) {
                eprintln!("agent collector: {error}");
            }
        }
    });
}

pub(super) fn cached_permissions(
    permissions: &AgentPermissionState,
    run_id: &str,
) -> Result<Vec<AgentPendingPermission>, String> {
    permissions
        .waiting(run_id)?
        .into_iter()
        .map(|waiting| {
            Ok(AgentPendingPermission {
                always: crate::tool_permissions::rememberable_agent_patterns(&waiting.request)
                    .unwrap_or_default()
                    .to_vec(),
                request_id: waiting.request.id,
                permission: waiting.request.permission,
                patterns: waiting.request.patterns,
                metadata: waiting.request.metadata,
                command: waiting.detail.command,
                cwd: waiting.detail.cwd.to_string_lossy().into_owned(),
            })
        })
        .collect()
}
