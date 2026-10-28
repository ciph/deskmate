# 性能整改便携证据

最新交付为 [Windows 0.4.22 发布回执](release-v0.4.22-windows/README.md)，本机审计结果见 [B7 完整报告](b7-windows-closeout-20261028/README.md) 与 [20 项矩阵](b7-windows-closeout-20261028/audit-matrix.json)。当前待验统一见 [验收待办](../verification-backlog.md)，此页只维护证据索引。

## 各轮记录

以下报告、截图、日志、失败与 SHA 清单保留原字节和原基线。旧报告中的“未发布”“下一步”“尚需几轮”仅描述其形成时点；后续通过不改写原失败，也不把旧测试数当本轮复测。

| 日期 | 证据范围 |
| --- | --- |
| 2026-10-28 | [0.4.22 公开资产、CI、签名与下载回读](release-v0.4.22-windows/README.md) |
| 2026-10-28 | [本机交付、20 项处置、最终检查及限制](b7-windows-closeout-20261028/README.md) |
| 2026-10-28 | [release 附件与完整日志物理验收](b7-windows-release-acceptance-20261028/README.md) |
| 2026-10-27 | [release 媒体与当时的桌面中断](b7-windows-release-worklog-20261027/README.md) |
| 2026-10-27 | [附件、日志回执与受控迁移恢复](b7-windows-attachments-20261027/README.md) |
| 2026-10-27 | [Tao 绘制重入和审批 SSE 修复](b7-windows-tao-sse-20261027/README.md) |
| 2026-10-27 | [原生截图、键鼠与发送焦点](b7-windows-native-gui-20261027/README.md) |
| 2026-10-27 | [冷历史发现和多窗口](b7-windows-cold-history-20261027/README.md) |
| 2026-10-27 | [首次长流式 Stop/焦点](b7-windows-stop-focus-20261027/README.md) |
| 2026-10-27 | [Windows 首批基线与原失败](b7-windows-20261027/README.md) |
| 2026-10-27 | [Mac 流式与 PDF 生命周期](b7-macos-20261027/README.md) |
| 2026-10-26 | [Mac 附件与长历史接续](b7-macos-20261026/README.md) |
| 2026-10-25 | [Mac B7 初次本机检查](b7-macos-20261025/README.md) |

## 原始审计与 0.4.21 发布原件

2026-10-24 从 Windows 原存档按字节复制；不是本轮重测或重新发布。

| 文件 | 时间与用途 |
| --- | --- |
| [audit-findings.json](audit-findings.json) | 2026-10-22、0.4.20 / d92a9c8 原始 20 项审计；含 B6 的 #18/#19 原始依据；不是当前未修缺陷清单 |
| [audit-verification-notes.json](audit-verification-notes.json) | 同一原审计基线的测试范围、退出码及未测边界 |
| [release-summary.json](release-summary.json) | 0.4.21 / fee4d70 最终 Windows 公开发布摘要及未安装、未发 Mac 包声明 |
| [ci-verification.json](ci-verification.json) | 最终两平台测试数、CI run/job ID、固定工作台重建与原日志 SHA-256 |
| [public-verification.json](public-verification.json) | 最终公开状态、标签、匿名安装包下载、签名和 manifest 回读结果 |
| [provenance.json](provenance.json) | 上述五个原文件的 Windows 来源、字节数与 SHA-256，可验证搬运未改写 |

JSON 内的 `latest`、`sourceClean` 等字段描述记录时点（最终公开回读为北京时间 2026-10-23 01:06），不代表未来实时状态；行号对应原审计源码。原日志文件名及 Windows 路径仅用于溯源，原始大日志/HTML/截图/安装包未随本目录入库。日志哈希是原回执记载值，不代表日志已在 Mac 本地复验。

发布链接：[v0.4.21](https://github.com/ciph/deskmate/releases/tag/v0.4.21)；[最终发布 CI](https://github.com/ciph/deskmate/actions/runs/39813641175)；[main Source checks](https://github.com/ciph/deskmate/actions/runs/39813642512)。Actions 的日志和候选 artifact 有保留期限，需要复核时使用原存档或重新生成对应证据，不能把 artifact 链接当永久备份。

本目录 `.gitattributes` 对 JSON 禁用换行转换，使 Windows/Mac checkout 保持原字节和哈希。已复制原件保持不变；后续新结果保存新回执并标清基线，不覆盖历史失败、未验边界或将旧通过继承为新通过。
