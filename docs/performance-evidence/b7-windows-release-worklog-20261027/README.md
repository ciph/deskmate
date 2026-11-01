# B7 Windows release 附件与日志：阶段检查点

2026-10-27，`codex/yume-performance-b6` / `db07e5b` / 0.4.21。历史 schema 3、记忆 schema 4。**B7 进行中；本轮物理验收因 Windows 安全桌面中断，未完成完整附件矩阵和日志物理验收。** 没有新增产品源码修改、提交、推送、安装或发布。既有改动保留。

- [HTML](index.html) · [结果](summary.json) · [逐项断言](raw/release-checkpoint.json)
- [真实输入与截图记录](physical-input-log.json) · [单次完整前端回归](raw/frontend-regression.json)
- [本机改动保留](raw/preservation.json) · [旧报告复核](frozen-reports.json) · [SHA 清单](evidence-manifest.json) · [最终审计](final-audit.json)

## 本轮实际完成

使用独立身份 `com.deskmate.unifiedhistoryqa.winb7release20261027a`，优化 release、fat LTO、codegen-units=1、panic=unwind，前端来自现有正常构建。QA EXE SHA-256：`A2CA7F279B6F4C6C76DABE7AFAE33D9BF853BDE4A1EA8C71009EE443D76DB977`。[构建回执](raw/complete-qa-build.json)

| 项目 | 实际结果与边界 |
| --- | --- |
| 22 MB MP4 正文拖放 | 从只提供合成文件的原生 DragSource，以真实鼠标拖入 release 聊天正文；卡片和原生媒体节点确认出现。 |
| 完整播放 | trusted play → ended，30,042 ms，视频到达 30 秒终点。 |
| 快速进度拖动 | 一次连续折线手势，2 个 seeking、1 个 seeked，落在 8.213315 秒；不写成 6 次独立完成的 seek，也不等同于取得原生 Range 取消回执。 |
| 继续播放、播放中移除 | 在 8.224069 秒恢复播放，移除时于 14.973985 秒触发 trusted pause；媒体节点归零，聊天仍响应。 |
| release 二进制 | 1/8/20 MiB 普通附件原始 ArrayBuffer 上传、SHA 回读一致；空内容、尺寸不符、伪装图片、JSON 数组、单项/总量超限均拒绝。 |
| 64 MiB WAV | 原生上传后文件 SHA 一致；WebView2 程序化解码、推进、seek 到末段并结束，owned 副本随后移除；超 64 MiB、扩展名/MIME 不符和非 chat 调用均拒绝。不是物理音频 UI 通过。 |
| 自动回归 | 本轮一次完整串行运行：172/172 文件，1367 pass / 0 fail / 8 skip / 0 unfinished。首次启动因沙箱 EPERM 失败，原日志保留；允许正常子进程后全套通过。不是上轮局部复跑的组合值。 |

核心证据：[播放/移除事件](raw/probe-removed-playing.json)、[视频拖入](raw/release-video-drop-body.png)、[视频播放](raw/release-video-playing.png)、[进度手势后](raw/release-video-rapid-seek.png)、[移除后](raw/release-removed-while-playing.png)、[二进制结果](raw/binary-results.json)、[64 MiB 解码](raw/media-64MiB-playback.json)、[资源回读](raw/resource-readback.json)。

本轮没有 Rust/React 产品源码改动，未重跑 Rust 全套和 React Doctor；上轮 Rust 974 pass / 24 ignored、迁移定向 32 pass、Doctor 无新增报告保留为来源结果，不计为本轮执行。正常前端构建中的 TypeScript 检查通过，不宣称本轮重新执行了三套完整类型检查。

## 中断与非通过项

19:34:08（北京时间），下一步打开附件选择器时，鼠标工具因无前台窗口拒绝输入，随后的只读窗口检查返回 `SecureDesktopActive`（锁屏或 UAC 安全桌面）。19:37:36 再次只读确认仍相同。没有自动解锁、处理 UAC 或向安全桌面输入。用户已授权本轮物理验收，但普通桌面恢复需要本机操作；此前授权不改变此系统限制。

这次 **release 原生选择器尚未打开**；MP3/WAV/文件夹/坏视频的 release 正文与输入区矩阵、视频切会话再添加、发送/历史恢复均未执行。上一轮 debug 中通过的同类项目仍只算上一轮 debug 证据。

本轮工作日志 entries/reports/runs/schedules 全为空，完整物理日志操作没有开始。编辑/冲突、日报周报与来源、人工版本/候选采用、复制和 MD/TXT 导出及同名不覆盖、定时计划编辑/暂停恢复、失败重试、运行中暂停拒绝迟到结果、关联删除确认/取消、重启保持、浅深色和固定窗口滚动，全部保留待验。[空数据回读](raw/worklog-before-secure-desktop-cleanup.json)

窗口准备时发现设置页被置顶聊天遮挡；最初两次点击虽返回成功，但未产生预期页面，未计通过，移开设置窗口后重新操作。Windows MCP 的 `move` 返回坐标与目标不符，该动作不作为鼠标几何通过证据；随后 click/drag/polyline 用实际落点、媒体事件和截图核对。未修改工具或全局配置。

本轮未发现新的 Tao panic。清理 provider 后日志含 `permission_generation_changed` 和一次 `permission_stream_body` 解码错误，原件保留，不能宣称 SSE 全程零警告；本轮未做新的审批/SSE 故障复测。[原生日志](raw/app-192704663.stderr.log)

## 收尾和接续

六个隔离 SQLite 完整性为 ok，移除后 resources/resource_messages 均为 0。虚拟 provider 经实际设置接口清除；所有记录过 PID/出生时间/路径的 QA、sidecar、拖放源和临时 MCP/provider 已停止，两个专属数据根和三个监听端口已清理。正常默认 feature 的 release 构建恢复，未运行生产身份。[完整性](raw/final-integrity.json)、[清理](raw/cleanup.json)、[正常构建](raw/normal-release-build.json)

保留可续测 QA EXE `output/b7-release-worklog/yume.release-qa.exe`（与上述 SHA 一致）、合成媒体和本轮脚本。它是缓存，不是安装包或可发布候选；不能直接双击输出目录中的副本，因为资源仍位于 target/release。接续应先核对源指纹、默认 release 和 QA SHA，重新登记新临时进程与凭据；禁止使用本报告中过期 HWND/PID/端口。所有 inputs 为当次复核材料，不是未经检查即可重放的自动化入口。

优先完成本节未执行的 release 附件和完整日志物理表面，通过后再推进活跃工具二次启动、Aborted 文案、余下恢复、PDF 堆/RSS、硬件/生产对照，以及历史回执恢复新增读取的性能复核和最终门禁。真实模型意图与 Mac 专有项仍单独待验。

**剩余预估：Windows 2–3 轮集中工作；出现新阻断则 3–4 轮。此次被安全桌面中断的物理项目仍在这个估计内，没有按“本轮完成”扣减。**

原始材料：`E:\Codex\工作伙伴\reports\yume-b7-release-worklog-20261027`。便携证据不含临时 bridge token、provider 凭据、数据库或大媒体。未尝试浏览器预览：此前 file 协议已被浏览器策略拒绝，没有改用其他协议绕过；HTML 本地链接与哈希经过检查。
