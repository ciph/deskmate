use super::{agent, temp_file};
use crate::{
    agent::{NativeMessage, NativePart},
    history::{upsert_agent_snapshot, AgentHistorySnapshot},
};
use std::fs;

fn text_message(id: &str, role: &str, created: Option<u64>) -> NativeMessage {
    NativeMessage {
        usage: None,
        system: None,
        id: id.to_owned(),
        role: Some(role.to_owned()),
        created,
        parent_id: Some("msg-run".to_owned()),
        completed: true,
        finish: Some("stop".to_owned()),
        error: None,
        parts: vec![NativePart {
            file_name: None,
            synthetic: false,
            id: format!("prt-{id}"),
            kind: Some("text".to_owned()),
            text: Some(format!("text-{id}")),
            call_id: None,
            tool: None,
            state: None,
        }],
    }
}

#[test]
fn native_snapshots_upsert_by_ids_in_stable_time_order() -> Result<(), String> {
    let path = temp_file("snapshot");
    let mut list = vec![agent("ses-agent")];
    let later = text_message("msg-two", "assistant", Some(20));
    let mut earlier = text_message("msg-one", "user", Some(10));
    earlier.parent_id = None;
    earlier.finish = None;
    upsert_agent_snapshot(
        &path,
        &mut list,
        AgentHistorySnapshot {
            session_id: "ses-agent",
            messages: &[later.clone(), earlier.clone(), later],
        },
    )?;
    upsert_agent_snapshot(
        &path,
        &mut list,
        AgentHistorySnapshot {
            session_id: "ses-agent",
            messages: &[earlier],
        },
    )?;
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].messages.len(), 2);
    assert_eq!(list[0].messages[0].message_id.as_deref(), Some("msg-one"));
    assert_eq!(list[0].messages[1].part_id.as_deref(), Some("prt-msg-two"));
    assert_eq!(list[0].updated, 20);
    fs::remove_dir_all(path.parent().ok_or_else(|| "missing parent".to_owned())?)
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[test]
fn synthetic_attachment_context_is_not_displayed_as_user_input() -> Result<(), String> {
    let path = temp_file("synthetic-resource-context");
    let mut list = vec![agent("ses-agent")];
    let mut message = text_message("msg-user", "user", Some(10));
    let mut synthetic = message.parts[0].clone();
    synthetic.id = "prt-generated-directory".into();
    synthetic.text = Some("Called the Read tool with path /selected/folder".into());
    synthetic.synthetic = true;
    message.parts.push(synthetic);
    upsert_agent_snapshot(&path, &mut list, AgentHistorySnapshot { session_id: "ses-agent", messages: &[message.clone()] })?;
    assert_eq!(list[0].messages.len(), 1);
    assert_eq!(list[0].messages[0].text, "text-msg-user");
    assert_eq!(message.parts.len(), 2); // Projection leaves the native model context intact.
    fs::remove_dir_all(path.parent().ok_or("missing parent")?).map_err(|error| error.to_string())?;
    Ok(())
}

#[test]
fn later_run_snapshot_keeps_first_trusted_origin() -> Result<(), String> {
    let path = temp_file("continued-session");
    let mut list = vec![agent("ses-agent")];
    let mut reply = text_message("msg-reply", "assistant", Some(20));
    reply.parent_id = Some("msg-second-run".to_owned());
    upsert_agent_snapshot(
        &path,
        &mut list,
        AgentHistorySnapshot {
            session_id: "ses-agent",
            messages: &[reply],
        },
    )?;
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].origin_run_id.as_deref(), Some("msg_origin"));
    assert_eq!(list[0].messages.len(), 1);
    fs::remove_dir_all(path.parent().ok_or_else(|| "missing parent".to_owned())?)
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[test]
fn malformed_native_text_does_not_mutate_history() {
    let path = temp_file("malformed-snapshot");
    let mut list = vec![agent("ses-agent")];
    let malformed = text_message("msg-reply", "assistant", None);
    assert_eq!(
        upsert_agent_snapshot(
            &path,
            &mut list,
            AgentHistorySnapshot {
                session_id: "ses-agent",
                messages: &[malformed],
            },
        ),
        Err("history_snapshot_invalid".to_owned())
    );
    assert!(list[0].messages.is_empty());
    assert!(!path.exists());
}

