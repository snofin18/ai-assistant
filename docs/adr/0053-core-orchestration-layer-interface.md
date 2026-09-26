# ADR-0053　`core` 编排层的接口面与依赖白名单（「组装」下沉到 binary 层）

状态：**Accepted**（2026-09-26，人类 chat：「drift-028 的 5 点都按照你的建议做」）　日期：2026-09-26　Supersedes：—　Superseded by：—
关联：`tasks/TASK-028-core-session-context-planner-memory.md` §5 的 **DRIFT-028-1 / 028-2 / 028-3 / 028-4 / 028-5**、`crates/core/README.md`（职责段与不变量 3）、`docs/storage-design.md` §3 / §4（`memory_fts` = L1 SQLite 层）、ADR-0031（一卡一文件 / D7 命名）、ADR-0036 / ADR-0037（号段与 sub-suffix 禁用）、ADR-0039（PLAN / README 可写面）、ADR-0041（`plans/*` 可写面）、ADR-0028（`guard` 锁）

## 背景（为什么现在要决定）

TASK-028（`core`：会话管理 + 上下文管理 + Planner + Memory + 组装）的领卡前置调研（2026-09-26，PR #59）报出 **5 条 DRIFT**，全部只做了只读调研、**零产品代码**：

| DRIFT | 触发器 | 一句话 |
|---|---|---|
| 028-1 | ⑤ 超 write scope | 卡标题里的 **FTS5 检索**属**存储层**（`docs/storage-design.md` L64 / L175 把 `memory_fts` 定为 L1），而本卡 write scope 只有 `crates/core/**` |
| 028-2 | ⑩ 超预估 2 倍 | 单张 `M` 卡塞了 **5 个子系统**；可比卡（TASK-022 / 026 / 027）各自只做 1 个且都逼近 600/900 行上限 |
| 028-3 | ③ / ⑨ 公共接口 + 新抽象层 | `crates/core` 目前**零 `pub` 项**，本卡要首次落地 `SessionManager` / `ContextManager` / `Planner` / `Memory` —— 新分层组件，铁律 10 要求**契约先行** |
| 028-4 | ③ 与不变量冲突 | 卡标题含「**组装**」，但 `crates/core/README.md` 不变量 3 写死 `core → {protocol, storage, platform/api 的 trait}`，**不含** `tool-bus` / `policy` / `audit` / `task-engine`；且 `crates/core/tests/arch_layering.rs` **只拦平台实现**，不拦 workspace crate 依赖 → 真做装配会**静默违反**不变量（无门禁会红） |
| 028-5 | ⑧ 正文占位 | 卡正文是「派单前由 Orchestrator 补全」的占位；顺序上**必须**在 028-1~4 定案之后才能展开（范围未定就展开 = 把错误范围写进只读正文） |

人类 2026-09-26 裁决：**「drift-028 的 5 点都按照你的建议做」** —— 即采纳 028-1 的「立前置卡」、028-2 的「拆卡」、028-3 的「先立 ADR + spec」、028-4 的「把组装下沉到 TASK-029」、028-5 的「事后由 Orchestrator 代展开正文」。

## 决策（一句话）

**`core` = 可装配的「编排组件库」（会话 / 上下文 / Planner / Memory），本身不做装配**；它的依赖白名单写死为 `protocol` / `storage` / `platform/api`（**仅 trait**）/ `task-engine`（Plan·Step DAG 类型）/ `model-gateway`（Provider trait），**其余 workspace crate 一律禁止**；FTS5 检索归 `crates/storage`（立前置卡）；原 TASK-028 拆为「会话 + 上下文」主卡 + Planner 卡 + Memory 卡，**「组装」并入 TASK-029（binary 装配层）**。

## 决策细化

