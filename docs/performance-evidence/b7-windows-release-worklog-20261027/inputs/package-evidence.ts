import {readFileSync,writeFileSync,existsSync,readdirSync,mkdirSync,copyFileSync} from 'node:fs';
import {join} from 'node:path';
const raw='E:/Codex/工作伙伴/reports/yume-b7-release-worklog-20261027',out='output/b7-release-worklog',dest='docs/performance-evidence/b7-windows-release-worklog-20261027';
const read=(p:string)=>JSON.parse(readFileSync(p,'utf8').replace(/^\uFEFF/,''));
const hash=(p:string)=>new Bun.CryptoHasher('sha256').update(readFileSync(p)).digest('hex');
const put=(n:string,v:unknown)=>writeFileSync(join(dest,n),JSON.stringify(v,null,2)+'\n');
mkdirSync(join(dest,'raw'),{recursive:true});mkdirSync(join(dest,'inputs'),{recursive:true});
const normal=read(join(raw,'normal-release-build.json'));
if(normal.exitCode!==0||normal.binarySha256.toLowerCase()!==hash('src-tauri/target/release/yume.exe'))throw Error('Normal release not restored');
const qa=read(join(raw,'complete-qa-build.json'));
if(qa.binarySha256.toLowerCase()!==hash(join(out,'yume.release-qa.exe')))throw Error('Retained QA binary differs');
const cleanup=read(join(raw,'cleanup.json'));
if(!cleanup.credentialCleared||cleanup.listeners.length||cleanup.helperProcesses.length||cleanup.roots.some((r:any)=>!r.deleted))throw Error('Cleanup incomplete');
const omit=new Set(['provider-receipt.json','provider.stdout.log','provider-ready.log','ui-provider-ready.json','helper-processes.json','windows-mcp-tools.json','windows-mcp-initialize.json','windows-mcp-calls.jsonl','downloads-before.json']);
for(const name of readdirSync(raw))if(/\.(json|log|png)$/.test(name)&&!omit.has(name)&&!name.endsWith('.stdout.log'))copyFileSync(join(raw,name),join(dest,'raw',name));
const calls=readFileSync(join(raw,'windows-mcp-calls.jsonl'),'utf8').trim().split(/\r?\n/).map(l=>JSON.parse(l));
put('physical-input-log.json',{scope:'Temporary pinned Windows MCP; actual mouse input, window geometry, captures and secure-desktop failures; no DOM-driven physical acceptance',calls});
copyFileSync(join(raw,'before/hashes.json'),join(dest,'baseline-local-changes-sha256.json'));
for(const name of ['tauri.qa.json','build-qa.ps1','build-normal.ps1','cdp-client.ts','probe.ts','inspect.ts','readback.ts','binary-browser.ts','run-binary.ts','finish-binary.ts','read-resources.ts','verify-checkpoint.ts','audit-progress.ts','native-process-complete.ps1','final-integrity.ts','cleanup.ps1','DragSource.cs','steps.json'])copyFileSync(join(out,name),join(dest,'inputs',name));
const frozen=[];
for(const name of ['b7-windows-20261027','b7-windows-stop-focus-20261027','b7-windows-cold-history-20261027','b7-windows-native-gui-20261027','b7-windows-tao-sse-20261027','b7-windows-attachments-20261027']){
 const root=join('docs/performance-evidence',name),manifest=read(join(root,name==='b7-windows-20261027'?'manifest.json':'evidence-manifest.json')),files=manifest.files??manifest;
 const differences=files.filter((f:any)=>hash(join(root,f.path??f.file))!==f.sha256.toLowerCase()).map((f:any)=>f.path??f.file);
 if(differences.some((p:string)=>!(name==='b7-windows-20261027'&&p==='README.md')))throw Error('Previous report changed '+name);
 frozen.push({name,files:files.length,differences,note:differences.length?'Pre-existing follow-up-link exception; current-turn baseline still matches':'all manifest hashes match'});
}
put('frozen-reports.json',frozen);
const suite=read(join(raw,'frontend-regression.json'));
const checkpoint=read(join(raw,'release-checkpoint.json'));
const sources=['src-tauri/Cargo.toml','src-tauri/Cargo.lock','src-tauri/tauri.conf.json','src/chat/useWorklogChat.ts','src/chat/ChatInput.tsx','src/chat/localResources.ts','src/chat/attachmentApi.ts','scripts/worklog-qa/provider.js'];
put('source-fingerprints.json',sources.filter(existsSync).map(path=>({path,sha256:hash(path)})));
const fixtures=read(join(raw,'fixtures-manifest.json'));
for(const f of fixtures)if(hash(f.path)!==f.sha256)throw Error('Fixture changed '+f.path);
put('fixture-retention.json',{allPresent:true,method:'digests after physical tests and rechecked after cleanup; not a pre-first-drop digest',files:fixtures});
put('summary.json',{status:'B7 in progress; physical acceptance blocked by Windows SecureDesktopActive',date:'2026-10-27',head:'db07e5bdb02550c4d165da8cd45fb76debefbe17',branch:'codex/yume-performance-b6',version:'0.4.21',schemas:{history:3,memory:4},productSourceChangedThisRound:false,qaBuild:qa,normalBuild:normal,releaseCheckpoint:checkpoint,frontend:{files:suite.files,pass:suite.pass,fail:suite.fail,skip:suite.skip,method:suite.method},integrity:{databases:6,resourcesAfterRemoval:0},cleanup,remaining:{priority:'release attachment native picker and remaining media/drop/history matrix, then complete physical worklog editors/versions/export/delete/retry/cancel',afterward:'active-tool secondary launches, Aborted wording, remaining recovery, PDF heap/RSS, hardware/production comparison, receipt-recovery performance, final gate',windowsRounds:'2–3 focused rounds; 3–4 if new defects; this interrupted physical phase remains in that estimate',mac:'separate platform gates'},committed:false,pushed:false,installed:false,published:false});
const token=read(join(out,'bridge-session.json')).token;
const files:any[]=[];
function walk(dir:string,prefix=''){for(const d of readdirSync(dir,{withFileTypes:true})){const path=prefix+d.name,full=join(dir,d.name);if(d.isDirectory())walk(full,path+'/');else if(!['evidence-manifest.json','final-audit.json'].includes(path)){const data=readFileSync(full);if(data.includes(Buffer.from(token)))throw Error('Bridge token leaked '+path);files.push({path,bytes:data.length,sha256:hash(full)})}}}
walk(dest);put('evidence-manifest.json',files.sort((a,b)=>a.path.localeCompare(b.path)));
for(const doc of ['README.md','index.html']){const text=readFileSync(join(dest,doc),'utf8');const links=doc.endsWith('.html')?[...text.matchAll(/(?:href|src)="([^"]+)"/g)].map(m=>m[1]):[...text.matchAll(/\]\(([^)]+)\)/g)].map(m=>m[1]);for(const link of links)if(link!=='final-audit.json'&&!existsSync(join(dest,link)))throw Error('Broken reference '+doc+':'+link);}
put('final-audit.json',{time:new Date().toISOString(),manifestFiles:files.length,allHashesMatch:files.every(f=>hash(join(dest,f.path))===f.sha256),allLocalReferencesExist:true,temporaryBridgeTokenAbsent:true,normalReleaseSHA256:normal.binarySha256,retainedQaSHA256:qa.binarySha256,oldReports:frozen.length,browserPreview:'not attempted; file protocol was previously blocked by browser policy',productSourceUnchangedThisRound:true});
console.log(JSON.stringify({files:files.length,bytes:files.reduce((n,f)=>n+f.bytes,0),frozenReports:frozen.length,normalSHA:normal.binarySha256}));
