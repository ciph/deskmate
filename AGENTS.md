# YUME 仓库 AI 工作入口

## 2026-10-28 Windows 本机整改交付收尾

当前入口仍是 `E:\Codex\工作伙伴\deskmate-windows-b7` / `codex/yume-performance-b6` / 0.4.21。原有改动已提交 `8702347`，本轮 Stop 取消误报与日志回执会话/目录作用域修复已提交 `36a3c2e`。完整前端 1373 pass、Rust 974 pass，0 fail；8 skip / 24 ignored 保留。补齐真实 1000 历史的 10k/50k 流式、活跃工具二次启动、PDF 堆/RSS、三种宠物隐藏恢复、同 DPI 多屏及 10 分钟空闲记录；隔离进程与数据已清理，正常 release 已恢复。

见 [最终报告](docs/performance-evidence/b7-windows-closeout-20261028/README.md) 与 [20 项审计矩阵](docs/performance-evidence/b7-windows-closeout-20261028/audit-matrix.json)。本机修复交付收尾，不再沿用下方历史的“还需 2–3 轮”估计。19 项代码处置已落地，1 项条件模型实验保留原版；不代表严格全平台 V2 全部通过。Mac 最终实机、混合 DPI/高刷、受控 GPU/功耗 A/B 与完整故障实验室范围仍缺证据，详见报告。后续开发从这些提交按实际瓶颈安排，不自动重复全套或扩大验收；本轮未推送、安装日常环境或发布。下文保留历史状态。

## 2026-10-28 Windows release 附件与日志检查点

B7 仍进行中，当前 E:\Codex\工作伙伴\deskmate-windows-b7 / codex/yume-performance-b6 / db07e5b / 0.4.21。复用经 SHA 核验的隔离 release QA：原生 MP4 选择器、MP3/WAV/文件夹双区域拖放、坏视频回退、发送与重启后历史恢复通过；日志记录、冲突草稿、日报周报、版本采用、复制与导出、计划编辑/暂停、失败后真实重试、关联删除和浅深色固定窗口均有物理证据。构造的最近漏周重启后运行中暂停并取消，服务延迟期过后未归档；不宣称客户端收到了迟到响应。十次普通二次启动通过，活跃工具期间仍待验。本轮无产品源码修改，835 项原有文件内容保留；正常 release 已校验恢复。

六库完整性检查均 ok，但随后发现宿主快照后新建的侧车，不能将检查称为所有进程停止后；该侧车已按父进程/出生时间/路径核验停止，隔离根和临时端口清理完成。详见 [本轮证据](docs/performance-evidence/b7-windows-release-acceptance-20261028/README.md) 与 [结果](docs/performance-evidence/b7-windows-release-acceptance-20261028/summary.json)。继续活跃工具二次启动、Aborted 文案、其余恢复/迁移、PDF 堆/RSS、硬件/生产对照、历史回执恢复新增读取的性能和最终完整门禁；真实模型意图和 Mac 单列。Windows 估计尚需 2–3 轮，出现新阻断为 3–4 轮；未提交、推送、安装或发布。以下历史记录保留当时状态。


## Windows 当前工作入口（2026-10-27）

当前入口 E:\Codex\工作伙伴\deskmate-windows-b7，分支 codex/yume-performance-b6，基线 db07e5b；旧 E 主仓与 C 工作树保留。优化 release 已完成 22 MB MP4 正文真实拖放、完整 30 秒播放、连续进度手势和播放中移除；1/8/20 MiB SHA 回读、64 MiB 媒体解码/清理与非法边界通过。本轮一次完整前端回归 1367 pass / 0 fail / 8 skip、172/172 文件，六库完整性与隔离清理完成，正常 release 恢复。Windows 安全桌面中断后续真实输入：release 原生选择器/其余媒体矩阵及完整工作日志物理表面未完成，不用前轮 debug 或后台 IPC 替代。既有改动保留，本轮无产品源码修改。B7 仍进行中，Windows 仍估 2–3 轮，新增阻断则 3–4 轮；Mac 专有项另计，未提交/推送。 先读 [最新检查点](docs/performance-evidence/b7-windows-release-worklog-20261027/README.md) 与 [Windows 接管入口](docs/performance-windows-handoff.md)。以下 Mac 交接保留来源状态。

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
