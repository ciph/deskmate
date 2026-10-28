$ErrorActionPreference='Stop'
$env:RUSTUP_TOOLCHAIN='stable'
$taskReport='E:\Codex\工作伙伴\reports\yume-b7-attachments-20261027'
$taskStarted=[DateTimeOffset]::Now.ToString('o')
& bun run tauri build --debug --no-bundle --features worklog-qa --config output/b7-attachments/tauri.qa.json *> (Join-Path $taskReport 'qa-build.log')
$taskCode=$LASTEXITCODE
@{exitCode=$taskCode;identity='com.deskmate.unifiedhistoryqa.winb7attachments20261027a';startedAt=$taskStarted;finishedAt=[DateTimeOffset]::Now.ToString('o');binarySha256=(Get-FileHash src-tauri/target/debug/yume.exe).Hash} | ConvertTo-Json | Set-Content (Join-Path $taskReport 'complete-qa-build.json') -Encoding utf8
Get-Content (Join-Path $taskReport 'complete-qa-build.json')
exit $taskCode
