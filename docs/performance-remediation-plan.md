# YUME 性能整改施工方案

创建：2026-10-22；交接核对：2026-10-24（Asia/Shanghai）。本文件是性能整改施工计划的唯一维护入口，原 Windows `.omo/plans/` 副本冻结为历史记录。

**当前（2026-10-25）：B1–B5 已进入远端 `main`，Windows v0.4.21 已发布；Mac 分支 `codex/yume-performance-b6` 的 H18a/H18b/H18c 代码与自动回归通过，历史目录 schema 3；M19 和 B7 待做，原生待验仍保留。** 下一步先读 [Mac 接续说明](performance-remediation-handoff.md)，核对本地 B6 检查点，再执行本计划第 10 节 M19a。不要切回旧 main 重做已有修改。

源码/发布基线：`fee4d707804f76e5c608ceef33c622e71d17b175`，版本 `0.4.21`。远端 `main` 在本次文档提交前已核验与该提交一致；此后从实际最新 `origin/main` 接续并检查新增差异。B5 最终 CI、公开下载和审计基线的便携回执见 [证据索引](performance-evidence/README.md)。

旧回执中的“下一批”“未推送”“未发布”、盘符路径与并行安排只描述当时状态；当前执行位置使用 Mac 的实际 clone，是否分派任务遵循接手会话规则。当前验收范围以 [验收待办](verification-backlog.md) 和接续说明为准。

目标：先修复会中断任务、阻断功能或破坏数据完整性的问题，再减少启动、流式聊天、后台采集、渲染和数据库的实际工作量。每项改动必须同时证明行为仍然正确、工作量下降和对应环境的实际表现；不以微测结果承诺整机提速，不以删除功能换取性能。

## 1. 已确认的基线与施工边界

- 接续入口：先核对实际 checkout、分支、工作区、HEAD、远端和 `package.json` / `Cargo.toml` 版本。当前发布源码基线为 `fee4d70` / 0.4.21；Mac 使用本机 clone，不复制 Windows 链接工作树的 `.git`。
- 原审计基线（历史）：`codex/optional-windows-component`，`d92a9c872b01300a27d90a675cfa9247bfd2fd2b`，0.4.20。B1–B5 施工分支为 `codex/yume-performance-remediation`，不是要求 Mac 回退的目标。
- Windows 开发路径（历史）：`C:/Users/chenglingxiao/.codex/worktrees/build-v0418-9081/deskmate-public-release`。旧 `E:/Codex/工作伙伴/deskmate-public-release` 是保留旧改动的元数据仓；不 reset/clean，不移动 `.git`。
- 本方案在仓库 `docs/performance-remediation-plan.md` 维护；原审计证据保持原样。每个小步更新 `docs/verification-backlog.md` 和实际受影响的现行契约，未验事项继续保留。
- 默认施工不自动发布、安装到日常环境、升级 OpenCode/React/Tauri 大版本、删除离线角色或引入新框架。此前 Windows 发布及本次交接提交推送已有明确授权；后续新发布按届时用户要求执行。
- 审计基线及其验证回执已保存至 [performance-evidence](performance-evidence/README.md)。全文后续出现的 `E:/.../reports/` 是原始大日志、截图及 HTML 的 Windows 历史存档位置，不是 Mac 命令输入，也不代表原始大文件已全部入库。

已有基线证据可复用：TypeScript 检查退出码 0；Rust 728 通过、15 ignored、退出码 0；前端 154 个文件、1117 通过、8 skip，全部子进程退出码 0、无 unfinished；前端构建退出码 0。前端外层 PowerShell 收尾挂起后按精确 PID 回收，不能把它记录为总命令退出码 0。构建只覆盖自有前端，复制了既有 public/workbench，未证明工作台完整重建、原生安装、GUI、GPU 或耗电结果。G0 不重复运行这套未改源码的完整基线。

必须守住的契约：完整会话身份 (sidecarId, canonical directory, sessionId)、宿主保存认证、窗口/目录授权、工具父消息与 call ID 校验、墓碑与不复活、权限结果只在真实提交后确认、关键状态先落盘再发布、原文件不删除、隐私真正擦除、独立数据库恢复、后台自动记忆/日志继续工作。原生 OpenCode 数据由 OpenCode 拥有，不直接改其数据库。

## 2. 已落定的性能取舍

1. 普通 AI 回复取消固定 2 秒展示门槛，首个可展示内容立即显示；固定人格台词继续保留原有节奏，测试分开。
2. rig2d 活跃 60 fps、idle/sleep 30 fps 是先做 A/B 的预算。动画按真实时间推进；30 fps 只有视觉通过才进入默认路径，未通过则保留已验帧率并记录功耗取舍。
3. GLB 贴图先生成 1024/512 候选，正式资源只在默认与最大展示尺寸、全部动作视觉验收后替换。三代角色继续离线可切换；不把删角色或远程下载作为本方案优化。
4. 先削减重复提交、重复扫描、重复编码；虚拟列表、Markdown 分块、KTX2、额外依赖只有本轮较小改动后仍有清晰瓶颈证据才做，不作为默认工作包。
5. 原方案以 Windows 原生验证先行；2026-10-24 起转交 Mac 接续，可先完成 macOS 对应场景。WebView2 与 WebKit、DPI/窗口和实际功耗各平台分别验收，任一平台证据都不能关闭另一平台项。

## 3. 八批顺序、门槛与并行安排

| 批次 | 任务 | 目的与依赖 | 可以并行 | 退出条件 |
| --- | --- | --- | --- | --- |
| B0 开工与先验 | G0、G1 | 核验入口/基线，准备隔离数据、生产路径原生场景、测量记录 | 只读依赖核验与合成数据准备 | 可复现命令和隔离边界明确；原生安全先验通过；开分支后再改代码 |
| B1 功能与数据阻断 | A01、A02、A03、A04 | #01–04 优先；各自依赖 G0/G1 的相关输入 | Rust 生命周期、日志桥、PDF、记忆导出可分属不同负责人 | 新的失败用例先复现再通过；原生 #01/#03 通过；无跨作用域和导出缺失 |
| B2 交互的小步改进 | C05、C10、C15、I07c | B1 已保存；用户授权先推进独立前端工作，B1 原生检查保留为候选交付门禁 | 导航、设置可独立；C05→C10 串行 | 首字即时、旧/重复事件隔离、导航稳定、隐藏设置无高频轮询 |
| B3 启动与后台工作量 | R06a、R06b、I07a、I07b、S08a、S08b、S09 | 依赖 B1；无变化不写、按目标读取、惰性工作台/3D、重启退避 | agent、worklog、宠物前端可分轨；lib.rs 串行接入 | 重复快照不写，空闲工作量下降；首开/恢复/重启合同通过 |
| B4 附件与桌宠 | F17、P11、P12、P13、P14a/P14b | F17 依赖 A03；P12 依赖 S08b；P13/P14 共享模型版本约定 | 附件、rig2d、GLB 资产三轨；PetApp.tsx 与原生接入顺序合并 | 大文件真实 IPC 验收；隐藏停机、动作不裁切；资产替换经视觉验收 |
| B5 较复杂的运行时改造 | R06c、R06d、C16、D20a/D20b | R06a/b 后再拆锁和 SSE；C05/C10/C15 后做流式合批 | agent、聊天、日志删除三轨 | 取消不被 HTTP 锁住，丢事件可恢复，最终文本一致，删除不漏快照 |
| B6 数据结构与迁移 | H18a/H18b/H18c、M19a/M19b/M19c | agent 持久化接口稳定后做 revision；A04 后做记忆库；迁移最后 | 历史库与记忆库可以分轨，各轨任务内部串行 | 隔离旧库迁移/失败恢复通过；隐私、墓碑、跨窗口并发不退化；规模预算达标 |
| B7 联合回归与交付 | V1、V2 | 所有已采纳任务完成；条件项作明确决策 | 静态审查可并行；测量/构建/套件集中排队 | 完整检查成功、Windows 场景完成、macOS 状态明确、逐项证据与回退记录齐全 |

不按天承诺工期；任务以小/中/大表达工作量。每个小步形成独立可回退差异；可保存本地检查点，但不自动推送。任何批次失败，后续可以继续无依赖的只读分析，不能把未通过的底层改动带入依赖任务。

### 共享文件的唯一写入顺序

