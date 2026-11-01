# Windows v0.4.24 发布回执

2026-10-29 15:21（Asia/Shanghai）已公开 [YUME v0.4.24](https://github.com/ciph/deskmate/releases/tag/v0.4.24)，非 prerelease，latest 指向本版本。源码 `b9c5606863ebd60d18a4004a31188ca6b6d100dc` 完整包含四个用户指定提交 `f59a4a0`、`df9b713`、`25bd5b1`、`3b4c4d0`；main 和任务分支已推送。

[发布工作流 40830633227](https://github.com/ciph/deskmate/actions/runs/40830633227) 六个 job 全部成功，包含 Windows/macOS 源码门禁、固定工作台重建、草稿、Windows 安装构建和 Mac 未签名候选。Mac 候选仅留在 Actions，未公开为下载。

初次 v0.4.23 门禁发现报告编辑器测试仍同步读取异步 Markdown 预览，两平台均失败，未公开安装资产。`7830555` 将三个断言改为等待真实渲染，定向 7 项通过，产品运行代码未修改。遵守不覆盖已公开 tag 的约定，原 v0.4.23 标签及失败记录保留，最终以 v0.4.24 发布。除这个受影响文件外未重跑本机已通过测试或 GUI；保留新标签发布流程自身门禁，取消四个重复分支 CI。

三个公开资产为 `YUME_0.4.24_x64-setup.exe`、对应 `.sig` 和 `latest.json`。核验版本、NSIS 内容/完整性、应用与安装器图标、GitHub SHA-256、Tauri payload/trusted-comment 签名及篡改拒绝；公开后匿名完整回读三个资产，字节与发布前一致。

公开回读时匿名 GitHub API 返回配额耗尽（403、remaining=0）；元数据及 tag 指向改用既有 gh 授权读取。公开 latest 页面及三个资产仍通过匿名请求核验，不能把授权 API 读取记为匿名 API 通过。具体方式见公开回读 JSON。

安装包 117,713,742 字节，SHA-256：`55c06cee9603840ea2dc8acbfbfb48100e422b7316d8e94e6d9e1d53d1b7188d`。Tauri 更新签名通过；Authenticode 为 `NotSigned`。既有草稿、已发布版本、旧工作树和其他改动未覆盖。安装包和原始记录保存在 `E:\Codex\工作伙伴\release-artifacts\v0.4.24-windows-release`。

最终包真实安装/跨版本 GUI 自动升级、Mac 实机、混合 DPI/高刷、GPU/功耗、真实模型语义、完整故障实验室范围仍未验证。首次正文分页和 R06/D20/M19c 剩余成本不属于本次交付。详见[发布说明](../../release-v0.4.24.md)和[长历史验收](../long-history-closeout-20261029/README.md)；不将候选实机结果冒充最终安装包安装验收。

- [发布摘要](release-summary.json) · [提交保存与差异](push-verification.json)
- [CI 门禁](ci-verification.json) · [初次失败与定向修复](failure-evidence.json)
- [安装包核验](package-verification.json) · [更新签名与清单](updater-verification.json)
- [公开匿名回读](public-verification.json) · [三个资产 SHA-256](SHA256SUMS.txt)
- [原标签与既有草稿保留](preservation-verification.json)
