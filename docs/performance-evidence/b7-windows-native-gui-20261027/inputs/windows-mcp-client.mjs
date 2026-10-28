import {spawn} from 'node:child_process';
import {createInterface} from 'node:readline';
import {readFileSync,writeFileSync,appendFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
const root='E:/Codex/工作伙伴/deskmate-windows-b7', report='E:/Codex/工作伙伴/reports/yume-b7-native-gui-20261027';
const binary=root+'/output/b7-native-gui/Sbroenne.WindowsMcp.exe';
const expected=JSON.parse(readFileSync(root+'/components/windows-mcp.json','utf8')).binarySha256;
if(createHash('sha256').update(readFileSync(binary)).digest('hex')!==expected)throw Error('Pinned Windows MCP hash mismatch');
const names=['window_management','screenshot_control','mouse_control','keyboard_control','ui_snapshot','ui_find','ui_click','ui_type','ui_read','ui_wait','app'];
// The pinned console host omits WinForms DPI initialization; scope this compatibility setting to this QA child only.
const child=spawn(binary,['--tools',names.join(',')],{stdio:['pipe','pipe','pipe'],windowsHide:true,env:{...process.env,__COMPAT_LAYER:'HIGHDPIAWARE'}});
const pending=new Map();let seq=0;
child.stderr.on('data',chunk=>appendFileSync(report+'/windows-mcp.stderr.log',chunk));
createInterface({input:child.stdout}).on('line',line=>{let value;try{value=JSON.parse(line);}catch{appendFileSync(report+'/windows-mcp.nonjson.log',line+'\n');return;}if(value.id!==undefined){const row=pending.get(value.id);if(row){pending.delete(value.id);clearTimeout(row.timer);value.error?row.reject(Error(JSON.stringify(value.error))):row.resolve(value.result);}}});
child.on('exit',(code,signal)=>{writeFileSync(report+'/windows-mcp-exit.json',JSON.stringify({pid:child.pid,code,signal,time:new Date().toISOString()},null,2));for(const row of pending.values()){clearTimeout(row.timer);row.reject(Error('Windows MCP exited'));}pending.clear();});
function request(method,params){return new Promise((resolve,reject)=>{const id=++seq,timer=setTimeout(()=>{pending.delete(id);reject(Error(method+' timed out'));},45000);pending.set(id,{resolve,reject,timer});child.stdin.write(JSON.stringify({jsonrpc:'2.0',id,method,params})+'\n');});}
export const client={pid:child.pid,async initialize(){const value=await request('initialize',{protocolVersion:'2024-11-05',capabilities:{},clientInfo:{name:'b7-scoped-native-gui',version:'1.0'}});child.stdin.write(JSON.stringify({jsonrpc:'2.0',method:'notifications/initialized'})+'\n');writeFileSync(report+'/windows-mcp-initialize.json',JSON.stringify({pid:child.pid,binary,sha256:expected,value,time:new Date().toISOString()},null,2));return value;},async tools(){const result=await request('tools/list',{});writeFileSync(report+'/windows-mcp-tools.json',JSON.stringify(result,null,2));return result;},async call(name,args){if(!names.includes(name))throw Error('Tool outside temporary QA allowlist');const result=await request('tools/call',{name,arguments:args});appendFileSync(report+'/windows-mcp-calls.jsonl',JSON.stringify({time:new Date().toISOString(),name,args,result})+'\n');return result;},close(){child.stdin.end();}};
