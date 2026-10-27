# 当前验收与待办

核对日期：2026-10-27，YUME 0.4.21 / OpenCode 1.18.21。这里集中现行命令及历史记录里尚无完成证据的事项；不是新增功能授权、发布授权或一次新的实机通过声明。各次测试的实际结果继续保存在带基线的原记录。

## 当前验证入口

### 2026-10-27 Mac B7 流式与 PDF

起点 `1add56b`，代码 `3069953`；版本/schema 不变，B7 **进行中**。[最新 HTML](performance-evidence/b7-macos-20261027/index.html)、[完整方法与边界](performance-evidence/b7-macos-20261027/README.md)、[结果与日志 SHA](performance-evidence/b7-macos-20261027/summary.json)。下方历史记录保持各自执行范围。

- 真实 OpenCode 的 text delta 原先被忽略，只有空白占位和最终正文；已接入现有 33 ms 缓冲，已确认 text part 才接受增量，保留请求/会话/Stop 和完整快照替换。新增用例先失败后通过；最终 172 文件 / 1359 pass，类型检查/生产构建/QA App 构建通过。Rust 本轮未改未重跑，上一轮 957 pass / 23 ignored 不算本轮执行。React Doctor 保留 2 条已审阅的既有模式提示。
- 两个独立的 1000 消息会话，500 块、计划 50 Hz，10k/50k 分别 232/212 次 DOM 文本变化；输入/滚动 p95 为 23/33 与 28/38 ms。各 5 次预热 + 30 对保留样本全部在流式中；1000 旧节点、输入焦点、草稿保持。DOM input/scroll 到第二个 rAF，不是物理输入或生产包指标；全部行仍挂载。旧版没有中途显示，不形成有效流式性能对照。
- 全文 native/provider SHA 与 DOM 代码块逐字一致。Stop 中断 162/500 块，native 保留 16,100 字符，继续得到预期回复；现有临时「出错了: Aborted」提示保留，不持久化成额外消息。Stop/继续结果不作为流式 p95。
- 最终二进制 PDF 正常/损坏/超预算/取消各 8 次，worker 32 创建/32 终止，峰值 1，每次归零。走生产附件模块和真实 WebKit 本地 worker/CSP；不是 picker/发送循环。heap 为 null，复杂文档/XFA/堆与 RSS 收敛仍待验。
- 5 个库完整性通过，隔离进程/虚拟凭据/数据根与端口已清理；标准生产三入口构建恢复，QA entry 不在 dist。沙箱 localhost 首次失败与允许监听后的全套通过、首次 CUA 长停顿均保留，不当作产品端口冲突或启动性能。

下一步优先：**Mac GUI 审批、SSE 断流/故障恢复与日志五工具/到期链路**。继续保留 Windows primary/orphan/WebView2、真实模型、PDF/附件重复内存、历史删除/多窗口、记忆导出/忘记/重开、2D/3D/隐藏/睡眠/DPI/高刷/GPU/能耗和同环境生产对照。M19c 外键扫描、小库成本、C16 全行挂载与 R06/D20 复杂度未关闭。

### 2026-10-27 Mac B7 接续

起点 `703c124`，`codex/yume-performance-b6`；最新代码 `dea6d77`，仍为 0.4.21 / 历史 schema 3 / 记忆 schema 4。B7 **进行中**。[最新 HTML](performance-evidence/b7-macos-20261026/index.html)、[完整边界](performance-evidence/b7-macos-20261026/README.md)、[结果与日志 SHA](performance-evidence/b7-macos-20261026/summary.json)。下方 10 月 25 日记录保持当时结果，本节覆盖其中已新增验证的范围。

- `c00528e`：从已有受完整身份约束的 native snapshot 恢复用户 file part 文件名标签，支持仅附件消息；不新增逐消息 HTTP/IPC，不读取 URL/字节，不改变持久 HistoryMessage 或 SQLite schema。去掉路径前缀、忽略无效/重复/synthetic 文件标签，local-only 记录不借用标签。原生 PDF 提取/发送后，历史重开及进程重启均显示文件名，关闭上一轮 PDF 徽标丢失观察。
- `dea6d77`：导航标题使用与气泡一致的资源上下文过滤；自动测试和最终原生 AX 均验证内部 JSON/路径不再出现在导航提示中。
- 最终前端 **172/172 文件，1354 pass / 0 fail / 0 skip / 0 unfinished**；Rust **957 pass / 0 fail / 23 ignored**。固定工具链，套件与测量串行；类型检查、生产 cargo check、前端及 QA App 构建通过。React Doctor 保留 1 条基线已有 `conversationTurns` 导出提示，无规则抑制；13 个 Rust warnings、Vite chunk warning 保留。初次编译/夹具错误、沙箱回环失败和修正后复跑日志分别保存。
- WAV/MP3/MP4 经原生 picker 登记，播放推进、暂停和 DOM seek 到中点通过，发送后重开/重启恢复卡片。WAV 原生播放/暂停控件已操作，物理滑块拖动未完成，不把 DOM seek 写成原生拖动。
- 生产 Tauri 原始 ArrayBuffer 入口在 WebKit 上接收 1/8/20 MiB，20 MiB + 1 B 普通附件按配额拒绝；64 MiB WAV 上传、播放、近末尾 seek 和清理通过。单次耗时 3/8/19/65 ms，不是 p95；不替代 OS 拖入或完整浏览器 File 生命周期。直接 fetch Range 被 CSP 拒绝，策略未放宽；没有取得原生 HTTP 206 回执。上传结果 JSON 明确为 CUA 工具输出转录，后续 AX 保存只得到 no-change diff，不冒称原始导出。
- 官方 OpenCode import 导入 500 轮/1000 消息的合成 Markdown，真实聊天加载。非流式程序化 DOM input/scroll 到第二个 rAF，各 5 次预热 + 30 样本，p50/p95 输入 20/22 ms、滚动 20/21 ms。全部行仍挂载；没有 50 Hz 流式、物理按键到显示或同环境前后对照证据，原 C16 风险不能据此关闭。
- 检查器开启时上传探针曾 SIGTRAP，堆栈位于 WebKit Inspector JSON 序列化；关闭检查器后的延迟探针完成，崩溃原因仍未定。重建后 QA 启动停在读取专属虚拟凭据，停掉核验过的进程并仅移除该条目后恢复。旧端口早期连接拒绝随后恢复；这些异常不覆盖为全程无故障。
- 最终 QA 正常退出、引擎随之结束；前次崩溃孤儿、阻塞宿主和合成服务均按 PID/出生时间核对后停止。5 个 SQLite 库 integrity_check 全为 ok；测试凭据不存在，两个端口关闭，三处专属根按创建时间清理；无日常数据迁移、安装、签名、公证、推送或发布。

该轮后续项（流式与 worker 循环已由上节补充，内存未关闭）：**1000 消息 + 50 Hz 流式 + 10k/50k 代码/表格尾消息**，复核终态、Stop、输入焦点、导航与滚动，再补正常/损坏/超预算 PDF 共至少 20 次循环及取消/堆收敛。Windows、完整 GUI 审批/SSE/故障恢复、日志五工具/到期、历史删除/多窗口、记忆导出/忘记/重开、2D/3D 全动作/隐藏/睡眠/DPI/高刷/GPU/能耗仍待验。M19c 外键扫描、小库成本及 R06/D20 残留复杂度继续保留。

### 2026-10-25 Mac B7

