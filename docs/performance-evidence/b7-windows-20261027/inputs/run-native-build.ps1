$ErrorActionPreference='Stop'
$taskRepo='E:\Codex\工作伙伴\deskmate-windows-b7'
$taskReport='E:\Codex\工作伙伴\reports\yume-b7-windows-20261027'
$taskBaseline=Get-Content -Raw -LiteralPath (Join-Path $taskReport '11-production-check.json') | ConvertFrom-Json
if($taskBaseline.exitCode -ne 0){throw 'Baseline must pass before native continuation'}
Set-Location -LiteralPath $taskRepo
$env:TEMP=Join-Path $taskReport 'tmp';$env:TMP=$env:TEMP;$env:TMPDIR=$env:TEMP
$env:RUSTUP_TOOLCHAIN='stable';$env:RUST_TEST_THREADS='4'
$env:YUME_OPENCODE_SRC=Join-Path $taskRepo 'output\opencode-v1.18.21'
$env:YUME_QA_OPENCODE_BINARY=Join-Path $taskRepo 'src-tauri\resources\opencode\opencode.exe'
$taskSteps=@(
  @{name='12-native-recovery';exe='bun';args=@('scripts/audit-remediation/verify-native.ts')},
  @{name='13-production-desktop-build';exe='bun';args=@('run','tauri','build','--debug','--no-bundle','--config','output/b7-qa/production-build.json')},
  @{name='14-qa-frontend-build';exe='bun';args=@('x','vite','build','--config','output/b7-qa/vite.qa.ts')},
  @{name='15-qa-desktop-build';exe='bun';args=@('run','tauri','build','--debug','--no-bundle','--features','worklog-qa','--config','output/b7-qa/tauri.qa.json')}
)
foreach($taskStep in $taskSteps){
 $taskLog=Join-Path $taskReport ($taskStep.name+'.log');$taskStart=[DateTimeOffset]::Now
 Write-Output ('START '+$taskStep.name+' '+$taskStart.ToString('o'))
 $taskArgs=$taskStep.args
 & $taskStep.exe @taskArgs *> $taskLog
 $taskExit=$LASTEXITCODE
 $taskValue=[ordered]@{name=$taskStep.name;command=($taskStep.exe+' '+($taskArgs -join ' '));startedAt=$taskStart.ToString('o');finishedAt=[DateTimeOffset]::Now.ToString('o');exitCode=$taskExit;head='db07e5bdb02550c4d165da8cd45fb76debefbe17';log=$taskLog}
 if($taskExit -eq 0 -and $taskStep.name -match 'desktop-build'){$taskValue.binarySha256=(Get-FileHash -LiteralPath (Join-Path $taskRepo 'src-tauri\target\debug\yume.exe')).Hash}
 $taskValue | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $taskReport ($taskStep.name+'.json')) -Encoding utf8
 Write-Output ('END '+$taskStep.name+' exit='+$taskExit)
 Get-Content -LiteralPath $taskLog -Tail 8
 if($taskExit -ne 0){exit $taskExit}
}
