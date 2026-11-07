# AI provider connections and usage

## 桌宠右键用量（2026-11-02）

2026-11-05 当前交互：点击右键扇环中的“AI 用量”后，圆环收起，在桌宠头顶显示云朵浮窗，不打开设置。云朵显示当前连接的剩余额度百分比和剩余金额；单击浮窗任意位置或按 Esc 关闭，失焦不关闭。复用透明菜单窗口、真实身体锚点和现有 `fetch_ai_usage`，窗口限制在屏幕工作区内。宠物移动、隐藏、切换角色或连接设置变化时关闭浮窗，避免残留或显示旧连接结果。

仅选择入口时查询一次，重新打开重新读取；打开右键菜单本身不查询，没有后台轮询或持久缓存。请求完成前也能关闭，关闭后丢弃迟到响应。传给浮窗的只有已格式化的展示数据，没有 API Key、Base URL 或供应商对象。库洛沿用 10000 单位换算为元；OpenRouter 按当前 Key 上限计算剩余比例；DeepSeek 仅显示原币种账户余额。没有上限或没有账户接口时，缺失数据用“—”及状态提示表达，不推测百分比或金额。未配置、权限失败、网络错误均留在可关闭的云朵内。

同日体验反馈后的动画调整：出现和单击/Esc 关闭均使用与百变小著消失一致的 300 毫秒线性透明度动画。原生窗口就绪后才开始淡入，淡出完成后才隐藏；淡入中关闭会从当前透明度继续淡出。系统减少动态效果时不等待动画，组件卸载会取消延迟关闭，避免误关下一次浮窗。

