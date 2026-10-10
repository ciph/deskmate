# AI provider connections and usage

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
