# Windows v0.5.0 发布回执

2026-11-01 22:09（Asia/Shanghai）已公开 [YUME v0.5.0](https://github.com/ciph/deskmate/releases/tag/v0.5.0)，非 prerelease，latest 指向本版本。源码 `be66f65dcdb7b91ee6edcba93f30d8978ea83ec4`，相对 v0.4.24 新增内置桌宠“百变小著”（`fba2c3d`、`bd71fcc`、`115e844`、合并 `65b4e29`）及相关文档；main 和 `codex/yume-performance-remediation` 均已推送。

[发布工作流 41311399840](https://github.com/ciph/deskmate/actions/runs/41311399840) 六个 job 全部成功：Windows/macOS 源码门禁、固定工作台重建、草稿、Windows 安装构建、Mac 未签名候选。Mac 候选仅留在 Actions，未公开为下载。

三个公开资产为 `YUME_0.5.0_x64-setup.exe`、对应 `.sig` 和 `latest.json`。公开前沿用 0.4.24 的核验脚本（版本改为 0.5.0）：GitHub SHA-256 与下载字节一致、NSIS 完整性、安装包内容、应用与安装器图标、内嵌程序产品版本 0.5.0、Tauri payload/trusted-comment 签名及篡改拒绝、更新清单两个 Windows 目标的仓库/版本/URL。内容检查本次新增要求四个小著角色及百变小著技能资源都在安装包内。公开后匿名请求：latest 页面跳转到 v0.5.0，匿名 API latest 为 v0.5.0，三个资产匿名下载字节与草稿一致。

安装包 117,779,077 字节，SHA-256：`bdf71e3c21d5de62b61e23a9e61e08a4059a8483070908c957d0b00c6d88697b`。Tauri 更新签名通过；Authenticode 为 `NotSigned`。安装包、解出的程序和核验脚本保存在 `E:\Codex\工作伙伴\release-artifacts\v0.5.0-windows-release`。

本机验证复用发布前已完成的合并树结果（类型检查、前端 182/182 文件、Rust 978、生产构建）；版本号提交后仅重跑版本校验、类型检查和 93 项相关测试，发布工作流仍执行自身门禁。百变小著的原生交互证据来自独立 debug 预览，不是本安装包安装后的实测。

**macOS 未随本版发布。** Mac 上传脚本与 finalize 工作流只接受未公开草稿，v0.5.0 已公开，因此 Mac 包无法再加入本版本，需在维护者 Mac 上随下一个版本（例如 v0.5.1）走完整的签名、公证与 updater 合并流程。在此之前 macOS 用户的检查更新不会提供 0.5.0。

最终包真实安装与跨版本 GUI 自动升级、双击打开聊天偶发无响应的原因、150%/175% 缩放、高刷、混合 DPI、macOS 实机仍未验证。详见[发布说明](../../release-v0.5.0.md)和[百变小著](../../xiaozhu-morph.md)。

- [发布摘要](release-summary.json) · [CI 门禁](ci-verification.json)
- [安装包核验](package-verification.json) · [更新签名与清单](updater-verification.json)
- [公开匿名回读](public-verification.json) · [三个资产 SHA-256](SHA256SUMS.txt)
