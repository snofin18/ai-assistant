# AGENTS.md — Agent 工作契约（精简版）

> 所有 AI coding agent（Codex / opencode / Claude Code）**每次会话必读的唯一入口**。刻意保持精简：只写"每次都必须知道"的，细节一律链接。
> **裁决顺序**：本文件铁律 > `docs/adr/` > `docs/spec/` > `PLAN.md` > 任务卡 > 代码现状 > **聊天讨论**。
> **聊天记录不是事实源**：讨论中达成但未落进 ADR/spec/任务卡的决定，视为未发生。

---

## 1. 项目与文档地图

跨 Windows/macOS/Linux 的本地 AI 助理：大模型负责理解与规划，通过**注册的工具**操作已绑定的 Windows 常用应用（记事本、画图、Edge、Excel、Photoshop…），全过程**受控、可审计、可撤销**。
栈：Rust（核心）+ Tauri 2 + React/TS + MCP(`rmcp`) + SQLite + Tokio。　当前阶段见 `PLAN.md`。

| 你要做什么 | 读什么（**只读需要的，不要全读**） |
|---|---|
| 开始任何工作 | 本文件 → `PLAN.md`（索引，≤60 行）→ `plans/<当前阶段>.md`（**阶段索引与批次表**）→ 你的 `tasks/TASK-NNN-<slug>.md`（**全文**：正文 + 执行记录）→ `LEDGER.md` 末 10 行 |
| 回忆"项目已知什么/否决过什么/踩过什么坑" | `MEMORY.md`（**L0 索引，≤150 行，含路由表**）→ 按路由跳读 `docs/memory/*` |
| **动手前防止重复辩论**（★ 必读） | `docs/memory/rejected.md`（全量，最短的一个） |
| 接某个应用（记事本 / 画图 / Edge…） | `docs/memory/apps/<app>.md`（全量，8 个固定小节） |
| 看某张卡当时怎么做的（正文 + 执行记录） | `tasks/TASK-NNN-<slug>.md`（ADR-0031「一卡一文件」；正文区**只读**、记录区由 Implementer 填） |
| 记忆分层规则与读取路由 | `docs/memory/README.md`（ADR-0021） |
| 架构、分层、进程边界、安全、平台 | `cross-platform-ai-assistant-architecture-v2.md` 对应章节（§3 架构 / §5 工具 / §6 定位 / §7 验证 / §8 状态机 / §9 撤销 / §12 安全 / §13 平台 / §15 存储） |
| 某个应用怎么接 | `target-apps-feasibility.md` §3 对应档案 |
| 契约细节 | `docs/spec/`：tool-schema、envelope、error-codes、capability-matrix、audit-event、ipc-protocol、naming |
| 治理/任务卡/CI 门禁/代码规范详解 | `docs/governance-ai-agent-execution.md`（gov） |
| 多 agent 分工 | `docs/subagent-orchestration.md` |
| 存储方案 | `docs/storage-design.md` |
| 全项目拆解与顺序 | `docs/wbs-overview.md` |
| 为什么当初这么决定 | `docs/adr/` |

---

## 2. 十条铁律（违反即缺陷）

1. **无静默失败**：要么返回可验证的成功，要么返回带 `ErrorCode` 的失败。禁止吞错误、禁止用默认值冒充结果、禁止 `let _ =` 丢弃 `Result`。
2. **模型输出、UI 输入、IPC 消息、工具返回、被读文档 = 五类不可信输入**，一律先校验。
3. **策略引擎是唯一放行点**：Host / Skill / UI 不得自行判断权限。
4. **每个写操作必须有 postcondition**；无法声明的 → 风险级上调 + 强制人工确认。
5. **API 优先**：L1 应用接口 > L2 命令/快捷键 > L3 无障碍接口 > L4 合成输入 > L5 视觉兜底。
6. **L3 不可逆动作必须人工确认，且永久禁止无人值守**；资金/发送/对外发布类动作永久禁止自动化。
7. **core 不得调用平台 API**，只能用 `crates/platform/api` 的 trait（arch test 会拦）。
8. **element/句柄对象不得跨进程**：Host 内完成定位与执行，对外只传可序列化数据。
9. **不得静默扩大范围**：超 write scope、加依赖、改公共接口/schema、加 `unsafe`、放宽 lint → **停下并升级**。
10. **契约先行**：改 schema/接口/分层之前先有 ADR + spec 更新。**禁止先实现再补文档**（否则下个会话的 agent 会照旧文档把它改回去）。