- src-tauri/src/lib.rs：A01 → A02 所需宿主接线 → S08a → S09 → P12/F17 必要注册。由一位集成人合并；其他人提交接口说明或模块差异，不同时改。
- src/chat/ChatApp.tsx：C05 → C10 → C16。C15 主要改 ChatNavigation.tsx；若必须改 ChatApp，由聊天负责人统一处理。
- src-tauri/src/worklog/bridge_worker.rs：A02 完成后才做 I07a；认证与调度优化不能同一未验差异混改。
- src-tauri/src/memory/repository.rs、storage.rs、migrations/：A04 → M19a → M19b → M19c。一位负责人分步实施并升级版本；不由不同人并发写 SQL/触发器。
- src/pet/PetApp.tsx、GlbPetView.tsx：S08b → P12；src/pet/PetRenderer.ts 由同一负责人按 P12 → P13 → P14 接入，避免隐藏生命周期与包络改造相撞。P13/P14 统一模型与包络版本，再交同一集成人接入。
- src-tauri/src/agent/ 与 history/ 中的 generation 接口：R06 完成后冻结写入接口，H18a 才接；H18a 不凭扫描结果自创另一套真相来源。
- 所有执行者保留他人编辑，不以 reset/checkout 覆盖共享文件。前端全套测试、前端构建、完整工作台生成不得并跑；性能测量期间不跑测试/构建/其他基准，避免干扰。

## 4. B0：开工准备与原生先验

### G0｜固定工作入口与证据（小）

操作：重新检查实际路径、branch、status、HEAD、package.json/Cargo.toml；确认与接续说明中的最新基线及后续提交一致，不回退到原审计基线。若出现新改动，记录归属与范围，不覆盖，按变化重评相关任务。使用命令级 Git safe.directory 解决这台机器的所有权告警，不修改全局配置。创建施工分支后在当前机器建立独立阶段证据目录，记录绝对路径；只放去敏的测试数据、日志、截图、测量表和任务状态。原 Windows B0/B1 证据目录为 E:/Codex/工作伙伴/reports/yume-performance-remediation-20261022/，Mac 不照搬此路径。

验收：基线摘要、证据文件可读，diff 清单与当前分支明确；无需再次跑旧完整套件。记录 Windows/CPU/GPU/RAM、显示器 Hz/DPI、WebView2、Bun/Rust/Node 版本；每次测试写明 commit+未提交 diff、配置、样本数、冷热状态、命令、退出码、skip/ignored。

### G1｜最先验证 #01 的生产启动生命周期，并准备测量（中）

文件/输入：src-tauri/src/lib.rs、Cargo.lock 中固定的 tauri-plugin-single-instance 及本地依赖源码、src-tauri/src/workbench.rs、scripts/tauri.dev.conf.json、scripts/audit-remediation/verify-native.ts、scripts/agent-qa/、现有 workbench/worklog QA 入口。

先核实：单实例插件 2.4.3 的 Windows mutex 发生在 plugin.setup，调用 builder.plugin 只是注册。已存在 mutex 而注册窗口尚未被 FindWindowW 发现时，插件可能继续返回成功；因此“把 cleanup 放到注册之后”甚至“无条件放 app.setup”都不是所有权证明。列出 run→plugin setup→primary 判定→清理→sidecar spawn 的真实路径、竞态与失败语义，再由 A01 修复。

原生测试必须走相同的生产单实例/清理分支。现有 worklog-qa feature 会 cfg 掉 Windows cleanup，不能用它声称覆盖 #01。准备专属 app identifier、数据/配置/keyring namespace、受控 sleep/任务 sidecar 和 PID/出生时间证据；隔离配置不得绕过被验证的代码。旧 cleanup 只有名称/命令行匹配，可能误杀其他 YUME，因此旧实现的破坏性复现只在 Windows Sandbox/VM 或确认不存在日常同类进程的隔离主机里做；普通维护机不能直接运行这个场景。若无法获得安全生产路径环境，先做注入/并发测试，#01 原生验收仍标待验，不伪造通过。

场景：并发首次启动；首次已经有任务时再启动 10 次；宿主崩溃留下真孤儿；PID 被复用/归属不确定/其他应用同名进程；退出时正在重启。记录胜出宿主、子进程树、是否重复启动、清理对象与理由。

其余测量复用现有探针并补最小场景，不新建通用基准框架：10 分钟空闲；2D/3D 冷启动及首开工作台；1000 条消息流式 50 次/秒；1/8/20 MiB 普通附件与 64 MiB 媒体；1k/10k/100k 会话/记忆。短耗时测量预热后至少 30 次报告中位数/p95；冷启动至少 10 次先报告完整分布，样本不足不声称稳定 p95。真实功耗记录电源状态与屏幕亮度，相同环境前后对照。

Windows 脚本须显式设置 YUME_QA_OPENCODE_BINARY 为实际 opencode.exe 路径；verify-native.ts 的默认文件名不带 .exe，且它主要验证 Agent fixture，不等于 Tauri GUI 验收。执行前读脚本参数与隔离约束，不照抄历史命令。

## 5. B1：先修复四项明确问题

### A01｜二次启动与孤儿回收（#01，中；依赖 G1）

文件：src-tauri/src/lib.rs；必要时只抽取小的启动归属/孤儿判断模块；现有 pet_recovery_tests.rs、Agent recovery/lifecycle 测试及新增相邻启动测试。

步骤：按 G1 的实际插件生命周期证明 primary ownership；清理只由唯一胜出实例执行。以记录的宿主/sidecar PID、进程出生时间、应用身份验证归属；宿主确实失去活性才回收，证据不确定就保留。避免 PID 复用、mutex/window 注册间隙、同时启动、第二实例退出触发首实例清理。保持真实崩溃后的孤儿回收能力，不扩大名称匹配范围，不强杀无关进程。

QA：先用可注入进程身份/生命周期测试覆盖并发与不确定状态，再执行 G1 全矩阵。活跃首实例的 sidecar PID、任务输出、工具子进程保持连续；真孤儿被回收；并发启动最多一个受管引擎。回退仅撤回本任务代码，隔离 fixture 以精确 PID 关闭，不能全局 taskkill opencode。

### A02｜工作日志桥认证与可信目录（#02，小；依赖 G0）

文件：src-tauri/src/worklog/bridge_worker.rs、bridge.rs、lib.rs 的受管认证接口；关联自动记忆/历史中已有 scoped 请求构造；scripts/worklog/bridge.test.ts、full-flow.test.ts、worklog/bridge_dispatch_tests.rs、bridge_dispatch_query_tests.rs，以及相邻认证 HTTP 测试。

步骤：先追踪五个 worklog 工具→Launch/Grant→父消息请求的完整调用链；复用宿主受管认证。当前 Launch.directory 是随机 IPC 目录，绝非 workspace；Request/register_turn 没有 workspace，现有 grant 仅绑定 session/user message。因此必须把宿主可信且规范化的工作目录贯穿登记、grant 和父消息回查，不可拿 Launch.directory 当查询作用域。目录只能来自宿主登记的可信上下文/授权，不接受请求文件自报目录，核对目录、会话、assistant 父消息、tool/call ID。每次请求使用当前受管连接身份，重启后旧 endpoint/auth/launch 不得沿用；凭据不写请求/回执/前端/日志。

QA：强制 Basic auth 的本地合成服务覆盖正确/缺失/错误凭据；两个目录具有相同 session/message/call ID、错误工具 ID、错误父消息、伪造 grant、sidecar 重启后旧 launch 请求。五类工具贯穿文件请求→真实 Rust 宿主校验→提交→回执；只有正确作用域操作成功，拒绝不能产生写入。verify-worklog.ts 等合成宿主回执不能代替真实 Rust 链路。认证不允许降级关闭。A02 后 I07a 才改此 worker。

### A03｜PDF worker 与生命周期（#03，小；依赖 G0/G1）

文件：src/chat/attachments.ts、attachments.test.ts、useChatAttachments.test.ts；必要的 Vite/TypeScript 资源声明及 src-tauri/tauri.conf.json CSP 检查；实际改动涉及哪些配置先按当前 worker URL 输出确认，不扩大 CSP 白名单。

步骤：为当前 pdfjs 版本通过构建工具生成本地 worker URL，显式配置；保持动态加载。保留 getDocument 返回的 loadingTask，在成功、异常、取消、字符截断路径 finally 中销毁；不调用不存在的 PDFDocumentProxy.destroy。核验生产产物含 worker，应用协议 URL、worker 资源 MIME/CSP 与 WebView2 一致，沿用现有 worker-src self blob 约束、无需放宽 CSP，无 CDN 依赖。

QA：真实浏览器分支先复现缺配置；生产构建的 WebView2 打开正常、损坏、超 160k 字符 PDF，循环各场景至总计至少 20 次，文本和截断一致，worker 活动数回落，堆/RSS 在回收后收敛。保存资源请求和失败提示证据；Bun/Node 测试单独通过不算完成。macOS WebKit 同场景列 V2。

