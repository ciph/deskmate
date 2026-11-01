import {Database} from 'bun:sqlite';
import {join,resolve} from 'node:path';
import {existsSync,readFileSync} from 'node:fs';
const report='E:/Codex/工作伙伴/reports/yume-b7-stop-focus-20261027';
const files=['chat-resources.sqlite3','deskmate-memory.db','history-catalog.sqlite','yume-worklog.db','opencode-home/xdg-data/opencode/opencode.db'];
const results=[];
for(const [receiptName,suffix] of [['native-processes.json','b'],['native-processes-fixed.json','c']]){const receipt=await Bun.file(join(report,receiptName)).json();if(receipt.identity!=='com.deskmate.unifiedhistoryqa.winb720261027'+suffix)throw Error('identity');const root=resolve(receipt.roots[0]);const rows=[];for(const relative of files){const file=resolve(root,relative);if(!file.startsWith(root+'\\'))throw Error('scope');const db=new Database(file,{readonly:true,create:false});try{const values=db.query('PRAGMA integrity_check').all().map(x=>Object.values(x)[0]);const schemaVersion=Object.values(db.query('PRAGMA user_version').get())[0];rows.push({file:relative,integrity:values,schemaVersion});if(values.length!==1||values[0]!=='ok')throw Error('integrity '+relative)}finally{db.close()}}
const erasure=suffix==='c'?['deskmate-memory.db','deskmate-memory.db-wal','deskmate-memory.db-shm'].map(relative=>{const file=join(root,relative),exists=existsSync(file);const bytes=exists?readFileSync(file):Buffer.alloc(0);return {file:relative,exists,bytes:bytes.length,forgottenSentinelPresent:bytes.includes(Buffer.from('B7_ERASE_SENTINEL_20261027_c')),retainedThenForgottenSentinelPresent:bytes.includes(Buffer.from('B7_KEEP_SENTINEL_20261027_c'))}}):undefined;
results.push({identity:receipt.identity,databases:rows,erasure});}
await Bun.write(join(report,'sqlite-integrity.json'),JSON.stringify(results,null,2));console.log(JSON.stringify(results));
