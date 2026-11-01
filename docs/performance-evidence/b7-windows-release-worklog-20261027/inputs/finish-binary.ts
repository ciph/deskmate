import {readFileSync,existsSync} from 'node:fs';import {connect,save} from './cdp-client';
const c=await connect('chat');try{
 const playback=await c.evaluate(`(async()=>{const m=window.b7Binary.state.media;const a=new Audio();a.src=m.previewUrl;a.preload='auto';const wait=(event,ms)=>new Promise((resolve,reject)=>{const t=setTimeout(()=>reject(Error(event+' timeout')),ms);a.addEventListener(event,()=>{clearTimeout(t);resolve(true)},{once:true});a.addEventListener('error',()=>reject(Error('media error '+a.error.code)),{once:true})});await wait('loadedmetadata',5000);const duration=a.duration;await a.play();await new Promise(r=>setTimeout(r,250));const advanced=a.currentTime>0;const seeked=wait('seeked',5000);a.currentTime=duration-0.3;await seeked;await wait('ended',5000);const result={probe:'programmatic WebView2 decoder, not physical UI',duration,advanced,ended:a.ended,position:a.currentTime,error:a.error?.code??null};a.pause();a.removeAttribute('src');a.load();await window.b7Binary.discard();return result;})()`);
 save('media-64MiB-playback.json',playback);if(!playback.advanced||!playback.ended)throw Error('Playback failed');
 const old=JSON.parse(readFileSync('E:/Codex/工作伙伴/reports/yume-b7-release-worklog-20261027/resource-readback.json','utf8'));const p=old.resources.find((r:any)=>r.owned===1).path;
 save('media-64MiB-cleanup.json',{copiedPath:p,removed:!existsSync(p)});if(existsSync(p))throw Error('Owned media copy not removed');
 console.log(JSON.stringify(playback));
}finally{c.close()}
const settings=await connect('settings');try{
 const value=await settings.evaluate(`window.__TAURI__.core.invoke('stage_chat_resource_upload_binary',new Uint8Array([65]).buffer,{headers:{'x-yume-upload':btoa(JSON.stringify({uploadId:'b7-forbidden',scopeKey:'b7',fileName:'B7.wav',mime:'audio/wav',size:1}))}}).then(()=>({forbidden:false}),e=>({forbidden:true,error:String(e)}))`);
 save('binary-settings-denied.json',value);if(!value.forbidden)throw Error('Non-chat upload allowed');console.log(JSON.stringify(value));
}finally{settings.close()}

