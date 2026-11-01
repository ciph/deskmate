# B7 Windows 冷启动历史发现修复（2026-10-27）

[HTML 报告](index.html) · [完整结果](summary.json) · [源码指纹](source-fingerprints.json) · [日志哈希](raw-log-manifest.json) · [冻结清单](evidence-manifest.json)

冷启动历史发现超时及初始化关闭历史面板已修复，最终全新身份与热重启复测通过；继续完成了真实多窗口所有权/目录更新和原生搜索控件验收。**B7 仍进行中**，不宣称完整物理 GUI 或所有性能门槛通过。

工作目录 `E:\Codex\工作伙伴\deskmate-windows-b7`，分支 `codex/yume-performance-b6`，起点 `db07e5bdb02550c4d165da8cd45fb76debefbe17`。版本 0.4.21、历史目录 schema 3、记忆库 schema 4；依赖和版本未变。本轮叠加在先前 Stop/焦点改动之上，未提交、推送、安装或发布。原始日志和修改前源码在 `E:\Codex\工作伙伴\reports\yume-b7-cold-history-20261027`。

## 原因与修复

OpenCode 的全局健康检查先就绪，但首次项目初始化仍会安装插件依赖。固定 1.18.21 源码的 `config/config.ts` 安装 `@opencode-ai/plugin`，`plugin/index.ts` 在加载本地插件前等待这些依赖；本机 sidecar 日志显示首次项目初始化耗时约 29–66 秒。原先历史发现直接使用普通 5 秒读取预算，因而产生 `history_native_timeout`；随后初始化创建会话还会关闭用户已经打开的历史面板。原始失败见 [baseline-first-cold.json](baseline-first-cold.json) 和 [baseline-sidecar.log](baseline-sidecar.log)。其中 `ok` 只代表旧探针执行结束，不代表产品验收通过。

- 历史后台维护先等待宿主和全局服务就绪，再用独立 **120 秒**预算等待默认项目完成初始化；此阶段不持有 Agent 操作锁。就绪缓存随 sidecar generation 失效，防止引擎重启后误用旧状态。普通历史读取仍为 5 秒，presence/preview 保留原有短预算；初始化失败仍报告错误。
- 返回瞬时 `CatalogPage.refreshing`，发现未结束时显示加载状态，已有缓存行保持可见，结束后发目录事件自动更新。没有修改持久化 schema。
- 仅初次创建会话保留用户已打开的视图；显式新会话仍按原意切换。
- 实机还发现 WebView2 在宿主 `manage` 前调用同步历史入口可触发 panic。同步入口改为有界错误；异步列表在后台最多等 10 秒，避免启动时短暂闪出 `history_state_unavailable`。新增边界回归测试。

这修复的是初始化被误判为普通读取超时，以及相关启动竞态，**没有消除首次插件依赖安装的等待时间**。超过预算或依赖安装失败仍会报错。网络更慢的首次启动、其他项目首次初始化仍需后续覆盖。

## 冷启动复测链

| 阶段 | 证据 | 实际结果 |
| --- | --- | --- |
| 原版本 D | [原始观察](baseline-first-cold.json) | 5 秒读取预算报错，历史面板被初始化关闭；缓存错误持续存在 |
| 候选 E | [早期 panic 日志](app-134223406.stderr.log)、[加守卫后](candidate-first-project-cold.json) | 修复 `state()` 初始化竞态；项目约 28 秒后发现。该轮已有崩溃前创建的空目录库，不冒充全新身份 |
| 60 秒候选 F | [失败原件](final-first-cold.json) | 项目约 63 秒才就绪，60 秒预算仍不足；先报超时，后来自动发现 |
| 120 秒候选 G | [冷启动](acceptance-first-cold.json)、[热启动早期闪错](acceptance-warm-restart.json) | 全新身份约 65.17 秒通过；热启动暴露宿主 state 早期竞态，继续修复 |
| 最终 H | [启动前缓存证明](complete-prelaunch-cache.json)、[冷启动](complete-first-cold.json)、[热重启](complete-warm-restart.json) | **66.16 秒 / 2.03 秒**自动出现两条各 1000 消息的会话；无错误、无假空态、历史面板保持 |

H 首次启动前没有 YUME 历史目录库、sidecar 日志或两个配置目录的 `node_modules`，仅通过官方 native import 导入合成消息。时间从 React 就绪后的 DOM 探针开始计算，非进程启动总时延，也非统计 p95。最终构建的 [九个产品源码哈希](complete-qa-build.json) 与冻结时一致。先前失败和错误探针断言均保留，解释见 [diagnostic-attempts.json](diagnostic-attempts.json)。

