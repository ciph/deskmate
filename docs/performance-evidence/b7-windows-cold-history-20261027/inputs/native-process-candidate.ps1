param([ValidateSet('seed','launch','snapshot','repeat','crash','stop')][string]$Action)
$ErrorActionPreference='Stop'
$taskRepo='E:\Codex\工作伙伴\deskmate-windows-b7'
$taskReport='E:\Codex\工作伙伴\reports\yume-b7-cold-history-20261027'
$taskQa=Join-Path $taskRepo 'output\b7-cold-history'
$taskIdentity='com.deskmate.unifiedhistoryqa.winb7cold20261027e'
$taskData=Join-Path ([Environment]::GetFolderPath('ApplicationData')) $taskIdentity
$taskLocal=Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) $taskIdentity
$taskExe=Join-Path $taskRepo 'src-tauri\target\debug\yume.exe'
$taskReceipt=Join-Path $taskReport 'native-processes-candidate.json'
function Save-TaskJson($name,$value){ $value | ConvertTo-Json -Depth 14 | Set-Content -LiteralPath (Join-Path $taskReport $name) -Encoding utf8 }
function Read-TaskReceipt { if(!(Test-Path -LiteralPath $taskReceipt)){ throw 'Missing owned-process receipt' }; Get-Content -Raw -LiteralPath $taskReceipt | ConvertFrom-Json }
function Get-TaskProcess($taskPid){
  $taskProcess=Get-Process -Id $taskPid -ErrorAction SilentlyContinue
  if(!$taskProcess){ return $null }
  [pscustomobject]@{pid=$taskProcess.Id;created=$taskProcess.StartTime.ToUniversalTime().ToFileTimeUtc();executable=$taskProcess.Path}
}
function Assert-TaskProcess($taskRecord){
  $taskLive=Get-TaskProcess $taskRecord.pid
  if(!$taskLive){ throw "Owned process absent: $($taskRecord.pid)" }
  if($taskLive.created -ne $taskRecord.created -or $taskLive.executable -ne $taskRecord.executable){ throw 'PID identity changed' }
  $taskLive
}
function Get-TaskOwnership {
  $taskFiles=@(Get-ChildItem -LiteralPath (Join-Path $taskData 'sidecar-ownership') -Filter '*.json' -ErrorAction SilentlyContinue)
  @($taskFiles | ForEach-Object { Get-Content -Raw -LiteralPath $_.FullName | ConvertFrom-Json })
}
function Capture-TaskDescendants($taskKnown){
  $taskAll=@(Get-CimInstance Win32_Process)
  $taskOwned=@($taskKnown)
  $taskChanged=$true
  while($taskChanged){
    $taskChanged=$false
    foreach($taskCandidate in $taskAll){
      if($taskCandidate.ProcessId -in @($taskOwned.pid)){continue}
      $taskParent=$taskOwned | Where-Object pid -EQ $taskCandidate.ParentProcessId | Select-Object -First 1
      if($taskParent -and $taskCandidate.CreationDate.ToUniversalTime().ToFileTimeUtc() -ge $taskParent.created){
        $taskExact=Get-TaskProcess $taskCandidate.ProcessId
        if($taskExact){$taskOwned+=,$taskExact;$taskChanged=$true}
      }
    }
  }
  $taskOwned
}
if($Action -eq 'seed'){
  foreach($taskRoot in @($taskData,$taskLocal)){
    if([IO.Path]::GetFileName([IO.Path]::GetFullPath($taskRoot)) -ne $taskIdentity -or (Test-Path -LiteralPath $taskRoot)){throw 'Refuse preexisting or invalid QA root'}
  }
  $taskProvider=Get-Content -Raw -LiteralPath (Join-Path $taskQa 'provider-session.json') | ConvertFrom-Json
  $taskUri=[uri]$taskProvider.baseUrl
  if($taskUri.Scheme -ne 'http' -or $taskUri.Host -ne '127.0.0.1'){throw 'Fixture must be loopback'}
  New-Item -ItemType Directory -Path $taskData,$taskLocal -Force | Out-Null
  [ordered]@{autostart=$false;memoryAutoExtract=$false;memoryAiUse=$false;worklogAutoArchive=$false;chatLarge=$true;mouseFollow=$false;providers=@(@{id='winb7fixture';sidecarId='yume';label='Windows B7 Local';baseUrl=$taskProvider.baseUrl;apiKey='b7-fixture-not-a-secret';manualModelIds='b7-local'});activeProviderId='winb7fixture';providerId='yume';modelId='b7-local'} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $taskData 'settings.json') -Encoding utf8NoBOM
  Save-TaskJson 'native-processes-candidate.json' ([ordered]@{identity=$taskIdentity;roots=@($taskData,$taskLocal);executable=$taskExe;processes=@();launches=@();createdAt=[DateTimeOffset]::Now.ToString('o')})
  Write-Output 'Fresh QA roots and synthetic local-provider settings created.'
  exit
}
$taskState=Read-TaskReceipt
if($taskState.identity -ne $taskIdentity -or $taskState.executable -ne $taskExe){throw 'Receipt scope mismatch'}
if($Action -eq 'launch'){
  $taskBuild=Get-Content -Raw -LiteralPath (Join-Path $taskReport 'candidate-build.json') | ConvertFrom-Json
  if($taskBuild.exitCode -ne 0 -or $taskBuild.binarySha256 -ne (Get-FileHash -LiteralPath $taskExe).Hash){throw 'Missing or stale QA build receipt'}
  $env:TEMP=Join-Path $taskRepo 'output\tmp';$env:TMP=$env:TEMP;$env:TMPDIR=$env:TEMP
  $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=9240 --enable-precise-memory-info'
  $taskStamp=Get-Date -Format 'HHmmssfff'
  $taskApp=Start-Process -FilePath $taskExe -WorkingDirectory (Split-Path $taskExe) -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $taskReport "app-$taskStamp.stdout.log") -RedirectStandardError (Join-Path $taskReport "app-$taskStamp.stderr.log")
  $taskRecord=[pscustomobject]@{pid=$taskApp.Id;created=$taskApp.StartTime.ToUniversalTime().ToFileTimeUtc();executable=$taskExe}
  $taskState.processes+=,$taskRecord
  $taskState.launches+=,[pscustomobject]@{pid=$taskRecord.pid;created=$taskRecord.created;binarySha256=(Get-FileHash -LiteralPath $taskExe).Hash;startedAt=[DateTimeOffset]::Now.ToString('o')}
  Save-TaskJson 'native-processes-candidate.json' $taskState
  Write-Output ('Launched isolated QA PID '+$taskApp.Id)
  exit
}
if($Action -eq 'snapshot'){
  $taskOwnership=Get-TaskOwnership
  foreach($taskOwner in $taskOwnership){
    if($taskOwner.identifier -ne $taskIdentity){throw 'Ownership record identity mismatch'}
    $null=Assert-TaskProcess $taskOwner.host
    $null=Assert-TaskProcess $taskOwner.sidecar
    foreach($taskIdentityRecord in @($taskOwner.host,$taskOwner.sidecar)){
      if($taskIdentityRecord.pid -notin @($taskState.processes.pid)){$taskState.processes+=,$taskIdentityRecord}
    }
  }
  $taskKnown=@($taskState.processes | Where-Object {Get-Process -Id $_.pid -ErrorAction SilentlyContinue})
  foreach($taskKnownItem in $taskKnown){$null=Assert-TaskProcess $taskKnownItem}
  $taskNew=@(Capture-TaskDescendants $taskKnown)
  foreach($taskItem in $taskNew){if($taskItem.pid -notin @($taskState.processes.pid)){$taskState.processes+=,$taskItem}}
  Save-TaskJson 'native-processes-candidate.json' $taskState
  Save-TaskJson ('native-snapshot-'+(Get-Date -Format 'HHmmssfff')+'.json') @{ownership=$taskOwnership;processes=$taskNew;time=[DateTimeOffset]::Now.ToString('o')}
  $taskOwnership | ConvertTo-Json -Depth 6
  exit
}
if($Action -eq 'repeat'){
  $taskOwners=@(Get-TaskOwnership)
  if($taskOwners.Count -ne 1){throw 'Expected one primary ownership record'}
  $taskOwner=$taskOwners[0]
  $null=Assert-TaskProcess $taskOwner.host;$null=Assert-TaskProcess $taskOwner.sidecar
  $taskResults=@()
  for($taskIndex=0;$taskIndex -lt 10;$taskIndex++){
    $taskBegin=[DateTimeOffset]::Now
    $taskSecondary=Start-Process -FilePath $taskExe -WorkingDirectory (Split-Path $taskExe) -WindowStyle Hidden -PassThru
    if(!$taskSecondary.WaitForExit(8000)){throw 'Secondary instance did not exit within 8 seconds'}
    $taskCode=$taskSecondary.ExitCode
    if($taskCode -ne 0){throw "Secondary exit code $taskCode"}
    $null=Assert-TaskProcess $taskOwner.host;$null=Assert-TaskProcess $taskOwner.sidecar
    $taskResults+=,[pscustomobject]@{iteration=$taskIndex;secondaryPid=$taskSecondary.Id;exitCode=$taskCode;elapsedMs=([DateTimeOffset]::Now-$taskBegin).TotalMilliseconds;hostPreserved=$true;sidecarPreserved=$true}
  }
  Save-TaskJson ('native-repeat-'+(Get-Date -Format 'HHmmssfff')+'.json') @{owner=$taskOwner;results=$taskResults}
  Write-Output '10 secondary launches exited 0; primary and sidecar PID/creation-time unchanged.'
  exit
}
if($Action -eq 'crash'){
  $taskOwners=@(Get-TaskOwnership)
  if($taskOwners.Count -ne 1){throw 'Expected one primary ownership record'}
  $taskOwner=$taskOwners[0]
  if($taskOwner.identifier -ne $taskIdentity){throw 'Refusing a foreign owner'}
  $null=Assert-TaskProcess $taskOwner.host;$null=Assert-TaskProcess $taskOwner.sidecar
  Save-TaskJson ('native-before-crash-'+(Get-Date -Format 'HHmmssfff')+'.json') $taskOwner
  Stop-Process -Id $taskOwner.host.pid -ErrorAction Stop
  Start-Sleep -Milliseconds 500
  $null=Assert-TaskProcess $taskOwner.sidecar
  Write-Output ('Stopped exact QA host; owned orphan remains PID '+$taskOwner.sidecar.pid)
  exit
}
if($Action -eq 'stop'){
  $taskAlive=@($taskState.processes | Where-Object {Get-Process -Id $_.pid -ErrorAction SilentlyContinue})
  foreach($taskItem in $taskAlive){$null=Assert-TaskProcess $taskItem}
  $taskAll=@(Capture-TaskDescendants $taskAlive)
  Save-TaskJson 'native-stop-owned.json' $taskAll
  foreach($taskItem in ($taskAll | Sort-Object created -Descending)){
    if(Get-Process -Id $taskItem.pid -ErrorAction SilentlyContinue){$null=Assert-TaskProcess $taskItem;Stop-Process -Id $taskItem.pid -ErrorAction Stop}
  }
  Write-Output 'Stopped only recorded and revalidated QA processes; data retained for integrity checks.'
}


