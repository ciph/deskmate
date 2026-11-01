import {readFileSync,writeFileSync,copyFileSync} from 'node:fs';
const root='output/b7-cold-history';
const source=readFileSync(root+'/webview-probes.ts','utf8');
const functions=source.slice(source.indexOf('export async function observeStream')).replaceAll('export ','');
const js=new Bun.Transpiler({loader:'ts'}).transformSync(functions);
const harness=`(()=>{const values={};const wait=ms=>new Promise(r=>setTimeout(r,ms));const frame=()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));const rows=()=>[...document.querySelectorAll('[data-message-id]')];const store=(name,value)=>values[name]=value;${js};return {observeStream,send,result:name=>values[name]}})()`;
for(const name of ['regression-10k','regression-stop']){
 const path=root+'/'+name+'.js';
 writeFileSync(root+'/'+name+'-inline.js',readFileSync(path,'utf8').replace("await import('/b7-qa.js')",harness));
}
console.log('Standalone DOM instrumentation prepared; product assets unchanged.');
