use super::{
    contract::OperationReceipt,
    contract_runtime::RunState,
    error::{WorklogError, WorklogResult},
    repository::{decode_enum, operation, Repository},
};
use rusqlite::OptionalExtension;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSummary {
    pub id: String,
    pub state: RunState,
    pub result_report_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleSummary {
    pub id: String,
    pub next_due_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationState {
    pub request_id: String,
    pub receipt: Option<OperationReceipt>,
    pub run: Option<RunSummary>,
    pub schedule: Option<ScheduleSummary>,
    pub entity_missing: bool,
}

impl Repository {
    pub fn operation_states(&self, request_ids: &[String]) -> WorklogResult<Vec<OperationState>> {
        if request_ids.len() > 50
            || request_ids
                .iter()
                .any(|id| uuid::Uuid::parse_str(id).is_err())
        {
            return Err(WorklogError::validation(
                "Expected at most 50 operation UUIDs",
            ));
        }
        if request_ids.is_empty() {
            return Ok(Vec::new());
        }
        self.store.with_connection(|db| {
            let mut seen = HashSet::new();
            let mut runs = HashMap::<String, Option<RunSummary>>::new();
            let mut schedules = HashMap::<String, Option<ScheduleSummary>>::new();
            let mut result = Vec::new();
            for id in request_ids {
                if !seen.insert(id) { continue; }
                let receipt = operation(db, id)?;
                let mut row = OperationState { request_id: id.clone(), receipt, run: None, schedule: None, entity_missing: false };
                if let Some(receipt) = row.receipt.as_ref().filter(|receipt| receipt.status != "deleted") {
                    match receipt.entity_kind.as_str() {
                        "run" => {
                            if !runs.contains_key(&receipt.entity_id) {
                                let run = db.query_row("SELECT id,state,result_report_id FROM report_runs WHERE id=?1", [&receipt.entity_id], |r| Ok(RunSummary {
                                    id: r.get(0)?, state: decode_enum(r.get(1)?, 1)?, result_report_id: r.get(2)?,
                                })).optional()?;
                                runs.insert(receipt.entity_id.clone(), run);
                            }
                            row.run = runs.get(&receipt.entity_id).cloned().flatten();
                            row.entity_missing = row.run.is_none();
                        }
                        "schedule" => {
                            if !schedules.contains_key(&receipt.entity_id) {
                                let schedule = db.query_row("SELECT id,next_due_at FROM report_schedules WHERE id=?1", [&receipt.entity_id], |r| Ok(ScheduleSummary { id: r.get(0)?, next_due_at: r.get(1)? })).optional()?;
                                schedules.insert(receipt.entity_id.clone(), schedule);
                            }
                            row.schedule = schedules.get(&receipt.entity_id).cloned().flatten();
                            row.entity_missing = row.schedule.is_none();
                        }
                        _ => {}
                    }
                }
                result.push(row);
            }
            Ok(result)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worklog::{
        contract::ReportKind, contract_runtime::GenerateReport, storage::WorklogStore,
    };

    #[test]
    fn exact_old_run_does_not_depend_on_recent_list_or_decode_snapshots() {
        let repo = Repository::new(WorklogStore::memory().unwrap());
        let mut ids = Vec::new();
        let mut first = String::new();
        for index in 0..102 {
            let request_id = uuid::Uuid::new_v4().to_string();
            let receipt = repo
                .generate_report(&GenerateReport {
                    request_id: request_id.clone(),
                    kind: ReportKind::Daily,
                    period_start: "2026-10-29".into(),
                    period_end: "2026-10-29".into(),
                    model_id: "fixture/local".into(),
                })
                .unwrap();
            if index == 0 {
                first = receipt.entity_id;
            }
            ids.push(request_id);
        }
        assert!(!repo.list_runs().unwrap().iter().any(|run| run.id == first));
        repo.store.with_connection(|db| { db.execute("UPDATE report_runs SET state='succeeded',source_snapshot='invalid JSON' WHERE id=?1", [&first])?; Ok(()) }).unwrap();
        let rows = repo
            .operation_states(&[ids[0].clone(), ids[0].clone()])
            .unwrap();
        assert_eq!(rows.len(), 1);
        let run = rows[0].run.as_ref().unwrap();
        assert_eq!(run.id, first);
        assert_eq!(run.state, RunState::Succeeded);
        assert_eq!(run.result_report_id, None);
        assert!(!serde_json::to_string(&rows)
            .unwrap()
            .contains("sourceSnapshot"));
    }

    #[test]
    fn empty_missing_deleted_invalid_and_storage_failure_are_distinct() {
        let repo = Repository::new(WorklogStore::memory().unwrap());
        assert!(repo.operation_states(&[]).unwrap().is_empty());
        assert!(repo.operation_states(&["bad".into()]).is_err());
        let id = uuid::Uuid::new_v4().to_string();
        assert!(repo.operation_states(&vec![id.clone(); 51]).is_err());
        assert!(repo.operation_states(&[id.clone()]).unwrap()[0]
            .receipt
            .is_none());
        repo.store.with_connection(|db| { db.execute("INSERT INTO worklog_operations VALUES(?1,'hash','run','absent',1,NULL,'committed')", [&id])?; Ok(()) }).unwrap();
        assert!(repo.operation_states(&[id.clone()]).unwrap()[0].entity_missing);
        repo.store
            .with_connection(|db| {
                db.execute(
                    "UPDATE worklog_operations SET status='deleted' WHERE request_id=?1",
                    [&id],
                )?;
                Ok(())
            })
            .unwrap();
        let row = repo.operation_states(&[id.clone()]).unwrap().remove(0);
        assert_eq!(row.receipt.unwrap().status, "deleted");
        assert!(!row.entity_missing);
        repo.store
            .with_connection(|db| {
                db.execute_batch("DROP TABLE worklog_operations")?;
                Ok(())
            })
            .unwrap();
        assert!(repo.operation_states(&[id]).is_err());
        assert!(repo.operation_states(&[]).unwrap().is_empty());
    }
}
