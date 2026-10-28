import {spawn} from 'node:child_process';
import {mkdirSync,writeFileSync} from 'node:fs';
import {resolve,join} from 'node:path';
import {freePort,stopChild} from '../../scripts/sidecar-harness/process';
const report='E:/Codex/工作伙伴/reports/yume-b7-tao-sse-20261027';
const root=resolve('output/b7-tao-sse/real-sse-home'),workspace=join(root,'workspace');mkdirSync(workspace,{recursive:true});
const port=await freePort(),base=`http://127.0.0.1:${port}`;
const env={...process.env,HOME:root,USERPROFILE:root,APPDATA:join(root,'AppData/Roaming'),LOCALAPPDATA:join(root,'AppData/Local'),XDG_CONFIG_HOME:join(root,'xdg-config'),XDG_DATA_HOME:join(root,'xdg-data'),XDG_CACHE_HOME:join(root,'xdg-cache'),OPENCODE_DISABLE_AUTOUPDATE:'1',OPENCODE_CONFIG_CONTENT:'{"$schema":"https://opencode.ai/config.json","permission":{"*":"deny"}}'};
delete env.OPENCODE_SERVER_PASSWORD;
const child=spawn(resolve('src-tauri/resources/opencode/opencode.exe'),['serve','--hostname','127.0.0.1','--port',String(port),'--print-logs'],{cwd:workspace,env,windowsHide:true,stdio:['ignore','pipe','pipe']});
let logs='';child.stdout.on('data',x=>logs+=x);child.stderr.on('data',x=>logs+=x);
const begin=performance.now(),events=[];
try{
 let healthy=false;for(let i=0;i<150;i++){try{const r=await fetch(base+'/global/health',{signal:AbortSignal.timeout(1000)});if(r.ok){healthy=true;break}}catch{}await Bun.sleep(200)}if(!healthy)throw Error('health timeout');
 events.push({event:'healthy',at:performance.now()-begin});
 const controller=new AbortController();const timer=setTimeout(()=>controller.abort(),55000);
 try{const response=await fetch(base+'/global/event',{signal:controller.signal});events.push({event:'headers',at:performance.now()-begin,status:response.status,headers:Object.fromEntries(response.headers)});const reader=response.body!.getReader();for(let i=0;i<6;i++){const r=await reader.read();events.push({event:'read',at:performance.now()-begin,done:r.done,text:new TextDecoder().decode(r.value)});if(r.done)break;}await reader.cancel();}catch(error){events.push({event:'error',at:performance.now()-begin,error:String(error)})}finally{clearTimeout(timer);controller.abort()}
}finally{await stopChild(child);writeFileSync(join(report,'real-sse-baseline.json'),JSON.stringify({base,port,pid:child.pid,events,stopped:child.exitCode!==null},null,2));writeFileSync(join(report,'real-sse-baseline.log'),logs)}
console.log(JSON.stringify(events));
