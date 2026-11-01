import {connect,save,until} from './cdp-client';
const c=await connect('chat');
const read=()=>c.evaluate(`({receipts:[...document.querySelectorAll('.chat-worklog-receipt')].map(x=>x.innerText),text:document.body.innerText})`);
try {
 const before:any=await until(read,(x:any)=>x.receipts.some((s:string)=>s.includes('NEEDS_EXPLICIT_REQUEST')));
 const entries:any=await c.invoke('worklog_query',{query:{start:'2026-10-27',end:'2026-10-27'}});
 if(entries.length!==1||entries[0].id!=='ce755513-642c-4d29-8038-6e4ab22b3dc2')throw Error('Unexpected synthetic entry');
 const e=entries[0];const receipt=await c.invoke('worklog_update',{request:{requestId:crypto.randomUUID(),id:e.id,expectedRevision:e.revision,businessDate:e.businessDate,project:e.project,text:e.text,status:e.status}});
 await Bun.sleep(500);
 const after:any=await read();
 const pass=after.receipts.some((s:string)=>s.includes('NEEDS_EXPLICIT_REQUEST'))&&!after.receipts.some((s:string)=>s.includes('正在确认保存结果'));
 save('receipt-native-retest.json',{method:'real mouse history selection and real Enter continuation; supported native IPC updates the synthetic entry to emit worklog-changed',before,after,receipt,pass});
 console.log(JSON.stringify({before:before.receipts,after:after.receipts,pass}));if(!pass)throw Error('Rejection lost after actual host change');
}finally{c.close()}
