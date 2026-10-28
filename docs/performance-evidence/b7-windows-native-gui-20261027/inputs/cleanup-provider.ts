import {connect} from './cdp-client';
import {readFileSync,writeFileSync} from 'node:fs';
const c=await connect('chat');
try{
 const final=await c.invoke('history_catalog_list',{query:{limit:50},refresh:false});
 writeFileSync('E:/Codex/工作伙伴/reports/yume-b7-native-gui-20261027/final-catalog.json',JSON.stringify(final,null,2));
 const cleared=await c.evaluate(readFileSync('output/b7-native-gui/clear-provider.js','utf8'));
 if(!cleared.ok)throw Error(JSON.stringify(cleared));
 writeFileSync('E:/Codex/工作伙伴/reports/yume-b7-native-gui-20261027/provider-credential-cleared.json',JSON.stringify(cleared,null,2));
 console.log(JSON.stringify(cleared));
}finally{c.close()}
