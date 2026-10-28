# YUME 性能整改：Windows 接管入口

## 2026-10-28 本机交付已收尾

当前 `E:\Codex\工作伙伴\deskmate-windows-b7` / `codex/yume-performance-b6` / 0.4.21。原有 Windows 改动提交 `8702347`；本轮 Stop 主动取消误报与历史日志回执会话/目录作用域修复提交 `36a3c2e`。完整前端 1373 pass、Rust 974 pass，0 fail，8 skip / 24 ignored 保留；正常 release 已恢复，临时凭据、专属进程/端口和两数据根已清理。

从 [最终交付报告](performance-evidence/b7-windows-closeout-20261028/README.md)、[结构化结果](performance-evidence/b7-windows-closeout-20261028/summary.json) 和 [20 项审计矩阵](performance-evidence/b7-windows-closeout-20261028/audit-matrix.json) 了解实际进度。Windows 本机修复与本轮验收交付收尾；不再延续历史轮次估计或重复已通过的检查。严格全平台 V2 尚缺 Mac 最终实机、混合 DPI/高刷、GPU/功耗受控 A/B 等证据，保留为明确的平台/实验室范围。未推送、安装到日常身份或发布；旧主仓与其他工作树保持。以下为当时检查点。

## 2026-10-28 Windows release 附件与日志检查点

B7 仍进行中，当前 E:\Codex\工作伙伴\deskmate-windows-b7 / codex/yume-performance-b6 / db07e5b / 0.4.21。复用经 SHA 核验的隔离 release QA：原生 MP4 选择器、MP3/WAV/文件夹双区域拖放、坏视频回退、发送与重启后历史恢复通过；日志记录、冲突草稿、日报周报、版本采用、复制与导出、计划编辑/暂停、失败后真实重试、关联删除和浅深色固定窗口均有物理证据。构造的最近漏周重启后运行中暂停并取消，服务延迟期过后未归档；不宣称客户端收到了迟到响应。十次普通二次启动通过，活跃工具期间仍待验。本轮无产品源码修改，835 项原有文件内容保留；正常 release 已校验恢复。

六库完整性检查均 ok，但随后发现宿主快照后新建的侧车，不能将检查称为所有进程停止后；该侧车已按父进程/出生时间/路径核验停止，隔离根和临时端口清理完成。详见 [本轮证据](performance-evidence/b7-windows-release-acceptance-20261028/README.md) 与 [结果](performance-evidence/b7-windows-release-acceptance-20261028/summary.json)。继续活跃工具二次启动、Aborted 文案、其余恢复/迁移、PDF 堆/RSS、硬件/生产对照、历史回执恢复新增读取的性能和最终完整门禁；真实模型意图和 Mac 单列。Windows 估计尚需 2–3 轮，出现新阻断为 3–4 轮；未提交、推送、安装或发布。以下历史记录保留当时状态。


交接日期：2026-10-27（Asia/Shanghai）。用户指定下一阶段由 Windows 电脑继续施工；本次保存并推送现有代码、进度和证据，不新增发布或日常数据迁移。

## Windows 最新：release 附件阶段检查点（2026-10-27）

优化 release 已完成 22 MB MP4 正文真实拖放、完整 30 秒播放、连续进度手势和播放中移除；1/8/20 MiB SHA 回读、64 MiB 媒体解码/清理与非法边界通过。本轮一次完整前端回归 1367 pass / 0 fail / 8 skip、172/172 文件，六库完整性与隔离清理完成，正常 release 恢复。Windows 安全桌面中断后续真实输入：release 原生选择器/其余媒体矩阵及完整工作日志物理表面未完成，不用前轮 debug 或后台 IPC 替代。既有改动保留，本轮无产品源码修改。B7 仍进行中，Windows 仍估 2–3 轮，新增阻断则 3–4 轮；Mac 专有项另计，未提交/推送。

当前入口 E:\Codex\工作伙伴\deskmate-windows-b7 / codex/yume-performance-b6 / db07e5b / 0.4.21，历史 schema 3、记忆 schema 4。见 [阶段 HTML](performance-evidence/b7-windows-release-worklog-20261027/index.html)、[方法、系统阻断与余项](performance-evidence/b7-windows-release-worklog-20261027/README.md)、[结果](performance-evidence/b7-windows-release-worklog-20261027/summary.json)。

