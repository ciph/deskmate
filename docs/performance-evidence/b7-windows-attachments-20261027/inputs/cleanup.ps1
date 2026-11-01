$ErrorActionPreference='Stop'
$taskReport='E:\Codex\工作伙伴\reports\yume-b7-attachments-20261027'
$taskIdentity='com.deskmate.unifiedhistoryqa.winb7attachments20261027a'
$taskState=Get-Content -Raw -LiteralPath (Join-Path $taskReport 'native-processes-complete.json') | ConvertFrom-Json
$taskCredential=Get-Content -Raw -LiteralPath (Join-Path $taskReport 'provider-credential-cleared.json') | ConvertFrom-Json
if($taskState.identity -ne $taskIdentity -or !$taskCredential.ok){throw 'Identity or credentials not verified'}
foreach($taskRecord in $taskState.processes){
  $taskLive=Get-Process -Id $taskRecord.pid -ErrorAction SilentlyContinue
  if($taskLive -and $taskLive.StartTime.ToUniversalTime().ToFileTimeUtc() -eq $taskRecord.created -and $taskLive.Path -eq $taskRecord.executable){throw 'Owned QA process remains alive'}
}
$taskExpected=@((Join-Path ([Environment]::GetFolderPath('ApplicationData')) $taskIdentity),(Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) $taskIdentity))
if($taskState.roots.Count -ne 2){throw 'Unexpected roots'}
$taskResults=@()
foreach($taskRoot in $taskState.roots){
 $taskResolved=[IO.Path]::GetFullPath($taskRoot)
 if($taskResolved -notin $taskExpected -or [IO.Path]::GetFileName($taskResolved) -ne $taskIdentity){throw 'Out of scope root'}
 if(Test-Path -LiteralPath $taskResolved){
  if((Get-Item -LiteralPath $taskResolved -Force).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Reparse root'}
  if(@(Get-ChildItem -LiteralPath $taskResolved -Recurse -Force | Where-Object {$_.Attributes -band [IO.FileAttributes]::ReparsePoint}).Count){throw 'Reparse descendant'}
  Remove-Item -LiteralPath $taskResolved -Recurse -Force
 }
 $taskResults+=@{root=$taskResolved;deleted=!(Test-Path -LiteralPath $taskResolved)}
}
$taskProvider=Get-Content -Raw -LiteralPath "output/b7-attachments/provider-session.json" | ConvertFrom-Json
$taskBridge=Get-Content -Raw -LiteralPath "output/b7-attachments/bridge-session.json" | ConvertFrom-Json
$taskListeners=@(Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue | Where-Object LocalPort -In @($taskProvider.port,$taskProvider.controlPort,$taskBridge.port,9242) | Select-Object LocalAddress,LocalPort,OwningProcess)
$taskHelpers=@(Get-Process -Id $taskProvider.pid,$taskBridge.pid,$taskBridge.mcpPid -ErrorAction SilentlyContinue | Select-Object Id,ProcessName)
$taskResult=@{identity=$taskIdentity;roots=$taskResults;listeners=$taskListeners;helperProcesses=$taskHelpers;credentialCleared=$true;productionIdentityRun=$false;time=[DateTimeOffset]::Now.ToString('o')}
$taskResult | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $taskReport 'cleanup.json') -Encoding utf8
if($taskListeners.Count -or $taskHelpers.Count){throw 'Cleanup incomplete'}
Write-Output 'Two exact QA roots removed; owned helpers and all three ports closed.'
