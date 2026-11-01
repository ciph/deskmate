# B7 Mac 接续结果（2026-10-26–27）

[HTML 报告](index.html) · [结果与日志哈希](summary.json) · [代码指纹](source-fingerprints.json) · [文件清单](manifest.json)

B7 **进行中**。从 `703c124` 接续，普通附件历史标签修复为 `c00528e`，导航提示过滤为 `dea6d77`。产品 0.4.21、历史 schema 3、记忆 schema 4、依赖与锁文件不变；未推送或发布。[10 月 25 日证据](../b7-macos-20261025/index.html) 保留原样，本页记录新增证据。

## 修复与自动回归

- 目录加载复用已有受完整身份约束的 native snapshot，携带用户 file part 的 ID 和有效文件名；不增加逐消息请求，不复制 URL、字节或路径到本地历史。仅附件的用户消息也能显示。兼容历史持久格式不变，local-only 记录不能借用附件标签。
- 对话导航复用消息气泡的内部资源上下文过滤，避免标题/可访问名称出现 `<yume-local-resources>` 及路径。
- 最终前端 **172 文件 / 1354 pass / 0 fail / 0 skip / 0 unfinished**；Rust **957 pass / 0 fail / 23 ignored**。固定 Bun 1.3.14 / Rust 1.94.1，套件串行，无测量期间并跑。类型检查、生产 cargo check、前端构建、隔离 QA App 构建通过。23 个 ignored 未在本轮冒充执行。
- React Doctor 0.9.12 扫描改动范围，保留 1 条既有 `only-export-components` 提示：`conversationTurns` 在基线已导出，没有新增该模式、抑制规则或为消除提示重构。Rust 13 个既有 warning 和 Vite chunk 警告仍保留；固定工具链无 rustfmt，不声称格式化通过。

## 原生证据

macOS 26.5.2 / ARM64，`worklog-qa` 未签名 debug App，专属身份 `com.deskmate.unifiedhistoryqa.b720261026a`。真实 WebKit、捆绑 OpenCode 1.18.21、本地合成 provider，无真实模型请求。QA 身份、二进制 SHA 和运行回执见摘要。

| 场景 | 本轮结果 | 边界 |
| --- | --- | --- |
| PDF | 原生选择器导入 601 B PDF，提取/发送后收到预期标记；历史重开与进程重启后仍显示文件名 | 本轮只关闭此前 PDF 徽标丢失观察；复杂 PDF、重复取消/堆收敛另验 |
| WAV / MP3 / MP4 | 12 s / 12.096 s / 8 s，播放推进、暂停、跳到中点均成功；发送后重开和重启仍恢复卡片 | WAV 原生 Play/Pause 有 UI 操作；seek 来自媒体 DOM API，未完成物理滑块拖动 |
| 原始二进制 IPC | 1 / 8 / 20 MiB 普通附件成功，20 MiB + 1 B 被拒绝；64 MiB WAV 被接收、播放、跳到接近末尾 | 调用生产 Tauri 原始 ArrayBuffer 入口；不是 OS 拖入/完整 File 选择路径；各大小单次观察，不报告 p95 |
| 1000 消息 | 官方 OpenCode import 导入 500 轮合成 Markdown，会话在原生聊天加载 | 全部 1000 行仍挂载，不是虚拟列表 |
| 非流式输入/滚动 | 各预热 5 次、采样 30 次；输入 p50/p95 **20/22 ms**，滚动 **20/21 ms** | DOM input/scroll 到第二个 requestAnimationFrame；非真实按键到显示延迟，非 50 Hz 流式，非前后对照 |

二进制单次上传观察为 3 / 8 / 19 / 65 ms，包含调用到宿主回复，不含缓冲区构造、后续媒体操作或清理。64 MiB 音频时长 2097.150625 s，播放推进 0.795926 s 后暂停，seek 到 2096.150625 s，无媒体错误。实际 HTTP Range 状态未采到，不能写成 206 已验证。完整输入/滚动样本见 [JSON](interaction-results.json)。

## 失败、恢复与证据来源

第一次检查器开启时的上传探针触发宿主 SIGTRAP；崩溃栈位于 WebKit Inspector JSON 序列化。该轮无完成结果，原因未定，不判断具体文件大小失败。后续先安排延迟任务、关闭检查器再上传，完整批次通过；这不等于已修复 WebKit 崩溃。

重建后 QA 启动停在 `SecKeychainFindGenericPassword`；核对 PID/出生时间后停止该进程，仅删除本次 QA 的虚拟凭据，再启同一二进制恢复。没有读取真实密钥或弱化钥匙串保护。新启动早期出现一次旧端口连接拒绝，随后引擎和历史加载正常。

直接 `fetch(chat-resource://...)` 的 Range 探针被现有 connect-src CSP 拒绝；宿主上传已成功。没有为探针放宽 CSP，随后改用实际媒体元素验证播放和近末尾 seek。原 combined probe 将后续 fetch 失败标成 `accepted=false`，摘要保留该原值及解释。

上传与 64 MiB 结果曾在 CUA 工具返回中完整显示；后续保存 AX 时拿到的是“无变化”diff。因此 [上传 JSON](binary-results-transcribed.json) 明确标为从工具输出转录，不能当作原始 AX 文件。媒体三格式、最终历史和交互结果有独立 AX 原文。初次编译/夹具错误、回环沙箱失败与复跑日志均留在本机输出并记录哈希。

## 清理与继续入口

最终 QA 正常退出，引擎随之结束；崩溃后的孤儿引擎、阻塞的 QA 进程及合成服务分别核对后退出。5 个 SQLite 库（含 OpenCode）完整性均 ok；上传探针资源无残留；占位凭据已不存在，两个端口关闭，Support/Caches/WebKit 三个本轮专属目录按创建时间核验后移除。PDF 的 SHA 与上轮夹具相同；四种原输入文件的最终哈希保留。没有读取/迁移日常库。

下一步优先补 **1000 消息 + 50 Hz 流式 + 10k/50k 字符尾消息**，验证终态、Stop、焦点和滚动，再做复杂/损坏/截断 PDF 的 20 次生命周期循环。Windows、GUI 权限/故障恢复、日志五工具、存储删除/多窗口、2D/3D/高刷/睡眠/DPI/GPU/能耗及同环境前后对照继续留在[验收待办](../../verification-backlog.md)。

`inputs/` 保存本轮实际使用的探针、隔离配置、导入器和合成附件，仅供复核；不是可直接重跑的通用入口。脚本原位置为 `output/performance-b7-20261026/`，依赖本轮已清理的隔离身份与回执。重现必须先按接续说明准备固定工具链，换用新的身份/输出目录，不指向日常应用数据。媒体夹具保留实际字节，不要求新机器生成同一编码哈希。原始日志、数据库、构建缓存不入库。
