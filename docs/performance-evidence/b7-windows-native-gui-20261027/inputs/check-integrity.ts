import {Database} from 'bun:sqlite';
import {join,resolve} from 'node:path';
import {copyFileSync,existsSync} from 'node:fs';
const report='E:/Codex/工作伙伴/reports/yume-b7-native-gui-20261027';
const files=['chat-resources.sqlite3','deskmate-memory.db','history-catalog.sqlite','yume-worklog.db','opencode-home/xdg-data/opencode/opencode.db'];
const pairs=[['complete','a']];
const results=[];
for(const [name,suffix] of pairs){
 const receipt=await Bun.file(join(report,'native-processes-'+name+'.json')).json();
 if(receipt.identity!=='com.deskmate.unifiedhistoryqa.winb7gui20261027'+suffix)throw Error('identity');
 const root=resolve(receipt.roots[0]);const rows=[];
 for(const relative of files){const file=resolve(root,relative);if(!file.startsWith(root+'\\'))throw Error('scope');const db=new Database(file,{readonly:true,create:false});try{const values=db.query('PRAGMA integrity_check').all().map(x=>Object.values(x)[0]);const schemaVersion=Object.values(db.query('PRAGMA user_version').get()!)[0];rows.push({file:relative,integrity:values,schemaVersion});if(values.length!==1||values[0]!=='ok')throw Error('integrity '+relative)}finally{db.close()}}
 const log=join(root,'sidecar.log');if(existsSync(log))copyFileSync(log,join(report,'sidecar-'+suffix+'.log'));
 results.push({identity:receipt.identity,databases:rows});
}
await Bun.write(join(report,'sqlite-integrity.json'),JSON.stringify(results,null,2));console.log(JSON.stringify({ok:true,identities:results.length,databases:results.reduce((n,x)=>n+x.databases.length,0)}));

