$ErrorActionPreference='Stop'
$env:RUSTUP_TOOLCHAIN='stable'
$taskReport='E:\Codex\工作伙伴\reports\yume-b7-release-worklog-20261027'
$taskStarted=[DateTimeOffset]::Now.ToString('o')
& bun run tauri build --no-bundle --features worklog-qa --config output/b7-release-worklog/tauri.qa.json *> (Join-Path $taskReport 'release-qa-build.log')
$taskCode=$LASTEXITCODE
$taskHash=if(Test-Path -LiteralPath 'src-tauri/target/release/yume.exe'){(Get-FileHash -LiteralPath 'src-tauri/target/release/yume.exe').Hash}else{$null}
@{exitCode=$taskCode;identity='com.deskmate.unifiedhistoryqa.winb7release20261027a';profile='release';lto='fat';panic='unwind';startedAt=$taskStarted;finishedAt=[DateTimeOffset]::Now.ToString('o');binarySha256=$taskHash} | ConvertTo-Json | Set-Content (Join-Path $taskReport 'complete-qa-build.json') -Encoding utf8
Get-Content (Join-Path $taskReport 'complete-qa-build.json')
exit $taskCode
