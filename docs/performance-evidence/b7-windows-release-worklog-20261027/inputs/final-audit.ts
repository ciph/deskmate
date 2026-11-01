import {readFileSync,existsSync} from 'node:fs';import {join} from 'node:path';
const root='docs/performance-evidence/b7-windows-release-worklog-20261027';
const read=(p:string)=>JSON.parse(readFileSync(p,'utf8').replace(/^\uFEFF/,''));
const hash=(p:string)=>new Bun.CryptoHasher('sha256').update(readFileSync(p)).digest('hex');
const manifest=read(join(root,'evidence-manifest.json'));
for(const f of manifest)if(hash(join(root,f.path))!==f.sha256)throw Error('Hash differs '+f.path);
for(const name of ['README.md','index.html']){const s=readFileSync(join(root,name),'utf8'),links=name.endsWith('.html')?[...s.matchAll(/(?:href|src)="([^"]+)"/g)].map(m=>m[1]):[...s.matchAll(/\]\(([^)]+)\)/g)].map(m=>m[1]);for(const link of links)if(!existsSync(join(root,link)))throw Error('Missing '+link);}
const normal=read(join(root,'raw/normal-release-build.json'));
if(hash('src-tauri/target/release/yume.exe')!==normal.binarySha256.toLowerCase())throw Error('Normal binary differs');
for(const f of read(join(root,'source-fingerprints.json')))if(hash(f.path)!==f.sha256)throw Error('Source differs '+f.path);
const proc=Bun.spawnSync(['git','-c','safe.directory='+process.cwd(),'rev-parse','HEAD']);
if(proc.exitCode||proc.stdout.toString().trim()!=='db07e5bdb02550c4d165da8cd45fb76debefbe17')throw Error('HEAD changed');
console.log(JSON.stringify({pass:true,manifestFiles:manifest.length,hashes:true,allLinksIncludingAudit:true,sourceFingerprints:true,normalBinary:true,head:proc.stdout.toString().trim()}));
