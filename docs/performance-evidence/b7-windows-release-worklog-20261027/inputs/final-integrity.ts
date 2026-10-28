import {Database} from 'bun:sqlite';import {readFileSync,readdirSync} from 'node:fs';import {join,basename} from 'node:path';import {save,report} from './cdp-client';
const receipt=JSON.parse(readFileSync(join(report,'native-processes-complete.json'),'utf8').replace(/^\uFEFF/,''));
if(receipt.identity!=='com.deskmate.unifiedhistoryqa.winb7release20261027a')throw Error('Unexpected QA identity');
for(const p of receipt.processes){let alive=false;try{process.kill(p.pid,0);alive=true}catch(e:any){if(e.code!=='ESRCH')throw e}if(alive)throw Error('Stop QA before integrity readback');}
const rows:any[]=[];
function scan(root:string){for(const d of readdirSync(root,{withFileTypes:true})){if(d.isSymbolicLink())throw Error('Unexpected link');const path=join(root,d.name);if(d.isDirectory())scan(path);else if(/\.(?:sqlite3?|db)$/.test(d.name)&&readFileSync(path).subarray(0,16).toString()==='SQLite format 3\0'){const db=new Database(path,{readonly:true,strict:true});try{const checks=db.query('PRAGMA integrity_check').all();rows.push({path,checks});if(checks.some((x:any)=>x.integrity_check!=='ok'))throw Error('Integrity check failed '+path)}finally{db.close()}}}}
for(const root of receipt.roots){if(basename(root)!==receipt.identity)throw Error('Wrong root');scan(root)}
const resources=new Database(join(receipt.roots[0],'chat-resources.sqlite3'),{readonly:true,strict:true});const resourceRows=resources.query('SELECT * FROM resources').all();const links=resources.query('SELECT * FROM resource_messages').all();resources.close();
save('final-integrity.json',{databases:rows,resourceRows,resourceMessages:links,pass:true,time:new Date().toISOString()});console.log(JSON.stringify({databases:rows.length,resources:resourceRows.length,links:links.length,pass:true}));