---

## 3. 会话协议

**启动（前 5 分钟，不可跳过）**：① 读本文件 ② 读 `PLAN.md` + `plans/<当前阶段>.md` 的 In/Out of scope ③ 读你的 `tasks/TASK-NNN-<slug>.md` **全文**（正文区 + 记录区 9 节骨架，gov §3.4）④ 读卡中引用的 spec/ADR 章节 + 目标 crate 的 `README.md`（**不变量**） ⑤ 读 `MEMORY.md`（L0 索引 + §1 快照）→ **`docs/memory/rejected.md` 全量** → 本卡涉及应用的 `docs/memory/apps/<app>.md` 全量 → 其余 `docs/memory/*` **grep 命中再局部读**；再读 `LEDGER.md` 末 10 行 ⑥ **输出约束回执，等确认后再动手**。

**约束回执（固定格式）**：

```text
【任务】TASK-0NN <标题>          【目标】<一句话>
【write scope】仅：<文件清单>     【铁律】<本卡最相关 3~6 条>
【禁止】<本卡 Out of scope 要点>  【验收】<命令> → <期望>
【依赖】<前置卡号，已核对 LEDGER>  【疑问】<有则列出+你的默认处理；无则写"无">
```

回执与任务卡不符 → 上下文已污染 → **请人类重开会话**（比纠正更省成本）。

**会话中重锚**：每完成一个子步骤自问 ①在 In scope 内吗 ②是否引入了卡里没提的文件/依赖/抽象 ③是否改了公共接口；每 20~30 轮重读本文件与任务卡；**一个会话最多完成 1~2 张卡**，做完即提交 + 更新 `LEDGER.md` + 结束会话。
**流程**：领卡 → 回执 → 小步实现 → 自跑全部验收命令 → 填执行记录 → 更新 LEDGER（+ 有新事实/坑则追加 `docs/memory/facts.md`｜`docs/memory/pitfalls.md`｜`docs/memory/rejected.md`；**应用专属的进 `docs/memory/apps/<app>.md`**；`MEMORY.md` 只在快照/规模表变化时才改）→ PR（gov §9.4 模板）→ 独立 review agent → 人类合并。
**禁止 drive-by refactor**：不相关的问题一行记入 `docs/PARKING_LOT.md`，本卡不动。
**改公共热点文件先取锁（ADR-0028）**：写 `LEDGER.md` / `docs/memory/*` / `docs/PARKING_LOT.md` /
`PLAN.md` / `README.md` / `plans/*` 之前必须（后三者自 **ADR-0039** 起也在名单内 —— 它们每张卡 Done 都会被写）
`cargo run -p xtask -- guard acquire <文件> --owner <会话级唯一标识> --task TASK-0NN --intent "<一句话>"`，
写完**立刻** `guard release`。`--owner` 必须会话级唯一（`<agent>-<thread 前 8 位>` 或 `TASK-NNN`）——
两个会话用同一 owner 会被判为同一持有者而直接复用锁，那等于没锁。
超时放弃（退出码 **5**）之后：① **必须在 `LEDGER.md` 追加一行**说明放弃了什么、为什么（缺这一行就只是「日志」不是「通报」）；
② **不得**改用 `--force` 硬抢（会直接踩掉别人的改动）；③ 改做别的卡或升级给人类。
并行分工的完整锁协议见 `docs/subagent-orchestration.md` §11。

---

## 4. 何时必须停下问人（漂移触发器）

命中任一 → **停止编码** → 在任务卡写 `DRIFT-<卡号>-<序号>`（现象/影响/建议/已停工作，格式见 gov §4.3）→ LEDGER 记一行 → 等裁决：

① 加第三方依赖　② 加 crate/顶层目录/模块　③ 改公共接口（trait、IPC 方法、Tool schema、DB schema、ErrorCode）　④ 改动 ADR 已决事项　⑤ 超出 write scope　⑥ 放宽 lint / 加 `#[allow]` / 加 `unsafe`　⑦ 需改测试断言才能通过（**默认视为缺陷**）　⑧ 发现 spec 自相矛盾或与代码矛盾　⑨ 想加新抽象层　⑩ 工作量超预估 2 倍　⑪ 需要真实网络/凭据/商业应用（超出靶机范围）　⑫ 删改既有公共 API

**不确定就问，不要猜。** 猜错的代价远高于问一句。

---

## 5. 代码规范速查

