(async () => {
  const invoke = (command, args, options) => window.__TAURI_INTERNALS__.invoke(command, args, options);
  const results = [];
  for (const size of [1048576, 8388608, 20971520, 20971521, 67108864]) {
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
        row.ranges=[];
        for(const range of ['bytes=0-1023',`bytes=${size-1024}-${size-1}`]) {
          const r=await fetch(value.previewUrl,{headers:{Range:range}});
          const b=new Uint8Array(await r.arrayBuffer());
          row.ranges.push({requested:range,status:r.status,contentRange:r.headers.get('content-range'),bytes:b.length,firstFour:Array.from(b.slice(0,4))});
        }
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
  console.log('B7_BINARY '+JSON.stringify(results));
})().catch(error=>console.log('B7_BINARY_ERROR '+error));
