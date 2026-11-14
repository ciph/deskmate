use super::*;
#[test]
fn report_view_reads_one_body_and_sources_in_bounded_pages_without_changing_archives() {
    let repo = Repository::new(WorklogStore::memory().unwrap());
    let saved = repo.save_report(&SaveReport { request_id: uuid::Uuid::new_v4().to_string(), kind: ReportKind::Daily,
        period_start: "2026-09-07".into(), period_end: "2026-09-07".into(), expected_revision: None, body_markdown: "Current body".into() }).unwrap();
    let id = saved.entity_id;
    let current = repo.get_report(&id).unwrap().versions[0].id.clone();
    let sources:Vec<SourceSnapshot> = (0..123).map(|n| SourceSnapshot { source: SourceRef { kind:"entry".into(), id:format!("s-{n}"), revision:1 },
        text:"large synthetic source ".repeat(2000), business_date:"2026-09-07".into(), project:None, entry_status:None }).collect();
    repo.store.with_transaction(|db| {
        db.execute("UPDATE report_versions SET source_snapshot=?1 WHERE id=?2",params![serde_json::to_string(&sources)?,current])?;
        // Historical payload is intentionally invalid: summary reads must never decode it.
        db.execute("INSERT INTO report_versions SELECT 'old',report_id,2,body_markdown,origin,'invalid','invalid','invalid',generated_at,model_id FROM report_versions WHERE id=?1",[&current])?;
        Ok(())
    }).unwrap();
    let view = repo.report_view(&id).unwrap();
    assert_eq!(view.versions.len(),2);
    assert_eq!(view.current_version.as_ref().unwrap().source_count,123);
    assert_eq!(view.current_version.as_ref().unwrap().body_markdown,"Current body");
    let encoded=serde_json::to_string(&view).unwrap();
    assert!(encoded.len()<2000);
    println!("REPORT_VIEW_METRICS {}",serde_json::json!({"sources":sources.len(),"summaryAndCurrentBodyBytes":encoded.len(),"sourceSnapshotJsonBytes":serde_json::to_string(&sources).unwrap().len(),"maxPageSources":50}));
    assert!(!encoded.contains("large synthetic"));
    let mut offset=0;
    let mut ids=Vec::new();
    loop {
        let page=repo.report_source_page(&id,&current,offset).unwrap();
        assert!(page.sources.len()<=50);
        ids.extend(page.sources.into_iter().map(|s|s.source.id));
        let Some(next)=page.next_offset else {break}; offset=next;
    }
    assert_eq!(ids,sources.iter().map(|s|s.source.id.clone()).collect::<Vec<_>>());
    assert!(repo.report_source_page("foreign",&current,0).is_err());
    assert!(repo.report_version_content("foreign",&current).is_err());
    assert!(repo.report_source_page(&id,&current,usize::MAX).is_err());
    assert_eq!(repo.report_source_page(&id,&current,123).unwrap().sources.len(),0);
    repo.store.with_connection(|db| {
        assert_eq!(db.query_row("SELECT source_snapshot FROM report_versions WHERE id='old'",[],|r|r.get::<_,String>(0))?,"invalid");
        Ok(())
    }).unwrap();
}
