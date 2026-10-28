import {writeFileSync} from 'node:fs';
import {randomBytes} from 'node:crypto';
import {client} from './windows-mcp-client.mjs';
const root='E:/Codex/工作伙伴/deskmate-windows-b7/output/b7-native-gui';
await client.initialize();await client.tools();
const token=randomBytes(24).toString('hex');
let busy=false;
const server=Bun.serve({hostname:'127.0.0.1',port:0,idleTimeout:60,async fetch(req){
 if(req.headers.get('authorization')!==`Bearer ${token}`)return new Response('forbidden',{status:403});
 if(req.method!=='POST')return new Response('method',{status:405});
 if(new URL(req.url).pathname==='/close'){client.close();setTimeout(()=>{server.stop();process.exit(0);},500);return Response.json({closing:true});}
 if(busy)return new Response('busy',{status:409});busy=true;
 try{const {name,args}=await req.json() as {name:string,args:Record<string,unknown>};return Response.json(await client.call(name,args));}catch(error){return Response.json({error:String(error)},{status:500});}finally{busy=false;}
}});
writeFileSync(root+'/bridge-session.json',JSON.stringify({pid:process.pid,mcpPid:client.pid,port:server.port,token,time:new Date().toISOString()},null,2));
console.log(JSON.stringify({pid:process.pid,mcpPid:client.pid,port:server.port,ready:true}));
process.on('SIGINT',()=>{client.close();server.stop(true);process.exit(0);});
