# Windows 性能整改状态与接续入口

核对日期：2026-10-29。原 Mac → Windows 交接已完成，本页集中当前状态；[原交接记录](archive/performance/windows-handoff-20261027.md) 只供溯源。

## 当前交付

- 工作目录：`E:\Codex\工作伙伴\deskmate-windows-b7`；分支 `codex/yume-performance-b6`；产品 0.4.24，历史 schema 3、记忆 schema 4。先核对实际 Git 状态，保留旧主仓及其他工作树。
- B6/B7 修复已随 [Windows v0.4.22](https://github.com/ciph/deskmate/releases/tag/v0.4.22) 发布。发布源码 `1901fb8` 包含保存提交 `8702347`、修复 `36a3c2e` 和交付证据 `2e90c94`；[发布回执](performance-evidence/release-v0.4.22-windows/README.md) 记录六项 CI、签名、内容和公开下载校验。
- 长历史 B0–B4、G1–G5 已完成：正文/导航虚拟化、Markdown Worker、回执精确读取及接续保护随 [Windows v0.4.24](https://github.com/ciph/deskmate/releases/tag/v0.4.24) 发布，源码 `b9c5606` 完整保留四个交付提交；[本次发布回执](performance-evidence/release-v0.4.24-windows/README.md) 记录六项 CI、安装资产核验与匿名回读。v0.4.23 门禁失败的标签保留，报告编辑器异步断言修复后定向 7 项及最终发布门禁通过。
- 原 20 项审计中 19 项代码处置已落实，P14 模型实验保留原版、默认 60 fps。[逐项矩阵](performance-evidence/b7-windows-closeout-20261028/audit-matrix.json) 与 [B7 报告](performance-evidence/b7-windows-closeout-20261028/README.md) 记录实际修复、测试及取舍；历史失败和原始证据继续保留。
- B7 当时本机最终前端 1373 pass / 8 skip、Rust 974 pass / 24 ignored，0 fail。已覆盖长流式 Stop/焦点、原生附件与日志、工具运行中二次启动、Tao/SSE、受控迁移失败恢复、PDF 内存、宠物隐藏恢复、同 DPI 多屏与十分钟空闲；不是最终安装包的全部实机验收。

## 保留的边界

最终安装包真实安装/跨版本 GUI 自动更新；Mac 实机 GUI/恢复；混合 DPI/高刷新；GPU/功耗生产前后对照；真实模型日志语义；磁盘实际耗尽/任意进程中断迁移/完整修复 UI；原生全部动作极值和长期零泄漏仍未建立完整证据。详细范围以 [发布说明](release-v0.4.24.md) 和 [当前待办](verification-backlog.md) 为准。

长历史全挂载、界面线程全文 Markdown、回执重复全会话读取三条链已完成[独立施工与验收](performance-evidence/long-history-closeout-20261029/README.md)。首次正文仍完整读取，Worker 内仍执行全文语义解析；R06 全快照比较、D20 一次全库读取及 M19c 硬删除外键扫描仍是记录中的剩余成本，不自动扩成下一轮全套验收或默认施工授权。

## 后续工作方式

每次先核对 checkout、HEAD、工作区、远端及版本，保留已有改动；只有需要同步时才 fetch/快进，分叉先调查，不强制重置。不要重做已交付的 B6/B7 或沿用历史“2–3 轮”估计。

固定环境和生成链见 [工作台构建](workbench-build.md) 与 [脚本入口](../scripts/README.md)：Bun 1.3.14、Rust 1.94.1、OpenCode 1.18.21，固定上游 commit `826d9ad46a22bef0294998e08daa3c4904fea28f`。生成目录清理后先按构建契约准备工作台和 sidecar；普通前端 build 不能替代完整工作台准备。

需要新的原生验证时使用新的隔离身份、输出目录和合成数据，按 PID/出生时间/路径清理自己的进程；不复制日常凭据或直接修改 OpenCode 数据库。数据库回退不等于程序回退，不让旧二进制写入新版库。仅在改动或证据缺口需要时执行对应验证。

## 证据入口

[发布回执](performance-evidence/release-v0.4.24-windows/README.md) · [长历史交付](performance-evidence/long-history-closeout-20261029/README.md) · [B7 交付](performance-evidence/b7-windows-closeout-20261028/README.md) · [各轮原件](performance-evidence/README.md) · [原施工方案与门槛](performance-remediation-plan.md) · [Mac 历史施工](performance-remediation-handoff.md)
