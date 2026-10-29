# YUME 文档入口

最近整理：2026-10-29，YUME 0.4.24 / OpenCode 1.18.21。B6/B7 本机整改与长历史 B0–B4 已随 Windows 发布；[当前状态](performance-windows-handoff.md)、[发布回执](performance-evidence/release-v0.4.24-windows/README.md) 和 [未验范围](verification-backlog.md) 为现行入口。长历史验收原件保留候选时点的版本及状态，公开发布以本次回执为准。历史记录不再用于安排新的验收轮次。

## 当前产品与维护契约

| 入口 | 职责 |
| --- | --- |
| [项目 README](../README.md) | 安装、功能、开发命令与用户可见行为 |
| [仓库 AI 入口](../AGENTS.md) | 本仓维护和发布约定 |
| [设计规范](../DESIGN.md) | 主题、组件与交互原则 |
| [模型连接与用量](ai-provider-support.md) | 模型发现、实际调用验证、用量来源及外部部署退役/旧快照恢复 |
| [记忆与工作日志](memory-and-worklog.md) | 自动处理、开关、数据归属、删除及恢复 |
| [本地附件](local-resource-attachments.md) | 原生选择、图片/文件/媒体预览与资源权限 |
| [附件原生回归](attachment-native-regression.md) | 拖拽、WebKit 播放与取消请求的实机 gate |
| [统一历史契约](migrations/opencode-native/unified-history-contract.md) | 会话身份、模型偏好、空白记录、组织操作与共享入口 |
| [工作台构建](workbench-build.md) | 当前固定输入、生成物、完整重建、轻量 Windows 包及按需组件 |
| [运行生命周期](runtime-lifecycle.md) | 工作台首次创建、3D 按需加载、隐藏停机、帧率/包络及受管引擎恢复 |
| [维护与验收脚本](../scripts/README.md) | 构建/作者/QA/发布 CLI 的职责、输入输出、平台和退出条件 |
| [原生能力矩阵](migrations/opencode-native/capabilities.md) | 固定引擎能力与带日期的运行证据；未验证项明确保留 |
| [架构决策](migrations/opencode-native/decisions.md) | 有效决定、被替代的理由和重审条件 |
| [Mac 本机发布指南](local-release-runbook.md) | 对应 Mac 的提交、签名公证与发布流程；Windows 按仓库 release workflow 与当前发布回执执行 |
| [macOS release gate](macos-release.md) | 分发信任、包验证和 Apple/updater 签名边界 |
| [macOS runtime checks](macos-runtime-checks.md) | 更新按钮当前规则、真实 App/硬件和 A→B 升级验收 |

## 活动工作与验收

| 入口 | 当前责任 |
| --- | --- |
| [长历史交互与回执读取施工方案](long-history-construction-plan.md) / [施工交付记录](performance-evidence/long-history-closeout-20261029/README.md) | B0–B4 已完成，G1–G5 通过；四个提交已推送，随 Windows 0.4.24 发布；测量与原始边界保留于交付记录，公开状态见[发布回执](performance-evidence/release-v0.4.24-windows/README.md) |
| [性能整改施工计划](performance-remediation-plan.md) / [Windows 接管入口](performance-windows-handoff.md) / [Mac 施工记录](performance-remediation-handoff.md) / [便携证据](performance-evidence/README.md) | 原 20 项审计的设计、已交付修复与证据；B6/B7 已随 Windows 0.4.22 发布，严格平台/硬件未验范围保留；旧施工顺序不自动重启 |
| [仓库审计](../AUDIT.md) | 2026-09-30 基线 findings；历史基线不等于当前缺陷均仍存在 |
| [审计整改方案](audit-remediation-construction-plan.md) / [施工进度](audit-remediation-progress.md) | 运行、记忆库、UI、工作台和历史简化的实施依赖与真实结果 |
| [文件清理方案](repository-cleanup-plan.md) / [执行进度](repository-cleanup-progress.md) | 文件全集、用途判定、实际处理与条件项 |
| [当前验收与待办](verification-backlog.md) | 现行验证命令及从旧计划提取的未完成事项 |
| [自动处理验收记录](automatic-memory-verification.md) | 已实现行为、合成联调和未完成真实模型语义验收 |

## 历史与发行说明

[历史记录索引](archive/README.md) 保留旧设计、基线、阶段回执与独有证据。旧记录中的“当前 / 下一步 / 未发布”和凭据阻塞不作为今天的状态或操作指令；未完成事项已列入当前验收入口。

发行说明按原版本保留：[0.4.10](release-v0.4.10.md)、[0.4.11](release-v0.4.11.md)、[0.4.12](release-v0.4.12.md)、[0.4.13](release-v0.4.13.md)、[0.4.14](release-v0.4.14.md)、[0.4.15](release-v0.4.15.md)、[0.4.16](release-v0.4.16.md)、[0.4.17](release-v0.4.17.md)、[0.4.18](release-v0.4.18.md)、[0.4.19](release-v0.4.19.md)、[0.4.20](release-v0.4.20.md)、[0.4.21](release-v0.4.21.md)、[0.4.22](release-v0.4.22.md)、[0.4.23（未发布）](release-v0.4.23.md)、[0.4.24](release-v0.4.24.md)。其中的测试数和待发布说明对应该次准备时点；公开状态与安装资产须按发布指南重新核验。
