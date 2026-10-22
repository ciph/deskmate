//! Windows primary ownership and conservative recovery of recorded sidecars.
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{GetLastError, ERROR_ALREADY_EXISTS, ERROR_INVALID_PARAMETER, FILETIME},
    System::Threading::{
        GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, WaitForSingleObject,
        PROCESS_QUERY_LIMITED_INFORMATION,
    },
    UI::WindowsAndMessaging::{
        FindWindowW, GetWindowThreadProcessId, SendMessageTimeoutW, SMTO_ABORTIFHUNG, SMTO_BLOCK,
        WM_COPYDATA,
    },
};

// Null security attributes produce a non-inheritable handle. Keep this one
// declaration local instead of enabling another windows-sys feature.
#[link(name = "kernel32")]
extern "system" {
    fn CreateMutexW(
        attributes: *const std::ffi::c_void,
        owner: i32,
        name: *const u16,
    ) -> *mut std::ffi::c_void;
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

fn named_handle(name: &str) -> io::Result<Option<OwnedHandle>> {
    let name = wide(name);
    // SAFETY: The terminated name lives through this call; null attributes mean
    // no inheritance. Existence, not thread ownership, is the launch boundary.
    let raw = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    let error = unsafe { GetLastError() };
    if raw.is_null() {
        return Err(io::Error::from_raw_os_error(error as i32));
    }
    // SAFETY: CreateMutexW returned one owned, valid, non-inherited handle.
    let handle = unsafe { OwnedHandle::from_raw_handle(raw) };
    Ok((error != ERROR_ALREADY_EXISTS).then_some(handle))
}

pub(crate) struct PrimaryGuard {
    _handle: OwnedHandle,
    identifier: String,
}

impl PrimaryGuard {
    pub(crate) fn acquire(identifier: &str) -> io::Result<Option<Self>> {
        use sha2::{Digest, Sha256};
        let name = format!(
            "Local\\YumePrimary-{:x}",
            Sha256::digest(identifier.as_bytes())
        );
        Ok(named_handle(&name)?.map(|handle| Self {
            _handle: handle,
            identifier: identifier.into(),
        }))
    }

    /// The pinned plugin may return success when an older instance owns its
    /// mutex but has not created its notification window. Never trust that gap.
    pub(crate) fn owns_plugin_window(&self) -> bool {
        let class = wide(&format!("{}-sic", self.identifier));
        let name = wide(&format!("{}-siw", self.identifier));
        // SAFETY: Both window names are terminated and remain alive.
        let window = unsafe { FindWindowW(class.as_ptr(), name.as_ptr()) };
        let mut pid = 0;
        // SAFETY: The PID output is initialized; a missing window returns zero.
        !window.is_null()
            && unsafe { GetWindowThreadProcessId(window, &mut pid) } != 0
            && pid == std::process::id()
    }

    pub(crate) fn cleanup(&self, data_dir: &Path) -> io::Result<usize> {
        let ProcessState::Running(host) = inspect(std::process::id()) else {
            return Err(io::Error::other("cannot verify current host identity"));
        };
        let directory = registry_dir(data_dir);
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
            Err(error) => return Err(error),
        };
        let mut reclaimed = 0;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json")
                || !entry.file_type()?.is_file()
                || entry.metadata()?.len() > 16 * 1024
            {
                continue;
            }
            let Some(record) = fs::read(&path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<LaunchRecord>(&bytes).ok())
            else {
                continue;
            };
            match decision(
                &record,
                &self.identifier,
                &host.executable,
                &inspect(record.host.pid),
                &inspect(record.sidecar.pid),
            ) {
                Decision::Keep => continue,
                Decision::RemoveRecord => {}
                Decision::Terminate => {
                    // The tree helper rechecks creation time on its retained
                    // root handle, so a PID reused after inspect is never killed.
                    if let Err(error) = crate::windows_process_tree::terminate_created(
                        record.sidecar.pid,
                        record.sidecar.created,
                    ) {
                        eprintln!("recorded sidecar recovery skipped: {error}");
                        continue;
                    }
                    reclaimed += 1;
                }
            }
            fs::remove_file(path)?;
        }
        Ok(reclaimed)
    }
}

