from pathlib import Path
import subprocess,json,datetime,os,signal,time,sqlite3,shutil,hashlib,socket
out=Path(__file__).resolve().parent;r=json.loads((out/'app-session.json').read_text());identity=r['identity'];data=Path(r['dataRoot'])
assert identity=='com.deskmate.unifiedhistoryqa.b720261025a'
assert data==Path.home()/'Library/Application Support'/identity and not data.is_symlink()
assert data.stat().st_birthtime>=json.loads((out/'app-session-before-pdf-fix.json').read_text())['startedAt']-10
rows=json.loads((out/'app-owned-processes-final.json').read_text())
old=json.loads((out/'second-launch.json').read_text())['before'];processes=[]
for row in rows+old:
    p=subprocess.run(['ps','-p',str(row['pid']),'-o','lstart='],capture_output=True,text=True)
    same=p.returncode==0 and p.stdout.strip()==row['birth']
    if same:
        os.kill(row['pid'],signal.SIGTERM)
        for _ in range(50):
            time.sleep(.1);p=subprocess.run(['ps','-p',str(row['pid']),'-o','lstart='],capture_output=True,text=True)
            if p.returncode or p.stdout.strip()!=row['birth']:break
        else:raise RuntimeError('Owned QA process failed to exit')
    processes.append({'pid':row['pid'],'birth':row['birth'],'gone':True,'neededTermination':same})
provider=json.loads((out/'gui-provider-receipt.json').read_text());pid=provider['pid']
p=subprocess.run(['ps','-p',str(pid),'-o','lstart=','-o','args='],capture_output=True,text=True)
if p.returncode==0:
    fields=p.stdout.strip().split(None,5);birth=' '.join(fields[:5])
    expected=datetime.datetime.fromisoformat(provider['startedAt'].replace('Z','+00:00')).timestamp()
    assert abs(datetime.datetime.strptime(birth,'%a %b %d %H:%M:%S %Y').timestamp()-expected)<2
    assert fields[5].endswith('output/performance-b7-20261025/gui-provider.ts')
    os.kill(pid,signal.SIGTERM)
    for _ in range(50):
        time.sleep(.1)
        if subprocess.run(['ps','-p',str(pid),'-o','pid='],capture_output=True).returncode:break
    else:raise RuntimeError('Provider failed to exit')
port=int(provider['baseUrl'].split(':')[2].split('/')[0]);probe=socket.socket();probe.settimeout(1);closed=probe.connect_ex(('127.0.0.1',port))!=0;probe.close();assert closed
settings=json.loads((data/'settings.json').read_text());keys=[]
for p in settings['providers']:
    assert p['label']=='B7 local fixture' and p['baseUrl']==provider['baseUrl']
    assert not p.get('apiKey')
    account='ai-api-key.'+p['id']
    delete=subprocess.run(['security','delete-generic-password','-s',identity,'-a',account],capture_output=True)
    assert delete.returncode in [0,44],delete.returncode
    verify=subprocess.run(['security','find-generic-password','-s',identity,'-a',account],capture_output=True)
    assert verify.returncode==44,verify.returncode
    keys.append({'service':identity,'account':account,'removedOrAbsent':True})
readback={}
for name in ['history-catalog.sqlite','deskmate-memory.db','yume-worklog.db','chat-resources.sqlite3']:
    c=sqlite3.connect((data/name).as_uri()+'?mode=ro',uri=True)
    readback[name]={'userVersion':c.execute('pragma user_version').fetchone()[0],'integrity':c.execute('pragma integrity_check').fetchone()[0],'tables':[x[0] for x in c.execute("select name from sqlite_master where type='table' and name not like 'sqlite_%'")]};c.close()
(out/'qa-storage-readback.json').write_text(json.dumps(readback,indent=2)+'\n')
roots=[data,Path.home()/'Library/Caches'/identity,Path.home()/'Library/WebKit'/identity,Path.home()/'Library/HTTPStorages'/identity,Path.home()/'Library/Saved Application State'/(identity+'.savedState')]
removed=[];retained=[]
started=json.loads((out/'app-session-before-pdf-fix.json').read_text())['startedAt']
for p in roots:
    if not p.exists():continue
    if p.is_symlink() or p.stat().st_birthtime<started-10:
        retained.append(str(p));continue
    shutil.rmtree(p);removed.append(str(p))
result={'appProcesses':processes,'providerPid':pid,'providerGone':True,'providerPortClosed':closed,'credentialCleanup':keys,'removedOwnedRoots':removed,'retainedRoots':retained,'dataRootRemoved':not data.exists(),'sourcePdfSha256':hashlib.sha256((out/'b7-fixture.pdf').read_bytes()).hexdigest(),'scope':'Only receipt-owned QA processes, synthetic credential and newly created identity roots; daily YUME untouched.'}
(out/'app-cleanup.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2));print('DATABASES',json.dumps(readback,indent=2))
