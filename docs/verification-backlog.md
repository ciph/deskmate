# 当前验收与待办

核对日期：2026-10-29，YUME 0.4.22。B6/B7 本机整改已经交付并随 Windows 发布；[当前状态](performance-windows-handoff.md)、[发布回执](performance-evidence/release-v0.4.22-windows/README.md) 和 [逐项审计](performance-evidence/b7-windows-closeout-20261028/audit-matrix.json) 集中维护完成结论。后续长历史施工使用独立记录，尚未升版或发布。下表是未验范围，不是自动开始下一轮施工的指令。

## 当前验证入口

按改动范围运行 `bun run typecheck`、`bun run test:frontend`、`bun run test:rust`、`bun run build` 或组合入口 `bun run check`。已通过且源码未变的不重复整套；原生 GUI、真实模型和硬件结果单独记录，发布工作流按目标版本执行。

## B7 与发布仍缺少的证据

| 范围 | 已有证据与剩余边界 |
| --- | --- |
| 最终安装与升级 | Windows 0.4.22 包/图标/签名/匿名下载已过；真实安装与跨版本 GUI 自动升级未实测 |
| Mac | CI 源码检查及未签名候选编译已过；最终实机 GUI/恢复、签名安装与更新仍未验，本次未发 Mac 包 |
| 显示与性能 | 同 125% DPI 多屏、当前版本输入/滚动/PDF 堆与 RSS/十分钟空闲已有记录；混合 DPI、高刷、受控 GPU/功耗前后对照仍缺少 |
| 真实模型 | 合成 provider 的审批/日志流程通过，不等于真实模型的日志意图与记忆语义准确率 |
| 故障与迁移 | 自动容量限制/事务故障、原生受控失败回滚、重开已验；磁盘实际耗尽、任意中断及完整修复 UI 未穷尽 |
| 渲染与生命周期 | 三类宠物隐藏恢复、PDF 32 次 worker 清理已验；全部动作极值和长期零泄漏未证明 |

