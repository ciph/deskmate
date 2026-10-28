import {stageChatAttachment,readChatAttachment,discardChatAttachment,tauriAttachmentHost} from '../../src/chat/attachmentApi';
import {stageMediaResource,discardChatResources} from '../../src/chat/localResources';
import {binaryUploadOptions} from '../../src/chat/binaryUpload';
import {invoke} from '@tauri-apps/api/core';
const state:any={rows:[],transport:[],media:null,started:new Date().toISOString()};
const hash=async(b:ArrayBuffer)=>Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',b))).map(x=>x.toString(16).padStart(2,'0')).join('');
const host={invoke:async(command:any,body:any,options:any)=>{state.transport.push({command,isArrayBuffer:body instanceof ArrayBuffer,size:body?.byteLength});return tauriAttachmentHost.invoke(command,body,options)}};
async function expectReject(name:string,fn:()=>Promise<any>,pattern:RegExp){try{const v=await fn();throw Error('Unexpected success: '+JSON.stringify(v))}catch(e){const error=String(e);if(!pattern.test(error))throw e;state.rows.push({name,pass:true,error})}}
async function run(){
 const M=1024*1024;
 for(const size of [M,8*M,20*M]){
  const bytes=new Uint8Array(size).fill(65),sessionId='b7-binary-'+size;
  const began=performance.now(),v=await stageChatAttachment({sessionId,fileName:'B7-'+size+'.txt',mime:'text/plain',size,bytes:bytes.buffer},host);
  const stagedMs=performance.now()-began;
  const read=await readChatAttachment({sessionId,attachmentId:v.id});
  const result=Uint8Array.from(atob(read.dataUrl.split(',')[1]),c=>c.charCodeAt(0));
  const expectedSha=await hash(bytes.buffer),actualSha=await hash(result.buffer);
  if(expectedSha!==actualSha)throw Error('Byte mismatch');
  await discardChatAttachment({sessionId,attachmentId:v.id});
  state.rows.push({name:'ordinary-'+size,size,pass:true,stagedMs,expectedSha,actualSha});
 }
 const meta=(size:number,extra={})=>({sessionId:'b7-boundaries',fileName:'B7-limit.txt',mime:'text/plain',size,uploadId:crypto.randomUUID(),scopeKey:'b7-boundaries',...extra});
 const raw=(b:ArrayBuffer,m:any,command='stage_chat_attachment_binary')=>invoke(command,b,binaryUploadOptions(m));
 await expectReject('ordinary-over-20MiB',()=>{const b=new Uint8Array(20*M+1);return raw(b.buffer,meta(b.length))},/exceeds 20 MiB/);
 await expectReject('empty',()=>raw(new ArrayBuffer(0),meta(0)),/must not be empty/);
 await expectReject('size-mismatch',()=>raw(new ArrayBuffer(8),meta(9)),/size does not match/);
 await expectReject('spoof-image',()=>raw(new Uint8Array(32).fill(65).buffer,meta(32,{fileName:'spoof.png',mime:'image/png'})),/signature does not match/);
 await expectReject('json-array-body',()=>invoke('stage_chat_attachment_binary',{bytes:[65]},binaryUploadOptions(meta(1))),/binary_body_required/);
 const q=await stageChatAttachment({sessionId:'b7-quota',fileName:'quota.txt',mime:'text/plain',size:20*M,bytes:new Uint8Array(20*M).fill(65).buffer});
 await expectReject('aggregate-over-20MiB',()=>stageChatAttachment({sessionId:'b7-quota',fileName:'one.txt',mime:'text/plain',size:1,bytes:new Uint8Array([65]).buffer}),/20 MiB/);
 await discardChatAttachment({sessionId:'b7-quota',attachmentId:q.id});
 const mediaBytes=new Uint8Array(64*M),d=new DataView(mediaBytes.buffer),str=(o:number,s:string)=>{for(let i=0;i<s.length;i++)mediaBytes[o+i]=s.charCodeAt(i)};
 str(0,'RIFF');d.setUint32(4,mediaBytes.length-8,true);str(8,'WAVEfmt ');d.setUint32(16,16,true);d.setUint16(20,1,true);d.setUint16(22,1,true);d.setUint32(24,16000,true);d.setUint32(28,32000,true);d.setUint16(32,2,true);d.setUint16(34,16,true);str(36,'data');d.setUint32(40,mediaBytes.length-44,true);
 const began=performance.now();const v=await stageMediaResource(new File([mediaBytes.buffer],'B7-64MiB.wav',{type:'audio/wav'}),{uploadId:'b7-media-limit',scopeKey:'b7-media-scope'});
 state.media={...v,stagedMs:performance.now()-began,sha256:await hash(mediaBytes.buffer)};
 await expectReject('media-over-64MiB-host',()=>raw(new ArrayBuffer(64*M+1),meta(64*M+1,{sessionId:undefined,fileName:'large.wav',mime:'audio/wav'}),'stage_chat_resource_upload_binary'),/size_limit/);
 await expectReject('media-mime-mismatch',()=>raw(new ArrayBuffer(16),meta(16,{sessionId:undefined,fileName:'bad.mp4',mime:'audio/wav'}),'stage_chat_resource_upload_binary'),/unsupported/);
 state.finished=new Date().toISOString();state.pass=true;return state;
}
(window as any).b7Binary={run,state,discard:async()=>{if(state.media)await discardChatResources([state.media.id]);return true;}};
