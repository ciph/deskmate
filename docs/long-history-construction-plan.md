# 长历史交互与回执读取施工方案

实施状态（2026-10-29）：B0–B4 全部完成，G1–G5 通过；三条实现链已保存为独立提交，文档/证据完成收尾，临时进程关闭。未升版、未推送、未发布。实际测量、失败与修复记录、原始证据哈希和剩余边界统一见[施工交付记录](performance-evidence/long-history-closeout-20261029/README.md)。下文保留施工前方案与既定门槛，不以新结果放宽标准。

2026-10-29。规划基线：`2ea18cf64695edf95d5da914739fa8cf30492ba5`，YUME 0.4.22，工作目录 `E:\Codex\工作伙伴\deskmate-windows-b7`，分支 `codex/yume-performance-b6`。制定方案时工作区干净；尚未实施产品改动或运行新测试。

本轮用一个里程碑收束三项已确认成本：日志回执重复读取完整会话、长历史全部挂载、流式 Markdown 在界面线程反复完整解析。实施者连续完成准备、修复、对应回归、原生联调及交付记录，不把内部批次变成需要用户反复说“继续”的任务。

“收束”以本方案的正确性与工作量门槛为准。它不代表消灭所有 O(n) 操作：首次打开会话仍须读取正文，Markdown 的完整语义仍须解析全文，用户主动选择大量文本也可能暂时需要大量 DOM。需要消除的是正常交互中的重复全量工作和无界排队。

## 范围与结束边界

| 纳入本次 | 明确保留到其他任务 |
| --- | --- |
| 回执发现与核验复用已有快照；正常 idle 不再为回执重读全会话；按 ID 批量获取宿主状态 | 首次正文分页加载、历史目录和记忆数据库再次改 schema |
| 聊天正文与导航按可见区域渲染；滚动、选择、焦点、媒体和审批行为保持 | 全局 UI 重设计、冷启动插件安装加速、工作台重构 |
| Markdown 解析移出界面线程、合并过期请求、复用稳定块；终态与现有渲染语义一致 | 自制 Markdown 语法、升级 OpenCode、改变链接或 HTML 安全策略 |
| 修复上述改动引入的缺陷，以及旧任务超过最近 100 条后的回执精确读取边界 | R06 全快照比较、D20 全库读取、M19c 硬删除扫描、模型语义、GPU/功耗、Mac 与安装升级验收 |

不升级产品版本，不触碰日常 AppData 和其他工作树，不修改已发布 tag 或冻结的 B7 证据。本次规划不授予新的发布操作；实施阶段的提交、推送按届时任务授权办理。

## 已核对的调用链

| 位置 | 现状与影响 |
| --- | --- |
| [useWorklogChat.ts](../src/chat/useWorklogChat.ts) 的 `recover` | 调用 `getSessionMessages` 读取全部消息，再遍历所有 part；会话打开及普通 idle 都能触发 |
| 同文件 `refresh` 和 `onWorklogChanged` | 每个 operation 单独查回执，再调用 `listRuns` 或 `listSchedules` 后 `find`；一次通知可产生多轮重复查询 |
| [repository_runs.rs](../src-tauri/src/worklog/repository_runs.rs) 的 `list_runs` | 只返回最近 100 条，并读取包含 source snapshot 的完整 Run；旧回执目标可能不在结果中。这是静态可达边界，需用合成回归确认，不称为已实机复现 |
| [history/commands.rs](../src-tauri/src/history/commands.rs) 的 `history_catalog_load` | 已取得带工具 output 的受作用域保护原生快照，当前主要输出正文/附件；可以在同一次读取中投影回执线索 |
| [ChatApp.tsx](../src/chat/ChatApp.tsx) | `messages.map` 全量挂载；流式更新还会重建消息 ID 集合、查找最后用户消息并安排跟随滚动 |
| [ChatNavigation.tsx](../src/chat/ChatNavigation.tsx) | 已避免重建观察器，但仍计算全会话摘要、遍历已挂载行测量位置，跳转依赖目标 DOM 已存在；导航按钮也全部挂载 |
| [ChatText.tsx](../src/chat/ChatText.tsx) | 每次文本变化执行 `remend` 及 `react-markdown`/GFM 全链；`memo` 不能跳过文本已变化的活动消息 |
| [streamText.ts](../src/chat/streamText.ts) | 已有首块立即发布、33 ms 合批、终态 flush；这组正确行为继续保留 |

