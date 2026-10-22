use crate::settings::SettingsState;
use chrono::{DateTime, Local, TimeZone};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};
use tauri::{Manager, State};

const LEDGER_FILE: &str = "ai-usage-local.json";
static LEDGER_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
const CAPTURE_CACHE_LIMIT: usize = 4096;
static CAPTURED_AGENT_USAGE: OnceLock<Mutex<UsageCaptureCache>> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UsageObservation {
    pub(crate) message_id: String,
    pub(crate) sidecar_id: String,
    pub(crate) model_id: String,
    pub(crate) created_at_ms: i64,
    pub(crate) tokens: u64,
}

impl UsageObservation {
    pub(crate) fn is_valid(&self) -> bool {
        !self.message_id.is_empty()
            && self.message_id.len() <= 200
            && !self.sidecar_id.is_empty()
            && self.sidecar_id.len() <= 200
            && !self.model_id.is_empty()
            && self.model_id.len() <= 200
            && self.tokens <= 100_000_000
            && self.created_at_ms > 0
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct UsageCaptureKey {
    root: PathBuf,
    run_id: String,
    workspace: PathBuf,
    session_id: String,
    run_created_at: String,
    message_id: String,
    provider_id: String,
}

#[derive(Default)]
struct UsageCaptureCache {
    entries: HashMap<UsageCaptureKey, UsageObservation>,
    order: VecDeque<UsageCaptureKey>,
}

impl UsageCaptureCache {
    fn remember(&mut self, key: UsageCaptureKey, observation: UsageObservation) {
        if !self.entries.contains_key(&key) {
            if self.entries.len() == CAPTURE_CACHE_LIMIT {
                if let Some(oldest) = self.order.pop_front() {
                    self.entries.remove(&oldest);
                }
            }
            self.order.push_back(key.clone());
        }
        self.entries.insert(key, observation);
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct StoredUsage {
    provider_id: String,
    model_id: String,
    created_at_ms: i64,
    tokens: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalUsageModel {
    name: String,
    tokens: u64,
    requests: u64,
}

#[derive(Default)]
pub(crate) struct LocalSummary {
    pub(crate) tokens: u64,
    pub(crate) requests: u64,
    pub(crate) top_models: Vec<LocalUsageModel>,
}

fn ledger_lock() -> Result<std::sync::MutexGuard<'static, ()>, String> {
    LEDGER_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| "usage_storage_unavailable".into())
}

fn read_ledger(root: &Path) -> Result<HashMap<String, StoredUsage>, String> {
    match fs::read(root.join(LEDGER_FILE)) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| "usage_storage_invalid".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(HashMap::new()),
        Err(_) => Err("usage_storage_unavailable".into()),
    }
}

fn save_ledger(root: &Path, entries: &HashMap<String, StoredUsage>) -> Result<(), String> {
    fs::create_dir_all(root).map_err(|_| "usage_storage_unavailable")?;
    let pending = root.join(format!(".{LEDGER_FILE}-{}.tmp", uuid::Uuid::new_v4()));
    let bytes = serde_json::to_vec(entries).map_err(|_| "usage_storage_invalid")?;
    let result = (|| {
        let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&pending)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&pending, root.join(LEDGER_FILE))
    })();
    if result.is_err() { let _ = fs::remove_file(&pending); }
    result.map_err(|_| "usage_storage_unavailable".into())
}

fn save_observations(
    root: &Path,
    providers: &[crate::settings::AiProvider],
    observations: impl IntoIterator<Item = UsageObservation>,
) -> Result<(), String> {
    let _guard = ledger_lock()?;
    let mut entries = read_ledger(root)?;
    let mut changed = false;
    for observation in observations {
        if !observation.is_valid() {
            continue;
        }
        let Some(provider) = providers
            .iter()
            .find(|provider| provider.sidecar_id == observation.sidecar_id)
        else {
            continue;
        };
        let next = StoredUsage {
            provider_id: provider.id.clone(),
            model_id: observation.model_id,
            created_at_ms: observation.created_at_ms,
            tokens: observation.tokens,
        };
        if entries.get(&observation.message_id).is_some_and(|existing| {
            existing.provider_id != next.provider_id
                || existing.model_id != next.model_id
                || existing.created_at_ms != next.created_at_ms
        }) {
            return Err("usage_message_identity_mismatch".into());
        }
        if entries
            .get(&observation.message_id)
            .is_none_or(|existing| existing.tokens < next.tokens)
        {
            entries.insert(observation.message_id, next);
            changed = true;
        }
    }
    if changed {
        save_ledger(root, &entries)?;
    }
    Ok(())
}