起点 `31e7724`，分支 `codex/yume-performance-b6`，产品 0.4.21；历史 schema 3、记忆 schema 4 均不变。B7 状态为**进行中**：本机完整检查通过，已有真实 OpenCode 与部分隔离 Mac GUI 证据，未满足 Windows/全部原生退出条件。[结果 HTML 与 #01–20 矩阵](performance-evidence/b7-macos-20261025/index.html)、[摘要/日志 SHA](performance-evidence/b7-macos-20261025/summary.json) 是本轮接续入口。

- 三个时钟边界修复单独保存为 `5170451`：automatic worker 首次立即同步、完成后十秒再同步；archive guard 在真实锁内注入观察时钟，测试向前验证 6 秒仍占用/6.001 秒过期；审批保存首次五分钟绝对期限，重复轮询不续期，测试 helper 无需回退单调时钟。新增普通回归验证 helper、迟到回复和后续拒绝，未更改 schema/依赖/产品版本。
- 固定 Bun 1.3.14 / Rust 1.94.1，顺序执行 `bun run typecheck` → `bun run test:frontend` → `cargo test --locked --manifest-path src-tauri/Cargo.toml --lib -- --test-threads=4` → `bun run build`，退出码均 0。最终前端 172/172 文件，1352 pass / 0 fail / 0 skip / 0 unfinished；Rust **953 pass / 0 fail / 23 ignored**，10k 检索预算用例通过。PDF 修复前的初轮为 171 文件 / 1347 pass；最终检查已按同一顺序重跑，日志不覆盖。Rust 源码在两轮之间未变。本轮不单独打印其通过时延，不复用 M19c 的 10/14 ms 冒充本轮计时。
- 原生 WebKit 发送真实 PDF 暴露 `undefined is not a function (near ...i of e...)`：PDF.js 6.2.108 的 `getTextContent` 使用 `for await`，本机 `ReadableStream[Symbol.asyncIterator]` 为 undefined 而 `getReader` 可用。新增局部 reader 兼容，保留 PDF.js XFA 分支、取消、锁释放、字符预算、loading task 销毁及原 worker/CSP；没有升级依赖或修改生产全局原型。修复前回归已复现同一错误，夹具初版误操作 Bun 不可 configurable 的方法也保留日志，改为临时替换 writable value 后完成复现。定向 12 + 4 + 17 项通过；重建 App 后同一 PDF 经 WebKit 提取到预期标记、真实 OpenCode 和本地 provider 后在 GUI 回显。修复单独保存为 `2026f8c`。
- 无 QA feature 的生产 `cargo check` 通过；随后 `bun scripts/audit-remediation/verify-native.ts` 显式执行一项 ignored 真实 OpenCode + 合成 provider 测试，1 pass。归档失败持续 31.258 秒期间审批仍可推进，批准执行、拒绝终结、取消清所有权、恢复不重放及后续新运行通过；provider 初始请求 3 次、恢复 1 次，清理四项均 true。这是 Rust 宿主/真实引擎证据，不是 GUI 或真实模型推理验收；其他 ignored 保持原边界。
- 使用 `worklog-qa`、独立身份 `com.deskmate.unifiedhistoryqa.b720261025a` 和输出目录中的 overlay 构建本机未签名 debug `.app`。生产前端资源装载、资源包检查通过；955 个工作台产物与 H18a 固定上游完整构建逐项 SHA 一致。本轮没有重建未改的 overlay。生产检查/QA 构建均保留 13 个既有 Rust warning，Vite 大 chunk 警告保留；固定工具链无 rustfmt，不记录为格式化通过。
- Mac QA 窗口与桌宠可见；首次/二次工作台打开保持同一会话；原生选择器导入、worker 提取、发送 PDF 并回显预期文本通过；20 秒延迟请求到达 provider 后点击 Stop，目标请求 aborted=true，后续同一会话恢复；从历史列表重开保留三轮文本与模型选择；二次启动进程退出 0，原 primary/引擎 PID 与出生时间未变。取消当下显示“出错了: Aborted”，历史重开后 PDF 徽标未重现，两点保留为待收口观察；不声称全部附件历史恢复通过。原生 HTML select 的自动控制出现 AXError.failure，角色切换未完成，不据此认定产品故障。首次 CUA getApp 阻塞约 1199 秒，只作为工具异常记录，不能算 App 启动性能。
- 十分钟有限范围采样（PDF 修复前 debug 构建）：600.003 秒 / 120 样本，宿主+OpenCode CPU 系统估算 p50/p95 9.1%/12.3%，RSS 1019.125–1026.875 MiB，QA 目录 size/mtime 无变化。默认 GIF 与空聊天可见，设置/工作台隐藏，无并跑构建/测试或模型请求；没有优化前原生基线，排除未归属的 WebKit/GPU，不证明零唤醒/磁盘读取或整机省电。
- 新鲜 QA 数据、钥匙串作用域、sidecar HOME/XDG 与日常 YUME 隔离；QA 禁用自启和全局快捷键。只保存过无实际权限的本地 fixture 占位值，已从独立钥匙串删除。两次 App 与各自 OpenCode 均正常退出，provider/端口及 Support/Caches/WebKit 三个专属目录清理成功；退出回读四个 SQLite 库 integrity_check 均 ok，历史 3/记忆 4。没有读取/迁移日常库、安装、签名、公证、推送或发布。启动日志有一次 SSE connection refused，随后引擎与工作台正常；公开更新 feed 没有当前 darwin 目标而报告无适用平台，未改变 feed 或执行更新。

继续 B7：Windows primary/orphan/并发与 WebView2；Mac 复杂 PDF/取消堆收敛/附件历史/媒体、真实 GUI 权限/SSE/故障恢复、1000 消息输入滚动、日志五工具/到期任务、历史删除/多窗口争用和记忆的导出/删除/重开；2D/3D 全动作、隐藏/睡眠/跨屏 DPI/高刷与原生内存/GPU/能耗前后对照。`system_profiler` 未返回显示器明细，不据此假定实际 Hz/DPI。M19c 完整 DELETE 外键扫描与小库开销、C16 1000 行全挂载、R06 残留锁和 D20 全库一次读取仍保留。默认 60 fps、原版模型不变；缺验证不算“条件实验未采纳”。

### 2026-10-25 Mac B6 M19c

起点 `7673984`，分支 `codex/yume-performance-b6`。记忆库 schema **3→4**，历史目录仍为独立 schema 3，产品仍为 0.4.21；远端核对仍为 `fcb4635`。本批代码与自动回归通过，下一步 B7 联合回归和原生验收。

- 独立迁移 `004_precise_fts.sql` 建立 UUID 唯一映射到 `search_id INTEGER PRIMARY KEY AUTOINCREMENT`，FTS 使用显式 search_id 作为 rowid；不依赖 VACUUM 可改变的隐式 rowid。active/expired/superseded 均保留映射，删除后不复用已提交身份。001/002/003 字节和 SHA-256 不变。
- 只有 id/content/规范化 key/status 的有效变化且涉及 active 时才更新全文。metadata 与无效值更新只写原表，FTS shadow tables 和映射逐项未变；内容/key/状态切换、合法 UUID 改名、重复/唯一约束冲突、REPLACE、事务后置失败回滚、重开/VACUUM/再激活均验证一致性。映射级联删除触发器按 OLD.search_id 定位，不依赖父表 AFTER DELETE 时映射是否已经消失。
- 保留 owned-content trigram 和短词 LIKE；开启持久 FTS `secure-delete=1`，配合原有 core secure_delete、WAL TRUNCATE 和批量 compact。合成 Unicode 哨兵有删除前文件阳性对照；忘记后验证原表、映射、FTS、来源/关联、MATCH 结果、数据库和 WAL 中的完整文本与唯一 trigram 字节消失，批量 persona clear 后其他记忆仍可用。依据与适用边界见[当前契约](memory-and-worklog.md#稳定全文身份与删除2026-10-25-m19c)及 [SQLite 官方说明](https://www.sqlite.org/fts5.html#the_secure_delete_configuration_option)。不是对系统备份或外部快照的擦除承诺。
- 迁移重建来自 memories 权威记录，不信任旧 FTS 副本；故意制造缺失/孤立旧索引后仍恢复完整、无多余全文。升级前后全记录、supersedes/来源/任务/事项/日志/候选/审计数据、公开搜索/导出视图一致，旧 schema 3 关闭后的只读副本及 SHA 保持不变。重建后注入失败、容量不足、未检查点 WAL 下名称冲突都恢复旧表/旧触发器/FTS rowid/版本，修复后可重试；并发打开只产生一套映射。M19a 测试明确隔离运行 2→3，继续验证其原有“只加索引不改 FTS rowid”；现行 0/1/2/3/4 升级和未来版本拒绝由完整版本矩阵覆盖。
- 记忆模块：`cargo test --locked --manifest-path src-tauri/Cargo.toml --lib memory:: -- --test-threads=4 --nocapture`，122 pass / 0 fail / 3 ignored，10k 检索 p50/p95 10/15 ms。真正子进程并发写入后验证原表、映射、FTS 一一对应；原有逻辑到期/候选重试/删除屏障/32767 与 50000 行完整导出继续通过。
- 完整 Rust：`cargo test --locked --manifest-path src-tauri/Cargo.toml --lib -- --test-threads=4 --nocapture`，**951 pass / 0 fail / 23 ignored**，10k 检索 p50/p95 10/14 ms，未改 50 ms 预算。生产 `cargo check --locked --manifest-path src-tauri/Cargo.toml` 退出 0，保留原有 13 warnings。收尾仅补充测试夹具中的 supersedes 关联及 SET NULL 验证，FTS 专项再次通过；生产代码未变。前端、工作台、依赖和锁文件未改变，复用 H18a 已记录基线；未重复无关全套。

规模命令：`cargo test --locked --manifest-path src-tauri/Cargo.toml --lib memory::storage::fts_perf_tests::precise_fts_scaling_benchmark -- --ignored --exact --test-threads=1 --nocapture`，退出 0。固定 Rust 1.94.1 / bundled SQLite 3.53.2 / Mac ARM64 debug，schema 3 与 4 使用独立临时合成库；并非旧应用二进制 A/B。先从真实触发器提取 DELETE 语句，仅把 OLD/NEW.id 替换为绑定参数，执行后回滚；随后按 metadata、content、delete 顺序各执行固定 20 条真实原表 DML。时延为 execute 阶段（不含准备和提交），提交耗时另记日志；p95 为 nearest-rank，未做额外预热或随机化顺序。

| 行数 | FTS DELETE VM steps（旧→新） | metadata p95 ms（旧→新） | content p95 ms（旧→新） | 完整 DELETE p95 ms（旧→新） |
| --- | ---: | ---: | ---: | ---: |
| 1k | 3017 → 26 | 1.105 → 0.004 | 0.835 → 1.257 | 0.814 → 1.217 |
| 10k | 30017 → 26 | 3.086 → 0.004 | 3.101 → 1.058 | 6.394 → 3.803 |
| 100k | 300017 → 26 | 32.143 → 0.006 | 33.070 → 1.389 | 66.161 → 44.787 |

新计划为 `memory_search VIRTUAL TABLE INDEX 0:=` 加 UUID 唯一索引（FTS 计划文字仍写 SCAN，末尾 `=` 表明 rowid 约束）；旧计划没有 `=`，按 memory_id 扫描。固定 20 次新 metadata/content UPDATE 分别为 920/2720 VM steps，三个规模均相同。完整 DELETE 仍有外键检查：实际计划含 `SCAN memories`（supersedes_id）及 `SCAN memory_candidates`（accepted_memory_id）；本夹具候选为空，20 次删除 FullscanStep 仍从 39560 增至 3999560，新版总 VM steps 为 143910/1403910/14003910，不能声称整个删除 O(1)。没有在 M19c 顺带修改这些外键索引。1k 小库 content/delete p95 有约 0.4 ms 的上升；本轮同时新增稳定映射和 FTS secure-delete，未做单项因果拆分，保留作为 B7 真实场景观测项。

复跑 M19b 的 `logical_expiry_scaling_benchmark -- --ignored --exact --test-threads=1 --nocapture` 同样通过：100k 无期限/全未来/20k 集中到期均为零读取写入，最近期限仍为 36/40/40 VM steps；集中到期一轮处理 8 条耗时 5.330 ms，后续显式忘记 42.255 ms。对应 M19b 当时为 1 条/40.800 ms；这是单批测量，不是 p95 或严格延迟保证。100k 检索本身仍扫描/排序，本轮三个场景 p50 约 99–116 ms。

本机证据：`output/performance-b6-m19c-20261025/` 中 `fts-tests.log`、`fts-final-tests.log`、`memory-tests.log`、`scaling.log`、`expiry-scaling.log`、`rust-full.log`、`cargo-check.log`、源码快照与汇总。新增 ignored 规模测试和 M19b 规模测试均已单独执行；其他 ignored 仍按原边界保留。所有库为临时合成数据，未迁移日常用户库、安装、推送、签名或发布。B7 保留 Windows/macOS 实机、真实 IPC/窗口、CPU/GPU/耗电、完整删除的外键成本与三个既有时钟边界，不因本轮自动回归关闭。


### 2026-10-25 Mac B6 M19b

起点 `73b254b`，分支 `codex/yume-performance-b6`。本批完成读取逻辑过期与独立维护；历史目录、记忆库各自 schema 仍为 3，产品仍为 0.4.21。远端核对仍为 `fcb4635`，从本地 B6 接续；下一步 M19c，B7 与平台实机验收保留。

- 列表/导出及分块关联使用 DEFERRED 只读快照，聊天检索单条 SELECT；复用已有连接，普通写事务仍为 IMMEDIATE。冻结时间下验证 active/expired/superseded、persona/global、NULL/边界/偏移量/小数 TEXT 比较、立即/远未来到期、重开恢复、FTS/短词 LIKE、来源/任务关联与完整导出；维护前后记录、顺序、updated_at/revision 一致。FTS 保持 active-only，显式 expired 的短词历史搜索保留；没有新增物理 status 计数 API。
- 后台每轮通过两个索引的最近期限判断是否有到期项；最多 8 条记忆/32 条 pending 候选，记忆逐条处理后检查 10 ms 时间预算。状态/连接忙时跳过，跨进程写锁竞争零等待让出并恢复 5 秒普通超时；失败整批回滚重试。候选单独到期也会触发清理，accepted/dismissed/未来候选不变。新建同 key 只释放对应过期槽位；编辑已到期项不会复活；自动提议目标和日志关联拒绝逻辑过期记录。
- 物化只改变 status，保留最后编辑时间与 revision，明确替代旧“首次读取时更新 updated_at”的副作用。读路径在 SQLite query_only 模式和另一连接持有 IMMEDIATE 写锁时成功，读取未提交写之前的一致内容；真正子进程在父进程持有只读快照时成功提交，父进程多表快照保持一致，结束后读到新数据。维护间隙的显式忘记、来源/FTS 删除和既有隐私、跨进程竞争、32767/50000 行导出测试均通过。
- 首次专项编译失败于新增测试用 `usize` 读取 SQLite count（不支持 FromSql）；改为 i64 后到期专项 8 pass / 0 fail / 1 ignored。随后记忆模块 111 pass / 0 fail / 2 ignored，10k 检索 p50/p95 11/14 ms。这些专项发生在后续维护时间预算收紧之前；最终状态由完整回归覆盖。
- 最终 `cargo test --locked --manifest-path src-tauri/Cargo.toml --lib -- --test-threads=4 --nocapture`：**941 pass / 0 fail / 22 ignored**，10k 检索 p50/p95 11/16 ms，保持原 50 ms 预算。首次完整回归同为 941 pass / 0 fail / 22 ignored（10/14 ms）；收尾恢复 superseded 状态参数复用枚举转换方法以消除新增闲置方法 warning 后再次完整通过。新增 1 项 ignored 规模测试已单独运行，其他 ignored 仍按原边界保留。M19a 的一次 95 ms 异常及当时复测记录保留，不用本轮通过宣称原生尾延迟稳定。
- 生产 `cargo check --locked --manifest-path src-tauri/Cargo.toml` 最终退出 0，保留原有 13 warnings；迁移 001/002/003、前端与锁文件未修改。

规模命令：`cargo test --locked --manifest-path src-tauri/Cargo.toml --lib memory::repository::expiry_index_tests::logical_expiry_scaling_benchmark -- --ignored --exact --test-threads=1 --nocapture`。固定 Rust 1.94.1 / Mac ARM64 debug，独立临时合成库，各 100k 行；每场景顺序采样 30 次检索，包含首次调用，无额外预热。下表 p50/p95 使用 nearest-rank；维护和忘记为单次测量，不是 p95 或 GUI 延迟。

| 场景 | 逻辑到期数 | 读取写入次数 | 最近期限 VM steps | 检索 p50/p95（ms） | 单轮维护（ms / 条数） |
| --- | ---: | ---: | ---: | ---: | ---: |
| 无期限 | 0 | 0 | 36 | 105.490 / 151.506 | 0.062 / 0 |
| 全未来 | 0 | 0 | 40 | 117.387 / 173.320 | 0.057 / 0 |
| 集中到期 | 20000 | 0 | 40 | 101.833 / 172.820 | 40.800 / 1 |

最近期限查询 FullscanStep 为 0，实际维护候选使用 `idx_memories_active_expiry`，无到期不进入写事务。初版 8 条固定批次在同规模集中到期场景耗时 254.388 ms；据此增加逐条 10 ms 让出预算，复测单批只处理 1 条，后续显式忘记单次 69.046 ms（含现有 FTS/检查点）。单条 FTS 触发器不能被时间预算打断，仍可能超过 10 ms；积压分多轮清理，worker 处理自动任务时会推迟下一次维护机会。M19c 继续处理 FTS 全扫描；100k 检索扫描、排序本身也仍随数据量增长。本轮不是旧二进制 A/B、严格时延上限或原生体验保证。

本机证据：`output/performance-b6-m19b-20261025/` 下 `expiry-tests.log`（首次编译失败）、`expiry-tests-recheck.log`、`memory-tests.log`、`scaling.log`（初版固定批次）、`scaling-time-bounded.log`（最终专项）、`rust-full.log` / `rust-full-final.log`、`cargo-check.log` / `cargo-check-final.log`、源码哈希与汇总。普通检查未更改固定工具链；本机未装 rustfmt，格式化命令未执行成功，不作为通过项。前端/工作台/依赖未改变，复用 H18a 已记录基线；所有数据库为合成临时库。未迁移日常用户库、安装、推送、签名或发布；Mac WebKit/真实 IPC/窗口、Windows、原生 CPU/GPU/耗电与既有三个时钟边界仍待验。


### 2026-10-25 Mac B6 M19a

起点 `c52379c`，分支 `codex/yume-performance-b6`。本批完成 M19a，记忆库 schema 2→3；历史目录仍为其独立 schema 3，产品仍为 0.4.21。下一步 M19b，M19c/B7 未实施。

- 新增独立迁移 `003_active_expiry.sql`：`idx_memories_active_expiry ON memories(expires_at) WHERE status='active' AND expires_at IS NOT NULL`。001/002 与现有 FTS、候选、来源/事项/任务/日志关联定义未改。生产到期 UPDATE 仅提取为常量供真实查询计划验证，谓词和调用时点保持原样；M19b 的读事务拆分尚未进行。
- A04 仍为全量导出与每批 400 参数取关联；保留 active/superseded/expired、NULL、等于边界、未来、ISO 偏移量及小数表示的原 TEXT 比较。实际 `expire_due` 在索引库与无该索引的对照库结果一致，FTS 和 pending 候选清理一致；候选删除失败会回滚记忆状态、索引成员和 FTS。另一 persona 的到期记录仍按既有全局维护处理，但不会出现在当前 persona 的列表中。
- 迁移用关闭连接后的 schema 2 副本做只读对照及 SHA-256 检查，比较记忆、来源/关联、FTS rowid 和自动状态；重复打开不重写数据。索引创建后注入后置迁移失败、SQLite 容量上限、未检查点 WAL 中的 schema 名称冲突均保留旧库和版本，修复夹具后可重试；四线程并发打开只完成一次升级。原有测试扩展至 0/1/2/3 的所有已支持版本，未来版本仍在改变持久 journal 设置前拒绝。
- 专项：首次 `cargo test --locked --manifest-path src-tauri/Cargo.toml memory:: -- --test-threads=4` 为 103 pass / 1 fail / 1 ignored，唯一失败是新增测试把 `USING INDEX` 写死，未识别实际合法的 `USING COVERING INDEX` 文本；修正后 `cargo test --locked --manifest-path src-tauri/Cargo.toml expiry_ -- --test-threads=4` 为 7 pass / 0 fail / 1 ignored。仍要求命中准确的范围索引，不放宽为任意计划。
- 完整 Rust 首次 4 线程运行 932 pass / 1 fail / 21 ignored：原有 10k 检索用例 p95 95 ms，超过 50 ms 预算。未改代码或预算，独立运行该用例为 p50/p95 9/20 ms；随后原 4 线程全套复测为 **933 pass / 0 fail / 21 ignored**，检索 p50/p95 10/15 ms。首次时延异常原因未进一步定位，失败日志保留，不能用复测结果证明原生尾延迟稳定。两次完整命令为 `cargo test --locked --manifest-path src-tauri/Cargo.toml --lib -- --test-threads=4`，复测另加 `--nocapture` 保存通过时的计时结果。
- `cargo check --locked --manifest-path src-tauri/Cargo.toml` 退出 0，保留既有 13 warnings。完整回归包含 32767/50000 条 A04 导出、跨进程迁移/竞争、检索预算及隐私删除。前端、工作台、依赖和锁文件未变，复用已记录的 H18a 前端/工作台基线。

规模命令 `cargo test --locked --manifest-path src-tauri/Cargo.toml expiry_index_scaling_benchmark -- --ignored --nocapture` 退出 0。五个 100k 合成库的真实 UPDATE 均计划为 `SEARCH memories USING COVERING INDEX idx_memories_active_expiry (expires_at>? AND expires_at<?)`。以下仅测同谓词候选 ID 定位，与强制 `NOT INDEXED` 的扫描 oracle 比较，结果逐项相等；不是完整到期更新或旧生产二进制测速。固定 Rust 1.94.1 / Mac ARM64 debug，预热 3 次、采样 30 次，nearest-rank p50/p95：

| 场景 | 命中数 | 索引 / 扫描 VM steps | 索引 p50/p95（ms） | 扫描 p50/p95（ms） |
| --- | ---: | ---: | ---: | ---: |
| 全部无期限 | 0 | 9 / 500008 | 0.002 / 0.003 | 18.180 / 19.165 |
| 全部未来到期 | 0 | 13 / 700008 | 0.003 / 0.003 | 23.215 / 24.275 |
| 少量到期 | 5 | 37 / 500028 | 0.006 / 0.008 | 18.282 / 19.721 |
| 集中到期 | 20000 | 100013 / 740008 | 12.339 / 13.721 | 31.639 / 32.578 |
| 其他 persona 到期 | 5 | 37 / 500028 | 0.006 / 0.006 | 18.082 / 18.832 |

两种无到期场景还执行了真实 UPDATE，分别为 17/21 VM steps、0 行变更。大规模实际到期 UPDATE 的 FTS 触发器总成本未在本批测量；其结果一致性由小库真实更新和既有回归验证，不能宣称整条到期链路恒定耗时。FTS 精确维护留给 M19c。

证据目录 `output/performance-b6-m19a-20261025/`：`memory-tests.log`、`expiry-tests.log`、`scaling.log`、首次 `rust-full.log`、`retrieval-isolated.log`、`rust-full-recheck.log`、`cargo-check.log`、源码指纹及 `summary.json`。所有数据库为隔离合成库，未访问/迁移日常用户库，未安装、推送或发布。旧版记忆 schema 门禁拒绝新版库；原生 Mac/Windows GUI、IPC、真实模型与整机性能待验仍保留。

### 2026-10-25 Mac B6 H18c

起点 `62ae45d`，本地分支 `codex/yume-performance-b6`。本批完成 H18c，历史目录 schema 2→3；产品仍为 0.4.21。下一步 M19a，M19b/c 和 B7 未实施。

- 新增独立 `entry_projection` 和分页/目录/来源/离线索引。迁移逐行校验旧 metadata 并回填，原 JSON 字节、模型偏好、目录和最近工作区保留；DDL、回填与版本同事务提交。单条/批量变更同步维护投影，物理移除同步清理。直接 SQL 写 metadata 留下未验证标记，列表明确报错，避免使用旧投影或跳过损坏数据。
- SQL count、分页、全局 offline 和目录在同一 DEFERRED 读快照内；只为返回页解码 metadata。Unicode 小写由 Rust 生成，SQL `instr` 保留字面子串，含中文、希腊 sigma、İ、%/_、反斜线和 NUL；8 字节大端时间戳保留全部 u64 顺序。默认归档/空记录/墓碑/置顶/完整 key 排序和大 offset 边界与旧 oracle 相同。工作台租约仅在返回页派生能力，不写入投影；窗口授权及操作时 live guard 保留。
- schema 回归：保留关闭连接后的独立 schema 2 只读副本及 SHA-256；重复打开、四线程并发打开、未来版本拒绝；第 50 行注入中断、SQLite `max_page_count` 模拟容量不足、非法 JSON/错 key 均回滚；投影写失败不提交 metadata；WAL 读快照期间另一连接归档、墓碑、物理删除和注册目录后，旧快照仍一致，新请求读取新状态。中断是确定性错误注入，不冒称真实进程崩溃或磁盘拔出。
- 自动回归：历史专项 `cargo test --locked --manifest-path src-tauri/Cargo.toml history:: -- --test-threads=4` 为 157 pass / 0 fail / 3 ignored。随后完善 WAL 删除用例后，完整 `cargo test --locked --manifest-path src-tauri/Cargo.toml --lib -- --test-threads=4` 为 **926 pass / 0 fail / 20 ignored**，退出 0。生产 `cargo check --locked --manifest-path src-tauri/Cargo.toml` 退出 0，保留既有 13 warnings。首次编译因 rusqlite 不支持直接读取 `usize` 失败，改为有检查的 i64→usize 转换后通过；日志保留。
- 规模专项 `cargo test --locked --manifest-path src-tauri/Cargo.toml sql_paging_scaling_benchmark -- --ignored --nocapture` 退出 0；1k/10k/100k 各 97 组过滤/搜索/时间/分页查询与旧全量 Rust oracle 完全一致，另测首页、深页、搜索、目录、来源五种形态。正常套件还逐页遍历 1007 行夹具。50 行页均只解码 50 行；EXPLAIN 分别命中 `catalog_page_order`、`catalog_directory_order`、`catalog_source_order` 和 metadata 主键。旧全量算法只编译到测试中。

规模测量串行，在固定 Rust 1.94.1、同一 Mac ARM64 的 debug 构建中每种形态预热 3 次、采样 30 次，p50/p95 使用 nearest-rank；计时包含连接、计数、离线/目录查询和页读取，不含来源恢复或真实 IPC。10万行结果：

| 形态 | 解码行数 | page VM steps | count VM steps | p50 / p95（ms） |
| --- | ---: | ---: | ---: | ---: |
| 首页 50 行 | 50 | 869 | 196935 | 6.752 / 8.444 |
| offset 50000 | 50 | 150869 | 196935 | 10.010 / 10.758 |
| 子串「标题」 | 50 | 3782 | 572029 | 103.987 / 157.866 |
| 目录过滤 | 50 | 871 | 67148 | 3.775 / 4.184 |
| 来源过滤 | 50 | 871 | 73862 | 3.736 / 4.656 |

首页 page VM steps 在三种规模均为 869，但 count 从 1980 增至 196935；深 offset 的 page steps 从 2369 增至 150869。搜索仍扫描投影以计算完整匹配数；这些数据不支持整个请求恒定工作量、稳定原生 p95 或 GUI 已流畅的结论。保留首次单样本测量 `scaling.log`，最终 30 样本及查询计划见 `scaling-final.log`。

证据目录 `output/performance-b6-h18c-20261025/` 包含首次失败/复跑 history 日志、`rust-full.log`、`cargo-check.log`、规模日志、`source-snapshot.json` 和 `summary.json`。没有改前端、工作台、锁文件或依赖，复用 H18a 的前端及固定工作台基线；未重复运行无关前端检查。数据库均为临时合成库；未触碰日常用户库、安装、推送或发布。schema 3 不通过改版本号降级；旧版本门禁拒绝新库。真实 Mac/Windows GUI、IPC、多窗口争用、显示器与 GPU/功耗门槛仍待 B7。

### 2026-10-24 Mac B6 H18b

起点为 H18a 本地检查点 `d877ba5`，分支 `codex/yume-performance-b6`。本批完成 H18b；schema 保持 2，产品版本保持 0.4.21。下一步 H18c，M19/B7 未实施。

- 单条目录编辑使用完整主键和 IMMEDIATE 事务，只解码目标行；注册、重命名、置顶、归档、内容证据、删除回执、自动清理已接入。批量来源导入与目录发现保留原子全量路径。未改变的条目不执行 SQL 写入。
- 事务内验证规范化身份且禁止换 key，保留跨目录/sidecar 同 session ID 隔离。提交时重验当前墓碑/可用性/runtime，旧检查结果不覆盖较新的运行/所有权证据；内容证据响应保护并发保存、删除、运行、不可用状态。空会话清理与删除回执保留已有 requested_at，不复活缺失条目。
- 物理移除条目及 model selection 删除同事务，偏好写入的存在性检查也纳入写事务，避免与条目移除交错留下孤立偏好。来源关联 revision 仅在成功提交的 native 新增/移除/来源变化后递增。回归注入闭包错误和 SQLite 触发器写失败，验证条目、偏好与 revision 不发生部分提交。
- 损坏边界明确：单条路径沿用既有 `get` 策略，目标损坏拒绝，无关损坏行原字节保留；`all()` 和批量 `update()` 仍执行完整验证并报错。全局身份唯一性仍由批量重复检查和 SQLite 主键约束保障，不跳过损坏或静默修复。
- 历史专项：`RUST_TEST_THREADS=4 cargo test --locked --manifest-path src-tauri/Cargo.toml history::`，145 pass / 0 fail / 2 ignored；此后补充的空会话回执及较新活动时间下的运行状态保护测试纳入完整套件。最终 `cargo test --locked --manifest-path src-tauri/Cargo.toml --lib -- --test-threads=4`：916 pass / 0 fail / 19 ignored，退出 0。包含既有 ownership、staged snapshot、WAL、migration/rollback、native unavailable 及跨窗口 guard 回归；未运行 ignored 原生用例。
- 规模测量：`cargo test --locked --manifest-path src-tauri/Cargo.toml single_entry_scaling_benchmark -- --ignored --nocapture`，退出 0。在 1k/10k/100k 合成库各执行 100 次置顶切换，分别均为解码 100 / UPDATE 100，计划均为 `SEARCH entries USING INDEX sqlite_autoindex_entries_1 (key=?)`。三次总耗时 75.5265 / 156.539458 / 51.47075 ms，仅为同机 debug 合成单样本，不作为稳定延迟预算或原生加速比。
- `cargo check --locked --manifest-path src-tauri/Cargo.toml` 退出 0，保留既有 13 条 production warnings。本轮没有改前端/工作台、锁文件或依赖，前端检查与固定工作台输入复用上一节 H18a 基线记录，未声称重新执行。测试和测量串行，所有数据库均为临时合成库。

证据目录：`output/performance-b6-h18b-20261024/`，含 `history-tests.log`、`scaling.log`、`rust-full-final.log`、`cargo-check-final.log`、`summary.json`，并保留收尾前的 `rust-full.log` 与 `cargo-check.log`。仍有全量列表、候选枚举、批量导入和低频来源恢复；单条存储优化不等于整个端点恒定耗时。Mac WebKit/真实 IPC/工作台争用实机、Windows 回归、Hz/DPI/GPU/耗电仍待验。未安装日常应用、迁移用户库、推送或发布。

### 2026-10-24 Mac B6 H18a

起点 `fcb4635`，本地分支 `codex/yume-performance-b6`，产品版本仍为 0.4.21。本批只完成 H18a，H18b/H18c/M19/B7 未实施。

- Mac ARM64、macOS 26.5.2（25F84）、Command Line Tools SDK 26.5；项目内固定 Bun 1.3.14、Cargo/Rust 1.94.1、Node 24.12.0。`bun install --frozen-lockfile`、`bun run prepare:sidecar` 成功；本仓锁文件未修改。
- 基线：`RUST_TEST_THREADS=4 bun run check` 退出 0，三套 TypeScript、前端 171/171 文件（1347 pass / 0 fail / 0 skip / 0 unfinished）、Rust 895 pass / 0 fail / 18 ignored、生产前端构建通过。该次检查发生在 H18a 修改之前；本轮没有修改前端源码。
- 工作台：官方 codeload 固定 commit 源码归档，冻结安装依赖，再 `bun run prepare:workbench` 退出 0；安装前后上游原始锁文件哈希一致。Git 克隆曾两次 TLS 失败，源码归档不等同于 Git checkout。保留既有大 chunk 警告。
- H18a：成功提交后递增来源 revision；历史/index/run 缓存命中不全量复制/读取/序列化/哈希。新 native 身份或来源关联触发重导入；pin、归档、墓碑与实时工作台租约不借用来源缓存做授权。Agent 流式 ID、用量回执及 pending finish 不触发无关全量导入。
- 新增/扩展回归覆盖正文十倍增长下的 100 次命中、导入失败重试、导入期间并发提交、外部源恢复/重启、关联事务回滚、删除墓碑、实际历史文件替换失败、不变快照及迟到投影拒绝、Agent 写失败及终态失效、旧 index 损坏保留内存。最终完整 Rust：`cargo test --locked --manifest-path src-tauri/Cargo.toml --lib -- --test-threads=4`，901 pass / 0 fail / 18 ignored，退出 0。
- H18a 生产配置 `cargo check --locked --manifest-path src-tauri/Cargo.toml` 退出 0；首次沙箱执行因未缓存 crate 的 DNS 下载失败，保留日志，授权下载后重跑成功。不是签名包或 GUI 启动验收。
- 失败回执保留：第一次 history 定向回归因沙箱禁止回环端口而失败；授权复跑后剩一个新增夹具的 `/var` 与 `/private/var` 路径规范化断言失败，改用产品规范化入口后完整 Rust 回归通过。未放松身份或删除断言。
- 来源恢复校验在首次读取以及 30 秒间隔到期后的下一次读取发生，完整来源导入本身仍是全量工作。列表 `store.all()`、单条 mutation 的全表解码尚未优化，留给 H18b/H18c；不能声称历史列表完全 O(1) 或已经原生流畅。

本机证据：`output/performance-b6-preflight-20261024/` 中 `baseline-check.log`、`workbench-build.log`、`h18a-history*.log`、`h18a-rust-full.log` 及汇总。仍待：Mac WebKit/真实 IPC/窗口所有权实机、Windows 回归、显示器 Hz/DPI 与 GPU/耗电测量。没有执行日常安装、迁移真实用户数据库或发布。

### 2026-10-24 性能整改 Mac 交接

[完整施工计划](performance-remediation-plan.md) 与 [Mac 接续说明](performance-remediation-handoff.md) 已纳入仓库，原 Windows 外部计划冻结。B1–B5 代码已进入远端 main，最终发布源码 `fee4d707804f76e5c608ceef33c622e71d17b175` / 0.4.21；B6 的 H18/M19 迁移和 B7 联合验收未开工。接手从最新 main 核对后开始 H18a，不从旧审计版本重做。

- 2026-10-23 最终发布 CI 六个 job 及 main Source checks 通过，固定上游工作台已完整重建。Windows 前端 1341 pass / 8 skip、Rust 904 pass / 19 ignored；Mac 前端 1347 pass / 0 skip、Rust 895 pass / 18 ignored；两平台均 0 fail、前端 0 unfinished。逐项回执见 [便携证据](performance-evidence/README.md)。这补充下方 B5 本地阶段“复用工作台”的历史记录，不改写原阶段覆盖范围。
- Windows 0.4.21 已公开并完成包内容、Tauri 签名和匿名下载核验；未安装、未跑 GUI 跨版本升级，Authenticode 未签名。Mac 仅构建 unsigned 候选，未公开新包。所有 Windows/Mac 原生缺口继续保留。
- 待独立处理的时钟边界：automatic worker 初始化 `now - 10s`；workbench archive guard 测试 `now - 6.001s`；仅 ignored 权限夹具 `checked_sub(300s)`。来源、文件和所需回归见接续说明，交接未修复这些代码。
- C16 长聊天仍需原生输入/滚动尾延迟测量；R06c 仍有锁内关键落盘/结算 HTTP；D20 仍单次全库读取。B6 要在一致性备份的隔离库上完成迁移、失败恢复、并发、墓碑与真实隐私擦除，Mac 通过不能关闭 Windows 待验。

### 2026-10-22 性能整改 B5

基线为 B4 本地提交 `5ef42d8e69c002fc1446e7a1ed943c658bd139a5`，基线版本 0.4.20；
本批按追加发布授权将候选升为 0.4.21，
开发入口及 `codex/yume-performance-remediation` 不变。按用户“下一批”授权实施。

- R06c：旧投影准备退出控制锁，提交前重验完整运行、引擎代际、真实工作台租约、历史基线和墓碑。
  30 个合成样本的控制锁取得等待 p95 从 25.5948 降至 0.0022 ms；旧锁范围为重建对照，
  并非旧生产二进制或原生 Stop 测量。关键落盘及既有 settlement HTTP 仍保留。
- R06d：复用受认证全局 SSE，仅合并失效 ID，以完整单消息 GET 刷新；终态/重连/溢出等全量，
  连接正常时 30 秒全量兜底。断线期间保留每轮全量检查，轮次间等待 1 秒；30 秒不是硬完成时限。
  完整身份、实际代际、权限 fallback 目录及迟到 ack 受保护，缓存不能直接触发破坏性结算。
  真实回环 HTTP 合成 60 个逻辑秒、1000 消息/10000 旧运行：有变化时响应体 13,073,510→448,302 B，
  消息 GET 仍 60；无变化时 GET 60→2。协议头、SSE/权限流量、CPU及原生延迟另计。
- C16：首个非空内容立即，后续 33 ms 合批；终态、工具和已确认 Stop flush，切换/卸载隔离旧队列。
  原生 part 恢复按 message 归组，修复重复行及迟到 Stop 污染；稳定消息行和头像引用，日志按 ID 查找。
  原 Markdown 安全规则与固定人格台词时序保留，不引入虚拟列表。
- D20：一次加载来源及快照，计算传递闭包，只写变化版本；独立快照、深链和运行记录参与擦除。
  删除同周期报告时取消既有再生成，迟到发布不能复活；新的显式生成仍可进行，失败整体回滚。
  10k 无关 + 20 相关版本的合成 SQLite 对照：版本/运行扫描各 21→1，无关版本 UPDATE 210,000→0，
  JSON 解析按 trace 行数计算 420,924→20,044；仍需读取全部快照，FK成本另计。
  取消立即撤销发布/续租资格，真实模型请求仍由 heartbeat 感知，不声称 HTTP 即时中止。

整批源码指纹、检查、失败/复验、浏览器记录及本地保存回执集中在
`E:/Codex/工作伙伴/reports/yume-performance-b5-20261022` 的 `verification-summary.json`、
`browser-validation.json`、`source-snapshot.json`、`b5-checkpoint.json`、`review.json` 与 HTML 报告。
专项通过不替代最终完整套件；浏览器 profiling/mock IPC 不等于原生 WebView2，Windows debug仅证明编译链接。
工作台及三个生产模型按 B4 产物哈希核对复用，未重建上游工作台或采纳资产候选。

B1–B5 原生待验继续保留：Windows 实际 SSE/权限/重启/Stop、长会话输入与滚动、磁盘和内存压力、
多窗口所有权、原生 IPC/显示器/DPI、GPU/功耗及 macOS。未执行日常安装；本节为发布前验证记录，
后续 Windows 发布范围见 [v0.4.21 发布说明](release-v0.4.21.md) 及独立公开回读回执。B6 数据结构与迁移尚未实施。

### 2026-10-22 性能整改 B4

基线为 B3 本地提交 `6f97c93b984ced6e7c36e2e1d15b9b84c8dc3994`，0.4.20，
开发入口及 `codex/yume-performance-remediation` 分支不变。按用户“下一步”授权继续实施；
B1–B3 原生待验保留，旧 0.4.3 主仓未实施功能或构建。

- F17：普通/NCM 与媒体 File 以顶层 ArrayBuffer 走 raw Tauri IPC，元数据头受大小限制；
  Rust 检查窗口、类型、长度和业务上限，旧 JSON 上传入口移除，写盘进入 blocking worker。
  uploadId/scopeKey 只关联生命周期，不替代目录授权；切换、移除、卸载和迟到响应均有隔离与清理。
  图片 data URL 复用严格校验结果，Blob 预览在成功/失败/取消/移除时释放。Rust 仍有一次字节复制。
  安装的 Tauri 2.11.5 serializer 确认 ArrayBuffer 身份保留；20 MiB 周期字节样本旧 JSON body
  74,874,991 B → raw body 20,971,520 B，元数据头另计。旧完整字节量为已验证的精确推算，
  不是原生 wire、峰值堆/RSS 或端到端耗时实测。
- P11：rig2d 默认保留 60 fps，deadline 余数避免高刷新率降成 48 fps；无睡眠叠加/过渡时少一次画布复制。
  确定性 5 秒 RAF 模拟中，144/240 Hz 渲染从 721/1201 次降至 301 次，动画时间仍为 5 秒。
  30 fps idle/sleep 仅为显式测试候选，未进入默认路径；上述次数不是实际 CPU/GPU/耗电收益。
- P12：位置/DPI 单次在途读取并缓存，move/resize/scale 失效，旧请求不可覆盖新几何；
  原生成功 show/hide 广播物理可见性，隐藏时停止 rig2d/GLB 渲染循环及三种宠物的光标轮询，恢复时刷新几何。
  真实产品 PetApp 的 Chromium 夹具使用模拟 Tauri 事件：三种宠物隐藏约 2 秒时光标/几何读取为 0，
  rig2d Canvas2D 与 GLB WebGL draw 调用为 0；恢复和移动后重新读取。番茄钟仍查询，GIF 解码未测，
  不声称所有 IPC 或整个宿主后台工作为零。
- P13：受信任的生成包络按实际模型 SHA、算法/Three 版本、scale 和 clipRoots 匹配；命中省去 175 次扫描，
  未知/变更模型按原生产算法回退，16 项 session LRU 仅保存数值记录。采样异常也恢复姿势及原边界引用。
  显式 `bun scripts/prepare-personas.ts --envelopes-only` 重建包络，普通 fetch 流程不变。
  同机暖 Bun CPU、3 次预热/30 次样本：旧扫描 p50/p95 1476.973/1569.177 ms；新 lookup/framing
  0.168/1.604 ms。75 MB 身份 SHA/JSON 验证另计 p50 45.01 ms，暖文件读取 p50 22.27 ms；
  不包含 GLTF 解析、纹理解码、GPU 首帧或原生冷启动。698 个 authored keytime 的镜头检查通过，
  不替代连续极值和鼠标跟随的原生视觉验收。
- P14a：隔离 1024/512 贴图候选为 46,729,848 / 39,181,072 B，较原版减少 38.22% / 48.20%；
  979 个非图片 bufferView 字节与动画/网格等结构保持。浏览器 3 模型 × 2 尺寸 × 7 动作共 42 组采样通过。
  P14b 决策仍为保留原版 75,641,656 B 模型；候选不进入生产资源，未引入 KTX2 或新依赖。

整批检查、源码 SHA、失败与复验记录及最终本地提交见本机
`E:/Codex/工作伙伴/reports/yume-performance-b4-20261022` 的 `verification-summary.json`、
`browser-validation.json`、`source-snapshot.json`、`b4-checkpoint.json`、`review.json` 与 HTML 报告。
工作台输入/产物沿用 B3 并逐文件比对，不称本批重新构建上游工作台。正常 Windows debug 构建仅证明编译链接，
不代表可分发包、原生 GUI 或已安装程序验收。未安装、推送或发布。

待验：Windows WebView2 原生 raw IPC 与真实文件选择/拖入/发送/历史恢复、取消/迟到清理及媒体编解码；
多 DPI 移屏、托盘/快捷键隐藏恢复、睡眠/拖动锁、60/144/240 Hz 连续动作、原生默认/最大尺寸视觉，
真实峰值堆/RSS、GPU 内存/功耗及 macOS。30 fps 与候选模型只有相应验收通过后才采纳。
B4 收尾时 B5 尚未实施；后续进展见上节 B5 回执。

### 2026-10-22 性能整改 B3

基线为 B2 本地提交 `19630b6a69d5944c53156bf666879cf3afd7cf0c`，0.4.20，
开发入口与 `codex/yume-performance-remediation` 分支不变。按用户“下一步”授权完成以下源码及验证；
B1/B2 原生待验继续保留。未在旧 0.4.3 主仓实施功能或构建。

- R06a：60 次相同的 1,000 消息快照，RunRecord 序列化、sync、rename 各从 60 次降至 0；
  真变化仍先持久化再发布 RAM/事件，落盘失败不提前发布。
- R06b：10,000 个旧记录加一个目标运行的隔离场景，归档从每轮读取 10,001 个记录变为 1 个；
  60 轮共 60 次目标读取，没有新增 RunRecord 写入。复用快照用量，缺失消息 ID 随运行记录持久保存，
  终态/重启可补偿；32 项窗口公平遍历完整 RAM 记录。缺失 usage 的补偿 HTTP 在独立单 worker 中，代际和完整身份校验后提交，
  用量账本 sync/rename 完成后才清待补偿标记。仍保留 1 Hz RAM 检查，不声称整个宿主无空闲唤醒。
- I07a：Windows 真实 OS 文件通知、原子 rename、丢通知实际 10 秒补扫、目录重挂与 junction 拒绝通过；
  五个工具覆盖真实文件→受控认证 HTTP→仓库→原子回执。独立 600.09 秒真实空闲对照，
  旧空目录循环枚举 5959 次，新通知/补扫路径 60 次，唤醒 60 次。
  仅为同机隔离目录操作数，非 Tauri 整机 CPU/GPU/耗电；在途串行 HTTP 可延长请求处理时间。
- I07b：最近计划/重试/租约期限唤醒，配置、恢复和退出通知可打断，60 秒兜底；
  10 次不变 next_due 检查从写 10 行变为 0。600 虚拟秒真实 SQLite trace 为 10 轮、30 SELECT、
  20 BEGIN IMMEDIATE、0 改行；仍有 10 条无命中的租约检查 UPDATE，不等于完全无 SQL 写语句。
- S08a：启动不预建工作台，首次打开串行创建；会话目标先保存，页面监听和路由就绪后领取，
  实际路由后认领输入，隐藏保留窗口。已从固定 OpenCode 1.18.21 提交 `826d9ad` 完整重建工作台；
  原锁文件和 560 个既有 Rust 依赖版本保持，新 notify 8.2.0 等七个依赖已校验并离线编译。
- S08b：真实 PetApp 默认动态 import 的 Chromium 夹具，GIF/rig2d 各新文档首开 3D chunk/model 请求为 0；
  切到真实 Three/VRM/GLB 后加载提示、模型请求和 canvas 就绪通过，再回 2D 时移除 canvas。
  模块图证明 3D 不在入口静态闭包。Tauri IPC/设置事件为隔离夹具，未注入 loader；
  浏览器缓存可能温热，不是原生冷启动耗时或 GPU 首帧测量。失败/迟到 import 另由单测覆盖。
- S09：自动重试最多 8 次，等待 1/2/4/8/16/30/30/30 秒；连续健康 30 秒才重置，
  稳定后健康 HTTP 每 30 秒、进程退出仍每秒检查。旧健康结果/自动任务隔离受代际保护；
  stdout/stderr 脱敏后最多 4 × 64 KiB，超长整行丢弃。未知/变形凭据无通用检测保证。
- 最终完整 Rust 819 pass、0 fail、17 ignored；前端 157/157 文件、1183 pass、0 fail、8 skip、
  0 unfinished，各文件与外层均退出 0。三套 TypeScript、完整工作台、产品前端和正常 Windows debug EXE 构建通过。
  前端首轮暴露测试运行器自测的 5 秒父级预算小于 10 秒内部预算，定向真实复现后仅该测试改为 20 秒；
  内部 100ms 超时/失败状态/进程回收断言保留，全套重跑通过。没有放宽产品性能门槛。
- React Doctor changed/HEAD/include-untracked 扫描 6 文件 93/100、无新增诊断；扫描期间仅 Rust 变化，
  不与 B2 不同扫描范围的 81 分作增益比较。独立审查无剩余阻断，41 个已审文件 SHA 再核验。
  Windows 构建期间仅测试夹具文件变化，原生源码稳定；普通 cargo debug 构建只证明编译链接，
  未验证可分发包或原生前端装载。既有大 chunk/未用代码构建警告保留。

仍待验：Tauri/WebView2 生产冷启动和首次工作台窗口/会话所有权、真实模型用量与引擎故障配置、睡眠恢复、
在途退出与任务连续性、Windows 本地 PDF 协议/堆收敛、macOS 通知与 WebKit、整机 10 分钟 CPU/GPU/功耗。
collector/补偿 worker 在 sleep 前后检查退出，在途 HTTP 不主动取消；退出标记至连接撤销之间仍可完成有效提交。
B3 收尾时 R06c 的投影移锁、R06d 的 SSE 及 B4 资产/附件/桌宠优化尚未实施；后续 B4 见上节。未安装、推送或发布。

证据：`E:/Codex/工作伙伴/reports/yume-performance-b3-20261022`，包括 `verification-summary.json`、
`source-snapshot.json`、`b3-checkpoint.json`、`review.json`、逐项 RED/GREEN、浏览器截图和 `yume-b3-progress.html`。

### 2026-10-22 性能整改 B2

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

### 2026-10-22 性能整改 B0 / B1

基线：0.4.20，HEAD `d92a9c872b01300a27d90a675cfa9247bfd2fd2b`；实际开发入口为
`C:/Users/chenglingxiao/.codex/worktrees/build-v0418-9081/deskmate-public-release`，施工分支
`codex/yume-performance-remediation`。以下为 B1 当时的差异证据，现已保存于 `a48ee36`，不是旧主仓或安装版。

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
以上为 B1 当时的门禁状态；后续用户授权推进独立 B2，仍未关闭原生检查。尚未测得端到端性能增益。
开工时记录的安装版 YUME/引擎 PID 在收尾只读检查时已不在运行；退出原因未确定，未为验证重启安装版，
也未据此声称活跃任务连续性通过。前后进程摘要保存在 baseline/relevant-processes-after 回执。

保留边界：旧版没有归属记录的孤儿，以及 spawn 到写记录之间崩溃的子进程，均保守保留；
当前启动期间 revoked/expired 回合保存 32 字节摘要墓碑，数量随该次启动的回合数增长。
另发现既有自然语言调度解析把“工作日志”中的“工作日”当工作日限定；未在认证修复中扩大修改，待独立语义回归。

完整回执：`E:/Codex/工作伙伴/reports/yume-performance-remediation-20261022`，其中
`verification-summary.json`、`source-snapshot.json`、`b1-tracked.patch` 与逐项日志保存证据。

### 2026-10-10 历史加载优化与空会话自动清理

基线仍为 0.4.18、`codex/optional-windows-component`、HEAD
`e3a546f3252671b5ed9c80393fd69340bda67d5b`，保留此前双方头像和历史角色恢复的未提交改动。
本轮没有运行生产数据删除脚本、安装或发布。

- 列表先返回本地目录，原生发现后台合并刷新；刷新期间的新事件保留到下一轮。
  目录按主键查询、事务只写变化行、导入按源数据指纹跳过；预览只失效更新时间变化的记录。
- 普通发送与选中状态检查改为 metadata-only IPC；打开会话的模型选择并行读取，
  正文和运行状态并行请求，复用已验证的作用域；角色资料缓存 30 秒，安装/卸载角色包立即失效。
- 已确认空闲且无人使用的空会话在 30 秒创建/活动保护期后自动删除。
  当前及加载中的聊天、隐藏工作台草稿、任务记录、本地回复、任何原生消息、子会话与读取不确定情况均保留。
  工作台显示期间暂停原生清理。删除前复验，失败不创建自动重试删除墓碑；确认远端不存在后才标记已清理。
  旧文本空记录在持有本地存储锁时核验并删除。完整边界见[统一历史契约](migrations/opencode-native/unified-history-contract.md)。
- `bun run test:frontend` 最终 154/154 文件、1117 pass、0 fail、8 平台 skip。
  首次运行的 provider 测试夹具未适配新 IPC，已修复并全量重跑；归档/删除测试改为观察轻量状态检查。
- `cargo test --manifest-path src-tauri/Cargo.toml --lib` 为 728 pass、0 fail、15 ignored。
  随后切换顺序、预览失效及读取避开清理锁的调整使用 `--lib history::` 复验：106 pass、0 fail、1 ignored。
  覆盖 SQLite 实际写入计数、并发/回滚、复合身份、文件/工具/用户消息保留、子会话、繁忙/重试、错误响应、更新时间变化和删除失败。
- 合成 305 条目录（Windows debug 构建，100 次读取）全表读取查找 468.198 ms、按键读取 79.4145 ms。
  50 次无变化更新：旧全表重写 SQL 路径 452.7457 ms、增量路径 341.8089 ms；后者实际记录写入为 0，单条变化只写 1 行。
  这是隔离微基准，不是安装版端到端打开时间。手工命令：
  `cargo test --manifest-path src-tauri/Cargo.toml --lib catalog_index_and_delta_benchmark -- --ignored --nocapture`。
- TypeScript 三套检查、生产前端构建通过。React Doctor 相对 HEAD 包含新增文件：80/100，0 error、1 warning，
  仍仅为已有 `userAvatar.ts` object URL 误报（`finally` 明确释放），没有为此屏蔽规则。
- Edge 浏览器真实 ChatApp + 隔离 IPC 夹具：混合历史角色、未知角色中性回退、32×32/8px 圆角、继续发送与旧头像保留通过，0 页面错误。
  浏览器不连接生产 sidecar，不能替代原生自动清理验收。
- 完整日志、截图、SQL 微基准与改动前文件快照：`E:\Codex\工作伙伴\release-artifacts\history-speed-work`。

待验：新安装包下实际首次目录呈现/打开聊天时间，空草稿切换与工作台显隐跨窗口原生验收。
本次源码尚未重新打包、安装或发布；此前头像安装包不包含本轮优化。

### 2026-10-10 聊天双方头像

基线：0.4.18，`codex/optional-windows-component`，HEAD
`e3a546f3252671b5ed9c80393fd69340bda67d5b`。本次修改前工作区干净。

- 双方头像为 32×32、8px 圆角正方形；通用设置提供更换、恢复默认、预览与失败提示。
  个人图片压缩存入 `userAvatar`，复用原设置持久化与通知；三代内置小著提供独立静态头像。
- 前端完整隔离回归：152/152 文件、1111 pass、8 平台 skip、0 fail。
  此后新增 `UserAvatarSettings.test.tsx` 定向 2/2 通过，验证立即保存合并待保存设置、
  重挂载恢复，以及保存失败后读取后端状态恢复头像；未把定向复验称为第二次全量测试。
- Rust 头像模块 2/2 通过；随后设置相关模块回归 82/82 通过，覆盖旧设置默认、字段往返、格式/大小/尺寸边界。
- 生产/测试/脚本 TypeScript 检查、生产前端构建与 `git diff --check` 通过。浏览器验收使用真实组件与模拟 IPC：更换、刷新保留、跨窗口同步、
  损坏图片保留原值、恢复默认、四主题×360/430/760px 宽度、长消息/代码块/表格、
  切换三代目头像全部通过，页面错误为 0。所有图片临时 URL 在成功与失败时均释放。
- React Doctor 相对 HEAD（包含新增文件）为 0 error、1 warning；唯一 warning 指向
  `userAvatar.ts` 的 object URL。该函数 `finally` 明确 revoke，且浏览器拦截记录确认
  两条路径均释放，判定为高置信度误报，未加规则屏蔽。默认 origin/main 扫描包含历史问题，
  不作为本次头像改动的新增缺陷。
- 本机截图、浏览器回执及测试日志在 `E:\Codex\工作伙伴\release-artifacts\avatar-work`。

待验：打包后的 Windows WebView 原生文件选择、真实进程重启后的头像保留，以及导入包
头像资源的原生 asset 协议加载。浏览器夹具中的持久化/同步不代替这些原生验收。
后续于 2026-10-10 完整构建 Windows x64 NSIS 本地候选（版本仍为 0.4.18），111.611 MiB，保存于 `E:\Codex\工作伙伴\release-artifacts\v0.4.18-avatar-windows-20261010`。安装包完整性通过，包内 EXE 仅有 Tauri UNK→NSS 标记的 3 字节差异，其余与本次 release 程序一致；SHA-256 为 `76b448b54380b85a695247f8e5100b5cb32a8922a72998e273c62d0d1d80606a`。源码指纹对应构建开始时快照，本段打包回执于构建后更新。未签名、未安装，未提交、推送或发布。


### 2026-10-10 历史消息角色头像恢复

沿用上述 0.4.18 / HEAD；修改对象为头像功能之后的未提交工作区。按每条原生
user message 的 system 明确标记、前置角色 YAML ID 或完整已安装角色提示词匹配，
通过 assistant.parentID 关联回复，避免用当前全局角色或邻近消息猜测。新轻聊轮次
在 system 首行持久化 `<!-- yume:persona=<id> -->`；local-only 回复保存可选 personaId。
缺少可靠身份的旧文本保持中性头像，未知 ID 不回退成小著；未改写或迁移生产聊天文件。

- 用实际 Rust 恢复模块编译只读核验器，读取本机原生数据库：57 段非空会话中，
  37 段有明确身份、121 个用户轮次、184 条 assistant message 可关联；包括小熊虫
  4 段、包包 2 段及 2 段混合角色。只输出计数，未保存聊天正文/系统提示词副本。
- `cargo test --locked --manifest-path src-tauri/Cargo.toml --lib history::`：100 pass，
  0 fail、0 ignored；`agent::opencode_wire_tests`：通过，覆盖 system 与 parentID 透传。
- 完整前端回归首轮：154 文件、1115 pass、1 fail、8 平台 skip、0 unfinished。
  唯一失败为新增组件测试在 Happy DOM 中断言实际图片加载（资源不可达而进入 fallback）；
  改为核对角色标签后，该文件 9 pass、0 fail。真实 PNG 加载另由浏览器验证，
  不把这次定向复验描述为第二次全量执行。
- `bun run typecheck`、`bun run build` 退出 0，保留既有大 chunk 提示。
  React Doctor HEAD+untracked 为 0 error、1 warning，仍为已核实的 object URL 误报。
- Edge 实际 ChatApp + 模拟 IPC：混合历史中小熊虫/包包真实本机包封面、三代目
  内置 PNG 均加载；无身份/已移除角色不冒用当前角色；继续发送持久化新角色标记，
  新回复为当前小著、旧头像不变；5 个历史头像均为 32×32、8px 圆角，页面错误为 0。
  QA 夹具修正多事件订阅广播，避免单个订阅覆盖其他订阅。
- 证据：`E:\Codex\工作伙伴\release-artifacts\avatar-work` 下
  `history-mixed-avatars.png`、`history-browser-qa.log`、`history-frontend-tests.log`、
  `history-chat-memory-retest.log`、`history-typecheck.log`、`history-build.log`，
  以及 `recovery-check.rs` / `.py`（复用实际模块的只读核验器）。

后续重新构建包含本次历史修复的 0.4.18 Windows x64 NSIS：111.664 MiB（117088391 bytes），目录 `E:\Codex\工作伙伴\release-artifacts\v0.4.18-history-avatar-windows-20261010`。SHA-256：`06c9d1a6f2ab587630ffb78c1b5befa568ece0b6f94b48befe1d8ecc39df8aca`。构建退出 0，NSIS 完整性、包内 EXE 与本次 release 的 Tauri 标记归一化比对、三份头像资源哈希、构建前后源码指纹均通过。未签名、未安装、未发布；Windows WebView2 原生 asset 协议仍待实机验证。本段为构建后回写，不属于 build-inputs.json 中的构建开始快照。
前一份 `v0.4.18-avatar-windows-20261010` 候选保持原样；请使用新目录内的安装包验证历史恢复。

### 2026-10-10 外部部署退役

基线为 0.4.18 的 `codex/optional-windows-component` 工作树，保留此前 Windows 按需组件未提交改动。实现范围及旧快照恢复见[模型连接](ai-provider-support.md#yume-内置连接与外部部署退役2026-10-10)。设置和聊天不再提供外部部署；普通应用无恢复/导入 IPC，维护恢复代码仅在显式 feature 下编译。

验证结果（Windows x64，2026-10-10）：

- 生产/测试/脚本 TypeScript 检查、生产前端构建和 `git diff --check` 通过。
- 前端全量首轮为 146/147 文件通过，唯一失败是等待已删除部署监听器的旧断言；修正后该文件 5/5 通过，新增通用工具解码回归 2/2 通过。按最终文件集合合计 148 文件、1103 pass、8 平台 skip；保留首轮失败记录，不把定向复验描述为第二次全量执行。
- Rust 默认库首轮 715 pass、1 启动等待超时、14 ignored；该未修改用例独立复验通过，完整 suite 用 `-- --test-threads=4` 复验为 716 pass、0 fail、14 ignored。没有跳过或放宽失败断言。
- `legacy-ai-recovery` 的快照/路径/加密/并发恢复测试 12/12 通过，维护 CLI 参数保护测试 1/1 通过，维护可执行文件已实际编译。普通依赖图不含 `uiautomation` 或 `chacha20poly1305`。
- React Doctor `--scope changed --base HEAD --include-untracked`：81/100，无错误；仅有此前 WindowsComponentControl 的复杂度提醒。本次未修改该组件或隐藏规则。
- 独立临时 HOME/工作区直接启动随包 OpenCode 1.18.21，经本机合成供应商完成一次真实 HTTP 对话；模型可用、6 个记忆/工作日志工具可发现、旧 CC Switch 工具不存在；子进程已退出、端口已关闭。未用生产 API Key。
- 源资源保留内置引擎与工作台，旧部署工具不再入资源目录；工作树 debug/release 中两份与旧资源逐字节一致的生成副本已清除。用户已有外部安装、PATH、外部配置和生产恢复快照均未操作。原工作树未提交 patch 中的 120 条新增非空行仍在。
- 证据位于本机 `E:/Codex/工作伙伴/output/remove-external-deploy-20261010`：`inventory.json`、`baseline-preservation.json`、`embedded-smoke.json`、`resource-audit.json` 及各检查日志；不作为发布资产。

尚需安装版实机验收：无外部客户端的全新用户、含旧工具/快照的升级用户、真实 API 连接后轻聊和工作台、用户密钥存储支持的真实旧快照恢复、macOS 编译及界面。合成回归不替代真实宿主、真实凭据或跨平台验收；未执行安装和发布。

### 2026-10-09 Windows 按需组件候选

基线 `5224ac7a1b240d1db7b90125eb5c9b62c9df14b5` / 0.4.18。实现契约见[轻量安装包](workbench-build.md#轻量-windows-安装包2026-10-09)。此次验证限定 Windows x64 本地候选，未发布。

- Windows Rust library suite：801 pass、0 fail、15 ignored。仅定向执行 `live_component_download_verifies_and_extracts -- --ignored --nocapture` 可联网下载固定组件、校验并安全解包；不要对全仓使用 `--include-ignored`，其中包含会打开已安装应用的手动测试。
- 下载核心测试覆盖并发去重、取消、超长/截断/损坏数据、网络和磁盘错误、固定文件解包、缓存校验、临时目录清理。真实固定 ZIP 双校验与真实 HTTP 下载分别通过。
- 前端定向运行 `WindowsComponentControl`、`ToolPermissionsTab`、`WidgetSettings`、`CcSwitchStatus`、`WorklogWidget` 及资源检查测试，共 64 pass；覆盖首次确认、取消重试、缓存启用、旧偏好补装、失败反馈、等待生效、重新打开设置页，以及其他设置延迟保存不覆盖组件启用事件。TypeScript 检查通过。
- React Doctor 以该基线为 `--base` 并包含 untracked：无新增错误，保留组件渲染函数的复杂度提醒；没有通过禁用规则隐藏提示。构建保留既有 Rust unused/dead-code 和 Vite 大 chunk 提示。
- 隔离浏览器加载真实组件和设置主题，使用模拟 IPC 核对首次确认、45% 进度、下载时禁用开关、取消后恢复下载入口；未使用生产 AppData。该预览不等于原生宿主验收。
- 尚需隔离安装版实机验收：首次下载到 AppData 后实际 MCP 启动、工作台/轻聊任务运行中等待及完成后重启、升级安装与离线复用、原生窗口取消关闭、磁盘满/断网真实恢复。单元测试与 HTTP 测试不替代这些 Tauri UI 场景；macOS 构建未复验。

仓库根目录运行：

```sh
export DEVELOPER_DIR=/Library/Developer/CommandLineTools
bun run check
```

`check` 顺序执行生产/测试类型检查、当前源码逐文件前端测试、适用平台 Rust library suite 和自有前端 build。需要定向复验时使用 `bun run typecheck`、`bun run test:frontend`、`bun run test:rust`、`bun run build`；`bun run test:frontend --list` 给出本次发现清单。裸 `bun test` 不作为全仓验证入口。HTTP fixture 需允许 loopback；不要与 suite 中的构建 fixture 同时启动第二个前端 build。

完整工作台按[当前构建入口](workbench-build.md)重建。隔离原生整改用 `bun scripts/audit-remediation/verify-native.ts`；自动处理联调用 `bun scripts/automation-qa/verify-sidecar.ts`。这两项都不等于完整 Tauri UI、Windows 或真实模型语义已验证。

模型语义脚本 `bun scripts/automation-qa/verify-semantics.ts` 需要显式配置测试服务商，只发送合成场景；正例仍需人工语义复核。发布、签名包和真实 A→B 更新按[本机发布指南](local-release-runbook.md)及[macOS runtime checks](macos-runtime-checks.md)执行。

## 2026-09-30 composer 浮层显示修复的定向验收

基线 `551d8d300c762d36a0e6304e91936c665f840676` / YUME 0.4.16，验证对象为本次未提交前端改动。共享浮层显式接入主题和字体，定位避开整个输入 footer，模型标签按内容伸缩并保留末尾识别信息；设计规则见 [DESIGN](../DESIGN.md#chat-composer-pickers)。

- Bun 1.3.14：`bun run typecheck` 退出 0；`DEVELOPER_DIR=/Library/Developer/CommandLineTools bun run test:frontend` 退出 0，155/155 个文件、1,190 pass、0 fail、0 skip、0 unfinished；`bun run build` 退出 0。初次沙箱 suite 的 loopback fixture 无法监听，已在允许本机监听的上下文完整重跑，通过数仅取重跑结果。构建保留既有大 chunk 提示。
- 浏览器合成真实 `ChatApp` 场景：两个菜单的 dark/mint/peach/lavender 外观及打开时主题变化；420×560、720×760、360px 压力宽度、420×300 高度压力及菜单打开时缩放；输入 footer 与菜单保持 8px 间距、视口边界内、DeepSeek 两种名称完整、长目录/长模型尾部可见、发送按钮不被挤出；32 模型列表内部滚动和搜索筛选、方向键/Enter/Escape 焦点返回、外部点击关闭均已检查，浏览器 error 日志为空。
- 新证据在本地忽略目录 `artifacts/composer-display-qa-2026-09-30/`：四主题截图、缩放/长名称/搜索截图、几何与主题检查 JSON，以及本次 frontend suite 回执副本。可复验入口为 `/scripts/automation-qa/chat-preview.html?composer=1`；多模型加 `&models=many`。主题切换控件只在该 QA 页面出现。
- 尚未重新构建并实测原生 Tauri 聊天窗、Windows WebView2、原生文件选择器或真实模型服务；浏览器 fixture 不关闭下表“历史与 composer”的完整原生回归余项。此次没有发布或更新已安装应用。

## 活动整改与平台门禁

| 项目 | 尚需完成 / 证据入口 |
| --- | --- |
| 原审计 UI U1/U2 | 重连、busy Stop、idle 后 scheduled、多个 text parts 和迟到响应最小反例与修复；见[施工进度](audit-remediation-progress.md) |
| 工作台 W1/W2 | 自有源码直接表达行为、固定输入统一验证、重复准备与失败恢复；普通前端绿灯不关闭此项 |
| 历史 H1/H2/H3 | H2 按 key 查询、变化行写入与导入指纹已于 2026-10-10 完成源码/隔离测试；新正文投影退出及目标安装版性能仍待验。保留 legacy/local-only、墓碑及身份约束 |
| 综合 V1 | 完整 Tauri interactive/scheduled 入口、跨 30 秒失败、真实工具子进程取消、两平台宿主与回退演练；已有 scoped OpenCode fixture 不替代完整 UI |
| CI / Windows | Source checks 真正运行、Windows 适用 suite/中断进程树清理、branch protection required checks；静态 YAML 不能当平台执行通过 |
| 文件清理条件项 | 以[清理执行进度](repository-cleanup-progress.md)为准；未验证的外部作者资产、生成链、Windows 和 Mac 包图标不因本地源码通过而关闭 |
| Windows QA harness 旧边界 | `p5-windows-e2e.ts` 仍需给 MCP 回传 Notepad PID补新建进程/出生时间/可执行文件归属证明，并保证 setup/cleanup 首项失败不跳过其余关闭与临时根移除；本轮已改默认资源路径、平台预检与schema断言，不宣称此脚本已完整 Windows验收。工作记录 capture 新身份/PID/窗口渲染校验也需要实际 Windows 运行 |

## 从功能施工记录转移的未完成事项

| 范围 | 需要的后续验证 | 保留的原证据 |
| --- | --- | --- |
| 自动记忆/日志 | 36 条中英文真实模型语义样例：引用/自述、同义更正、去重、完成/取消、敏感负例与连续事项；确认当前日常应用完整交互。只用合成样例和隔离库，不把模拟模型联调当语义准确率 | [自动处理验收](automatic-memory-verification.md#真实模型验收与当前边界) |
| 附件原生事件/播放器 | MP4/MP3/WAV 逐项原生拖入，快速跳转的 Range 取消、播放中移除/切换、解码失败恢复；Windows WebView2。用户确认文件夹可拖入不扩大为所有媒体压力场景通过 | [原生回归](attachment-native-regression.md#2026-09-28-本机记录) |
| 历史与 composer | 当前原生小窗口/大窗口、离线、旧数据、首条消息计数、模型切换/恢复、Stop/审批完整回归；历史 Windows 证据与较新 Mac QA 不互相替代。360px CSS 压力规则没有 360px 原生截图 | [organizer 回执](archive/history/history-organizer-redesign-plan.md#8-施工记录2026-09-27)、[composer 回执](archive/chat/chat-composer-workspace-model-plan.md#10-实施与验收记录2026-09-27) |
| 设置与原生材质 | 原 P0 中剩余角色包/异常状态、跨窗口主题同步、全部页面语言组合；标题栏真实拖动、不同系统缩放。P1 材质统一/P2 原生 Vibrancy/Mica 应先核对后续样式接入，再确定尚未覆盖的范围和性能对照 | [P0/P1/P2 历史计划](archive/ui/frontend-redesign-phase-2-plan.md#后续验收边界)、[较晚原生检查](archive/ui/yume-latest-integration-review.md#后续macos-原生窗口检查) |
| 星运 | Windows 原生窗口及日期变更/恢复行为复验；已有本机持久化、四语言和四主题截图保留，不当 Windows 通过 | [首版验收](archive/horoscope/daily-horoscope-implementation-plan.md#12-首版实施验证记录2026-09-30) |
| 发布与更新 | 每次目标版本的 Mac 签名公证/下载回读/隔离 A→B、活动任务等待、取消/失败恢复；Windows 对应更新及安装图标。旧 release notes 的“尚未发布”不判断今日公开状态 | [macOS gate](macos-release.md)、[runtime checks](macos-runtime-checks.md) |

## 原生迁移能力余项

固定引擎能力矩阵仍保留：多题/多选/自定义 question 回答、真实鼠标拖放及更多 MIME、需要模型执行的 slash 命令、可核验 `/terminal` 面板、CLI/TUI 专有入口、非 Git 目录审查/撤销，以及同环境迁移前后的性能对照。不同版本升级/降级须用隔离旧/新包和数据副本验证；UI 回退不等于数据库格式降级。逐项状态见[能力矩阵](migrations/opencode-native/capabilities.md#8-p6-安装版能力终验状态2026-09-23)。

这些是 2026-09-23 记录留下的未验项，后续若已有独立证据，先对照版本、平台和场景再关闭。旧记录的“私钥密码未知 / 无签名 / sidecar 无鉴权 / 不自动重启 / 静态工作台记忆”已不是当前维护入口：当前签名能力按发布指南预检，当前上下文按[记忆契约](memory-and-worklog.md)及 D1-12 修订读取。

[早期 Agent 架构审计](archive/architecture/AGENT_HARNESS_ROADMAP_AUDIT.md)的目录/权限/停止/恢复与真实产物闭环要求，按当前 U/V 与原生能力项核对；性能改进仍需同环境前后测量。其 Desktop Context、通用触发器、可执行 Skills、多 Agent 等候选方向保留为历史建议，未因归档或移入索引转成已批准的施工待办。

兼容条件持续保留：旧格式 reader、纯旧文本、完整复合身份、local-only 回复、墓碑和不复活；不得以清理文档或完成某项门禁为由让旧二进制写新版数据库、覆盖新增数据、重放 prompt 或削弱批准边界。

## 完成回写

执行时记录当前 commit/未提交 diff、运行时、完整命令、退出码、通过/失败/skip/ignored、合成数据和证据路径；实机、模型、源码与公开资产分别记录。关闭一项需对应范围的真实新证据，不用旧测试数、截图存在或 archive 标记代替。
