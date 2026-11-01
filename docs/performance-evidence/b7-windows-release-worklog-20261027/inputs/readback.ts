import {connect,save} from './cdp-client';
const label=process.argv[2]??'state',c=await connect('settings');
try{
 const entries:any=await c.invoke('worklog_query',{query:{start:'2026-10-01',end:'2026-11-30',project:null}});
 const reports:any=await c.invoke('worklog_list_reports',{query:{start:'2026-10-01',end:'2026-11-30',project:null}});
 const runs:any=await c.invoke('worklog_list_runs');const schedules:any=await c.invoke('worklog_list_schedules');
 const details=await Promise.all(reports.map((r:any)=>c.invoke('worklog_get_report',{id:r.id})));
 save('worklog-'+label+'.json',{time:new Date().toISOString(),entries,reports,runs,schedules,details});
 console.log(JSON.stringify({entries,reports,runs:runs.map((r:any)=>({id:r.id,scheduleId:r.scheduleId,state:r.state,attempt:r.attempt,errorCode:r.errorCode,periodStart:r.periodStart,periodEnd:r.periodEnd})),schedules,versions:details.map((d:any)=>({id:d.report.id,current:d.report.currentVersionId,versions:d.versions.map((v:any)=>({id:v.id,version:v.version,origin:v.origin,body:v.bodyMarkdown.slice(0,200)}))}))}));
}finally{c.close()}
