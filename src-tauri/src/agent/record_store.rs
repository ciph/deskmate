use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RunOutcome {
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunRecord {
    pub(crate) run_id: String,
    pub(crate) session_id: Option<String>,
    pub(crate) workspace_path: PathBuf,
    pub(crate) created_at: String,
    pub(crate) ended_at: Option<String>,
    pub(crate) outcome: Option<RunOutcome>,
    pub(crate) error_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) pending_outcome: Option<RunOutcome>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) pending_error_summary: Option<String>,
    #[serde(default)]
    pub(crate) message_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) pending_usage_message_ids: Vec<String>,
    #[serde(default)]
    pub(crate) part_ids: Vec<String>,
    #[serde(default)]
    pub(crate) call_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) initial_input: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunListing {
    pub(crate) active: Option<RunRecord>,
    pub(crate) recent: Vec<RunRecord>,
    pub(crate) artifacts: Vec<super::artifacts::AgentArtifact>,
}

#[derive(Clone, Debug)]
pub(crate) struct NativePart {
    pub(crate) file_name: Option<String>,
    pub(crate) id: String,
    pub(crate) synthetic: bool,
    pub(crate) kind: Option<String>,
    pub(crate) text: Option<String>,
    pub(crate) call_id: Option<String>,
    pub(crate) tool: Option<String>,
    pub(crate) state: Option<NativeToolState>,
}

#[derive(Clone, Debug)]
pub(crate) struct NativeToolState {
    pub(crate) status: String,
    pub(crate) input: serde_json::Value,
    pub(crate) output: serde_json::Value,
    pub(crate) metadata: serde_json::Value,
}

#[derive(Clone, Debug)]
pub(crate) struct NativeMessage {
    pub(crate) usage: Option<crate::ai_usage::UsageObservation>,
    pub(crate) system: Option<String>,
    pub(crate) id: String,
    pub(crate) role: Option<String>,
    pub(crate) created: Option<u64>,
    pub(crate) parent_id: Option<String>,
    pub(crate) completed: bool,
    pub(crate) finish: Option<String>,
    pub(crate) error: Option<String>,
    pub(crate) parts: Vec<NativePart>,
}

#[cfg(test)]
#[derive(Default)]
struct StoreIo {
    fail: std::sync::atomic::AtomicBool,
    reads: std::sync::atomic::AtomicUsize,
    serializations: std::sync::atomic::AtomicUsize,
    syncs: std::sync::atomic::AtomicUsize,
    renames: std::sync::atomic::AtomicUsize,
}

#[derive(Clone)]
pub(crate) struct RunStore(
    PathBuf,
    #[cfg(test)] std::sync::Arc<StoreIo>,
);

impl RunStore {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self(
            root,
            #[cfg(test)]
            std::sync::Arc::new(StoreIo::default()),
        )
    }

    #[cfg(test)]
    pub(crate) fn inject_write_failure(&self, fail: bool) {
        self.1.fail.store(fail, std::sync::atomic::Ordering::SeqCst);
    }

    #[cfg(test)]
    pub(crate) fn io_counts(&self) -> [usize; 4] {
        use std::sync::atomic::Ordering::SeqCst;
        [self.1.reads.load(SeqCst), self.1.serializations.load(SeqCst),
         self.1.syncs.load(SeqCst), self.1.renames.load(SeqCst)]
    }

    pub(crate) fn load(&self) -> Result<Vec<RunRecord>, String> {
        if !self.0.exists() {
            return Ok(Vec::new());
        }
        let mut records = Vec::new();
        for entry in fs::read_dir(&self.0).map_err(|_| "agent_storage_unavailable")? {
            let path = entry.map_err(|_| "agent_storage_unavailable")?.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            #[cfg(test)]
            self.1.reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let bytes = fs::read(path).map_err(|_| "agent_storage_unavailable")?;
            records.push(serde_json::from_slice(&bytes).map_err(|_| "agent_record_invalid")?);
        }
        records.sort_by(|left: &RunRecord, right| right.created_at.cmp(&left.created_at));
        Ok(records)
    }

    pub(crate) fn get(&self, run_id: &str) -> Result<Option<RunRecord>, String> {
        if !crate::worklog::bridge::safe_id(run_id) {
            return Err("agent_record_invalid".into());
        }
        let path = self.0.join(format!("{run_id}.json"));
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err("agent_storage_unavailable".into()),
        };
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err("agent_record_invalid".into());
        }
        #[cfg(test)]
        self.1.reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let bytes = fs::read(path).map_err(|_| "agent_storage_unavailable")?;
        let record: RunRecord = serde_json::from_slice(&bytes).map_err(|_| "agent_record_invalid")?;
        if record.run_id != run_id {
            return Err("agent_record_invalid".into());
        }
        Ok(Some(record))
    }

    pub(crate) fn write(&self, record: &RunRecord) -> Result<(), String> {
        #[cfg(test)]
        if self.1.fail.load(std::sync::atomic::Ordering::SeqCst) {
            return Err("agent_storage_unavailable".into());
        }
        fs::create_dir_all(&self.0).map_err(|_| "agent_storage_unavailable")?;
        #[cfg(test)]
        self.1.serializations.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let bytes = serde_json::to_vec_pretty(record).map_err(|_| "agent_record_invalid")?;
        let pending = self
            .0
            .join(format!(".{}-{}.tmp", record.run_id, uuid::Uuid::new_v4()));
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&pending)
            .map_err(|_| "agent_storage_unavailable")?;
        #[cfg(test)]
        self.1.syncs.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| "agent_storage_unavailable")?;
        drop(file);
        let target = self.0.join(format!("{}.json", record.run_id));
        #[cfg(test)]
        self.1.renames.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if fs::rename(&pending, &target).is_err() {
            let _ = fs::remove_file(&pending);
            return Err("agent_storage_unavailable".into());
        }
        Ok(())
    }
}
