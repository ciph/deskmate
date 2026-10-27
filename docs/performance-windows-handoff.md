# YUME 性能整改：Windows 接管入口

交接日期：2026-10-27（Asia/Shanghai）。用户指定下一阶段由 Windows 电脑继续施工；本次保存并推送现有代码、进度和证据，不新增发布或日常数据迁移。

## 接管位置与完成状态

- 仓库：`https://github.com/ciph/deskmate.git`；施工分支：**`codex/yume-performance-b6`**。Windows 从该远端分支接续，不从旧 `main` 重做 B6，也不依赖旧 Windows 分支的 upstream。
- 交接前检查点：`db96597`（B7 流式/PDF 证据），最近代码修复：`3069953`（原生文本 delta）。本交接文档作为后续独立提交；拉取后记录实际 `HEAD`。本次核对的 `origin/main` 仍为 `fcb463516bdde3f8bd0e3f8aa2ab287f0d355356`。
- B1–B5 已随 Windows 0.4.21 发布。B6 H18a/b/c、M19a/b/c 代码及 Mac 自动回归完成；历史目录 schema **3**、记忆库 schema **4**，产品版本仍为 **0.4.21**。不得改回 schema 版本号或让旧二进制写入新库。
- **B7 进行中**。Mac 最新前端 172 文件 / **1359 pass / 0 fail / 0 skip**，类型检查、生产构建与隔离 QA App 构建通过。最近 Rust 全套为上一轮 **957 pass / 23 ignored**；最新流式修改未改 Rust，未重复执行，不把 Mac 数量或 ignored 状态预先写成 Windows 通过。
- Mac 已补 1000 消息、计划 50 Hz 的 10k/50k 流式、Stop/继续与 PDF worker 32 次循环。PDF 内存收敛未测得，GUI/硬件与 Windows 原生缺口仍保留。详情见[最新证据](performance-evidence/b7-macos-20261027/index.html)、[完整边界](performance-evidence/b7-macos-20261027/README.md)和[统一待办](verification-backlog.md)。

## Windows 拉取

在选定的 Windows clone 中逐条执行并检查退出码：

```powershell
git remote -v
git status --short --branch
git log -5 --oneline --decorate
git fetch origin --prune
```

先保存已有本机工作；不自动 stash/reset/clean，不覆盖其他任务的未提交改动。确认 origin 为上述仓库、工作区干净后，选择适合当前状态的一组命令。

本地还没有施工分支：

```powershell
git switch --create codex/yume-performance-b6 --track origin/codex/yume-performance-b6
```

本地已有施工分支：

```powershell
git switch codex/yume-performance-b6
git merge --ff-only origin/codex/yume-performance-b6
```

若无法 fast-forward，保留双方提交，先检查分叉原因，不强制重置。随后核验：

```powershell
git merge-base --is-ancestor db96597 HEAD
git rev-parse HEAD
git rev-parse origin/codex/yume-performance-b6
git status --short --branch
```

祖先检查应成功；无本地额外施工时两个 SHA 应相同，并与本次推送回执核对。尚无 clone 时，可先在新的目录执行 `git clone --branch codex/yume-performance-b6 https://github.com/ciph/deskmate.git`。不要复制 Mac 的 `.git`、`node_modules`、`target`、sidecar 或生成的工作台。

## 重建本平台环境

先核对 Windows 原有开发环境与 [Source checks](../.github/workflows/check.yml)：Bun **1.3.14**、Rust **1.94.1**、OpenCode **1.18.21**。记录 Windows、WebView2、CPU/GPU、显示器 Hz/DPI 及实际工具链；不修改锁文件来规避环境差异。Mac 的 `output/` 与 `context.sh` 未提交，不是 Windows 启动依赖。

```powershell
bun --version
rustc +1.94.1 --version
cargo +1.94.1 --version
$env:RUSTUP_TOOLCHAIN = "1.94.1"
bun install --frozen-lockfile
bun run prepare:sidecar
```

完整工作台按[构建契约](workbench-build.md)准备独立上游源码：固定 commit `826d9ad46a22bef0294998e08daa3c4904fea28f`；Bun 1.3.14 安装冻结依赖，安装前后根 `bun.lock` SHA-256 均应为 `55cde1d8d6921ddd764ad99e6be8755767ca81e10f439e047014b7aaacffae5c`。不要覆盖已有其他用途的上游 checkout。

回到 YUME 根目录，把下面的路径替换为实际 Windows 上游绝对路径，再执行：

