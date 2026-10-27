import os, json, subprocess, pathlib, hashlib, re
r = pathlib.Path(__file__).resolve().parent
repo = r.parents[1]
receipt = json.loads((r/'app-session.json').read_text())
for item in json.loads((r/'final-processes.json').read_text())[:2]:
    p = subprocess.run(['ps','-p',str(item['pid']),'-o','lstart='],capture_output=True,text=True)
    assert p.returncode != 0 or p.stdout.strip() != item['birth'], 'QA process still running'
data = pathlib.Path(receipt['dataRoot'])
seed = json.loads((r/'seed-root.json').read_text())
assert not data.is_symlink() and data.stat().st_birthtime == seed['birth']
sidehome = data/'opencode-home'
env = {k:v for k,v in os.environ.items() if k in ['PATH','TMPDIR','LANG','LC_ALL','USER','LOGNAME']}
env.update(HOME=str(sidehome),USERPROFILE=str(sidehome),XDG_CONFIG_HOME=str(sidehome/'xdg-config'),XDG_DATA_HOME=str(sidehome/'xdg-data'),XDG_CACHE_HOME=str(sidehome/'xdg-cache'),OPENCODE_DISABLE_AUTOUPDATE='1')
results = []
for label, sid, size in [('fixed-10k','ses_b7fixed10k20261027',10000),('fixed-50k','ses_b7fixed50k20261027',50000)]:
    with (r/(label+'-native-export.json')).open('w') as out, (r/(label+'-export.log')).open('w') as err:
        p = subprocess.run([str(repo/'src-tauri/resources/opencode/opencode'),'export',sid],cwd=data/'workspace',env=env,stdout=out,stderr=err)
    assert p.returncode == 0
    x = json.loads((r/(label+'-native-export.json')).read_text())
    a = [m for m in x['messages'] if m['info']['role']=='assistant']
    texts = [''.join(p.get('text','') for p in m['parts'] if p['type']=='text') for m in a]
    complete = [t for t in texts if 'B7_STREAM_END_'+str(size) in t]
    assert len(complete) == 1
    text = complete[0]
    expected = [p for p in json.loads((r/'provider-requests.json').read_text()) if p['size']==size and p['done']][-1]
    sha = hashlib.sha256(text.encode()).hexdigest()
    assert len(text)==size and sha==expected['expectedSha256']
    dom = json.loads((r/(label+'.json')).read_text())
    code = re.search(r'```ts\n(.*?)\n```',text,re.S)[1]
    assert dom['codeText'] == code+'\n', (label,len(dom['codeText']),len(code))
    row = {'sessionId':sid,'messages':len(x['messages']),'characters':len(text),'sha256':sha,'providerMatches':True,'renderedCodeMatches':True,'renderedCodeCharacters':len(dom['codeText'])}
    if size==50000:
        partial = [t for t in texts if t.startswith('# B7_STREAM_50000') and 'B7_STREAM_END_50000' not in t]
        assert len(partial)==1 and 0<len(partial[0])<50000
        assert texts[-1]=='B7_LOCAL_REPLY_OK'
        row.update(stoppedTextCharacters=len(partial[0]),stoppedTextSha256=hashlib.sha256(partial[0].encode()).hexdigest(),resumeText=texts[-1],stopErrorPresentation='Transient UI row: 出错了: Aborted; not an extra persisted assistant message')
    results.append(row)
(r/'native-verification.json').write_text(json.dumps(results,indent=2,ensure_ascii=False)+'\n')
print(json.dumps(results,indent=2,ensure_ascii=False))
