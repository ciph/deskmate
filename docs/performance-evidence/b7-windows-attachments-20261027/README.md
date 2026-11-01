# Windows B7：附件、工作日志与迁移恢复

2026-10-27，`E:\Codex\工作伙伴\deskmate-windows-b7`，分支 `codex/yume-performance-b6`，HEAD `db07e5bdb02550c4d165da8cd45fb76debefbe17`，YUME 0.4.21。历史 schema 3、记忆 schema 4 不变。**B7 仍进行中，未提交、推送、安装或发布。** [可视报告](index.html) · [结果](summary.json) · [回归口径](frontend-regression.json) · [本轮差异](changes-this-round.patch)。

## 本轮实际完成

| 范围 | 实际验证 | 方法与边界 |
| --- | --- | --- |
| 物理附件 | MP4、MP3、WAV、文件夹、坏 MP4 真实跨窗口拖放；文件夹展开两个子文件；坏视频显示不可预览，卡片可移除 | Windows MCP 1.3.24 固定 SHA、临时子进程；独立 WinForms `FileDrop` 合成源与真实鼠标，不是合成 DOM drop |
| 媒体 | 视频重播、6 次快速往返 seek、播放中移除；音频播放/暂停/seek/结束；MP4 原生选择器添加后完整播放 30 秒 | 音频为静音夹具，不能算扬声器听感验收；MP3/WAV seek 到尾部后结束，不能算全时长播放 |
| 发送与恢复 | 四个有效附件发送、历史重开后恢复；播放中切会话后 pause、媒体节点卸载 | 四个资源均为引用原路径，最终四条资源/消息关联；源文件仍存在且后续记录的 SHA 未变 |
| 二进制 | 普通 1/8/20 MiB 原始上传、Rust 读回 SHA；64 MiB WAV 实际落盘 SHA、WebView2 解码推进及 seek 后结束、清理副本 | 调用生产上传模块及原生 IPC 的后台探针，非真实文件选择器上传；耗时仅单次观察，不是 p95 或 release 性能结论 |
| 拒绝边界 | 空内容、大小不符、伪 PNG、JSON 数组、单项/累计 20 MiB+1、媒体 64 MiB+1、MIME/扩展名不符、设置窗口越权上传 | [二进制结果](raw/binary-results.json)、[非 chat 拒绝](raw/binary-settings-denied.json) |
| 日志五工具 | record/query/update/generate_report/schedule_report，真实侧车、宿主保存、物理审批及页面回执 | 本机合成 provider 强制选择工具，**不证明真实模型意图识别**；自然措辞中一次 update 被 `NEEDS_EXPLICIT_REQUEST` 拒绝，保留原记录；明确“修改工作日志”后成功 |
| 报告版本 | 人工正文保留、重新生成候选不覆盖当前版本、采用候选创建新人工快照、旧 revision 拒绝 | [正式 settings IPC 结果](raw/report-lifecycle.json)，不是完整物理编辑/导出验收 |
| 漏期周报 | 停止后设置合成漏期场景，仅生成 10/19–10/23 最新一期；再次重启仍同一 run、attempt=1 | [首次](raw/worklog-catchup-first.json)、[重启](raw/worklog-catchup-restart.json)；未改 OS 时间 |
| 原生迁移恢复 | 历史 v2 注入坏元数据导致 setup 拒绝启动；事务回滚，schema 2、原权威行逐字节未变；只移除注入行后原生迁移 v3，重启仍可读。合成记忆 v3→v4，中文导出保留 | [失败读回](raw/migration-native-failed.json)、[回滚/修复](raw/migration-rollback-and-repair.json)、[恢复](raw/migration-native-recovered.json)、[再次重启](raw/migration-native-restart.json)。该受控 setup abort 不是 Tao 绘制崩溃；未宣称自动修复真实坏数据或原生磁盘满验收 |

## 找到的问题与修复

1. QA provider 把工作日志自定义工具参数直接放在 arguments 中，侧车实际契约要求 `{input: ...}`。仅对五个工作日志工具补包装，保留原生 read/edit 参数。JSON/SSE 的五工具回归覆盖 10 组组合，provider 测试 5 pass / 47 assertions。第一次现场失败未写入条目。
2. `useWorklogChat` 收到其他日志变更时，查询不存在的操作回执会把已拒绝状态清成 pending；同一操作由 pending 变 rejected 也可能复用旧错误状态。现在空回执不覆盖既有状态，后到的工具拒绝更新错误。红测复现后转绿，真实鼠标打开会话、Enter 续聊后，实际宿主变更仍保留拒绝原因。
3. 重开历史只恢复正文，日志回执要下一次 idle 才出现。现在会话变化后主动恢复已有工具回执，仍拒绝旧会话迟到结果。新增红测转绿；最终原生构建以后台 DOM 激活历史后，无需发送新消息即显示五张回执，再次迁移重启后也通过。该最后一步不是物理鼠标输入。[最终原生截图](raw/receipt-history-final.png)

