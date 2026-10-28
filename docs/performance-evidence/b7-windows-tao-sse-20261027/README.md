# B7 Windows：Tao 绘制与审批 SSE

Tao 绘制重入已复现并修复，原生回归 25 次通过；审批 SSE 响应头等待与正文空闲期限分离，真实故障注入从旧版 40 秒未就绪恢复到约 4.1 秒重连，旧连接已关闭。物理审批取消/允许一次、缩放/截图、最小化恢复及桌宠重开通过；Rust 974 pass / 24 ignored，正常构建恢复。B7 仍进行中，预计 Windows 还需 3–4 轮，Mac 专有项另计，未提交/推送。

## 修复与复测

- Tao：隐藏 Win32 窗口的嵌套 WM_PAINT 稳定触发原断言。补丁按 flush 返回的所有权决定是否结束绘制，保留外层周期。修复前退出 101，修复后 1/1 及 25 个独立进程通过。源码/许可证/校验值见 [补丁来源](inputs/tao-provenance.md)。
- SSE：旧读取将 45 秒正文预算用于状态行，且 stop/generation 不能打断阻塞。响应头改为独立 3 秒，正文保持 45 秒空闲期限，并检测停止/换代。原解析器及权限边界不变。依赖版本没有升级，只显式引用已有锁定依赖。
- 真实引擎、合成模型、TCP 响应头黑洞：旧二进制 40.02 秒仍未就绪；新二进制首个请求后 4103.68 ms 就绪。旧 TCP 在 2998.87 ms 关闭，就绪时挂起旧连接 0。之后跨心跳、收到 webfetch 并拒绝，所有进程/端口清理。
- Windows Rust 全量 **974 pass / 0 fail / 24 ignored**；审批模块 **23 pass / 0 fail / 1 ignored**。单独运行的真实审批测试通过，不混入默认 ignored 数。前端本轮未改，前次 **1361 pass / 8 skip** 只是复用基线。正常构建通过并在 QA 后恢复。

## 物理 GUI

用户确认暂停其他 GUI 操作后，使用固定 SHA 的临时 Windows MCP 1.3.24，child-only HIGHDPIAWARE，无安装/全局配置变化。隔离身份 com.deskmate.unifiedhistoryqa.winb7taosse20261027a，QA PID 158032。真实鼠标、键盘完成供应商验证、Enter 发送、取消、允许一次；工具回传分别是拒绝原因与本机 model-a JSON。不是通过改 DOM 或伪造 UI 状态验收。CDP 仅用于只读观察及最终虚拟凭据清理。

12 次快速 resize+PrintWindow 和 6 次等待 350 ms 的 resize+PrintWindow 均无 Tao panic；快速截图出现过尚未完成的旧画面/黑边，不当完整视觉通过。等待后的首末截图已人工看图核对；最小化恢复、关闭聊天后真实点击桌宠重开通过。UIA snapshot 本轮超时，使用原生截图完成验收。初次供应商夹具非流式返回格式错误已修正，失败观察保留。

截图：[待拒绝](gui-permission-reject.png) · [拒绝完成](gui-rejected.png) · [待允许](gui-permission-allow.png) · [缩小完成](gui-settled-0.png) · [恢复大小](gui-settled-5.png) · [桌宠重开](gui-reopened.png)。

## 剩余边界

Tao 原事故的具体外部回调、旧 SSE 服务端为何迟迟不返回状态行，现有旧日志不能追溯证明。修复和反例证明的是本地重入/恢复机制。QA 期间可恢复正文断流及重连保留在宿主/sidecar 日志中，强制清理也产生断流；不宣称零网络错误。原生 webfetch 缺失 metadata.timeout 导致 REST 编码拒绝时，已有 SSE fallback 在本次 GUI 中实际工作。

本轮继续通过日志 record/query/查询拒绝区分的传输合同，但使用合成宿主回执，不能关闭完整五工具/到期 UI 验收。B7 仍需拖放/其他媒体二进制、Stop 的 Aborted 文案、完整日志、迁移失败恢复、活跃工具二次启动、PDF 内存、硬件/电源/混合 DPI/生产对照和最终回退检查。**Windows 预计还需 3–4 轮；Mac 专有检查另计，出现新缺陷时调整。**

保留旧改动、四份旧证据；未提交、推送、安装或发布。两个 QA 数据根、虚拟凭据、引擎/供应商/MCP/调试端口已清理；恢复正常构建，未运行生产身份。

## 文件

[结果](summary.json) · [源码指纹](source-hashes.json) · [保留检查](preservation.json) · [旧报告校验](frozen-reports.json) · [Rust 全量](rust-full.log) · [Tao 失败](tao-regression-before.log) · [Tao 通过](tao-regression-after.log) · [SSE 旧故障](live-approval-before-fault.json) · [SSE 新恢复](live-approval-after-fault-lifecycle.json) · [工具结果](gui-provider-observations-final.json) · [清理](cleanup.json) · [正常构建](normal-build-final.json)。