| # | 内容 |
|---|---|
| **D1** | **`core` 的定位 = 编排组件库，不是装配层**。它提供会话 / 上下文 / Planner / Memory 四组可被 binary 装配的组件；它**不 new 出** `tool-bus` / `policy` / `audit` / `hitl` / `verify` / `undo` / `lease`，也不决定「谁先调用谁」—— 那是 TASK-029 的 binary 装配职责 |
| **D2** | **依赖白名单（`crates/core/Cargo.toml` 只许出现这些 workspace crate）**：`assistant-protocol`（跨边界类型 SSOT）、`assistant-storage`（持久化**机制**）、`assistant-platform-api`（**只有 trait**）、`assistant-task-engine`（`Plan` / `PlanStep` / `PlanId` / `StepId` / `Budget` / `Reversibility` / `StepEffect` / `ready_step_ids` —— Planner 的**输出契约**，禁止在 `core` 里重新定义同义类型）、`assistant-model-gateway`（`ModelProvider` trait —— Planner 与上下文压缩的模型调用入口）。第三方依赖仍受铁律与漂移触发器 ① 约束（先登记 `docs/DEPENDENCIES.md`） |
| **D3** | **依赖黑名单（`core` 永久禁止依赖）**：`crates/platform/windows` / `macos` / `linux`（平台**实现**，铁律 7）、`assistant-tool-bus`、`assistant-policy`、`assistant-audit`、`assistant-hitl`、`assistant-verify`、`assistant-undo`、`assistant-lease`、`assistant-secrets`、`assistant-ipc`、`apps/*`、UI。理由：① **铁律 3**「策略引擎是唯一放行点」—— `core` 既不得自行判权限，也不得绕过它去拿工具；② 保持依赖 DAG 无环、**装配单点**在 binary；③ `core` 一旦依赖 `policy` / `audit`，就会把「放行点」与「编排」耦合成第二个事实源。要改这条 = 漂移触发器 ③④，**必须改本 ADR** |
| **D4** | **公开接口面 = 四组组件**：① 会话（会话生命周期 + 消息树，持久化经 `assistant-storage`，**不持有 SQLite 连接**）；② 上下文（树裁剪 / 压缩 / 预算）；③ Planner（模型输出 → 可校验的 Plan / Step DAG，**复用** `task-engine` 类型）；④ Memory（App Map 加载 + 检索）。**不重定义** `Plan` / `Step`（归 `task-engine`）、**不重定义**跨进程与持久化结构（归 `protocol` 与 `storage` 记录类型）、**不新增** `ErrorCategory`（归 `protocol`） |
| **D5** | **「组装」下沉到 TASK-029**：把卡标题与 `crates/core/README.md` 职责段里的「装配」删掉，改为「提供可装配组件」；TASK-029 的正文补上 Host 装配职责（binary 才是装配点）。`crates/core/README.md` 的**不变量 3 同步改写**为 D2/D3 的白名单口径 —— 否则不变量与实现会继续互相矛盾（028-4 的「静默漂移」） |
| **D6** | **FTS5 归 `crates/storage`（立前置卡）**：`memory_fts`（FTS5 虚表）+ 检索 API + `StorageError` 变体 + **存储侧测试**都由前置卡落地（与 `docs/storage-design.md` L64 / L175、`crates/storage/src/error.rs` 的既有预告一致）；`core` 的 Memory 卡**只消费**该 API。依赖不是障碍：`libsqlite3-sys` 的 `bundled` 构建已带 `-DSQLITE_ENABLE_FTS5`，**不需要新第三方依赖**。任何 crate 往迁移链加表都必须**顺手改 storage 侧测试**（人类 2026-09-26 已把这条上升为正式机制） |
| **D7** | **拆卡 + 号段**：原 TASK-028 收窄为「`core`：会话管理 + 上下文管理（树裁剪 / 压缩 / 预算）」；Planner 与 Memory 各立新卡；「组装」并入 TASK-029。新卡走 **200~299 治理池**（ADR-0037 D1），先例 = TASK-201 / 202 / 203 / 205（都是「阶段 1 结构件的前置 / 拆分卡」）。**sub-suffix 永久禁用**（ADR-0031 D7 / ADR-0037 D5）—— 不许用 `TASK-028a` / `028b` 这类命名 |
| **D8** | **契约先行**：先落本 ADR + `docs/spec/core-orchestration.md`（新 spec），**再**实现。`core` 的公开项、错误语义、边界与不变量以该 spec 为准；实现卡只按契约写代码，不得「先实现再补文档」（铁律 10） |
| **D9** | **授权同步（本 ADR 的实施面）**：`docs/adr/README.md` 登记 0053 + 「下一个可用编号」0053 → **0054**；`docs/memory/decisions.md` 追加一条；新建 `docs/spec/core-orchestration.md`；`plans/stage-1-pilots.md` 做**拆卡登记**（批次表 A2 的 TASK-028 行拆为 4 行、批次表 A4 的 TASK-029 行补装配职责、卡片位置表登记新卡）+ 更新「当前进度」句；新建 `tasks/TASK-206` / `TASK-207` / `TASK-208`；改 `tasks/TASK-028`（正文收窄 + §5 追加裁决行）与 `tasks/TASK-029`（正文补装配）；改 `crates/core/README.md` 与 `crates/core/src/lib.rs` 的职责 / 不变量 3；`LEDGER.md` / `docs/PARKING_LOT.md` / `docs/memory/facts.md` / `PLAN.md` 当前状态块 / `README.md` 三处 / `MEMORY.md` 规模表。**排期与各点内容不改**（人类 2026-09-26 的长期约束 + ADR-0041）：只做「新增行 + 拆卡登记 + 完成状态 + 当前进度块」 |