事实源维持原分工：OpenCode 消息/part 拥有工具调用事实；`yume-worklog.db` 的 operation 和实体状态拥有提交结果；React 索引、测量缓存和 Markdown AST 都可丢弃重建。工具 output、缓存命中或空结果均不能代替真实提交回执。

## 固定工作负载与预算

本表是待实施的验收目标，不能当作已达到的性能数据。已有 B7 的输入/滚动第二帧 p95 约 31–43 ms 只作参考，不作为不同构建之间的严格 A/B。

| 工作负载 | 本轮用途 |
| --- | --- |
| 100、1000、10000 条合成混合消息 | 短会话兼容、既有规模对照、验证 DOM/读取工作量不随总历史线性放大；不做消息数与全部媒体类型笛卡尔积 |
| 1000 条历史上 10k/50k 字符流式，50 Hz、各 500 块 | 与已有场景对应，检验正文、输入、滚动、停止和终态 |
| 回执 K=0、5、100；数据库中超过 100 条任务且目标在旧段 | 空集零查询、批量查询、越过旧列表截断的精确性 |
| 两个目录同名 session、离开后重访、乱序返回、事件丢失 | 完整会话身份、generation 与恢复正确性 |
| 长代码块/单段、嵌套列表、表格、跨块引用/脚注及恶意链接 | 完整 Markdown 语义、安全与最坏形态；不靠空行切块冒充完整解析 |

采样固定同一机器、窗口尺寸、缩放、运行身份和合成输入；记录冷热状态、源码与构建 SHA。已删除 Rust 缓存，需要重建的编译时间不计入应用性能。

## 施工批次

### B0 冻结比较口径

1. 核对路径、分支、工作区、版本，记录已有改动；留存源码差异，不 reset/clean。
2. 复用 [B7 收尾证据](performance-evidence/b7-windows-closeout-20261028/README.md) 和本机 `output/b7-closeout/` 的合成 provider/流式探针；原件只读。将需要修改的最小探针提取到拟新增的 `scripts/long-history-qa/`，运行输出使用新的隔离目录。
3. 只补当前缺少的计数：回执专属全会话请求、IPC/SQL 查询与返回字节、正文/导航挂载数、Markdown parse 次数/耗时/队列长度。采一次同口径基线，不重跑 0.4.22 全套验收。
4. 固定后文 G1–G5；不能看见新结果后放宽门槛。测量无效时修正探针和说明原因，只重采受影响样本。

### B1 回执恢复与精确读取

**决定：复用会话快照，增加小批量精确查询；不新建回执数据库或持久缓存。**

