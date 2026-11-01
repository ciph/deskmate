import {readFileSync} from 'node:fs';import {connect,save} from './cdp-client';
const c=await connect('chat');try{
 await c.evaluate(readFileSync('output/b7-attachments/binary-browser.js','utf8'));
 await c.evaluate('window.b7Binary.run().then(x=>window.b7BinaryDone=x).catch(e=>window.b7BinaryDone={pass:false,error:String(e),state:window.b7Binary.state});true');
 for(let i=0;i<180;i++){await Bun.sleep(1000);const r=await c.evaluate('window.b7BinaryDone??null');if(r){save('binary-results.json',r);console.log(JSON.stringify(r));if(!r.pass)process.exitCode=1;break;}if(i===179)throw Error('Binary probe deadline');}
}finally{c.close()}
