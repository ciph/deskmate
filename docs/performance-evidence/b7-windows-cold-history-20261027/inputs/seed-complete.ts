import {join} from 'node:path';
import {mkdirSync} from 'node:fs';
const repo=process.cwd(), report='E:/Codex/工作伙伴/reports/yume-b7-cold-history-20261027';
const identity='com.deskmate.unifiedhistoryqa.winb7cold20261027h';
const receipt=await Bun.file(join(report,'native-processes-complete.json')).json();
if(receipt.identity!==identity)throw Error('identity');
const data=receipt.roots[0], home=join(data,'opencode-home'), workspace=join(data,'workspace');
if(!data.endsWith(identity))throw Error('root');
mkdirSync(workspace,{recursive:true});
const env={...process.env,HOME:home,USERPROFILE:home,APPDATA:join(home,'AppData/Roaming'),LOCALAPPDATA:join(home,'AppData/Local'),XDG_CONFIG_HOME:join(home,'xdg-config'),XDG_DATA_HOME:join(home,'xdg-data'),XDG_CACHE_HOME:join(home,'xdg-cache'),OPENCODE_DISABLE_AUTOUPDATE:'1'};
const results=[];
for(const size of [10000,50000]){
const sid='ses_winb7focus'+size+'20261027',stamp=Date.now()-100000;
const info={id:sid,slug:'b7-windows-long-'+size,projectID:'global',directory:workspace,title:'B7 Windows 1000 messages '+size,version:'1.18.21',time:{created:stamp,updated:stamp+999}};
const messages=Array.from({length:1000},(_,i)=>{const id='msg_winb7_'+size+'_'+String(i).padStart(5,'0');const common:any={id,sessionID:sid,role:i%2?'assistant':'user',time:{created:stamp+i},agent:'build'};let text;if(i%2){Object.assign(common,{parentID:'msg_winb7_'+size+'_'+String(i-1).padStart(5,'0'),providerID:'yume',modelID:'b7-local',mode:'build',path:{cwd:workspace,root:workspace},cost:0,tokens:{input:0,output:0,reasoning:0,cache:{read:0,write:0}},finish:'stop'});common.time.completed=stamp+i;text='### Synthetic reply '+i+'\n\n**Scope:** isolated QA data.\n\n- Check state\n- Keep evidence\n\n```ts\nconst round = '+i+';\n```\n\n| Item | Status |\n| --- | --- |\n| B7 | Complete |';}else{common.model={providerID:'yume',modelID:'b7-local'};text='Synthetic question '+i;}return {info:common,parts:[{id:'prt_winb7_'+size+'_'+i,messageID:id,sessionID:sid,type:'text',text}]};});
const file=join(repo,'output/b7-cold-history/long-'+size+'.json');await Bun.write(file,JSON.stringify({info,messages}));const start=performance.now();const p=Bun.spawn([join(repo,'src-tauri/resources/opencode/opencode.exe'),'import',file],{cwd:workspace,env,stdout:'pipe',stderr:'pipe'});const [out,err,code]=await Promise.all([new Response(p.stdout).text(),new Response(p.stderr).text(),p.exited]);await Bun.write(join(report,'complete-long-'+size+'-import.log'),out+'\n'+err);results.push({sessionId:sid,count:1000,exitCode:code,ms:performance.now()-start,fixtureSha256:new Bun.CryptoHasher('sha256').update(await Bun.file(file).arrayBuffer()).digest('hex')});if(code!==0)throw Error('import failed');}
await Bun.write(join(report,'complete-long-import.json'),JSON.stringify(results,null,2));console.log(JSON.stringify(results));






