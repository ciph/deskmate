use super::{contract::*, error::WorklogResult};
use rusqlite::{params, Connection};

pub(crate) fn stale_date(db: &Connection, date: &str) -> WorklogResult<()> {
    db.execute(
        "UPDATE reports SET stale=1 WHERE period_start<=?1 AND period_end>=?1",
        [date],
    )?;
    Ok(())
}
pub(crate) fn stale_source(db: &Connection, kind: &str, id: &str) -> WorklogResult<()> {
    db.execute("WITH RECURSIVE affected(id) AS (SELECT v.report_id FROM report_sources s JOIN report_versions v ON v.id=s.version_id WHERE s.source_kind=?1 AND s.source_id=?2 UNION SELECT v.report_id FROM report_sources s JOIN report_versions v ON v.id=s.version_id JOIN report_versions source ON source.id=s.source_id JOIN affected a ON a.id=source.report_id WHERE s.source_kind='daily_version') UPDATE reports SET stale=1 WHERE id IN (SELECT id FROM affected)",params![kind,id])?;
    Ok(())
}
pub(crate) fn erase_source(
    db: &Connection,
    kind: &str,
    request: &DeleteRecord,
) -> WorklogResult<()> {
    erase_batch(
        db,
        Some((kind, &request.id)),
        None,
        request.delete_linked_reports,
    )
}
pub(crate) fn erase_report(db: &Connection, id: &str) -> WorklogResult<()> {
    // Deleting one report does not authorize deleting the bodies of dependent reports.
    erase_batch(db, None, Some(id), false)
}

