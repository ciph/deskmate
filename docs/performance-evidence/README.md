# 性能整改便携证据

## 2026-10-28 Windows release 附件与日志检查点

B7 仍进行中，当前 E:\Codex\工作伙伴\deskmate-windows-b7 / codex/yume-performance-b6 / db07e5b / 0.4.21。复用经 SHA 核验的隔离 release QA：原生 MP4 选择器、MP3/WAV/文件夹双区域拖放、坏视频回退、发送与重启后历史恢复通过；日志记录、冲突草稿、日报周报、版本采用、复制与导出、计划编辑/暂停、失败后真实重试、关联删除和浅深色固定窗口均有物理证据。构造的最近漏周重启后运行中暂停并取消，服务延迟期过后未归档；不宣称客户端收到了迟到响应。十次普通二次启动通过，活跃工具期间仍待验。本轮无产品源码修改，835 项原有文件内容保留；正常 release 已校验恢复。

六库完整性检查均 ok，但随后发现宿主快照后新建的侧车，不能将检查称为所有进程停止后；该侧车已按父进程/出生时间/路径核验停止，隔离根和临时端口清理完成。详见 [本轮证据](b7-windows-release-acceptance-20261028/README.md) 与 [结果](b7-windows-release-acceptance-20261028/summary.json)。继续活跃工具二次启动、Aborted 文案、其余恢复/迁移、PDF 堆/RSS、硬件/生产对照、历史回执恢复新增读取的性能和最终完整门禁；真实模型意图和 Mac 单列。Windows 估计尚需 2–3 轮，出现新阻断为 3–4 轮；未提交、推送、安装或发布。以下历史记录保留当时状态。


2026-10-27 Windows 最新：[release 阶段 HTML](b7-windows-release-worklog-20261027/index.html) · [方法、系统阻断与余项](b7-windows-release-worklog-20261027/README.md) · [结果](b7-windows-release-worklog-20261027/summary.json)。优化 release 已完成 22 MB MP4 正文真实拖放、完整 30 秒播放、连续进度手势和播放中移除；1/8/20 MiB SHA 回读、64 MiB 媒体解码/清理与非法边界通过。本轮一次完整前端回归 1367 pass / 0 fail / 8 skip、172/172 文件，六库完整性与隔离清理完成，正常 release 恢复。Windows 安全桌面中断后续真实输入：release 原生选择器/其余媒体矩阵及完整工作日志物理表面未完成，不用前轮 debug 或后台 IPC 替代。既有改动保留，本轮无产品源码修改。B7 仍进行中，Windows 仍估 2–3 轮，新增阻断则 3–4 轮；Mac 专有项另计，未提交/推送。

2026-10-27 Windows 前次：[附件/日志/迁移 HTML](b7-windows-attachments-20261027/index.html) · [方法与边界](b7-windows-attachments-20261027/README.md) · [结果](b7-windows-attachments-20261027/summary.json)。MP4/MP3/WAV/文件夹与坏视频物理拖放、MP4 原生选择器及播放、1/8/20 MiB 二进制与 64 MiB 媒体原生 IPC 通过；工作日志五工具、报告版本和漏期补偿重启去重通过。修复回执拒绝被空查询覆盖及历史重开不恢复回执，并修正 QA provider 参数包装。历史旧库受控失败完整回滚、修复夹具后原生升级/重启及中文记忆导出通过。最终前端组合覆盖 1367 pass / 8 skip（全套后修正测试桩并复跑受影响 28 项），Rust 迁移定向 32 pass，类型与正常构建通过，6 库完整性和隔离清理完成。B7 仍进行中，Windows 预计再需 2–3 轮，新增阻断则 3–4 轮；Mac 专有项另计，未提交/推送。

