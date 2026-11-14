//! Picker-issued capabilities. Handles pin the selected file; callers cannot supply paths.
use serde::Serialize;
use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
pub(crate) const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const CHUNK_BYTES: u64 = 1024 * 1024;
const IDLE_TTL: Duration = Duration::from_secs(10 * 60);
const MAX_HANDLES: usize = 1024;

#[derive(Serialize)]
pub(crate) struct PickedFile {
    name: String,
    path: String,
    size: u64,
    token: String,
}
struct Selection {
    file: File,
    size: u64,
    touched: Instant,
}
#[derive(Default)]
struct Registry {
    files: HashMap<String, Selection>,
}
impl Registry {
    fn expire(&mut self, now: Instant) {
        self.files
            .retain(|_, file| now.saturating_duration_since(file.touched) < IDLE_TTL);
    }
    fn pick(&mut self, paths: Vec<PathBuf>, now: Instant) -> Result<Vec<PickedFile>, String> {
        self.expire(now);
        if self.files.len().saturating_add(paths.len()) > MAX_HANDLES {
            return Err("too many pending attachments".into());
        }
        // Stage the whole selection: a failure closes all newly opened handles.
        let mut staged = Vec::new();
        for path in paths {
            let file = File::open(&path).map_err(|_| "picked file unreadable")?;
            let meta = file.metadata().map_err(|_| "picked file missing")?;
            if !meta.is_file() {
                continue;
            }
            if meta.len() > MAX_FILE_BYTES {
                return Err("picked file too large".into());
            }
            let token = uuid::Uuid::new_v4().to_string();
            let picked = PickedFile {
                name: path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                path: path.to_string_lossy().into_owned(),
                size: meta.len(),
                token,
            };
            staged.push((
                picked,
                Selection {
                    file,
                    size: meta.len(),
                    touched: now,
                },
            ));
        }
        Ok(staged
            .into_iter()
            .map(|(picked, selection)| {
                self.files.insert(picked.token.clone(), selection);
                picked
            })
            .collect())
    }
    fn read(&mut self, token: &str, offset: u64, now: Instant) -> Result<Vec<u8>, String> {
        self.expire(now);
        let selected = self
            .files
            .get_mut(token)
            .ok_or("attachment selection expired")?;
        if offset > selected.size
            || selected.file.metadata().map_err(|_| "file missing")?.len() != selected.size
        {
            return Err("attachment changed".into());
        }
        selected.touched = now;
        let mut bytes = vec![0; (selected.size - offset).min(CHUNK_BYTES) as usize];
        selected
            .file
            .seek(SeekFrom::Start(offset))
            .map_err(|_| "file unreadable")?;
        selected
            .file
            .read_exact(&mut bytes)
            .map_err(|_| "attachment changed")?;
        if offset + bytes.len() as u64 == selected.size {
            self.files.remove(token);
        }
        Ok(bytes)
    }
}
fn registry() -> &'static Mutex<Registry> {
    static FILES: OnceLock<Mutex<Registry>> = OnceLock::new();
    FILES.get_or_init(|| {
        // Abandoned pages also release handles, even if no later picker command occurs.
        std::thread::spawn(|| loop {
            std::thread::sleep(Duration::from_secs(60));
            if let Some(files) = FILES.get() {
                if let Ok(mut files) = files.lock() {
                    files.expire(Instant::now());
                }
            }
        });
        Mutex::new(Registry::default())
    })
}
pub(crate) fn pick(paths: Vec<PathBuf>) -> Result<Vec<PickedFile>, String> {
    registry()
        .lock()
        .map_err(|_| "attachment registry unavailable")?
        .pick(paths, Instant::now())
}
pub(crate) fn read(token: &str, offset: u64) -> Result<Vec<u8>, String> {
    registry()
        .lock()
        .map_err(|_| "attachment registry unavailable")?
        .read(token, offset, Instant::now())
}
pub(crate) fn release(tokens: &[String]) -> Result<(), String> {
    let mut files = registry()
        .lock()
        .map_err(|_| "attachment registry unavailable")?;
    for token in tokens {
        files.files.remove(token);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chunks_reconstruct_maximum_file_and_release_the_handle() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("64m.bin");
        let original: Vec<u8> = (0..MAX_FILE_BYTES).map(|i| (i % 251) as u8).collect();
        std::fs::write(&path, &original).unwrap();
        let mut registry = Registry::default();
        let now = Instant::now();
        let picked = registry.pick(vec![path.clone()], now).unwrap().remove(0);
        assert!(registry.read(&path.to_string_lossy(), 0, now).is_err());
        let mut offset = 0;
        while offset < MAX_FILE_BYTES {
            let bytes = registry.read(&picked.token, offset, now).unwrap();
            assert!(bytes.len() <= CHUNK_BYTES as usize);
            assert_eq!(
                bytes,
                original[offset as usize..offset as usize + bytes.len()]
            );
            offset += bytes.len() as u64;
        }
        assert!(registry.files.is_empty());
        assert!(registry.read(&picked.token, 0, now).is_err());
    }
    #[test]
    fn rejects_growth_oversize_and_expired_selections_without_partial_registration() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("input");
        std::fs::write(&path, b"small").unwrap();
        let mut registry = Registry::default();
        let now = Instant::now();
        let token = registry
            .pick(vec![path.clone()], now)
            .unwrap()
            .remove(0)
            .token;
        std::fs::write(&path, b"changed").unwrap();
        assert!(registry.read(&token, 0, now).is_err());
        registry.expire(now + IDLE_TTL);
        assert!(registry.files.is_empty());
        assert!(registry.read(&token, 0, now + IDLE_TTL).is_err());
        let huge = directory.path().join("huge");
        File::create(&huge)
            .unwrap()
            .set_len(MAX_FILE_BYTES + 1)
            .unwrap();
        assert!(registry.pick(vec![path, huge], now).is_err());
        assert!(registry.files.is_empty());
    }
}