1. 在 `history_catalog_load` 已有 `NativeMessage` 快照中提取白名单 worklog 工具的最小线索：messageId、partId/requestId、明确错误和工具种类。不要再次请求消息；不要把 input、任意 output 或正文原样扩散到新 DTO。legacy/local-only 不伪造原生工具结果。
2. 扩展 `CatalogLoaded` / `CatalogLoadResult`，通过统一加载结果送入 `useWorklogChat`。普通历史与 Agent 历史入口都必须接入；`entry.key + sidecarId + canonical directory + sessionId + view generation` 共同约束应用结果的作用域。初次打开先建立目标作用域和事件接收，再读取快照并合并其间事件，避免快照完成到订阅建立之间漏掉工具终态。
3. 正常流式由现有 `acceptTool` 接受 SSE 工具结果；idle 只 flush/核验已知请求，不再调用全会话 `recover`。初次打开使用 B1.1 的快照线索。
4. 不能仅删除恢复兜底。给 `subscribeEvents` 增加保持旧调用兼容的连接代次/断线通知；断线后重连或明确发现缺失时，合并成每个视图最多一个完整快照恢复，正文和回执共用该结果。恢复期间新事件缓冲后按 message/part ID 和可比较的来源版本合并；缺少可靠版本时标 dirty 补一次核对，不按抵达顺序覆盖终态或已核验回执。正常 idle 不触发这一例外。
5. 增加拟命名 `worklog_get_operation_states` 的只读命令，单批最多 50 个去重 UUID。一次宿主连接内按 operation 主键查询，并按关联实体 ID 返回需要的状态摘要；禁止用 `listRuns/listSchedules` 查单个目标，禁止为聊天返回完整 source snapshot。未知、已删除、失败与待确认分别表达，SQL/解析错误不能冒充空集。
6. Run 摘要只包含界面使用的 id/state/resultReportId 等必要字段；Schedule 只返回 id/nextDueAt 等必要字段。现有设置页的完整 list API 保持原契约。按实体主键读取最近 100 条之外的旧任务，不增加 schema。
7. 前端按 requestId 保存 operation，按 messageId 建派生索引；单轮合并状态更新。查询只保留一个 in-flight 批次和一个 dirty 标记，后到结果需同时核对视图 generation 与该请求的刷新代次。全局 changed 事件暂不新增事件 payload：对本视图已知 operation 合批刷新即可，不能为优化漏掉已完成实体后来被删除的变化。
8. 维持现有规则：空宿主回执不抹掉明确拒绝；只有匹配 requestId 的真实回执能确认提交；离开后重访也拒绝旧异步结果；只读恢复不写库、不重放 prompt、不重新执行工具。

涉及现有文件：`src/chat/useWorklogChat.ts`、`worklogActions.ts`、`WorklogReceipt.tsx`、`ChatApp.tsx`、`useAgentHistoryView.ts`；`src/lib/worklog.ts`、`unifiedHistory.ts`、`opencode.ts`；`src-tauri/src/history/{commands,catalog_query}.rs`；`src-tauri/src/worklog/{commands,contract,repository,repository_runs,repository_schedules}.rs`；`src-tauri/src/lib.rs` 的 IPC 注册。新窄模块按职责添加；命令权限保持现有宿主/窗口边界，不扩大能力。

### B2 长列表与导航共同虚拟化

**决定：按动态高度的可见区渲染，消息数据保持完整；使用一个虚拟列表依赖，不自写平衡树和滚动引擎。**

1. 实施时固定 `@tanstack/react-virtual` 的具体兼容版本并更新锁文件，使用经过核对的 `getItemKey`、`measureElement`、`rangeExtractor`、`scrollToIndex`。不要依赖未核实的 latest 新接口。
2. 将正文列表抽为拟新增 `ChatTimeline.tsx`；只渲染可见区、上下各 8 行 overscan 和必须保留的行。composer、Stop、审批 dock、拖放入口始终位于虚拟列表外，不能因列表加载或测量而被卸载。
3. 缓存以完整会话身份和稳定消息 ID 为 key。宽度、字体/语言或影响几何的内容变化时重新测量；只观察已挂载行。滚动锚点用“消息 ID + 行内偏移”，在尾部时跟随新内容，离开尾部后保持阅读位置；会话切换丢弃旧测量和排队回调。
4. `ChatNavigation` 使用数据索引跳转：先算 message index，定位并挂载目标，再校正偏移。活动轮次按虚拟位置/可见范围确定，不遍历所有消息 DOM；导航 rail 也虚拟化，保留当前、聚焦和预览条目。
5. 导航标题/摘要仅在对应消息发生变化时更新；消息 ID 集合、最后用户 ID 不再随每块正文全表重建。数据结构更新的正确性覆盖插入、替换、删除、local-only 和多 text part。
6. 聚焦的代码/表格、正在播放或操作的媒体、拖放目标及文本选择涉及的行必须保留。非挂载行的横向滚动位置用会话内缓存恢复；不得通过卸载媒体偷偷暂停播放或撤销 Blob。
7. 多行文本选择期间，已加载的选择范围和沿拖动扩展的行保持挂载；在用户主动选择全会话时允许临时全量挂载并在解除选择后恢复。该例外必须由正文区域的真实选择触发，不能截获输入框的全选，也不能作为默认逃避虚拟化的路径。键盘导航按数据索引挂载下一个目标，维护语义顺序、总数和位置；不新增全局搜索产品功能。
8. ArtifactCard、孤立回执、typing 与 WorkspaceTask 都计入布局；悬空回执不能因没有 messageId 对应行而丢失。共享消息列表不变更读写权限、历史保存或附件生命周期。

