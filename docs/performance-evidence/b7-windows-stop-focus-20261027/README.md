# B7 Windows：首次长会话 Stop 与焦点修复

[HTML](index.html) · [结果](summary.json) · [逐场景校验](stream-checks-final.json) · [源码指纹](source-fingerprints.json) · [原始日志索引](raw-log-manifest.json)

2026-10-27，从 `codex/yume-performance-b6` / `db07e5bdb02550c4d165da8cd45fb76debefbe17` 接续，工作目录 `E:\Codex\工作伙伴\deskmate-windows-b7`。本轮修改聊天展示控制与回归测试；产品仍为 0.4.21，依赖、锁文件、历史 schema 3 / 记忆库 schema 4 不变。未提交、推送、安装或发布。旧主仓及 C 盘工作树保留。**本轮关闭已复现的 Stop/composer 丢失机制；B7 仍进行中。**

## 复现与修复

首次报告的失败原件仍在[上一批报告](../b7-windows-20261027/README.md)。本轮修复前，新的 1000 消息首轮 10k 自然运行通过，说明异常并非每次发生；随后在另一个新 1000 消息会话的真实流式中重放 `history-catalog-changed`。宿主真实返回 `ownership=unowned / runtime=running / send=false / readOnlyReason=active_task`，4028.1 ms 发出事件，4049 ms 输入框、Stop、焦点同时消失。[修复前时序](baseline-event-10k.json)。没有伪造 catalog 返回值。原首轮未记录事件时序，因此不声称已经证明那次触发事件的具体来源。

原因是把“不能再发送一条消息”的能力值直接用来卸载整个 composer，连本视图正在运行的回复的 Stop 和草稿输入也一起移除。修复只为本视图发起、完整 catalog key 匹配、可用且未归档/未删除、无外部 ownership 的当前 native 回复保留控制。外部运行、工作台/Agent 所有权、未知状态和不可用会话仍遵循原权限。新发送仍重新向宿主校验能力和完整身份。

正常结束、Stop 与错误结束先刷新当前能力，再撤销本地控制范围；旧视图及旧目录刷新结果不能覆盖新视图/新刷新。终态先 flush 再 reset 增量缓冲，拒绝迟到 delta 继续追加到最终正文，保留完整快照替换。新增测试验证目录刷新期间及三种结束方式后的原输入节点、焦点、草稿、Stop、后续发送，以及外部后续运行不能借用已结束的本地控制范围。

## Windows 原生复测

同一 Windows 10 / WebView2 155.0.4283.62、2560×1440 / 60 Hz / DPR 1.25；Bun 1.3.14、Rust 1.94.1、OpenCode 1.18.21。修复前后分别使用专属 QA 身份 `…winb720261027b` / `…winb720261027c`，真实捆绑引擎、localhost 合成模型输出、生产前端模块及本地资源，额外 QA 入口/CDP。官方 `opencode import` 在进程退出时导入四份独立 1000 消息夹具。每个流式用例都在运行中重放相同目录事件，验证真实宿主能力仍为 running/send=false。

| 新会话首轮 | 10k | 50k | Stop | 重启后另一会话 10k |
| --- | --- | --- | --- | --- |
| 初始 / 最终 UI 行 | 1000 / 1002 | 1000 / 1002 | 1000 / 1003 | 1000 / 1002 |
| 旧节点保持 | 1000 | 1000 | 1000 | 1000 |
| DOM 正文变化次数 | 171 | 172 | 53 | 173 |
| 流式中 Stop 可见样本 | 30/30 | 30/30 | 主动点击前可见 | 30/30 |
| 流式焦点 / 草稿 / 原输入节点 | 保持 | 保持 | 保持 | 保持 |
| native 全文 SHA / DOM 代码块 | 一致 | 一致 | 中断，不要求完整 | 一致 |
| 输入 / 滚动 p95 | 32.4 / 40.5 ms | 33.9 / 38.8 ms | 不作性能样本 | 31.6 / 36.7 ms |

500 块、计划 50 Hz；每组 5 对预热后 30 对 input/scroll 到第二个 rAF，间隔 200 ms。不是物理按键、FPS、GPU、生产包前后对照，仍全量挂载历史行。测试/构建与采样串行；没有建立整机空闲基线。

