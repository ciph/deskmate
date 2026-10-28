# Windows v0.4.22 发布回执

2026-10-28 12:48（Asia/Shanghai）已公开 [YUME v0.4.22](https://github.com/ciph/deskmate/releases/tag/v0.4.22)，非 prerelease，latest 已指向本版本。发布源码为 `1901fb8fd46346acfd5e34661f707f7aafde7eab`，完整包含用户要求推送的 `8702347`、`36a3c2e`、`2e90c94`；额外发布提交仅同步四处版本和新增发布说明。

[发布工作流 40630310481](https://github.com/ciph/deskmate/actions/runs/40630310481) 六个 job 全部成功。未重复执行本机已通过的完整测试或 GUI 验收；同提交自动触发的两个分支工作流已取消，仅保留 tag 发布工作流。Mac CI 候选只留在 Actions，未公开为安装包。

三个公开资产为 `YUME_0.4.22_x64-setup.exe`、对应 `.sig` 和 `latest.json`。已下载核验版本、NSIS 内容/完整性、应用与安装器图标、GitHub SHA-256、Tauri payload 与 trusted-comment 密码学签名及篡改拒绝；公开后匿名完整回读安装包、签名及 latest 清单，字节与发布前一致。

安装包大小 117,637,151 字节，SHA-256：`b904c00c43bcb4077481a31bc1edc5edd1c33aee733c931aa68deeb163d67682`。Tauri 更新签名已验证；Authenticode 为 `NotSigned`。既有 v0.4.20 草稿及资产未改动，旧工作树和原有改动保留。

最终安装包真实安装/跨版本 GUI 自动升级、Mac 实机 GUI/恢复、混合 DPI/高刷、GPU/功耗 A/B、真实模型日志语义及完整故障实验室范围仍未实测，已如实写入 [发布说明](../../release-v0.4.22.md)。原本机验证范围见 [B7 报告](../b7-windows-closeout-20261028/README.md)，不将它冒充为本次新安装包的全部实机验收。

- [发布摘要](release-summary.json)
- [提交保存与发布差异](push-verification.json)
- [CI 门禁](ci-verification.json)
- [安装包校验](package-verification.json)
- [更新签名与清单](updater-verification.json)
- [公开匿名下载回读](public-verification.json)
- [三个资产的 SHA-256](SHA256SUMS.txt)

完整本地安装包与发布过程记录位于 `E:\Codex\工作伙伴\release-artifacts\v0.4.22-windows-release`。此目录中的原始 CLI 日志下载尝试未用于证明测试通过；CI 结论以对应 run/job 的成功状态为准。没有安装到本机日常身份。
