# B7 Windows 原生 GUI 与发送焦点（2026-10-27）

本轮在 codex/yume-performance-b6 / db07e5b 的现有改动上继续，产品 0.4.21、历史 schema 3、记忆 schema 4 不变。**物理 GUI 核心用例已补；B7 整体仍进行中。** 未提交、推送、安装或发布。

## 结果与方法

用户明确授权临时改用本机已核对 SHA 的 Windows MCP 1.3.24，并留出前台操作时间。没有安装、修改 Codex 或全局配置。原 Computer Use 在空白记事本上仍出现截图超时/鼠标几何不可用，因此不宣称已修复原工具。Windows MCP 的默认 DPI 感知导致 125% 缩放下坐标和截图尺寸不一致；仅给该临时子进程设置 HIGHDPIAWARE 后，主屏 2560×1440、窗口 900×950 与实际一致。其余屏幕/混合缩放未验收。

真实动作使用 Windows MCP SendInput 鼠标与键盘，严格绑定本轮 PID/窗口；主要截图为目标窗口可见区域。CDP 只读采样焦点、节点、Stop 和原生历史，不注入点击、输入或焦点。截图显示透明窗口背后的桌面是实际视觉，不是内容拼贴。因前台被其他任务切走而失败的早期动作不计通过；原始日志保留在 E:/Codex/工作伙伴/reports/yume-b7-native-gui-20261027。

首次物理 Enter 发送暴露 textarea disabled 会令浏览器丢失焦点；之前自动脚本在发送后主动 focus，未覆盖此点。产品只将临时锁定改成 readOnly，保持准备期间不可编辑及同步防重复发送。新增用例在修复前失败，修复后 24 个聊天用例通过；原资源准备测试更新为只读锁定，10 个用例通过。

最终使用第三条独立 1000 消息夹具（ses_winb7focus1000120261027）进行首次物理发送：104 个流式采样 Stop 均可见、焦点丢失 0 次、1000 旧节点全部保持，流式期间草稿采样 76 次。终态草稿为 B7 physical draft 中文；原生保存全文 10000 字符，SHA 为 841fce2bd7eb0ebec5cd55cbadb3840f1db36a590bc978892507b234f67053a1。不是输入延迟 p95 或刷新率测量。

随后物理 Stop 中断 50k 计划流（97/500 分片），服务端 aborted=true；中文草稿保留并续聊收到 B7_LOCAL_REPLY_OK。鼠标点击 Stop 时焦点正常转到按钮，不计发送输入框意外失焦。TXT/PDF/WAV 经原生选择器选入、发送并历史重开；WAV 真实点击后 currentTime=duration=1、ended=true、readyState=4，仅证明播放器链路，未验扬声器声学输出或模型转写。工作台真实按钮接管后小窗只读，关闭工作台恢复输入；真实历史菜单置顶、中文改名、归档和恢复通过。

## 原始失败和仍待验

- 第一次 QA 宿主在 15:15 发生 Tao 0.35.3 flush_paint_messages 断言退出。此前存在跨屏移动/前台切换/PrintWindow 调用，不能确定因果。保留 app-150316761.stderr.log。后续区域截图及一次 PrintWindow 对照未复现，不能据此关闭崩溃问题。
- physical-10k.json 是修复前失败（108 个流式采样全部失焦）；physical-10k-fixed.json 因 QA 配置跳过前端构建仍嵌入旧 dist（110 个采样失焦），也是失败，不是修复通过。完整前端重建后 physical-10k-fixed-final.json 才是有效最终结果。构建日志/回执均保留，后续不得只用源文件 SHA 推定嵌入资源已更新。
- 完整前端第一次 171/172 文件通过，nativeResourceFlow 旧 disabled 断言失败；更新契约后最终 172/172、1361 pass / 0 fail / 8 skip。类型检查、标准前端和正常 Windows debug 构建通过。Doctor changed 82 分、无新增报告；Rust 未修改未重跑，上一轮 970 pass / 24 ignored 仅作为既有基线。
- Stop 取消前端显示“出错了: Aborted”；宿主日志出现 permission stream /global/event 的 os error 10060。二者保留为体验/审批 SSE 待定位，不能将普通聊天成功当审批/故障恢复成功。
- 物理拖放、其他媒体/二进制、PDF 内存收敛、GUI 审批与 SSE 故障恢复、日志五工具/到期、迁移恢复、活跃工具二次启动、硬件/GPU/耗电/混合 DPI/同机生产对照、Mac 专有范围继续待验。下一轮优先隔离复现 Tao 绘制崩溃和审批 SSE，再扩展剩余矩阵。

## 保留与清理

此前 19 个已改文件的哈希/补丁先保存，既有 Rust/历史修复保留。本轮只新增一行产品修复、相关测试和当前交接文档；没有清理旧主仓或 C 工作树。五个隔离 SQLite integrity_check 均为 ok；虚拟凭据由正式设置 API 清除，QA 进程/记事本/临时 MCP/provider 退出，两个精确 QA 数据根删除、51404/61309/9240 端口关闭。正常 dist 与正常配置 debug EXE 已恢复，未运行生产身份。便携包不含临时 bridge bearer token、工具二进制、QA 用户库或无关窗口截图。

数据入口：[summary.json](summary.json)、[物理最终采样](physical-10k-fixed-final.json)、[原生 SHA/附件核对](native-results-check.json)、[紧凑工具审计](native-tool-audit.json)、[最终前端套件](frontend-full-final.json)、[清理](cleanup.json)、[保留检查](preservation.json)。重跑先读 inputs；其中路径/身份为本次固定值，不可直接复用已删除身份而混淆冷启动。
