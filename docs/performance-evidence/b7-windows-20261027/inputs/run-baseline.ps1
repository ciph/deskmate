param([int]$StartStep=0)
$ErrorActionPreference = 'Stop'
$taskRepo = 'E:\Codex\工作伙伴\deskmate-windows-b7'
$taskReport = 'E:\Codex\工作伙伴\reports\yume-b7-windows-20261027'
$taskUpstream = Join-Path $taskRepo 'output\opencode-v1.18.21'
$env:TEMP = Join-Path $taskReport 'tmp'
$env:TMP = $env:TEMP
$env:TMPDIR = $env:TEMP
Remove-Item Env:BUN_INSTALL_CACHE_DIR -ErrorAction SilentlyContinue
$env:RUSTUP_TOOLCHAIN = 'stable'
$env:RUST_TEST_THREADS = '4'
$env:YUME_OPENCODE_SRC = $taskUpstream
if ((rustc --version) -ne 'rustc 1.94.1 (e408947bf 2026-03-25)') { throw 'Rust version mismatch' }
if ((bun --version) -ne '1.3.14') { throw 'Bun version mismatch' }
$taskSteps = @(
    @{ name='05b-upstream-install'; cwd=$taskUpstream; exe='bun'; args=@('install','--frozen-lockfile') },
    @{ name='06-workbench-build'; cwd=$taskRepo; exe='bun'; args=@('run','prepare:workbench') },
    @{ name='07-typecheck'; cwd=$taskRepo; exe='bun'; args=@('run','typecheck') },
    @{ name='08-frontend'; cwd=$taskRepo; exe='bun'; args=@('run','test:frontend') },
    @{ name='09b-rust-full'; cwd=$taskRepo; exe='cargo'; args=@('test','--locked','--manifest-path','src-tauri/Cargo.toml','--lib','--','--test-threads=4','--nocapture') },
    @{ name='10-frontend-build'; cwd=$taskRepo; exe='bun'; args=@('run','build') },
    @{ name='11-production-check'; cwd=$taskRepo; exe='cargo'; args=@('check','--locked','--manifest-path','src-tauri/Cargo.toml') }
)
foreach ($taskStep in ($taskSteps | Select-Object -Skip $StartStep)) {
    Set-Location -LiteralPath $taskStep.cwd
    $taskLog = Join-Path $taskReport ($taskStep.name + '.log')
    $taskStart = [DateTimeOffset]::Now
    Write-Output ('START ' + $taskStep.name + ' ' + $taskStart.ToString('o'))
    $taskArguments = $taskStep.args
    & $taskStep.exe @taskArguments *> $taskLog
    $taskExit = $LASTEXITCODE
    $taskReceipt = [ordered]@{name=$taskStep.name;cwd=$taskStep.cwd;command=($taskStep.exe+' '+($taskArguments -join ' '));startedAt=$taskStart.ToString('o');finishedAt=[DateTimeOffset]::Now.ToString('o');exitCode=$taskExit;log=$taskLog;head='db07e5bdb02550c4d165da8cd45fb76debefbe17';rustToolchainAlias='stable; exact 1.94.1 hash asserted';testThreads=4}
    $taskReceipt | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $taskReport ($taskStep.name+'.json')) -Encoding utf8
    Write-Output ('END ' + $taskStep.name + ' exit=' + $taskExit)
    Get-Content -LiteralPath $taskLog -Tail 5
    if ($taskExit -ne 0) { exit $taskExit }
    if ($taskStep.name -eq '05b-upstream-install') {
        if ((Get-FileHash -LiteralPath (Join-Path $taskUpstream 'bun.lock') -Algorithm SHA256).Hash -ne '55CDE1D8D6921DDD764AD99E6BE8755767CA81E10F439E047014B7AAACFFAE5C') { throw 'Upstream lock changed' }
    }
}
