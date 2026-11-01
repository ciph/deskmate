import subprocess,json,time,pathlib,statistics,math
out=pathlib.Path(__file__).resolve().parent
receipt=json.loads((out/'app-session.json').read_text());pid=receipt['pid']
def processes():
    result={}
    for line in subprocess.check_output(['ps','-axo','pid=,ppid=,lstart=,%cpu=,rss=,comm=']).decode().splitlines():
        p=line.strip().split(None,9)
        if len(p)!=10: continue
        result[int(p[0])]={'pid':int(p[0]),'ppid':int(p[1]),'birth':' '.join(p[2:7]),'cpuPercent':float(p[7]),'rssKiB':int(p[8]),'command':p[9]}
    if pid not in result or result[pid]['birth']!=receipt['birth']: raise RuntimeError('QA root process ownership lost')
    owned={pid}
    while True:
        expanded=owned|{p for p,r in result.items() if r['ppid'] in owned}
        if expanded==owned: break
        owned=expanded
    return [result[p] for p in sorted(owned)]
def files():
    return {str(p.relative_to(receipt['dataRoot'])):{'size':p.stat().st_size,'mtimeNs':p.stat().st_mtime_ns} for p in pathlib.Path(receipt['dataRoot']).rglob('*') if p.is_file()}
initial=processes()
# One sequential second launch: the original primary and engine must survive.
with (out/'second-launch.log').open('w') as log:
    second=subprocess.Popen([receipt['binary']],stdout=log,stderr=subprocess.STDOUT)
    try: code=second.wait(timeout=15)
    except subprocess.TimeoutExpired:
        second.terminate();second.wait(timeout=5);raise RuntimeError('Second launch did not exit')
after=processes()
second_receipt={'secondaryPid':second.pid,'secondaryExit':code,'primarySurvived':True,'before':initial,'after':after}
(out/'second-launch.json').write_text(json.dumps(second_receipt,indent=2)+'\n')
print('SECOND_LAUNCH',code,'primary and descendants recorded',flush=True)
# Warm-up after the second launch reveals the chat; no interactions during the sample.
time.sleep(5)
start=time.monotonic();wall=time.time();before=files();samples=[]
with (out/'idle-samples.jsonl').open('w') as log:
    while True:
        elapsed=time.monotonic()-start
        rows=processes()
        sample={'elapsedSeconds':round(elapsed,3),'processes':rows,'totalCpuPercent':sum(r['cpuPercent'] for r in rows),'totalRssKiB':sum(r['rssKiB'] for r in rows)}
        samples.append(sample);log.write(json.dumps(sample)+'\n');log.flush()
        if len(samples)%12==1: print('IDLE',round(elapsed),len(rows),'processes',flush=True)
        if elapsed>=600: break
        time.sleep(min(5,600-(time.monotonic()-start)))
end=files()
def summary(key):
    values=sorted(s[key] for s in samples)
    return {'p50':statistics.median(values),'p95':values[math.ceil(len(values)*.95)-1],'min':values[0],'max':values[-1],'first':samples[0][key],'last':samples[-1][key]}
result={'startedAt':wall,'elapsedSeconds':samples[-1]['elapsedSeconds'],'samples':len(samples),'intervalSeconds':5,'scope':'Owned QA host and descendants only; excludes unattributable WebKit/GPU processes and system energy. ps %cpu is an OS estimate, not interval CPU accounting.','condition':'debug worklog-qa; default GIF pet visible, empty chat visible, settings/workbench hidden; no active model requests; other user apps remain running; no concurrent build/test','cpuPercent':summary('totalCpuPercent'),'rssKiB':summary('totalRssKiB'),'changedFiles':[p for p in sorted(before.keys()|end.keys()) if before.get(p)!=end.get(p)],'fileStatsBefore':before,'fileStatsAfter':end}
(out/'idle-summary.json').write_text(json.dumps(result,indent=2)+'\n');print('COMPLETE',result['cpuPercent'],result['rssKiB'],flush=True)
