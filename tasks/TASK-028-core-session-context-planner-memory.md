# TASK-028　`core`：会话管理 + 上下文管理（树裁剪/压缩/预算）+ Planner + Memory(App Map 加载/FTS5 检索) + 组装

- 状态：**Ready**
- 阶段：1　子阶段：**1a**　批次：**A2**　依赖：020~027　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：020~027　**预估**：M　**难度**：M
- **write scope**：`crates/core/**`
- **关联**：`plans/stage-1-pilots.md` 批次表 A2（1a）、`docs/wbs-overview.md` §6（DoD）

**目标**

`core`：会话管理 + 上下文管理（树裁剪/压缩/预算）+ Planner + Memory(App Map 加载/FTS5 检索) + 组装。

**write scope**（本卡独有部分，完整列表见 plan 批次表）

`crates/core/**`

**步骤**（占位 —— 派单前由 Orchestrator 按 gov §3.2 模板与实际调研补充）

1. 环境记录（OS / 依赖版本 / 输入 fixture）
2. 实现 card 标题声明的能力，附最小自检命令
3. 跑 `cargo test --workspace` + 本卡专项测试；不合格 → DRIFT
4. 更新 `docs/memory/apps/<app>.md` 或 `facts/pitfalls.md`（应用专属去 apps，跨应用去 pitfalls）

**DoD**

- [ ] card 标题声明的能力可被测试用例覆盖
- [ ] `cargo fmt --all --check` 0 diff
- [ ] `cargo clippy --all-targets -- -D warnings` 退出码 0
- [ ] `cargo test --workspace` 全绿
- [ ] `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check` 全部 PASSED
- [ ] LEDGER.md 追加一行；如新增事实/坑则追加 `docs/memory/{facts,pitfalls}.md`

**验收命令**

```powershell
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check
```

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

### 2. 实际改动文件

（本会话未进入实现 —— 见 §5。本卡记录区之外**零改动**。）

### 3. 验收输出摘要

（未进入实现，故无本卡验收输出。只读调研用到的命令与一手结论见 §5 各条的「佐证」。）

### 4. DoD 逐条核对

- [ ] card 标题声明的能力可被测试用例覆盖 —— **未做**（§5 阻塞）
- [ ] `cargo fmt --all --check` 0 diff —— 未做
- [ ] `cargo clippy --all-targets -- -D warnings` 退出码 0 —— 未做
- [ ] `cargo test --workspace` 全绿 —— 未做
- [ ] `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check` 全 PASSED —— 未做
- [ ] LEDGER.md 追加一行；如新增事实/坑则追加 `docs/memory/{facts,pitfalls}.md` —— **已做**（LEDGER 记 `Blocked` + 5 条 DRIFT）

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

### 6. 更合理做法

（未进入实现，无「更合理做法」可记。§5 各条的「建议」即本会话给出的更合理路径。）

### 7. 遗留问题

1. **本卡处于 `Blocked`**（5 条 DRIFT 待裁决）——裁决后需按裁决结果展开正文（DRIFT-028-5）再领卡。
2. 若采纳 DRIFT-028-1 的建议 1（前置卡）或 DRIFT-028-2 的拆卡，**都要改 `plans/stage-1-pilots.md`**（Orchestrator-only / 人类禁止改排期）→ 人类裁决时一并授权。
3. **顺带发现（不属本卡，未改任何文件）**：`crates/core/README.md` 不变量 3 与自身「职责」段的「装配」口径已互相矛盾（DRIFT-028-4）—— 即使本卡不动，也建议单独立卡或并入 ADR。

### 8. 新增长期记忆

（未产生验证过的硬事实 / 坑 / 否决方案，故**不**追加 `docs/memory/*`。§5 里的一手结论（`bundled` 已带 FTS5、`memory_fts` 属存储层、`core` 零 `pub` 项）在裁决落地前**不当作已确认事实**写入长期记忆 —— 避免把「待裁决」写成「已决定」。）

### 9. 给审阅者的关注点

1. **DRIFT-028-1 最硬**：`docs/storage-design.md` 与 `crates/storage/src/error.rs` 的注释都**已经**把 FTS5 归给 TASK-028，但本卡 write scope 只有 `crates/core/**` —— 这是**卡面与存储设计的既存矛盾**，请裁决是「前置卡」还是「扩 scope」。
2. **DRIFT-028-2 是排期问题**：5 个子系统塞进一张 `M` 卡不成立；但拆卡要动 `plans/*`，而那正是人类禁止 agent 改动的排期区 —— 需要人类点头。
3. **DRIFT-028-4 是静默漂移**：装配会违反 README 不变量 3，而**现有门禁不会红**（arch test 只拦平台实现）。建议把「装配」下沉到 TASK-029。