pub(crate) fn today_summary(root: &Path, provider_id: &str) -> Result<LocalSummary, String> {
    let _guard = ledger_lock()?;
    let today = Local::now().date_naive();
    let mut summary = LocalSummary {
        tokens: 0,
        requests: 0,
        top_models: Vec::new(),
    };
    let mut models: HashMap<String, (u64, u64)> = HashMap::new();
    for entry in read_ledger(root)?.values() {
        let created: Option<DateTime<Local>> =
            Local.timestamp_millis_opt(entry.created_at_ms).single();
        if entry.provider_id != provider_id || created.is_none_or(|time| time.date_naive() != today)
        {
            continue;
        }
        summary.tokens = summary.tokens.saturating_add(entry.tokens);
        summary.requests += 1;
        let model = models.entry(entry.model_id.clone()).or_default();
        model.0 = model.0.saturating_add(entry.tokens);
        model.1 += 1;
    }
    summary.top_models = models
        .into_iter()
        .map(|(name, (tokens, requests))| LocalUsageModel {
            name,
            tokens,
            requests,
        })
        .collect();
    summary.top_models.sort_by(|a, b| b.tokens.cmp(&a.tokens));
    summary.top_models.truncate(3);
    Ok(summary)
}

#[tauri::command]
pub(crate) async fn record_ai_usage(
    app: tauri::AppHandle,
    state: State<'_, SettingsState>,
    observation: UsageObservation,
) -> Result<(), String> {
    let providers = state
        .0
        .lock()
        .map_err(|_| "settings_state")?
        .providers
        .clone();
    let root = app
        .path()
        .app_data_dir()
        .map_err(|_| "usage_storage_unavailable")?;
    tauri::async_runtime::spawn_blocking(move || {
        save_observations(&root, &providers, [observation])
    })
    .await
    .map_err(|_| "usage_storage_unavailable".to_string())?
}

fn capture_agent_usage_with(
    root: &Path,
    providers: &[crate::settings::AiProvider],
    record: &crate::agent::RunRecord,
    snapshot: &[crate::agent::NativeMessage],
    cache: &mut UsageCaptureCache,
    save: impl FnOnce(Vec<UsageObservation>) -> Result<(), String>,
) -> Result<Vec<String>, String> {
    let session_id = record.session_id.as_deref().ok_or("usage_session_unknown")?;
    let mut missing = Vec::new();
    let mut pending: HashMap<UsageCaptureKey, UsageObservation> = HashMap::new();
    for message in snapshot.iter().filter(|message| {
        message.parent_id.as_deref() == Some(record.run_id.as_str())
            && message.role.as_deref() == Some("assistant")
            && message.completed
    }) {
        let observation = message.usage.as_ref().filter(|observation| {
            observation.is_valid()
                && observation.message_id == message.id
                && message.created.and_then(|created| i64::try_from(created).ok())
                    == Some(observation.created_at_ms)
        });
        let provider = observation.and_then(|observation| {
            providers.iter().find(|provider| provider.sidecar_id == observation.sidecar_id)
        });
        let (Some(observation), Some(provider)) = (observation, provider) else {
            if !missing.contains(&message.id) {
                missing.push(message.id.clone());
            }
            continue;
        };
        let key = UsageCaptureKey {
            root: root.to_path_buf(),
            run_id: record.run_id.clone(),
            workspace: record.workspace_path.clone(),
            session_id: session_id.to_owned(),
            run_created_at: record.created_at.clone(),
            message_id: message.id.clone(),
            provider_id: provider.id.clone(),
        };
        if cache.entries.get(&key) != Some(observation)
            && pending.get(&key).is_none_or(|existing| existing.tokens < observation.tokens)
        {
            pending.insert(key, observation.clone());
        }
    }
    if !pending.is_empty() {
        save(pending.values().cloned().collect())?;
        // Failed persistence must leave every observation eligible for retry.
        for (key, observation) in pending {
            cache.remember(key, observation);
        }
    }
    Ok(missing)
}

