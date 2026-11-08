# 当前验收与待办

核对日期：2026-10-29，YUME 0.4.24。B6/B7 本机整改与长历史 B0–B4 已交付并随 Windows 发布；[当前状态](performance-windows-handoff.md)、[发布回执](performance-evidence/release-v0.4.24-windows/README.md) 和 [逐项审计](performance-evidence/b7-windows-closeout-20261028/audit-matrix.json) 集中维护完成结论。长历史候选验收使用独立记录，公开资产核验见本次回执。下表是未验范围，不是自动开始下一轮施工的指令。

## 当前验证入口

按改动范围运行 `bun run typecheck`、`bun run test:frontend`、`bun run test:rust`、`bun run build` 或组合入口 `bun run check`。已通过且源码未变的不重复整套；原生 GUI、真实模型和硬件结果单独记录，发布工作流按目标版本执行。

## 模型选择主线程阻塞修复（2026-11-08）

`history_model_selection_set` 改为异步 IPC 命令，任务锁等待、目录读写和模型校验进入阻塞工作线程；拿锁后仍重读活动会话与目录状态，保持与任务启动互斥。新增 3 项真实生成 IPC handler 的隔离回归：无关任务持锁时派发线程可继续、等待期间目标任务启动后拒绝写入、非聊天窗口及未验证模型被拒绝。旧同步实现先跑出 2 failed / 1 passed，修复后 3 passed。

本机 `cargo test --locked --manifest-path src-tauri/Cargo.toml --lib` 复跑通过：993 passed / 0 failed / 25 ignored。首轮 992 passed / 1 failed / 25 ignored，既有事件连接用例 `generation_change_interrupts_silent_connection_and_worker_reconnects` 遇到 Windows socket `WouldBlock`；单项重跑及完整复跑均通过，未修改该用例或跳过检查。原始日志保存在开发工作树 `output/model-selection-dispatch-20261108/`。此记录覆盖源码和 mock IPC，不代表重新安装或原生 GUI 验收；合并以修复后提交的 Windows/macOS Source checks 结果为准。

同轮 CI 的 Windows 两次在准备 ncmdump 时遇到发布下载 URL 的 HTTP 504，尚未执行源码测试；macOS 与固定工作台构建通过。构建脚本补充同一固定发布资产的 GitHub 官方 asset API 备用路径，每条请求限时 30 秒，双重失败继续终止，原有可执行文件 SHA-256 校验不变。3 项下载回归及类型检查通过；本机真实复现主地址失败后从备用路径下载 406438 字节，解压后二进制哈希匹配原 pin。最终候选重新执行两平台完整 CI，不能沿用前一提交的绿灯。

## 任务残留与模型选择（2026-11-06）

用户确认保留圆角菜单后，本轮修复活动任务 Windows 路径误识别、其他任务全局误锁模型、已提交但无助手回复的空闲任务未结算。类型检查、189/189 个前端测试文件、Rust 990 passed / 0 failed / 25 ignored、生产前端构建通过。新增前端用例覆盖返回/停止、跨目录身份、无关任务下切模型及选择/发送竞态；新增 Rust 用例覆盖恢复后重启、busy/retry/审批/提问/网络未知/路径边界、迟到完成及世代/取消竞态。React Doctor changed 范围无新增诊断（本轮 14 文件得分 82，前轮 9 文件得分 83，不把空诊断报告当作全仓无债务）。圆角轮廓源码与已安装版哈希一致。

本机候选、构建及安装回执位于 `E:/Codex/工作伙伴/release-artifacts/agent-model-recovery-20261106`。112.89 MiB 安装包完成校验并原位安装，退出码 0；安装后 EXE 与 21 个资源匹配，旧版 26 个文件及原始任务记录已备份。18:40 启动后，那条 11 月 5 日的真实残留任务由新逻辑自动结算为 `interrupted / agent_execution_lost`，目录运行态恢复 `idle`；只读数据库对比确认原始会话、用户消息和文本片段保留。原生无障碍接口确认百变小著页面加载。截图捕获仍超时，坐标输入报 geometry unavailable，未完成安装版模型菜单点击验收，交由用户试用；自动化组件测试已覆盖选择与返回。用户此前已确认圆角效果可保留，本轮轮廓源码未变。未公开发布，macOS 和物理多屏 DPI 未验。

