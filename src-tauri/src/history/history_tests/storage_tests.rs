use super::{ordinary, temp_file};
use crate::history::{
    load_path,
    storage::{persist_path, persist_path_with, prepare_path, prepare_path_with_write},
    HistorySession,
};
use std::{fs, io::Write, path::Path};

#[test]
fn old_history_json_remains_readable() -> Result<(), String> {
    let json = r#"[{"id":"ses-old","title":"Old chat","created":1,"updated":2,"messages":[{"role":"user","text":"hello","time":1}]}]"#;
    let sessions: Vec<HistorySession> =
        serde_json::from_str(json).map_err(|error| error.to_string())?;
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, "ses-old");
    assert_eq!(sessions[0].messages[0].text, "hello");
    assert!(sessions[0].origin_run_id.is_none());
    assert!(!sessions[0].deleted);
    Ok(())
}

#[test]
fn corrupt_json_surfaces_and_is_not_overwritten() -> Result<(), String> {
    let path = temp_file("corrupt");
    fs::create_dir_all(path.parent().ok_or_else(|| "missing parent".to_owned())?)
        .map_err(|error| error.to_string())?;
    fs::write(&path, b"{broken").map_err(|error| error.to_string())?;
    assert_eq!(load_path(&path), Err("history_invalid".to_owned()));
    assert_eq!(
        fs::read(&path).map_err(|error| error.to_string())?,
        b"{broken"
    );
    fs::remove_dir_all(path.parent().ok_or_else(|| "missing parent".to_owned())?)
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[test]
fn atomic_replace_overwrites_existing_history_without_temp_files() -> Result<(), String> {
    let path = temp_file("replace-existing");
    let parent = path.parent().ok_or_else(|| "missing parent".to_owned())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    persist_path(&path, &[ordinary("old")])?;
    persist_path(&path, &[ordinary("new")])?;
    assert_eq!(load_path(&path)?[0].id, "new");
    assert_eq!(
        fs::read_dir(parent)
            .map_err(|error| error.to_string())?
            .count(),
        1
    );
    fs::remove_dir_all(parent).map_err(|error| error.to_string())?;
    Ok(())
}

#[test]
fn failed_replace_preserves_old_file_and_memory() -> Result<(), String> {
    let path = temp_file("write-failure");
    let parent = path.parent().ok_or_else(|| "missing parent".to_owned())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    fs::write(&path, b"old bytes").map_err(|error| error.to_string())?;
    let list = vec![ordinary("new")];
    assert_eq!(
        persist_path_with(&path, &list, |_, _| Err(
            "injected_replace_failure".to_owned()
        )),
        Err("injected_replace_failure".to_owned())
    );
    assert_eq!(
        fs::read(&path).map_err(|error| error.to_string())?,
        b"old bytes"
    );
    assert_eq!(list[0].id, "new");
    assert_eq!(
        fs::read_dir(parent)
            .map_err(|error| error.to_string())?
            .count(),
        1
    );
    fs::remove_dir_all(parent).map_err(|error| error.to_string())?;
    Ok(())
}

fn pending_files(path: &Path) -> Result<Vec<std::path::PathBuf>, String> {
    fs::read_dir(path.parent().ok_or_else(|| "missing parent".to_owned())?)
        .map_err(|error| error.to_string())?
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(|error| error.to_string())
        })
        .filter(|entry| entry.as_ref().map_or(true, |entry| entry != path))
        .collect()
}

#[test]
fn prepared_history_waits_for_commit_before_replacing_target() -> Result<(), String> {
    let path = temp_file("prepare-commit");
    persist_path(&path, &[ordinary("old")])?;
    let prepared = prepare_path(&path, &[ordinary("new")])?;
    assert_eq!(load_path(&path)?[0].id, "old");
    let pending = pending_files(&path)?;
    assert_eq!(pending.len(), 1);
    assert_eq!(load_path(&pending[0])?[0].id, "new");
    prepared.commit()?;
    assert_eq!(load_path(&path)?[0].id, "new");
    assert!(pending_files(&path)?.is_empty());
    fs::remove_dir_all(path.parent().ok_or_else(|| "missing parent".to_owned())?)
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[test]
fn abandoning_prepared_history_removes_only_its_temporary_file() -> Result<(), String> {
    let path = temp_file("prepare-abandon");
    persist_path(&path, &[ordinary("old")])?;
    let prepared = prepare_path(&path, &[ordinary("abandoned")])?;
    let concurrent = prepare_path(&path, &[ordinary("concurrent")])?;
    assert_eq!(pending_files(&path)?.len(), 2);
    drop(prepared);
    assert_eq!(load_path(&path)?[0].id, "old");
    let pending = pending_files(&path)?;
    assert_eq!(pending.len(), 1);
    assert_eq!(load_path(&pending[0])?[0].id, "concurrent");
    concurrent.commit()?;
    assert_eq!(load_path(&path)?[0].id, "concurrent");
    assert!(pending_files(&path)?.is_empty());
    fs::remove_dir_all(path.parent().ok_or_else(|| "missing parent".to_owned())?)
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[test]
fn failed_prepared_commit_preserves_target_and_removes_temporary_file() -> Result<(), String> {
    let path = temp_file("prepare-failed-commit");
    persist_path(&path, &[ordinary("old")])?;
    let previous_bytes = fs::read(&path).map_err(|error| error.to_string())?;
    let prepared = prepare_path(&path, &[ordinary("new")])?;
    assert_eq!(pending_files(&path)?.len(), 1);
    assert_eq!(
        prepared.commit_with(|_, _| Err("injected_replace_failure".to_owned())),
        Err("injected_replace_failure".to_owned())
    );
    assert_eq!(
        fs::read(&path).map_err(|error| error.to_string())?,
        previous_bytes
    );
    assert!(pending_files(&path)?.is_empty());
    fs::remove_dir_all(path.parent().ok_or_else(|| "missing parent".to_owned())?)
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[test]
fn failed_prepare_write_or_sync_preserves_target_and_cleans_up() -> Result<(), String> {
    for fail_after_full_write in [false, true] {
        let path = temp_file("prepare-failed-write-sync");
        persist_path(&path, &[ordinary("old")])?;
        let previous_bytes = fs::read(&path).map_err(|error| error.to_string())?;
        let result = prepare_path_with_write(&path, &[ordinary("new")], |file, bytes| {
            let written = if fail_after_full_write {
                bytes.len()
            } else {
                bytes.len() / 2
            };
            file.write_all(&bytes[..written])?;
            Err(std::io::Error::other(if fail_after_full_write {
                "injected_sync_failure"
            } else {
                "injected_write_failure"
            }))
        });
        assert_eq!(result.err(), Some("history_storage_unavailable".to_owned()));
        assert_eq!(
            fs::read(&path).map_err(|error| error.to_string())?,
            previous_bytes
        );
        assert!(pending_files(&path)?.is_empty());
        fs::remove_dir_all(path.parent().ok_or_else(|| "missing parent".to_owned())?)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
