# TASK-028　`core`：会话管理 + 上下文管理（树裁剪 / 压缩 / 预算）

- 状态：**Ready**
- 阶段：1　子阶段：**1a**　批次：**A2**（原 TASK-028 拆卡后的**主卡**）　依赖：020~027（✅ Done）　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`；契约见 `docs/spec/core-orchestration.md` 与 `docs/adr/0053-core-orchestration-layer-interface.md`。
- 本卡与 TASK-207 / TASK-208 **共用 `crates/core/**`** → **必须严格串行**（依赖列已保证顺序），不得与它们并行。
- 来源：原 TASK-028 的 5 条 DRIFT 裁决（人类 2026-09-26：「drift-028 的 5 点都按照你的建议做」）→ 本卡收窄为「会话 + 上下文」；Planner → **TASK-207**；Memory → **TASK-208**；「组装」→ **TASK-029**（ADR-0053 D7）。
- 本卡原文件名 `tasks/TASK-028-core-session-context-planner-memory.md`；2026-09-26 拆卡时改名为 `tasks/TASK-028-core-session-context.md`（**号不变**，ADR-0031 D7）。

---

## 目标（一句话）

在 `crates/core` 落地**会话管理**与**上下文管理**：会话生命周期 + 消息树；树裁剪 / 压缩 / 预算 —— 且**每一次「少给了东西」都必须在返回值里显式标注**（铁律 1）。

## 背景（为什么现在做）

| # | 事实 | 证据 |
|---|---|---|
| 1 | 原 TASK-028 一张 `M` 卡塞了 5 个子系统（会话 / 上下文 / Planner / Memory / 组装）→ 触发器 ⑩ | `tasks/TASK-028-core-session-context.md` §5 的 **DRIFT-028-2** |
| 2 | `core` 首次落地公开接口面 → 必须**契约先行**（铁律 10） | **ADR-0053**（D4 / D8）+ `docs/spec/core-orchestration.md` §3 / §4 |
| 3 | 上下文管理的形状有架构级定义 | 架构 v2 §11.3（上下文管理）、§7.1 / §7.2（树裁剪 / 预算压缩） |
| 4 | 持久化**不**在本卡：`core` 不持有 SQLite 连接 | `crates/core/README.md` 边界 + `docs/spec/core-orchestration.md` 不变量 5 |
| 5 | 阶段 1 的验收要点含「上下文预算生效」 | `plans/stage-1-pilots.md` 批次表 A2 的 TASK-028 行验收要点 |

## write scope

- `crates/core/src/**`（**会话 / 上下文相关模块** + `lib.rs` 的 re-export；新增模块属漂移触发器 ②，本卡的存在即 ADR-0053 的授权）
- `crates/core/tests/**`（**新增**测试；不得改既有断言 —— 漂移触发器 ⑦）
- `crates/core/Cargo.toml`（**仅当**需要白名单内的依赖；白名单见 ADR-0053 D2）
- `crates/core/README.md`（仅当「职责 / 边界 / 不变量 / 已知限制」因本卡需要同步）
- `tasks/TASK-028-core-session-context.md`（本文件）
- `LEDGER.md` / `PLAN.md`（仅「当前状态」块 4 行）/ `README.md`（仅三处）/ `plans/stage-1-pilots.md`（仅完成状态与「当前进度」块）/ `MEMORY.md`（仅规模表）：AGENTS.md §11.1 强制的进度同步

## Out of scope（写了就停）

- **不做** Planner（归 **TASK-207**）、**不做** Memory / App Map / 检索（归 **TASK-208** / **TASK-206**）
- **不做**装配：把 platform / tool-bus / policy / storage / audit 组装成 Host 归 **TASK-029**（ADR-0053 D1 / D5）
- **不定义**跨进程 / 持久化结构（归 `assistant-protocol` / `assistant-storage`）；**不定义** `Plan` / `Step`（归 `assistant-task-engine`）
- **不判权限**、不执行工具、不调平台 API（铁律 3 / 7）
- **不引**黑名单 crate（ADR-0053 D3）：`tool-bus` / `policy` / `audit` / `hitl` / `verify` / `undo` / `lease` / `secrets` / `ipc` / `platform/{windows,macos,linux}` / `apps/*`
- **不做**向量检索、不做无人值守相关能力（阶段 1 Out of scope）
- **不放宽**任何 lint、不加 `#[allow]`、不加 `unsafe`、不改 `protocol` schema / `ErrorCode`

## 必须遵守

- **无静默失败**（铁律 1）：裁剪 / 压缩的每一次丢弃都必须**显式标注**（丢了什么、为什么、可否认）；预算不足时 fail-closed 或显式要求调用方决定，**不**静默截断到「看起来成功」
- **五类不可信输入**（铁律 2）：模型输出与工具返回进入上下文时**先校验**，校验失败带 `ErrorCode` 返回
- **不持有连接**：持久化只经 `assistant-storage` 的公开 API（`docs/spec/core-orchestration.md` 不变量 5）
- **可注入**：时钟 / 随机 / UUID / FS 一律 trait 注入（AGENTS.md §5.3，保证可回放）
- **可装配**：组件由 binary 构造 + 注入；**不**依赖全局单例、**不**在 `core` 内 new 出别家的实现（`docs/spec/core-orchestration.md` 不变量 10）
- **文档注释**：公共 API 100% 有文档注释（含**错误语义**与幂等性）
- **单文件 ≤ 600 行**（硬限 900，ADR-0033）；函数 ≤ 80 行、参数 ≤ 6 个
- **只追加文件**（`LEDGER.md` / `docs/PARKING_LOT.md`）**不改写既有行**

## 验收命令

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-core
cargo test -p assistant-core arch::
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- check-ledger
cargo deny check
```

## DoD

- [ ] 会话：创建 / 恢复 / 结束 + 消息树的增删与顺序，全部可被测试用例覆盖；非法迁移（如结束后再追加）fail-closed
- [ ] 上下文预算**生效**：给定预算下产出的片段不超预算；**超预算时必须显式标注被丢弃的内容与理由**（负向用例）
- [ ] 树裁剪：可裁剪 / 不可裁剪（如锚点、未完成步骤的证据）两类行为都有用例，且裁剪结果**可审计**（带理由）
- [ ] 压缩：压缩失败 / 模型不可用 → 带 `ErrorCode` 失败，**不**退化成「静默丢历史」
- [ ] `core` 的 `[dependencies]` ⊆ ADR-0053 D2 白名单；`cargo test -p assistant-core arch::` 全绿
- [ ] `core` 单元测试**零真实 IO / 网络 / 时钟**（可回放）
- [ ] `core` 行覆盖 ≥ 85%（阶段 1 DoD）
- [ ] 既有测试零改动通过（漂移触发器 ⑦）
- [ ] 上列 14 条验收命令全绿；`hygiene` / `card-check` / `docscan` 的 warning **不新增**
- [ ] §11.1 进度同步：`PLAN.md` 当前状态块 / `README.md` 三处 / `LEDGER.md` / `plans/stage-1-pilots.md`（仅完成状态与「当前进度」块）/ `MEMORY.md` 规模表
- [ ] 若产生新 FACT / PITFALL → 追加 `docs/memory/{facts,pitfalls}.md`

---

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-028　core：会话管理 + 上下文管理（树裁剪/压缩/预算）+ Planner + Memory(App Map 加载/FTS5 检索) + 组装
【目标】在 crates/core 落地编排层五件套（会话 / 上下文 / Planner / Memory / 组装）
【write scope】仅：crates/core/**（完整列表见 plans/stage-1-pilots.md 批次表 A2）
【铁律】1（无静默失败）· 5（API 优先）· 7（core 不得调平台 API，只用 platform/api 的 trait）· 9（不得静默扩大范围）· 10（契约先行）
【禁止】stage-1 Out of scope：向量检索记忆 · 无人值守执行 · 外部 MCP server 加载 · macOS/Linux 代码 · 多 agent 并行执行任务（plans/stage-1-pilots.md §Out of scope）
【验收】cargo fmt --all --check 0 diff；cargo clippy --all-targets -- -D warnings exit 0；cargo test --workspace 全绿；
        cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check 全 PASSED
【依赖】020~027 —— 已核对 PLAN.md「当前状态」列（011~027 全 ✅）＋ LEDGER.md 末 10 行：**全部 Done**
【疑问】5 条（DRIFT-028-1 ~ 5，见 §5）；其中 4 条命中 AGENTS §4 漂移触发器（③⑤⑨⑩），1 条是本卡正文尚未展开
```

> **结论：本卡未开工。** 按 AGENTS §4「命中任一 → **停止编码** → 写 DRIFT → 等裁决」，本会话只做**只读调研 + 回执 + 偏差登记**，未改任何产品代码。

> **2026-09-26 拆卡后**：本卡范围已收窄为「会话 + 上下文」（见正文）；本节回执是**拆卡前**（Blocked 那次）的记录，按 ADR-0031「记录区只追加」保留原样。

**2026-09-26 实现轮次回执（拆卡后）**

```text
【任务】TASK-028　core：会话生命周期 / 消息树 / 上下文裁剪 / 压缩 / token 预算
【目标】让 TASK-029 装配一个已校验、可回放、预算内且无静默丢失的 Core 组件集。
【write scope】仅：crates/core/**（Cargo.toml、src、tests、README）＋本卡记录区＋§11.1 进度同步文件。
【铁律】1 无静默失败；2 不可信输入先校验；9 不得静默扩大范围；10 契约先行。
【禁止】Planner / Memory / FTS5 / Host 装配 / 权限判定 / 平台调用 / 黑名单依赖。
【验收】本文件正文 14 条命令 → 全绿；assistant-core 行覆盖 92.62% → PASS（门槛 85%）。
【依赖】ADR-0053 与 docs/spec/core-orchestration.md 已在 main；020~027 Done；TASK-206 的 DRIFT-206-1 不阻塞本卡。
【疑问】真实 conversations storage adapter 当前不存在；默认处理为 SessionStore 注入 trait + 内存实现，生产 adapter 留 TASK-029 前另案，见 §5 DRIFT-028-6。
```

### 2. 实际改动文件

（本会话未进入实现 —— 见 §5。本卡记录区之外**零改动**。）

**2026-09-26 实现轮次追加**

- `crates/core/Cargo.toml`：仅新增白名单内 workspace 依赖 `assistant-protocol`，复用稳定 `ErrorCode`。
- `crates/core/src/{lib,error,identifiers,message,session,store,context}.rs`：公共错误、校验标识符、消息树、可注入 session store/clock、上下文预算与压缩。
- `crates/core/tests/{common/mod,session_tests,context_tests}.rs`：27 个测试（5 arch + 10 context + 12 session）；既有 `arch_layering.rs` 未改。
- `crates/core/README.md`：职责 / 边界 / 不变量 / 用法 / 已知限制同步。
- `Cargo.lock`：workspace 依赖关系更新；无第三方 crate 新增。
- 本卡记录区与 §11.1 进度同步文件。

### 3. 验收输出摘要

（未进入实现，故无本卡验收输出。只读调研用到的命令与一手结论见 §5 各条的「佐证」。）

**2026-09-26 实现轮次追加**

- `cargo fmt --all --check` → PASS（0 diff）。
- `cargo clippy --all-targets -- -D warnings` → PASS。
- `cargo test --workspace` → PASS；`assistant-core` = 5 arch + 10 context + 12 session tests，0 failed。
- `cargo test -p assistant-core` / `cargo test -p assistant-core arch::` → PASS。
- `verify-schemas` / `codegen --check` → PASS。
- `hygiene` → PASS，0E/4W（回到既有基线，无新增）。
- `docscan` → PASS，0E/468W（既有基线，无新增）；`card-check` → PASS，0E/27W（既有基线，无新增）。
- `memory-counts` / `adr-index` / `check-ledger` → PASS。
- `cargo deny check` → PASS（advisories/bans/licenses/sources ok；既有重复版本与未命中 license warning 未新增）。
- `cargo llvm-cov -p assistant-core --fail-under-lines 85` → PASS，行覆盖 **92.62%**。
- `RUSTDOCFLAGS=-D warnings cargo doc -p assistant-core --no-deps` → PASS。

### 4. DoD 逐条核对

- [ ] card 标题声明的能力可被测试用例覆盖 —— **未做**（§5 阻塞）
- [ ] `cargo fmt --all --check` 0 diff —— 未做
- [ ] `cargo clippy --all-targets -- -D warnings` 退出码 0 —— 未做
- [ ] `cargo test --workspace` 全绿 —— 未做
- [ ] `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check` 全 PASSED —— 未做
- [ ] LEDGER.md 追加一行；如新增事实/坑则追加 `docs/memory/{facts,pitfalls}.md` —— **已做**（LEDGER 记 `Blocked` + 5 条 DRIFT）

**2026-09-26 实现轮次核对**

- [x] 会话创建 / 恢复 / 结束、消息树增删与顺序、结束后 fail-closed：已覆盖。
- [x] 预算生效，超预算显式 omission；必留内容超预算返回错误：已覆盖。
- [x] Required / Summarizable / Droppable 三档裁剪可审计：已覆盖。
- [x] 压缩失败 / 摘要来源不匹配 / 摘要过大：带 `ErrorCode` 失败，不静默丢历史。
- [x] `[dependencies]` 仅新增白名单 `assistant-protocol`；arch test 全绿。
- [x] 单元测试零真实 IO / 网络 / 时钟：使用内存 store + 固定时钟。
- [x] `assistant-core` 行覆盖 ≥ 85%：实测 92.62%。
- [x] 既有测试零改动通过。
- [x] 14 条验收全绿；hygiene / card-check / docscan warning 未新增。
- [x] §11.1 同步已在本轮一次性完成。
- [x] 新增长期记忆：见 §8 与 `docs/memory/pitfalls.md`。

### 5. 偏差

#### DRIFT-028-1（漂移触发器 ⑤：超出 write scope）—— FTS5 检索属**存储层**，本卡 write scope 只有 `crates/core/**`

- **现象**：卡标题的「Memory(App Map 加载 / **FTS5 检索**)」中，FTS5 虚表 `memory_fts` 明确属**存储层**：`docs/storage-design.md` L64「`+ FTS5: memory_fts（任务历史与偏好检索）`」、L175「`| memory_fts（FTS5 虚表） | L1 | 任务历史、偏好、领域笔记的全文检索 |`」。而 `crates/core/README.md` 的**边界**明写「**不直接持有 SQLite 连接：持久化经 `assistant-storage`（TASK-012）**」。
- **佐证（一手）**：① `crates/storage/src/error.rs` L24 的注释已预告「后续卡（TASK-013 审计、**TASK-028 FTS5**）会新增变体」→ 设计上**就预期本卡改 storage**；② `crates/storage` 现有公开面（`crates/storage/src/lib.rs`）**没有任何检索 API**（只有 `Database::open/clock/blob_store/schema_version/close` + 记录类型）→ FTS5 检索必须**新加**：迁移链加 `memory_fts` 虚表 + 公开检索 API + `StorageError` 变体 + 存储侧测试；③ **依赖不是障碍**：`libsqlite3-sys` 的 `bundled` 构建已带 `-DSQLITE_ENABLE_FTS5`（本机 `build.rs` L132 实测）→ **不需要新第三方依赖**，缺的只是**写面**。
- **影响**：不扩 scope → FTS5 无法实现（本卡 DoD 第 1 条永远不满足）；扩 scope → 本卡要同时改 `crates/storage/**`（含迁移登记与 storage 测试），与「一卡一个 write scope / 并行不重叠」的分工直接冲突。
- **建议**（择一，需人类裁决）：
  1. **立一张前置卡**（`crates/storage`：`memory_fts` 迁移 + 检索 API + 测试）→ 本卡的 Memory 只消费该 API（**推荐**：符合 storage-design 的分层与「任何 crate 往迁移链加表都得顺手改 storage 测试」的既有机制）；
  2. 授权把本卡 write scope 扩为 `crates/core/**` + `crates/storage/**`（须同时按机制改 storage 迁移登记与测试）；
  3. 本卡只做 Memory 的 **App Map 加载 + 检索 trait 抽象**，FTS5 后端实现另案。
- **已停工作**：FTS5 相关**未写任何代码**。

#### DRIFT-028-2（漂移触发器 ⑩：工作量超预估 2 倍）—— 单卡承载 5 个子系统

- **现象**：卡标题 = **5 个子系统**（会话管理 / 上下文管理（树裁剪·压缩·预算）/ Planner / Memory / 组装），而卡的 `预估：M`、`难度：M`。可比卡：TASK-022（task-engine 状态机）、TASK-026（model-gateway）、TASK-027（hitl）**各自只做 1 个子系统**，且都逼近 gov §5.4 的 600/900 行上限。
- **影响**：DoD 第 1 条要求「card 标题声明的能力**可被测试用例覆盖**」，而 stage-1 DoD 另有「`core` 覆盖率 ≥ 85%」。5 个子系统在一个会话内做完 + 达标覆盖**不可能**；硬做 = 大面积未测试代码 = 违反铁律 1 与 DoD。
- **建议**：拆卡（例：028a 会话 + 上下文预算/压缩；028b Planner（Plan/Step DAG 构造与校验）；028c Memory（App Map 加载 + 检索）；028d 组装/Host wiring）。**注意**：拆卡要改 `plans/stage-1-pilots.md` 的批次表，而人类明确「**禁止更改原有 plan 的排期以及各点要做的内容**」→ 必须**人类裁决**后才可动。
- **已停工作**：未开始编码。

#### DRIFT-028-3（漂移触发器 ③ / ⑨：公共接口 + 新抽象层）—— `crates/core` 的接口面是否需先有 ADR + spec？

- **现象**：`crates/core` 目前**零 `pub` 项**（`crates/core/src/lib.rs` 全文只有文档 + `#![deny(unsafe_code)]`），其 README 明写「骨架期**不发明接口**：公共 API 的形状属接口设计，归 TASK-020~028」。本卡要首次落地 `SessionManager` / `ContextManager` / `Planner` / `Memory` 等公开类型与 trait —— 其中 `ContextManager` / `Memory` 是**新抽象层**（⑨），且 TASK-029（binary 装配）与后续卡会消费 = **公共接口**（③）。铁律 10「契约先行：改 schema/接口/分层之前先有 ADR + spec 更新」。
- **判据待裁决**：先例不一致 —— TASK-026 / TASK-027 各自定义 crate 公开 API 时**未**单独立 ADR（ADR-0043/0044/0048 是因为**协议/schema 契约**变更）；而本卡引入的是**新的分层组件**（编排层首次成型），比单纯「crate 内 API」重。
- **建议**：裁决「是否需要 ADR」——
  1. 若需要：先立 ADR（`core` 编排层接口面 + 与 `task-engine` / `storage` / `platform/api` 的分工）+ 补 spec（如 `docs/spec/core-orchestration.md`），本卡按契约实现；
  2. 若不需要（沿用 TASK-026/027 先例）：本卡自行定义 API，但须在 `crates/core/README.md` 的**不变量**里写清边界。
- **已停工作**：未定义任何 `pub` 项。

#### DRIFT-028-4（漂移触发器 ③：与既有不变量冲突）—— 「组装」与 `crates/core/README.md` 不变量 3 矛盾

- **现象**：卡标题含「**组装**」，`crates/core/README.md` 职责也写「装配：把 platform / tool-bus / policy / storage / audit 组装成 Host」；但同一份 README 的**不变量 3** 明写「依赖方向单向：`core → {protocol, storage, platform/api 的 trait}`」—— **不含** `tool-bus` / `policy` / `audit` / `hitl` / `verify` / `undo` / `task-engine`。
- **影响**：`crates/core/tests/arch_layering.rs` 只拦**平台实现**（`windows::` 等）与平台实现依赖，**不拦** workspace crate 依赖 → 真做装配会**静默违反** README 不变量（无任何门禁会红），属铁律 1 意义上的静默漂移。
- **建议**（择一，需裁决）：① 立 ADR 明确 `core` 作为装配层的**依赖白名单**（哪些 workspace crate 可入 `core`）并同步 README 不变量 3；② 把「组装」从本卡移到 **TASK-029**（binary 骨架 / desktop UI —— 装配天然属 binary 层），本卡只产出可装配的组件。
- **已停工作**：未改任何 `Cargo.toml`。

#### DRIFT-028-5（漂移触发器 ⑧：卡片正文未按 gov §3.2 展开）

- **现象**：本卡 §步骤 = 「占位 —— 派单前由 Orchestrator 按 gov §3.2 模板与实际调研补充」；`plans/stage-1-pilots.md` L193 同注「Ready（**批次表占位派单前补全**）」。正文缺少「必须遵守」「In/Out scope」「具体验收命令」等 gov §3.2 要求的条目。
- **影响**：AGENTS §3 启动协议要求以卡**全文**对齐回执；占位正文使 §1 回执的若干字段只能从 plan 与 README 反推，**不是卡面事实源**。
- **建议**：由 Orchestrator（人类此前已授权「代 Orchestrator 展开正文」）在 **DRIFT-028-1~4 裁决之后**展开正文 —— 顺序不能反：范围未定就展开 = 把错误范围写进只读正文。
- **已停工作**：本会话**刻意未展开**本卡正文。

#### DRIFT-028-1 ~ 028-5 的裁决与落地（2026-09-26）

人类 2026-09-26 裁决：**「drift-028 的 5 点都按照你的建议做」** → 全部采纳 §5 各条的**建议 1**。落地物如下：

| DRIFT | 采纳的方案 | 落地物 |
|---|---|---|
| **028-1**（⑤ 超 write scope） | 立**前置卡**：存储侧做 `memory_fts`（FTS5）+ 检索 API + 存储侧测试；本卡的 Memory 只**消费**该 API | **ADR-0053 D6**；`tasks/TASK-206-storage-memory-fts5-search.md` |
| **028-2**（⑩ 超预估 2 倍） | **拆卡**：本卡收窄为「会话 + 上下文」；Planner → **TASK-207**；Memory → **TASK-208**；「组装」→ TASK-029（见 028-4） | **ADR-0053 D7**；`tasks/TASK-207-core-planner-plan-step-dag.md` / `tasks/TASK-208-core-memory-app-map-fts-retrieval.md` |
| **028-3**（③⑨ 公共接口 / 新抽象层） | **先立 ADR + spec**，再实现（铁律 10） | **ADR-0053** + `docs/spec/core-orchestration.md` |
| **028-4**（③ 与不变量冲突） | 「组装」**下沉到 TASK-029**（binary 装配层）；本卡只产出**可装配组件** | **ADR-0053 D1 / D5**；`tasks/TASK-029-binary-skeleton-agent-core-desktop-ui.md` 正文补 Host 装配；`crates/core/README.md` 不变量 3 改写为依赖白名单口径 |
| **028-5**（⑧ 正文占位） | 由 Orchestrator（人类此前已授权**代行**）在 028-1~4 **之后**展开正文 | 本卡正文已按 gov §3.2 重写（见分界线以上）；TASK-207 / 208 正文同步展开 |

**顺序遵守**：028-5 的正文展开**发生在** 028-1~4 定案**之后**（不是之前 —— 范围未定就展开 = 把错误范围写进只读正文）。

**改名**：本卡文件名由 `tasks/TASK-028-core-session-context-planner-memory.md` 改为 `tasks/TASK-028-core-session-context.md`（**号不变**，ADR-0031 D7 的「按号寻卡」契约不受影响；ADR-0051 D5 同口径：历史台账保留旧名）。

**未改**：任何产品代码 —— 本卡至今仍未开工。

#### DRIFT-028-6（漂移触发器 ⑤ / ⑧，非阻塞，已按默认路径继续）

- **现象**：契约要求会话持久化经 `assistant-storage` 公开 API，且 `0001_init.sql` 注释把 `conversations` 表标为“归 TASK-028”；但本卡 write scope 不含 `crates/storage/**`，storage 当前也没有 `conversations` 表或公开 session 记录 API。
- **影响**：本轮可以完成无 SQL 的 `SessionStore` 边界、快照校验与内存实现，但不能声称生产持久化已经接通；若在 TASK-029 前不补 storage API，装配点将没有可调用的真实 adapter。
- **处理**：按 automation 的 ADR-0046 D8 非阻塞路径继续；不越界改 storage，不把内存 store 冒充生产持久化。记录 PL-092，后续 storage 卡补齐公开 conversation/session 记录 API 后，由 TASK-029 实现 adapter。
- **风险**：进程重启后的 session 恢复仍依赖未来 adapter；这是本卡 README 已明列的已知限制。

### 6. 更合理做法

（未进入实现，无「更合理做法」可记。§5 各条的「建议」即本会话给出的更合理路径。）

**2026-09-26 实现轮次追加**：先把消息树选择与 context 预算做成纯函数，再让 session store/compressor 都成为注入边界。这样实现不依赖 SQLite、模型厂商或系统时钟，回放测试可完全确定；代价是 TASK-029 必须提供真实 adapter，不能由 Core 内部 `new` 出实现。

### 7. 遗留问题

1. **本卡处于 `Blocked`**（5 条 DRIFT 待裁决）——裁决后需按裁决结果展开正文（DRIFT-028-5）再领卡。
2. 若采纳 DRIFT-028-1 的建议 1（前置卡）或 DRIFT-028-2 的拆卡，**都要改 `plans/stage-1-pilots.md`**（Orchestrator-only / 人类禁止改排期）→ 人类裁决时一并授权。
3. **顺带发现（不属本卡，未改任何文件）**：`crates/core/README.md` 不变量 3 与自身「职责」段的「装配」口径已互相矛盾（DRIFT-028-4）—— 即使本卡不动，也建议单独立卡或并入 ADR。

**2026-09-26 实现轮次追加**

- 生产 `SessionStore` adapter 尚无落点；已记 PL-092，不能留作静默缺口。
- 真实 tokenizer 与模型压缩提示词仍分别归 model provider / TASK-029 装配；本卡只冻结预算和 omission 语义。
- 分支删除目前仅允许叶节点；tombstone / 分支级删除需要新契约。

### 8. 新增长期记忆

（未产生验证过的硬事实 / 坑 / 否决方案，故**不**追加 `docs/memory/*`。§5 里的一手结论（`bundled` 已带 FTS5、`memory_fts` 属存储层、`core` 零 `pub` 项）在裁决落地前**不当作已确认事实**写入长期记忆 —— 避免把「待裁决」写成「已决定」。）

**2026-09-26 实现轮次追加**

- `docs/memory/pitfalls.md`：新增「Core session 不得内置 SQLite；真实 `SessionStore` adapter 必须等 storage 公开记录 API 并在装配层接线」。
- `docs/PARKING_LOT.md`：新增 PL-092，登记 storage 缺 conversation/session 公开记录 API。

### 9. 给审阅者的关注点

1. **DRIFT-028-1 最硬**：`docs/storage-design.md` 与 `crates/storage/src/error.rs` 的注释都**已经**把 FTS5 归给 TASK-028，但本卡 write scope 只有 `crates/core/**` —— 这是**卡面与存储设计的既存矛盾**，请裁决是「前置卡」还是「扩 scope」。
2. **DRIFT-028-2 是排期问题**：5 个子系统塞进一张 `M` 卡不成立；但拆卡要动 `plans/*`，而那正是人类禁止 agent 改动的排期区 —— 需要人类点头。
3. **DRIFT-028-4 是静默漂移**：装配会违反 README 不变量 3，而**现有门禁不会红**（arch test 只拦平台实现）。建议把「装配」下沉到 TASK-029。

**2026-09-26 实现轮次追加**

1. **最高风险 = DRIFT-028-6**：请先确认 `SessionStore` trait + TASK-029 adapter 的分工是否可接受；若要求本卡直接接通 SQLite，则 write scope 与 storage API 都需要人类裁决。
2. **上下文算法取舍**：只压缩从预算截断点起的连续旧后缀，保留较新的连续历史；这样避免“新消息被摘要、旧消息却明文保留”的倒序语义，但比任意背包式裁剪少放一些内容。
3. **错误映射**：缺失 session/message 沿用 task-engine 先例映射 `TargetNotFound`，预算不足映射 `PolicyDenied`，内部结构错误映射 `Fatal`；若希望新增 Core 专属错误类别必须先改 protocol schema/ADR。
