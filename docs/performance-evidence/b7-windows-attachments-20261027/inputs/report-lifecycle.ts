import {connect,save,until} from './cdp-client';
const c=await connect('settings');const reportId='47d0e11d-03d4-4935-8b9e-26a01a2e7480';
try{
 const before:any=await c.invoke('worklog_get_report',{id:reportId});
 const body='# B7 合成日报\n\n人工修订保留：完成附件和物理拖放验收。';
 const saved=await c.invoke('worklog_save_report',{request:{requestId:crypto.randomUUID(),kind:'daily',periodStart:'2026-10-27',periodEnd:'2026-10-27',expectedRevision:before.report.revision,bodyMarkdown:body}});
 const manual:any=await c.invoke('worklog_get_report',{id:reportId});
 const generated:any=await c.invoke('worklog_generate_report',{request:{requestId:crypto.randomUUID(),kind:'daily',periodStart:'2026-10-27',periodEnd:'2026-10-27',modelId:'b7-local'}});
 const runs:any=await until(()=>c.invoke('worklog_list_runs'),(x:any)=>x.some((r:any)=>r.id===generated.entityId&&r.state==='succeeded'),45000);
 const candidate:any=await c.invoke('worklog_get_report',{id:reportId});
 if(candidate.report.currentVersionId!==manual.report.currentVersionId)throw Error('Generation overwrote manual current version');
 const version=candidate.versions.find((v:any)=>!manual.versions.some((old:any)=>old.id===v.id));if(!version||version.origin!=='generated')throw Error('Generated candidate missing');
 const adopted=await c.invoke('worklog_apply_version',{request:{requestId:crypto.randomUUID(),reportId,versionId:version.id,expectedRevision:candidate.report.revision}});
 const after:any=await c.invoke('worklog_get_report',{id:reportId});
 let conflict:any;try{await c.invoke('worklog_save_report',{request:{requestId:crypto.randomUUID(),kind:'daily',periodStart:'2026-10-27',periodEnd:'2026-10-27',expectedRevision:before.report.revision,bodyMarkdown:'must not replace'}});throw Error('Stale revision accepted')}catch(e){if(!String(e).includes('CONFLICT'))throw e;conflict=String(e)}
 const current=after.versions.find((v:any)=>v.id===after.report.currentVersionId);
 const pass=current?.origin==='manual'&&current.bodyMarkdown===version.bodyMarkdown&&after.versions.some((v:any)=>v.bodyMarkdown===body);
 save('report-lifecycle.json',{method:'actual settings WebView2 native IPC, not physical UI or clipboard/export test',before,saved,manual,generated,runs,candidate,adopted,after,conflict,pass});
 if(!pass)throw Error('Report lifecycle failed');console.log(JSON.stringify({versions:after.versions.length,current:after.report.currentVersionId,manualRetained:true,conflictRejected:true,pass}));
}finally{c.close()}
