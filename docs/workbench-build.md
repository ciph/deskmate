# 原生工作台：当前构建入口

核对日期：2026-09-30。当前 YUME 使用一个受管 OpenCode 引擎，工作台是同一服务的固定上游 UI 加本仓 Platform bridge，不另建执行引擎。

工作台 WebView 在首次显示或会话交接时创建；宿主启动不再预建隐藏工作台。首次创建、页面就绪后的会话交接、隐藏保留及受管引擎恢复规则见[运行生命周期](runtime-lifecycle.md)。这不改变完整工作台随包交付和固定输入重建的要求。

## 固定输入

| 输入 | 当前约束 |
| --- | --- |
| OpenCode 引擎 | `package.json` / `bun.lock` 固定 `opencode-ai` 1.18.21；由 `prepare:sidecar` 准备当前平台二进制 |
| 原生 UI 源码 | `anomalyco/opencode` commit `826d9ad46a22bef0294998e08daa3c4904fea28f`（v1.18.21） |
| 上游依赖 | 保留该提交的 monorepo 和根 `bun.lock`，使用 Bun 1.3.14、`bun install --frozen-lockfile` |
| 上游 lockfile 指纹 | 2026-10-11 从上述官方固定提交重新检出的原始 `bun.lock` SHA-256 为 `55cde1d8d6921ddd764ad99e6be8755767ca81e10f439e047014b7aaacffae5c`；须与该提交的 Git blob 一致，安装后保持未修改。旧历史记录的指纹不作为当前构建输入 |
| 本仓适配 | `src/workbench/`、prepare 脚本管理的 overlay、主题桥和 terminal WASM |

上游静态核验和 Windows 阶段指纹保存在[历史基线](archive/migrations/opencode-native/baseline.md)，旧机器路径和候选包哈希不是本机当前构建输入。固定版本决策见[架构决策](migrations/opencode-native/decisions.md)。

## 准备与构建

准备上述固定提交的源码 clone，安装其锁定依赖，然后在 YUME 根目录执行：

```sh
export DEVELOPER_DIR=/Library/Developer/CommandLineTools
YUME_OPENCODE_SRC=/absolute/path/to/opencode-v1.18.21 bun run prepare:workbench
bun run prepare:sidecar
bun run build
```

PowerShell 对应先设置 `$env:YUME_OPENCODE_SRC`。macOS 的 `DEVELOPER_DIR` 只对当前命令生效，不修改全局 Xcode 选择或许可。准备脚本会放入 overlay 并生成完整工作台，主题资源随当前本仓源码生成；terminal 的 `ghostty-vt.wasm` 需要落在工作台 bundle 根目录。

`public/workbench/` 是忽略的生成物，前端 build 将其作为静态输入。普通 `bun run build` 成功只证明自有前端通过；桌面 dev hook 不自动证明完整工作台刚重建。打包入口与 Source checks 的完整工作台 job 应从固定源码准备，不能把旧 bundle 当成验证证据。

## 轻量 Windows 安装包（2026-10-09）

- `prepare:workbench` 生成的构建配置关闭 sourcemap；`prepare:opencode` 不交付 taskkill PDB。`beforeBundleCommand` 检查 `dist/` 与 OpenCode 资源，发现 `.map` / `.pdb` 时阻止打包。
- Windows MCP 不再由 `prepare:sidecar` 或 NSIS 自动包含。`components/windows-mcp.json` 是维护脚本与运行时共用的固定版本、URL、长度及双 SHA-256 清单；`prepare:windows-mcp` 保留为维护者手动校验入口。
- 首次启用从固定 GitHub Release 下载 1.3.24，校验 ZIP 后仅解出指定根目录 exe，再校验 exe，通过后原子安装至应用数据目录 `components/windows-mcp/1.3.24/`。断网、校验失败、磁盘写入失败可重试；取消与失败清理本次临时目录；进程强制退出可能留下 `.download-*` 临时目录。已安装组件可离线复用，每次生成 MCP 配置仍校验文件。
- 老版本的启用偏好保留，缺少组件时设置页提供补装。下载完成后启用或关闭组件均等待宿主任务、后台报告/记忆请求、已知原生会话空闲，并要求工作台关闭，然后重启服务；新任务与重启通过同一读写锁边界互斥。等待失败显示重试入口，不强行中断任务。
- 初始安装包仍包含 OpenCode、完整工作台、宠物和基础功能。此次没有引入在线安装器或工作台资源远程加载。下载依赖 GitHub 可达；未下载组件的首次离线启用不可用。

## 当前限制与退出条件

当前 prepare 脚本仍存在隐式 sibling clone、输入指纹验证不足、overlay 重复准备和自有入口文本 transform 等原审计问题。上述“固定输入”是必须核对的契约，不宣称现行脚本已自动验证所有指纹。W1/W2 仍按[审计整改方案](audit-remediation-construction-plan.md)实施；错误 ref/lock/version、相同输入重复准备、overlay 改动后重建以及失败不混用新旧输出均需验收。

受管连接用宿主鉴权、scoped HTTP/SSE 和窗口标签门控。工作台输入所有权按完整会话身份和租约管理；关闭/路由切换释放，取消需确认原生状态。正文、part 和工具事实继续由 OpenCode 拥有，宿主不写其数据库。具体能力及运行证据见[能力矩阵](migrations/opencode-native/capabilities.md)，数据回退约束见[统一历史契约](migrations/opencode-native/unified-history-contract.md)。