const SOURCE_BATCH: usize = 32;
#[cfg(test)]
thread_local! { pub(super) static PEAK_SOURCE_BATCH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

struct StoredSources {
    id: String,
    report_id: Option<String>,
    manifest: Vec<SourceRef>,
    snapshots: Vec<SourceSnapshot>,
}
impl StoredSources {
    fn read(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self { id:row.get(0)?,report_id:row.get(1)?,
            manifest:super::repository::decode(row.get(2)?,2)?,
            snapshots:super::repository::decode(row.get(3)?,3)? })
    }
    fn erase(&mut self, db:&Connection) -> WorklogResult<bool> {
        let before=(self.manifest.len(),self.snapshots.len());
        let mut query=db.prepare_cached("SELECT EXISTS(SELECT 1 FROM erase_sources WHERE kind=?1 AND id=?2)")?;
        let mut failure=None;
        let mut keep=|source:&SourceRef| match query.query_row(params![source.kind,source.id],|r|r.get::<_,bool>(0)) {
            Ok(found)=>!found,
            Err(error)=>{ failure=Some(error); true }
        };
        self.manifest.retain(&mut keep);
        self.snapshots.retain(|snapshot|keep(&snapshot.source));
        if let Some(error)=failure { return Err(error.into()); }
        Ok(before!=(self.manifest.len(),self.snapshots.len()))
    }
}
fn source_batches(db:&Connection, runs:bool, mut visit:impl FnMut(&mut StoredSources)->WorklogResult<()>) -> WorklogResult<()> {
    let select=if runs {
        "SELECT run.id,report.id,run.source_manifest,run.source_snapshot FROM report_runs run LEFT JOIN reports report ON report.kind=run.kind AND report.period_start=run.period_start AND report.period_end=run.period_end AND run.state IN ('queued','running','retry_wait')"
    } else { "SELECT id,report_id,source_revision_manifest,source_snapshot FROM report_versions" };
    let id=if runs {"run.id"} else {"id"};
    let mut after:Option<String>=None;
    loop {
        let sql=format!("{select}{} ORDER BY {id} LIMIT {SOURCE_BATCH}",if after.is_some() {format!(" WHERE {id}>?1")} else {String::new()});
        let mut statement=db.prepare(&sql)?;
        let mut batch=if let Some(after)=&after { statement.query_map([after],StoredSources::read)?.collect::<Result<Vec<_>,_>>()? }
            else { statement.query_map([],StoredSources::read)?.collect::<Result<Vec<_>,_>>()? };
        let Some(last)=batch.last() else {break};
        after=Some(last.id.clone());
        drop(statement);
        #[cfg(test)] PEAK_SOURCE_BATCH.with(|peak|peak.set(peak.get().max(batch.len())));
        for row in &mut batch {visit(row)?;}
    }
    Ok(())
}
fn erase_batch(db:&Connection,source:Option<(&str,&str)>,deleted_report:Option<&str>,delete_linked_reports:bool) -> WorklogResult<()> {
    // Transaction-local ID edges cover orphan snapshots too; no source text is kept in this projection.
    db.execute_batch("CREATE TEMP TABLE erase_edges(kind TEXT,id TEXT,report_id TEXT,PRIMARY KEY(kind,id,report_id));
        CREATE TEMP TABLE erase_reports(id TEXT PRIMARY KEY);
        CREATE TEMP TABLE erase_sources(kind TEXT,id TEXT,PRIMARY KEY(kind,id));")?;
    let mut edge=db.prepare("INSERT OR IGNORE INTO erase_edges VALUES(?1,?2,?3)")?;
    source_batches(db,false,|version| {
        for source in version.manifest.iter().chain(version.snapshots.iter().map(|snapshot|&snapshot.source)) {
            edge.execute(params![source.kind,source.id,version.report_id])?;
        }
        Ok(())
    })?;
    drop(edge);
    db.execute("INSERT OR IGNORE INTO erase_edges SELECT s.source_kind,s.source_id,v.report_id FROM report_sources s JOIN report_versions v ON v.id=s.version_id",[])?;
    db.execute("WITH RECURSIVE affected(id) AS (
        SELECT report_id FROM erase_edges WHERE kind=?1 AND id=?2
        UNION SELECT id FROM reports WHERE id=?3
        UNION SELECT edge.report_id FROM erase_edges edge JOIN report_versions v ON edge.kind='daily_version' AND edge.id=v.id JOIN affected a ON a.id=v.report_id
        ) INSERT INTO erase_reports SELECT id FROM affected",params![source.map(|s|s.0),source.map(|s|s.1),deleted_report])?;
    if let Some((kind,id))=source {db.execute("INSERT INTO erase_sources VALUES(?1,?2)",params![kind,id])?;}
    db.execute("INSERT OR IGNORE INTO erase_sources SELECT 'daily_version',v.id FROM report_versions v JOIN erase_reports r ON r.id=v.report_id",[])?;
    let deletes=|id:&str|->WorklogResult<bool> {
        Ok(deleted_report==Some(id) || (delete_linked_reports && db.query_row("SELECT EXISTS(SELECT 1 FROM erase_reports WHERE id=?1)",[id],|r|r.get(0))?))
    };
    source_batches(db,false,|version| {
        if !deletes(version.report_id.as_deref().expect("version has report id"))? && version.erase(db)? {
            db.execute("UPDATE report_versions SET source_revision_manifest=?1,source_snapshot=?2 WHERE id=?3",params![serde_json::to_string(&version.manifest)?,serde_json::to_string(&version.snapshots)?,version.id])?;
        }
        Ok(())
    })?;
    source_batches(db,true,|run| {
        if run.erase(db)? {
            db.execute("UPDATE report_runs SET source_manifest=?1,source_snapshot=?2,state=CASE WHEN state IN ('queued','running','retry_wait') THEN 'cancelled' ELSE state END WHERE id=?3",params![serde_json::to_string(&run.manifest)?,serde_json::to_string(&run.snapshots)?,run.id])?;
        } else if run.report_id.as_deref().map(&deletes).transpose()?.unwrap_or(false) {
            db.execute("UPDATE report_runs SET state='cancelled' WHERE id=?1",[&run.id])?;
        }
        Ok(())
    })?;
    db.execute("DELETE FROM report_sources WHERE EXISTS(SELECT 1 FROM erase_sources s WHERE s.kind=report_sources.source_kind AND s.id=report_sources.source_id)",[])?;
    db.execute("UPDATE worklog_operations SET status='deleted' WHERE entity_kind='report' AND (entity_id=?1 OR (?2 AND entity_id IN (SELECT id FROM erase_reports)))",params![deleted_report,delete_linked_reports])?;
    db.execute("DELETE FROM reports WHERE id=?1 OR (?2 AND id IN (SELECT id FROM erase_reports))",params![deleted_report,delete_linked_reports])?;
    db.execute("UPDATE reports SET stale=1,source_deleted=1 WHERE id IN (SELECT id FROM erase_reports) AND (stale=0 OR source_deleted=0)",[])?;
    db.execute_batch("DROP TABLE erase_edges; DROP TABLE erase_reports; DROP TABLE erase_sources;")?;
    Ok(())
}

pub(crate) fn version_stale(db: &Connection, version: &ReportVersion) -> WorklogResult<bool> {
    use rusqlite::OptionalExtension;
    for source in &version.source_revision_manifest {
        let current: Option<i64> = match source.kind.as_str() {
            "entry" => db.query_row("SELECT revision FROM work_entries WHERE id=?1",[&source.id],|r|r.get(0)).optional()?,
            "daily_version" => db.query_row("SELECT v.version FROM report_versions v JOIN reports r ON r.current_version_id=v.id WHERE v.id=?1 AND r.stale=0",[&source.id],|r|r.get(0)).optional()?,
            _ => return Err(super::error::WorklogError::validation("Unknown report source kind")),
        };
        if current != Some(source.revision) {
            return Ok(true);
        }
    }
    Ok(db.query_row("SELECT EXISTS(SELECT 1 FROM work_entries e JOIN reports r ON r.id=?1 WHERE e.business_date BETWEEN r.period_start AND r.period_end AND e.updated_at>?2) OR EXISTS(SELECT 1 FROM reports WHERE id=?1 AND source_deleted=1)",params![version.report_id,version.generated_at],|r|r.get(0))?)
}
