# B7 Windows 接管与首批验收（2026-10-27）

后续：[Stop/焦点修复及存储验收](../b7-windows-stop-focus-20261027/README.md) 已补真实事件复现与新 1000 消息复测。本页保持首批原始失败、观察和当时结论，不以新结果覆盖旧样本。

[HTML 报告](index.html) · [完整结果](summary.json) · [源码指纹](source-fingerprints.json) · [原始日志校验值](raw-log-manifest.json)

**Windows 本机基线通过，B7 仍进行中。** 从 `ciph/deskmate` 的 `codex/yume-performance-b6`、`db07e5bdb02550c4d165da8cd45fb76debefbe17` 接续。新独立 clone 为 `E:\Codex\工作伙伴\deskmate-windows-b7`；原始日志为 `E:\Codex\工作伙伴\reports\yume-b7-windows-20261027`。产品仍为 0.4.21，历史 schema 3、记忆 schema 4；本批仅保存文档与证据，没有修改产品代码、锁文件或版本，没有提交、推送、安装或发布。

旧主仓 `E:\Codex\工作伙伴\deskmate-public-release` 的未提交状态按相同 `git status --porcelain=v1 --untracked-files=all` 命令复核一致。原先指定的 C 盘工作树实际为干净的 0.4.21 / `codex/yume-performance-remediation` / `fcb4635`，而非旧指引描述的 0.4.18。本轮未在二者写入、stash、reset、clean 或切换分支。[复核回执](preservation-check.json)是状态比对，不冒充每个原文件的前后字节校验。

## 本机基线

Windows 10 Pro 19045；Ryzen 7 7700，RTX 5060；2560×1440 / 60 Hz，QA WebView2 的 devicePixelRatio 为 1.25；WebView2 155.0.4283.62。Bun 1.3.14、Node 24.14.1，`stable` 别名实际为 rustc/cargo 1.94.1，执行前检查精确版本。没有为使用别名升级全局 Rust。

| 检查 | 本轮结果 |
| --- | --- |
| 固定依赖、sidecar | frozen install；OpenCode 1.18.21 / ncmdump 1.5.1 |
| 完整工作台 | 独立上游 `826d9ad46a22bef0294998e08daa3c4904fea28f` 重建，锁 SHA `55cde1d8d6921ddd764ad99e6be8755767ca81e10f439e047014b7aaacffae5c` 安装前后相同 |
| 类型检查 | 三套 TypeScript 配置通过 |
| 前端 | 172 文件，1353 pass / 0 fail / 8 skip / 0 unfinished；8 项为平台跳过 |
| Rust | 966 pass / 0 fail / 24 ignored；不把 ignored 计为通过 |
| 10k retrieval 样本 | p50 16 ms、p95 23 ms、max 26 ms；无同机旧版本对照 |
| 前端生产构建 / Cargo 生产检查 | 通过；既有 chunk/编译警告保留 |
| 正常配置 Windows EXE | debug、无 QA feature、no-bundle 编译通过；未运行生产身份或安装 |
| 隔离 QA EXE | `worklog-qa` debug，独立身份与合成 provider；本地生产协议/资源，额外 QA JS 入口与 CDP |

测试、工作台构建、打包与采样顺序执行。不能把 QA 数字与 Mac、发行包或无调试器生产性能直接比较；机器曾观察到非本任务的编译进程，未干预，也未建立整机空闲性能基线。

首次 Rust 为 962 pass / 4 fail：三项 permission 夹具受 Git 目录内 TEMP 影响，将 TEMP 移到仓库外的本轮报告目录后 19 项定向通过；一个 sidecar health readiness 超时在定向与完整复跑均通过，但首次超时原因未完全证明。随后完整 966 pass，无测试断言或产品代码改动。Bun 的权限/临时目录、一次上游字体包解压失败，以及最后恢复 dist 时的 esbuild `spawn EPERM` 均留首次日志；使用可运行的执行上下文后对应步骤通过，没有修改锁文件规避。日志及退出码见 manifest。

## 启动所有权与真实引擎

使用身份 `com.deskmate.unifiedhistoryqa.winb720261027a`。当前源代码的 PrimaryGuard、Windows orphan cleanup 和 record_child 路径在 QA feature 下仍执行；旧 G1 文字中“QA 排除 cleanup”不适用于当前源码。QA 禁用自动启动/全局快捷键并隔离凭据，仍不等于生产身份运行验收。

- 三个近同时启动只保留一个宿主；空闲时十次二次启动全部退出 0，原宿主和引擎 PID、出生时间均保持。
- 60 秒、3000 块、计划 50 Hz 的 50k 回复期间，再执行十次二次启动，原宿主和引擎身份保持，正文最终 50,000 字符，SHA 与 provider 相同。**没有覆盖活跃工具任务中的并发启动。**
- 精确终止登记的宿主后，确认旧引擎仍存活；重启新宿主后旧引擎被回收，新引擎成功登记。没有按进程名批量清理。
- 真实捆绑 OpenCode + localhost 合成服务的 `live_audit_remediation` 通过：恢复确认不重放、归档故障保持 31,246 ms、允许工具执行并完成、拒绝/取消清除 ownership、后续任务可运行。provider initial 请求 3，recovery initial 仅 1。[原始测试日志](recovery-native.log)、[请求计数](recovery-native-provider.json)、[清理](recovery-native-cleanup.json)。这是 Rust 宿主路径验收，不是 GUI 点选审批验收。

## WebView2 PDF

生产附件模块、本地 PDF.js worker 和实际 WebView2/CSP 下，正常、损坏、超预算、取消交替 32 次，各 8 次。正常提取 808 字符；损坏为 InvalidPDFException；超预算为 160,000 字符正文加 28 字符提示；5 ms 取消为 AbortError。worker 创建 32 / 终止 32，峰值 1，每轮结束活动数 0。[逐次结果](pdf-evidence.json)。

