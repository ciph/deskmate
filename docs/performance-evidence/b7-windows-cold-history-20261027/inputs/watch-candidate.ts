import {readFileSync,writeFileSync} from 'node:fs';
const out=process.argv[2],script=process.argv[3]??'output/b7-cold-history/cold-monitor-fixed.js';
let target;const start=Date.now();
while(Date.now()-start<60000){try{const rows=await(await fetch('http://127.0.0.1:9240/json')).json();target=rows.find(x=>x.url.endsWith('/chat.html'));if(target)break}catch{}await Bun.sleep(100)}
if(!target)throw Error('CDP not ready');
const ws=new WebSocket(target.webSocketDebuggerUrl);await new Promise((r,j)=>{ws.addEventListener('open',r,{once:true});ws.addEventListener('error',j,{once:true})});
let sequence=0;const pending=new Map();ws.addEventListener('message',e=>{const x=JSON.parse(String(e.data)),p=pending.get(x.id);if(p){pending.delete(x.id);if(x.error||x.result?.exceptionDetails)p.reject(Error(JSON.stringify(x.error||x.result.exceptionDetails)));else p.resolve(x.result.result.value)}});
const evaluate=expression=>new Promise((resolve,reject)=>{const id=++sequence;pending.set(id,{resolve,reject});ws.send(JSON.stringify({id,method:'Runtime.evaluate',params:{expression,returnByValue:true,awaitPromise:true}}))});
const timer=setTimeout(()=>{console.error('probe deadline');process.exit(1)},100000);
try{let ready=false;while(Date.now()-start<60000){try{ready=await evaluate("!!window.__TAURI__ && !!document.querySelector('button[aria-label=历史]')");if(ready)break}catch{}await Bun.sleep(50)}if(!ready)throw Error('React view not ready');const readyMs=Date.now()-start;const result=await evaluate(readFileSync(script,'utf8'));writeFileSync(out,JSON.stringify({cdpAndReactReadyMs:readyMs,elapsedMs:Date.now()-start,result},null,2));console.log(JSON.stringify({out,ok:result?.ok}));if(result?.ok===false)process.exitCode=1;}finally{clearTimeout(timer);ws.close()}
