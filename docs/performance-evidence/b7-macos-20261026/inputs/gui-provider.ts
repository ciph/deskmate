import { writeFileSync } from 'node:fs';
import { join } from 'node:path';
const output = import.meta.dir;
let delayMs = 0;
const requests: {at:string, pdfTextPresent:boolean, delayMs:number, aborted:boolean}[]=[];
const server = Bun.serve({hostname:'127.0.0.1',port:0,async fetch(request){
  const url=new URL(request.url);
  if(request.method==='GET' && url.pathname==='/v1/models') return Response.json({object:'list',data:[{id:'b7-local',object:'model',owned_by:'b7-fixture'}]});
  if(url.pathname==='/control' && request.method==='POST') { const x=await request.json() as {delayMs:number}; if(!Number.isInteger(x.delayMs)||x.delayMs<0||x.delayMs>30000)return new Response('bad delay',{status:400});delayMs=x.delayMs;return Response.json({delayMs}); }
  if(url.pathname==='/status') return Response.json({requests});
  if(request.method!=='POST'||url.pathname!=='/v1/chat/completions')return new Response('not found',{status:404});
  const body=await request.json() as {messages:unknown,stream?:boolean};
  const row={at:new Date().toISOString(),pdfTextPresent:JSON.stringify(body.messages).includes('YUME B7 PDF WebKit fixture'),delayMs,aborted:false};requests.push(row);
  request.signal.addEventListener('abort',()=>{row.aborted=true;writeFileSync(join(output,'gui-provider-requests.json'),JSON.stringify(requests,null,2));},{once:true});
  if(row.delayMs)await Bun.sleep(row.delayMs);
  const content=row.pdfTextPresent?'B7_PDF_TEXT_CONFIRMED: YUME B7 PDF WebKit fixture':'B7_LOCAL_REPLY_OK';
  const base={id:'chatcmpl-b7-'+crypto.randomUUID(),model:'b7-local',created:Math.floor(Date.now()/1000)};
  writeFileSync(join(output,'gui-provider-requests.json'),JSON.stringify(requests,null,2));
  if(!body.stream)return Response.json({...base,object:'chat.completion',choices:[{index:0,message:{role:'assistant',content},finish_reason:'stop'}],usage:{prompt_tokens:1,completion_tokens:1,total_tokens:2}});
  const chunks=[{index:0,delta:{role:'assistant',content},finish_reason:null},{index:0,delta:{},finish_reason:'stop'}];
  return new Response(chunks.map(c=>'data: '+JSON.stringify({...base,object:'chat.completion.chunk',choices:[c]})+'\n\n').join('')+'data: [DONE]\n\n',{headers:{'Content-Type':'text/event-stream'}});
}});
const receipt={pid:process.pid,baseUrl:`http://127.0.0.1:${server.port}/v1`,controlUrl:`http://127.0.0.1:${server.port}/control`,statusUrl:`http://127.0.0.1:${server.port}/status`,model:'b7-local',dummyValue:'b7-fixture-not-a-secret',startedAt:new Date().toISOString()};
writeFileSync(join(output,'gui-provider-receipt.json'),JSON.stringify(receipt,null,2));console.log(JSON.stringify(receipt));
for(const signal of ['SIGTERM','SIGINT'] as const)process.on(signal,()=>{server.stop(true);writeFileSync(join(output,'gui-provider-cleanup.json'),JSON.stringify({pid:process.pid,stopped:true,requests},null,2));process.exit(0);});
