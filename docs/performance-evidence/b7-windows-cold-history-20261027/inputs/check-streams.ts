import {readFileSync} from 'node:fs';
import {connect,save,until,report} from './cdp-client';
const read=(name:string)=>JSON.parse(readFileSync(report+'/'+name,'utf8').replace(/^\uFEFF/,''));
const normal=read('regression-10k-final.json'),stopped=read('regression-stop-final.json');
for(const x of [normal,stopped])if(!x.ok||x.result.initialRows!==1000||x.result.stableOldNodes!==1000||!x.result.focusPreserved||!x.result.draftRestored||!x.final.composer||!x.final.sameInput||x.result.busy||x.trace.some((t:any)=>t.kind==='ipc-error'))throw Error('Stream regression');
if(!stopped.result.stopClicked)throw Error('Stop was not clicked');
const c=await connect('chat');
try {
 const page=await c.invoke('history_catalog_list',{query:{limit:50},refresh:false});
 const find=(id:string)=>page.items.find((x:any)=>x.identity.sessionId===id);
 const row=find('ses_winb7focus1000020261027'), stopRow=find('ses_winb7focus5000020261027');
 const loaded=await c.invoke('history_catalog_load',{key:row.key});
 const text=loaded.messages.filter((x:any)=>x.role==='assistant').at(-1).text;
 const expected=await(await fetch('http://127.0.0.1:58031/expected/10000')).text();
 const sha=(text:string)=>new Bun.CryptoHasher('sha256').update(text).digest('hex');
 if(text!==expected)throw Error('Native streamed text mismatch');
 await c.evaluate(`(()=>{const input=document.querySelector('textarea.chat-input');if(!input||input.disabled)throw Error('Composer not ready after Stop');Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype,'value').set.call(input,'B7_CONTINUE');input.dispatchEvent(new Event('input',{bubbles:true}));setTimeout(()=>input.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',code:'Enter',bubbles:true})),100)})()`);
 await until(()=>c.evaluate(`({done:!document.querySelector('button.chat-abort')&&[...document.querySelectorAll('.chat-msg-assistant')].at(-1)?.textContent.includes('B7_LOCAL_REPLY_OK')})`),x=>x.done,20000);
 const after=await until(()=>c.invoke('history_catalog_load',{key:stopRow.key}),x=>x.messages.length===1004&&x.messages.filter((m:any)=>m.role==='assistant').at(-1).text==='B7_LOCAL_REPLY_OK');
 const requests=await(await fetch('http://127.0.0.1:58031/status')).json();
 if(!requests.requests.some((x:any)=>x.size===10000&&x.done&&x.emitted===500)||!requests.requests.some((x:any)=>x.size===50000&&x.aborted&&!x.done&&x.emitted<500))throw Error('Provider completion/abort mismatch');
 save('stream-regression-checks.json',{ok:true,initialMessages:1000,preservedOldNodes:1000,focusAndDraftPreserved:true,composerIdentityPreserved:true,stopClicked:true,normal:{messages:loaded.messages.length,characters:text.length,sha256:sha(text),expectedSha256:sha(expected)},continuedAfterStop:{messages:after.messages.length,reply:'B7_LOCAL_REPLY_OK'},provider:requests.requests.map(({times,...x}:any)=>x)});
 console.log(JSON.stringify({ok:true,normalMessages:loaded.messages.length,afterStopContinuation:after.messages.length,sha256:sha(text)}));
}finally{c.close()}