下一步从 release MP4 原生选择器继续，完成 MP3/WAV/文件夹/坏视频的正文与输入区、切会话/重加/发送/历史，再完成日志编辑/冲突、版本/来源、复制/导出/同名、计划、失败重试/运行中暂停、关联删除、重启和浅深色固定窗口滚动。通过后再推进活跃工具二次启动、Aborted 文案、剩余恢复、PDF 内存、硬件/生产对照与最终门禁。历史回执恢复新增读取并入性能复核；真实模型意图与 Mac 范围仍单列。QA 缓存已按 SHA 保存，但旧 PID/HWND/端口作废；复测必须重建临时会话并重新观察窗口。前次记录保留当时范围。

## Windows 前次：附件、工作日志与迁移恢复（2026-10-27）

MP4/MP3/WAV/文件夹与坏视频物理拖放、MP4 原生选择器及播放、1/8/20 MiB 二进制与 64 MiB 媒体原生 IPC 通过；工作日志五工具、报告版本和漏期补偿重启去重通过。修复回执拒绝被空查询覆盖及历史重开不恢复回执，并修正 QA provider 参数包装。历史旧库受控失败完整回滚、修复夹具后原生升级/重启及中文记忆导出通过。最终前端组合覆盖 1367 pass / 8 skip（全套后修正测试桩并复跑受影响 28 项），Rust 迁移定向 32 pass，类型与正常构建通过，6 库完整性和隔离清理完成。B7 仍进行中，Windows 预计再需 2–3 轮，新增阻断则 3–4 轮；Mac 专有项另计，未提交/推送。

当前仍为 E:\Codex\工作伙伴\deskmate-windows-b7 / codex/yume-performance-b6 / db07e5b / 0.4.21，历史 schema 3、记忆 schema 4。见 [最新 HTML](performance-evidence/b7-windows-attachments-20261027/index.html)、[完整证据与限制](performance-evidence/b7-windows-attachments-20261027/README.md)、[结果](performance-evidence/b7-windows-attachments-20261027/summary.json)。

下一步优先附件 release 与完整日志物理表面（编辑/版本/导出/删除/失败重试与取消）；再做活跃工具二次启动、Aborted 文案、余下恢复、PDF 内存/硬件/生产对照及最终全套。历史恢复新增消息读取需并入性能复核。五工具为本机合成模型强制选工具，不等于真实模型意图通过；本轮原生构建为 debug，不能关闭 release 与 Mac 门禁。前次记录以下保留，不覆盖当时失败与边界。

## Windows 前次：Tao 绘制与审批 SSE（2026-10-27）

Tao 绘制重入已复现并修复，原生回归 25 次通过；审批 SSE 响应头等待与正文空闲期限分离，真实故障注入从旧版 40 秒未就绪恢复到约 4.1 秒重连，旧连接已关闭。物理审批取消/允许一次、缩放/截图、最小化恢复及桌宠重开通过；Rust 974 pass / 24 ignored，正常构建恢复。B7 仍进行中，预计 Windows 还需 3–4 轮，Mac 专有项另计，未提交/推送。

仍在 E:\Codex\工作伙伴\deskmate-windows-b7 / codex/yume-performance-b6 / db07e5b 叠加改动。版本 0.4.21、历史 schema 3、记忆 schema 4 保持。见 [HTML](performance-evidence/b7-windows-tao-sse-20261027/index.html)、[完整证据与边界](performance-evidence/b7-windows-tao-sse-20261027/README.md)、[结果](performance-evidence/b7-windows-tao-sse-20261027/summary.json)。

Tao 使用固定 0.35.3 源码的最小本地补丁，仅外层 flush 结束绘制周期；应用依赖版本未升级。审批流复用已锁定的 reqwest/Tokio/rustls，保留原解析器、8 MiB 上限与目录/会话/generation 隔离。响应头 3 秒、正文空闲 45 秒；停止及换代能中断等待，停止后不能重新置为连接。

本次还补工作日志 record/query/拒绝区分的隔离传输合同；该合同使用合成宿主回执，不关闭完整五工具/到期 GUI 门禁。原现场 Tao 外部触发源、旧 SSE 服务端停顿原因未追溯证明。QA 见一次可恢复正文断流，重连与后续审批成功，不宣称零断流。前端源码本轮未改，沿用前次 1361 pass / 8 skip，不计为本轮重跑。所有 QA 根、凭据与进程清理，正常 EXE 已恢复。

