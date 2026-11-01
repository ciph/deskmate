import base from '../../vite.config';
import {mergeConfig} from 'vite';
import {resolve} from 'node:path';
export default mergeConfig(base,{build:{rollupOptions:{input:{b7qa:resolve(__dirname,'webview-probes.ts')},preserveEntrySignatures:'strict',output:{entryFileNames:(chunk:any)=>chunk.name==='b7qa'?'b7-qa.js':'assets/[name]-[hash].js'}}}});