```powershell
$env:YUME_OPENCODE_SRC = "C:\replace-with-your-path\opencode-v1.18.21"
bun run prepare:workbench
bun run check
cargo +1.94.1 check --locked --manifest-path src-tauri/Cargo.toml
```

这些操作在 Windows 上建立新的基线，本次 Mac 交接没有代执行。套件、工作台生成、构建、性能采样顺序进行；出现失败先记录日志并区分平台环境与代码问题，再修复和做必要复测。`bun run build` 单独成功不代表工作台重建完成；`dev:client` 的 QA 配置也不代表生产启动验收。真实 OpenCode 辅助脚本如默认使用无扩展名二进制，须先检查其 Windows 路径/环境参数和清理规则，不照搬 Mac 命令。

## 下一阶段施工顺序

1. **Windows 本机基线与启动所有权**：固定依赖完整检查、生产配置检查；独立 QA 身份下验首实例、二次/并发启动、真实孤儿恢复，确认活跃宿主、引擎与工具任务连续，不误杀日常进程。
2. **WebView2 对应链路**：PDF 本地 worker/CSP、正常/损坏/超预算/取消及内存循环；原生 picker/drop、媒体/二进制上传、发送/历史重开；1000 消息 50 Hz、10k/50k 的终态、Stop、焦点、导航与滚动。Mac 数字只作来源记录，不作为 Windows 基线或性能对照。
3. **GUI 运行与存储闭环**：审批/拒绝/取消、SSE 断流/故障恢复、日志五工具/到期；历史删除/多窗口争用、记忆导出/忘记/重开。继续保持完整身份、所有权、墓碑、隐私和迟到事件边界。
4. **硬件与剩余成本**：2D/3D 全动作、隐藏/睡眠、跨屏 DPI/高刷、内存/GPU/能耗和同环境生产对照；M19c 完整删除外键扫描/小库成本、C16 全行挂载、R06/D20 残留复杂度。证据不足的项目仍列待验，不把平台转换当作完成。

Mac 专有的 GUI 审批/SSE/日志、WebKit PDF 堆/RSS、原生硬件与签名包范围继续留给 Mac 验证；Windows 结果不关闭这些缺口。完整逐项门槛以[施工计划第 11 节](performance-remediation-plan.md#11-b7验证退出条件与回退)和[验收待办](verification-backlog.md)为准。

## 数据、证据与收尾

Mac 本轮专属 QA 进程、虚拟凭据、端口与数据根已清理，5 个 SQLite 完整性通过，标准生产 dist 已恢复。[清理回执](performance-evidence/b7-macos-20261027/cleanup.json)和便携报告已入 Git。旧报告内“未推送”描述的是其采样时状态，不影响本次用户授权的分支交接。

Git 包含代码、迁移、计划、待办与 `docs/performance-evidence/` 下的报告、采样、截图和校验值。原始日志、完整 native 合成导出、构建缓存及工具链仍在 Mac 的忽略目录；Windows 无需复制它们即可接续，若要复核未入库原件再按其记录定位。冻结 `inputs/` 脚本只供复核，含当次 Mac 身份与路径，不能直接重跑。

每轮使用新的 Windows 隔离身份、输出目录和合成 provider；不复制钥匙串/DPAPI 凭据或日常库。迁移仅在一致性备份/合成副本执行，保留 WAL 与回退边界；清理只针对登记的 PID/出生时间/路径和专属目录。每批记录实际 HEAD、平台、命令/退出码、原始样本及未验项，更新本入口和统一待办，按当前用户授权保存提交。此次仅推送施工分支，不合并 main、不移动既有标签、不升版或发布。

## 给 Windows 接手助手的提示

> 继续 ciph/deskmate 的 B7。先读 AGENTS.md、docs/performance-windows-handoff.md、性能施工计划、verification-backlog.md 和最新 b7-macos-20261027 证据。保留本机改动，fetch 后接续 origin/codex/yume-performance-b6，核对 db96597 为祖先并记录实际 HEAD；不要回旧 main 重做 H18/M19。B6 已完成，历史 schema 3、记忆 schema 4、产品 0.4.21。先准备固定 Windows 依赖并跑本机基线，再验 Windows 首实例/并发/孤儿、WebView2、GUI 审批/SSE/日志与存储链路。使用新隔离身份，测试/构建/测量串行，保持 Mac/Windows 与生产/QA 证据边界。逐批更新进度和未验项，保留日常数据，不自动升版或发布。
