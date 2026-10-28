# YUME 仓库 AI 工作入口

## 当前入口与状态

- 当前 Windows 开发 checkout：`E:\Codex\工作伙伴\deskmate-windows-b7`，分支 `codex/yume-performance-b6`。每次先核对实际路径、分支、Git 状态及 package.json，不能只信聊天 cwd。
- 最近发布 Windows 0.4.22，发布源码 `1901fb8`；历史 schema 3、记忆 schema 4。[发布回执](docs/performance-evidence/release-v0.4.22-windows/README.md) 与 [B7 交付报告](docs/performance-evidence/b7-windows-closeout-20261028/README.md) 是当前证据入口。
- B7 本机整改已交付，严格跨平台/硬件 V2 未全部验收。当前边界与后续成本只在 [Windows 状态](docs/performance-windows-handoff.md) 和 [验收待办](docs/verification-backlog.md) 维护，不继续复制历史进度或轮次估计。
- 旧 `E:\Codex\工作伙伴\deskmate-public-release`、其他工作树和其中的未提交改动保留。不要因为旧项目入口指向它们就切换施工；不 reset/clean，不移动或删除其 `.git`。

## 开发、清理与验证

- 从 [文档索引](docs/README.md) 查当前契约，脚本职责见 [脚本索引](scripts/README.md)，固定工作台输入见 [构建契约](docs/workbench-build.md)。
- 保留已有改动和独有证据；清理前按 [文件治理规则](docs/repository-cleanup-plan.md#12-防止再次遗漏) 核对 tracked/untracked/ignored、消费者、路径与正在使用的进程。不能把无静态引用或日期旧直接当删除理由。
- 生成物可按记录重建；本机输出保留说明在 `output/README.md`，实际清理结果见 [清理进度](docs/repository-cleanup-progress.md)。不删除用户 AppData、凭据、角色作者成果或发布回执。
- 已通过且源码未变的检查不重复；修改后执行对应范围的必要验证。类型、前端、Rust、原生 GUI 和硬件证据分别记录，不互相替代。
- 历史文档只保留原时点结论；完成后的规则写回现行契约，未验项转入待办，归档后修复链接。

## 提交、推送与发布

- 沿用用户对本次任务的授权，不为已有授权重复询问。分析、清理或代码修改不自动触发新版本发布；不覆盖已公开 tag/资产。
- Windows 使用 [release workflow](.github/workflows/release.yml) 和 [版本校验](scripts/check-version.ps1)：同步四处版本、写发布说明、生成草稿、核验安装包与 Tauri 更新签名，再公开并匿名回读。未验范围如实保留。
- macOS 发布先读 [本机 Mac 发布指南](docs/local-release-runbook.md) 与 [macOS gate](docs/macos-release.md)。Mac 的钥匙串、签名身份和 Apple 命令只在对应 Mac 环境适用，不能当 Windows 本机配置。
- 发布凭据留在既有安全存储/Actions secrets，不导出秘密。保留 commit、workflow run ID、输出路径和阶段回执；CI 通过或上传草稿均不等于发布完成。