### A04｜记忆全量导出的 SQL 分批（#04，小；依赖 G0）

文件：src-tauri/src/memory/repository.rs、export.rs、repository_tests.rs；复用既有 400 个 ID 分批，不新建通用查询层。

QA：旧失败边界 32767 条和 50000 条，混合 active/expired/superseded、无/多 applicability、sources/task links；核对记录数、上下文归属、完整字段和稳定顺序。所有 ID 查询每批最多 400 参数；不因规模裁剪。保留 540/450 条既有回归。任务只影响导出，不能混入 M19 迁移。

## 6. B2：先降低可见等待与重复 UI 工作

### C05｜普通回复首字展示（#05，小）

文件：src/chat/ChatApp.tsx、chatFixedReply.test.tsx、chatPetActivity.test.tsx、useAgentRun.test.tsx 中相关生命周期场景。

步骤：普通 AI 路径移除 MIN_REPLY_PRESENTATION_MS 门槛，固定人格台词保留；调整明确保护旧普通回复等待行为的测试。100 ms 首字/300 ms 完成的合成回复应在首个展示机会出现，不等 2000 ms；以调度容差而非机器绝对帧数断言。

QA：Stop、切会话、卸载、首字前异常、工具先于正文、迟到事件不污染新会话；固定台词仍按原节奏。后续 C16 首块立即发布，不能合批又引入新的首字门槛。

### C10｜情绪事件去重和有界账本（#10，中；依赖 C05）

文件：src/chat/petActivity.ts、ChatApp.tsx、petActivity.test.ts、chatPetActivity.test.tsx；src/pet/gifState.ts、useGifState.ts、gifState.test.ts、useGifState.test.tsx。

步骤：同一请求仅在 mood 变化时广播；核对旧 mood 通道消费者后移除重复发送。把去重账本放到不会每事件拷贝整个 Set 的位置，保留请求身份、首次状态和终态；依据请求完成/切代清理，并留下拒绝迟到旧请求的边界，不能简单清空。

QA：单请求 10000 个连续 talking 只有一次状态广播；交错开始/完成、重复 UUID、迟到事件、切角色/会话不误唤醒；1000/5000/10000 重放记录操作数与耗时，增长接近线性；大量已终止请求后账本有明确上限/回收边界。三类角色共用 hook 均回归。

### C15｜稳定对话导航观察器（#15，小）

文件：src/chat/ChatNavigation.tsx、ChatNavigation.test.tsx；若必须调整父组件 props，由聊天负责人改 ChatApp.tsx。

QA：500 轮/1000 消息、50 次纯正文增量不重建观察器、不全量 observe；新增/删除轮次只调整差异节点。图片延迟加载、尺寸变化、展开折叠仍更新当前位置，跳转、键盘焦点/Escape/自动滚动正确；原生长会话记录 scroll 长任务与帧耗时。

### I07c｜设置更新状态的按需读取（#07 前端，小）

文件：src/settings/UpdateFooter.tsx、SettingsApp.tsx、UpdateFooter.test.tsx、updateState.test.ts。

QA：设置打开即读取；隐藏且无更新活动时不 500 ms 轮询；活动更新期间适当刷新，慢 IPC 同时在途最多 1；相同状态不生成无效重渲染。覆盖其他窗口开始更新、隐藏后重新打开、恢复可见、失败重试、卸载和迟到结果。优先复用现有更新事件，缺事件时保留低频兜底。

## 7. B3：启动与后台按实际需要工作

### R06a｜无变化 RunRecord 不落盘（#06 第一步，小）

文件：src-tauri/src/agent/record_store.rs、lifecycle.rs、collector.rs；collector_tests.rs、lifecycle_tests.rs、lifecycle_contract_tests.rs、lifecycle_recovery_tests.rs。

步骤/QA：在现有持久化边界比较有意义状态，无变化跳过序列化、sync_all、rename；不靠忽略错误实现。1000 消息/10000 个旧 run、连续 60 个相同快照新增记录写入为 0。变化状态仍先落盘再发 RAM/事件；模拟写失败、重启恢复、权限变化、pending outcome，不能丢关键状态。记录逻辑写入次数与文件 I/O。

### R06b｜按目标读取，并先将用量 HTTP 移出锁（#06 第二步，中；依赖 R06a）

文件：agent/collector.rs、lifecycle.rs、record_store.rs、history/archive.rs、ai_usage/local.rs；collector/race/reply 测试、历史 archive/recovery 测试和用量相邻测试。

步骤/QA：沿用现有按 key 读取能力，替换每 tick 扫全部 run 的路径；从当前完整快照取已有用量，不为同一完成消息再次 HTTP 拉全文。对确需补查的缺失数据保留显式补偿，先把这条用量 HTTP 工作移出 operation 锁，用受管连接代际和完整 run/workspace/session 身份校验结果；旧投影出锁留给 R06c，不合成一次大改。10000 个旧 run 不增加当前 run 的文件读取数；相同完成消息重放不重复计费/归档，用量接口阻塞时取消能获得控制锁，失败可恢复且陈旧结果不覆盖新状态。

### I07a｜工作日志文件桥通知与低频补扫（#07 宿主桥，中；依赖 A02）

文件：worklog/bridge_worker.rs、bridge.rs、对应 bridge/tool/full-flow 测试。

步骤：先检查已有 OS/依赖通知能力，选择最小可维护方案，不造事件框架；监听原子 request rename，唤醒消费，保留低频补扫/到期回收。目录或 launch 切换重建监听；退出可打断等待。新依赖只有现有能力不能满足跨平台且范围清楚时才引入。

QA：10 分钟无请求不再 10 Hz 枚举；通知丢失通过补扫仍消费，重复通知不重复提交；半写文件/响应原子性、到期清理、符号链接/reparse point 拒绝、重启切 launch、权限回执不回退。设补扫上限并验证最坏延迟，而非只验证名义频率。

### I07b｜按最近到期时间调度（#07 调度，中）

文件：worklog/runner.rs、scheduler.rs、runner_tests.rs、runner_configuration_tests.rs、runner_publication_tests.rs、calendar_tests.rs。

QA：无变化 next_due_at 不 UPDATE；等待到最近到期时间或配置变更/退出/恢复唤醒，保留适当低频校验。覆盖无规则、最近到期变更、时区/03:00 业务日、时钟前跳后跳、睡眠恢复、配置关闭、两次唤醒争领；同一 due 任务只执行一次，漏唤醒可补偿。统计 10 分钟 SQL 读写与唤醒次数。

### S08a｜首次打开才创建工作台（#08 宿主，中；依赖 A01）

文件：lib.rs、workbench.rs、workbench_visibility_tests.rs；scripts/workbench-overlay/workbench/entry.tsx、scripts/workbench-routing.test.ts、workbench-ownership.test.ts，必要时 workbench 适配测试。

QA：2D/3D 冷启动均无 workbench WebView；首次 show 创建一次，重复/并发 show 不重复创建。handoff 先保存、ready 后消费，打开不产生新消息/会话；创建失败可重试且不错误占有输入。隐藏后继续保留状态，关闭/隐藏/超时释放 ownership，轻聊与工作台不会同时发送。若改 overlay，按 docs/workbench-build.md 固定上游 ref/lock 完整重建，普通 build 不算工作台验证。

### S08b｜按角色加载 GLB 渲染链（#08 前端，小）

文件：src/pet/PetApp.tsx、GlbPetView.tsx；PetApp.reload.test.tsx、PetApp.glbMood.test.tsx、personaCatalog.test.ts、构建输出。

QA：GIF/rig2d 冷启动不请求/解析 Three/VRM 动态 chunk；2D→3D 显示加载/失败状态，快速切回 2D 后迟到加载不得挂载；首次动画、mood、sleep、导入角色均正确。记录 chunk 图、首帧与首次切换耗时，避免只看单个 chunk bytes。

### S09｜受管引擎失败退避与日志（#09，中；依赖 A01，S08a 接线后）

文件：lib.rs supervisor/spawn 路径及最小相邻可注入测试；agent/lifecycle_restart_tests.rs、lifecycle_recovery_tests.rs、windows_component_tests.rs。

步骤：连续失败指数退避并设上限/自动重试预算；健康检查持续稳定后才重置，不把 spawn 成功称可用。等待不占 recovery 锁，退出可打断；保留最近故障日志并有数量/体积边界，沿用现有恢复状态显示和显式重试入口。

QA：假时钟/注入启动器覆盖 spawn 失败、立即退出、启动慢、短暂健康后又崩溃、稳定恢复、手动重试、退出中等待；重试频率有界，组件重启等待与任务互斥不破坏，第一次故障日志仍可读且不包含凭据。原生受控坏配置验证无重启风暴。

