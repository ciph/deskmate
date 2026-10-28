$ErrorActionPreference='Stop'
$env:RUSTUP_TOOLCHAIN='stable'
$taskReport='E:\Codex\工作伙伴\reports\yume-b7-release-worklog-20261027'
$taskStart=[DateTimeOffset]::Now.ToString('o')
& bun run tauri build --no-bundle *> (Join-Path $taskReport 'normal-release-build.log')
$taskCode=$LASTEXITCODE
$taskHash=if(Test-Path -LiteralPath 'src-tauri/target/release/yume.exe'){(Get-FileHash -LiteralPath 'src-tauri/target/release/yume.exe').Hash}else{$null}
@{command='bun run tauri build --no-bundle';exitCode=$taskCode;profile='release';features='default';startedAt=$taskStart;finishedAt=[DateTimeOffset]::Now.ToString('o');binarySha256=$taskHash;productionIdentityRun=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskReport 'normal-release-build.json') -Encoding utf8
Get-Content -LiteralPath (Join-Path $taskReport 'normal-release-build.json')
exit $taskCode