涉及 `ChatApp.tsx`、`ChatMessageRow.tsx`、`ChatNavigation.tsx`、相关 CSS、`ArtifactCard.tsx`/资源卡片的状态连接，以及新 Timeline hook；根 `package.json` / `bun.lock` 只增加实际需要的依赖，不升产品版本。

### B3 Markdown 解析与界面更新解耦

**决定：保持完整 CommonMark/GFM 语义，解析放入本地 Worker，稳定块复用。此次不承诺通用 Markdown 的严格增量解析。**

引用定义可能改变前文解释，空行并不构成普遍安全的冻结边界。直接把每个段落作为独立文档会破坏引用、脚注、列表和表格。选择继续使用现有 unified/remark/GFM 渲染语义，拆开 parse/transform 与 React render；直接使用的底层包显式声明兼容版本，不借用偶然存在的传递依赖。

1. 提取拟新增 `markdownPipeline.ts` 和 `markdown.worker.ts`。在 Worker 中执行 `remend`、parse/transform，传回结构化 AST/渲染模型；主线程复用现有代码复制、表格、链接与图片替代组件。不传可执行 HTML，不使用 `dangerouslySetInnerHTML`，不启用 raw HTML，不扩大 URL 协议和网络图片权限。现有 `worker-src self/blob` 能容纳本地 Worker，不放宽 CSP。
2. 33 ms 文本收集与首块立即出现继续保持；富文本解析最多每 125 ms 启动一次。单个活动消息只保留一个运行中版本和一个最新待处理版本，不排队解析每一块。Worker 通过 append delta 更新源文本；非追加替换、重连修正时显式 reset，避免每 33 ms 复制全文进入 Worker。
3. 首次 Worker 就绪前先显示原始文字；已有格式快照可以展示其后新增的纯文本尾部，新的解析到达后替换。这样限制格式刷新频率不等于延迟正文可见。只有同作用域且仍为当前文本前缀的旧格式快照可临时使用；非前缀替换后的旧结果直接丢弃。
4. Worker 全文解析后按根节点的源位置和内容指纹复用未变的块模型；主线程只更新改变的块。最终态与流式态使用同一组件树/key 策略，保住代码复制状态、焦点和横向滚动。引用定义确实改变的前文块必须更新，不能为了复用显示过期解释。
5. 正常完成、Stop 和错误收尾均强制最终版本解析，停止 `remend` 补全，保证未闭合 Markdown 的终态与 0.4.22 一致。解析完成不能触发新的模型请求；会话切换/卸载取消订阅和排队工作，迟到结果不能串入。
6. 单窗口最多一个 Worker；优先处理活动/可见消息，离开可见区的预解析可取消。完成消息缓存按内容版本复用，采用有容量上限的会话内 LRU，初始预算 16 MiB，不持久化 AST。Worker 失败只允许一次受控重建；仍失败显示可复制的原始正文并明确格式暂不可用，不静默回到每 33 ms 主线程全文解析。
7. 完整解析本身仍为 O(正文长度)，本轮消除它占据界面线程、逐块排队和稳定 DOM 反复变更的成本。单个巨大段落/代码块也必须满足交互门槛；不能仅拿多短段落演示成功。