## 8. B4：附件、帧率与资产

### F17｜浏览器附件二进制 IPC 与预览生命周期（#17，中；依赖 A03）

文件：src/chat/useChatAttachmentsUtils.ts、localResources.ts、attachmentApiBoundary.ts、attachmentApi.ts、attachments.ts；src-tauri/src/chat_attachments.rs、chat_attachments/types.rs、validation.rs、resources.rs、resources/store.rs 以及必要 command 注册。测试：attachmentApi/Boundary 相关测试、useChatAttachments、chatAttachmentFlow/Lifecycle/Send、localResources/useLocalResources、nativeResourceFlow、Rust chat_attachments/tests/ipc.rs、input_validation.rs、resources/tests.rs。

步骤：图片复用已验证 dataURL，避免重复解码/编码；浏览器 File 以 Tauri 原始二进制请求体传输，元数据独立校验并与正文长度/会话代际绑定。先做一个端到端普通附件，再接媒体副本；不要仅把 number[] 换 Uint8Array 后仍走 JSON。预览 Blob URL 统一在移除、切会话、完成/失败、卸载时 revoke。

QA：1/8/20 MiB 与边界+1、64 MiB 媒体副本与超限、空文件/伪 MIME/坏编码、取消、重试、迟到回应、配额、切目录。确认实际 IPC 字节不再是逐数 JSON；比较 WebView2 长任务、峰值 heap/RSS、编码次数。原生 picker/drop 的 opaque ID 引用路径保持原有生命周期；不能扩大任意路径权限，不能删除原文件。修改后按附件现行契约做原生选择/拖入/发送/重开历史。

### P11｜rig2d 帧率与合成路径（#11，中）

文件：src/pet/rig2d/renderer.ts、renderer.test.ts；参考 src/pet/frameGate.ts 现有门控、frameGate.test.ts。

QA：模拟 60/144/240 Hz RAF，活跃绘制每秒最多约 60，候选 idle/sleep 最多约 30，时间推进与刷新率无关；保留 120 ms 过渡和命中区域。无过渡/覆盖时 source→output，过渡仍走需要的合成路径。真实高刷屏 A/B 检查动作周期、缩放与拖动、睡眠/唤醒、透明边缘，记录 CPU/GPU/帧间隔；视觉不通过则不采纳 30 fps 默认。

### P12｜几何缓存与统一隐藏停机（#12，中；依赖 S08b，lib.rs 串行）

文件：src/pet/useGifPassthrough.ts、PetApp.tsx、GlbPetView.tsx、PetRenderer.ts；src-tauri/src/pet_visibility.rs 及所有直接 hide/show 路径；useGifPassthrough.test.ts、useGifPassthrough.lifecycle.test.tsx、useGifVisibility.test.tsx、pet_visibility_tests.rs、pet_geometry_tests.rs、pet_geometry_native_tests.rs。

QA：窗口不动时只轮询鼠标，position/DPI 由移动/跨屏/缩放事件失效并刷新；保留单飞/过期响应隔离。所有原生 hide 路径发统一可见性结果，隐藏后新增穿透轮询为 0，主动渲染暂停；恢复先刷新几何、重置时间差，不能动作大跳。双屏不同 DPI、拖动锁/边缘点击、快捷键/托盘隐藏恢复均测试；可见静止 2D 读取预算约由 75 次降为 25 次/秒，实际数由计数验证。

### P13｜内置 GLB 包络预计算与导入缓存（#13，中）

文件：src/pet/PetRenderer.ts、PetAnimation.ts、personaCatalog.ts；PetAnimation.test.ts、PetRendererActions.test.ts、PetRendererResources.test.ts、sandaimeAsset.test.ts；现有 scripts/prepare-personas.ts 与 persona 资产构建入口，优先扩展既有流程。

步骤：按当前生产算法为内置模型离线生成联合包络，携带模型 hash+算法版本；运行时校验后直接读。未知/导入模型保留完整计算兜底并按相同键缓存；缓存坏/缺/旧则重算，不相信外部包宣称的包络。

QA：已知模型 7 动作不再首帧做 175 样本扫描，包络读取/相机初始化 CPU 目标 <10 ms（目标，不是已测收益）；全部动作极值与最大缩放不裁切，采样后姿态恢复、重复 load/dispose、hash 变化/算法升级/坏缓存测试。保留实际 WebView2 首帧测量，CPU 微测不可冒充整机启动。

### P14a/P14b｜模型候选实验与采纳（#14，大；依赖 P13 的版本键）

文件：public/personas/xiaozhu-sandaime/figure.glb、personaCatalog.ts、相关资产 manifest/pack 构建入口；sandaimeAsset.test.ts、xiaozhuSandaime.test.ts、scripts/bundled-personas.test.ts、persona-asset-policy.test.ts。

P14a 只生成隔离候选，不覆盖原模型：原图/1024/512 同机 A/B；对 10 张贴图、7 meshes、7 animations 做差异核验，不因大小相同就去重共享。记录 bytes、解析/纹理解码、首帧、纹理数量和实际 GPU 驻留；没有可用 GPU 计量时明确未测，不能用 RGBA 理论值替代。

P14b 仅采纳已通过视觉和结构检查的候选：默认及最大尺寸截图逐部位比对，7 段动画、透明/细节/根节点可见性全验，离线切三代角色成功；重新生成 P13 包络/hash 及资源校验。若都不通过，保留原图，记录实验结果；KTX2/结构共享只在剩余瓶颈明确时作为额外条件工作，不提前加解码依赖。回退换回原资产和对应包络/manifest，不能只换 GLB 留错 hash。

## 9. B5：拆控制锁、增量采集与长会话

### R06c｜带完整身份的旧投影移出控制锁（#06 第三步，中；依赖 R06a/b）

文件：agent/collector.rs、lifecycle.rs、opencode.rs、history/archive.rs、ai_usage/local.rs；collector_tests/race_tests.rs、permission_tests.rs、lifecycle_contract/recovery/restart 测试。

步骤：在 R06b 用量 HTTP 已出锁后，锁内只验证当前 run/workspace/session/generation 与决定关键状态；旧投影工作在锁外执行。发布/持久化前重验完整身份、版本、墓碑与当前所有权，使用既有幂等边界防重复，删除或移交后的旧投影不得复活会话。关键状态耐久性、陈旧快照拒绝、权限来源校验与归档失败隔离保留；不能把所有操作简单移出锁。

QA：挂起用量/归档 HTTP 直至原 8 秒超时期间，Cancel/Stop 能取得控制权（合成目标锁等待 p95 <100 ms，原生记录真实值）；用屏障测试确定“不等待网络释放”而非脆弱绝对时长断言。并发完成/取消/换 run/重启/写失败不发布陈旧结果；记录锁持有 p95、持锁路径和最终持久化记录。

### R06d｜SSE 驱动增量刷新与全量校验（#06 第四步，大；依赖 R06c）

文件：agent/collector.rs、opencode.rs、已有 SSE client/event 注册点、record_store.rs；opencode_wire_tests.rs、opencode_settlement_tests.rs、collector 全组。

步骤：先确认固定 OpenCode 1.18.21 的事件字段/重连语义，复用现有 SSE；按 run/session 合并刷新请求，利用消息/part ID 去重，必要时有界拉取。低频全量校验负责漏事件与恢复；重连、sidecar generation 变化、终态前确认触发校验。保留无 active 时不发 HTTP 的现状，不重造第二个事件总线。

QA：重复/乱序/丢失事件、断线重连、终态漏通知、权限事件、工具结算与 sidecar 切换；最终结果和全量参考一致，不双结算，取消仍快。1000 消息/10000 run 对照 60 次刷新，记录网络 bytes、HTTP 次数、文件读写、CPU/锁 p95。只收到事件不代表任务已提交；恢复测试全部通过后采纳。

### C16｜流式增量合批与稳定消息行（#16，中；依赖 C05/C10/C15）

文件：src/chat/ChatApp.tsx、ChatText.tsx、必要的现有消息渲染子组件；ChatText.test.tsx、chatPetActivity.test.tsx、chatFixedReply.test.tsx、chatWorklogPrompt.test.tsx、chatAttachmentSend/Lifecycle、键盘导航测试。

步骤：第一个内容立即发布，之后按 message ID 累积文本，每帧或初始 33 ms 预算合并；完成/错误/Stop 前 flush 合法当前代数据。切会话先完成旧代的持久/状态处理再清队列，旧代绝不刷入新会话。旧消息引用稳定，memo 消息行、日志操作按 ID 查找；先 profile，确认仍有主要瓶颈后才考虑活动 Markdown 分块/虚拟化。