2026-10-27 Windows 前次：[Tao / SSE HTML](b7-windows-tao-sse-20261027/index.html) · [完整证据](b7-windows-tao-sse-20261027/README.md) · [结果](b7-windows-tao-sse-20261027/summary.json)。Tao 绘制重入已复现并修复，原生回归 25 次通过；审批 SSE 响应头等待与正文空闲期限分离，真实故障注入从旧版 40 秒未就绪恢复到约 4.1 秒重连，旧连接已关闭。物理审批取消/允许一次、缩放/截图、最小化恢复及桌宠重开通过；Rust 974 pass / 24 ignored，正常构建恢复。B7 仍进行中，预计 Windows 还需 3–4 轮，Mac 专有项另计，未提交/推送。

2026-10-27 Windows 前次：[原生 GUI HTML](b7-windows-native-gui-20261027/index.html) · [方法与边界](b7-windows-native-gui-20261027/README.md) · [结果](b7-windows-native-gui-20261027/summary.json)。已补用户授权的临时 Windows MCP 原生截图/真实键鼠，修复 Enter 提交时 disabled 导致的失焦（改 readOnly）。首次 1000 消息 10k、Stop/续聊、TXT/PDF/WAV 原生选择/发送/重开、工作台接管/释放和历史菜单操作通过；前端 1361 pass / 8 skip，类型/正常构建通过。早期 Tao 绘制崩溃根因、取消 Aborted 文案、审批 SSE 与其他剩余验收保留，B7 进行中。QA 清理、正常 dist/EXE 恢复，未提交/推送。

2026-10-27 Windows 前次：[冷启动历史修复 HTML](b7-windows-cold-history-20261027/index.html) · [方法与边界](b7-windows-cold-history-20261027/README.md) · [结果](b7-windows-cold-history-20261027/summary.json)。冷启动误超时、早期 state 竞态与历史面板初始化关闭已修复，最终全新身份冷 66.16 秒/热 2.03 秒通过；真实多窗口目录/所有权与 UIA 搜索通过，10k/Stop/续聊回归通过。前端 1360 pass / 8 skip，Rust 970 pass / 24 ignored；物理 GUI/视觉、迁移、硬件等边界保留，B7 进行中，未提交/推送。

2026-10-27 Windows 前次：[Stop/焦点修复 HTML](b7-windows-stop-focus-20261027/index.html) · [方法与边界](b7-windows-stop-focus-20261027/README.md) · [结果](b7-windows-stop-focus-20261027/summary.json)。真实目录事件复现并修复本地流式 composer 卸载；新 1000 消息首轮 10k/50k、Stop/继续及重启另一首轮通过，新增历史删除/记忆导出/忘记/重启链路，最终前端 1358 pass / 8 skip。冷启动历史发现超时与 GUI/硬件等缺口继续保留，B7 进行中，未提交/推送。见 [Windows 接管入口](../performance-windows-handoff.md)。

Windows 首批：[HTML](b7-windows-20261027/index.html) · [方法与边界](b7-windows-20261027/README.md) · [结果](b7-windows-20261027/summary.json)。启动所有权/孤儿、真实引擎恢复、WebView2 PDF 32 次及首轮 10k 原始失败保持当时记录；以上修复报告为后续证据，不改写原采样结论。下方历史记录也保持其采样时状态。

Mac 最新：[2026-10-27 B7 流式与 PDF HTML](b7-macos-20261027/index.html) · [结果/日志 SHA](b7-macos-20261027/summary.json)。修复原生 text delta，新增 1000 消息 50 Hz 的 10k/50k、Stop/继续和 PDF worker 32 次循环；前端 1359 pass。该 Mac 轮次仍有 GUI 审批/SSE/日志、内存与硬件边界，Windows 新增结果见上方。

上一轮：[2026-10-26–27 B7 接续 HTML](b7-macos-20261026/index.html) · [结果/日志 SHA](b7-macos-20261026/summary.json)。新增普通附件历史标签与导航修复、三种媒体/原始上传和 1000 消息非流式采样；该轮提出的流式与 PDF 循环已由最新记录补充。下方旧记录保持其当时范围。

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
