use super::{
    contract::*,
    contract_runtime::*,
    error::{WorklogError, WorklogResult},
    repository::Repository,
    storage::{WorklogStore, DB_FILE_NAME},
};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};

pub const WORKLOG_CHANGED_EVENT: &str = "deskmate://worklog-changed";
pub struct WorklogState(pub Mutex<Option<Arc<Repository>>>);
impl WorklogState {
    pub fn initialize(app: &tauri::AppHandle) -> Self {
        let store = app
            .path()
            .app_data_dir()
            .map_err(|_| WorklogError::new("STORAGE_UNAVAILABLE", "Data directory unavailable"))
            .and_then(|dir| WorklogStore::open(&dir.join(DB_FILE_NAME)));
        match store {
            Ok(store) => Self(Mutex::new(Some(Arc::new(Repository::new(store))))),
            Err(error) => {
                eprintln!("worklog disabled: {}", error.code);
                Self(Mutex::new(None))
            }
        }
    }
    pub fn repository(&self) -> WorklogResult<Arc<Repository>> {
        self.0
            .lock()
            .map_err(|_| WorklogError::new("STORAGE_UNAVAILABLE", "Journal state unavailable"))?
            .as_ref()
            .cloned()
            .ok_or_else(|| WorklogError::new("WORKLOG_DISABLED", "Work journal is unavailable"))
    }
    pub fn with_repository<T>(
        &self,
        body: impl FnOnce(&Repository) -> WorklogResult<T>,
    ) -> WorklogResult<T> {
        body(self.repository()?.as_ref())
    }

    pub(crate) fn has_running_run(&self) -> WorklogResult<bool> {
        self.repository()?.has_running_run()
    }
}
fn configured_model<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> WorklogResult<String> {
    let state = app.state::<crate::settings::SettingsState>();
    let settings = state
        .0
        .lock()
        .map_err(|_| WorklogError::new("MODEL_CONFIGURATION", "Model settings unavailable"))?;
    if settings.provider_id.is_empty() || settings.model_id.is_empty() {
        return Err(WorklogError::new(
            "MODEL_CONFIGURATION",
            "Choose a model in settings",
        ));
    }
    Ok(format!("{}/{}", settings.provider_id, settings.model_id))
}
fn notify<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let _ = app.emit(WORKLOG_CHANGED_EVENT, ());
}

