(async()=>{const wait=ms=>new Promise(r=>setTimeout(r,ms));const started=performance.now(),trace=[];const log=(kind,value)=>trace.push({at:performance.now()-started,kind,...value});const original=window.__TAURI_INTERNALS__.invoke;const commands=new Set(['history_catalog_access','history_catalog_load','history_register_native_session','workbench_session_owned','workbench_session_status','history_chat_selected']);window.__TAURI_INTERNALS__.invoke=async function(cmd,args,...rest){try{const value=await original.call(this,cmd,args,...rest);if(commands.has(cmd)){const entry=value?.entry??value;log('ipc',{cmd,key:args?.key,sessionId:args?.sessionId,value:entry&&typeof entry==='object'?{key:entry.key,identity:entry.identity,runtime:entry.runtime,ownership:entry.ownership,capabilities:entry.capabilities,state:entry.state}:value})}return value}catch(e){if(commands.has(cmd))log('ipc-error',{cmd,error:String(e)});throw e}};
const stopEvent=await window.__TAURI__.event.listen('history-catalog-changed',()=>log('catalog-event',{}));let timer;let observedInput;let previous='';try{const history=[...document.querySelectorAll('button')].find(b=>b.getAttribute('aria-label')==='历史');if(history)history.click();else if(!document.querySelector('.history-organizer'))throw Error('history missing');await window.__TAURI__.core.invoke('history_catalog_list',{query:{limit:50},refresh:true});let selected=false;let selectedKey;for(let i=0;i<100;i++){const b=[...document.querySelectorAll('button.history-organizer-open')].find(b=>b.textContent.includes('B7 Windows 1000 messages 10000'));if(b){selectedKey=b.closest('[data-preview-key]').dataset.previewKey;b.click();selected=true;break}await wait(50)}if(!selected)throw Error('history row missing');for(let i=0;i<200&&document.querySelectorAll('[data-message-id]').length!==1000;i++)await wait(50);if(document.querySelectorAll('[data-message-id]').length!==1000)throw Error('expected 1000');observedInput=document.querySelector('textarea.chat-input');timer=setInterval(()=>{const value={composer:!!document.querySelector('textarea.chat-input'),sameInput:observedInput===document.querySelector('textarea.chat-input'),focus:document.activeElement===observedInput,stop:!!document.querySelector('button.chat-abort'),notices:[...document.querySelectorAll('.chat-memory-notice,.chat-composer-notice')].map(x=>x.textContent),tailChars:[...document.querySelectorAll('.chat-msg-assistant')].at(-1)?.innerText.length};const key=JSON.stringify({...value,tailChars:undefined});if(key!==previous){log('dom',value);previous=key}},25);const m=(()=>{const values={};const wait=ms=>new Promise(r=>setTimeout(r,ms));const frame=()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));const rows=()=>[...document.querySelectorAll('[data-message-id]')];const store=(name,value)=>values[name]=value;async function observeStream(name, stopAfterMs) {
  const oldRows = rows();
  const input = document.querySelector("textarea.chat-input");
  const list = document.querySelector(".chat-list");
  if (!input || !list)
    throw Error("Missing chat controls");
  const initial = input.value;
  const inputMs = [], scrollMs = [];
  const updates = [];
  let lastText = "";
  const started = performance.now();
  const oldIds = new Set(oldRows.map((x) => x.dataset.messageId));
  const observer = new MutationObserver(() => {
    const tail = rows().filter((x) => !oldIds.has(x.dataset.messageId) && x.classList.contains("chat-msg-assistant")).at(-1);
    const text = tail?.innerText || "";
    if (text !== lastText) {
      lastText = text;
      updates.push({ at: performance.now() - started, chars: text.length });
    }
  });
  observer.observe(list, { childList: true, subtree: true, characterData: true });
  const set = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value").set;
  let stopClicked = false;
  const timer = stopAfterMs === undefined ? undefined : setTimeout(() => {
    const b = document.querySelector("button.chat-abort");
    if (b) {
      b.click();
      stopClicked = true;
    }
  }, stopAfterMs);
  try {
    while (!lastText && performance.now() - started < 20000)
      await wait(20);
    const firstAt = performance.now() - started;
    input.focus();
    const sampleWindows = [];
    for (let i = 0;i < 35; i++) {
      const inputAt = performance.now() - started;
      const busy = !!document.querySelector("button.chat-abort");
      const textChars = lastText.length;
      const t = performance.now();
      set.call(input, "B7 draft " + i);
      input.dispatchEvent(new Event("input", { bubbles: true }));
      await frame();
      const inputTime = performance.now() - t;
      const u = performance.now();
      list.scrollTop = i % 2 ? list.scrollHeight : 0;
      await frame();
      const scrollTime = performance.now() - u;
      if (i >= 5) {
        inputMs.push(inputTime);
        scrollMs.push(scrollTime);
        sampleWindows.push({ at: inputAt, busy, textChars });
      }
      await wait(200);
    }
    set.call(input, initial);
    input.dispatchEvent(new Event("input", { bubbles: true }));
    while (document.querySelector("button.chat-abort") && performance.now() - started < 45000)
      await wait(100);
    await wait(500);
    const summarize = (a) => {
      const x = [...a].sort((a, b) => a - b);
      return { n: a.length, p50: x[Math.ceil(x.length * 0.5) - 1], p95: x[Math.ceil(x.length * 0.95) - 1], max: x.at(-1), samples: a };
    };
    const nowRows = rows();
    const tail = nowRows.filter((x) => !oldIds.has(x.dataset.messageId) && x.classList.contains("chat-msg-assistant")).at(-1);
    const value = { name, initialRows: oldRows.length, finalRows: nowRows.length, stableOldNodes: oldRows.filter((x) => x.isConnected && nowRows.includes(x)).length, firstAt, elapsedMs: performance.now() - started, updates, sampleWindows, input: summarize(inputMs), scroll: summarize(scrollMs), focusPreserved: document.activeElement === input, draftRestored: input.value === initial, stopClicked, busy: !!document.querySelector("button.chat-abort"), tailText: tail?.innerText ?? "", codeText: tail?.querySelector("code")?.textContent ?? "", method: "live production DOM; 5 warmup pairs +30 paired samples spaced 200 ms; input/scroll to second rAF; remote debug connected without Inspector UI" };
    store(name, value);
    return { ...value, tailText: undefined, codeText: undefined };
  } finally {
    clearTimeout(timer);
    observer.disconnect();
  }
}
function send(text) {
  const input = document.querySelector("textarea.chat-input");
  if (!input)
    throw Error("Missing input");
  const set = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value").set;
  set.call(input, text);
  input.dispatchEvent(new Event("input", { bubbles: true }));
  setTimeout(() => input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", code: "Enter", bubbles: true })), 100);
}
;return {observeStream,send,result:name=>values[name]}})();const p=m.observeStream('fixed-10k');m.send('B7_10K');setTimeout(async()=>{const entry=await window.__TAURI__.core.invoke('history_catalog_access',{key:selectedKey});log('native-access',{entry});log('replay-catalog-event',{});await window.__TAURI__.event.emit('history-catalog-changed');},2800);await p;const result=m.result('fixed-10k');for(let i=0;i<250;i++){const tail=[...document.querySelectorAll('.chat-msg-assistant')].at(-1);if(tail?.textContent.includes('B7_STREAM_END_10000'))break;await wait(100)}const tail=[...document.querySelectorAll('.chat-msg-assistant')].at(-1);return {ok:true,trace,result,final:{rows:document.querySelectorAll('[data-message-id]').length,composer:!!document.querySelector('textarea.chat-input'),sameInput:observedInput===document.querySelector('textarea.chat-input'),focus:document.activeElement===observedInput,stop:!!document.querySelector('button.chat-abort'),text:tail?.innerText,code:tail?.querySelector('code')?.textContent}}}catch(e){return {ok:false,error:String(e),trace}}finally{clearInterval(timer);stopEvent();window.__TAURI_INTERNALS__.invoke=original}})()