Stop 时 provider 在 158/500 块中断，native 保留 15,700 字符及 MessageAbortedError；同一会话随后收到 `B7_LOCAL_REPLY_OK`，native 共 1004 条消息。临时 UI 的「出错了: Aborted」使 Stop 后有 1003 行，未新增持久错误消息。快速续发时发送阶段现有 `disabled={isSubmitting}` 会使发送前焦点失去，回执明确为 false；这不计入流式焦点通过，也没有声称本轮修复了所有发送前焦点行为。[继续](fixed-resume.json)、[native](fixed-native-predelete.json)、[provider](provider-requests.json)。

冷启动最早打开历史列表仍观察到 `history_native_timeout`，未开始发送；刷新也曾超时，后续目录可用后才打开尚未发送的 1000 消息会话。原失败保存，**不声称冷启动历史发现已无问题**。重启后的第二份新会话复测通过。[入口失败](fixed-cold-entry-inspect.json)、[首次刷新](fixed-cold-entry-refresh.json)、[后续 native discovery](fixed-discovery-health.json)。

## 继续完成的存储验收

- 在真实 settings IPC 创建两条明确合成记忆，导出 2 条；忘记其中一条后搜索为 0，另一条保持。重启后导出仍为 1 条，原忘记项不返回；最后忘记保留项，导出为 0。**导出格式 schemaVersion=1，数据库 user_version=4，两者不同。** 初版探针混淆两种版本的断言失败保留，更正探针后沿用同两条数据，未改产品实现。[导出/忘记](memory-cycle.json)、[重启](storage-restart.json)。
- 通过 chat 的正式 history mutation 删除 Stop/续发会话（native 1004 条），宿主 load 返回 `history_deleted`，列表不再出现，完整作用域 native GET 返回 404；重启后仍成立。没有直接写 OpenCode 数据库。[删除](history-delete.json)。
- 上述为真实宿主/SQLite/引擎链路，使用程序化调用；不是 GUI 删除确认框、系统导出保存框、多窗口争用或迁移恢复验收。

## 自动检查、证据与收尾

新回归修复前 2 fail；修复后 fixed-reply 22 pass、shared-conversation 5 pass、终态相关 28 pass。第一次完整前端检查捕获终态晚到 delta，修复后重新跑完整套件，最终退出码、逐文件计数见 [summary](summary.json) 和 [完整前端日志](frontend-full-final.log)。三套类型检查、React Doctor changed-scope、标准前端及正常配置 Windows debug/no-bundle 构建的实际结果也在 summary。Doctor 为 83/100，4 文件诊断为空；不把评分当作全仓无问题。Rust 未修改，本轮没有重跑 Rust 全套；上一批 966 pass / 24 ignored 是先前基线。

本机完整原始日志在 `E:\Codex\工作伙伴\reports\yume-b7-stop-focus-20261027`。44 MB 的红灯日志只在原始目录保存，便携证据保留摘要及 SHA。`inputs/` 冻结当次脚本/配置，仅供复核，重跑须重新选择身份、端口和进程回执，不直接照搬清理脚本。初版尝试包装 Tauri invoke 未截获内部调用；只以显式 native-access 回执和真实事件/DOM 时序为证据。

专属虚拟 provider 经 settings 事务清空；仅停止登记且核对 PID/出生时间/路径的 QA 进程和本次 provider。两个身份共十个 SQLite 完整性、记忆已忘记合成标记在数据库/现存 WAL/SHM 的字节检查、端口与专属根清理见 [完整性](sqlite-integrity.json) 和 [清理](cleanup.json)。字节检查不代表磁盘未分配区或外部备份的安全擦除。标准 dist 恢复，无 QA JS；最后 EXE 为正常配置 debug 编译件，没有运行生产身份或安装。

## 下一步

继续定位冷启动历史发现超时，补 Windows GUI picker/drop、媒体/二进制发送与重开、审批/拒绝/取消、SSE 断流/恢复、日志五工具/到期、历史多窗口和记忆迁移恢复；活跃工具中的二次启动、PDF 堆/RSS、2D/3D/隐藏/睡眠、DPI/高刷/GPU/能耗与同机生产对照仍待验。Mac 专有范围、M19c/C16/R06/D20 成本继续保留。无本轮视觉截图通过声明，B7 未达到全部退出条件。
