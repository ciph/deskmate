import pathlib,json,subprocess,os,signal,time,sqlite3,shutil,socket
r=pathlib.Path(__file__).resolve().parent
seed=json.loads((r/'seed-root.json').read_text());scope=json.loads((r/'provider-scope.json').read_text())
identity=seed['identity'];assert scope['identity']==identity=='com.deskmate.unifiedhistoryqa.b720261027a'
records=json.loads((r/'final-processes.json').read_text());result={'identity':identity,'processes':[],'databases':[],'roots':[]}
for row in records:
    p=subprocess.run(['ps','-p',str(row['pid']),'-o','lstart='],capture_output=True,text=True)
    alive=p.returncode==0 and p.stdout.strip()==row['birth']
    if alive:
        assert row['pid']==48431, 'QA app or engine still alive; stop normally first'
        command=subprocess.check_output(['ps','-p',str(row['pid']),'-o','command=']).decode().strip()
        assert command=='bun output/performance-b7-20261027/provider.ts'
        os.kill(row['pid'],signal.SIGTERM)
        for _ in range(50):
            p=subprocess.run(['ps','-p',str(row['pid']),'-o','lstart='],capture_output=True,text=True)
            if p.returncode or p.stdout.strip()!=row['birth']:break
            time.sleep(.1)
        assert p.returncode or p.stdout.strip()!=row['birth']
    result['processes'].append({'pid':row['pid'],'birth':row['birth'],'gone':True})
allps=subprocess.check_output(['ps','-axo','pid=,command=']).decode().splitlines()
bundle='/src-tauri/target/debug/bundle/macos/YUME B7 Stream PDF QA.app/'
assert not any(bundle in x for x in allps), 'Unregistered QA bundle process remains'
data=pathlib.Path(seed['dataRoot']);assert not data.is_symlink() and data.stat().st_birthtime==seed['birth']
for db in sorted(data.rglob('*')):
    if db.suffix not in ('.db','.sqlite','.sqlite3') or not db.is_file():continue
    con=sqlite3.connect(db.as_uri()+'?mode=ro',uri=True)
    checks=[x[0] for x in con.execute('PRAGMA integrity_check')];con.close();assert checks==['ok'],(str(db),checks)
    result['databases'].append({'path':str(db.relative_to(data)),'integrity':checks})
account='ai-api-key.'+scope['providerId']
deleted=subprocess.run(['security','delete-generic-password','-s',identity,'-a',account],capture_output=True)
assert deleted.returncode in (0,44)
absent=subprocess.run(['security','find-generic-password','-s',identity,'-a',account],capture_output=True)
assert absent.returncode==44
result['dummyCredential']={'deleteExit':deleted.returncode,'absentExit':absent.returncode}
for folder in ('Application Support','Caches','WebKit'):
    p=pathlib.Path.home()/'Library'/folder/identity
    if not p.exists():result['roots'].append({'path':str(p),'alreadyAbsent':True});continue
    assert not p.is_symlink() and p.name==identity
    birth=p.stat().st_birthtime
    assert seed['birth']-1<=birth<=time.time()
    result['roots'].append({'path':str(p),'birth':birth,'removed':True})
    shutil.rmtree(p)
result['ports']={}
for port in (57464,62914):
    with socket.socket() as s:s.settimeout(.2);closed=s.connect_ex(('127.0.0.1',port))!=0
    assert closed;result['ports'][str(port)]='closed'
result['finishedAt']=time.time()
(r/'cleanup.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
