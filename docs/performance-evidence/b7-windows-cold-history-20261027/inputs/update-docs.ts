import {readFileSync,writeFileSync} from 'node:fs';
const edit=(file:string,transform:(s:string)=>string)=>{const before=readFileSync(file,'utf8'),after=transform(before);if(after===before)throw Error('No update '+file);writeFileSync(file,after);};
const links='[HTML](performance-evidence/b7-windows-cold-history-20261027/index.html)、[方法/边界](performance-evidence/b7-windows-cold-history-20261027/README.md)、[结果](performance-evidence/b7-windows-cold-history-20261027/summary.json)';
const section=`## Windows 最新：冷启动历史与多窗口（2026-10-27）

继续在 \`E:\\Codex\\工作伙伴\\deskmate-windows-b7\` / \`codex/yume-performance-b6\` / \`db07e5b\` 叠加现有改动，未提交/推送。最新入口：${links}。产品 0.4.21、历史 schema 3、记忆 schema 4 和依赖未变。

已定位首次项目插件依赖初始化晚于全局健康检查，原 5 秒历史读取预算误报超时。现用独立 120 秒后台项目就绪预算，普通读取仍为 5 秒；等待不持有 Agent 操作锁，就绪按引擎 generation 缓存。还修复 WebView2 早期 IPC 的 state 未注册 panic/短暂报错、初次会话创建关闭历史视图、发现中误显示空态。最终全新身份冷启动 **66.16 秒**、热重启 **2.03 秒**自动显示两条各 1000 消息的会话，无错误或面板关闭。首次依赖安装仍慢，时间为 React 就绪后 DOM 探针测量；不是启动性能提升或 p95 声明。60 秒候选失败与早期 panic 原件保留。

复测通过后，真实 chat/workbench/settings 多窗口接管/只读、越权拒绝、目录改名同步、并发 pin+rename、归档恢复和释放通过。Sky 原生 UIA 搜索 2→1→2 通过；截图捕获超时、鼠标几何信息不可用，不能算物理 GUI/视觉通过。1000 消息 10k 全文 SHA、Stop/续聊、旧节点/焦点/草稿回归通过。

最终完整前端 **1360 pass / 0 fail / 8 skip，172 文件**；Rust **970 pass / 0 fail / 24 ignored**；三套类型、标准前端与正常 Windows debug/no-bundle 编译通过。Doctor changed scope 无新增诊断，同 7 文件前后均 81 分/12 条既有诊断，不能与前次 4 文件 83 分直接比较。25 个 SQLite 完整性通过，五个身份凭据/进程/数据根和两个端口已清理，正常 dist/EXE 恢复；未运行生产身份。旧 E 主仓状态与 C 工作树保留，先前报告原件保留（首批 README 早已有后续链接，哈希例外见复核回执）。

**B7 仍进行中**：继续物理 GUI picker/drop、媒体/二进制、审批/SSE 故障恢复、日志五工具/到期、迁移恢复和活跃工具二次启动；PDF 内存/硬件/生产对照与 Mac 专有范围保留。下方为先前记录，不覆盖原失败与当时边界。

`;
edit('docs/performance-windows-handoff.md',s=>s.replace('## Windows 最新：Stop/焦点修复（2026-10-27）',section+'## Windows 前次：Stop/焦点修复（2026-10-27）'));
edit('AGENTS.md',s=>s.replace(/已从 `db07e5b`[^\n]+/, '已从 `db07e5b` 的 `codex/yume-performance-b6` 在 **`E:\\Codex\\工作伙伴\\deskmate-windows-b7`** 独立接管；后续性能施工使用此 clone，先核验实际 HEAD/status/version。旧 E 主仓未提交改动和原 C 工作树保留。Stop/焦点修复已保留并复测；最新已修复冷启动历史误超时、宿主早期 state 竞态和初始化关闭历史面板。最终全新身份冷 66.16 秒/热 2.03 秒自动发现，无错误；首次插件安装等待仍存在。真实多窗口目录/所有权及 UIA 搜索通过，完整前端 1360 pass / 8 skip、Rust 970 pass / 24 ignored，类型与正常 Windows debug 构建通过。先读 [Windows 最新验收](docs/performance-evidence/b7-windows-cold-history-20261027/README.md) 和 [接管入口](docs/performance-windows-handoff.md)。**B7 仍进行中**：物理 GUI/视觉、审批/SSE/日志、迁移、硬件及 Mac 专有范围待验。QA 已清理，标准 dist 和正常配置 debug EXE 已恢复；未运行生产身份，本轮源码/测试/证据尚未提交或推送。下方 Mac 段保留来源状态，以本节和 Windows 接管入口最新回写为准。'));
edit('docs/verification-backlog.md',s=>s.replace('## 当前验证入口','## 当前验证入口\n\n'+section.replace('## Windows 最新：','### 2026-10-27 Windows ').trim()));
edit('docs/performance-evidence/README.md',s=>s.replace('2026-10-27 Windows 最新：','2026-10-27 Windows 最新：[冷启动历史修复 HTML](b7-windows-cold-history-20261027/index.html) · [方法与边界](b7-windows-cold-history-20261027/README.md) · [结果](b7-windows-cold-history-20261027/summary.json)。冷启动误超时、早期 state 竞态与历史面板初始化关闭已修复，最终全新身份冷 66.16 秒/热 2.03 秒通过；真实多窗口目录/所有权与 UIA 搜索通过，10k/Stop/续聊回归通过。前端 1360 pass / 8 skip，Rust 970 pass / 24 ignored；物理 GUI/视觉、迁移、硬件等边界保留，B7 进行中，未提交/推送。\n\n2026-10-27 Windows 前次：'));
edit('docs/performance-remediation-plan.md',s=>s.replace(/\*\*Windows 当前（2026-10-27）：\*\*[^\n]+/,'**Windows 当前（2026-10-27）：** 在 `E:\\Codex\\工作伙伴\\deskmate-windows-b7` / `db07e5b` 保留既有 Stop/焦点改动，已修复冷启动历史误超时、宿主早期 state 与初次会话视图竞态。最终冷 66.16 秒/热 2.03 秒、真实多窗口目录/所有权、UIA 搜索及 10k/Stop/续聊通过；前端 1360 pass / 8 skip、Rust 970 pass / 24 ignored，类型/正常构建通过。首次插件安装仍慢，物理 GUI/视觉、迁移、硬件及 Mac 专有门槛保留，B7 进行中。见 [Windows 最新报告](performance-evidence/b7-windows-cold-history-20261027/README.md) 和第 11 节，本批尚未提交/推送。').replace('2026-10-27 Windows 最新修复：','2026-10-27 Windows 最新接续：冷启动历史误超时及早期 state/视图竞态已修复，最终全新身份和热重启通过；已推进真实多窗口目录/所有权、UIA 搜索，并复测 1000 消息 10k/Stop/续聊。前端 1360 pass / 8 skip、Rust 970 pass / 24 ignored，正常构建通过，QA 清理完成。物理 GUI/视觉、完整故障恢复/迁移、硬件及 Mac 专有范围继续保留。见 [最新证据](performance-evidence/b7-windows-cold-history-20261027/README.md)。以下保留先前原始记录。\n\n2026-10-27 Windows 前次修复：'));
console.log('Updated five current handoff entries; earlier frozen evidence unchanged.');