保留原模型与默认 60 fps。长历史全挂载、界面线程全文 Markdown、回执重复全会话读取三条链的本轮实现与验收见[长历史施工交付记录](performance-evidence/long-history-closeout-20261029/README.md)。首次正文仍完整加载，Markdown Worker 仍执行全文解析；R06 全快照比较、D20 全库读取、M19c 硬删除扫描仍保留，见 [B7 交付报告的开发建议](performance-evidence/b7-windows-closeout-20261028/README.md#后续开发建议)。这些边界不自动转成无限验收轮次。

## 其他既有审计与平台待办


| 项目 | 尚需完成 / 证据入口 |
| --- | --- |
| 原审计 UI U1/U2 | 重连、busy Stop、idle 后 scheduled、多个 text parts 和迟到响应最小反例与修复；见[施工进度](audit-remediation-progress.md) |
| 工作台 W1/W2 | 自有源码直接表达行为、固定输入统一验证、重复准备与失败恢复；普通前端绿灯不关闭此项 |
| 历史 H1/H2/H3 | H2 按 key 查询、变化行写入与导入指纹已于 2026-10-10 完成源码/隔离测试；新正文投影退出及目标安装版性能仍待验。保留 legacy/local-only、墓碑及身份约束 |
| 综合 V1 | B7 已补隔离 Windows 审批、跨 30 秒归档失败时的控制推进、取消和活跃工具二次启动；完整 Mac、最终安装包及升级/回退仍按范围待验，不能把合成模型当真实语义验收 |
| CI / Windows | v0.4.22 两平台 Source checks 与发布流程已实际通过，见[发布回执](performance-evidence/release-v0.4.22-windows/README.md)；branch protection required checks 配置未在本轮核验 |
| 文件清理条件项 | 以[清理执行进度](repository-cleanup-progress.md)为准；Windows 0.4.22 包图标已验，外部作者资产、其余生成链与 Mac 实机条件不因本机清理而关闭 |
| Windows QA harness 旧边界 | `p5-windows-e2e.ts` 仍需给 MCP 回传 Notepad PID补新建进程/出生时间/可执行文件归属证明，并保证 setup/cleanup 首项失败不跳过其余关闭与临时根移除；本轮已改默认资源路径、平台预检与schema断言，不宣称此脚本已完整 Windows验收。工作记录 capture 新身份/PID/窗口渲染校验也需要实际 Windows 运行 |

## 从功能施工记录转移的未完成事项

| 范围 | 需要的后续验证 | 保留的原证据 |
| --- | --- | --- |
| 自动记忆/日志 | 36 条中英文真实模型语义样例：引用/自述、同义更正、去重、完成/取消、敏感负例与连续事项；确认当前日常应用完整交互。只用合成样例和隔离库，不把模拟模型联调当语义准确率 | [自动处理验收](automatic-memory-verification.md#真实模型验收与当前边界) |
| 附件原生事件/播放器 | Windows 的 MP4/MP3/WAV、文件夹拖放、播放跳转/移除/坏媒体和重开已有 [release 物理证据](performance-evidence/b7-windows-release-acceptance-20261028/README.md)；Mac、未覆盖 MIME 与长期压力仍待验 | [原生回归](attachment-native-regression.md#2026-09-28-本机记录) |
| 历史与 composer | B7 Windows 已补长流式、Stop/焦点、审批与历史重开；旧记录中的全部窗口尺寸/离线/模型组合仍需按场景核对，不能按一项通过整行关闭。360px CSS 压力规则仍没有 360px 原生截图 | [organizer 回执](archive/history/history-organizer-redesign-plan.md#8-施工记录2026-09-27)、[composer 回执](archive/chat/chat-composer-workspace-model-plan.md#10-实施与验收记录2026-09-27) |
| 设置与原生材质 | 原 P0 中剩余角色包/异常状态、跨窗口主题同步、全部页面语言组合；标题栏真实拖动、不同系统缩放。P1 材质统一/P2 原生 Vibrancy/Mica 应先核对后续样式接入，再确定尚未覆盖的范围和性能对照 | [P0/P1/P2 历史计划](archive/ui/frontend-redesign-phase-2-plan.md#后续验收边界)、[较晚原生检查](archive/ui/yume-latest-integration-review.md#后续macos-原生窗口检查) |
| 星运 | Windows 原生窗口及日期变更/恢复行为复验；已有本机持久化、四语言和四主题截图保留，不当 Windows 通过 | [首版验收](archive/horoscope/daily-horoscope-implementation-plan.md#12-首版实施验证记录2026-09-30) |
| 发布与更新 | v0.4.22 Windows 内容、图标、更新签名和匿名下载已核验；真实安装/隔离 A→B、活动任务等待、取消/失败恢复，以及后续 Mac 签名公证与实机链路仍按目标版本验证 | [macOS gate](macos-release.md)、[runtime checks](macos-runtime-checks.md) |

## 原生迁移能力余项

固定引擎能力矩阵仍保留：多题/多选/自定义 question 回答、未覆盖平台的原生拖放及更多 MIME、需要模型执行的 slash 命令、可核验 `/terminal` 面板、CLI/TUI 专有入口、非 Git 目录审查/撤销，以及同环境迁移前后的性能对照。不同版本升级/降级须用隔离旧/新包和数据副本验证；UI 回退不等于数据库格式降级。逐项状态见[能力矩阵](migrations/opencode-native/capabilities.md#8-p6-安装版能力终验状态2026-09-23)。

这些是 2026-09-23 记录留下的未验项，后续若已有独立证据，先对照版本、平台和场景再关闭。旧记录的“私钥密码未知 / 无签名 / sidecar 无鉴权 / 不自动重启 / 静态工作台记忆”已不是当前维护入口：当前签名能力按发布指南预检，当前上下文按[记忆契约](memory-and-worklog.md)及 D1-12 修订读取。

[早期 Agent 架构审计](archive/architecture/AGENT_HARNESS_ROADMAP_AUDIT.md)的目录/权限/停止/恢复与真实产物闭环要求，按当前 U/V 与原生能力项核对；性能改进仍需同环境前后测量。其 Desktop Context、通用触发器、可执行 Skills、多 Agent 等候选方向保留为历史建议，未因归档或移入索引转成已批准的施工待办。

兼容条件持续保留：旧格式 reader、纯旧文本、完整复合身份、local-only 回复、墓碑和不复活；不得以清理文档或完成某项门禁为由让旧二进制写新版数据库、覆盖新增数据、重放 prompt 或削弱批准边界。

## 完成回写

执行时记录当前 commit/未提交 diff、运行时、完整命令、退出码、通过/失败/skip/ignored、合成数据和证据路径；实机、模型、源码与公开资产分别记录。关闭一项需对应范围的真实新证据，不用旧测试数、截图存在或 archive 标记代替。

## 历史验收记录

[完整历史记录](archive/verification/history-through-20261028.md) 保留原基线、失败、测试数和未验条件。以下锚点保留旧契约链接的去向。

<a id="2026-10-24-mac-b6-h18a"></a>

[原记录：2026-10-24-mac-b6-h18a](archive/verification/history-through-20261028.md#2026-10-24-mac-b6-h18a)

<a id="2026-10-24-mac-b6-h18b"></a>

[原记录：2026-10-24-mac-b6-h18b](archive/verification/history-through-20261028.md#2026-10-24-mac-b6-h18b)

<a id="2026-10-25-mac-b6-h18c"></a>

[原记录：2026-10-25-mac-b6-h18c](archive/verification/history-through-20261028.md#2026-10-25-mac-b6-h18c)

<a id="2026-10-25-mac-b6-m19a"></a>

[原记录：2026-10-25-mac-b6-m19a](archive/verification/history-through-20261028.md#2026-10-25-mac-b6-m19a)

<a id="2026-10-25-mac-b6-m19b"></a>

[原记录：2026-10-25-mac-b6-m19b](archive/verification/history-through-20261028.md#2026-10-25-mac-b6-m19b)

<a id="2026-10-25-mac-b6-m19c"></a>

[原记录：2026-10-25-mac-b6-m19c](archive/verification/history-through-20261028.md#2026-10-25-mac-b6-m19c)

<a id="2026-10-25-mac-b7"></a>

[原记录：2026-10-25-mac-b7](archive/verification/history-through-20261028.md#2026-10-25-mac-b7)

<a id="2026-10-27-mac-b7-接续"></a>

[原记录：2026-10-27-mac-b7-接续](archive/verification/history-through-20261028.md#2026-10-27-mac-b7-接续)

<a id="2026-10-27-mac-b7-流式与-pdf"></a>

[原记录：2026-10-27-mac-b7-流式与-pdf](archive/verification/history-through-20261028.md#2026-10-27-mac-b7-流式与-pdf)