下一步：物理拖放/更多媒体与二进制、取消 Aborted 文案、日志五工具/到期、迁移失败恢复和活跃工具二次启动，再补 PDF 内存/硬件/生产对照与最终收口。Mac 验证与 Windows 证据分开。

## Windows 前次：原生 GUI 与发送焦点（2026-10-27）

继续在 E:\Codex\工作伙伴\deskmate-windows-b7 / codex/yume-performance-b6 / db07e5b 叠加现有改动，版本 0.4.21、历史 schema 3、记忆 schema 4 不变，未提交/推送。最新入口：[HTML](performance-evidence/b7-windows-native-gui-20261027/index.html)、[方法/边界](performance-evidence/b7-windows-native-gui-20261027/README.md)、[结果](performance-evidence/b7-windows-native-gui-20261027/summary.json)。

用户授权临时 Windows MCP 1.3.24，SHA 与仓库固定清单一致；仅子进程 DPI 环境修正后截图/真实鼠标可用，无安装或全局配置修改。原 Computer Use 截图超时未修复。物理 Enter 发送发现 textarea disabled 丢失焦点，改为 readOnly，保留准备锁和防重复发送。最终新 1000 消息首次 10k 的 104 个流式采样 Stop 全可见、失焦 0、1000 旧节点保留，完整原生全文 SHA 匹配，中文草稿保留。实际鼠标 Stop（97/500 分片）及续聊通过；原生 TXT/PDF/WAV picker、发送/历史重开、WAV 播放器走到结束、工作台接管/小窗只读/释放、历史置顶/中文改名/归档/恢复通过。

最终前端 **1361 pass / 0 fail / 8 skip，172/172 文件**，类型检查、正常 Windows debug/no-bundle 构建通过，Doctor changed 82 分无新增报告。Rust 未改未重跑，先前 970 pass / 24 ignored 保留为基线。五个隔离 SQLite 完整性通过，虚拟凭据/进程/临时 MCP/provider/两个 QA 根/三个端口清理，正常 dist/EXE 已恢复，未运行生产身份。

**B7 仍进行中**：早期 Tao flush_paint_messages 绘制断言崩溃一次，后续未复现，根因未定；取消仍显示 Aborted；宿主 permission SSE 曾记录 os error 10060。下一轮先隔离复现崩溃与审批/SSE 故障恢复，再推进物理拖放/其他媒体与二进制、日志五工具/到期、迁移恢复、活跃工具二次启动、PDF 内存、硬件/生产对照及 Mac 专有范围。修复前失焦、旧 dist 无效复测、完整套件旧 disabled 断言失败均保留，不能当通过。

## Windows 前次：冷启动历史与多窗口（2026-10-27）

继续在 `E:\Codex\工作伙伴\deskmate-windows-b7` / `codex/yume-performance-b6` / `db07e5b` 叠加现有改动，未提交/推送。最新入口：[HTML](performance-evidence/b7-windows-cold-history-20261027/index.html)、[方法/边界](performance-evidence/b7-windows-cold-history-20261027/README.md)、[结果](performance-evidence/b7-windows-cold-history-20261027/summary.json)。产品 0.4.21、历史 schema 3、记忆 schema 4 和依赖未变。

已定位首次项目插件依赖初始化晚于全局健康检查，原 5 秒历史读取预算误报超时。现用独立 120 秒后台项目就绪预算，普通读取仍为 5 秒；等待不持有 Agent 操作锁，就绪按引擎 generation 缓存。还修复 WebView2 早期 IPC 的 state 未注册 panic/短暂报错、初次会话创建关闭历史视图、发现中误显示空态。最终全新身份冷启动 **66.16 秒**、热重启 **2.03 秒**自动显示两条各 1000 消息的会话，无错误或面板关闭。首次依赖安装仍慢，时间为 React 就绪后 DOM 探针测量；不是启动性能提升或 p95 声明。60 秒候选失败与早期 panic 原件保留。

复测通过后，真实 chat/workbench/settings 多窗口接管/只读、越权拒绝、目录改名同步、并发 pin+rename、归档恢复和释放通过。Sky 原生 UIA 搜索 2→1→2 通过；截图捕获超时、鼠标几何信息不可用，不能算物理 GUI/视觉通过。1000 消息 10k 全文 SHA、Stop/续聊、旧节点/焦点/草稿回归通过。