## 考虑过的选项（至少 2 个，含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | 不立 ADR，沿用 TASK-026 / 027 的先例（crate 内 API 自行定义） | ❌ 否决 | 那两张卡引入的是**单一 crate 内**的 API；本卡引入的是**新分层组件**（`core` 首次成型，TASK-029 与后续卡都消费），且**与既有不变量 3 直接冲突** → 属铁律 10 的「改分层之前先有 ADR + spec」 |
| 2 | **立 ADR-0053 + spec + 拆卡 + 组装下沉（本 ADR）** | ✅ **采纳** | 一次把 5 条 DRIFT 全部收口；契约先落地，实现卡才有可依赖的边界 |
| 3 | 把 TASK-028 的 write scope 扩成 `crates/core/**` + `crates/storage/**`（028-1 的选项 2） | ❌ 否决 | 破坏「一卡一个 write scope / 并行不重叠」；且 `docs/storage-design.md` 已把 `memory_fts` 定为 L1 存储层 —— 扩 scope 等于把分层矛盾固化 |
| 4 | 让 `core` 直接依赖 `tool-bus` / `policy` / `audit`，把 `core` 当装配层 | ❌ 否决 | 与铁律 3 冲突（放行点必须唯一且不被编排层包裹）；且 `crates/core/tests/arch_layering.rs` **拦不住**这种依赖，等于制造静默漂移 |
| 5 | 用 `TASK-028a` / `028b` 这类 sub-suffix 拆卡 | ❌ 否决 | ADR-0031 D7 + ADR-0037 D5 **永久禁用**，`xtask card-check` 判据 ⑤ 会直接报 Error |

## 影响（需要改的文档）

- 新建：`docs/adr/0053-core-orchestration-layer-interface.md`（本文件）、`docs/spec/core-orchestration.md`、`tasks/TASK-206-*.md`（storage 前置卡）、`tasks/TASK-207-*.md`（Planner）、`tasks/TASK-208-*.md`（Memory）
- 改：`tasks/TASK-028-*.md`（正文收窄 + §5 裁决行）、`tasks/TASK-029-*.md`（正文补 Host 装配）、`crates/core/README.md`、`crates/core/src/lib.rs`（模块文档）、`plans/stage-1-pilots.md`、`docs/adr/README.md`、`docs/memory/decisions.md`、`docs/memory/facts.md`、`docs/PARKING_LOT.md`、`LEDGER.md`、`PLAN.md`（仅「当前状态」块 4 行）、`README.md`（仅三处）、`MEMORY.md`（仅规模表）
- **不改**：任何 `crates/**` 产品代码（本 ADR 只定契约，不实现）、`protocol/**` schema、`.github/workflows/**`、`AGENTS.md`

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 白名单过窄，Planner 真的需要 `tool-bus` 的工具目录 | 用 `protocol::ToolSchema` 作**参数**传入（D4：不依赖实现）；真需要时改本 ADR（漂移触发器 ③） |
| 「组装下沉」被误读成「`core` 不参与启动」 | D1 明确：binary **装配** `core` 的组件；`core` 只是不 new 出别家的实现 |
| 拆卡被误读成「改了排期」 | D9 + ADR-0041：只做新增行 / 拆卡登记 / 完成状态 / 当前进度块；原有排期与各点内容一字不动 |
| storage 前置卡与 `core` 卡并行时 write scope 重叠 | 前置卡只碰 `crates/storage/**`；Memory 卡只碰 `crates/core/**`；两卡依赖顺序由计划登记 |
| `memory_fts` 迁移链加表时忘了改 storage 侧测试 | D6 + 既有机制（人类 2026-09-26 已把「任何 crate 往迁移链加表都得顺手改 storage 测试」升为正式机制）；`check-migrations` 仍是机器门禁 |

## 验证方式（怎么知道这个决策是对的；何时应重新评估）

1. `crates/core/Cargo.toml` 的 `[dependencies]` 只出现 D2 白名单里的 workspace crate（黑名单命中 = 违反本 ADR）；
2. `plans/stage-1-pilots.md` 里 TASK-028 的批次表行已拆为 4 行，且每个新卡号都有对应 `tasks/TASK-NNN-*.md`（`xtask card-check` 判据 ② 机器校验）；
3. `docs/spec/core-orchestration.md` 存在，且其「不变量」与 `crates/core/README.md` 的不变量 3 **口径一致**（无第二份事实源）；
4. `cargo test -p assistant-core arch::` 仍全绿（本 ADR 不改 arch 断言本身，只让 README 与它对齐）；
5. **何时重新评估**：Memory 或 Planner 的实现真的需要黑名单里的 crate；或 `core` 的公开面被 TASK-029 之外的第二个装配点消费。

## 相关 ADR

- ADR-0031（一卡一文件 / D7 命名）、ADR-0036 / ADR-0037（号段与 sub-suffix 禁用）
- ADR-0039（PLAN / README 可写面）、ADR-0041（`plans/*` 可写面）
- ADR-0028（公共热点文件的 `guard` 锁）
- ADR-0021（记忆分层：本 ADR 的落地要追加 `docs/memory/decisions.md` 与 `docs/memory/facts.md`）
