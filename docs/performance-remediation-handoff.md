# YUME 性能整改：Mac 接续说明

核对日期：2026-10-24（Asia/Shanghai）。原始交接提交只保存施工计划、证据与待办；下节记录随后在 Mac 上完成的 H18a/H18b，后面的原发布回执仍保留其历史范围。

## Mac 接续进度：H18a/H18b

- 当前工作区 `/Users/xmaczone/Documents/Codex/deskmate`，分支 `codex/yume-performance-b6`，起点 `fcb463516bdde3f8bd0e3f8aa2ab287f0d355356`。先检查此分支和本地检查点；尚未推送时不能仅凭远端 main 判断 B6 未施工。
- 固定 Bun 1.3.14 / Rust 1.94.1 已放在本工作区 `output/toolchains/`。命令环境入口 `output/performance-b6-preflight-20261024/context.sh`，只设置当前 shell 的工具链、缓存与 `DEVELOPER_DIR`。这些忽略目录是本机生成物，新机器须重新准备。
- 已安装冻结依赖、准备 OpenCode 1.18.21 与 ncmdump 1.5.1，并完整重建工作台。Git 克隆上游两次 TLS 失败后，使用 GitHub 官方 codeload 的固定 commit `826d9ad46a22bef0294998e08daa3c4904fea28f` 源码归档；目录为 `output/upstream/opencode-v1.18.21`，不冒称 Git clone。安装前后上游锁文件 SHA-256 均为下文要求的 `55cde1…ae5c`。
- Mac ARM64 / macOS 26.5.2 / SDK 26.5：基线完整检查通过；H18a 后完整 Rust 901 pass / 0 fail / 18 ignored。具体覆盖及边界见 [验收记录](verification-backlog.md#2026-10-24-mac-b6-h18a)。H18a 未修改 SQLite schema、产品版本或公开资产。
- H18b 从本地 H18a 检查点 `d877ba5` 接续：单条目录事务按完整 key 读/改，批量路径保留；并发状态重验、模型偏好原子删除/写入、提交后 revision、墓碑和损坏边界均有回归。1k/10k/100k 行各 100 次修改均解码 100 行、写 100 行，EXPLAIN 命中主键索引。完整 Rust 916 pass / 0 fail / 19 ignored，新增 ignored 规模测试单独通过；生产 `cargo check` 通过。见 [H18b 验收记录](verification-backlog.md#2026-10-24-mac-b6-h18b)。schema 仍为 2，版本仍为 0.4.21。
- 下一步 **H18c**：筛选/排序投影列及 SQL 分页，先在隔离合成库验证迁移与旧实现查询等价；M19a/b/c 与 B7 尚未实施。下方三个时钟边界待办仍未改，本轮的自动回归不关闭 Windows/macOS 实机缺口。

本机基线/H18a 日志、源码校验与工作台生成物清单集中在 `output/performance-b6-preflight-20261024/`；H18b 日志与汇总在 `output/performance-b6-h18b-20261024/`，两目录均忽略于 Git。源代码与现行契约以当前 B6 分支为准，历史发布证据仍在 `docs/performance-evidence/`。

## 从哪里继续

仓库：[ciph/deskmate](https://github.com/ciph/deskmate)。先读 [AGENTS](../AGENTS.md)、本页、[完整施工计划](performance-remediation-plan.md)、[当前验收待办](verification-backlog.md)；存储实施前读 [统一历史契约](migrations/opencode-native/unified-history-contract.md) 和 [记忆与工作日志](memory-and-worklog.md)。本次计划是 2026-10-22 性能审计的接续，不与 9 月旧审计计划混用。

| 项目 | 交接时状态 |
| --- | --- |
| 已发布源码基线 | `fee4d707804f76e5c608ceef33c622e71d17b175`，`v0.4.21`，产品版本 0.4.21 |
| Git 入口 | 文档交接提交在此基线之后；Mac 拉取最新 `origin/main`，重新核对新增提交 |
| B1 | 功能/数据阻断修复；`a48ee36be8195766322439843fd76db69819fa40` |
| B2 | 交互与前端工作量；`19630b6a69d5944c53156bf666879cf3afd7cf0c` |
| B3 | 启动与后台；`6f97c93b984ced6e7c36e2e1d15b9b84c8dc3994` |
| B4 | 附件、桌宠与模型候选；`5ef42d8e69c002fc1446e7a1ed943c658bd139a5` |
| B5 | 锁范围、SSE、流式合批、删除扫描；`324374839ad04a7f2820ff0c0e41480ba810c1ee`；随后两次测试夹具修复至 `fee4d70` |
| B1–B5 验收 | 代码和所列自动回归通过，原生缺口保留；默认 60 fps 与原版模型保留，30 fps / 缩图候选未采纳 |
| B6 | **尚未开始**：H18 历史库与 M19 记忆库，见计划第 10 节 |
| B7 | **尚未开始**：联合回归、平台实机验证、逐项结论与交付 |

Windows [v0.4.21](https://github.com/ciph/deskmate/releases/tag/v0.4.21) 已于 2026-10-23 公开，01:06（北京时间）完成匿名下载回读。最终 [发布 CI](https://github.com/ciph/deskmate/actions/runs/39813641175) 六个 job 及 [main 检查](https://github.com/ciph/deskmate/actions/runs/39813642512) 全部通过；固定上游工作台已完整重建。

| 最终发布 CI | 前端 | Rust library |
| --- | --- | --- |
| Windows | 171 文件，1341 pass / 0 fail / 8 skip / 0 unfinished | 904 pass / 0 fail / 19 ignored |
| macOS | 171 文件，1347 pass / 0 fail / 0 skip / 0 unfinished | 895 pass / 0 fail / 18 ignored |

这是发布源码的历史回执，不是 Mac 本机或后续 B6 提交的测试结果。Mac CI 只构建了未签名候选，未公开 macOS 新包；Windows 未进行原生安装和 GUI 升级验证，Authenticode 为 `NotSigned`，Tauri updater 签名已验证。细节与哈希见 [便携证据](performance-evidence/README.md)。

## Mac 先准备工作区

使用已有 Mac clone，或在新目录 clone。不要拷贝 Windows 链接工作树的 `.git`：它引用另一目录的 Git 元数据。不要迁移 Windows `node_modules`、`target`、sidecar 二进制或生成的工作台作为 Mac 构建输入。

```sh
# 仅没有 clone 时，在选定父目录执行
git clone https://github.com/ciph/deskmate.git
cd deskmate

# 已有 clone 则直接进入它，先检查，保留已有改动
pwd
git remote -v
git status --short --branch
git log -5 --oneline --decorate
git fetch origin --prune
```

确认工作区干净、origin 指向该仓库，且本机 main 没有未合并提交后，再逐条执行；如有本机工作先核对并保存，不自动 stash/reset/clean。已有同名分支时检查后接续，不强制重建。

```sh
git switch main
git merge --ff-only origin/main
git merge-base --is-ancestor fee4d707804f76e5c608ceef33c622e71d17b175 HEAD
git switch -c codex/yume-performance-b6
bun -p 'require("./package.json").version'
```

原 Windows 分支 `codex/yume-performance-remediation` 的 upstream 曾指向本地旧分支；接手不要依赖这一配置，明确使用 `origin/main`。发布指南中的 `/Users/xiao/my-project/deskmate` 和 Windows 的 C/E 盘路径均为历史机器路径；本机以 `pwd` 与 Git 状态为准。

## 准备依赖和构建输入

与仓库 [Source checks](../.github/workflows/check.yml) 对齐：Bun **1.3.14**、Rust **1.94.1**、`opencode-ai` **1.18.21**。记录 Mac 芯片/架构、系统、WebKit、屏幕 Hz/DPI 和实际工具链；先完成对应工具安装，避免全局升级依赖或改锁文件来消除环境差异。

```sh
bun --version
cargo +1.94.1 --version
rustc +1.94.1 --version
bun install --frozen-lockfile
env DEVELOPER_DIR=/Library/Developer/CommandLineTools bun run prepare:sidecar
```

按 [工作台构建](workbench-build.md) 准备独立的 OpenCode 源码 clone，核对提交 **`826d9ad46a22bef0294998e08daa3c4904fea28f`**，在该 clone 中以 Bun 1.3.14 执行 `bun install --frozen-lockfile`。上游根锁文件 SHA-256 应为 `55cde1d8d6921ddd764ad99e6be8755767ca81e10f439e047014b7aaacffae5c`，与固定提交 Git blob 一致且安装后不变；不要覆盖已有其他用途的 clone。

回到 YUME 根目录，替换下面的上游绝对路径后执行：

```sh
env DEVELOPER_DIR=/Library/Developer/CommandLineTools   YUME_OPENCODE_SRC=/absolute/path/to/opencode-v1.18.21   bun run prepare:workbench
env DEVELOPER_DIR=/Library/Developer/CommandLineTools   RUSTUP_TOOLCHAIN=1.94.1 bun run check
```

这里只设置命令环境，不修改全局 Xcode 选择或许可证。TagLib/SDK 编译问题按 [本机发布指南](local-release-runbook.md) 排查，不以改业务代码绕过。新 Mac 先做一次本机基线；之后仅在相关修改、失败或未解决问题需要时重复全套。测试、工作台生成、构建和性能测量顺序执行，避免资源争用污染结果。

普通 `bun run build` 不证明工作台完整重建；`dev:client` 使用 QA 配置，不代表完整生产启动路径。凭据留在各平台自己的受管存储/钥匙串，交接不复制凭据或日常应用数据。开始施工不要求先签名、公证或发布。

## 第一批施工的具体边界

1. 复核 G0，记录实际 HEAD、未提交差异和基线测试结果；保留只读旧库及校验值，再建立可写隔离副本。正在使用 WAL 的数据库用 SQLite 一致性备份；不要只复制主 `.db` 而遗漏未检查点的 WAL 数据。迁移只跑副本，生成 1k / 10k / 100k 合成规模数据。
2. **先 H18a**：盘点全部历史写入入口，成功提交后才更新 revision/generation；缓存命中不再克隆/哈希全量正文。保留外部变化的启动/低频 reconcile，工作台 6 秒租约过期不能只依赖数据库写入失效。验收失败写、并发写、重启、迟到快照和墓碑后，保存独立提交。
3. H18a 稳定后 **H18b → H18c**：完整身份下按 key 事务，再增加投影/索引/SQL 分页。保留筛选大小写/子串、空会话、置顶、稳定排序、所有权、墓碑和总数语义；用 EXPLAIN 与解码行数验证，不承诺深 offset 恒定耗时。
4. 记忆库按 **M19a → M19b → M19c**：到期部分索引 → 读取实时过滤及有界维护 → 显式稳定 INTEGER PRIMARY KEY / FTS rowid 映射。历史库和记忆库可独立组织工作，但同一 schema/触发器内部按顺序实施，不同时混改迁移。
5. 每步验证迁移失败/中断、重开、并发、未来 schema、回滚和删除隐私。不得编辑已应用的旧迁移；保持 FTS trigram/短 LIKE 语义、secure_delete、WAL checkpoint/TRUNCATE 与敏感哨兵擦除证据。程序回退不等于数据库可安全降级，不能让旧二进制直接写新 schema，不能用旧备份覆盖日常新增数据。

任务级文件、用例和验收预算以完整计划第 10 节为准。本次只做交接，上述操作均留给 Mac 接续执行。

## 一并带走的待办

| 待办 | 当前依据与下一步 |
| --- | --- |
| 自动记忆 worker 极早启动 | `src-tauri/src/memory/automatic/worker.rs` 的 `start` 仍用 `Instant::now() - 10s` 初始化，低 uptime 平台可能越界；独立修复首次立即同步与完成后 10 秒节奏，不与 schema 迁移混成一个差异 |
| 工作台租约测试时钟 | `src-tauri/src/workbench.rs` 的 `archive_guard_obeys_live_lease_full_scope_and_expiration` 仍倒退 6.001 秒；改为确定性观察时间时必须继续走真实 archive guard，不能削弱完整作用域/租约断言 |
| ignored 权限夹具 | `src-tauri/src/agent/permissions.rs` 的测试 helper 使用 `checked_sub(300s)`，低 uptime 可返回测试时钟错误；仅 ignored 原生用例使用，独立处理，不宣称生产超时已坏 |
| 长聊天仍有卡顿风险 | C16 浏览器样本 input p95 113 ms、scroll p95 436.2 ms，1000 行仍全部挂载；需要 Mac WebKit 实测，不能凭减少 commit 次数声称已流畅 |
| 尚存线性工作/锁 | R06c 关键落盘与既有结算 HTTP 仍在锁内；D20 仍加载全库一次，真实 HTTP 取消由 heartbeat 感知；后续以新证据决定是否继续拆分 |

前三项来自 B5 发布审查后续清单，交接时已与当前源码核对，尚未修复。不要把 CI 通过理解为不存在这些边界问题。

B7 要补 Mac WebKit 的 PDF worker/CSP、真实附件 IPC/媒体、Stop/权限/SSE 断流与恢复、历史及工作台所有权、隐藏/多屏/高刷新、模型完整动作、数据库恢复和真实内存/GPU/耗电。Mac CI 与 Chromium fixture 不能代替这些实机检查；Mac 通过也不能关闭 Windows 单实例/孤儿、WebView2、真实升级和 Windows 性能待验。逐项写入 [验收待办](verification-backlog.md)。

已经公开的 `v0.4.21` 标签和三个 Windows 资产不再覆盖，旧 `v0.4.20` 草稿也不处理。若未来发布 macOS 或新的修复版，先按当时授权使用新版本及完整发布门槛；当前 0.4.21 更新清单没有 Mac 目标，因此旧 Mac 应用可能显示该版本暂无适用安装包。

## 证据在哪里

仓库 [performance-evidence](performance-evidence/README.md) 包含原始 20 项审计、原基线验证、最终发布/CI/公开回读 JSON 及逐文件 SHA-256；施工全文和 `verification-backlog.md` 保存 B1–B5 结果及未验边界。原始回执内的日期、行号和路径保持原样，不冒充当前源码位置或本机测试。

大日志、完整 HTML、截图、安装包与候选资产仍仅在 Windows：`E:/Codex/工作伙伴/reports/yume-performance-*` 和 `E:/Codex/工作伙伴/release-artifacts/v0.4.21-windows-release/`。这些内容未全部上传 Git；要复核旧逐次测量时单独转移对应去敏目录。凭证、应用数据库、安装包及构建缓存不入库。继续 B6 所需任务、契约、代码与摘要已经在仓库，新测量在 Mac 生成，不沿用 Windows 的时间值作为本机基线。

## 给 Mac 上接手助手的提示

> 继续 YUME 性能整改。先读 AGENTS.md、docs/performance-remediation-handoff.md、docs/performance-remediation-plan.md 和 docs/verification-backlog.md，核对实际路径/分支/status/版本及 origin/main。B1–B5 已完成代码并随 Windows 0.4.21 发布，源码基线 fee4d707804f76e5c608ceef33c622e71d17b175；B6/B7 未开工。保留本机改动，准备固定依赖和完整工作台、记录 Mac 基线，从 H18a 开始小步修复，再按 H18b/H18c 和 M19a/b/c 推进。只用隔离数据库验证迁移，保留身份、所有权、墓碑和隐私擦除契约。各步保存证据及待验事项，不把 Mac CI 当 GUI 实测，不改公开标签/资产，不自动发布或安装到日常环境。
