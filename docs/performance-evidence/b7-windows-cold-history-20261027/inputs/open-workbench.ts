import {connect,save,until} from './cdp-client';
const chat=await connect('chat');
try {
 const page=await chat.invoke('history_catalog_list',{query:{limit:50},refresh:false});
 const row=page.items.find((x:any)=>x.identity.sessionId==='ses_winb7focus1000020261027');
 if(!row)throw Error('Missing synthetic history');
 await chat.evaluate(`(()=>{const button=[...document.querySelectorAll('button.history-organizer-open')].find(x=>x.textContent.includes('10000'));if(!button)throw Error('No history button');button.click()})()`);
 await until(()=>chat.evaluate(`({messages:document.querySelectorAll('.message-row').length,history:!!document.querySelector('.history-organizer'),text:document.body.innerText.slice(-800)})`),x=>!x.history,15000);
 const before=await chat.evaluate(`({text:document.body.innerText.slice(-1000),buttons:[...document.querySelectorAll('button')].map(x=>({text:x.innerText,label:x.getAttribute('aria-label'),title:x.title}))})`);
 await chat.invoke('workbench_open_session',{sessionId:row.identity.sessionId,directory:row.identity.directory});
 save('multiwindow-open.json',{method:'actual host IPC and WebView2 DOM (not physical mouse)',row,before,opened:true});
 console.log(JSON.stringify({ok:true,key:row.key}));
}finally{chat.close()}
