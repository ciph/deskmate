$ErrorActionPreference='Stop'
$taskReport='E:\Codex\工作伙伴\reports\yume-b7-cold-history-20261027'
$taskRepo='E:\Codex\工作伙伴\deskmate-windows-b7'
$taskResults=@()
foreach($taskPair in @(@{receipt='native-processes-fixed.json';suffix='d';credential='baseline-credential-cleanup.json'},@{receipt='native-processes-candidate.json';suffix='e';credential='candidate-credential-cleanup.json'},@{receipt='native-processes-final.json';suffix='f';credential='final-credential-cleanup.json'},@{receipt='native-processes-acceptance.json';suffix='g';credential='acceptance-credential-cleanup.json'},@{receipt='native-processes-complete.json';suffix='h';credential='complete-credential-cleanup.json'})){
  $taskIdentity='com.deskmate.unifiedhistoryqa.winb7cold20261027'+$taskPair.suffix
  $taskState=Get-Content -Raw -LiteralPath (Join-Path $taskReport $taskPair.receipt) | ConvertFrom-Json
  $taskCredential=Get-Content -Raw -LiteralPath (Join-Path $taskReport $taskPair.credential) | ConvertFrom-Json
  if($taskState.identity -ne $taskIdentity -or !$taskCredential.ok){throw 'Identity or credential cleanup failed'}
  $taskOwnedAlive=@()
  foreach($taskRecord in $taskState.processes){$taskLive=Get-Process -Id $taskRecord.pid -ErrorAction SilentlyContinue;if($taskLive -and $taskLive.StartTime.ToUniversalTime().ToFileTimeUtc() -eq $taskRecord.created -and $taskLive.Path -eq $taskRecord.executable){$taskOwnedAlive+=,$taskRecord}}
  if($taskOwnedAlive.Count){throw 'QA process remains alive'}
  $taskExpected=@((Join-Path ([Environment]::GetFolderPath('ApplicationData')) $taskIdentity),(Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) $taskIdentity))
  if($taskState.roots.Count -ne 2){throw 'Unexpected roots'}
  foreach($taskRoot in $taskState.roots){
    $taskResolved=[IO.Path]::GetFullPath($taskRoot)
    if($taskResolved -notin $taskExpected -or [IO.Path]::GetFileName($taskResolved) -ne $taskIdentity){throw 'Out-of-scope root'}
    if(Test-Path -LiteralPath $taskResolved){
      $taskItem=Get-Item -LiteralPath $taskResolved -Force
      if($taskItem.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Reparse root'}
      $taskReparse=@(Get-ChildItem -LiteralPath $taskResolved -Recurse -Force | Where-Object { $_.Attributes -band [IO.FileAttributes]::ReparsePoint })
      if($taskReparse.Count){throw 'Reparse descendant'}
      Remove-Item -LiteralPath $taskResolved -Recurse -Force
    }
    $taskResults+=,[ordered]@{identity=$taskIdentity;root=$taskResolved;deleted=!(Test-Path -LiteralPath $taskResolved);ownedProcessesAlive=0;credentialCleared=$true}
  }
}
$taskPorts=@(Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue | Where-Object LocalPort -In @(58031,9240) | Select-Object LocalAddress,LocalPort,OwningProcess)
$taskProvider=Get-Content -Raw -LiteralPath (Join-Path $taskRepo 'output\b7-cold-history\provider-session.json') | ConvertFrom-Json
$taskProviderLive=Get-Process -Id $taskProvider.pid -ErrorAction SilentlyContinue
$taskProviderGone=!$taskProviderLive
$taskValue=[ordered]@{time=[DateTimeOffset]::Now.ToString('o');roots=$taskResults;listeners=$taskPorts;portsClosed=$taskPorts.Count -eq 0;providerPid=$taskProvider.pid;providerProcessGone=$taskProviderGone;providerStop='Ctrl-C to owned PTY session; Windows exit 1';noProductionIdentityRun=$true}
$taskValue | ConvertTo-Json -Depth 7 | Set-Content -LiteralPath (Join-Path $taskReport 'cleanup.json') -Encoding utf8
if($taskPorts.Count -or !$taskProviderGone){throw 'Listener or provider remains'}
Write-Output 'Five QA identities, ten exact data roots and owned processes cleaned; both ports closed.'

