setTimeout(()=>{void (async()=>{
 const input=document.querySelector('textarea.chat-input');
 const first=document.querySelector('[data-message-id]');
 const list=first?.parentElement;
 if(!input||!list)throw new Error('Missing production chat controls');
 const initial=input.value;
 const frame=()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));
 const inputMs=[],scrollMs=[];
 const set=Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype,'value').set;
 const started=performance.now();
 for(let i=0;i<35;i++){
  const start=performance.now();set.call(input,'B7 synthetic input '+i);input.dispatchEvent(new Event('input',{bubbles:true}));await frame();
  if(i>=5)inputMs.push(performance.now()-start);
 }
 set.call(input,initial);input.dispatchEvent(new Event('input',{bubbles:true}));
 for(let i=0;i<35;i++){
  const start=performance.now();list.scrollTop=i%2===0?0:list.scrollHeight;await frame();
  if(i>=5)scrollMs.push(performance.now()-start);
 }
 const summarize=x=>{const s=[...x].sort((a,b)=>a-b);return {n:x.length,p50:s[Math.ceil(s.length*.5)-1],p95:s[Math.ceil(s.length*.95)-1],max:s.at(-1),samples:x}};
 const result={rows:document.querySelectorAll('[data-message-id]').length,viewport:[innerWidth,innerHeight],listHeight:list.clientHeight,scrollHeight:list.scrollHeight,input:summarize(inputMs),scroll:summarize(scrollMs),elapsedMs:performance.now()-started,method:'programmatic DOM input/scroll to second requestAnimationFrame; 5 warmups + 30 samples; Inspector closed'};
 localStorage.setItem('b7-interaction-result',JSON.stringify(result));
 const out=document.createElement('pre');out.id='b7-interaction-result';out.textContent='B7_INTERACTION '+JSON.stringify(result);document.body.append(out);
 })().catch(e=>{const p=document.createElement('pre');p.textContent='B7_INTERACTION_ERROR '+e;document.body.append(p);});},8000);void 0;