#[test]
fn unchanged_snapshot_does_not_touch_storage() -> Result<(), String> {
    let path = temp_file("unchanged-snapshot");
    let mut list = vec![agent("ses-agent")];
    let reply = text_message("msg-reply", "assistant", Some(20));
    upsert_agent_snapshot(
        &path,
        &mut list,
        AgentHistorySnapshot {
            session_id: "ses-agent",
            messages: &[reply.clone()],
        },
    )?;
    fs::remove_file(&path).map_err(|error| error.to_string())?;
    fs::create_dir(&path).map_err(|error| error.to_string())?;
    upsert_agent_snapshot(
        &path,
        &mut list,
        AgentHistorySnapshot {
            session_id: "ses-agent",
            messages: &[reply],
        },
    )?;
    assert_eq!(list[0].messages.len(), 1);
    fs::remove_dir_all(path.parent().ok_or_else(|| "missing parent".to_owned())?)
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[test]
fn sixty_archive_ticks_read_only_the_origin_among_ten_thousand_old_runs() -> Result<(), String> {
    use crate::agent::{AgentRunState, RunOutcome, RunRecord, RunStore};
    let path = temp_file("target-record-read");
    let root = path.parent().ok_or("missing parent")?;
    let runs_path = root.join("runs");
    let workspace = root.join("workspace");
    fs::create_dir_all(&workspace).map_err(|e| e.to_string())?;
    let store = RunStore::new(runs_path.clone());
    let mut record = RunRecord {
        run_id: "msg_origin".into(), session_id: Some("ses-agent".into()),
        workspace_path: workspace.canonicalize().map_err(|e| e.to_string())?,
        created_at: "2026-01-01T00:00:00Z".into(), ended_at: Some("2026-01-01T00:00:01Z".into()),
        outcome: Some(RunOutcome::Completed), error_summary: None, pending_outcome: None,
        pending_error_summary: None, message_ids: Vec::new(), pending_usage_message_ids: Vec::new(),
        part_ids: Vec::new(), call_ids: Vec::new(), initial_input: None,
    };
    store.write(&record)?;
    for index in 0..10000 {
        record.run_id = format!("old_{index}");
        record.session_id = Some(format!("old_session_{index}"));
        fs::write(runs_path.join(format!("{}.json", record.run_id)), serde_json::to_vec(&record).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    }
    let runs = AgentRunState::load(store.clone())?;
    let baseline = store.io_counts();
    assert_eq!(runs.all_records()?.len(), 10001); // The prior per-tick path, retained as a count oracle.
    let prior_reads = store.io_counts()[0] - baseline[0];
    let mut list = vec![agent("ses-agent")];
    let messages = [text_message("msg-reply", "assistant", Some(20))];
    super::super::save_snapshot_from_run_store(&path, &mut list, &runs, &workspace,
        AgentHistorySnapshot { session_id: "ses-agent", messages: &messages })?;
    let before = store.io_counts();
    for _ in 0..60 {
        super::super::save_snapshot_from_run_store(&path, &mut list, &runs, &workspace,
            AgentHistorySnapshot { session_id: "ses-agent", messages: &messages })?;
    }
    let after = store.io_counts();
    assert_eq!(after[0] - before[0], 60);
    assert_eq!(&after[1..], &before[1..]);
    assert_eq!(list[0].messages.len(), 1);
    // An unrelated malformed record is still rejected by explicit full validation,
    // while a scoped write reads and validates its own origin only.
    fs::write(runs_path.join("unrelated.json"), b"malformed").map_err(|e| e.to_string())?;
    super::super::save_snapshot_from_run_store(&path, &mut list, &runs, &workspace,
        AgentHistorySnapshot { session_id: "ses-agent", messages: &messages })?;
    assert!(runs.all_records().is_err());
    assert!(store.get("../escape").is_err());
    assert!(store.get("absent")?.is_none());
    fs::create_dir(runs_path.join("not_a_file.json")).map_err(|e| e.to_string())?;
    assert!(store.get("not_a_file").is_err());
    println!("archive_target old_runs=10000 snapshots=60 prior_reads_per_tick={prior_reads} current_reads_total={} run_writes=0", after[0]-before[0]);
    fs::remove_dir_all(root).map_err(|e| e.to_string())?;
    Ok(())
}
