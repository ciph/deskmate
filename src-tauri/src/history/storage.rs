use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

use super::HistorySession;

pub(super) fn load_path(path: &Path) -> Result<Vec<HistorySession>, String> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| "history_invalid".to_owned()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(_) => Err("history_storage_unavailable".to_owned()),
    }
}

pub(super) struct PreparedHistoryFile {
    pending: std::path::PathBuf,
    target: std::path::PathBuf,
    committed: bool,
}

impl PreparedHistoryFile {
    pub(super) fn commit(self) -> Result<(), String> {
        self.commit_with(replace_file)
    }

    pub(super) fn commit_with(
        mut self,
        replace: impl FnOnce(&Path, &Path) -> Result<(), String>,
    ) -> Result<(), String> {
        replace(&self.pending, &self.target)?;
        self.committed = true;
        Ok(())
    }
}

impl Drop for PreparedHistoryFile {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_file(&self.pending);
        }
    }
}

pub(super) fn prepare_path(
    path: &Path,
    list: &[HistorySession],
) -> Result<PreparedHistoryFile, String> {
    prepare_path_with_write(path, list, |file, bytes| {
        file.write_all(bytes).and_then(|_| file.sync_all())
    })
}

pub(super) fn prepare_path_with_write(
    path: &Path,
    list: &[HistorySession],
    write_and_sync: impl FnOnce(&mut fs::File, &[u8]) -> std::io::Result<()>,
) -> Result<PreparedHistoryFile, String> {
    let parent = path
        .parent()
        .ok_or_else(|| "history_storage_unavailable".to_owned())?;
    fs::create_dir_all(parent).map_err(|_| "history_storage_unavailable")?;
    let bytes = serde_json::to_vec_pretty(list).map_err(|_| "history_invalid")?;
    let pending = parent.join(format!(".history-{}.tmp", uuid::Uuid::new_v4()));
    let file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&pending)
        .map_err(|_| "history_storage_unavailable")?;
    let prepared = PreparedHistoryFile {
        pending,
        target: path.to_owned(),
        committed: false,
    };
    // The handle closes before cleanup, including when an injected writer panics.
    let written = {
        let mut file = file;
        write_and_sync(&mut file, &bytes)
    };
    written.map_err(|_| "history_storage_unavailable".to_owned())?;
    Ok(prepared)
}

pub(super) fn persist_path(path: &Path, list: &[HistorySession]) -> Result<(), String> {
    persist_path_with(path, list, replace_file)
}

pub(super) fn persist_path_with(
    path: &Path,
    list: &[HistorySession],
    replace: impl FnOnce(&Path, &Path) -> Result<(), String>,
) -> Result<(), String> {
    prepare_path(path, list)?.commit_with(replace)
}

#[cfg(windows)]
fn replace_file(pending: &Path, target: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt as _;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let pending_wide: Vec<u16> = pending.as_os_str().encode_wide().chain(Some(0)).collect();
    let target_wide: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: Category 8 (FFI boundary). Both pointers reference live,
    // NUL-terminated UTF-16 buffers for the duration of this synchronous call;
    // the Windows API only reads them and accepts this documented flag pair.
    let moved = unsafe {
        MoveFileExW(
            pending_wide.as_ptr(),
            target_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    (moved != 0)
        .then_some(())
        .ok_or_else(|| "history_storage_unavailable".to_owned())
}

#[cfg(not(windows))]
fn replace_file(pending: &Path, target: &Path) -> Result<(), String> {
    fs::rename(pending, target).map_err(|_| "history_storage_unavailable".to_owned())
}
