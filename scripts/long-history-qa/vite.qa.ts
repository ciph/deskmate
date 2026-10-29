import base from '../../vite.config';
import {mergeConfig} from 'vite';
import {resolve} from 'node:path';
export default mergeConfig(base,{build:{rollupOptions:{input:{longHistoryQA:resolve(__dirname,'webview-probes.ts')},preserveEntrySignatures:'strict',output:{entryFileNames:(chunk:{name:string})=>chunk.name==='longHistoryQA'?'long-history-qa.js':'assets/[name]-[hash].js'}}}});
