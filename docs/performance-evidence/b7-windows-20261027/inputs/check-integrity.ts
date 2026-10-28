import { Database } from 'bun:sqlite';import {join,resolve} from 'node:path';
const report='E:/Codex/工作伙伴/reports/yume-b7-windows-20261027';const receipt=await Bun.file(join(report,'native-processes.json')).json();
if(receipt.identity!=='com.deskmate.unifiedhistoryqa.winb720261027a')throw Error('identity mismatch');
const files=['chat-resources.sqlite3','deskmate-memory.db','history-catalog.sqlite','yume-worklog.db','opencode-home/xdg-data/opencode/opencode.db'];const rows=[];
for(const relative of files){const path=resolve(receipt.roots[0],relative);if(!path.startsWith(resolve(receipt.roots[0])+'\\'))throw Error('path outside');const db=new Database(path,{readonly:true,create:false});try{const result=db.query('PRAGMA integrity_check').all();const values=result.map(x=>Object.values(x)[0]);rows.push({file:relative,integrity:values});if(values.length!==1||values[0]!=='ok')throw Error('integrity '+relative)}finally{db.close()}}
await Bun.write(join(report,'sqlite-integrity.json'),JSON.stringify(rows,null,2));console.log(JSON.stringify(rows));