2026-11-03 历史交互：分层扇环替换原生列表，AI 用量当时只打开 AI 设置，移除了旧摘要缓存和原生菜单桥。以下列表摘要实现和验收保留为 2026-11-02 的历史记录，当前圆环见[百变小著菜单契约](xiaozhu-morph.md#分层扇形菜单2026-11-03)，云朵验收边界见[当前待办](verification-backlog.md#桌宠右键-ai-用量)。

本轮基于实测为 0.5.0 / `fb2ad51` 的 `build-v0418-9081/deskmate-public-release` 工作树施工；另一 Windows checkout 仍为 0.4.24，不迁移其源码或改动。

本轮按以下顺序完成源码：
1. 复用 `fetch_ai_usage`，增加当前供应商的两行摘要、60 秒内存缓存与并发请求合并。账户/平台金额和 YUME 本地 tokens 分别标注，跨日或连接配置变更时重新查询。
2. 在所有角色的原生右键菜单顶部加入摘要和 AI 设置详情入口。先弹菜单，再异步更新；未配置、权限失败、网络失败各有状态。关闭后停止写入菜单资源，切换供应商时忽略旧响应。
3. 验证摘要数值、缓存/迟到响应、菜单资源与角色操作回归，执行类型检查、生产构建和 React Doctor；单独检查 Windows 原生菜单更新与关闭。

底层注意：当前 Tauri 2.11.5 `plugin:menu|popup` 在模态菜单期间持有 WebView 资源表锁。本功能使用仅接受 pet WebView 资源的 `popup_pet_menu`，先取得菜单的 Arc 并释放锁，再显示菜单，允许异步文字更新。后端查询来源、凭据保存和账本均复用既有实现。

展示规则：库洛为本周剩余额度/百分比及平台今日金额/请求数；DeepSeek 为账户余额及 YUME 今日 tokens/请求数；OpenRouter 为当前 Key 剩余额度（无上限单独注明）及平台今日费用；其他来源显示 YUME 今日记录及本地统计提示。未配置时“配置 AI…”进入现有 AI 设置，其他状态由“查看详情…”进入。金额沿用后端原币种与换算，本地统计不表示平台账单。菜单不常驻轮询，不新增依赖、账本或凭据存储。

验证：64 项相关前端测试、Rust 菜单入口边界测试、类型检查及 Windows 原生 debug 构建通过；React Doctor（相对 HEAD，含未跟踪源码）93/100，未新增诊断。隔离身份 `com.deskmate.unifiedhistoryqa.petaiusage20261102` 下，通过真实菜单 IPC 验证“打开 → 延迟更新 → 文字回读 → Esc 结束 → 资源关闭”；该探针使用合成文本，并非真实账户调用或完整右键操作。详情见[验收回执](performance-evidence/pet-ai-usage-20261102/receipt.json)。

2026-11-03 已提交并推送源码 `6b8ee44`，对应 Windows/macOS 源码检查及固定工作台重建 CI 全部通过。已从该提交构建 0.5.0 Windows 本地安装包，完成原位更新并重新打开 YUME；安装后的主程序与包内文件 SHA-256 一致，21 项资源校验通过，无障碍接口读到百变小著桌宠。旧安装目录已备份；没有升版本、签名或公开发布。见[本地安装回执](performance-evidence/pet-ai-usage-20261102/local-install-20261103.json)。

安装后再次尝试原生右键验收：Windows 工具截图超时，鼠标定位仍返回 `coordinate input geometry is unavailable`，键盘替代也未取得菜单结果。因此尚未确认实际菜单显示、真实账户摘要刷新与详情跳转；这些项目及 macOS 原生验收见[当前待办](verification-backlog.md#桌宠右键-ai-用量)。

## Provider support

Yume discovers models from the configured provider's model-list endpoint. A successful connection lists only returned chat-capable models when the endpoint provides capability metadata. A model-list response alone does not prove that every returned ID is callable with the current key. To require an actual call per model, or when the provider has no usable model-list endpoint, enter up to 12 comma-separated model IDs in the optional field. Verification then sends a short paid inference request to each ID and adds the set only if all calls succeed. This manual path currently requires an OpenAI-compatible chat endpoint. A model can still fail later because of changed permissions, quotas, or provider-specific features.

| Provider / model family | Example Base URL | Request adapter | Account usage in Yume |
| --- | --- | --- | --- |
| OpenAI GPT / o-series | `https://api.openai.com/v1` | Native OpenAI | Yume local tokens and requests |
| Anthropic Claude | `https://api.anthropic.com/v1` | Native Anthropic | Yume local tokens and requests |
| Google Gemini | `https://generativelanguage.googleapis.com/v1beta` | Native Google | Yume local tokens and requests |
| Cohere Command | `https://api.cohere.com/v2` | Native Cohere; models discovered at `/v1/models` | Yume local tokens and requests |
| DeepSeek | `https://api.deepseek.com` | OpenAI compatible | Official balance plus Yume local tokens and requests |
| Moonshot Kimi | `https://api.moonshot.ai/v1` | OpenAI compatible | Yume local tokens and requests |
| Zhipu GLM | `https://open.bigmodel.cn/api/paas/v4` | OpenAI compatible | Yume local tokens and requests |
| Alibaba Qwen | `https://dashscope.aliyuncs.com/compatible-mode/v1` | OpenAI compatible | Yume local tokens and requests |
| ByteDance Doubao | `https://ark.cn-beijing.volces.com/api/v3` | OpenAI compatible | Yume local tokens and requests |
| xAI Grok | `https://api.x.ai/v1` | OpenAI compatible | Yume local tokens and requests |
| Mistral | `https://api.mistral.ai/v1` | OpenAI compatible | Yume local tokens and requests |
| OpenRouter (many model families) | `https://openrouter.ai/api/v1` | OpenAI compatible | Official current-key limit and daily spend plus Yume local tokens and requests |
| MiniMax, Meta Llama, Perplexity Sonar, Amazon Nova and other hosted families | An OpenAI-compatible gateway that exposes them | OpenAI compatible | Yume local tokens and requests, or the gateway-specific account card above |
| Other OpenAI-compatible gateways | Their documented API prefix | OpenAI compatible | Yume local tokens and requests |

Kuro gateway keeps its weekly balance card. The DeepSeek balance and OpenRouter key metrics are fetched from their official APIs. If an account query fails, the same card shows local Yume usage without presenting it as the provider's billed total. Local records cover completed Yume assistant messages only; earlier conversations and traffic from other clients are not included. A regular inference key is insufficient for some platforms' organization-wide billing APIs; those would require separate admin credentials and explicit integrations.

The current Yume chat and agent flow uses text-generation adapters. Model-list endpoints may also include image, audio, video, embedding, or special Responses-only models that this flow cannot run. Capability metadata is used when available, but providers without that metadata still require an inference check for each model. The app does not claim unconditional support for every model ID or every modality.


## YUME 内置连接与外部部署退役（2026-10-10）

0.4.18 开发工作树已移除设置页两个部署按钮、CC Switch 卡片、聊天导入卡片、对应工具与 IPC。当前流程是填写供应商 → 验证并连接 → 选择模型。验证会保存凭据和模型目录并重启受管引擎；模型选择继续同步到工作台。API 调用规则和手动模型验证费用边界不变。内置 OpenCode、完整工作台和 Windows MCP 按需组件仍然保留。

升级时，在引擎启动前将 YUME 应用数据目录 `workspace/.opencode/tools/ccswitch_prepare_opencode_provider.ts` 移至同一应用数据目录的 `retired-tools/<uuid>.ts.disabled`，保留原文件字节及用户修改，不再参与工具发现。操作拒绝链接/reparse point、非普通文件及错误目录；失败时阻止本次引擎启动，不带着残留工具继续运行。重复启动无需再次处理。其他工具、会话、模型目录与凭据不清理。

外部客户端、用户 PATH、用户目录下 `.config/opencode` / `.local/share/opencode` 与 CC Switch 数据库不自动卸载或修改。若需要清理以前安装的外部副本，应先确认没有其他终端或应用使用它，再单独处理。

### 旧版未完成导入的恢复

应用升级保留 `ccswitch-recovery/*.json` 与系统凭据存储中的恢复密钥。独立维护工具使用原有加密格式、路径校验和并发修改检测，默认应用不编译此模块，也不注册恢复或部署 IPC。恢复工具只能恢复已存在的快照，不安装客户端、不创建新导入。

在仓库根目录构建（不要启用 `worklog-qa`，它使用隔离的测试密钥命名空间）：

```powershell
cargo build --locked --manifest-path src-tauri/Cargo.toml --features legacy-ai-recovery --bin recover-external-ai
```

关闭外部客户端，以创建快照的同一操作系统用户运行下面的命令。`APP_DATA_ABSOLUTE_PATH` 是创建快照的 YUME 应用数据目录（其下应存在 `ccswitch-recovery`），`USER_HOME_ABSOLUTE_PATH` 是当时用户主目录；UUID 取快照文件名，不读取或粘贴文件内容：

```text
recover-external-ai --app-data APP_DATA_ABSOLUTE_PATH --home USER_HOME_ABSOLUTE_PATH --snapshot UUID --restore
```

`--restore` 明确授权覆盖该快照对应的旧外部配置；恢复前会比较当前配置和认证文件，发生再次修改时拒绝覆盖，不提供强制绕过。密钥不存在、快照损坏或冲突时保留快照并报告错误。工具不输出凭据，也不删除 CC Switch 数据库或外部认证文件。恢复模块由 `cargo test --locked --manifest-path src-tauri/Cargo.toml --features legacy-ai-recovery --lib legacy_ai_recovery` 验证。
