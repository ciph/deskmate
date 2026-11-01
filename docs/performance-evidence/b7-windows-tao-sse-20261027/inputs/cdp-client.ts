import {writeFileSync} from 'node:fs';
export const report='E:/Codex/工作伙伴/reports/yume-b7-tao-sse-20261027';
export const save=(name:string,data:unknown)=>writeFileSync(`${report}/${name}`,JSON.stringify(data,null,2));
export async function connect(page:string){
 const targets=await(await fetch('http://127.0.0.1:9241/json',{signal:AbortSignal.timeout(5000)})).json();
 const target=targets.find((x:any)=>page==='workbench'?x.url.includes('/workbench/'):x.url.endsWith('/'+page+'.html'));
 if(!target)throw Error('No target '+page);
 const ws=new WebSocket(target.webSocketDebuggerUrl);
 await new Promise((resolve,reject)=>{ws.addEventListener('open',resolve,{once:true});ws.addEventListener('error',reject,{once:true})});
 let sequence=0;const pending=new Map<number,any>();
 ws.addEventListener('message',event=>{const x=JSON.parse(String(event.data)),p=pending.get(x.id);if(p){pending.delete(x.id);clearTimeout(p.timer);if(x.error||x.result?.exceptionDetails)p.reject(Error(JSON.stringify(x.error||x.result.exceptionDetails)));else p.resolve(x.result)}});
 const call=(method:string,params:unknown)=>new Promise<any>((resolve,reject)=>{const id=++sequence;const timer=setTimeout(()=>{pending.delete(id);reject(Error('Deadline '+method))},45000);pending.set(id,{resolve,reject,timer});ws.send(JSON.stringify({id,method,params}))});
 const evaluate=async(expression:string)=>(await call('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true})).result.value;
 const invoke=(command:string,args:unknown={})=>evaluate(`window.__TAURI__.core.invoke(${JSON.stringify(command)},${JSON.stringify(args)})`);
 return {evaluate,invoke,close:()=>ws.close(),url:target.url};
}
export async function until<T>(fn:()=>Promise<T>,accept:(x:T)=>boolean,budget=15000){const start=Date.now();let value:T;do{value=await fn();if(accept(value))return value;await Bun.sleep(100)}while(Date.now()-start<budget);throw Error('Condition deadline; last='+JSON.stringify(value!));}