最终完整前端 **1360 pass / 0 fail / 8 skip，172 文件**；Rust **970 pass / 0 fail / 24 ignored**；三套类型、标准前端与正常 Windows debug/no-bundle 编译通过。Doctor changed scope 无新增诊断，同 7 文件前后均 81 分/12 条既有诊断，不能与前次 4 文件 83 分直接比较。25 个 SQLite 完整性通过，五个身份凭据/进程/数据根和两个端口已清理，正常 dist/EXE 恢复；未运行生产身份。旧 E 主仓状态与 C 工作树保留，先前报告原件保留（首批 README 早已有后续链接，哈希例外见复核回执）。

**B7 仍进行中**：继续物理 GUI picker/drop、媒体/二进制、审批/SSE 故障恢复、日志五工具/到期、迁移恢复和活跃工具二次启动；PDF 内存/硬件/生产对照与 Mac 专有范围保留。下方为先前记录，不覆盖原失败与当时边界。

## Windows 前次：Stop/焦点修复（2026-10-27）

继续使用 `E:\Codex\工作伙伴\deskmate-windows-b7`，分支 `codex/yume-performance-b6`，基线 `db07e5b`，源码与证据未提交/推送。最新入口：[HTML](performance-evidence/b7-windows-stop-focus-20261027/index.html)、[方法/边界](performance-evidence/b7-windows-stop-focus-20261027/README.md)、[结果](performance-evidence/b7-windows-stop-focus-20261027/summary.json)。

已通过真实 catalog 事件重放复现 `send=false / active_task` 卸载本地运行 composer 的机制，并修复其 Stop、草稿和焦点保持。两个独立 1000 消息首轮 10k（其中一个在宿主重启后）、50k、主动 Stop/继续通过；全文 SHA 和代码块一致。最终前端 **1358 pass / 0 fail / 8 skip，172 文件**，类型/标准前端/正常 Windows debug 编译通过；Rust 本轮未改未重跑，先前 966 pass / 24 ignored 保留为基线。新增正式 IPC 的历史删除/native 404/重启 tombstone、记忆导出/忘记/重启验收。导出格式为 schema 1，记忆库仍 schema 4。

冷启动最早历史发现仍观察到超时，未误记为流式通过；发送阶段现有 textarea disabled 引起的发送前焦点变化也单独保留。继续定位入口超时，补 GUI/多窗口、迁移恢复与硬件。专属 QA 身份、虚拟凭据、端口和进程清理完毕，十个 SQLite 完整性通过，标准 dist 恢复，最后 EXE 为正常配置 debug 编译件，未运行生产身份/安装/发布。**B7 进行中**，下节首批原始失败记录由本节补充，不能据此删除。

## Windows 首批接管（2026-10-27 先前记录）

实际拉取 HEAD 为 `db07e5bdb02550c4d165da8cd45fb76debefbe17`，分支 `codex/yume-performance-b6`。当前工作入口 **`E:\Codex\工作伙伴\deskmate-windows-b7`**，独立 clone；旧主仓未提交状态复核一致，原 C 盘工作树保持干净。下方“Windows 拉取”保留为其他机器的接管方法，本机已经完成。

Windows 基线通过：冻结依赖和固定工作台完整重建；三套类型检查、前端 **1353 pass / 8 skip**、Rust **966 pass / 24 ignored**、前端构建、生产 Cargo check、正常配置 Windows debug EXE 编译。原始日志 `E:\Codex\工作伙伴\reports\yume-b7-windows-20261027`，首次失败与复跑均保留。[本轮 HTML](performance-evidence/b7-windows-20261027/index.html)、[方法/边界](performance-evidence/b7-windows-20261027/README.md)、[结果](performance-evidence/b7-windows-20261027/summary.json)。

B7 新增隔离 Windows QA primary/二次启动/孤儿回收、真实 OpenCode 恢复/归档故障/拒绝/取消、WebView2 PDF worker 32 次、50k 流式中二次启动及 Stop/继续。1000 消息 50k 全文/代码块/旧节点/焦点通过；**1000 消息 10k 首轮 Stop 不可见、焦点丢失，终态探针提前结束，不能采用该轮性能值**。同一会话稳定后的 1002 消息复查通过，原因仍未证明；下一步优先冷启动重现并抓取 catalog/ownership/SSE/composer 诊断。原生捕获连续超时，GUI/硬件、PDF 内存、生产对照和 Mac 专有范围仍待验。