pub(crate) fn capture_agent_usage(
    app: &tauri::AppHandle,
    record: &crate::agent::RunRecord,
    snapshot: &[crate::agent::NativeMessage],
) -> Result<Vec<String>, String> {
    let providers = app
        .state::<SettingsState>()
        .0
        .lock()
        .map_err(|_| "settings_state")?
        .providers
        .clone();
    let root = app.path().app_data_dir().map_err(|_| "usage_storage_unavailable")?;
    let mut cache = CAPTURED_AGENT_USAGE
        .get_or_init(|| Mutex::new(UsageCaptureCache::default()))
        .lock()
        .map_err(|_| "usage_storage_unavailable")?;
    capture_agent_usage_with(&root, &providers, record, snapshot, &mut cache, |observations| {
        save_observations(&root, &providers, observations)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::AiProvider;

    fn agent_fixture() -> (PathBuf, AiProvider, crate::agent::RunRecord, crate::agent::NativeMessage) {
        let root = std::env::temp_dir().join(format!("yume-agent-usage-{}", uuid::Uuid::new_v4()));
        let provider = AiProvider {
            id: "provider".into(), sidecar_id: "native-provider".into(), label: "Synthetic".into(),
            base_url: "https://example.invalid".into(), manual_model_ids: String::new(), api_key: String::new(),
        };
        let record = crate::agent::RunRecord {
            run_id: "run".into(), session_id: Some("session".into()), workspace_path: root.join("workspace"),
            created_at: "2026-10-22T00:00:00Z".into(), ended_at: None, outcome: None,
            error_summary: None, pending_outcome: None, pending_error_summary: None,
            message_ids: vec![], pending_usage_message_ids: vec![], part_ids: vec![], call_ids: vec![], initial_input: None,
        };
        let created = Local::now().timestamp_millis();
        let message = crate::agent::NativeMessage {
            system: None, id: "answer".into(), role: Some("assistant".into()), created: Some(created as u64),
            parent_id: Some(record.run_id.clone()), completed: true, finish: Some("stop".into()), error: None,
            parts: vec![], usage: Some(UsageObservation {
                message_id: "answer".into(), sidecar_id: provider.sidecar_id.clone(), model_id: "model".into(),
                created_at_ms: created, tokens: 20,
            }),
        };
        (root, provider, record, message)
    }

    #[test]
    fn snapshot_usage_is_local_and_replay_skips_ledger_access() {
        let (root, provider, record, mut message) = agent_fixture();
        let providers = [provider];
        let mut cache = UsageCaptureCache::default();
        let missing = capture_agent_usage_with(&root, &providers, &record, &[message.clone()], &mut cache,
            |observations| save_observations(&root, &providers, observations)).unwrap();
        assert!(missing.is_empty());
        capture_agent_usage_with(&root, &providers, &record, &[message.clone()], &mut cache,
            |_| panic!("an unchanged snapshot must not read or write the ledger")).unwrap();
        message.usage.as_mut().unwrap().tokens = 30;
        capture_agent_usage_with(&root, &providers, &record, &[message], &mut cache,
            |observations| save_observations(&root, &providers, observations)).unwrap();
        let summary = today_summary(&root, "provider").unwrap();
        assert_eq!((summary.tokens, summary.requests), (30, 1));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_or_invalid_usage_and_unknown_providers_require_compensation() {
        let (root, provider, record, message) = agent_fixture();
        let mut missing = message.clone();
        missing.id = "missing".into(); missing.usage = None;
        let mut unknown = message.clone();
        unknown.id = "unknown".into();
        unknown.usage.as_mut().unwrap().message_id = unknown.id.clone();
        unknown.usage.as_mut().unwrap().sidecar_id = "unknown-provider".into();
        let mut wrong_message = message.clone();
        wrong_message.id = "wrong-message".into();
        let mut wrong_created = message.clone();
        wrong_created.id = "wrong-created".into();
        wrong_created.usage.as_mut().unwrap().message_id = wrong_created.id.clone();
        wrong_created.usage.as_mut().unwrap().created_at_ms += 1;
        let mut unfinished = missing.clone(); unfinished.id = "unfinished".into(); unfinished.completed = false;
        let mut foreign = missing.clone(); foreign.id = "foreign".into(); foreign.parent_id = Some("other-run".into());
        let mut user = missing.clone(); user.id = "user".into(); user.role = Some("user".into());
        let snapshot = [missing.clone(), missing, unknown, wrong_message, wrong_created, unfinished, foreign, user];
        let ids = capture_agent_usage_with(&root, &[provider], &record, &snapshot, &mut UsageCaptureCache::default(),
            |_| panic!("ineligible usage must never be persisted")).unwrap();
        assert_eq!(ids, ["missing", "unknown", "wrong-message", "wrong-created"]);
    }

    #[test]
    fn failed_storage_does_not_cache_and_can_retry() {
        let (root, provider, record, message) = agent_fixture();
        let providers = [provider];
        let snapshot = [message];
        let mut cache = UsageCaptureCache::default();
        fs::write(&root, b"blocks ledger directory").unwrap();
        assert!(capture_agent_usage_with(&root, &providers, &record, &snapshot, &mut cache,
            |observations| save_observations(&root, &providers, observations)).is_err());
        assert!(cache.entries.is_empty());
        fs::remove_file(&root).unwrap();
        capture_agent_usage_with(&root, &providers, &record, &snapshot, &mut cache,
            |observations| save_observations(&root, &providers, observations)).unwrap();
        assert_eq!(cache.entries.len(), 1);
        assert_eq!(today_summary(&root, "provider").unwrap().tokens, 20);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn conflicting_persisted_identity_is_never_cached_as_saved() {
        let (root, provider, record, message) = agent_fixture();
        let providers = [provider];
        let mut cache = UsageCaptureCache::default();
        capture_agent_usage_with(&root, &providers, &record, &[message.clone()], &mut cache,
            |observations| save_observations(&root, &providers, observations)).unwrap();
        let mut conflicting = message;
        conflicting.usage.as_mut().unwrap().model_id = "other-model".into();
        let result = capture_agent_usage_with(&root, &providers, &record, &[conflicting], &mut cache,
            |observations| save_observations(&root, &providers, observations));
        assert_eq!(result, Err("usage_message_identity_mismatch".into()));
        assert_eq!(cache.entries.values().next().unwrap().model_id, "model");
        assert_eq!(today_summary(&root, "provider").unwrap().top_models[0].name, "model");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn duplicate_snapshot_rows_preserve_highest_observed_tokens() {
        let (root, provider, record, mut high) = agent_fixture();
        let mut low = high.clone();
        high.usage.as_mut().unwrap().tokens = 30;
        low.usage.as_mut().unwrap().tokens = 10;
        let mut cache = UsageCaptureCache::default();
        let captured = std::cell::RefCell::new(Vec::new());
        capture_agent_usage_with(&root, &[provider], &record, &[high, low], &mut cache,
            |observations| { *captured.borrow_mut() = observations; Ok(()) }).unwrap();
        assert_eq!(captured.borrow().len(), 1);
        assert_eq!(captured.borrow()[0].tokens, 30);
    }

    #[test]
    fn capture_cache_uses_complete_scope_and_current_provider_mapping() {
        let (root, provider, record, message) = agent_fixture();
        let mut cache = UsageCaptureCache::default();
        let saves = std::cell::Cell::new(0);
        for field in 0..5 {
            let mut candidate = record.clone();
            let mut answer = message.clone();
            match field {
                1 => candidate.workspace_path = root.join("other-workspace"),
                2 => candidate.session_id = Some("other-session".into()),
                3 => candidate.created_at = "2026-10-22T00:00:01Z".into(),
                4 => { candidate.run_id = "other-run".into(); answer.parent_id = Some(candidate.run_id.clone()); },
                _ => {},
            }
            capture_agent_usage_with(&root, &[provider.clone()], &candidate, &[answer], &mut cache,
                |_| { saves.set(saves.get() + 1); Ok(()) }).unwrap();
        }
        capture_agent_usage_with(&root.join("other-data"), &[provider.clone()], &record, &[message.clone()], &mut cache,
            |_| { saves.set(saves.get() + 1); Ok(()) }).unwrap();
        let mut remapped = provider;
        remapped.id = "replacement-provider".into();
        capture_agent_usage_with(&root, &[remapped], &record, &[message.clone()], &mut cache,
            |_| { saves.set(saves.get() + 1); Ok(()) }).unwrap();
        assert_eq!(saves.get(), 7);
        let missing = capture_agent_usage_with(&root, &[], &record, &[message], &mut cache,
            |_| panic!("a removed provider must not be captured")).unwrap();
        assert_eq!(missing, ["answer"]);
    }

    #[test]
    fn capture_cache_evicts_old_entries_at_a_fixed_limit() {
        let (root, provider, record, mut message) = agent_fixture();
        let mut cache = UsageCaptureCache::default();
        for index in 0..=CAPTURE_CACHE_LIMIT {
            message.id = format!("answer-{index}");
            message.usage.as_mut().unwrap().message_id = message.id.clone();
            capture_agent_usage_with(&root, &[provider.clone()], &record, &[message.clone()], &mut cache,
                |_| Ok(())).unwrap();
        }
        assert_eq!(cache.entries.len(), CAPTURE_CACHE_LIMIT);
        assert_eq!(cache.order.len(), CAPTURE_CACHE_LIMIT);
        assert!(!cache.entries.keys().any(|key| key.message_id == "answer-0"));
        message.id = "answer-0".into();
        message.usage.as_mut().unwrap().message_id = message.id.clone();
        let retried = std::cell::Cell::new(false);
        capture_agent_usage_with(&root, &[provider], &record, &[message], &mut cache,
            |_| { retried.set(true); Ok(()) }).unwrap();
        assert!(retried.get());
        assert_eq!(cache.entries.len(), CAPTURE_CACHE_LIMIT);
    }

    #[test]
    fn local_ledger_deduplicates_messages_and_groups_by_provider() {
        let root =
            std::env::temp_dir().join(format!("yume-ai-usage-test-{}", uuid::Uuid::new_v4()));
        let provider = AiProvider {
            id: "deepseek-provider".into(),
            sidecar_id: "yume".into(),
            label: "DeepSeek".into(),
            base_url: "https://api.deepseek.com".into(),
            manual_model_ids: String::new(),
            api_key: String::new(),
        };
        let observation = UsageObservation {
            message_id: "msg-1".into(),
            sidecar_id: "yume".into(),
            model_id: "deepseek-flash".into(),
            created_at_ms: Local::now().timestamp_millis(),
            tokens: 20,
        };
        save_observations(
            &root,
            &[provider.clone()],
            [observation.clone(), observation.clone()],
        )
        .unwrap();
        let first = today_summary(&root, &provider.id).unwrap();
        assert_eq!((first.tokens, first.requests), (20, 1));
        save_observations(
            &root,
            &[provider],
            [UsageObservation {
                tokens: 30,
                ..observation
            }],
        )
        .unwrap();
        let updated = today_summary(&root, "deepseek-provider").unwrap();
        assert_eq!((updated.tokens, updated.requests), (30, 1));
        assert_eq!(updated.top_models[0].name, "deepseek-flash");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_ledger_records_non_deepseek_providers() {
        let root =
            std::env::temp_dir().join(format!("yume-ai-usage-test-{}", uuid::Uuid::new_v4()));
        let provider = AiProvider {
            id: "claude-provider".into(),
            sidecar_id: "yume-2".into(),
            label: "Claude".into(),
            base_url: "https://api.anthropic.com".into(),
            manual_model_ids: String::new(),
            api_key: String::new(),
        };
        save_observations(
            &root,
            &[provider],
            [UsageObservation {
                message_id: "claude-msg-1".into(),
                sidecar_id: "yume-2".into(),
                model_id: "claude-sonnet".into(),
                created_at_ms: Local::now().timestamp_millis(),
                tokens: 42,
            }],
        )
        .unwrap();
        let summary = today_summary(&root, "claude-provider").unwrap();
        assert_eq!((summary.tokens, summary.requests), (42, 1));
        fs::remove_dir_all(root).unwrap();
    }
}
