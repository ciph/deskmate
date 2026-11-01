import os,sys,json,time,subprocess,pathlib,hashlib
root=pathlib.Path(__file__).resolve().parent
identity='com.deskmate.unifiedhistoryqa.b720261027a'
app=root.parents[1]/'src-tauri/target/debug/bundle/macos/YUME B7 Stream PDF QA.app'
binary=app/'Contents/MacOS/yume'
data=pathlib.Path.home()/'Library/Application Support'/identity
resume = '--resume' in sys.argv
if data.exists() and not resume:
    seed=json.loads((root/'seed-root.json').read_text())
    if seed['identity']!=identity or seed['dataRoot']!=str(data) or data.is_symlink() or seed['birth']!=data.stat().st_birthtime:
        raise SystemExit('Refusing unowned QA data directory')
if resume:
    previous=json.loads((root/'app-session.json').read_text())
    if previous['identity']!=identity or previous['dataRoot']!=str(data) or data.is_symlink(): raise SystemExit('QA receipt mismatch')
    prior=subprocess.run(['ps','-p',str(previous['pid']),'-o','lstart='],capture_output=True,text=True)
    if prior.returncode==0 and prior.stdout.strip()==previous['birth']: raise SystemExit('Previous QA process still alive')
    (root/'app-session-before-restart.json').write_text(json.dumps(previous,indent=2)+'\n')
if not binary.is_file(): raise SystemExit('Missing QA app')
env={k:v for k,v in os.environ.items() if k in ['HOME','USER','LOGNAME','PATH','TMPDIR','LANG','LC_ALL','SHELL','__CF_USER_TEXT_ENCODING']}
log=(root/('app-runtime-final.log' if resume else 'app-runtime.log')).open('w')
process=subprocess.Popen([str(binary)],cwd=root.parents[1],env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
def birth(pid):
    return subprocess.check_output(['ps','-p',str(pid),'-o','lstart=']).decode().strip()
receipt={'identity':identity,'app':str(app),'binary':str(binary),'binarySha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'pid':process.pid,'birth':birth(process.pid),'dataRoot':str(data),'feature':'worklog-qa','config':str(root/'tauri.qa.json'),'profile':'debug','startedAt':time.time()}
(root/'app-session.json').write_text(json.dumps(receipt,indent=2)+'\n')
print(json.dumps(receipt),flush=True)
try:
    code=process.wait()
    print('APP_EXIT',code,flush=True)
finally:
    log.close()
