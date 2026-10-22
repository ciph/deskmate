use super::calendar::CalendarRule;
use super::contract::{enum_text, ReportKind};
use super::error::{WorklogError, WorklogResult};
use super::repository::Repository;
use chrono::{DateTime, Local, Utc};
use rusqlite::params;

impl Repository {
    pub(super) fn next_runner_deadline(
        &self,
        now: DateTime<Utc>,
    ) -> WorklogResult<Option<DateTime<Utc>>> {
        self.store.with_connection(|db| {
            let next: Option<String> = db.query_row(
                "SELECT MIN(due_at) FROM (
                    SELECT MIN(next_due_at) AS due_at FROM report_schedules WHERE enabled=1
                    UNION ALL SELECT ?1 WHERE EXISTS(SELECT 1 FROM report_runs WHERE state='queued')
                    UNION ALL SELECT MIN(next_retry_at) FROM report_runs WHERE state='retry_wait'
                    UNION ALL SELECT MIN(lease_until) FROM report_runs WHERE state='running'
                )",
                [now.to_rfc3339()],
                |row| row.get(0),
            )?;
            next.map(|value| {
                DateTime::parse_from_rfc3339(&value)
                    .map(|date| date.with_timezone(&Utc))
                    .map_err(|_| WorklogError::validation("Invalid runner deadline"))
            })
            .transpose()
        })
    }
    pub fn enqueue_due(&self, now: DateTime<Utc>, model_id: &str) -> WorklogResult<usize> {
        self.store.with_transaction(|tx| {
            let mut statement=tx.prepare("SELECT id,kind,weekday_set,local_time,created_at,next_due_at FROM report_schedules WHERE enabled=1")?;
            let schedules=statement.query_map([],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,String>(3)?,row.get::<_,String>(4)?,row.get::<_,Option<String>>(5)?)))?.collect::<Result<Vec<_>,_>>()?;
            let mut count=0;
            for(id,kind,weekdays,time,created,previous_next) in schedules {
                let kind:ReportKind=serde_json::from_value(serde_json::Value::String(kind))?;
                let weekdays:Vec<u32>=serde_json::from_str(&weekdays)?;
                let rule=CalendarRule::parse(&weekdays,&time,kind==ReportKind::Weekly).map_err(WorklogError::validation)?;
                let created=DateTime::parse_from_rfc3339(&created).map_err(|_|WorklogError::validation("Invalid schedule date"))?.with_timezone(&Utc);
                if let Some(due)=rule.latest_due((created,now),&Local) {
                    let start=due.period_start.to_string(); let end=due.period_end.to_string();
                    count+=tx.execute("INSERT OR IGNORE INTO report_runs(id,schedule_id,kind,period_start,period_end,occurrence_key,state,model_id,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,'queued',?7,?8,?8)",params![uuid::Uuid::new_v4().to_string(),id,enum_text(&kind)?,start,end,format!("{id}:{start}:{end}"),model_id,now.to_rfc3339()])?;
                }
                let next=rule.next_due(now,&Local).map(|value|value.due_at.to_rfc3339());
                if next != previous_next {
                    tx.execute("UPDATE report_schedules SET next_due_at=?2 WHERE id=?1",params![id,next])?;
                }
            }
            Ok(count)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::storage::WorklogStore;
    use super::*;
    #[test]
    fn latest_missed_cycle_is_enqueued_once_and_creation_is_respected() {
        let repo = Repository::new(WorklogStore::memory().unwrap());
        repo.store.with_connection(|db|{
            db.execute("INSERT INTO report_schedules(id,kind,weekday_set,local_time,enabled,created_at,updated_at) VALUES('old','weekly','[5]','17:00',1,'2026-08-01T00:00:00+00:00','2026-08-01T00:00:00+00:00')",[])?;
            db.execute("INSERT INTO report_schedules(id,kind,weekday_set,local_time,enabled,created_at,updated_at) VALUES('new','weekly','[5]','17:00',1,'2026-09-08T00:00:00+00:00','2026-09-08T00:00:00+00:00')",[])?;Ok(())
        }).unwrap();
        let now = DateTime::parse_from_rfc3339("2026-09-08T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(repo.enqueue_due(now, "fixture/test").unwrap(), 1);
        assert_eq!(repo.enqueue_due(now, "fixture/test").unwrap(), 0);
        repo.store
            .with_connection(|db| {
                let (start, end): (String, String) = db.query_row(
                    "SELECT period_start,period_end FROM report_runs",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?;
                assert_eq!(start, "2026-08-31");
                assert_eq!(end, "2026-09-04");
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn unchanged_next_due_does_not_update_any_schedule_row() {
        let repo = Repository::new(WorklogStore::memory().unwrap());
        let now = Utc::now();
        repo.store.with_connection(|db| {
            db.execute("INSERT INTO report_schedules(id,kind,weekday_set,local_time,enabled,created_at,updated_at) VALUES('idle','daily','[1,2,3,4,5]','17:00',1,?1,?1)", [now.to_rfc3339()])?;
            Ok(())
        }).unwrap();
        repo.enqueue_due(now, "fixture/test").unwrap();
        let before = repo
            .store
            .with_connection(|db| Ok(db.total_changes()))
            .unwrap();
        for _ in 0..10 {
            assert_eq!(repo.enqueue_due(now, "fixture/test").unwrap(), 0);
        }
        let after = repo
            .store
            .with_connection(|db| Ok(db.total_changes()))
            .unwrap();
        assert_eq!(
            after - before,
            0,
            "an unchanged next_due_at must not UPDATE"
        );
    }
}