### 5.1 命名（要求"一眼可懂"）
- **禁缩写**（除通用：`id/url/ui/os/db/ipc/mcp/uia/ax/atspi/cdp/dpi`）、禁拼音、禁单字母（循环变量除外）、禁 `data/info/temp/manager2` 这类无信息名。
- 函数 = **动词+宾语**：`resolve_target_descriptor()`、`verify_postconditions()`、`acquire_target_lease()`。
- 布尔量带 `is_/has_/can_/should_`；枚举变体用完整词：`TaskStatus::AwaitingApproval`（不是 `Wait`）；错误变体说原因：`TargetNotFound`、`PolicyDenied`（不是 `Error1`）。
- **受控词汇表**（同一概念全项目只用一个词，禁止同义混用）：`Target`(不叫 Element/Object/Item)、`Step`(不叫 Action/Operation)、`Tool`(模型可见能力) vs `Skill`(可分发单元)、`Adapter`(应用适配包)、`Lease`(目标租约)、`Anchor`(撤销锚点)、`Fingerprint`(状态指纹)。完整表见 `docs/spec/naming.md`。
- 工具名 `<app>.<domain>.<action>`；常量 `SCREAMING_SNAKE`；配置键 `kebab-case`；测试 `test_<unit>_<condition>_<expected>`；分支 `task/TASK-0NN-<slug>`。

### 5.2 注释（要求"密度偏高"，但仍以 why 为主）
| 位置 | 要求 |
|---|---|
| 模块/文件头 | **必须**：职责、边界（不做什么）、不变量、典型用法、相关 spec 章节 |
| 公共 API | **必须**：语义、参数、返回、**错误语义（何时返回哪个 ErrorCode）**、副作用、是否幂等、超时与取消行为 |
| 关键私有函数 | why + 非显然的 what |
| 复杂算法/状态机 | 分步注释 + 一个具体示例 |
| `unsafe` / FFI | **必须** `// SAFETY:` 说明（仅允许在 `crates/platform/*`） |
| 应用坑 | 结构化标签：`// PITFALL(app=excel): COM 修改会清空 undo 栈，故此处强制快照` → 会被工具汇总进 `docs/memory/apps/<app>.md` §6（跨应用的进 `docs/memory/pitfalls.md`） |
| 临时方案 | `// TODO(TASK-0NN):` / `// STUB(TASK-0NN):`，**无卡号即 CI 失败** |
- 目标密度：公共 API 100% 有文档注释；每 20~40 行有效代码至少一条解释性注释。
- **禁止**：注释掉的代码、无信息注释（`// 把 x 设为 1`）、与代码矛盾的过期注释（改函数必须同步改注释）。

### 5.3 Rust
Edition 2024；crate 顶层 `#![deny(clippy::unwrap_used, expect_used, panic, todo, dbg_macro, print_stdout, print_stderr, indexing_slicing)]` + `#![warn(clippy::pedantic, missing_docs)]`（`tests/` 内可 allow）；库用 `thiserror` 定义领域错误枚举、二进制才用 `anyhow`，对外错误必带 `ErrorCode`；时钟/随机/UUID/FS/网络一律 trait 注入（保证可回放）；跨进程与持久化结构只放 `crates/protocol`（由 schema 生成，**禁止各处手写重复结构体**）；单文件 ≤400 行（软）/600（硬）**[写作规范，CI 门禆见 gov §5.4 = 600/900（ADR-0033）]**，函数 ≤80 行，参数 ≤6 个。

### 5.4 TypeScript
`strict` + `noUncheckedIndexedAccess` + `exactOptionalPropertyTypes`；禁 `any`、禁非空断言、禁 `console.*`；**类型全部由 `protocol/` schema 生成**，IPC 返回用 zod 运行时校验；UI 层禁 `fetch`/`WebSocket`（出网一律走 Core）；业务逻辑不写在组件里，组件 ≤200 行；文案全走 i18n key。

### 5.5 测试
单元（纯逻辑，**禁真实 IO/网络**）／契约（schema、类型同步、ErrorCode 完整性）／回放（录制的 UI 树快照跑端到端逻辑）／靶机（`fixtures/apps/*`）／真机（仅手工验收与阶段末）／**安全回归（注入靶页常驻 CI）**。覆盖率：workspace ≥75%，`core`/`policy`/`task-engine` ≥85%。

---


## 11. 进度同步规则（每个 commit / push 之后必做）

