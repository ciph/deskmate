# 维护和验收脚本入口

2026-09-30 核对。命令从仓库根执行；Bun 1.3.14，依赖由根 `bun.lock` 固定。实际发布从 [本机发布指南](../docs/local-release-runbook.md) 开始；本索引登记已有能力，不自动授权真实模型请求、外部发布或用户数据操作。

## 构建、资源和检查

| 入口 | 输入 / 用途 | 输出、平台及退出条件 |
| --- | --- | --- |
| `bun run typecheck` / `bun run test:frontend` / `bun run test:rust` / `bun run check` | 当前源码与受管测试入口 | 前端逐文件进程隔离；Rust 宿主相关测试需要实际平台。裸 `bun test` 不代替仓库 runner |
| `bun run prepare:sidecar` | 固定 OpenCode、ncmdump 版本 | 当前平台的忽略二进制；不包含 Windows MCP |
| `bun run prepare:windows-mcp` | 维护者手动校验固定 Windows MCP | Windows x64 的忽略资源；ZIP/exe 双哈希。产品首次启用改由宿主下载，维护资源不打包 |
| `bun scripts/check-package-resources.ts` | Tauri `beforeBundleCommand` 发布资源检查 | 拒绝 frontend/OpenCode 的 `.map`、`.pdb` 及 Windows MCP 随包配置 |
| `bun run prepare:workbench` | [固定上游 UI 与本仓 overlay](../docs/workbench-build.md) | `public/workbench/`，含 terminal WASM；上游输入与 W1/W2 限制见构建契约 |
| `bun run fetch:persona-assets` | pinned 私有角色归档；显式资产下载凭据 | 忽略的 GLB/PNG；固定文件数与哈希校验，过滤旧 `figure3d.json`/provenance，marker 命中也清理 |
| `bun scripts/prepare-personas.ts --envelopes-only` | 已存在的内置变体 GLB、当前动作包络算法及角色配置；不下载 | 生成 `src/pet/variantEnvelopes.generated.json`，实际模型 hash/算法版本/scale/clipRoots 校验；无变化不重写，缺模型报错。替换模型后必须重生成并视觉回归；普通资产 fetch 不生成包络 |
| `scripts/prepare-xiaozhu-sandaime.ps1` | Windows 模型生成器及现有二代模型 | 三代 GLB 与 `docs/assets/` 溯源；PowerShell、Python，需模型结构/动作测试 |
| `bun scripts/pack-personas.ts` | 现有作者 pack CLI 参数、当前 persona/skill 及封面 | `.dmpack`；按渲染器校验必需文件，只接收允许的作者元数据；参数见文件顶部 |
| `scripts/pack-archive.ps1` / `scripts/pack-inspect.ps1` | Windows 作者 archive/检查 CLI | 保留作者操作职责；不作为自动产品启动入口 |
| `scripts/pack-upgrade.ps1 -SourceArchive ... -OutputArchive ... -MetadataPath ... -ReportPath ... -Version X.Y.Z` | 已有包的 manifest/封面升级 | Windows；新包及逐 entry SHA-256 报告。原资产字节必须保留，不覆盖源包/现有输出；与重新打包不同，保留此入口 |
| `scripts/persona-packs/POSITION.md` | 作者位置/rig/GIF打包约束 | [位置说明](persona-packs/POSITION.md)；`pack-position`/`pack-figure2d`/`pack-rig2d` 为被打包 CLI 使用的模块 |
| `scripts/audit-remediation/verify-native.ts` | 原生故障/恢复矩阵 | 独立临时根与证据；用法见文件顶部及[整改计划](../docs/audit-remediation-construction-plan.md) |

Tauri 的 `src-tauri/gen/schemas/` 来自 `tauri-build` 和实际 capability/plugin 源，是忽略的开发生成物。首次需执行 `bun run test:rust` 或 Tauri dev/build 生成编辑器 schema；当前权限源保留在 `src-tauri/capabilities/`。仓库历史中的生成 JSON 不能代替现行构建检查。

## 聊天头像验收与素材

- `scripts/automation-qa/chat-preview.html?avatars` 使用真实聊天组件与隔离的浏览器 IPC 夹具。
  同源打开 `?settings&avatars` 可验证真实通用设置组件；头像通过 localStorage/storage
  模拟持久化与跨窗口事件，仅在此预览入口生效。发送包含“排版”的消息会返回长代码块与表格。
