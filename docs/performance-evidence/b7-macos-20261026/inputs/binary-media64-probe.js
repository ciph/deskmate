setTimeout(()=>{void (async () => {
  const invoke = (command, args, options) => window.__TAURI_INTERNALS__.invoke(command, args, options);
  const results = [];
  for (const size of [67108864]) {
    const media = size === 67108864;
    const metadata = { uploadId: crypto.randomUUID(), scopeKey: 'b7-binary-probe', fileName: media ? 'b7-silence.wav' : 'b7-binary.txt', mime: media ? 'audio/wav' : 'text/plain', size, ...(media ? {} : {sessionId:'b7-binary-probe'}) };
    const bytes = new Uint8Array(size);
    if (media) {
      const view = new DataView(bytes.buffer);
      for (const [at, text] of [[0,'RIFF'],[8,'WAVE'],[12,'fmt '],[36,'data']]) bytes.set(new TextEncoder().encode(text),at);
      view.setUint32(4,size-8,true);view.setUint32(16,16,true);view.setUint16(20,1,true);view.setUint16(22,1,true);view.setUint32(24,16000,true);view.setUint32(28,32000,true);view.setUint16(32,2,true);view.setUint16(34,16,true);view.setUint32(40,size-44,true);
    } else bytes.fill(97);
    const start = performance.now();
    let value;
    const row = {size,media};
    try {
      const response = await invoke(media?'stage_chat_resource_upload_binary':'stage_chat_attachment_binary',bytes.buffer,{headers:{'x-yume-upload':btoa(JSON.stringify(metadata))}});
      value=response.value;
      row.identityMatched=response.uploadId===metadata.uploadId&&response.scopeKey===metadata.scopeKey;
      row.hostSize=value.size;row.accepted=true;row.uploadMs=performance.now()-start;
      if(media) {
        const audio=new Audio(value.previewUrl);audio.muted=true;audio.preload='metadata';document.body.append(audio);
        const event=(name)=>new Promise((resolve,reject)=>{const timer=setTimeout(()=>reject(new Error(name+' timeout')),15000);audio.addEventListener(name,()=>{clearTimeout(timer);resolve();},{once:true});audio.addEventListener('error',()=>{clearTimeout(timer);reject(new Error('media error '+audio.error?.code));},{once:true});});
        try {await event('loadedmetadata');row.duration=audio.duration;await audio.play();await new Promise(r=>setTimeout(r,1100));audio.pause();row.advanced=audio.currentTime;const seek=event('seeked');audio.currentTime=audio.duration-1;await seek;row.seekedTo=audio.currentTime;row.errorCode=audio.error?.code??null;row.mediaPassed=row.advanced>0&&row.seekedTo>audio.duration-2&&row.errorCode===null;}
        finally {audio.pause();audio.removeAttribute('src');audio.load();audio.remove();}
      }
    } catch(error) {row.accepted=false;row.error=String(error);}
    finally {
      if(value) {
        await invoke(media?'discard_chat_resources':'discard_chat_attachment',media?{resourceIds:[value.id]}:{request:{sessionId:'b7-binary-probe',attachmentId:value.id}});
        row.discarded=true;
      }
    }
    results.push(row);
  }
  const result=document.createElement('pre');result.id='b7-binary-result';result.textContent='B7_MEDIA64 '+JSON.stringify(results);document.body.append(result);
  localStorage.setItem('b7-media64-result',JSON.stringify(results));
})().catch(error=>{const out=document.createElement('pre');out.textContent='B7_BINARY_ERROR '+error;document.body.append(out);});},8000);void 0;