> **强制**：用户已在 chat 中明示此要求。每次完成任何工作并 commit / push 后**必须**按本节同步进度文件，遗漏即算漂移。

### 11.1 必更新的文件

| 文件 | 何时更新 | 谁写 |
|---|---|---|
| `LEDGER.md` | 每次 commit 之后（含 worktree 内的 fix） | agent（必追加，不改写） |
| `PLAN.md` 的「当前状态」块 | **每张卡 Done**（不再只是阶段切换） | Implementer 可代 Orchestrator 写（**ADR-0039 D4**；**只许改那 4 行**） |
| `README.md` 状态行 + `## 当前阶段` + `## 最近进展` | **每张卡 Done** | agent（**仅这三处**，其余章节只读 —— ADR-0039 D5） |
| `MEMORY.md` §1 快照 | 阶段切换 / 阶段索引变化 / 规模表变化 | agent（必更新） |
| `docs/memory/pitfalls.md` | 发现新坑（含 v4 那种 supersede 多个旧 entry） | agent（必 supersede 旧 entry） |
| `docs/memory/facts.md` | 验证过的硬事实 | agent（必追加） |
| `docs/memory/rejected.md` | 否决方案 | agent（必追加） |
| `docs/memory/apps/<app>.md` | 某应用的版本/UIA 形状/坑变化 | agent（必追加） |
| `PLAN.md` 的其余段落（阶段索引 / 范围冻结提示 / 关联文件 / 变更历史） | 阶段索引或任务队列变化 | **Orchestrator-only**（agent 不可改 —— ADR-0039 D4） |

### 11.2 必做的次序

1. `git status` + `git diff --stat HEAD` 确认变更范围
2. 更新 `PLAN.md` 的「当前状态」块 **4 行**（**每张卡 Done 都要**，日期必须改 —— ADR-0039 D1 / D2）
3. 更新 `README.md` 的**三处**（状态行 + `## 当前阶段` + `## 最近进展`；无阶段变化也要追加「最近进展」—— ADR-0039 D1 / D2）
4. 更新 `LEDGER.md`（一行一事件，与 gov §9.2 模板一致）
5. 若 `MEMORY.md` 规模表变化 → 同步该文件 + `cargo run -p xtask -- memory-counts` 验证
6. 跑 `cargo run -p xtask -- hygiene / card-check / docscan` 三项全绿（TASK-015 起加 `check-ledger` —— 它机器校验第 2/3 步的**新鲜度**）
7. `git add` + `git commit -m "docs(memory): ..."` + `git push -u origin <branch>`

### 11.3 进度文件 fail 信号（漂移触发器 ⑤ 备查）

- ❌ commit 后没更新 LEDGER → 漂移（违反会话协议）
- ❌ 卡 Done 但没改 `PLAN.md` / `README.md` → 漂移（**ADR-0039 D1 / D2**；这正是 DRIFT-202-2 的形态）
- ❌ 改了 README 三处（状态行 / `## 当前阶段` / `## 最近进展`）以外的章节 → 漂移（ADR-0039 D5）
- ❌ 改了 `PLAN.md`「当前状态」块以外的段落 → 漂移（ADR-0039 D4）
- ❌ memory-counts 失败时把表数字写错 → 漂移

