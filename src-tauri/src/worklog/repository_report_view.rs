//! Read one version body or one source page; summaries never select snapshot text.
use super::{
    contract::*,
    error::{WorklogError, WorklogResult},
    repository::{decode, decode_enum, Repository},
    repository_reports::report_from_row,
};
use rusqlite::{params, Connection, OptionalExtension, Row};

fn summary(row: &Row<'_>) -> rusqlite::Result<ReportVersionSummary> {
    Ok(ReportVersionSummary {
        id: row.get(0)?,
        report_id: row.get(1)?,
        version: row.get(2)?,
        origin: decode_enum(row.get(3)?, 3)?,
        generated_at: row.get(4)?,
        model_id: row.get(5)?,
    })
}
fn content(
    db: &Connection,
    report_id: &str,
    version_id: &str,
) -> WorklogResult<ReportVersionContent> {
    db.query_row("SELECT id,report_id,version,origin,generated_at,model_id,body_markdown,coverage_dates,json_array_length(source_snapshot) FROM report_versions WHERE id=?1 AND report_id=?2",params![version_id,report_id],|row| {
        Ok(ReportVersionContent { summary:summary(row)?,body_markdown:row.get(6)?,coverage_dates:decode(row.get(7)?,7)?,source_count:row.get::<_,u32>(8)? as usize })
    }).optional()?.ok_or_else(WorklogError::missing)
}
impl Repository {
    pub fn report_metadata(&self, id: &str) -> WorklogResult<Report> {
        self.store.with_connection(|db| {
            db.query_row("SELECT * FROM reports WHERE id=?1", [id], report_from_row)
                .optional()?
                .ok_or_else(WorklogError::missing)
        })
    }
    pub fn report_view(&self, id: &str) -> WorklogResult<ReportView> {
        self.store.with_connection(|db| {
            let report=db.query_row("SELECT * FROM reports WHERE id=?1",[id],report_from_row).optional()?.ok_or_else(WorklogError::missing)?;
            let versions=db.prepare("SELECT id,report_id,version,origin,generated_at,model_id FROM report_versions WHERE report_id=?1 ORDER BY version DESC")?
                .query_map([id],summary)?.collect::<Result<Vec<_>,_>>()?;
            let current=report.current_version_id.as_deref().filter(|id|versions.iter().any(|v|v.id==*id)).or_else(||versions.first().map(|v|v.id.as_str()));
            let current_version=current.map(|version|content(db,id,version)).transpose()?;
            Ok(ReportView {report,versions,current_version})
        })
    }
    pub fn report_version_content(
        &self,
        report_id: &str,
        version_id: &str,
    ) -> WorklogResult<ReportVersionContent> {
        self.store
            .with_connection(|db| content(db, report_id, version_id))
    }
    pub fn report_source_page(
        &self,
        report_id: &str,
        version_id: &str,
        offset: usize,
    ) -> WorklogResult<ReportSourcePage> {
        self.store.with_connection(|db| {
            let found:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM report_versions WHERE id=?1 AND report_id=?2)",params![version_id,report_id],|r|r.get(0))?;
            if !found { return Err(WorklogError::missing()); }
            let offset=i64::try_from(offset).map_err(|_|WorklogError::validation("Invalid source offset"))?;
            let mut sources=db.prepare("SELECT s.value FROM report_versions v,json_each(v.source_snapshot) s WHERE v.id=?1 AND v.report_id=?2 AND s.key>=?3 ORDER BY s.key LIMIT 51")?
                .query_map(params![version_id,report_id,offset],|row|decode(row.get(0)?,0))?.collect::<Result<Vec<SourceSnapshot>,_>>()?;
            let next_offset=(sources.len()>50).then_some(offset as usize+50);
            sources.truncate(50);
            Ok(ReportSourcePage {sources,next_offset})
        })
    }
}
