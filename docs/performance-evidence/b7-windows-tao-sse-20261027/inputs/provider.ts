import {startProvider} from './tool-provider';
import {writeFileSync} from 'node:fs';
import {join} from 'node:path';
const provider=await startProvider();
provider.selectTool('webfetch',{url:provider.baseUrl+'/models',format:'text'});
provider.setCompletionText('B7_APPROVAL_COMPLETED');
const control=Bun.serve({hostname:'127.0.0.1',port:0,async fetch(req){
 const path=new URL(req.url).pathname;
 if(path==='/status')return Response.json(provider.observations);
 if(path==='/select'&&req.method==='POST'){const value=await req.json();provider.selectTool(value.name,value.args);return Response.json({selected:value.name});}
 if(path==='/close'&&req.method==='POST'){writeFileSync(join(import.meta.dir,'provider-observations.json'),JSON.stringify(provider.observations,null,2));await provider.close();setTimeout(()=>{control.stop(true);process.exit(0)},300);return Response.json({closing:true});}
 return new Response('not found',{status:404});
}});
const receipt={pid:process.pid,port:provider.port,baseUrl:provider.baseUrl,controlPort:control.port};
writeFileSync(join(import.meta.dir,'provider-session.json'),JSON.stringify(receipt,null,2));console.log(JSON.stringify(receipt));