QA：1000 历史消息、50 次/秒、10k/50k 字符代码/表格尾消息，最终字符串逐字一致；首字不退回等待，后续提交不超过已选预算，工具状态及终态无丢失。Stop、重试、切会话、卸载、UTF-16 边界、partial Markdown、链接安全、复制、历史行 DOM 不重挂载、滚动锚点/输入焦点均回归。测提交次数、输入响应、滚动 p95，不能只减少 setState 计数就宣称流畅。

### D20a/D20b｜工作记录传递擦除减少重复扫描（#20，中）

文件：src-tauri/src/worklog/repository_sources.rs、tests/repository_sources.rs、runner_publication_tests.rs、reports_tests.rs。

D20a：过滤前后快照相同则不 UPDATE，先做行为不变的小步。D20b：收集本轮需擦除的 source/report 传递闭包，集中处理版本和未完成 run 快照；一次加载建立本轮临时关系，不持久化新索引体系，不每个相关版本递归全扫。关系表不是唯一来源，必须遍历可能独立存在的快照。

QA：10000 无关版本、1 个来源、20 个相关版本、链式/重复引用、脱离 report_sources 的未完成 run；无关版本零写入，扫描次数不随相关版本倍增。保留取消在途生成、stale/source_deleted、删除或保留报告的用户选择、事务失败回滚与遗忘不可复活。记录解析数、SQL 写数与锁时长。

## 10. B6：存储优化按三小步推进

本批只能使用合成数据库或经过一致性备份的隔离副本；不得直接拿日常用户库试迁移。复制活跃 WAL 库必须用 SQLite 在线备份或关闭连接后的完整一致性复制，不只拷贝 .db。源副本先哈希并只读保留；迁移测试库与原库分开。每个实际 schema 变更独立升级版本，事务失败原版本可重开，未来版本拒绝行为保留。

### H18a｜成功写入驱动 revision/generation（#18 第一步，中；依赖 R06 相关接口稳定）

状态（2026-10-24 Mac）：**代码与自动测试通过但原生待验**。历史文件、旧 native index、Agent 目录证据及 native 关联分别跟踪 revision；命中时不读/克隆/编码全量来源。历史文件替换、Agent 持久化、catalog 事务成功后才发布相应失效。流式 part/call ID、用量重试和 pending outcome 不触发无关来源重导入；组织操作与墓碑仍从数据库读取，工作台所有权仍实时派生。导入失败/并发变更不确认完成代际，变更请求后续刷新。首次及距上次成功恢复校验 30 秒后的下一次读取会重读历史/index/run 来源，异常保留原内存并允许重试；这不是无条件每 30 秒唤醒，也不是外部变化 30 秒内必然可见的 SLA。

