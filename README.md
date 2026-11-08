# YUME

桌面上的 AI 伙伴。陪你聊天、处理任务，也帮你记住重要的事、整理每天的工作。

[下载安装](https://github.com/ciph/deskmate/releases) · [使用与开发文档](docs/README.md)

## 主要功能

- **桌宠陪伴**：内置小著系列角色，支持拖动、互动和角色切换，也可导入 `.dmpack` 角色包。
- **AI 对话与工作台**：连接自己的模型服务，进行对话、发送附件、使用工具和管理历史会话。
- **长期记忆**：保存偏好与重要信息，支持查看、编辑、导出和删除，可分别控制自动记忆与 AI 使用记忆。
- **工作日志**：记录工作进展，整理日报、周报，支持修改、导出和定时生成。
- **个性化与小组件**：自定义聊天头像，查看每日星运，按需启用 Windows 桌面控制。

## 开始使用

1. 从[发布页](https://github.com/ciph/deskmate/releases)下载并安装适合系统的版本。当前 0.5.0 提供 Windows x64 安装包；macOS 本版未更新。
2. 打开 **设置 → AI**，填写模型服务信息，点击 **验证并连接**，再选择模型。
3. 选择喜欢的角色，打开聊天即可开始使用；工具权限可在设置中调整。
4. 在设置中按需配置记忆、工作日志和小组件。有适用的新版本时，设置页会显示 **下载更新**。

Windows 桌面控制首次启用时需下载可选组件，完成后可复用本地缓存。找不到桌宠时，可从系统托盘菜单选择 **找回桌宠**。

## 数据与隐私

记忆和工作日志保存在本机，可在设置中管理。记忆数据库未加密，依赖系统文件权限保护；删除聊天记录不会自动删除独立归档的工作日志。

AI 对话、记忆提取和报告生成会将相关内容发送给你配置的模型服务。新安装默认启用自动记忆与工作归档，可分别关闭；定时报告默认关闭，执行时应用需保持运行。

## 本地开发

项目使用 **Tauri 2、React、TypeScript 和 Bun**。需安装 Bun、Rust 及对应平台的系统构建依赖，工具版本参见 [CI 配置](.github/workflows/check.yml)。

安装项目依赖：

```sh
bun install --frozen-lockfile
```

桌面开发和打包前，先按[构建说明](docs/workbench-build.md)准备完整工作台资源，再启动应用：

```sh
bun run tauri dev
```

常用命令：

| 命令 | 用途 |
| --- | --- |
| `bun run check` | 类型检查、前端测试、Rust 测试及前端构建 |
| `bun run build` | 仅构建前端 |
| `bun run tauri build` | 构建桌面应用；发布签名要求见平台文档 |

macOS 开发环境需先设置 `DEVELOPER_DIR=/Library/Developer/CommandLineTools`。平台打包与发布流程见[文档索引](docs/README.md)。

## 更多文档

- [模型连接与用量](docs/ai-provider-support.md)
- [记忆与工作日志](docs/memory-and-worklog.md)
- [维护与构建脚本](scripts/README.md)
- [当前验收与待办](docs/verification-backlog.md)
