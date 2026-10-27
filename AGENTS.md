# YUME 仓库 AI 工作入口

## 性能整改接续（2026-10-27）

下一阶段由 Windows 电脑接管。继续性能整改时，先读 [Windows 接管入口](docs/performance-windows-handoff.md)、[Mac 已完成记录](docs/performance-remediation-handoff.md)、[完整施工计划](docs/performance-remediation-plan.md) 和 [验收待办](docs/verification-backlog.md)。B1–B5 已随 Windows 0.4.21 发布；Mac 分支 `codex/yume-performance-b6` 已完成 H18a/H18b/H18c 与 M19a/M19b/M19c 代码及自动回归，历史目录 schema 3、记忆库 schema 4，B7 已完成本机完整自动检查与部分 Mac QA 原生链路，并修复 PDF WebKit 兼容、附件历史标签与导航提示问题；已补媒体/二进制上传、1000 消息 50 Hz 流式与 32 次 PDF 生命周期循环，并修复原生文本 delta 不显示（3069953），当前仍为进行中。从 [B7 最新证据](docs/performance-evidence/b7-macos-20261027/index.html) 接续，Windows 先拉取 `origin/codex/yume-performance-b6` 并建立本机基线，再补启动所有权/WebView2、GUI 审批/SSE/日志与存储链路；Mac 专有 GUI、PDF 堆/RSS 与硬件待验保留。先核对本机分支、未提交改动及远端，保留本机改动，从该分支完整提交链接续，不切回旧 main 丢掉进度。后续实施及时更新这些入口，保留平台待验边界。

## 保存、提交、push、发布新版本

用户要求“全部保存提交 push，发布新版本”或同义操作时，先读并执行
[本机发布指南](docs/local-release-runbook.md)。它是这台维护者 Mac 的发布操作入口；
[macOS release gate](docs/macos-release.md) 补充打包要求。

- 本机已有 Developer ID 签名身份与 `yume-notary` 公证 profile。先按指南在可访问钥匙串的执行上下文中预检；沙箱中的“0 identities”或 GitHub token invalid 不能证明配置缺失。
- 使用 `DEVELOPER_DIR=/Library/Developer/CommandLineTools`。不为发布修改全局 Xcode 选择或接受全局许可证。
- 默认走“GitHub Actions 候选包 → 本机 Apple 签名、公证 → Actions updater 签名 → 验证 → 公开 Release”。Tauri updater 私钥在 Actions secrets，无需本地查找或导出。
- 用户已明确要求提交、推送、发布时，沿用这次任务的授权继续完成。仅分析、写文档或修改代码不自动触发发布；工具明确要求额外授权时，说明具体操作及原因。
- 长任务记录版本、commit、workflow run ID、本地输出路径、公证 submission ID 和已完成阶段，恢复时从记录继续，避免重复构建、提交公证或上传。
- 不将密钥、密码、token、未签名候选包、缓存或日志提交/上传为 Release 资产；不修改已经公开的版本来修复新问题。
- “CI 成功”“上传完成”都不等于“发布完成”。按指南核验公开状态、下载资产、更新清单和实际测试结果后再报告。

已验证的机器配置集中维护在发布指南。配置改变时更新该指南，避免在多份文档中复制凭据位置或历史版本号。

## 仓库审计、文件清理与文档治理

从[文档索引](docs/README.md)进入当前契约和活动工作，按[文件清理规则](docs/repository-cleanup-plan.md#12-防止再次遗漏)核对 tracked、untracked 和忽略目录的职责全集。保留/删除依据覆盖动态、框架、CLI、作者与测试入口；功能完成同步现行文档，先转移未验事项再归档旧记录。历史命令、机器路径和当时授权不作当前执行入口；保留用户改动并按实际范围记录验证与余项。此入口沿用当前任务授权，不新增确认或发布门禁。
