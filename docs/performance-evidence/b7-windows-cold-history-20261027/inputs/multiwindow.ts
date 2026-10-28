import {connect,save,until} from './cdp-client';
const output=process.argv[2]??'multiwindow-acceptance.json';
const chat=await connect('chat'),workbench=await connect('workbench'),settings=await connect('settings');
const trace:any[]=[];const start=Date.now();
const log=(step:string,data:any)=>trace.push({step,atMs:Date.now()-start,...data});
const fail=(ok:boolean,message:string)=>{if(!ok)throw Error(message)};
try {
 const page=await chat.invoke('history_catalog_list',{query:{limit:50},refresh:false});
 const row=page.items.find((x:any)=>x.identity.sessionId==='ses_winb7focus1000020261027');
 if(!row)throw Error('Missing synthetic history');
 const key=row.key;
 for(const c of [chat,workbench])await c.evaluate(`(async()=>{window.__b7CatalogEvents=[];window.__b7Stop=await window.__TAURI__.event.listen('history-catalog-changed',e=>window.__b7CatalogEvents.push({at:Date.now(),event:e.event}));return true})()`);
 const owned=await until(()=>chat.invoke('history_catalog_access',{key}),x=>x.ownership==='workbench');
 const chatOwnedDom=await until(()=>chat.evaluate(`({composer:!!document.querySelector('.chat-input-row'),tail:document.body.innerText.slice(-500)})`),x=>!x.composer);
 fail(!owned.capabilities.send,'Chat must be read only while workbench owns session');log('workbench-owns-session',{owned,chatOwnedDom});
 const denied=async(c:any,command:string,args:any)=>{try{await c.invoke(command,args);throw Error('unexpected success '+command)}catch(error){const message=String(error);if(message.includes('unexpected success'))throw error;return message}};
 const deniedSettings=await denied(settings,'history_catalog_list',{query:{limit:50},refresh:false});
 log('settings-denied',{deniedSettings});
 fail(deniedSettings.includes('history_forbidden'),'Settings window authorization');
 const deniedClaim=await denied(chat,'workbench_claim_session',{sessionId:row.identity.sessionId,directory:row.identity.directory});
 fail(deniedClaim.includes('only available to the workbench'),'Chat cannot claim workbench ownership');
 const deniedWrite=await denied(chat,'history_save_local_messages',{key,messages:[]});
 log('local-write-denied',{deniedWrite});
 fail(deniedWrite.includes('history_local_messages_denied'),'Owned conversation rejects stale local write');
 log('authorization',{deniedSettings,deniedClaim,deniedWrite});
 const renamed='B7 multiwindow verified 10000';
 await workbench.invoke('history_catalog_mutate',{key,mutation:{action:'rename',title:renamed}});
 await until(()=>workbench.evaluate(`document.body.innerText.includes(${JSON.stringify(renamed)})`),Boolean);
 await chat.evaluate(`document.querySelector('button[aria-label=历史]').click()`);
 await until(()=>chat.evaluate(`!![...document.querySelectorAll('button.history-organizer-open')].find(x=>x.textContent.includes(${JSON.stringify(renamed)}))`),Boolean);
 const concurrent=await Promise.all([
   chat.invoke('history_catalog_mutate',{key,mutation:{action:'pin',pinned:true}}),
   workbench.invoke('history_catalog_mutate',{key,mutation:{action:'rename',title:renamed+' concurrent'}}),
 ]);
 const after=await chat.invoke('history_catalog_access',{key});
 fail(after.pinned&&after.displayTitle===renamed+' concurrent','Concurrent independent metadata updates survive');
 log('cross-window-metadata',{concurrent:concurrent.map(x=>({pinned:x.pinned,displayTitle:x.displayTitle})),after});
 await workbench.invoke('history_catalog_mutate',{key,mutation:{action:'archive',archived:true}});
 const archived=await until(()=>chat.invoke('history_catalog_access',{key}),x=>x.archived===true);
 fail(!archived.capabilities.send,'Archived history must remain read only');
 await until(()=>chat.evaluate(`document.querySelectorAll('button.history-organizer-open').length`),x=>x===1);
 log('archive-sync',{archived,visibleRows:1});
 await workbench.invoke('history_catalog_mutate',{key,mutation:{action:'archive',archived:false}});
 await until(()=>chat.evaluate(`document.querySelectorAll('button.history-organizer-open').length`),x=>x===2);
 await workbench.invoke('history_catalog_mutate',{key,mutation:{action:'rename',title:'B7 Windows 1000 messages 10000'}});
 await chat.invoke('history_catalog_mutate',{key,mutation:{action:'pin',pinned:false}});
 await workbench.invoke('workbench_hide');
 const released=await until(()=>chat.invoke('history_catalog_access',{key}),x=>x.ownership==='unowned'&&x.capabilities.send);
 await chat.evaluate(`(()=>{const b=[...document.querySelectorAll('button.history-organizer-open')].find(x=>x.textContent.includes('10000'));if(!b)throw Error('Missing restored row');b.click()})()`);
 const releasedDom=await until(()=>chat.evaluate(`({composer:!!document.querySelector('.chat-input-row'),textarea:!!document.querySelector('textarea'),tail:document.body.innerText.slice(-200)})`),x=>x.composer&&x.textarea);
 const loaded=await chat.invoke('history_catalog_load',{key});
 fail(loaded.messages.length===1000,'Native transcript preserved through ownership and metadata changes');
 const events=await Promise.all([chat.evaluate('window.__b7CatalogEvents'),workbench.evaluate('window.__b7CatalogEvents')]);
 fail(events.every(x=>x.length>=6),'Both actual windows receive catalog events');
 log('release-and-preservation',{released,releasedDom,messageCount:loaded.messages.length,eventCounts:events.map(x=>x.length)});
 save(output,{ok:true,method:'real WebView2 windows + host IPC + native OpenCode; no physical mouse or visual acceptance',trace});console.log(JSON.stringify({ok:true,steps:trace.length,events:events.map(x=>x.length)}));
}catch(error){save(output,{ok:false,error:String(error),trace});console.error(error);process.exitCode=1}
finally{for(const c of [chat,workbench])await c.evaluate('window.__b7Stop?.()').catch(()=>{});for(const c of [chat,workbench,settings])c.close()}
