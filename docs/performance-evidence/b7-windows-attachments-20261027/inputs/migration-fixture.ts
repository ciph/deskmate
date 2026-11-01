import {Database} from 'bun:sqlite';
import {readFileSync,writeFileSync,copyFileSync,lstatSync,realpathSync,existsSync,unlinkSync} from 'node:fs';
import {join,basename,resolve} from 'node:path';
import {save,report} from './cdp-client';
const receipt=JSON.parse(readFileSync(join(report,'native-processes-complete.json'),'utf8').replace(/^\uFEFF/,''));
const identity='com.deskmate.unifiedhistoryqa.winb7attachments20261027a';
const root=receipt.roots[0];
if(receipt.identity!==identity||basename(root)!==identity||lstatSync(root).isSymbolicLink()||realpathSync(root).toLowerCase()!==resolve(root).toLowerCase())throw Error('Invalid isolated QA root');
for(const p of receipt.processes){let alive=false;try{process.kill(p.pid,0);alive=true}catch(e:any){if(e.code!=='ESRCH')throw e}if(alive)throw Error('Stop recorded QA process before fixture writes');}
const hp=join(root,'history-catalog.sqlite');const mp=join(root,'deskmate-memory.db');
const action=process.argv[2];
const h=new Database(hp,{strict:true});
try{
 if(action==='seed'){
  if(existsSync(join(report,'migration-fixture.json')))throw Error('Refuse repeated fixture seed');
  h.exec('PRAGMA wal_checkpoint(TRUNCATE)');
  const rows=h.query('SELECT key,metadata FROM entries ORDER BY key').all();
  copyFileSync(hp,join(report,'history-before-fixture.sqlite'));
  const m=new Database(mp,{strict:true});
  if((m.query('SELECT count(*) AS n FROM memories').get() as any).n!==0)throw Error('Memory fixture requires empty QA memory store');
  m.exec('PRAGMA wal_checkpoint(TRUNCATE)');m.close();copyFileSync(mp,join(report,'memory-before-fixture.sqlite'));
  // This root is task-created and stopped; replace only the empty synthetic memory database.
  for(const suffix of ['', '-wal','-shm'])if(existsSync(mp+suffix))unlinkSync(mp+suffix);
  const old=new Database(mp,{strict:true});
  try{
   for(const file of ['001_initial.sql','002_automatic.sql','003_active_expiry.sql'])old.exec(readFileSync('src-tauri/src/memory/migrations/'+file,'utf8'));
   old.exec('PRAGMA user_version=3');
   old.query("INSERT INTO memories(id,scope,type,content,status,confidence,importance,sensitivity,source_kind,valid_from,created_at,updated_at,revision) VALUES(?, 'global','preference',?,'active',1,3,'normal','explicit',?,?,?,1)").run('b7-native-migration','B7 原生迁移合成记录 保留中文内容','2026-10-27T00:00:00Z','2026-10-27T00:00:00Z','2026-10-27T00:00:00Z');
  }finally{old.close()}
  h.transaction(()=>{
   h.exec('DROP TRIGGER catalog_insert_dirty;DROP TRIGGER catalog_update_dirty;DROP TRIGGER catalog_delete_projection;DROP TABLE entry_projection;DROP TABLE catalog_projection_dirty;PRAGMA user_version=2;');
   h.query('INSERT INTO entries(key,metadata) VALUES(?,?)').run('zz-b7-injected-invalid-metadata','{');
  })();
  save('migration-fixture.json',{fixtureOnly:true,stoppedApp:true,historyVersion:2,memoryVersion:3,historyRows:rows,invalidKey:'zz-b7-injected-invalid-metadata',failure:'invalid synthetic history metadata causes transactional v2-to-v3 backfill rollback',memoryMarker:'b7-native-migration',time:new Date().toISOString()});
  console.log('Seeded stopped QA history v2 + invalid metadata, memory v3; authoritative history rows retained.');
 }else if(action==='repair'){
  const before={version:h.query('PRAGMA user_version').get(),projection:h.query("SELECT name FROM sqlite_master WHERE name='entry_projection'").all(),rows:h.query('SELECT key,metadata FROM entries ORDER BY key').all()};
  if((before.version as any).user_version!==2||before.projection.length!==0)throw Error('Migration did not roll back');
  const seed=JSON.parse(readFileSync(join(report,'migration-fixture.json'),'utf8'));
  const retained=before.rows.filter((x:any)=>x.key!==seed.invalidKey);
  if(JSON.stringify(retained)!==JSON.stringify(seed.historyRows))throw Error('Authoritative rows changed during failed migration');
  if(h.query('DELETE FROM entries WHERE key=? AND metadata=?').run(seed.invalidKey,'{').changes!==1)throw Error('Injected row missing');
  save('migration-rollback-and-repair.json',{before,authoritativeRowsUnchanged:true,removedOnlyInjectedInvalidRow:true,time:new Date().toISOString()});
  console.log('Verified rollback retained schema v2 and all authoritative rows; removed only injected invalid row.');
 }else throw Error('Unknown fixture action');
}finally{h.close()}