2026-11-06 用户在已安装版实际试用后确认：“现在可以正常选择模型了，圆角效果也没变，就保留这个版本。”因此模型选择和圆角保留两项本机验收通过，`agent-model-recovery-r01` 作为当前用户确认保留的版本。此确认不扩展为其他平台、混合 DPI 或真实模型调用验收。

2026-11-07 按用户要求将该版本保存为本地 Git 基线，标签 `baseline/yume-0.5.0-accepted-20261107`。基线包含 AI 用量云朵及淡入淡出、圆角菜单、任务恢复和模型选择修复；应用版本仍为 0.5.0。源码与已验收包的 29 文件快照核对一致，仅验收文档随后补充；已安装 EXE 的 SHA-256 为 `FA05DA557A83B28D3BC4660D894DC1F739130EF4F821D999631877FFF0F1B13F`。从该标签创建 `codex/` 分支可恢复对应源码，原安装包及回退备份继续保存在上述回执目录。本次为本地提交和标签，不含远程推送或公开发布。

## 分层扇形菜单（2026-11-03）

2026-11-06 圆角调整：共用扇区轮廓改为内外四角 8 CSS px 圆角，保留圆环布局和间距。8 项几何/菜单测试、类型检查、生产前端构建及真实 WebView2 菜单首开/复开与云朵关闭测试通过，React Doctor 83/100 无新增诊断。实际组件浏览器验证居中、底边、深色角落、子菜单/返回/关闭；28 个裁掉的角不再命中按钮，7 个标签仍正确命中。浏览器证据：`E:/Codex/工作伙伴/design-artifacts/xiaozhu-rounded-menu-20261106`；本机包和安装回执单独记录在 `E:/Codex/工作伙伴/release-artifacts/xiaozhu-rounded-menu-20261106`。真实桌面圆角观感、物理鼠标、混合 DPI 与 macOS 待验，不以浏览器结果替代。

同日圆角体验包 112.89 MiB 已校验、原位安装并启动；安装退出码 0，安装后的 EXE 和 21 个资源均与包一致。旧安装目录 26 个文件备份已核对，原生无障碍接口确认百变小著页面加载。窗口截图捕获仍超时，桌面视觉效果交由用户试用；未签名、未公开发布。

2026-11-04 修复安装版右键无菜单：动态菜单窗口使用了 WebView2 默认启动参数，与共享配置目录的 pet/chat/settings 不一致，原生创建异步失败（`0x8007139F`），但 Tauri `build()` 已返回成功。菜单改为直接沿用 pet 配置的 `additionalBrowserArgs`。新增真实 Windows WebView2 集成测试，加载构建后的菜单页面并调用产品 IPC；首开、可见、完成动作、隐藏、复开均通过，连同 3 项原有 Rust 测试共 4 项通过。回归命令：先 `bun run build`，再 `cargo test --manifest-path src-tauri/Cargo.toml --lib pet_menu::tests -- --include-ignored --nocapture`。测试使用独立应用身份，不读取用户账户；其调用 IPC 不等于物理鼠标命中验收。修复前失败及修复后日志位于 `E:/Codex/工作伙伴/design-artifacts/xiaozhu-radial-menu-r04-20261103`。r04 Windows 体验包已完整构建、校验、安装并重开；安装器退出码 0，安装后的 EXE 和 21 个资源与包一致，r03 安装完整备份。原生无障碍接口确认百变小著页面加载；截图接口仍超时、自动鼠标定位不可用；用户随后在安装版确认“能正常展开”环形菜单。该用户验收覆盖实际右键展开，原生各选项操作/失焦关闭、混合 DPI 和 macOS GUI 仍未完整验收。包与回执：`E:/Codex/工作伙伴/release-artifacts/xiaozhu-radial-menu-r04-20261104`。

