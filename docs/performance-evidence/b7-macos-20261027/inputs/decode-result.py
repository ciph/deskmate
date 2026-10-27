import re,gzip,base64,json,pathlib,sys
r=pathlib.Path(__file__).resolve().parent
for name in sys.argv[1:]:
 chunks=re.findall(r'text B7_EXPORT '+re.escape(name)+r' (\d+) ([A-Za-z0-9+/=]+)',(r/(name+'-ax.txt')).read_text());assert chunks,name
 numbers=[int(i) for i,v in chunks];assert numbers==list(range(len(numbers)))
 x=json.loads(gzip.decompress(base64.b64decode(''.join(v for i,v in chunks))));(r/(name+'.json')).write_text(json.dumps(x,indent=2))
 print(name,{k:v for k,v in x.items() if k not in ['tailText','codeText','updates','input','scroll','sampleWindows','events']})
 if 'input' in x:print('updates',len(x['updates']),'input p95',x['input']['p95'],'scroll p95',x['scroll']['p95'],'samples during stream',sum(v['busy'] for v in x.get('sampleWindows',[])),'sample chars range',[(x['sampleWindows'][i]['textChars']) for i in [0,-1]])
