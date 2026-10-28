import {readFileSync} from 'node:fs';import {connect,save,report,until} from './cdp-client';
const fixture=JSON.parse(readFileSync(report+'/missed-week-fixture.json','utf8'));const stage=process.argv[2]??'first';const c=await connect('chat');
try{
 const matches=(runs:any[])=>runs.filter(r=>r.scheduleId===fixture.scheduleId);
 const runs:any=await until(()=>c.invoke('worklog_list_runs'),(x:any)=>matches(x).some(r=>r.state==='succeeded'),45000);
 const found=matches(runs);const schedules:any=await c.invoke('worklog_list_schedules');
 const details=await Promise.all(found.map(r=>c.invoke('worklog_get_report',{id:r.resultReportId})));
 const pass=found.length===1&&found[0].periodStart===fixture.expectedStart&&found[0].periodEnd===fixture.expectedEnd&&found[0].attempt===1&&details.length===1;
 save('worklog-catchup-'+stage+'.json',{fixture,runs,scheduledRuns:found,schedules,details,pass,method:'stopped-app synthetic overdue fixture; actual native scheduler and loopback model; OS clock unchanged'});
 console.log(JSON.stringify({stage,runs:found.map(r=>({id:r.id,state:r.state,attempt:r.attempt,periodStart:r.periodStart,periodEnd:r.periodEnd})),pass}));if(!pass)throw Error('Catch-up duplicate or wrong period');
}finally{c.close()}