源码 `75155be` 的 macOS 全量检查、固定工作台重建通过。[Windows CI 首轮及重跑](https://github.com/ciph/deskmate/actions/runs/41766403073/attempts/2) 均为 980 passed / 1 failed / 25 ignored，失败项为 `memory::cross_process_tests::simultaneous_first_launches_migrate_the_database_exactly_once`：子进程读取数据库版本时出现 `database is locked`。2026-11-04 已用持有排他锁的独立进程稳定复现；根因为版本预检只允许 100 毫秒锁等待。修复为预检等待 5 秒，保留 WAL 阶段原有短等待/有界重试及未来 schema 拒绝。新增多进程回归修复前失败、修复后通过；完整记忆模块 124 passed / 0 failed / 3 ignored，包含并发首次迁移、跨进程写入、迁移回滚和未来版本保护。证据：`E:/Codex/工作伙伴/design-artifacts/ci-memory-initialization-20261104`。修复提交 `6c893fb` 的 [CI](https://github.com/ciph/deskmate/actions/runs/41769511312) 已全部通过：Windows/macOS 源码检查和固定工作台重建成功，该初始化锁竞争修复验收完成。此次未重建或更新已安装的 r04 体验包。

第三版缩小内圈留白和选项区，新增半透明渐变、轮廓高光与薄投影。13 项菜单前端测试、3 项 Rust 定位/动作边界测试、类型检查与构建通过，React Doctor 93/100 无新增诊断；浏览器实际组件核对了紧凑尺寸、角落排布、浅深色主题、四形态和经典 2D。Windows 包完整性、样式、图标与资源校验通过，安装器退出码 0，安装后的程序与 21 个资源均与包一致；第二版已备份，启动后无障碍接口确认桌宠加载。原生截图再次捕获超时，浏览器不能代替实际桌面透底、鼠标、焦点与混合 DPI 验收；macOS 未验，未公开发布。证据目录为 `E:/Codex/工作伙伴/design-artifacts/xiaozhu-radial-menu-r03-20261103`；包和安装回执为 `E:/Codex/工作伙伴/release-artifacts/xiaozhu-radial-menu-r03-20261103`。

第二版将圆心改为透明并对齐真实身体，靠屏幕边缘时在可见方向展开扇环，不移动或复制桌宠。AI 用量只保留设置入口，打开菜单不再查询摘要。浏览器使用实际组件与模拟原生宿主验证居中、底边、角落、四形态、经典 2D 及 AI 设置入口。33 项相关前端测试、3 项 Rust 测试、类型检查和构建通过，React Doctor 93/100 无新增诊断。本机安装器退出码 0，安装后的 EXE 和 21 个资源与包一致；旧安装已备份，重开后无障碍接口确认桌宠加载。物理混合 DPI、Windows 原生鼠标/焦点和 macOS 仍需单独验收，未公开发布。第二版证据：`E:/Codex/工作伙伴/design-artifacts/xiaozhu-radial-menu-r02-20261103`；包、安装与源码回执：`E:/Codex/工作伙伴/release-artifacts/xiaozhu-radial-menu-r02-20261103`。


第一版历史记录：独立透明窗口中的分层圆环，经典 2D 保留。浏览器实际组件已验证右键打开、分类切换、形态和外观操作、中心返回、键盘打开、Esc 收起；菜单、共享交互、加载/重载、鼠标穿透和用量缓存相关测试通过。第二版已移除中心信息区和用量缓存，返回移到环上。屏幕边界及负坐标在 Rust 单元测试覆盖，不等于真实跨屏验收。

Windows 本地构建与安装已完成（安装器退出码 0），包内 EXE 与打包输入一致，安装后的 EXE 和 21 个资源与包一致；旧安装已备份，未公开发布。重开后无障碍接口确认桌宠加载。截图仍超时、鼠标定位不可用，Tab/Shift+F10 尝试仍停在 WebView 窗格，未观察到原生菜单，因此真实右键、失焦关闭、混合 DPI、多屏边缘和 macOS 继续待验；真实账户 API 未重测。78 项相关前端测试、3 项 Rust 测试、类型检查与构建通过，React Doctor 93/100 无新增诊断。回执：`E:/Codex/工作伙伴/release-artifacts/xiaozhu-radial-menu-r01-20261103`；浏览器证据：`E:/Codex/工作伙伴/design-artifacts/xiaozhu-radial-menu-20261103`。

## 桌宠右键 AI 用量

2026-11-05 用户体验反馈后更新为 r02：云朵出现及单击/Esc 关闭均为 300 毫秒线性淡入淡出，原生窗口就绪后淡入、动画结束后关闭；快速关闭从当前透明度继续，兼容减少动态效果。9 项云朵测试、类型检查、前端构建和真实 WebView2 窗口复用/关闭回归通过，React Doctor 83/100 无新增诊断。浏览器实际组件采样淡入透明度 0.056→1、淡出 1→0.167→移除；证据为 `E:/Codex/工作伙伴/design-artifacts/ai-usage-cloud-fade-20261105`。112.94 MiB 本机安装包已校验并原位安装，退出码 0，安装后 EXE 与 21 个资源匹配，旧版 26 个文件已备份；已重启且原生无障碍接口确认百变小著加载。未公开发布，真实桌面动画手感待用户复验。包及安装回执：`E:/Codex/工作伙伴/release-artifacts/ai-usage-cloud-fade-20261105`。

2026-11-05 云朵交互已在 0.5.0 开发树实现：右键选择 AI 用量后在头顶显示剩余百分比和金额，单击关闭，不进入设置。类型检查、生产前端构建、78 项相关前端测试、5 项 Rust 菜单/云朵定位与数据边界测试通过。Windows WebView2 原生测试实际加载生产页面，连续打开/关闭两次圆环、两次云朵，并由云朵 DOM 按钮触发关闭 IPC；同时覆盖失焦事件不关闭云朵。React Doctor（`--scope changed --base HEAD --include-untracked`）83/100，无新增诊断。

实际组件浏览器验证了菜单进入、加载后显示 91% / ¥2726.00、单击关闭、深色左下角和查询失败状态，使用合成数据与模拟原生宿主。证据：`E:/Codex/工作伙伴/design-artifacts/ai-usage-cloud-20261105`。本轮未打包安装、提交、推送或发布；真实账户 API、Windows 物理鼠标及混合 DPI、多屏切换、macOS 原生云朵仍未验。以下为旧列表摘要的历史验收，不代表当前云朵完成这些未验项。

2026-11-05 后续按用户要求完成本机打包与原位更新：0.5.0 云朵体验包 112.92 MiB，安装器退出码 0，安装后的 EXE 与包内文件 SHA-256 一致，21 个资源一致；旧安装目录 26 个文件已备份并校验。已重新启动，原生无障碍接口确认百变小著加载，实际用量云朵交由用户试用。复用与 r04 相同的工作台输入及资源，无新增工作台重建声明；没有升版本、提交、推送或公开发布。包、校验和安装回执：`E:/Codex/工作伙伴/release-artifacts/ai-usage-cloud-20261105`。上述物理鼠标、真实账户、混合 DPI 和 macOS 未验项仍保留。

2026-11-02 在 0.5.0 开发树完成源码。64 项相关前端测试、Rust IPC 边界、类型检查和构建通过；React Doctor 93/100，无新增诊断。2026-11-03 已推送源码 `6b8ee44`，Windows/macOS 源码检查及固定工作台重建 CI 全部通过，并完成本机 0.5.0 原位更新、文件校验与重开；没有公开发布。[功能契约](ai-provider-support.md#桌宠右键用量2026-11-02)、[源码验收回执](performance-evidence/pet-ai-usage-20261102/receipt.json)、[本地安装回执](performance-evidence/pet-ai-usage-20261102/local-install-20261103.json)。

- Windows 原生运输已验：独立 QA 身份下创建真实菜单，通过 `popup_pet_menu` 弹出，保持打开时延迟 `setText` 成功，回读文本正确，系统 Esc 结束调用、随后释放资源。
- Windows 原生操作待验：真实鼠标右键命中/点外关闭、真实账户摘要刷新、详情进入 AI 页、百变小著及 GIF/rig2d/GLB 菜单视觉、长供应商名称和不同 DPI 下的宽度。2026-11-03 安装后无障碍接口确认百变小著界面，但截图工具仍超时、坐标输入不可用，键盘尝试未取得菜单结果；不能把启动成功、IPC 探针与组件测试视作这些项目已通过。
- macOS 原生菜单刷新/关闭待验；真实账户 API 未重测。本轮沿用已有后端来源与统计边界。

## 百变小著（`xiaozhu-morph`）

第 06 版已于 2026-11-03 原位安装并重新打开，主程序与包内文件一致、21 个资源校验通过。无障碍接口确认百变小著已加载；原生截图首次与重新选窗重试均超时，因此安装后像素外观仍未自动验收。具体进程、备份和校验值见第 06 版目录的 `installation.json`。

2026-11-03 第 06 版修复透明点击框的矩形焦点描边：真实浏览器 Tab 输入复现 400 像素横线，移出焦点后消失；改为沿绘制像素显示焦点，原横线区域的 11 张对照截图均无残留。浏览器覆盖四形态切换、经典 2D、四主题与 100%～175% CSS zoom、空格互动；这是浏览器模拟显示比例，不是物理混合 DPI 验收。组件 8、共享交互 12、菜单 4 项、类型检查与构建通过，React Doctor 93/100，无新增问题。证据：`E:/Codex/工作伙伴/design-artifacts/xiaozhu-transparent-focus-20261103`；第 06 版包校验及安装回执：`E:/Codex/工作伙伴/release-artifacts/xiaozhu-milk-r06-20261103`。macOS、原生截图及多屏 DPI 仍未完成验收。

2026-11-03 第 05 版安装：按用户要求原位更新本机 YUME，安装器退出码 0，安装后主程序哈希与体验包一致、21 个资源匹配；已重启，原生无障碍接口确认百变小著加载，旧独立奶团预览已退出。原始经典 2D 保留，用户数据与设置未清理。回执：`E:/Codex/工作伙伴/release-artifacts/xiaozhu-milk-r05-20261103/installation.json`；本次未执行原生菜单外观切换、视觉、性能及混合 DPI 验收。

2026-11-03 第 05 版：移除奶团眉毛，保留嘴部表情及镜像腮红，经典 2D 不变。类型检查、8 项组件测试、6 项渲染测试通过，React Doctor 93/100，无新增问题；浏览器验证待机、拎起落下和睡眠时没有眉毛且嘴形不同，无控制台错误。Windows 安装器完整性、内嵌 EXE 与打包输入一致、21 个资源和图标校验通过，回执位于 `E:/Codex/工作伙伴/release-artifacts/xiaozhu-milk-r05-20261103/package-verification.json`。工作台输入与第 03 版相同；未安装、签名或公开发布，原生视觉及混合 DPI 验收仍待完成。

2026-11-03 表情细化第 04 版：奶团短圆弧眉毛、腮红及高光镜像；经典 2D 不变。类型检查、8 项组件测试、6 项渲染测试通过；React Doctor 93/100，无新增问题。浏览器实看待机及拖放表情、镜像引用和眉毛显隐，未见控制台错误。Windows 安装器完整性、内嵌 EXE、21 个资源、图标及第 04 版前端标记校验通过，回执位于 `E:/Codex/工作伙伴/release-artifacts/xiaozhu-milk-r04-20261103/package-verification.json`。沿用第 03 版同一固定输入的工作台构建，源码未变；未执行安装后原生验收或公开发布。

2026-11-03 奶团造型细化版：更新眼镜比例、奶团表面与分层圆脚/阴影；原始 2D 模型不变。类型检查、前端构建、组件与几何专项通过，React Doctor 93/100；网页实看四种形态、160 像素浅/深色背景及戳碰、睡眠/叫醒，浏览器无错误。源码与网页预览一致，证据位于 `E:/Codex/工作伙伴/design-artifacts/xiaozhu-milk-motion-v1-20261103/revision-03`。随后按用户要求重建 Windows x64 NSIS 体验包，固定上游工作台完整重建、包完整性、内嵌 EXE、21 个资源、图标与两套外观产物检查通过，交付回执位于 `E:/Codex/工作伙伴/release-artifacts/xiaozhu-milk-r03-20261103/package-verification.json`。未安装或公开发布；原生视觉、性能和多屏 DPI 验收继续保留。

2026-11-03 奶团外观本地候选：保留经典 2D，右键切换并记住外观，幽灵变回团子时同步下降。类型检查、前端 186/186 文件、外观切换/重载/共享动作与菜单专项通过；React Doctor 93/100，无新增问题。独立身份 `com.deskmate.unifiedhistoryqa.milk20261103` 的 Windows 原生预览已启动，无障碍接口确认加载百变小著；截图接口连续超时，鼠标定位几何不可用，键盘尝试未取得菜单结果，故奶团材质视觉、真实外观切换/重启记忆、拖放与原生帧耗时仍待验。此次未覆盖安装、macOS 或混合 DPI 多屏，未公开发布。本地构建与证据位于 `E:/Codex/工作伙伴/release-artifacts/xiaozhu-milk-candidate-20261103`。

2026-11-01 在 0.4.21 分支实现并于 Windows 原生预览（v1–v5）逐版验证，整合进 main 后随 Windows v0.5.0 发布（[回执](performance-evidence/release-v0.5.0-windows/README.md)）；最终安装包未做安装后实测，macOS 未发布。版本记录、命令与原生测量方法见[百变小著](xiaozhu-morph.md#验证与交付边界)。

| 范围 | 已有证据与剩余边界 |
| --- | --- |
| 原生交互 | Windows 125% 三屏 60 Hz 下用真实系统输入实测（v4 及整合 main 后的 v5）：60 fps 级帧节奏、像素墨镜稳定、透明区域穿透、单击/空格戳碰、键盘 1–4 切换形态、双击与 Enter 开关聊天、聊天时暂停自主移动、空闲后自动散步、静止松手垂直落下、甩出保留惯性、跨屏落地、右键菜单 |
| 已知问题 | 双击打开聊天偶发无响应：v5 粗采样约 6 次、v4 光标确在宠物上 2 次，未复现且原因未查明；Enter 与右键菜单可替代 |
| 未覆盖操作 | 睡着/叫醒的完整原生周期、AI 实际工作时的状态标记 |
| 显示 | 150%/175% 缩放、120/144 Hz、DPI 不同的显示器间拖动未实测 |
| 平台 | macOS 未验；Mac 资源清单已含百变小著资源，仅源码测试覆盖 |

## B7 与发布仍缺少的证据

| 范围 | 已有证据与剩余边界 |
| --- | --- |
| 最终安装与升级 | Windows 0.4.24 包/图标/签名/匿名下载已过；真实安装与跨版本 GUI 自动升级未实测 |
| Mac | CI 源码检查及未签名候选编译已过；最终实机 GUI/恢复、签名安装与更新仍未验，本次未发 Mac 包 |
| 显示与性能 | 同 125% DPI 多屏、当前版本输入/滚动/PDF 堆与 RSS/十分钟空闲已有记录；混合 DPI、高刷、受控 GPU/功耗前后对照仍缺少 |
| 真实模型 | 合成 provider 的审批/日志流程通过，不等于真实模型的日志意图与记忆语义准确率 |
| 故障与迁移 | 自动容量限制/事务故障、原生受控失败回滚、重开已验；磁盘实际耗尽、任意中断及完整修复 UI 未穷尽 |
| 渲染与生命周期 | 三类宠物隐藏恢复、PDF 32 次 worker 清理已验；全部动作极值和长期零泄漏未证明 |

保留原模型与默认 60 fps。长历史全挂载、界面线程全文 Markdown、回执重复全会话读取三条链的本轮实现与验收见[长历史施工交付记录](performance-evidence/long-history-closeout-20261029/README.md)。首次正文仍完整加载，Markdown Worker 仍执行全文解析；R06 全快照比较、D20 全库读取、M19c 硬删除扫描仍保留，见 [B7 交付报告的开发建议](performance-evidence/b7-windows-closeout-20261028/README.md#后续开发建议)。这些边界不自动转成无限验收轮次。

## 其他既有审计与平台待办


| 项目 | 尚需完成 / 证据入口 |
| --- | --- |
| 原审计 UI U1/U2 | 重连、busy Stop、idle 后 scheduled、多个 text parts 和迟到响应最小反例与修复；见[施工进度](audit-remediation-progress.md) |
| 工作台 W1/W2 | 自有源码直接表达行为、固定输入统一验证、重复准备与失败恢复；普通前端绿灯不关闭此项 |
| 历史 H1/H2/H3 | H2 按 key 查询、变化行写入与导入指纹已于 2026-10-10 完成源码/隔离测试；新正文投影退出及目标安装版性能仍待验。保留 legacy/local-only、墓碑及身份约束 |
| 综合 V1 | B7 已补隔离 Windows 审批、跨 30 秒归档失败时的控制推进、取消和活跃工具二次启动；完整 Mac、最终安装包及升级/回退仍按范围待验，不能把合成模型当真实语义验收 |
| CI / Windows | v0.4.24 两平台 Source checks 与发布流程已实际通过，见[发布回执](performance-evidence/release-v0.4.24-windows/README.md)；branch protection required checks 配置未在本轮核验 |
| 文件清理条件项 | 以[清理执行进度](repository-cleanup-progress.md)为准；Windows 0.4.22 包图标已验，外部作者资产、其余生成链与 Mac 实机条件不因本机清理而关闭 |
| Windows QA harness 旧边界 | `p5-windows-e2e.ts` 仍需给 MCP 回传 Notepad PID补新建进程/出生时间/可执行文件归属证明，并保证 setup/cleanup 首项失败不跳过其余关闭与临时根移除；本轮已改默认资源路径、平台预检与schema断言，不宣称此脚本已完整 Windows验收。工作记录 capture 新身份/PID/窗口渲染校验也需要实际 Windows 运行 |

## 从功能施工记录转移的未完成事项

| 范围 | 需要的后续验证 | 保留的原证据 |
| --- | --- | --- |
| 自动记忆/日志 | 36 条中英文真实模型语义样例：引用/自述、同义更正、去重、完成/取消、敏感负例与连续事项；确认当前日常应用完整交互。只用合成样例和隔离库，不把模拟模型联调当语义准确率 | [自动处理验收](automatic-memory-verification.md#真实模型验收与当前边界) |
| 附件原生事件/播放器 | Windows 的 MP4/MP3/WAV、文件夹拖放、播放跳转/移除/坏媒体和重开已有 [release 物理证据](performance-evidence/b7-windows-release-acceptance-20261028/README.md)；Mac、未覆盖 MIME 与长期压力仍待验 | [原生回归](attachment-native-regression.md#2026-09-28-本机记录) |
| 历史与 composer | B7 Windows 已补长流式、Stop/焦点、审批与历史重开；旧记录中的全部窗口尺寸/离线/模型组合仍需按场景核对，不能按一项通过整行关闭。360px CSS 压力规则仍没有 360px 原生截图 | [organizer 回执](archive/history/history-organizer-redesign-plan.md#8-施工记录2026-09-27)、[composer 回执](archive/chat/chat-composer-workspace-model-plan.md#10-实施与验收记录2026-09-27) |
| 设置与原生材质 | 原 P0 中剩余角色包/异常状态、跨窗口主题同步、全部页面语言组合；标题栏真实拖动、不同系统缩放。P1 材质统一/P2 原生 Vibrancy/Mica 应先核对后续样式接入，再确定尚未覆盖的范围和性能对照 | [P0/P1/P2 历史计划](archive/ui/frontend-redesign-phase-2-plan.md#后续验收边界)、[较晚原生检查](archive/ui/yume-latest-integration-review.md#后续macos-原生窗口检查) |
| 星运 | Windows 原生窗口及日期变更/恢复行为复验；已有本机持久化、四语言和四主题截图保留，不当 Windows 通过 | [首版验收](archive/horoscope/daily-horoscope-implementation-plan.md#12-首版实施验证记录2026-09-30) |
| 发布与更新 | v0.4.24 Windows 内容、图标、更新签名和匿名下载已核验；真实安装/隔离 A→B、活动任务等待、取消/失败恢复，以及后续 Mac 签名公证与实机链路仍按目标版本验证 | [macOS gate](macos-release.md)、[runtime checks](macos-runtime-checks.md) |

## 原生迁移能力余项

固定引擎能力矩阵仍保留：多题/多选/自定义 question 回答、未覆盖平台的原生拖放及更多 MIME、需要模型执行的 slash 命令、可核验 `/terminal` 面板、CLI/TUI 专有入口、非 Git 目录审查/撤销，以及同环境迁移前后的性能对照。不同版本升级/降级须用隔离旧/新包和数据副本验证；UI 回退不等于数据库格式降级。逐项状态见[能力矩阵](migrations/opencode-native/capabilities.md#8-p6-安装版能力终验状态2026-09-23)。

这些是 2026-09-23 记录留下的未验项，后续若已有独立证据，先对照版本、平台和场景再关闭。旧记录的“私钥密码未知 / 无签名 / sidecar 无鉴权 / 不自动重启 / 静态工作台记忆”已不是当前维护入口：当前签名能力按发布指南预检，当前上下文按[记忆契约](memory-and-worklog.md)及 D1-12 修订读取。

[早期 Agent 架构审计](archive/architecture/AGENT_HARNESS_ROADMAP_AUDIT.md)的目录/权限/停止/恢复与真实产物闭环要求，按当前 U/V 与原生能力项核对；性能改进仍需同环境前后测量。其 Desktop Context、通用触发器、可执行 Skills、多 Agent 等候选方向保留为历史建议，未因归档或移入索引转成已批准的施工待办。

兼容条件持续保留：旧格式 reader、纯旧文本、完整复合身份、local-only 回复、墓碑和不复活；不得以清理文档或完成某项门禁为由让旧二进制写新版数据库、覆盖新增数据、重放 prompt 或削弱批准边界。

## 完成回写

执行时记录当前 commit/未提交 diff、运行时、完整命令、退出码、通过/失败/skip/ignored、合成数据和证据路径；实机、模型、源码与公开资产分别记录。关闭一项需对应范围的真实新证据，不用旧测试数、截图存在或 archive 标记代替。

## 历史验收记录

[完整历史记录](archive/verification/history-through-20261028.md) 保留原基线、失败、测试数和未验条件。以下锚点保留旧契约链接的去向。

<a id="2026-10-24-mac-b6-h18a"></a>

[原记录：2026-10-24-mac-b6-h18a](archive/verification/history-through-20261028.md#2026-10-24-mac-b6-h18a)

<a id="2026-10-24-mac-b6-h18b"></a>

[原记录：2026-10-24-mac-b6-h18b](archive/verification/history-through-20261028.md#2026-10-24-mac-b6-h18b)

<a id="2026-10-25-mac-b6-h18c"></a>

[原记录：2026-10-25-mac-b6-h18c](archive/verification/history-through-20261028.md#2026-10-25-mac-b6-h18c)

<a id="2026-10-25-mac-b6-m19a"></a>

[原记录：2026-10-25-mac-b6-m19a](archive/verification/history-through-20261028.md#2026-10-25-mac-b6-m19a)

<a id="2026-10-25-mac-b6-m19b"></a>

[原记录：2026-10-25-mac-b6-m19b](archive/verification/history-through-20261028.md#2026-10-25-mac-b6-m19b)

<a id="2026-10-25-mac-b6-m19c"></a>

[原记录：2026-10-25-mac-b6-m19c](archive/verification/history-through-20261028.md#2026-10-25-mac-b6-m19c)

<a id="2026-10-25-mac-b7"></a>

[原记录：2026-10-25-mac-b7](archive/verification/history-through-20261028.md#2026-10-25-mac-b7)

<a id="2026-10-27-mac-b7-接续"></a>

[原记录：2026-10-27-mac-b7-接续](archive/verification/history-through-20261028.md#2026-10-27-mac-b7-接续)

<a id="2026-10-27-mac-b7-流式与-pdf"></a>

[原记录：2026-10-27-mac-b7-流式与-pdf](archive/verification/history-through-20261028.md#2026-10-27-mac-b7-流式与-pdf)