## 通过后继续的验收

- [多窗口](multiwindow-final.json)：真实 Tauri/WebView2 chat、workbench、settings 与真实 OpenCode。Workbench 接管后 chat 只读；settings 越权列表、chat 抢占与过期本地保存均被宿主拒绝。改名跨窗口更新，chat pin 与 workbench rename 并发结果都保留；归档/恢复正确，释放后 composer 回来，1000 条原消息保持。两个窗口各收到 15 次真实目录事件。
- [原生 GUI 搜索](native-gui-final.json)：Sky Windows UIA 读取控件并 `set_value`，实际 React 搜索结果由 2 条变 1 条、清空后回到 2 条。历史面板由 CDP 打开。窗口捕获超时，鼠标输入无几何信息；**未通过物理鼠标、完整键盘或视觉验收**，原始失败在 [native-gui-accessibility.json](native-gui-accessibility.json)。
- [流式回归](stream-regression-checks.json)：两个独立的 1000 消息场景；完整 10k 为 500/500 块，native 10000 字符全文 SHA 与 provider 一致；Stop 在 50k 的 153/500 块中断，后续回复 `B7_LOCAL_REPLY_OK`。旧 1000 节点、原 composer、流式焦点与草稿保持。程序化重放目录事件检验先前修复，不据此证明首批原事件的具体来源，不作为新性能对照。
- [前端全套](frontend-summary-final.json)：172/172 文件，**1360 pass / 0 fail / 8 skip / 0 unfinished**；[Rust 全套](rust-full-final.log)：**970 pass / 0 fail / 24 ignored**；[三套类型检查](typecheck-final.log) 通过。
- [React Doctor](react-doctor-comparison.json)：changed scope 无新增诊断、exit 0；同样 7 文件前后均 81 分，12 条既有诊断规则集合相同。两个全文件扫描 exit 1 来自既有诊断，未宣称它们通过；前次 83 分只扫描 4 文件，不能直接比较。
- [正常前端构建](production-frontend-build.json) 与 [正常 Windows debug/no-bundle 编译](production-desktop-build.json) 通过，最后 EXE 为正常配置 `yume.exe`，未运行生产身份。最终 GUI QA 使用标准前端产物；CDP 观察器仅存在于隔离 QA 会话，没有产品 QA 入口残留。

## 保留、清理与边界

[保留复核](preservation-check.json)：旧 E 盘 0.4.3 主仓按相同 porcelain/all 命令比较状态一致，原 C 盘 0.4.21 工作树仍干净；未在二者写入。当前 checkout 的既有 sharedConversation 两文件、Bun/Cargo 锁文件与本轮开始时字节一致。ChatApp 和相关测试叠加修复，先前 Stop 回归通过。旧主仓状态比较不冒充全文件字节比较。

两个先前报告的冻结清单也已核对。首批报告 README 在前次任务增加过 Stop/焦点后续链接，原 manifest 未更新；去掉仅该段后能复现原 SHA，其余 52 项全部匹配。Stop/焦点报告 128 项全部匹配。本轮不改写旧报告、原失败或原清单。

[25 个 SQLite 完整性检查](sqlite-integrity.json) 全部通过。五个 QA 身份的虚拟凭据均已清除，十个精确限定的数据根已删除；所属进程与本地 provider 退出，两个端口关闭，见 [cleanup.json](cleanup.json)。没有使用日常数据。探针、QA 配置、管理/清理脚本冻结在 `inputs/`；脚本包含本轮专属路径和身份，复跑需新建隔离身份、保持路径与进程校验，不能直接替换为生产身份。

下一步见 [统一待办](../../verification-backlog.md)：物理 GUI picker/drop、媒体/二进制发送与历史重开、GUI 审批/SSE 断流/恢复、日志五工具/到期、记忆迁移恢复、活跃工具二次启动，以及 PDF heap/RSS、2D/3D、隐藏/睡眠、DPI/高刷、GPU/能耗和同机生产对照。Mac 专有范围与 M19c/C16/R06/D20 成本继续保留。本轮多窗口目录/所有权通过不等于所有多窗口故障与迁移场景完成。

报告本身的本地 HTML 预览被浏览器 `file:` URL 策略拦截，未绕过。静态内容、53 个报告链接与证据哈希已检查；不宣称报告经过浏览器视觉检查。此限制与上文应用原生截图/鼠标限制分别记录。
