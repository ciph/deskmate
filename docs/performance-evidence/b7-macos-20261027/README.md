# B7 Mac 流式与 PDF 生命周期（2026-10-27）

[HTML 报告](index.html) · [结果与日志 SHA](summary.json) · [代码指纹](source-fingerprints.json) · [证据清单](manifest.json)

B7 **进行中**。起点 `1add56b`，流式修复提交 `3069953`。产品 0.4.21、历史 schema 3、记忆 schema 4、Rust 与依赖不变；未推送、安装、签名或发布。[上轮证据](../b7-macos-20261026/index.html) 保持原样，本页只新增本轮范围。

## 实际问题与修复

捆绑的 OpenCode 1.18.21 在 text-start/text-end 发送完整 `message.part.updated`，中途发送 `message.part.delta`。此前轻聊天只处理前者：在真实 WebKit + OpenCode + 本地合成 provider 的 10k 回复中，仅观察到空白占位和最终正文两次 DOM 文本变化；最终持久化全文正确，但中途没有显示文本。这不是一组有效的流式性能基线，不能拿其输入耗时作优化前后对照。

修复把已确认 text part 的增量接入现有 33 ms 缓冲。首个非空内容立即发布，完整快照仍替换该 part；保留 part 顺序、会话/请求父消息/用户角色/Stop 边界。未知或 reasoning part 的 delta 不会显示。固定上游源码证据位于本机 `output/upstream/opencode-v1.18.21/packages/opencode/src/session/processor.ts` 的 text-start/delta/end 与 `session.ts` 的 PartDelta 发布处。匿名增量没有事件 ID，不新增重放去重承诺。

新增回归先复现 2 个失败，修复后通过；覆盖终态替换、跨会话/用户/旧请求/推理内容拒绝、Stop 后迟到增量、UTF-16 分片及 10k/50k 的 50 Hz 缓冲。最终前端 **172 文件 / 1359 pass / 0 fail / 0 skip / 0 unfinished**，类型检查、生产构建和隔离 QA App 构建通过。本轮 Rust 未修改、未重跑；上一轮 957 pass / 23 ignored 只作为历史依据。React Doctor 留 2 条既有模式提示（多 useState、稳定回调依赖），已对照基线审阅；没有抑制或借此重构。

## 原生流式验收

macOS 26.5.2 ARM64，未签名 `worklog-qa` debug App，专属身份 `com.deskmate.unifiedhistoryqa.b720261027a`。官方 OpenCode import 分别导入两个独立的 500 轮/1000 消息合成会话。生产 composer → Tauri/OpenCode → localhost provider → 原生 SSE → 生产 DOM；无真实模型或付费调用。每个长回复分成 500 块，按 20 ms 间隔计划发送，实际时间戳保存在 [provider 回执](provider-requests.json)。

| 场景 | 10k 字符 | 50k 字符 |
| --- | ---: | ---: |
| 初始 / 最终消息行 | 1000 / 1002 | 1000 / 1002 |
| DOM 文本变化次数 | 232 | 212 |
| 输入 p95 | 23 ms | 28 ms |
| 滚动 p95 | 33 ms | 38 ms |
| 流式期间保留采样 | 30 / 30 | 30 / 30 |
| 原有 DOM 节点保持 | 1000 / 1000 | 1000 / 1000 |
| 输入焦点 / 草稿恢复 | 通过 / 通过 | 通过 / 通过 |
| 最终全文 / 代码块 | provider 与 native SHA 一致 / DOM 逐字一致 | provider 与 native SHA 一致 / DOM 逐字一致 |

测量为程序化 DOM input/scroll 到第二个 `requestAnimationFrame`：5 次预热后保留 30 对样本，间隔 200 ms；采样窗口记录当时 busy 与正文长度。检查器在延迟任务触发前关闭，期间没有构建或测试套件。全部历史行仍挂载，探针读取 DOM 也有开销；不是物理按键到显示、生产包、FPS/GPU/能耗或跨平台达标证据。`firstAt` 可能包含空白占位，不当作首 token 指标。原始结果：[10k](fixed-10k.json)、[50k](fixed-50k.json)、[全文校验](native-verification.json)。