涉及 `ChatText.tsx`、`ChatMessageRow.tsx`、`streamText.ts` 的终态接线、新 Worker/调度模块、必要的 Vite/类型测试配置及实际新增依赖。既有安全和多语言组件复用，不引入第二套 Markdown 规则。

### B4 集中联调和交付

B1、B2、B3 在同一源码状态集成后，执行一次限定矩阵；发现由本轮引入的问题就修复并重验对应项和其直接依赖。通过且未再改变的项不重跑。全部 G1–G5 满足后直接整理交付，不再自动扩充场景或开展另一轮全面审计。

现有 B7 的 `initialRows===1000`、`stableOldNodes===1000` 探针不适用于虚拟化：原报告保留，新探针改为检查逻辑消息完整、已挂载且仍需保留的节点稳定，以及导航可到达全部消息。不能通过保留 1000 个隐藏 DOM 或放宽正文校验绕过目标。

## 必须同时满足的完成门槛

| 门槛 | 可判定的通过条件 |
| --- | --- |
| G1 回执工作量与正确性 | 正常打开历史只有原正文快照读取，回执额外全会话请求为 0；普通 idle 为 0。K=0 无状态查询；K 条请求每刷新至多 `ceil(K/50)` 个批量 IPC，每个实体去重按主键读取，无列表 API/无 source snapshot 返回。旧于最近 100 条的目标仍正确恢复 |
| G2 列表规模 | 1000→10000 条历史时，正常视图正文行挂载数不超过可见行数 +16 overscan +明确保留行，导航同样有界；默认测试中正文、导航分别不超过 120 项。聚焦/播放/选择造成的保留单列计数；禁止无理由无限保留 |
| G3 Markdown 调度与语义 | 长流式主线程完整 parse 次数为 0；每消息解析队列最多 1 运行 +1 最新待处理；除首块/终态外启动间隔不少于 125 ms；终态文本与原解析器的结构/安全语义等价，正常 Worker 下 50k 终态格式收敛目标 ≤250 ms |
| G4 交互 | 固定 1000 条历史、10k/50k 负载中输入与滚动第二帧 p95 ≤50 ms，且相对同口径基线不回退超过 20%；记录 p50/p95，p99 仅在 ≥100 个有效样本时解释。Stop 可见可点、焦点与草稿保留、可续聊；阅读锚点在内容重测后稳定偏差 ≤4 CSS px；无本轮同步解析导致的 >50 ms 界面长任务 |
| G5 完整性和兼容 | 收齐各 500 块、最终原始正文 SHA 一致；拒绝/待确认/已提交/已删除状态正确；同 session 不同目录、重访、乱序和断线恢复不串状态。代码复制、表格滚动、链接安全、附件播放/移除、键盘与文本选择、普通/Agent/只读/legacy 路径不退化 |

耗时门槛可能被新实现证伪；失败要定位本轮路径并修复，不能把“基线也慢”当作完成。若发现超出三项目标的独立阻断，记录具体证据和影响，由用户决定是否另开范围；不得把它悄悄加进本次验收，也不得将受阻项写成通过。

## 最小验证矩阵

