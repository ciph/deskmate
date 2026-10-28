import {cpSync,mkdirSync,readFileSync,writeFileSync,existsSync} from 'node:fs';
import {join} from 'node:path';
const repo=process.cwd(), report='E:/Codex/工作伙伴/reports/yume-b7-cold-history-20261027',out=join(report,'doctor-baseline');
if(existsSync(out))throw Error('baseline already exists');
mkdirSync(out,{recursive:true});
cpSync(join(repo,'src'),join(out,'src'),{recursive:true});
cpSync(join(repo,'scripts'),join(out,'scripts'),{recursive:true});
for(const file of ['package.json','tsconfig.json','tsconfig.test.json','tsconfig.runner.json','vite.config.ts'])if(existsSync(file))cpSync(file,join(out,file));
for(const file of ['ChatApp.tsx','sharedConversation.ts','chatFixedReply.test.tsx','sharedConversation.test.ts'])writeFileSync(join(out,'src/chat',file),readFileSync(join(report,'before',file)));
for(const file of ['src/chat/HistoryOrganizer.tsx','src/chat/HistoryOrganizer.test.tsx','src/lib/unifiedHistory.ts']){const p=Bun.spawnSync(['git','show','HEAD:'+file],{cwd:repo});if(p.exitCode!==0)throw Error(file);writeFileSync(join(out,file),p.stdout)}
console.log(out);