Stop 在上游发送 162/500 块时中断，provider `aborted=true / done=false`；native 保留 16,100 字符的已接受正文。现有 UI 会另外显示临时「出错了: Aborted」提示，未在这次流式修复中改写其文案。随后新请求收到 `B7_LOCAL_REPLY_OK`，无残留 busy，焦点和草稿正常；该临时错误行不写成额外 native 消息。Stop/恢复样本不全部发生在流式中，不作为上述 p95 统计。[停止结果](stop.json) · [继续结果](resume.json)。

## PDF worker 生命周期

最终修复二进制中，正常、损坏、超预算、取消交替执行 **32 次（各 8 次）**：

- 正常：2 页、1788 B，提取 808 字符，8 次 SHA 相同。
- 损坏：41 B，8 次均为预期 `InvalidPDFException`。
- 超预算：100 页、396726 B，得到 160,000 字符正文加 28 字符截断提示，8 次 SHA 相同。
- 取消：同一 100 页夹具，5 ms 后中止，8 次均为预期 `AbortError`。

真实 WebKit/PDF.js worker 创建 **32**、终止 **32**，峰值 **1**，每次结束活动数 **0**。QA 独立入口导入与聊天共享的生产附件模块，使用相同 CSP/本地 worker；没有修改全局 ReadableStream 原型。此 WebKit 确实缺少异步迭代器，走现有 reader 兼容路径。不是完整文件选择/发送 UI 循环，也未覆盖 XFA、扫描件或所有复杂文档。[最终结果](pdf-final.json) 与 [原始 AX 导出](pdf-final-ax.txt) 保留。

修复前另跑过同样 32 次（PDF 代码未改），记录保留在 [首次循环](pdf-cycle.json)，不与最终 32 次混为单批。`performance.memory` 不可用，前后 heap 均为 null；没有测得 JS heap/RSS 收敛，不关闭内存验收。

## 失败、环境与清理

第一次全套前端因沙箱不允许监听 localhost，4 个文件失败（Bun 报 port 0，不据此判断实际端口冲突）；允许本地夹具后完整 172 文件通过。原失败/复跑日志都保留哈希。首次 CUA `getApp` 工具耗时 29947.4393 秒；该时间是 UI 工具停顿，未计作应用启动或性能样本。切换历史后控制台曾失去焦点，确认请求未发出后关闭/重开检查器恢复，没有把空跑计入结果。

重建前正常退出旧 QA，只清理该身份的虚拟凭据，再由最终二进制迁入同一个虚拟值，避免旧未签名二进制的钥匙串所有权干扰。没有读取真实凭据或降低系统保护。启动早期的旧端口连接拒绝与现有更新源提示保留在本机日志。

最终宿主正常退出，引擎随之结束；合成服务按精确 PID/出生时间/命令退出。5 个 SQLite（含 `.sqlite3`）完整性均 ok；本轮 Support/Caches/WebKit 根、虚拟凭据及两端口已清理。[清理回执](cleanup.json)。标准 `bun run build` 已恢复：只有 chat/pet/settings 三个 HTML 入口，`b7-qa.js` 不存在。QA 额外入口改变构建图，不能称 QA dist 与生产 dist 逐字相同。

## 接续

下一轮优先补 **Mac GUI 审批、SSE 断流/故障恢复与日志五工具/到期链路**，继续沿用新的独立 QA 身份。PDF 堆/RSS、真实模型、Windows/WebView2、历史删除/多窗口、记忆导出/忘记/重开、2D/3D/隐藏/睡眠/DPI/高刷/GPU/能耗与生产同环境对照继续待验。M19c 外键扫描与小库成本、C16 全行挂载、R06/D20 复杂度保留。见[统一待办](../../verification-backlog.md)。

`inputs/` 是本轮实际探针、配置与清理脚本的冻结副本，原路径为 `output/performance-b7-20261027/`，仅供复核，不能直接按旧身份重跑。重现时更换身份、输出目录和回执，先准备固定工具链；不要指向日常数据。原始日志、完整 native 导出、数据库、构建与依赖缓存留在忽略的本机 output，不进入便携证据或发行资产。
