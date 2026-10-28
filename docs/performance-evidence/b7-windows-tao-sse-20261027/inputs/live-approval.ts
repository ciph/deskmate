import { spawn } from 'node:child_process';
import { mkdir, mkdtemp, writeFile, readFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { childEnv, freePort, stopChild } from '../../scripts/sidecar-harness/process';
import { portClosed, processGone, removeTempRoot } from '../../scripts/sidecar-harness/cleanup';
import { buildSidecarConfig } from '../../scripts/sidecar-harness/permissions';
import { startProvider } from '../../scripts/worklog/provider';
import { createServer, request as httpRequest } from 'node:http';
const report=resolve('../reports/yume-b7-tao-sse-20261027');
const label=process.argv[2]??'before';
const root=await mkdtemp(resolve('output/b7-tao-sse/live-'));
const workspace=join(root,'workspace');await mkdir(workspace);
const provider=await startProvider();const port=await freePort();const base=`http://127.0.0.1:${port}`;
const env=childEnv({root,providerBaseUrl:provider.baseUrl,runtimeCanary:'b7-sse-synthetic'});
env.OPENCODE_CONFIG_CONTENT=JSON.stringify({...buildSidecarConfig(provider.baseUrl),permission:{'*':'deny',webfetch:'ask'}});
const engine=spawn(resolve('src-tauri/resources/opencode/opencode.exe'),['--pure','serve','--hostname','127.0.0.1','--port',String(port),'--print-logs'],{cwd:workspace,env,windowsHide:true,stdio:['ignore','pipe','pipe']});
let logs='';engine.stdout.on('data',x=>logs+=x);engine.stderr.on('data',x=>logs+=x);
let test:ReturnType<typeof Bun.spawn>|undefined;const evidence:any={label,base,pid:engine.pid,events:[]};const begin=performance.now();
function mark(event:string,extra:any={}){evidence.events.push({event,ms:performance.now()-begin,...extra})}
let sseRequests=0;const sockets=new Set<any>();const stalledSockets=new Set<any>();
const proxy=createServer((req,res)=>{
 const sse=req.url==='/global/event';
 if(sse){sseRequests++;mark('sse-request',{attempt:sseRequests});}
 if(sse&&sseRequests===1&&label.includes('fault')){stalledSockets.add(req.socket);req.socket.on('close',()=>{stalledSockets.delete(req.socket);mark('stalled-header-closed')});return;}
 const upstream=httpRequest(base+req.url,{method:req.method,headers:{...req.headers,host:`127.0.0.1:${port}`}},reply=>{
  res.writeHead(reply.statusCode??500,reply.headers);reply.pipe(res);
  if(sse)reply.on('data',chunk=>{const text=chunk.toString();if(text.includes('server.heartbeat'))mark('heartbeat');if(text.includes('permission.asked'))mark('permission-event');});
 });
 upstream.on('error',()=>res.destroy());res.on('close',()=>upstream.destroy());req.pipe(upstream);
});
proxy.on('connection',socket=>{sockets.add(socket);socket.on('close',()=>sockets.delete(socket))});
await new Promise<void>(done=>proxy.listen(0,'127.0.0.1',done));
const proxyPort=(proxy.address() as any).port;const nativeBase=`http://127.0.0.1:${proxyPort}`;
async function api(path:string,body?:unknown){const r=await fetch(base+path,{method:body?'POST':'GET',headers:{'Content-Type':'application/json'},body:body?JSON.stringify(body):undefined,signal:AbortSignal.timeout(10000)});if(!r.ok)throw Error(`${path} ${r.status}`);const t=await r.text();return t?JSON.parse(t):null;}
try{
 const deadline=Date.now()+30000;while(true){try{await api('/global/health');break}catch(e){if(Date.now()>deadline)throw e;await Bun.sleep(100)}}mark('healthy');
 const ready=join(root,'ready');const out=join(root,'rust.log');const err=join(root,'rust.stderr.log');
 const binary=process.env.B7_TEST_EXE??resolve('src-tauri/target/debug/deps/yume_lib-7c57d07baf708583.exe');
 evidence.binary=binary;
 test=Bun.spawn([binary,'tool_permissions::events::tests::live_web_permission_without_timeout','--ignored','--exact','--nocapture'],{cwd:resolve('src-tauri'),env:{...process.env,YUME_PERMISSION_TEST_BASE:nativeBase,YUME_PERMISSION_TEST_DIRECTORY:workspace,YUME_PERMISSION_TEST_READY:ready},stdout:Bun.file(out),stderr:Bun.file(err),windowsHide:true});
 const until=Date.now()+50000;while(!await Bun.file(ready).exists()){if(test.exitCode!==null)throw Error('native reader exited before readiness');if(Date.now()>until)throw Error('native readiness timeout');await Bun.sleep(50)}mark('native-ready');
 mark('stalled-sockets-at-ready',{count:stalledSockets.size});
 if(label.includes('after'))await Bun.sleep(12000);
 provider.selectTool('webfetch',{url:provider.baseUrl+'/models',format:'text'});
 const session=await api('/session',{title:'B7 isolated approval SSE'});mark('session',{id:session.id});
 await api(`/session/${session.id}/prompt_async`,{model:{providerID:'yume',modelID:'model-a'},parts:[{type:'text',text:'Run the selected test tool'}]});mark('prompt');
 const code=await test.exited;mark('native-done',{code});if(code!==0)throw Error('native approval failed');
 const pending=await api('/permission');if(pending.length)throw Error('stale pending requests');
 if(label.includes('fault')&&sseRequests<2)throw Error('fault did not reconnect');evidence.pass=true;
}catch(error){evidence.failure=String(error);process.exitCode=1}
finally{
 if(test?.exitCode===null){test.kill();await test.exited}
 for(const socket of sockets)socket.destroy();await new Promise<void>(done=>proxy.close(()=>done()));
 for(const file of ['rust.log','rust.stderr.log'])try{await writeFile(join(report,`live-approval-${label}-${file}`),await readFile(join(root,file)))}catch{}
 await stopChild(engine);await provider.close();
 evidence.cleanup={engineGone:await processGone(engine.pid),portClosed:await portClosed(port),providerClosed:await portClosed(provider.port),proxyClosed:await portClosed(proxyPort)};
 await writeFile(join(report,`live-approval-${label}.json`),JSON.stringify(evidence,null,2));await writeFile(join(report,`live-approval-${label}-engine.log`),logs);
 await removeTempRoot(root);console.log(JSON.stringify(evidence));
}