本机固定 Bun 1.3.14 / Rust 1.94.1，基线完整 `bun run check` 通过；H18a 后完整 Rust 901 pass / 0 fail / 18 ignored。正文 10k/100k 字符各 100 次命中：新增 RunStore 文件读取与 catalog 导入均为 0；没有把该操作计数当原生耗时收益。完整工作台从固定 commit 的官方源码归档重建，原锁文件哈希保持不变。实际命令、失败与复验及本地证据位置见 [当前验收记录](verification-backlog.md#2026-10-24-mac-b6-h18a)。未改 schema、版本或发布资产。

文件：history/commands.rs、maintenance.rs、storage.rs、native_index.rs、catalog_local.rs、catalog_import.rs；agent/record_store.rs、lifecycle.rs 的已稳定持久化边界；history/catalog_local/content/migration/reconcile 和 recovery 测试。

步骤：列清所有失效入口后才替换 fingerprint：history_save/delete、legacy/local messages、Agent input/snapshot/outcome/最终提交、native index 变化、启动 index_agent_records、native register/reconcile、catalog mutation 与删除回执、目录与启动恢复。revision 只在对应存储成功提交后推进；读取抓取一致 generation，导入期间若再变更就请求下一轮，不把旧结果标成最新。分清真正需要重导入的来源与只需刷新派生所有权的变化，避免任何事件都触发全量导入。workbench ownership 的 6 秒租约可以自然过期，不能只依赖 mutation generation；读取/操作时仍动态派生与验证。未知外部变化用启动/低频恢复校验兜底，不能继续每次 clone+serialize 所有正文。

QA：缓存命中不 clone/hash 完整 HistorySession/records；固定会话数把正文放大 10 倍，未变列表的工作量不随正文增长。逐个写入口、失败写、并发写、重启、late Agent snapshot、墓碑、工作台 ownership 切换正确失效；当前导入失败不推进 initialized revision。

### H18b｜单条目录事务按 key 修改（#18 第二步，中；依赖 H18a）

状态（2026-10-24 Mac）：**代码与自动测试通过但原生待验**。`update_entry` 在 IMMEDIATE 事务内按完整主键读取、验证并更新目标行，禁止换 key；未改变不写入。注册、组织操作、内容证据、删除回执及空会话清理已接入。批量导入/目录发现保留全量原子路径；来源关联 revision 仍仅在提交成功后推进。物理移除条目及偏好删除同事务，偏好写入也在同一事务检查目标存在。迟到操作重新验证当前墓碑、可用性和运行状态，保留较新的组织、所有权和内容证据。

1k/10k/100k 合成库各执行 100 次单条修改，解码均为 100、UPDATE 均为 100，EXPLAIN 使用主键索引；这是存储操作计数，不是完整端点或 GUI 性能保证。无关损坏行保持原字节，单条路径沿用 `get` 的隔离策略；目标损坏拒绝，`all`/批量事务继续全量验证并报错，未引入静默修复。完整 Rust 916 pass / 0 fail / 19 ignored，其中新增手工规模测试已单独执行通过。命令与覆盖见 [H18b 验收记录](verification-backlog.md#2026-10-24-mac-b6-h18b)。schema 仍为 2，产品仍为 0.4.21；H18c 未开始。

文件：history/catalog.rs、catalog_mutation.rs、catalog_content.rs、commands.rs、maintenance.rs；catalog_tests.rs、catalog_local_tests.rs、catalog_model_tests.rs、catalog_content_tests.rs、ownership/migration_rollback 测试。

步骤：为确实单条的重命名/置顶/归档/内容证据更新复用按 key 查询，事务内只读/验证/写目标行；批量导入仍有自己的原子路径。保留完整身份验证、目录规范化、当前 runtime/ownership guard、删除 model selection 与墓碑逻辑，不把 UI capability 当授权。

QA：1k/10k/100k 行中单条修改解码数量恒定；无关损坏数据如何处理沿用当前策略并有明确验证入口，不能意外跳过必须的全局约束。跨目录同 session ID、并发改/删、原生不可用、失败回滚、自动空会话清理与轻聊/工作台争用不退化。

### H18c｜筛选排序投影列与 SQL 分页（#18 第三步，大；依赖 H18b）

状态（2026-10-25 Mac）：**代码与自动测试通过但原生待验**。独立 schema 2→3 迁移增加查询投影表、分页/目录/来源/离线索引；不重写旧 metadata，偏好与目录注册表保留。单条/批量 metadata 与投影同事务更新，直接 SQL metadata 写入会标记未验证投影，列表拒绝返回旧结果。列表/count/离线/目录读取共用 DEFERRED 快照，只解码返回页；工作台实时租约在页内重算能力，旧全量查询仅保留为测试 oracle。

1k/10k/100k 数据各 97 组查询矩阵及首页/深页/搜索/目录/来源样本与旧 oracle 等价；50 行页均仅解码 50 行，对应索引均命中。保留 Rust Unicode 小写与字面子串、全部 u64 时间戳、空记录/墓碑、默认归档、置顶及 key 排序。中断注入、SQLite 容量上限、损坏、未来 schema、重复/并发打开、WAL 读写、物理删除及投影写失败均覆盖。完整 Rust 926 pass / 0 fail / 20 ignored；新增 ignored 规模测试单独通过，生产 `cargo check` 通过。预热后的 30 样本分布、扫描工作量及边界见 [H18c 验收记录](verification-backlog.md#2026-10-25-mac-b6-h18c)。count、搜索、深 offset 和来源恢复仍可能随数据增长；不宣称整个端点 O(1)。

文件：history/catalog.rs、catalog_schema.rs、catalog_sql.rs、catalog_query.rs、commands.rs；catalog_schema_tests.rs、catalog_sql_tests.rs 及既有 history/ownership/迁移回退测试。起点 catalog schema 为 2；新增版本 3 保留旧基线含义，旧版拒绝写入新库。

步骤：把实际查询需要的稳定字段投影为列/索引，metadata 与投影同事务更新；保留当前搜索大小写/substring、空会话排除、tombstone、pinned 优先、时间排序和 key tie-break。不凭 SQL 默认排序替换既有语义。列表/count 在 SQL 执行，最多解码返回页；全局目录与错误元数据单独读取。搜索与深 offset 的代价分别记录，不能声称任意深页恒定成本；先不改外部分页 API。

QA：迁移前后同一 1k/10k/100k fixture 的所有过滤/排序/分页结果完全一致，含同时间戳、未知 hasRecords、中文搜索和跨目录碰撞。50 行页不解码全 N 行；EXPLAIN QUERY PLAN 用到预期索引。迁移中断/磁盘满/损坏 JSON/未来 schema 拒绝、旧库只读对照、重复打开、并发 mutation 全测。把全量旧实现保留为测试 oracle，不在正式热路径运行双份。

### M19a｜有界到期索引（#19 第一步，小；依赖 A04）

文件：memory/repository.rs、storage.rs、新增独立 migration；repository_tests.rs、perf_tests.rs、cross_process_tests.rs。

步骤/QA：只为 active 且 expires_at 非空建立适配谓词的部分索引，不顺手改变读语义；新建 migration，不修改已应用的 001_initial.sql。100k 行无到期、有少量到期、集中到期、无关 persona 下 EXPLAIN 不再全扫到期 UPDATE；保留 ISO 时间与时区比较语义，旧 schema 迁移原子成功，失败仍可打开旧副本。

### M19b｜读取实时过滤，物化过期独立维护（#19 第二步，中；依赖 M19a）

文件：memory/repository.rs、retrieval.rs、commands.rs、automatic/worker.rs 或已有维护入口；retrieval_tests.rs、repository_tests.rs、perf_tests.rs、commands_forget_tests.rs、cross_process_tests.rs。

步骤：list/retrieve 查询实时排除到期项，避免每次 BEGIN IMMEDIATE；独立维护按最近到期和有界批次做状态物化。expire_due 还处理 pending candidates，拆分时必须保留其到期与重试语义。明确 active/expired/superseded 管理列表、显式 expired 查询、导出、统计的即时语义；不能因为延后物化让 expired 列表少项，或让已过期文本重新被 FTS/LIKE 返回。读取使用已有连接与正确事务模式，不全局把写事务改 DEFERRED。

QA：冻结时钟覆盖边界、立即到期、远未来、集中到期、关闭后重启、persona/global 作用域；对 active/expired/list/export/FTS/短词 LIKE/pending candidate 的逻辑过期和已物化结果逐一对照。只读检索不拿写事务，读期间有并发写仍正确，导出/管理页面一致。保留 10k 检索预算及跨进程锁测试；过期维护不能阻塞显式忘记。

### M19c｜稳定 rowid 的 FTS 精确维护（#19 第三步，大；依赖 M19b）

文件：memory/storage.rs、repository.rs、retrieval.rs、新增 migration；repository/retrieval/perf/cross_process/commands_forget 测试。

步骤：设计显式持久 INTEGER PRIMARY KEY 映射到 memory UUID（或同等稳定显式主键），由该稳定 id 精确定位 FTS rowid；不要使用可能在 VACUUM 变化的隐式 rowid。新增迁移中原子更新映射、FTS、触发器和 schema/user_version，不修改 001 历史迁移。触发器只在 content/memory_key/status 等实际检索字段变化时重建；非检索 metadata 更新不删重插全文。迁移重建、增删改、事务回滚、状态变更均保持映射与 FTS 一致，trigram 与短查询 LIKE 行为保留；保留旧版本拒绝写新 schema 的明确行为。

QA：1k/10k/100k 行固定一批 UPDATE/DELETE 的 FTS 定位不全扫、不随全库线性增长；VACUUM 后 UUID↔rowid 仍一致；重复/冲突/回滚/重开/迁移失败不漏搜不误删。保留 active-only、真正擦除、secure_delete、WAL checkpoint(TRUNCATE)、批量遗忘后的 compact；用已知敏感哨兵检验原表、FTS、来源/关联和可控数据库文件残留，保留现有隐私测试。不得以 external-content FTS 改造悄悄削弱删除保证。

## 11. B7：验证、退出条件与回退

### V1｜集成检查与 Windows 原生全链路（中）

每个任务先运行本任务的定向测试，再提交集成检查。小步不反复跑无关全套；批次触及多个核心模块和最终 B7 时运行完整套件。前端文件隔离测试沿用 scripts/test-frontend.ts，单文件用 bun test 对应文件；避免把互相污染的 DOM 文件放进同一进程。

最终顺序（所有命令在开发入口执行，各命令完成后再继续）：

~~~powershell
bun run typecheck
bun run test:frontend
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib -- --test-threads=4
bun run build
~~~

React 改动按项目 react-doctor skill 做差异诊断，记录工具版本与实际命令，比较新增问题而非以总分代替正确性；使用 npx react-doctor@latest --verbose --scope changed 时记录解析到的版本，不为刷分加入无关重构。完整前端 suite 与 build 严禁并跑。若 overlay 改动，在主 build 前按 workbench-build.md 固定 OpenCode 源/lock 运行 prepare:workbench，记录生成物指纹；资源打包检查走现有脚本。Windows 生产配置编译与可运行候选必须明确记录 feature/config；不安装到日常环境，不改变自动启动设置。

原生场景：G1 单实例/孤儿/并发启动；首次/二次工作台与所有权；普通/固定回复、Stop 与批准；1000 消息长会话；真实 PDF；浏览器上传与原生附件；2D/3D 切换、隐藏、睡眠、跨屏 DPI、高刷；10 分钟空闲；引擎失败/手动恢复；日志五工具/到期任务；历史目录/空会话/删除；记忆读取/导出/忘记/迁移重开。使用隔离数据，记录 mock、真实引擎、真实 GUI 和真实模型分别覆盖到哪里。性能任务优先用可重复合成服务，不把模型随机延迟当客户端收益。

### V2｜完成判定与平台边界（小）

任务状态只有：待做、进行中、代码与自动测试通过但原生待验、已验收、条件实验未采纳。每一条 #01–20 都有任务、证据、结论；“未采纳”仅用于预先明确的视觉/结构实验，缺验证不等于不采纳。

Windows 完成门槛：正确性定向与全套测试无新失败；关键控制/权限/数据删除/迁移用例全过；对应工作量计数达标；真实性能测量有同环境前后对照，不用理论 bytes 或 Node 微测代替原生内存/GPU；首开、输入、滚动、空闲、峰值内存没有无法解释的退化。时间门槛为事先目标，不作保证；重复采样退化则先定位，不能以另一项收益掩盖。

macOS 单独待验：单实例/恢复语义、WebKit PDF worker/CSP、媒体/二进制上传、GLB/rig2d、高刷与隐藏、跨屏 DPI/窗口所有权、原生 CPU/GPU/耗电、存储恢复。没有 Mac 实机本批可产出 Windows 验收结果与清晰待验表，不能声称跨平台完成；不触发签名、公证或发布。

交付：更新本计划任务状态，生成结果 HTML/证据索引和前后指标表，代码变更与条件实验分开说明；更新 docs/verification-backlog.md、memory-and-worklog.md、local-resource-attachments.md、unified-history-contract.md、workbench-build.md 中实际发生变化的部分。失败/skip/ignored/缺平台仍写明，不继承旧“通过”冒充本轮证据。

### 回退与故障处置

- 无迁移代码：按任务/小步回退自有差异，保留用户和其他人的编辑；不全仓 reset。资产和包络/manifest 一起回退。
- collector/合批/通知：每一步先留完整回归证据；退回最近通过的小步版本，保留关键落盘、原有恢复与补偿路径。不得为了回退重新关闭认证或移除权限检查。
- 数据库：回退程序与降级数据库是两件事。迁移前的隔离副本可还原用于实验；日常新增数据不得被旧备份覆盖。新版本数据库不让旧二进制直接写；无法证明无损降级时保留新库和可读入口，修复前进。不要把“有备份”写成已证明可逆。
- 隐私与删除：改动破坏擦除/墓碑/恢复时阻断本任务验收，先修正确性；不接受改成软删除或延迟无限期清理换性能。
- 原生进程与测试环境：只清理本次登记的 PID/出生时间/专属目录；确认绝对路径在证据/fixture 根内后才做目录清理。自动批准拒绝若发生，按拒绝的实际范围处理，不另造全局绕行。

## 12. 审计发现到施工任务的完整映射

| 审计 | 施工任务 | 批次 | 工作量 | 必须保留的结果 |
| --- | --- | --- | --- | --- |
| #01 二次启动误杀 | G1、A01 | B0/B1 | 中 | 确认唯一 primary 后仅回收真孤儿；生产路径原生矩阵 |
| #02 日志桥无认证 | A02 | B1 | 小 | 认证+可信目录+父消息/call ID 全链路 |
| #03 PDF worker | A03 | B1 | 小 | 本地生产资源/CSP、loadingTask 清理、WebView2 |
| #04 导出参数超限 | A04 | B1 | 小 | 32767/50000 条导出完整，参数有界 |
| #05 普通回复等 2 秒 | C05 | B2 | 小 | 普通首字即时、固定台词节奏保留 |
| #06 collector 全量与控制锁 | R06a/b/c/d | B3/B5 | 大，四步 | 不变不写→按目标读→拆锁→SSE+补偿 |
| #07 空闲轮询 | I07a/b/c | B2/B3 | 中，三处 | bridge、scheduler、UpdateFooter 各有计数与恢复 QA |
| #08 未用工作台/3D 先加载 | S08a/b | B3 | 中 | 惰性窗口和 chunk，首开/交接不丢失 |
| #09 重启风暴/日志覆盖 | S09 | B3 | 中 | 健康后清零、等待可取消、故障日志保留 |
| #10 mood 平方复制 | C10 | B2 | 中 | 状态变化才广播，有界去重，迟到事件拒绝 |
| #11 rig2d 高刷/双合成 | P11 | B4 | 中 | 60/30 A/B、真实时间、视觉与功耗 |
| #12 几何 IPC/隐藏轮询 | P12 | B4 | 中 | 几何失效缓存、全 hide 路径停机/恢复 |
| #13 GLB 175 次采样 | P13 | B4 | 中 | 版本化包络、未知资产兜底、不裁切 |
| #14 GLB 贴图/结构 | P14a/b | B4 | 大，条件采纳 | 候选实测、视觉通过才替换、离线角色保留 |
| #15 导航观察器 | C15 | B2 | 小 | 正文增量不重建全观察器，跳转准确 |
| #16 流式提交/Markdown | C16 | B5 | 中 | 首块立即、后续合批、终态 flush、旧行稳定 |
| #17 number[] IPC | F17 | B4 | 中 | 真实 binary body、元数据边界、Blob 回收 |
| #18 历史全量读/分页 | H18a/b/c | B6 | 大，三步 | revision→按 key 事务→投影索引/SQL 分页 |
| #19 到期扫描/FTS 扫描 | M19a/b/c | B6 | 大，三步 | 部分索引→读过滤/维护→显式稳定 rowid |
| #20 删除递归全扫 | D20a/b | B5 | 中，两步 | 不变不写→闭包一次处理；独立快照不漏擦除 |

历史顺序：2026-10-22 首次开工执行 G0/G1 和 B1，随后用户逐批授权推进至 B5；Mac 已完成 B6 H18a/H18b/H18c 的代码与自动回归。当前接续核对本地检查点后从 M19a 开始；原生缺口仍按 V1/V2 补验，不重做已保存步骤。


## 历史回执：B1 开工（2026-10-22）

基线：0.4.20，HEAD `d92a9c872b01300a27d90a675cfa9247bfd2fd2b`；实际开发入口为
`C:/Users/chenglingxiao/.codex/worktrees/build-v0418-9081/deskmate-public-release`，施工分支
`codex/yume-performance-remediation`。以下证据对应未提交差异，不是旧主仓或安装版。

- A01：Windows 原子主实例占有在插件初始化之前完成；旧插件窗口须归属当前 PID。
  删除名称/CORS 全局进程清理，按宿主与子进程出生时间、镜像和应用身份回收记录中的孤儿。
  原生隔离子进程测试 5 pass、1 ignored fixture，包括并发占有、宿主崩溃回收与无关 sibling 保留。
- A02：日志桥使用宿主认证、规范化工作目录和精确父消息/工具 call ID；工作台先登记无权限待验证回合，
  执行前核实真实用户消息。关闭自动提取不禁止显式工具；重启（含创建新目录或连接校验失败）先撤销旧启动。
  同回合重放不能恢复撤销授权或清空失败查询。真实 Rust HTTP 桥 48 pass；TypeScript 传输 10 pass。
- A03：本地 PDF worker 随构建输出；成功、异常、截断、取消都释放 loading task；附件准备支持中止。
  同产品源码的 Vite 浏览器探针正常/损坏/超 160k 各 7 次，共 21/21，结束后 0 存活 worker。
  浏览器探针与产品构建 worker SHA-256 相同；不等于 Tauri 本地协议、WebView2 RSS 或 macOS 验收。
- A04：记忆导出的 applicability 查询复用 400 ID 分批；32767 / 50000 条旧逻辑实际失败，修复后完整字段、
  状态、上下文、来源、任务关联及顺序校验通过。记忆模块 97 pass；未改 schema 或迁移。
- 最终完整 Rust 库：747 pass、0 fail、16 ignored，退出 0（4 线程）。
  前端 155/155 文件、1135 pass、0 fail、8 skip、0 unfinished，每文件子进程退出 0。
  前端 Bun 子进程结束后日志包装进程未收尾，已核对命令/PID 后回收；没有声称总包装命令退出 0。
- 三套 TypeScript、前端生产构建、正常 Windows debug 可执行文件构建和 `git diff --check` 退出 0。
  React Doctor 0.9.18 changed 扫描 9 文件、82/100，No issues found，退出 0。保留已有构建警告。
  未改工作台 overlay，未完整重建上游工作台；未安装、推送或发布。
- 修复过程中保留失败日志：A01 死亡进程镜像查询失败改为同句柄退出/出生时间判断；A02 Windows HTTP
  测试夹具继承非阻塞 socket，明确设回阻塞并避免析构二次 panic 后并行回归通过；未削弱业务断言。

待验：完整 Tauri 生产启动路径的并发/二次启动任务连续性；WebView2 本地 worker 协议、真实取消、20+ 次循环
后的堆/RSS 收敛；macOS WebKit。当前工具不能操作 Windows 原生 GUI，以上保持未验，B1 不标整体验收通过，
此处记录的是 B1 收尾时状态。后续用户授权保存并继续，已推进独立 B2 前端修改，但未关闭这些原生门槛。尚未测得端到端性能增益。
开工时记录的安装版 YUME/引擎 PID 在收尾只读检查时已不在运行；退出原因未确定，未为验证重启安装版，
也未据此声称活跃任务连续性通过。前后进程摘要保存在 baseline/relevant-processes-after 回执。

保留边界：旧版没有归属记录的孤儿，以及 spawn 到写记录之间崩溃的子进程，均保守保留；
当前启动期间 revoked/expired 回合保存 32 字节摘要墓碑，数量随该次启动的回合数增长。
另发现既有自然语言调度解析把“工作日志”中的“工作日”当工作日限定；未在认证修复中扩大修改，待独立语义回归。

完整回执：`E:/Codex/工作伙伴/reports/yume-performance-remediation-20261022`，其中
`verification-summary.json`、`source-snapshot.json`、`b1-tracked.patch` 与逐项日志保存证据。


## 历史回执：B2（2026-10-22）

B1 已保存为本地提交 `a48ee36be8195766322439843fd76db69819fa40`。按用户“保存然后下一步”授权，
在 0.4.20 的 `codex/yume-performance-remediation` 上完成独立前端整改；B1 原生门槛保留为交付检查。

- C05：普通模型正文和工具内容不再等满 2 秒，本地固定台词节奏保留。停止、已知旧父消息、跨目录/会话和
  卸载有回归；重开运行中原生历史继续接收正文与 idle。审查发现的历史恢复回归保留 RED→GREEN 证据。
- C10：同请求连续 10,000 次相同 mood 广播从 10,000 次降为 1 次；双方生产者共享 epoch/sequence。
  消费端历史 Set 被固定大小水位替代。10,000 个不同事件累计复制元素 50,015,002→0、历史集合元素 10,002→0；
  这不是总内存为零或原生速度测量。GLB/GIF/rig2d 共用事件通道，保留 GLB 原始动作语义。
- C15：500 轮/1,000 行/50 次正文更新，观察器创建 51→1、累计 observe 51,051→1,001。
  真浏览器正文更新阶段创建/订阅/断开/几何读取增量均为 0；实际高度变化、新增删除、跳转和 Tab/Escape/Enter 通过。
- I07c：焦点/可见性/联网事件唤醒；计划状态查询在前次完成后等待可见/隐藏 idle 5/30 秒、busy 1/5 秒。
  可见性与状态探测最多在途一个，manifest 单独处理。当前窗口更新用 Channel，相同快照不额外提交 React 状态。
  语言切换保留进度和取消；字典 ref 在提交阶段更新。32 项定向测试通过。
- 最终三套 TypeScript、完整前端 156/156 文件（1172 pass、0 fail、8 skip、0 unfinished）、生产前端构建均退出 0。
  Node 24.14.1 / Bun 1.3.14。套件和构建串行，包装器直接写文件并正常退出。
- React Doctor 0.9.18 changed / HEAD / include-untracked 扫描 23 文件，退出 0，新增诊断 0；81/100。
  B1 的 82 分来自不同基线和 9 文件；当前 13 项评分诊断全部匹配既有基线，不声明仓库零诊断。
  新增 render-ref 问题已修，未禁用规则；独立代码复核无剩余阻断。
- Chromium 直接导入最终产品源码，合成 SSE/IPC：普通回复首次事件→DOM 8.8 ms，结束→就绪 DOM 3.4 ms，
  最终正文一致，实际 Stop 回到可编辑状态。这是单次回归预算，非真实模型或 WebView2 性能统计。
  合成慢 IPC 下 100 次唤醒仅 2 次状态查询、最多并发 1；模拟隐藏 idle 6.1 秒无额外查询，恢复显示 50% 进度。
  三个页面无捕获浏览器错误；未建模的历史/工作台/权限非关键 IPC 记录在 chat-result.json，不能当这些功能验收。
- B2 没有改 Rust/工作台 overlay，未重跑 Rust 或构建含 B2 的 Windows EXE；B1 的 747 项 Rust 通过是保留基线。
  Vite 既有大 chunk 警告保留。未安装、推送或发布。

待验：B1 原生二次/并发启动任务连续性与 PDF 本地协议/堆收敛；B2 Tauri/WebView2 焦点/隐藏、真实更新生命周期、
长会话滚动与宠物动作像素、10 分钟空闲 CPU/GPU/功耗、macOS 与真实模型。现有工具不能操作 Windows 原生 GUI，
浏览器和单测不能关闭这些门槛。未知父消息先于元数据、无 requestId 的 session idle/error 仍不能严格归属到请求；
存储禁用+完整页面重载+时钟回退的 epoch 顺序边界保留，不保证跨消费者完整重建的旧事件去重。

证据：`E:/Codex/工作伙伴/reports/yume-performance-b2-20261022`，包括 `verification-summary.json`、
`source-snapshot.json`、`b2-checkpoint.json`、`review.json`、`doctor-score-review.json` 和 HTML/浏览器截图。


B2 已保存为本地提交 `19630b6a69d5944c53156bf666879cf3afd7cf0c`，26 个文件；保存后工作树干净。回执见上述目录的 `b2-checkpoint.json`。


### B3 执行回执（2026-10-22）

R06a/b、I07a/b、S08a/b、S09 源码完成；完整 Rust 819 pass / 17 ignored，前端 1183 pass / 8 skip，157 文件全通过。三套 TypeScript、完整固定版本工作台、前端和 Windows debug 构建通过。实际隔离文件桥 600 秒对照 5959→60 次枚举。Chromium 两种 2D 首开均无 3D 请求，真实 GLB 加载和切回通过。详细边界、失败修复与本地保存点见 E:/Codex/工作伙伴/reports/yume-performance-b3-20261022/yume-b3-progress.html。

下一批为 B4：F17 附件、P11/P12 桌宠生命周期与帧率、P13/P14 资产候选；正式资产替换必须先满足原方案视觉门槛。B1–B3 原生待验继续作为交付门禁，不因推进独立任务关闭。

## B4 保存记录（2026-10-22）

- 本地检查点：`5ef42d8e69c002fc1446e7a1ed943c658bd139a5`；分支 `codex/yume-performance-remediation`，保存后工作区干净，未推送、安装或发布。
- 当前契约已回写 `docs/runtime-lifecycle.md`、`docs/local-resource-attachments.md`、`docs/verification-backlog.md` 与资产生成入口文档。
- 证据与 HTML：`E:/Codex/工作伙伴/reports/yume-performance-b4-20261022/`。保留所有失败回执及复验；浏览器夹具不关闭原生门槛。
- 后续 B5 顺序：先 R06c 将旧投影构建移出控制锁；再独立 R06d SSE/断流恢复；C16 流式合批与 D20 删除扫描分轨推进，各自保持取消、最终文本、完整身份和隐私擦除合同。


## B5 发布补充记录（2026-10-23）

- B5 功能保存点：`324374839ad04a7f2820ff0c0e41480ba810c1ee`；两次 CI 测试夹具修复分别为 `3f946aabdddf63b4784266ca6f4cd7f7024670ad` 和 `fee4d707804f76e5c608ceef33c622e71d17b175`，均未改生产逻辑。
- 当前发布 run：`39813641175`；main 源码检查：`39813642512`。Windows 0.4.21 未公开候选的每次推进均核验无 Release/资产、远端 main 与 tag 精确对象、旧运行已结束；main 快进，标签仅以旧对象 SHA lease 更新。旧 v0.4.20 草稿保留。
- Windows 低 uptime 夹具已修，定向测试各 1 pass；模拟 fixture expiry sweep 不等同于执行真实 worker 调度，真实授权边界由现有 authorize_at 的 1800/1801 秒测试覆盖。
- 本次类似代码搜索发现独立待办：automatic worker 初始化 now−10 秒的极早启动边界；workbench 测试 now−6.001 秒；仅 ignored 原生测试使用的 permission checked_sub(300 秒)。本次未改，不声称全仓时间运算已清零。详见 B5 `review/release-instant-search.json` 与 `review/release-ci-instant-review.json`；生产 worker 修复须独立验证首次同步和 10 秒节奏。
- 包内容、更新签名、公开下载核验及最终发布状态以 `release-artifacts/v0.4.21-windows-release/` 回执为准；原生安装、真实跨版本 GUI 更新和 macOS 发布仍不在本次完成声明内。


## 历史状态记录与最终发布回执

日期：2026-10-22（Asia/Shanghai）。状态：B0 已核验，B1 四项代码及自动化回归已保存为本地提交 `a48ee36`，原生 GUI 门槛待验；按用户“保存然后下一步”授权，B2 四项代码、整批测试与浏览器回归通过，原生待验。B2 已保存为本地提交 `19630b6`；按用户“下一步”授权，B3 的 R06a/b、I07a/b、S08a/b、S09 已实现并保存为本地提交 `6f97c93`，整批自动化、完整构建、浏览器及实际文件桥空闲对照通过；原生待验门槛继续保留。按用户再次“下一步”授权，B4 的 F17、P11、P12、P13 与 P14a 隔离候选已实现并保存为本地提交 `5ef42d8`；完整前端 1307 pass、Rust 832 pass、三套类型检查、前端与 Windows debug 编译、三种宠物隐藏/恢复和 42 组模型动作截图核验通过。P14b 决策保留原版模型，默认仍为 60 fps；30 fps 和资产替换须原生视觉验收后再采纳。B1–B4 的原生 GUI/真实 IPC/高刷新与多 DPI/GPU/功耗及 macOS 门槛继续保留。B5 按用户“下一批”授权从干净的 `5ef42d8` 开工；R06c、C16、D20 分轨，R06d 在 R06c 边界稳定后接入，测试与测量串行。B5 的 R06c/R06d/C16/D20 已实现；最终前端 171 文件 / 1341 pass、Rust 904 pass / 19 ignored、类型检查及前端/Windows 编译通过。用户随后明确授权“保存提交发布新版”，本次新增 Windows 0.4.21 发布，已保存为 `324374839ad04a7f2820ff0c0e41480ba810c1ee`，main 与 v0.4.21 标签已原子推送；首次 macOS CI 暴露 B3 测试的符号链接清理平台差异，已单独修复为 `3f946aabdddf63b4784266ca6f4cd7f7024670ad`，仅改测试三行；main 和未公开 0.4.21 候选已更新，第二次 CI 的 macOS 和工作台通过，Windows 发现测试倒退 1801 秒导致新机器 Instant 下溢；已仅修测试为确定性未来观察并保存 fee4d707804f76e5c608ceef33c622e71d17b175，两项定向回归与独立复核通过，main 与未公开标签已按精确 lease 更新，新发布 run 39813641175 六个 job 全部通过，main 源码 run 39813642512 也全部通过；Windows 0.4.21 已公开为 latest，并于 2026-10-23 01:06（Asia/Shanghai）完成包内容、Tauri 签名及匿名全量下载回读验证，源码工作树干净，以最终 release-summary.json 回执为准；不安装到日常环境，macOS 不发新包。B6–B7 尚未实施。

原始发布回执现可从仓库 [证据索引](performance-evidence/README.md) 读取。已经公开的 v0.4.21 标签和资产不再覆盖；后续修复使用新的提交及届时决定的新版本。