本轮没有产品代码、schema、依赖或版本变化；尚未提交、推送、安装或发布。专属进程/端口/虚拟凭据/数据根已清理，五个 SQLite 完整性通过，标准生产 dist 已恢复。target/debug/yume.exe 最后为 QA 产物，不作正式候选。**B7 仍进行中**，不把稳定复查当作首次异常已修复。

## Mac 交接时的位置与完成状态

- 仓库：`https://github.com/ciph/deskmate.git`；施工分支：**`codex/yume-performance-b6`**。Windows 从该远端分支接续，不从旧 `main` 重做 B6，也不依赖旧 Windows 分支的 upstream。
- 交接前检查点：`db96597`（B7 流式/PDF 证据），最近代码修复：`3069953`（原生文本 delta）。本交接文档作为后续独立提交；拉取后记录实际 `HEAD`。本次核对的 `origin/main` 仍为 `fcb463516bdde3f8bd0e3f8aa2ab287f0d355356`。
- B1–B5 已随 Windows 0.4.21 发布。B6 H18a/b/c、M19a/b/c 代码及 Mac 自动回归完成；历史目录 schema **3**、记忆库 schema **4**，产品版本仍为 **0.4.21**。不得改回 schema 版本号或让旧二进制写入新库。
- **B7 进行中**。Mac 最新前端 172 文件 / **1359 pass / 0 fail / 0 skip**，类型检查、生产构建与隔离 QA App 构建通过。最近 Rust 全套为上一轮 **957 pass / 23 ignored**；最新流式修改未改 Rust，未重复执行，不把 Mac 数量或 ignored 状态预先写成 Windows 通过。
- Mac 已补 1000 消息、计划 50 Hz 的 10k/50k 流式、Stop/继续与 PDF worker 32 次循环。PDF 内存收敛未测得，GUI/硬件与 Windows 原生缺口仍保留。详情见[最新证据](performance-evidence/b7-macos-20261027/index.html)、[完整边界](performance-evidence/b7-macos-20261027/README.md)和[统一待办](verification-backlog.md)。

## Windows 拉取

在选定的 Windows clone 中逐条执行并检查退出码：

```powershell
git remote -v
git status --short --branch
git log -5 --oneline --decorate
git fetch origin --prune
```

先保存已有本机工作；不自动 stash/reset/clean，不覆盖其他任务的未提交改动。确认 origin 为上述仓库、工作区干净后，选择适合当前状态的一组命令。

本地还没有施工分支：

```powershell
git switch --create codex/yume-performance-b6 --track origin/codex/yume-performance-b6
```

本地已有施工分支：

```powershell
git switch codex/yume-performance-b6
git merge --ff-only origin/codex/yume-performance-b6
```

若无法 fast-forward，保留双方提交，先检查分叉原因，不强制重置。随后核验：

```powershell
git merge-base --is-ancestor db96597 HEAD
git rev-parse HEAD
git rev-parse origin/codex/yume-performance-b6
git status --short --branch
```

祖先检查应成功；无本地额外施工时两个 SHA 应相同，并与本次推送回执核对。尚无 clone 时，可先在新的目录执行 `git clone --branch codex/yume-performance-b6 https://github.com/ciph/deskmate.git`。不要复制 Mac 的 `.git`、`node_modules`、`target`、sidecar 或生成的工作台。

## 重建本平台环境

先核对 Windows 原有开发环境与 [Source checks](../.github/workflows/check.yml)：Bun **1.3.14**、Rust **1.94.1**、OpenCode **1.18.21**。记录 Windows、WebView2、CPU/GPU、显示器 Hz/DPI 及实际工具链；不修改锁文件来规避环境差异。Mac 的 `output/` 与 `context.sh` 未提交，不是 Windows 启动依赖。

```powershell
bun --version
rustc +1.94.1 --version
cargo +1.94.1 --version
$env:RUSTUP_TOOLCHAIN = "1.94.1"
bun install --frozen-lockfile
bun run prepare:sidecar
```

完整工作台按[构建契约](workbench-build.md)准备独立上游源码：固定 commit `826d9ad46a22bef0294998e08daa3c4904fea28f`；Bun 1.3.14 安装冻结依赖，安装前后根 `bun.lock` SHA-256 均应为 `55cde1d8d6921ddd764ad99e6be8755767ca81e10f439e047014b7aaacffae5c`。不要覆盖已有其他用途的上游 checkout。

回到 YUME 根目录，把下面的路径替换为实际 Windows 上游绝对路径，再执行：