| 验证层 | 只执行这些相关内容 |
| --- | --- |
| 回执数据 | `useWorklogChat`、`worklogActions`、`WorklogReceipt`；Rust worklog 精确/批量查询和 history 线索投影；覆盖 K、旧任务、缺失/删除、返回乱序、重复通知、目录与视图代次 |
| 列表交互 | Timeline/Navigation、ChatMessageRow、焦点与选择；普通消息、ArtifactCard、回执、Agent 历史、只读、live stream；动态高度/宽度变化、头中尾跳转 |
| Markdown | 复用 `ChatText` 和消息行既有断言；增加 Worker 调度/迟到结果、跨块引用/脚注、巨段/代码围栏、终态不补全、危险 URL/HTML、稳定节点 |
| 静态与构建 | 实施后一次三套 typecheck、一次最终 frontend suite、受影响 Rust history/worklog 测试、React Doctor changed 范围；最终前端与隔离 Windows release 构建。若同一源码已有合格结果直接复用 |
| Windows 集中原生 | 一次连续会话覆盖 1000 条 +10k/50k 流式、Stop/焦点/草稿/续聊、头中尾导航、代码/表格/选择、一个媒体播放与移除、旧拒绝和旧任务回执、目录切换与一次 SSE 恢复；10000 条仅补规模/导航，不重跑全部组合 |

前端定向测试按文件独立进程执行，遵循 `bunfig.toml` 和既有 runner 的 mock 隔离规则，不一次混跑多个依赖 mock.module 的文件。完整前端套件只在最终集成源码上跑一次；后续修复仅使相关证据失效。Rust 不重跑无关宠物/安装/模型矩阵。

所有资料使用合成数据和新的隔离身份。桌面自动化仅在用户留出的独占窗口内使用；已有 Windows MCP 临时会话方式可复用，前台身份异常时停止输入并保留当前进度。计划集中预留一个 30–45 分钟窗口；窗口占用中断是环境阻断，不冒充产品失败或 GUI 通过。

## 交付清单和停止规则

1. 三条实现链及其必要的回归落在独立、可审阅的提交中；清理只清本轮临时进程和测试数据，保留原始失败与修复后证据。
2. 拟新增 `docs/performance-evidence/long-history-closeout-YYYYMMDD/`，固定包含源码/构建身份、G1–G5 对照、工作量与延迟、实际测试数量、已知边界和证据哈希；不复制日常数据库、令牌或私有桌面内容。
3. 更新 `docs/memory-and-worklog.md`、必要的历史/UI 契约、`docs/verification-backlog.md` 和本方案完成状态；不修改冻结的 B7 报告，不重复维护多份轮次估计。
4. 完成判定：三条链实装 + G1–G5 通过 + 无本轮已知正确性回归 + 文档/证据齐全。达到后停止测试扩展并汇报。用户无需逐批批准普通修复；真实缺少硬件、桌面访问或任务范围改变才需要新的输入。
5. 不通过时继续修复失败链，复用仍有效的其他结果；预算或外部阻断只允许如实报告未完成，不能用改口径关闭问题。

工作量估计为一个里程碑、五个内部批次，约 5–8 个有效工程日：B0 半天，B1 1–1.5 天，B2 1.5–2.5 天，B3 1–2 天，B4 约一天。最大不确定性是虚拟化对选择/媒体状态的保护和 Markdown 组件树稳定性；不含用户桌面占用等待、冷缓存编译等待及另行发布。该估计不转换成固定聊天轮数。

## 方案依据

- [0.4.22 交付报告](performance-evidence/b7-windows-closeout-20261028/README.md)、[当前未验范围](verification-backlog.md)、[记忆和日志事实源](memory-and-worklog.md)。
- [TanStack Virtual 官方 API](https://tanstack.com/virtual/latest/docs/api/virtualizer)：动态测量、稳定 key、范围选择与按索引定位适合本列表；具体版本在实施时固定。
- [react-markdown 官方处理链](https://github.com/remarkjs/react-markdown#architecture)：parse/transform/render 可以按统一语义拆分，主线程自定义组件继续保留。
- [micromark 的内容解析说明](https://github.com/micromark/micromark#content)：全局引用定义影响前文，完整语义不能靠空行冻结任意前缀。此次采用有界完整解析与稳定块复用，不另造不兼容解析器。