/// Reuse the pinned single-instance plugin's notification protocol; wait only
/// for its window, and bound delivery if the original UI thread is hung.
pub(crate) fn notify_primary(identifier: &str) {
    #[repr(C)]
    struct CopyData {
        tag: usize,
        length: u32,
        data: *const u8,
    }
    let class = wide(&format!("{identifier}-sic"));
    let name = wide(&format!("{identifier}-siw"));
    let data = format!(
        "{}|{}\0",
        std::env::current_dir().unwrap_or_default().display(),
        std::env::args().collect::<Vec<_>>().join("|")
    );
    let Ok(length) = u32::try_from(data.len()) else {
        return;
    };
    let payload = CopyData {
        tag: 1542,
        length,
        data: data.as_ptr(),
    };
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        // SAFETY: Names and the WM_COPYDATA payload remain alive until the
        // synchronous send returns; user32 copies this message across processes.
        let window = unsafe { FindWindowW(class.as_ptr(), name.as_ptr()) };
        if !window.is_null() {
            unsafe {
                SendMessageTimeoutW(
                    window,
                    WM_COPYDATA,
                    0,
                    &payload as *const _ as isize,
                    SMTO_ABORTIFHUNG | SMTO_BLOCK,
                    500,
                    std::ptr::null_mut(),
                )
            };
            return;
        }
        if Instant::now() >= deadline {
            return;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct ProcessIdentity {
    pid: u32,
    created: u64,
    executable: String,
}

#[derive(Debug)]
enum ProcessState {
    Missing,
    Running(ProcessIdentity),
    Exited { pid: u32, created: u64 },
    Unknown,
}

fn inspect(pid: u32) -> ProcessState {
    // SAFETY: Query/synchronize rights only, with no handle inheritance.
    let raw = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | 0x0010_0000, 0, pid) };
    if raw.is_null() {
        let error = unsafe { GetLastError() };
        #[cfg(test)]
        eprintln!("inspect pid={pid}: OpenProcess error={error}");
        return if error == ERROR_INVALID_PARAMETER {
            ProcessState::Missing
        } else {
            ProcessState::Unknown
        };
    }
    // SAFETY: OpenProcess returned a valid owned handle.
    let handle = unsafe { OwnedHandle::from_raw_handle(raw) };
    let zero = || FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let (mut created, mut exited, mut kernel, mut user) = (zero(), zero(), zero(), zero());
    // SAFETY: The retained handle has query access and all outputs point to
    // independent initialized buffers of the required sizes.
    if unsafe { GetProcessTimes(raw, &mut created, &mut exited, &mut kernel, &mut user) } == 0 {
        #[cfg(test)]
        eprintln!("inspect pid={pid}: process times query error={}", unsafe {
            GetLastError()
        });
        return ProcessState::Unknown;
    }
    let created = u64::from(created.dwLowDateTime) | (u64::from(created.dwHighDateTime) << 32);
    // SAFETY: The same retained process handle grants synchronize rights.
    match unsafe { WaitForSingleObject(handle.as_raw_handle(), 0) } {
        // Windows can unload a dead process's image while retaining its kernel
        // object. Birth time still identifies that object; querying its image
        // would fail with ERROR_GEN_FAILURE and lose proven exit evidence.
        0 => return ProcessState::Exited { pid, created },
        258 => {}
        _ => return ProcessState::Unknown,
    }
    let mut image = vec![0; 32768];
    let mut size = image.len() as u32;
    // SAFETY: The retained query handle and initialized image buffer are valid.
    if unsafe { QueryFullProcessImageNameW(raw, 0, image.as_mut_ptr(), &mut size) } == 0 {
        #[cfg(test)]
        eprintln!("inspect pid={pid}: process image query error={}", unsafe {
            GetLastError()
        });
        return ProcessState::Unknown;
    }
    let Ok(executable) = String::from_utf16(&image[..size as usize]) else {
        return ProcessState::Unknown;
    };
    ProcessState::Running(ProcessIdentity {
        pid,
        created,
        executable,
    })
}

#[derive(Serialize, Deserialize)]
struct LaunchRecord {
    version: u8,
    identifier: String,
    host: ProcessIdentity,
    sidecar: ProcessIdentity,
}

#[derive(Debug, PartialEq, Eq)]
enum Decision {
    Keep,
    RemoveRecord,
    Terminate,
}

fn decision(
    record: &LaunchRecord,
    identifier: &str,
    host_executable: &str,
    host: &ProcessState,
    sidecar: &ProcessState,
) -> Decision {
    if record.version != 1
        || record.identifier != identifier
        || record.host.executable != host_executable
        || record.host.pid == 0
        || record.sidecar.pid == 0
        || record.host.pid == record.sidecar.pid
        || record.sidecar.pid == std::process::id()
        || record.host.created == 0
        || record.sidecar.created < record.host.created
        || record.sidecar.executable.is_empty()
    {
        return Decision::Keep;
    }
    let host_dead = matches!(host, ProcessState::Missing)
        || matches!(host, ProcessState::Exited { pid, created } if *pid == record.host.pid && *created == record.host.created);
    if !host_dead {
        return Decision::Keep;
    }
    match sidecar {
        ProcessState::Missing => Decision::RemoveRecord,
        ProcessState::Exited { pid, created }
            if *pid == record.sidecar.pid && *created == record.sidecar.created =>
        {
            Decision::RemoveRecord
        }
        ProcessState::Running(identity) if identity == &record.sidecar => Decision::Terminate,
        _ => Decision::Keep,
    }
}

fn registry_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("sidecar-ownership")
}
fn record_path(data_dir: &Path, pid: u32) -> PathBuf {
    registry_dir(data_dir).join(format!("{pid}.json"))
}

pub(crate) fn record_child(identifier: &str, data_dir: &Path, child_pid: u32) -> io::Result<()> {
    let ProcessState::Running(host) = inspect(std::process::id()) else {
        return Err(io::Error::other("cannot identify sidecar host"));
    };
    let ProcessState::Running(sidecar) = inspect(child_pid) else {
        return Err(io::Error::other("cannot identify sidecar process"));
    };
    let record = LaunchRecord {
        version: 1,
        identifier: identifier.into(),
        host,
        sidecar,
    };
    fs::create_dir_all(registry_dir(data_dir))?;
    let path = record_path(data_dir, child_pid);
    let temporary = path.with_extension(format!("{}.pending", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(&serde_json::to_vec(&record)?)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

pub(crate) fn forget_child(data_dir: &Path, child_pid: u32) {
    if let Err(error) = fs::remove_file(record_path(data_dir, child_pid)) {
        if error.kind() != io::ErrorKind::NotFound {
            eprintln!("sidecar ownership cleanup failed: {error}");
        }
    }
}

#[cfg(test)]
#[path = "windows_startup_tests.rs"]
mod tests;