```powershell
$env:YUME_OPENCODE_SRC = "C:\replace-with-your-path\opencode-v1.18.21"
bun run prepare:workbench
bun run check
cargo +1.94.1 check --locked --manifest-path src-tauri/Cargo.toml
```

这些操作在 Windows 上建立新的基线，本次 Mac 交接没有代执行。套件、工作台生成、构建、性能采样顺序进行；出现失败先记录日志并区分平台环境与代码问题，再修复和做必要复测。`bun run build` 单独成功不代表工作台重建完成；`dev:client` 的 QA 配置也不代表生产启动验收。真实 OpenCode 辅助脚本如默认使用无扩展名二进制，须先检查其 Windows 路径/环境参数和清理规则，不照搬 Mac 命令。

## 下一阶段施工顺序

1. **Windows 本机基线与启动所有权**：固定依赖完整检查、生产配置检查；独立 QA 身份下验首实例、二次/并发启动、真实孤儿恢复，确认活跃宿主、引擎与工具任务连续，不误杀日常进程。
2. **WebView2 对应链路**：PDF 本地 worker/CSP、正常/损坏/超预算/取消及内存循环；原生 picker/drop、媒体/二进制上传、发送/历史重开；1000 消息 50 Hz、10k/50k 的终态、Stop、焦点、导航与滚动。Mac 数字只作来源记录，不作为 Windows 基线或性能对照。
3. **GUI 运行与存储闭环**：审批/拒绝/取消、SSE 断流/故障恢复、日志五工具/到期；历史删除/多窗口争用、记忆导出/忘记/重开。继续保持完整身份、所有权、墓碑、隐私和迟到事件边界。
4. **硬件与剩余成本**：2D/3D 全动作、隐藏/睡眠、跨屏 DPI/高刷、内存/GPU/能耗和同环境生产对照；M19c 完整删除外键扫描/小库成本、C16 全行挂载、R06/D20 残留复杂度。证据不足的项目仍列待验，不把平台转换当作完成。

Mac 专有的 GUI 审批/SSE/日志、WebKit PDF 堆/RSS、原生硬件与签名包范围继续留给 Mac 验证；Windows 结果不关闭这些缺口。完整逐项门槛以[施工计划第 11 节](performance-remediation-plan.md#11-b7验证退出条件与回退)和[验收待办](verification-backlog.md)为准。

## 数据、证据与收尾

Mac 本轮专属 QA 进程、虚拟凭据、端口与数据根已清理，5 个 SQLite 完整性通过，标准生产 dist 已恢复。[清理回执](performance-evidence/b7-macos-20261027/cleanup.json)和便携报告已入 Git。旧报告内“未推送”描述的是其采样时状态，不影响本次用户授权的分支交接。

Git 包含代码、迁移、计划、待办与 `docs/performance-evidence/` 下的报告、采样、截图和校验值。原始日志、完整 native 合成导出、构建缓存及工具链仍在 Mac 的忽略目录；Windows 无需复制它们即可接续，若要复核未入库原件再按其记录定位。冻结 `inputs/` 脚本只供复核，含当次 Mac 身份与路径，不能直接重跑。

每轮使用新的 Windows 隔离身份、输出目录和合成 provider；不复制钥匙串/DPAPI 凭据或日常库。迁移仅在一致性备份/合成副本执行，保留 WAL 与回退边界；清理只针对登记的 PID/出生时间/路径和专属目录。每批记录实际 HEAD、平台、命令/退出码、原始样本及未验项，更新本入口和统一待办，按当前用户授权保存提交。此次仅推送施工分支，不合并 main、不移动既有标签、不升版或发布。

## 给 Windows 接手助手的提示

> 继续 ciph/deskmate 的 B7。先读 AGENTS.md、docs/performance-windows-handoff.md、性能施工计划、verification-backlog.md 和最新 b7-macos-20261027 证据。保留本机改动，fetch 后接续 origin/codex/yume-performance-b6，核对 db96597 为祖先并记录实际 HEAD；不要回旧 main 重做 H18/M19。B6 已完成，历史 schema 3、记忆 schema 4、产品 0.4.21。先准备固定 Windows 依赖并跑本机基线，再验 Windows 首实例/并发/孤儿、WebView2、GUI 审批/SSE/日志与存储链路。使用新隔离身份，测试/构建/测量串行，保持 Mac/Windows 与生产/QA 证据边界。逐批更新进度和未验项，保留日常数据，不自动升版或发布。
