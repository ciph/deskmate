import {Database} from 'bun:sqlite';import {readFileSync,readdirSync,statSync,writeFileSync} from 'node:fs';import {join} from 'node:path';
import {save} from './cdp-client';
const root=join(process.env.APPDATA!,'com.deskmate.unifiedhistoryqa.winb7release20261027a');
const db=new Database(join(root,'chat-resources.sqlite3'),{readonly:true});
const resources=db.query('SELECT * FROM resources').all() as any[];const messages=db.query('SELECT * FROM resource_messages').all();
const hashes=resources.map(r=>({...r,exists:!!statSync(r.path),sha256:statSync(r.path).isFile()?new Bun.CryptoHasher('sha256').update(readFileSync(r.path)).digest('hex'):null}));
save('resource-readback.json',{resources:hashes,messages,integrity:db.query('PRAGMA integrity_check').all()});db.close();
const b=JSON.parse(readFileSync('E:/Codex/工作伙伴/reports/yume-b7-release-worklog-20261027/binary-results.json','utf8'));
if(hashes.find(r=>r.id===b.media.id)?.sha256!==b.media.sha256)throw Error('64MiB owned copy hash mismatch');
const fs=(p:string):any[]=>readdirSync(p).flatMap(n=>{const q=join(p,n);return statSync(q).isDirectory()?fs(q):[{path:q,size:statSync(q).size,sha256:new Bun.CryptoHasher('sha256').update(readFileSync(q)).digest('hex')}]});
save('fixtures-manifest.json',fs('output/b7-release-worklog/fixtures'));
console.log(JSON.stringify({resourceCount:resources.length,messageLinks:messages.length,media64MiBHashMatch:true,unownedReferences:resources.filter(r=>r.owned===0).length}));