修复源码只涉及 `src/chat/useWorklogChat.ts`；其测试、`chatPetActivity` 测试桩和 QA provider/测试同步。启动多一次回执消息读取的成本需纳入最终 1000 消息性能复核，本轮没有重新声称流式性能预算通过。

## 回归、失败记录与构建

- 原回执修复的完整前端运行 172/172 文件通过。加入历史恢复后全套为 1364 pass / 3 fail / 8 skip、171/172 文件；三项失败来自旧测试桩把启动读取误当成发送确认的延迟读取。桩改为仅在已提交消息后延迟，受影响文件重跑 **28 pass / 0 fail**。最终组合覆盖 **1367 pass / 0 fail / 8 skip，172 文件**，不是另一遍全绿全套。[完整失败输出](raw/frontend-final.log) · [受影响复跑](raw/receipt-startup-pet-regression.log)
- 日志定向 **24 pass / 69 assertions**；Rust 迁移定向 **32 pass / 0 fail**；三套类型检查、前端构建、隔离 QA 构建与默认身份 debug/no-bundle 构建通过。Rust 全套本轮未重跑，前轮 974 pass / 24 ignored 仅作前轮记录。
- React Doctor changed scope：13 文件、81 分、无新增报告；不宣称全仓无历史问题。
- 报告采用版本的第一版探针错误要求 currentVersionId 等于候选 ID；现有契约会复制为新人工快照。原失败 JSON 保留，按正文、来源、旧人工版本保留和冲突拒绝复测通过，没有为测试修改产品行为。
- QA stderr 有一次可恢复审批 SSE 正文解码错误，后续实际审批成功；未声称零断流。预期坏库 setup abort 与早于调试端口就绪的探针连接失败单独记录，不混作 Tao 回归。
- 最终 QA SHA：`9894BC03A999CE816386B9C00E4C9676F03A9F33F69AB9487B578F2B86BC2763`。最初附件物理阶段使用 `375C1216945E21C5E00709E9AB64B2B9E4FA8BD16775CEE52D20F1DF081C0039`；回执第一次物理复测为 `56D8B4F3315792F175FC50E2AF2EF2C123B28712CF4A3332AB391684A6CFEAFB`。
- 已恢复正常 EXE，SHA `BD8679909680F40E8E2D9615219476B51DAD0BE204F1F0A7853D8A7628B263B4`，未运行生产身份。以上均为 debug/no-bundle，**release 附件矩阵仍待验**。

## 隔离、保留与收尾

全部运行位于本轮新建身份 `com.deskmate.unifiedhistoryqa.winb7attachments20261027a`，只使用合成文件/日志/本机 provider。6 个 SQLite 完整性检查通过；临时媒体副本删除；虚拟 provider 凭据通过设置接口移除；精确 PID/创建时间/路径核验后停止 QA/sidecar/provider/拖放源与临时 MCP。两个 QA 数据根、三个端口已清理。[完整性](raw/final-integrity.json) · [清理](raw/cleanup.json)

原有 579 个已修改/未跟踪文件先行快照；除本轮列明的源码、测试和进度入口外保留原字节。[保留核验](preservation.json) · [前次报告清单核验](frozen-reports.json) · [源码指纹](source-fingerprints.json)。旧 E 主仓和 C 工作树未操作。原生选择器截图含系统快速访问名称，仅留原始诊断目录，不纳入便携证据；桥接会话 token、数据库和大媒体夹具不复制到这里。

## 剩余工作与轮次估计

Windows 预计还需 **2–3 个集中工作轮次**；若继续发现阻断问题，按 **3–4 轮**准备。这个估计是工作包数量，不是完成百分比或固定工期：

1. 附件 release 矩阵、剩余格式/取消与生命周期；工作日志完整物理编辑、版本、复制导出、删除、失败重试/取消；真实模型意图场景。
2. 活跃工具中二次启动、取消 Aborted 文案、余下恢复场景，PDF 堆/RSS、硬件与生产基线对照；含新增历史回执读取的性能复核。
3. 视前两轮结果完成一次最终全套回归、证据审计及交付门禁。Mac 专有 GUI/硬件/性能验收另计，不能由本 Windows 结果关闭。

下一轮从上述缺口继续，保留当前全部改动。不要重做已经通过的测试来替代仍缺的物理表面、release 和性能证据。
