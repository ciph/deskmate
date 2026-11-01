# B7 Mac 本轮结果（2026-10-25）

[HTML 报告](index.html) · [摘要/逐项矩阵/证据哈希](summary.json) · [代码指纹](source-fingerprints.json)

B7 **进行中**。源码起点 `31e7724`；时钟修复 `5170451`，原生实测发现的 PDF WebKit 兼容修复 `2026f8c`。产品版本仍为 0.4.21，历史 schema 3、记忆 schema 4。最终前端 172 文件 / 1352 pass，Rust 953 pass / 23 ignored；两套完整检查的原始日志均保留在本机输出目录。第一套检查之后发生了 PDF 修复，不能只读第一套结果。

## 实际验证

- 使用仓库固定 Bun 1.3.14 / Rust 1.94.1：`bun run typecheck` → `bun run test:frontend` → `cargo test --locked --manifest-path src-tauri/Cargo.toml --lib -- --test-threads=4` → `bun run build`，严格串行。PDF 定向分别运行 `attachments.test.ts`、`pdfPageText.test.ts`、`attachmentsPdfLifecycle.test.ts`，不把 DOM mock 文件合进同一 Bun 进程。
- 生产配置 cargo check；`bun scripts/audit-remediation/verify-native.ts` 运行真实 OpenCode + 合成 provider 的一项 ignored 宿主测试。其余 ignored 不冒充已执行。
- `bun run tauri build --debug --features worklog-qa --bundles app --no-sign --config <本轮 tauri.qa.json 的绝对路径>`。配置故意跳过重复 beforeBuild；必须先准备冻结依赖、固定 sidecar、完整工作台和生产前端。955 工作台文件与 H18a 完整构建指纹相同。此配置只用于本轮隔离 debug QA，不用于发布或日常安装。
- 使用 CUA 操作真实 Mac WebKit 窗口，默认桌宠、工作台首开/二开、二次启动、PDF 选择/提取/发送/回复、延迟请求 Stop/恢复、非空历史文本重开。真实模型未使用；provider 的占位值没有真实权限，已从 QA 独立钥匙串作用域删除。
- PDF 原错误由 WebKit Inspector 捕获；`ReadableStream[Symbol.asyncIterator]` 为 undefined，`getReader` 为 function。修复前自动回归复现同一 TypeError，修复后真实 PDF、取消/锁释放/XFA 与原生完整发送均通过。取消时 GUI 显示“出错了: Aborted”；历史重开未重现 PDF 徽标，作为待收口观察保留。

## 采样和边界

十分钟采样在 **PDF 修复前的 debug App** 上完成：600.003 秒、120 个样本，默认 GIF/空聊天可见、设置/工作台隐藏，无模型请求、无并行构建/测试；用户其他应用继续运行。`ps` 的 `%cpu` 是系统估算，不是区间 CPU 计账。仅记录归属明确的宿主和 OpenCode，排除无法归属的 WebKit/GPU；文件观察只比较本次 QA 目录的 size/mtime，不能证明完全无磁盘读取或后台唤醒。

没有同环境优化前原生对照；不据此宣称省电、尾延迟稳定或最终生产性能达标。Windows、长会话、高刷/睡眠/多屏、全角色动作、媒体及完整存储删除/恢复等仍待验。

## 脚本与清理

`inputs/` 逐字节保存本次实际运行的脚本及 601 B 合成 PDF，供复核；**不是通用启动入口**。脚本使用固定 QA 身份，原执行位置为仓库 `output/performance-b7-20261025/`，依赖该目录中的执行回执；原始输出被忽略，不假定新机器存在。重现时先按接续说明准备环境，并使用新的隔离身份、独立输出目录，核对路径后相应调整配置与脚本，绝不指向日常数据。

两次 App 均正常退出且 OpenCode 随之结束；夹具 PID/端口、占位凭据、Support/Caches/WebKit 三个新建专属根已按 PID/出生时间和目录创建时间校验后清理。原始 PDF 未改动；新鲜 QA 四个 SQLite 库完整性均 ok。`summary.json` 保存清理回执及原始文件 SHA，原始日志/数据库不入库。未推送、安装、签名、公证或发布。