## 6. 验证命令（提交前全绿，输出粘进 PR）

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-core arch::            # 依赖方向（gov §5.3）
cargo run -p xtask -- verify-schemas           # Tool/Adapter/审计事件 schema
cargo run -p xtask -- codegen --check          # Rust↔TS 类型与 schema 同步
cargo run -p xtask -- hygiene                  # 仓库卫生（gov §5.4）
cargo run -p xtask -- memory-counts            # MEMORY.md 规模表 ↔ docs/memory/ 实测（ADR-0030，硬门禁 #12b）
cargo run -p xtask -- adr-index                # ADR 登记表 ↔ docs/adr/*.md ↔ docs/memory/decisions.md（ADR-0030，硬门禁 #12b）
cargo run -p xtask -- check-comments           # 命名/注释/PITFALL-TODO 卡号规范
cargo deny check
cargo llvm-cov --fail-under-lines 75
pnpm lint; pnpm typecheck; pnpm test           # UI
cargo run -p xtask -- replay --suite core      # 回放基准
```

> 阶段 0 仓库尚未脚手架化，上述命令自 TASK-001（仓库骨架）起生效。**阶段 0 的产出是 `SPIKE_REPORT.md`，不是产品代码。**

> **`memory-counts` / `adr-index` 已经生效**（不等 TASK-001）：它们只查文档，阶段 0 就有检查对象。
> 两个子命令都**只读** —— 报错时消息里直接给出「可粘贴的正确值」，照着改文档即可，工具不会替你写。
> **禁止**为了让它们变绿而放宽判据或删表；判据改动属漂移触发器 ④（改 ADR 已决事项）。

---

## 7. 禁止事项

❌ 注册 `execute_code` / 通用 `run_shell` / 通用 `run_powershell`　❌ 静默覆盖文件、不走回收站的删除　❌ `DisplayAlerts=false` 等"跳过应用确认"的属性　❌ 从用户浏览器 profile 复制 Cookie/凭据　❌ 自动化股票交易类资金操作、绕过验证码或风控　❌ 密钥明文入库/入日志/入 prompt（必须走 OS keychain）　❌ 在 core/policy/task-engine 里调平台 API 或 `std::process::Command`　❌ 用控件可见文本作主 selector　❌ 全局默认的 `Ctrl+Z`（必须由 Adapter 显式声明；终端类目标禁用）　❌ 依赖 X11 会话（Linux 已 Wayland-first）　❌ 为通过测试而改断言　❌ 顺手重构/顺手优化/顺手升级依赖　❌ 硬编码密钥、内网地址、真实业务数据（本项目未来要开源）

---

## 8. 文件权限（write scope）

| 文件 | 你（Implementer）的权限 |
|---|---|
| `tasks/TASK-NNN-<slug>.md`（**仅本卡号那一个文件**） | **记录区**（分界线以下的 9 节）可写；**正文区**（分界线以上：In/Out scope、必须遵守、验收命令、DoD）**只读**（ADR-0031 D3/D5） |
| `LEDGER.md`、`docs/PARKING_LOT.md`、`docs/memory/{facts,pitfalls,rejected,decisions,open}.md`、本卡涉及应用的 `docs/memory/apps/<app>.md` | **可追加**（不改写他人条目；更正用 `[supersedes:日期]`） |
| 任务卡列出的 write scope 内文件、相关 crate `README.md` | 可改 |
| `AGENTS.md`、`PLAN.md`、`plans/*`、`docs/spec/*`、`docs/adr/*`、`MEMORY.md`（L0 由 Orchestrator 维护）、其他 crate、**别的应用的 `apps/<app>.md`** | **只读**（要改 → 提案 → DRIFT/ADR）。**唯一例外**：`PLAN.md` 的「当前状态」块 4 行 —— 卡 Done 时 Implementer 可代写（**ADR-0039 D4**） |

多 agent 并行时 **write scope 必须互不重叠**；并行度 ≤ 3（**人类审阅速度决定项目速度**）。

**write scope 与 `guard` 锁的关系（ADR-0028）**：scope 是**文档约定**（事前划分），锁是**运行时机制**（事中互斥），两者拦的是不同的失效模式，**都要**。上表第 2 行那类「所有会话都可追加」的公共热点文件**天然无法用 scope 划分**，只能靠锁；而「读文件 → 想 → 写回」之间没有任何原子性，两个进程各自基于旧内容写回就是经典 lost update，**git 不会报冲突**（两次写都「成功」了）。反过来，锁是协作式的、**无法技术强制**，所以它不能替代 scope。

---

## 9. 交付格式（每次任务结束）

```text
## TASK-0NN 完成报告
改动文件：<清单，逐个核对是否在 write scope 内>
验收：<命令> → <结果>（全绿/失败项）
DoD 核对：<逐条打勾>
偏差：none / DRIFT-0NN-x（附处理结果）
文档同步：README / spec / LEDGER / MEMORY / PARKING_LOT
新增长期记忆：<FACT/PITFALL/REJECTED 条目，若无写"无">
遗留与建议：<下一张卡要注意什么>
给审阅者的关注点：<风险最高的 1~3 处>
```

---

*本文件变更需 ADR。发现本文件与其他文档矛盾 → 按裁决顺序处理并记 DRIFT。*

*本版变更的 ADR 授权：§3「改公共热点文件先取锁」与 §8「锁与 scope 的关系」← **ADR-0028**（其影响表明写「`AGENTS.md` §3 / §8 … ✅ 本 ADR 即授权」）；§6 新增的两条命令 ← **ADR-0030** D5（CI 硬门禁 #12b，gov §5.1）。*