- `scripts/automation-qa/chat-preview.html?historyAvatars` 提供混合角色历史，覆盖小熊虫、包包、三代目与无身份/已移除角色；图片路径由验收浏览器映射到本机包素材。
- `scripts/automation-qa/avatar-render.html?persona=xiaozhu` 从现有模型导出 128px 静态 PNG。
  参数也支持 `xiaozhu-nidaime`、`xiaozhu-sandaime`；开发服务器启动后读取
  `document.body.dataset.avatar` 的 PNG data URL，保存到 `public/persona-avatars/<id>.png`。
  使用与桌宠相同的模型、材质和 idle 动作；此作者入口不打包到产品中。
- 导入角色可在既有允许的 PNG 路径内提供 `personas/<id>/avatar.png`，无需更改包 manifest。
  缺图回退到已验证的包封面，再回退到名称标识。

## 引擎、权限、工作台及工作记录

下列合成 harness 使用新建的临时 workspace、隔离配置与 loopback provider；核对证据中的进程、端口和临时目录清理结果。CLI 是独立入口，不能因没有 import 就删除；`client`/`runtime`/provider/evidence/fixture 模块是这些入口的共享实现。

| 入口 | 输入 / 验证职责 | 输出与执行前提 |
| --- | --- | --- |
| `bun scripts/agent-qa/contract.ts` | 核心目录隔离、read/edit/bash/webfetch、unknown deny、取消和恢复 | 当前时间戳 `.omo/evidence/yume-agent-mode-sol/*`，固定 sidecar、loopback |
| `bun scripts/agent-qa/contract.ts --case permission-paths` | 中文空格目录；query/header 的 read批准、edit/write拒绝、字节不变及同目录权限绑定 | 已取代四个 permission spike；Windows普通/扩展 filesystem alias 的 realpath身份与 host wire归一化只在 Windows 执行；raw扩展HTTP别名未验证，其他平台明确 skipped |
| 同一 contract 的 lifecycle/recovery/recovery-malformed/discovery-failure/spawn-failure 模式 | setup/中断/恢复失败 | 模式参数见 main；成功与预期失败都需核对 exit code 和独立证据 |
| `bun scripts/agent-qa/flows.ts` | 合成文档/修复代码完整流程 | 实际工具、原文件字节与最终测试；`--negative` 预期退出失败 |
| `bun scripts/agent-qa/tool-lifecycle.ts` / `p4-cancel-matrix.ts` / `p4-shared-service.ts` | 子进程、Stop、拒绝/出错及会话并发隔离 | 时间戳 `artifacts/opencode-native/`；共享服务场景会启动对应 Rust 测试，需可用 Cargo |
| `bun scripts/agent-qa/p5-mcp-inventory.ts` | pinned Playwright + Windows MCP 来源/schema/最小工具清单 | Windows、`npx.cmd`、已准备 MCP；旧 candidate inventory 的独有来源信息已合入 |
| `bun scripts/agent-qa/p5-browser-e2e.ts` | 合成浏览器成功/失败/拒绝/取消矩阵 | 固定 Playwright MCP/浏览器；具体矩阵见 CLI，独立证据与清理 |
| `bun scripts/agent-qa/p5-windows-e2e.ts [scenario] [exe]` | Notepad 成功、拒绝、窗口消失、保存失败、Stop | Windows；默认取 pinned `resources/windows-mcp/1.3.24`，无二进制或平台不符时在创建 fixture 前失败；实际进程所有权/cleanup边界见[待验清单](../docs/verification-backlog.md) |
| `scripts/agent-qa/p5-real-browser.ts` / `p5-real-windows.ts` | 自选实际模型/真实工具验收 | 显式 `YUME_P5_LIVE_KEY`；有模型费用和 GUI 前提，本轮未执行，不属于合成测试 |
| `bun run verify:mcp-permission` / `bun scripts/verify-tool-permissions.ts` | MCP 与工具权限回归 | `sidecar-harness/` 提供共享传输、进程、隔离配置及清理；显式合成端点 |
| `bun scripts/verify-worklog.ts` | 工作记录 bridge 与全流程证明核验 | fixture运输不能证明真实语义或 GUI；外部原生回执要求见 `worklog/full-flow.ts`，默认 `.omo/evidence/task-6-worklog-natural-recall-native.json` |
| `scripts/worklog-qa/run.js`、provider/readback/seed/capture | 独立工作记录桌面身份及持久化验收 | [领域操作说明](worklog-qa/README.md)；共享显式 evidence目录，seed只写 receipt拥有且已停止的 QA数据库；capture只接受receipt拥有的窗口 |
| `bun scripts/worklog-qa/verify-provider.js` / `verify-report.js` | 自有provider一次工具/401/503/SSE与报告schema | provider默认新时间戳目录；精确进程与端口关闭，报告fixture无需桌面 |
| `scripts/workbench-qa/unified-history-desktop.ps1` / `unified-history-native-smoke.ps1` | 工作台统一历史、身份/归属与原生读回 | Windows、独立 workbench QA 身份；保留与工作记录领域不同的 launch/stop/purge 回执；`provider`/`cdp` 为受管fixture和桥 |
| `bun scripts/automation-qa/verify-sidecar.ts` | 自动记忆工具运输与内置 sidecar | 隔离合成输入/loopback；当前Mac二进制路径，其他平台前提需核对 |
| `bun scripts/automation-qa/verify-semantics.ts` | 真实模型记忆/工作记录语义矩阵 | 显式 `OPENAI_BASE_URL`/`OPENAI_API_KEY`/`MEMORY_QA_MODEL`；时间戳 `artifacts/automation-qa/`，可设 `YUME_MEMORY_QA_OUTPUT`；非hard-negative需人工审阅 |