该 WebView2 有 ReadableStream 异步迭代器。heap 两次读数为 4,576,561 → 7,614,933 bytes，包含模块初始化、探针与尚未回收数据；没有 GC/RSS 时序或收敛实验，**不据此宣称内存收敛，也不能仅凭两点判定泄漏**。不是原生文件选择、drop、发送与历史重开全流程，复杂/XFA/扫描 PDF 仍未覆盖。

## 流式、Stop 与长会话

通过仓库已有 CDP 驱动调用生产 composer 与 DOM 探针。捆绑真实 OpenCode，模型输出来自 localhost 合成服务，无真实模型或付费调用。两份独立 500 轮 / 1000 消息夹具通过官方 `opencode import` 导入，导入时 QA 进程已退出。

| 场景 | 10k 首轮 | 50k | 10k 定向复查 |
| --- | --- | --- | --- |
| 初始 / 最终行 | 1000 / 1002 | 1000 / 1002 | 1002 / 1004 |
| 原有节点保持 | 1000 | 1000 | 1002 |
| DOM 正文变化 | 183 | 179 | 185 |
| 输入 / 滚动 p95 | **不采用** | 42 / 43.2 ms | 33.3 / 37 ms |
| 30 个样本中的 Stop 可见 | 0 | 30 | 30 |
| 焦点保持 / 草稿恢复 | 否 / 是 | 是 / 是 | 是 / 是 |
| native 全文 SHA | 正确 | 正确 | 正确 |
| 探针捕获终态代码块 | 未完成 | 逐字正确 | 逐字正确 |
| 结论 | **异常，未通过** | 隔离 QA 通过 | 复查通过，不能替代首次场景 |

**首轮 10k 待定位：** 真实流式仍在更新，但探针所见 Stop 不存在、输入焦点丢失；探针仅按 Stop 判断是否完成，因而提前读取了尚未完整渲染的代码块（9815 字符）。native 全文已正确落盘。稳定 1500 ms 后，在同一 1002 消息会话复查，30 个样本均处于流式，composer 保持、9900 字符代码块和 10k 全文 SHA 均正确。首次没有采集 composer/ownership 诊断时序，不能证明是 UI 竞态、探针时机或其他原因，也没有证明修复；**必须在新的冷启动/新导入 1000 消息会话补抓状态并复现**。原始失败和复查均保留：[首次](long-10000.json)、[复查](long-10000-recheck.json)、[50k](long-50000.json)。

延迟为程序化 input/scroll 到第二个 rAF，5 对预热后保留 30 对，每对间隔 200 ms；CDP 连接存在，未打开 Inspector UI。不是物理按键到显示、FPS、GPU 或生产前后对照。全部历史行仍挂载。`firstAt` 不是首 token 指标。50k/复查合格不能覆盖首轮 10k 异常。

Stop 场景真实中断 provider，native 保留 16,300 字符；随后正常收到 `B7_LOCAL_REPLY_OK`，无残留 busy。临时 UI「出错了: Aborted」仍是现有行为，没有为本批改变文案。[Stop](stop.json)、[继续](resume.json)、[native 校验](native-verification.json)、[provider 时间戳](provider-requests.json)。

## 工具限制与清理

Windows Sky 原生捕获三次分别出现 FrameArrived/window capture timeout，包括交互上下文重启后的尝试。本轮没有视觉截图通过记录，CDP 程序化检查不能代替真实 GUI、输入、系统 picker、跨屏或硬件验收。一次长会话探针误用“历史记录”标签，读取实际“历史”后纠正；该空跑不计产品失败或性能样本。

已通过应用 settings 事务删除唯一合成 provider 及虚拟凭据，关闭并逐个核验登记 PID/出生时间/路径；provider 通过本次工具会话 Ctrl-C 结束（Windows 返回 1），两个端口确认关闭。五个应用 SQLite integrity_check 均 ok，校验后仅删除本轮专属且无重解析点的 Roaming/Local 根。标准 `bun run build` 已恢复，dist 仅 chat/pet/settings 三个 HTML，无 b7-qa.js。[清理](cleanup.json)、[数据库](sqlite-integrity.json)、[标准 dist](16b-restore-production-build.json)。target/debug/yume.exe 最后产物仍是 QA 编译件，不能当作正式候选；正常配置构建的哈希见独立回执。

`inputs/` 是本轮实际脚本/配置的冻结副本，包含当次绝对路径、身份、端口与 target ID，仅供复核。重跑须新建身份、输出目录与进程回执，重新获取 target，不直接照搬清理脚本；数据库、完整缓存和构建产物未提交到便携证据。日志原件留在本机报告目录。

## 接续优先级

1. 定位上述首次 10k composer/焦点异常；捕获冷启动到首轮的 catalog ownership、SSE、composer 节点与 native 终态，保留原失败，修复后重跑相同 1000 消息场景。
2. Windows 原生 picker/drop、媒体/二进制发送与历史重开；GUI 审批/拒绝/取消、SSE 断流和引擎恢复、日志五工具/到期、历史删除/多窗口、记忆导出/忘记/重开。
3. 活跃工具任务二次启动、PDF 堆/RSS、2D/3D、隐藏/睡眠、跨屏 DPI/高刷、GPU/能耗及同环境生产对照。M19c 外键扫描/小库成本、C16 全行挂载、R06/D20 成本继续保留。

Mac 专有缺口仍按 [统一待办](../../verification-backlog.md) 保留；本批没有关闭 B7 退出门槛。
