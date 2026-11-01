import {readFileSync,writeFileSync,existsSync} from 'node:fs';
import {join} from 'node:path';
const raw='E:/Codex/工作伙伴/reports/yume-b7-release-worklog-20261027';
const read=(n:string)=>JSON.parse(readFileSync(join(raw,n),'utf8').replace(/^\uFEFF/,''));
const assert=(ok:unknown,message:string)=>{if(!ok)throw Error(message)};
const build=read('complete-qa-build.json'),probe=read('probe-removed-playing.json');
assert(build.exitCode===0&&build.profile==='release'&&build.panic==='unwind','Release build missing');
const events=probe.events;
const ended=events.find((e:any)=>e.type==='ended'&&e.trusted&&e.position===30);
const firstPlay=events.find((e:any)=>e.type==='play'&&e.trusted);
const resumed=events.filter((e:any)=>e.type==='play'&&e.trusted).at(-1);
const removed=events.filter((e:any)=>e.type==='pause'&&e.trusted).at(-1);
assert(ended&&ended.time-firstPlay.time>=29000,'Complete playback missing');
assert(events.some((e:any)=>e.type==='seeked'&&e.trusted&&e.position>0&&e.position<30),'Seek missing');
assert(resumed&&removed&&removed.time>resumed.time&&removed.position<30&&probe.media.length===0,'Playing removal missing');
const binary=read('binary-results.json'),decoder=read('media-64MiB-playback.json'),cleanup=read('media-64MiB-cleanup.json'),denied=read('binary-settings-denied.json');
assert(binary.pass&&binary.rows.every((r:any)=>r.pass),'Binary checks failed');
assert(decoder.advanced&&decoder.ended&&!decoder.error&&cleanup.removed&&!existsSync(cleanup.copiedPath)&&denied.forbidden,'64 MiB lifecycle failed');
const result={time:new Date().toISOString(),buildSha256:build.binarySha256,pass:true,
 physical:{MP4BodyDrop:true,completePlaybackMs:ended.time-firstPlay.time,seek:'one continuous physical polyline; two seeking events and one seeked event, not six independent seeks',resumedAt:resumed.position,removedAt:removed.position,removedNode:true},
 binary:{checks:binary.rows.length,ordinaryMiB:[1,8,20],allSHAReadbacksMatch:true,mediaMiB:64,decoder:'programmatic WebView2 decoder; not physical UI',ownedCopyRemoved:true,nonChatDenied:true},
 pending:['release picker','composer/body MP3 WAV folder invalid-media matrix','session switch/readd/send/history','all complete worklog physical surfaces'],
 interruption:'Windows MCP get_foreground returned SecureDesktopActive at the next picker action; no secure-desktop input attempted',
 scope:'Partial release checkpoint, not B7 completion; all product changes from previous rounds preserved'};
writeFileSync(join(raw,'release-checkpoint.json'),JSON.stringify(result,null,2)+'\n');
console.log(JSON.stringify(result));
