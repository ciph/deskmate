import pathlib,subprocess,json,os,time
root=pathlib.Path(__file__).resolve().parent
identity='com.deskmate.unifiedhistoryqa.b720261027a'
data=pathlib.Path.home()/'Library/Application Support'/identity
if data.exists(): raise SystemExit('Refuse preexisting QA root')
data.mkdir()
receipt={'identity':identity,'dataRoot':str(data),'birth':data.stat().st_birthtime,'createdAt':time.time()}
(root/'seed-root.json').write_text(json.dumps(receipt,indent=2)+'\n')
workspace=data/'workspace';workspace.mkdir()
(data/'settings.json').write_text(json.dumps({'autostart':False,'memoryAutoExtract':False,'memoryAiUse':False,'worklogAutoArchive':False,'updateRepo':'','chatLarge':True,'mouseFollow':False,'providers':[{'id':'66705bf8-dd7d-4421-baff-e81c5dc9e892','sidecarId':'yume','label':'B7 Local Stream','baseUrl':'http://127.0.0.1:57464/v1','apiKey':'b7-fixture-not-a-secret'}],'activeProviderId':'66705bf8-dd7d-4421-baff-e81c5dc9e892','providerId':'yume','modelId':'b7-local'}))
sidehome=data/'opencode-home';sidehome.mkdir()
env={k:v for k,v in os.environ.items() if k in ['PATH','TMPDIR','LANG','LC_ALL','USER','LOGNAME']}
env.update(HOME=str(sidehome),USERPROFILE=str(sidehome),XDG_CONFIG_HOME=str(sidehome/'xdg-config'),XDG_DATA_HOME=str(sidehome/'xdg-data'),XDG_CACHE_HOME=str(sidehome/'xdg-cache'),OPENCODE_DISABLE_AUTOUPDATE='1')
sid='ses_b7long20261027';stamp=int(time.time()*1000)-100000
info={'id':sid,'slug':'b7-long','projectID':'global','directory':str(workspace),'title':'B7 1000 条合成长会话','version':'1.18.21','time':{'created':stamp,'updated':stamp+999}}
messages=[]
for i in range(1000):
 mid=f'msg_b727long{i:05d}'
 common={'id':mid,'sessionID':sid,'role':'user' if i%2==0 else 'assistant','time':{'created':stamp+i},'agent':'build'}
 if i%2==0:
  common['model']={'providerID':'b7-fixture','modelID':'b7-local'}
  text=f'第 {i//2+1} 轮合成提问：请检查范围、进度和验证结果。'
 else:
  common.update(parentID=f'msg_b727long{i-1:05d}',providerID='b7-fixture',modelID='b7-local',mode='build',path={'cwd':str(workspace),'root':str(workspace)},cost=0,tokens={'input':0,'output':0,'reasoning':0,'cache':{'read':0,'write':0}},finish='stop')
  common['time']['completed']=stamp+i
  text=f'### 合成回复 {i//2+1}\n\n本条只用于界面性能验收。**范围**：独立测试数据。\n\n- 检查状态\n- 保留证据\n- 核对结果\n\n```ts\nconst round = {i//2+1};\n```\n\n| 项目 | 状态 |\n| --- | --- |\n| 本轮 | 完成 |'
 messages.append({'info':common,'parts':[{'id':f'prt_b727long{i:05d}','messageID':mid,'sessionID':sid,'type':'text','text':text}]})
fixture=root/'long-history.json';fixture.write_text(json.dumps({'info':info,'messages':messages},ensure_ascii=False))
binary=root.parents[1]/'src-tauri/resources/opencode/opencode'
start=time.monotonic()
with (root/'long-history-import.log').open('w') as log:
 result=subprocess.run([str(binary),'import',str(fixture)],cwd=workspace,env=env,stdout=log,stderr=subprocess.STDOUT)
(root/'long-history-import.json').write_text(json.dumps({'sessionId':sid,'count':len(messages),'command':'bundled OpenCode import <synthetic JSON>','exitCode':result.returncode,'seconds':time.monotonic()-start,'dataRoot':str(data)},indent=2))
print('IMPORT',result.returncode,flush=True)
raise SystemExit(result.returncode)
