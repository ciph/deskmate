import {Database} from 'bun:sqlite';import {readFileSync} from 'node:fs';import {join} from 'node:path';import {connect,save,report} from './cdp-client';
const stage=process.argv[2];if(!['failed','recovered','restart'].includes(stage))throw Error('Unknown stage');
const receipt=JSON.parse(readFileSync(join(report,'native-processes-complete.json'),'utf8').replace(/^\uFEFF/,''));
if(receipt.identity!=='com.deskmate.unifiedhistoryqa.winb7attachments20261027a')throw Error('Wrong identity');
let c:Awaited<ReturnType<typeof connect>>|undefined;let startupError:string|undefined;
try{c=await connect('chat')}catch(e){if(stage!=='failed')throw e;startupError=String(e)}
try{
 let catalog:any;try{if(!c)throw Error('Native setup aborted before IPC: '+startupError);catalog={ok:true,value:await c.invoke('history_catalog_list',{query:{limit:50},refresh:false})}}catch(e){catalog={ok:false,error:String(e)}}
 const memory:any=c?await c.invoke('memory_export'):null;
 const h=new Database(join(receipt.roots[0],'history-catalog.sqlite'),{readonly:true,strict:true});
 const m=new Database(join(receipt.roots[0],'deskmate-memory.db'),{readonly:true,strict:true});
 const history={version:h.query('PRAGMA user_version').get(),integrity:h.query('PRAGMA integrity_check').all(),rows:h.query('SELECT key,metadata FROM entries ORDER BY key').all(),projection:h.query("SELECT name FROM sqlite_master WHERE name='entry_projection'").all()};
 const memoryDb={version:m.query('PRAGMA user_version').get(),integrity:m.query('PRAGMA integrity_check').all(),rows:m.query('SELECT id,content FROM memories ORDER BY id').all()};h.close();m.close();
 const result={stage,method:'actual native IPC plus readonly SQLite',catalog,memory,history,memoryDb,time:new Date().toISOString()};save('migration-native-'+stage+'.json',result);
 if(stage!=='failed'&&((memoryDb.version as any).user_version!==4||!JSON.stringify(memory).includes('B7 原生迁移合成记录 保留中文内容')))throw Error('Native memory v3-to-v4 upgrade/export failed');
 if(stage==='failed'){if(catalog.ok||(history.version as any).user_version!==2||history.projection.length)throw Error('Expected native history migration failure with rollback');}
 else if(!catalog.ok||(history.version as any).user_version!==3||history.rows.some((x:any)=>x.key==='zz-b7-injected-invalid-metadata'))throw Error('Native repaired history migration failed');
 console.log(JSON.stringify({stage,catalogOk:catalog.ok,historyVersion:history.version,memoryVersion:memoryDb.version,pass:true}));
}finally{c?.close()}