#[tauri::command]
pub async fn worklog_available<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> bool {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<WorklogState>();
        state.repository().is_ok()
    })
    .await
    .unwrap_or(false)
}
#[tauri::command]
pub async fn worklog_record<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    request: RecordEntry,
) -> WorklogResult<OperationReceipt> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<WorklogState>();
        let result = state.with_repository(|repo| repo.record_entry(&request))?;
        notify(&app);
        Ok(result)
    })
    .await
    .map_err(|_| WorklogError::new("STORAGE_UNAVAILABLE", "Journal worker failed"))?
}
#[tauri::command]
pub async fn worklog_update<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    request: UpdateEntry,
) -> WorklogResult<OperationReceipt> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<WorklogState>();
        let result = state.with_repository(|repo| repo.update_entry(&request))?;
        notify(&app);
        Ok(result)
    })
    .await
    .map_err(|_| WorklogError::new("STORAGE_UNAVAILABLE", "Journal worker failed"))?
}
#[tauri::command]
pub async fn worklog_get_entry<R: tauri::Runtime>(
    window: tauri::WebviewWindow<R>,
    app: tauri::AppHandle<R>,
    id: String,
) -> WorklogResult<Option<Entry>> {
    if window.label() != "settings" {
        return Err(WorklogError::new("FORBIDDEN", "Settings window required"));
    }
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<WorklogState>().with_repository(|repo| repo.get_entry(&id))
    }).await.map_err(|_| WorklogError::new("STORAGE_UNAVAILABLE", "Journal worker failed"))?
}
#[tauri::command]
pub async fn worklog_query<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    query: DateQuery,
) -> WorklogResult<Vec<Entry>> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<WorklogState>();
        state.with_repository(|repo| repo.query_entries(&query))
    })
    .await
    .map_err(|_| WorklogError::new("STORAGE_UNAVAILABLE", "Journal worker failed"))?
}
#[tauri::command]
pub async fn worklog_delete_entry<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    request: DeleteRecord,
) -> WorklogResult<OperationReceipt> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<WorklogState>();
        let result = state.with_repository(|repo| repo.delete_entry(&request))?;
        notify(&app);
        Ok(result)
    })
    .await
    .map_err(|_| WorklogError::new("STORAGE_UNAVAILABLE", "Journal worker failed"))?
}
#[tauri::command]
pub async fn worklog_list_reports<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    query: DateQuery,
) -> WorklogResult<Vec<Report>> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<WorklogState>();
        state.with_repository(|repo| repo.list_reports(&query))
    })
    .await
    .map_err(|_| WorklogError::new("STORAGE_UNAVAILABLE", "Journal worker failed"))?
}
#[tauri::command]
pub async fn worklog_get_report<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    id: String,
) -> WorklogResult<ReportView> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<WorklogState>();
        state.with_repository(|repo| repo.report_view(&id))
    })
    .await
    .map_err(|_| WorklogError::new("STORAGE_UNAVAILABLE", "Journal worker failed"))?
}
#[tauri::command]
pub async fn worklog_get_report_version<R:tauri::Runtime>(app:tauri::AppHandle<R>,report_id:String,version_id:String) -> WorklogResult<ReportVersionContent> {
    tauri::async_runtime::spawn_blocking(move || app.state::<WorklogState>().with_repository(|repo|repo.report_version_content(&report_id,&version_id)))
        .await.map_err(|_|WorklogError::new("STORAGE_UNAVAILABLE","Journal worker failed"))?
}
#[tauri::command]
pub async fn worklog_get_report_sources<R:tauri::Runtime>(app:tauri::AppHandle<R>,report_id:String,version_id:String,offset:usize) -> WorklogResult<ReportSourcePage> {
    tauri::async_runtime::spawn_blocking(move || app.state::<WorklogState>().with_repository(|repo|repo.report_source_page(&report_id,&version_id,offset)))
        .await.map_err(|_|WorklogError::new("STORAGE_UNAVAILABLE","Journal worker failed"))?
}
#[tauri::command]
pub async fn worklog_save_report<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    request: SaveReport,
) -> WorklogResult<OperationReceipt> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<WorklogState>();
        let result = state.with_repository(|repo| repo.save_report(&request))?;
        notify(&app);
        Ok(result)
    })
    .await
    .map_err(|_| WorklogError::new("STORAGE_UNAVAILABLE", "Journal worker failed"))?
}
#[tauri::command]
pub async fn worklog_apply_version<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    request: ApplyVersion,
) -> WorklogResult<OperationReceipt> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<WorklogState>();
        let result = state.with_repository(|repo| repo.apply_version(&request))?;
        notify(&app);
        Ok(result)
    })
    .await
    .map_err(|_| WorklogError::new("STORAGE_UNAVAILABLE", "Journal worker failed"))?
}
#[tauri::command]
pub async fn worklog_delete_report<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    request: DeleteRecord,
) -> WorklogResult<OperationReceipt> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<WorklogState>();
        let result = state.with_repository(|repo| repo.delete_report(&request))?;
        notify(&app);
        Ok(result)
    })
    .await
    .map_err(|_| WorklogError::new("STORAGE_UNAVAILABLE", "Journal worker failed"))?
}
#[tauri::command]
pub async fn worklog_list_schedules<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> WorklogResult<Vec<Schedule>> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<WorklogState>();
        state.with_repository(Repository::list_schedules)
    })
    .await
    .map_err(|_| WorklogError::new("STORAGE_UNAVAILABLE", "Journal worker failed"))?
}
#[tauri::command]
pub async fn worklog_save_schedule<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    request: SaveSchedule,
) -> WorklogResult<OperationReceipt> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<WorklogState>();
        let result = state.with_repository(|repo| repo.save_schedule(&request))?;
        notify(&app);
        Ok(result)
    })
    .await
    .map_err(|_| WorklogError::new("STORAGE_UNAVAILABLE", "Journal worker failed"))?
}
#[tauri::command]
pub async fn worklog_delete_schedule<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    request: DeleteRecord,
) -> WorklogResult<OperationReceipt> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<WorklogState>();
        let result = state.with_repository(|repo| repo.delete_schedule(&request))?;
        notify(&app);
        Ok(result)
    })
    .await
    .map_err(|_| WorklogError::new("STORAGE_UNAVAILABLE", "Journal worker failed"))?
}
#[tauri::command]
pub async fn worklog_generate_report<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    mut request: GenerateReport,
) -> WorklogResult<OperationReceipt> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<WorklogState>();
        request.model_id = configured_model(&app)?;
        let result = state.with_repository(|repo| repo.generate_report(&request))?;
        notify(&app);
        Ok(result)
    })
    .await
    .map_err(|_| WorklogError::new("STORAGE_UNAVAILABLE", "Journal worker failed"))?
}
#[tauri::command]
pub async fn worklog_list_runs<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> WorklogResult<Vec<Run>> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<WorklogState>();
        state.with_repository(Repository::list_runs)
    })
    .await
    .map_err(|_| WorklogError::new("STORAGE_UNAVAILABLE", "Journal worker failed"))?
}
#[tauri::command]
pub async fn worklog_retry_run<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    request: RetryRun,
) -> WorklogResult<OperationReceipt> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<WorklogState>();
        let model = configured_model(&app)?;
        let result = state.with_repository(|repo| repo.retry_run(&request, &model))?;
        notify(&app);
        Ok(result)
    })
    .await
    .map_err(|_| WorklogError::new("STORAGE_UNAVAILABLE", "Journal worker failed"))?
}
#[tauri::command]
pub async fn worklog_get_operation<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    request_id: String,
) -> WorklogResult<Option<OperationReceipt>> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<WorklogState>();
        state.with_repository(|repo| repo.operation(&request_id))
    })
    .await
    .map_err(|_| WorklogError::new("STORAGE_UNAVAILABLE", "Journal worker failed"))?
}
#[cfg(test)]
#[path = "tests/commands.rs"]
mod tests;

#[tauri::command]
pub async fn worklog_get_operation_states<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    window: tauri::WebviewWindow<R>,
    request_ids: Vec<String>,
) -> WorklogResult<Vec<super::operation_states::OperationState>> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<WorklogState>();
        if window.label() != "chat" {
            return Err(WorklogError::new("FORBIDDEN", "Chat window required"));
        }
        state.with_repository(|repo| repo.operation_states(&request_ids))
    })
    .await
    .map_err(|_| WorklogError::new("STORAGE_UNAVAILABLE", "Journal worker failed"))?
}