QA中的固定业务日期、Windows标准系统路径和PID精度日期是合成fixture，不是某次机器路径。它们和固定运行目录/旧可执行文件路径区别明确。

## 视觉 fixture

`bun run dev -- --host 127.0.0.1` 后，打开以下独立 HTML。只作为当前组件的视觉入口；Tauri API由合成fixture替代，不能据此宣称原生持久化/权限验收。启动及关闭dev进程由操作者管理。

| URL | 职责 |
| --- | --- |
| `/scripts/history-organizer-qa/index.html` | 生产 HistoryOrganizer 的合成46行；尺寸、theme/lang见[README](history-organizer-qa/README.md) |
| `/scripts/horoscope-qa/index.html` | 预览整个 SettingsApp，含星运；目录历史名不代表独立星运验收 |
| `/scripts/automation-qa/chat-preview.html` | 合成聊天/自动记忆显示，配合语义案例；不连接用户数据库。`?composer=1` 提供两个 DeepSeek 显示名、短/长最近目录和主题切换；加 `&models=many` 验证搜索/滚动，`&theme=dark` 等设置初始主题。配合浏览器 viewport 检查 420×560、720×760 与 360px 压力宽度；不代替原生目录选择或真实模型调用验收 |

## 发布

| 脚本组 | 当前职责 |
| --- | --- |
| `check-version.ps1`、`release.ps1`、`publish.ps1`、`verify-windows-icon.ps1` | Windows维护者入口、版本/draft/安装器图标检查；先按当前runbook确认所选发布路径 |
| `release-macos.sh`、`package-macos.sh`、`notarize-macos.sh`、`prepare-macos-dependencies.sh`、`verify-macos-dependencies.sh` | 候选包签名/公证/包及固定依赖准备；共享验证链保留 |
| `upload-macos-payloads.sh`、`publish-macos.sh`、`verify-macos-{app,payloads,downloads}.sh`、`merge-updater-manifest.ts` | draft资产/最终清单/下载验收；不能用本索引直接越过runbook门禁 |
| `scripts/macos/` 与 `windows-process-kill.rs` | 签名entitlements/依赖许可、受管进程终止器源码；由上述准备/验证入口消费 |

完整当前文档见[索引](../docs/README.md)，本轮处理与待验项见[清理进度](../docs/repository-cleanup-progress.md)。没有引入按名称/日期自动删除仓库文件的逻辑。

外部部署退役后，共享回归设施迁至 `sidecar-harness/`；专属部署验证器和 mock API 已删除。历史加密快照通过显式构建的维护工具处理，见[模型连接与旧版恢复](../docs/ai-provider-support.md#旧版未完成导入的恢复)。
