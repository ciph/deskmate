import {connect,save} from './cdp-client';
const name=process.argv[2]??'chat';const c=await connect(name);
try{
 const value=await c.evaluate(`({text:document.body.innerText,focus:document.hasFocus(),active:document.activeElement?.outerHTML?.slice(0,500),inputs:[...document.querySelectorAll('textarea,input')].map(x=>({tag:x.tagName,placeholder:x.placeholder,disabled:x.disabled,value:x.type==='password'?'[redacted]':x.value}))})`);
 save('gui-'+(process.argv[3]??name)+'.json',value);console.log(JSON.stringify(value));
}finally{c.close()}
