use super::{contract::*, error::WorklogResult};
use rusqlite::{params, Connection};
use std::collections::{HashMap, HashSet};

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

struct StoredSources {
    id: String,
    report_id: Option<String>,
    manifest: Vec<SourceRef>,
    snapshots: Vec<SourceSnapshot>,
}
impl StoredSources {
    fn read(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            report_id: row.get(1)?,
            manifest: super::repository::decode(row.get(2)?, 2)?,
            snapshots: super::repository::decode(row.get(3)?, 3)?,
        })
    }
    fn erase(&mut self, sources: &HashMap<String, HashSet<String>>) -> bool {
        let before = (self.manifest.len(), self.snapshots.len());
        let removed = |s: &SourceRef| sources.get(&s.kind).is_some_and(|ids| ids.contains(&s.id));
        self.manifest.retain(|s| !removed(s));
        self.snapshots.retain(|s| !removed(&s.source));
        before != (self.manifest.len(), self.snapshots.len())
    }
}
fn erase_batch(
    db: &Connection,
    source: Option<(&str, &str)>,
    deleted_report: Option<&str>,
    delete_linked_reports: bool,
) -> WorklogResult<()> {
    // Snapshots and unfinished runs can exist independently of report_sources.
    let mut statement = db.prepare(
        "SELECT id,report_id,source_revision_manifest,source_snapshot FROM report_versions",
    )?;
    let mut versions = statement
        .query_map([], StoredSources::read)?
        .collect::<Result<Vec<_>, _>>()?;
    let mut statement = db.prepare("SELECT run.id,report.id,run.source_manifest,run.source_snapshot FROM report_runs run LEFT JOIN reports report ON report.kind=run.kind AND report.period_start=run.period_start AND report.period_end=run.period_end AND run.state IN ('queued','running','retry_wait')")?;
    let mut runs = statement
        .query_map([], StoredSources::read)?
        .collect::<Result<Vec<_>, _>>()?;
    let mut dependents: HashMap<(String, String), HashSet<String>> = HashMap::new();
    let mut report_versions: HashMap<&str, Vec<&str>> = HashMap::new();
    for version in &versions {
        let report = version.report_id.as_deref().expect("version has report id");
        report_versions.entry(report).or_default().push(&version.id);
        for source in version
            .manifest
            .iter()
            .chain(version.snapshots.iter().map(|s| &s.source))
        {
            dependents
                .entry((source.kind.clone(), source.id.clone()))
                .or_default()
                .insert(report.to_owned());
        }
    }
    let mut statement = db.prepare("SELECT s.source_kind,s.source_id,v.report_id FROM report_sources s JOIN report_versions v ON v.id=s.version_id")?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        dependents
            .entry((row.get(0)?, row.get(1)?))
            .or_default()
            .insert(row.get(2)?);
    }
    let mut pending = Vec::new();
    if let Some((kind, id)) = source {
        pending.push((kind.to_owned(), id.to_owned()));
    }
    let mut affected_reports = HashSet::new();
    if let Some(report) = deleted_report {
        affected_reports.insert(report.to_owned());
        if let Some(versions) = report_versions.get(report) {
            pending.extend(
                versions
                    .iter()
                    .map(|id| ("daily_version".to_owned(), (*id).to_owned())),
            );
        }
    }
    let mut sources: HashMap<String, HashSet<String>> = HashMap::new();
    while let Some((kind, id)) = pending.pop() {
        if !sources.entry(kind.clone()).or_default().insert(id.clone()) {
            continue;
        }
        if let Some(reports) = dependents.get(&(kind, id)) {
            for report in reports {
                if affected_reports.insert(report.clone()) {
                    if let Some(versions) = report_versions.get(report.as_str()) {
                        pending.extend(
                            versions
                                .iter()
                                .map(|id| ("daily_version".to_owned(), (*id).to_owned())),
                        );
                    }
                }
            }
        }
    }
    // Release borrowed version IDs before filtering the single decoded snapshot set.
    drop(report_versions);
    let deletes = |id: &str| {
        deleted_report == Some(id) || (delete_linked_reports && affected_reports.contains(id))
    };
    let mut update_version = db.prepare(
        "UPDATE report_versions SET source_revision_manifest=?1,source_snapshot=?2 WHERE id=?3",
    )?;
    for version in &mut versions {
        if !deletes(version.report_id.as_deref().expect("version has report id"))
            && version.erase(&sources)
        {
            update_version.execute(params![
                serde_json::to_string(&version.manifest)?,
                serde_json::to_string(&version.snapshots)?,
                version.id
            ])?;
        }
    }
    let mut update_run = db.prepare("UPDATE report_runs SET source_manifest=?1,source_snapshot=?2,state=CASE WHEN state IN ('queued','running','retry_wait') THEN 'cancelled' ELSE state END WHERE id=?3")?;
    for run in &mut runs {
        if run.erase(&sources) {
            update_run.execute(params![
                serde_json::to_string(&run.manifest)?,
                serde_json::to_string(&run.snapshots)?,
                run.id
            ])?;
        } else if run.report_id.as_deref().is_some_and(deletes) {
            // A regeneration may use only entries, not a version of its deleted target.
            db.execute(
                "UPDATE report_runs SET state='cancelled' WHERE id=?1",
                [&run.id],
            )?;
        }
    }
    let mut remove_source =
        db.prepare("DELETE FROM report_sources WHERE source_kind=?1 AND source_id=?2")?;
    for (kind, ids) in sources {
        for id in ids {
            remove_source.execute(params![kind, id])?;
        }
    }
    for report in &affected_reports {
        if deletes(report) {
            db.execute("DELETE FROM reports WHERE id=?1", [report])?;
            db.execute("UPDATE worklog_operations SET status='deleted' WHERE entity_kind='report' AND entity_id=?1", [report])?;
        } else {
            db.execute("UPDATE reports SET stale=1,source_deleted=1 WHERE id=?1 AND (stale=0 OR source_deleted=0)", [report])?;
        }
    }
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
