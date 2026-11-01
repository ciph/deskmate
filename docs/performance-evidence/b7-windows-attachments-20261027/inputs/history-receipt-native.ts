import {connect,save,until} from './cdp-client';
const c=await connect('chat');
try{
 const initial=await c.evaluate(`({text:document.body.innerText,buttons:[...document.querySelectorAll('button')].map(x=>({text:x.innerText,label:x.getAttribute('aria-label'),title:x.title}))})`);
 save('history-receipt-native-initial.json',initial);
 const selector=await c.evaluate(`(()=>{const b=[...document.querySelectorAll('button')].find(x=>/历史/.test(x.getAttribute('aria-label')||''));if(!b)throw Error('No history control');const label=b.getAttribute('aria-label');b.click();return label})()`);
 const row:any=await until(()=>c.evaluate(`(()=>{const rows=[...document.querySelectorAll('.history-organizer-row')];return rows.map(x=>({text:x.innerText,html:x.outerHTML.slice(0,1800)}))})()`),(x:any)=>x.length>0);
 save('history-receipt-native-rows.json',{selector,rows:row});
 const catalog:any=await c.invoke('history_catalog_list',{query:{limit:50},refresh:false});
 const key=catalog.items.find((x:any)=>x.identity.sessionId==='ses_e83489dffffepPRvAJzPXGkYh9')?.key;
 if(!key)throw Error('Missing synthetic session');
 const chosen=await c.evaluate(`(()=>{const r=[...document.querySelectorAll('.history-organizer-row')].find(x=>x.dataset.previewKey===${JSON.stringify(key)});if(!r)throw Error('Synthetic worklog session row missing');const b=r.querySelector('.history-organizer-open');if(!b)throw Error('Missing row opener');b.click();return r.innerText})()`);
 const restored:any=await until(()=>c.evaluate(`({receipts:[...document.querySelectorAll('.chat-worklog-receipt')].map(x=>x.innerText),text:document.body.innerText})`),(x:any)=>x.receipts.length>=5);
 const pass=restored.receipts.some((s:string)=>s.includes('NEEDS_EXPLICIT_REQUEST'))&&!restored.receipts.some((s:string)=>s.includes('正在确认保存结果'));
 save('history-receipt-native-restored.json',{method:'programmatic DOM activation inside actual QA WebView2, not physical mouse input; no new chat message',chosen,restored,pass});
 if(!pass)throw Error('Historical receipts were not restored');
 await c.evaluate(`[...document.querySelectorAll('.chat-worklog-receipt')].find(x=>x.innerText.includes('NEEDS_EXPLICIT_REQUEST'))?.scrollIntoView({block:'center'})`);
 console.log(JSON.stringify({receipts:restored.receipts,pass}));
}finally{c.close()}
