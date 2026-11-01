import {connect,save} from './cdp-client';
const page=process.argv[2]??'settings',label=process.argv[3]??'inspect',c=await connect(page);
try{
 const v=await c.evaluate(`({text:document.body.innerText,dpr:devicePixelRatio,focus:document.hasFocus(),active:document.activeElement?.outerHTML.slice(0,350),controls:[...document.querySelectorAll('button,input,textarea,select,summary')].map(x=>({tag:x.tagName,type:x.type,text:x.innerText??'',label:x.getAttribute('aria-label'),title:x.title,value:x.type==='password'?'[redacted]':x.value,disabled:x.disabled,rect:x.getBoundingClientRect().toJSON()})),scrolls:[...document.querySelectorAll('*')].filter(x=>x.scrollHeight>x.clientHeight+20&&/auto|scroll/.test(getComputedStyle(x).overflowY)).map(x=>({class:x.className,top:x.scrollTop,height:x.clientHeight,total:x.scrollHeight,rect:x.getBoundingClientRect().toJSON()}))})`);
 save('ui-'+label+'.json',v);console.log(JSON.stringify({text:v.text.slice(-1800),dpr:v.dpr,focus:v.focus,active:v.active,controls:v.controls.filter((x:any)=>x.rect.width&&x.rect.height).map((x:any)=>({...x,rect:{x:x.rect.x,y:x.rect.y,width:x.rect.width,height:x.rect.height}})),scrolls:v.scrolls}));
}finally{c.close()}
