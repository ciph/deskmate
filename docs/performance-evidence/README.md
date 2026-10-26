# 性能整改便携证据

最新：[2026-10-26–27 B7 接续 HTML](b7-macos-20261026/index.html) · [结果/日志 SHA](b7-macos-20261026/summary.json)。新增普通附件历史标签与导航修复、三种媒体/原始上传和 1000 消息非流式采样；B7 仍进行中，下一步补 50 Hz 流式与 PDF 循环。下方旧记录保持其当时范围。

新增 [2026-10-25 Mac B7 结果 HTML](b7-macos-20261025/index.html)、[摘要与证据 SHA](b7-macos-20261025/summary.json) 和 [代码指纹](b7-macos-20261025/source-fingerprints.json)。B7 进行中：本机全套与部分原生 QA 通过，Windows/完整原生性能待验；下方 Windows 历史原件保持不变。

2026-10-24 为 Mac 交接从 Windows 存档逐字节复制；没有重新执行这些测试或重新发布。当前状态及下一步见 [接续说明](../performance-remediation-handoff.md)，任务全文见 [施工计划](../performance-remediation-plan.md)。

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
