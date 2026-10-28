import {connect} from './cdp-client';
import {writeFileSync} from 'node:fs';
const c=await connect('chat');
try {
 const page=await c.invoke('history_catalog_list',{query:{limit:50},refresh:false});
 const row=page.items.find(x=>x.identity.sessionId==='ses_winb7focus1000120261027');
 if(!row)throw Error('QA session absent');
 const loaded=await c.invoke('history_catalog_load',{key:row.key});
 const expected=await(await fetch('http://127.0.0.1:51404/expected/10000')).text();
 const full=loaded.messages.find(x=>x.role==='assistant'&&x.text===expected);
 if(!full)throw Error('Full 10k native text mismatch');
 const media=await c.evaluate(`Array.from(document.querySelectorAll('audio')).map(a=>({time:a.currentTime,duration:a.duration,paused:a.paused,ended:a.ended,error:a.error?.code,ready:a.readyState,source:a.currentSrc}))`);
 const data={session:row.identity.sessionId,messages:loaded.messages.length,sha256:new Bun.CryptoHasher('sha256').update(full.text).digest('hex'),chars:full.text.length,attachments:loaded.messages.filter(x=>JSON.stringify(x).includes('B7-native-')).map(x=>({role:x.role,text:x.text,attachments:x.attachments})),media};
 writeFileSync('E:/Codex/工作伙伴/reports/yume-b7-native-gui-20261027/native-results-check.json',JSON.stringify(data,null,2));
 console.log(JSON.stringify(data));
}finally{c.close()}
