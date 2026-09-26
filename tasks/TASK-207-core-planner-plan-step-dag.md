# TASK-207　`core`：Planner（模型输出 → 可校验的 Plan / Step DAG）

- 状态：**Ready**
- 阶段：1　子阶段：**1a**　批次：**A2**（原 TASK-028 的拆卡）　依赖：022 / 026 / 028　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`；契约见 `docs/spec/core-orchestration.md` 与 `docs/adr/0053-core-orchestration-layer-interface.md`。
- 本卡与 TASK-028 / TASK-208 **共用 `crates/core/**`** → **必须严格串行**（依赖列已保证顺序），不得与它们并行。
- 来源：**DRIFT-028-2**（原 TASK-028 §5；触发器 ⑩）+ **DRIFT-028-3**（触发器 ③⑨）+ **ADR-0053 D4 / D7 / D8**

---

## 目标（一句话）

在 `crates/core` 落地 **Planner**：把 `ModelProvider` 返回的补全结果变成 `assistant-task-engine` 的 `Plan` / `PlanStep`，并在**进入任务引擎之前**完成结构与安全校验（重复 id / 缺依赖 / 环 / 非法工具名 / 写步骤缺 postcondition / L3 未标 point-of-no-return）。

## 背景（为什么现在做）

| # | 事实 | 证据 |
|---|---|---|
| 1 | 原 TASK-028 一张 `M` 卡塞了 5 个子系统（会话 / 上下文 / Planner / Memory / 组装）→ 触发器 ⑩ | `tasks/TASK-028-core-session-context-planner-memory.md` §5 的 **DRIFT-028-2** |
| 2 | 人类 2026-09-26 裁决采纳「拆卡」 | 人类 chat 2026-09-26；**ADR-0053 D7** |
| 3 | `assistant-task-engine` **已经**导出 Planner 需要的全部类型与校验入口 | `crates/task-engine/src/lib.rs`：`Plan` / `PlanStep` / `PlanId` / `StepId` / `TaskId` / `Budget` / `Reversibility` / `StepEffect` / `ready_step_ids` 等 |
| 4 | `core` 首次落地公开接口面 → 必须**契约先行**（铁律 10） | **ADR-0053**（D4 / D8）+ `docs/spec/core-orchestration.md` §3 / §4 |
| 5 | 计划 DAG 的形状有架构级定义 | 架构 v2 §8.3（Plan / Step DAG JSON 示例）、§5（工具与 Planner 分工） |

## write scope

- `crates/core/src/**`（**Planner 相关模块** + `lib.rs` 的 re-export；新增模块属漂移触发器 ②，本卡的存在即 ADR-0053 的授权）
- `crates/core/tests/**`（**新增**测试；不得改既有断言 —— 漂移触发器 ⑦）
- `crates/core/Cargo.toml`（**仅当**需要白名单内的依赖；白名单见 ADR-0053 D2）
- `crates/core/README.md`（仅当「职责 / 边界 / 不变量 / 已知限制」因本卡需要同步）
- `tasks/TASK-207-core-planner-plan-step-dag.md`（本文件）
- `LEDGER.md` / `PLAN.md`（仅「当前状态」块 4 行）/ `README.md`（仅三处）/ `plans/stage-1-pilots.md`（仅完成状态与「当前进度」块）/ `MEMORY.md`（仅规模表）：AGENTS.md §11.1 强制的进度同步

## Out of scope（写了就停）

- **不定义** `Plan` / `PlanStep` / `PlanId` / `StepId` / `Budget` / `Reversibility` / `StepEffect` —— 一律**复用** `assistant-task-engine`（ADR-0053 D2 / `docs/spec/core-orchestration.md` 不变量 4）
- **不做**任务状态机、检查点、恢复、取消、预算执行、看门狗（归 TASK-022 的 `task-engine`，已 Done）
- **不做**模型路由 / 重试 / 降级 / 成本（归 TASK-026 的 `model-gateway`，已 Done）—— 本卡只**消费** `ModelProvider`
- **不判权限**：工具是否放行归 `crates/policy`（铁律 3）；本卡只做**结构与安全形状**校验，不做策略判定
- **不执行**任何工具、**不**持有 SQLite 连接、**不**调平台 API（铁律 7）
- **不引**黑名单 crate（ADR-0053 D3）：`tool-bus` / `policy` / `audit` / `hitl` / `verify` / `undo` / `lease` / `platform/{windows,macos,linux}` / `apps/*`
- **不做**多轮对话规划、不做向量检索、不做无人值守相关能力（阶段 1 Out of scope）
- **不放宽**任何 lint、不加 `#[allow]`、不加 `unsafe`、不改 `protocol` schema / `ErrorCode`

## 必须遵守

- **五类不可信输入**（铁律 2）：模型输出进入本卡时**先校验**；校验失败一律带 `ErrorCode` 返回，**禁止**用默认值或空计划冒充成功
- **fail-closed**：任何解析 / 校验失败都不得降级成「一个看起来能跑的计划」；`NeedsHuman` 之外的路径不得猜测
- **可注入**：模型调用经 `ModelProvider` trait 注入，时钟 / 随机 / UUID / FS 一律 trait 注入（AGENTS.md §5.3，保证可回放）
- **文档注释**：公共 API 100% 有文档注释（语义 / 参数 / 返回 / **错误语义** / 副作用 / 是否幂等 / 超时与取消行为）
- **命名**：遵守 `docs/spec/naming.md` 的受控词汇表（`Step` 不叫 Action / Operation；`Tool` vs `Skill`）
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

- [ ] Planner 能把一份**合法**的模型输出转成 `task-engine` 的 `Plan` / `PlanStep`，并被 `task-engine` 接受
- [ ] 负向用例齐全：重复 id / 缺失依赖 / 环 / 非法工具名 / 写步骤缺 postcondition / L3 未标 point-of-no-return —— **每一类都有独立用例**且返回带 `ErrorCode` 的失败
- [ ] 模型输出不可解析 / 缺字段 / 类型错误 → 失败（**不**产生空计划、**不**猜测）
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
【任务】TASK-207　`core` Planner
【目标】把模型输出转成经结构与工具目录校验的 task-engine Plan
【write scope】仅：crates/core/src/**、crates/core/tests/**、crates/core/Cargo.toml、crates/core/README.md、本卡与 §11.1 进度同步文件
【铁律】1 无静默失败；2 模型输出先校验；3 core 不判权限；7 core 不调平台 API；9 不扩范围；10 契约先行
【禁止】不定义 Plan/PlanStep；不做路由/重试/执行；不改 schema/ErrorCode；不放宽 lint；不动既有测试断言
【验收】本文件正文 14 条命令 → 全绿；assistant-core 行覆盖 88.60% → PASS（门槛 85%）
【依赖】TASK-022 / 026 / 028 已 Done，已核对 LEDGER 末 10 行与当前状态
【疑问】无
```

### 2. 实际改动文件

- `crates/core/src/planner.rs`：新增 `Planner` / `PlannerRequest`，构建 JSON-only provider 请求、消费 `ModelProvider` 流、解析 `PlanStep` 数组并调用 `Plan::validate()` 与工具目录校验。
- `crates/core/src/{lib,error}.rs`：导出 Planner 模块；新增 Planner 模型失败、非法 Plan、非法模型输出、目录外工具四类稳定错误映射。
- `crates/core/tests/planner_tests.rs`：14 个正常/负向/流契约测试，覆盖六类指定负向用例。
- `crates/core/tests/arch_dependencies.rs`：3 个测试把 ADR-0053 D2 workspace 依赖白名单变成机器断言（含正反例）。
- `crates/core/Cargo.toml`：新增白名单内 workspace 依赖 `assistant-task-engine` 与 `assistant-model-gateway`；零第三方新增。
- `crates/core/README.md`：同步职责、边界、不变量、已知限制与相关卡。
- `Cargo.lock`：Cargo 为 `assistant-core` 自动追加两条既有 workspace crate 依赖边；无新 package、无版本变化。
- 本卡记录区、`LEDGER.md`、`PLAN.md` 当前状态块、`README.md` 三处、`plans/stage-1-pilots.md` 完成状态与当前进度块、`docs/memory/facts.md` 新 FACT、`MEMORY.md` 规模表。

### 3. 验收输出摘要

- `cargo fmt --all --check` → PASS（0 diff）。
- `cargo clippy --all-targets -- -D warnings` → PASS。
- `cargo test --workspace` → PASS；core 新增 Planner 14 tests，arch 依赖 3 tests。
- `cargo test -p assistant-core` → PASS：arch 5 + context 10 + planner 14 + session 12 = 44 tests。
- `cargo test -p assistant-core arch::` → PASS：2 个 test binary 共 8 tests。
- `cargo run -p xtask -- verify-schemas` → PASS（5 schemas，0 error）。
- `cargo run -p xtask -- codegen --check` → PASS（0 drift）。
- `cargo run -p xtask -- hygiene` → PASS（0 error / 4 既有 warning，`scanned=260`）。
- `cargo run -p xtask -- docscan` → PASS（0 error / 468 既有 warning）。
- `cargo run -p xtask -- card-check` → PASS（0 error / 27 既有 warning）。
- `cargo run -p xtask -- memory-counts` → PASS（0 error）。
- `cargo run -p xtask -- adr-index` → PASS（0 error）。
- `cargo run -p xtask -- check-ledger` → PASS（0 error）。
- `cargo deny check` → PASS（advisories / bans / licenses / sources 全 ok；输出只有既有 duplicate / unmatched-license warning）。
- `cargo llvm-cov -p assistant-core --fail-under-lines 85` → PASS：TOTAL 行覆盖 **88.60%**（Planner 模块 77.12%，总门槛由既有模块保证）。

### 4. DoD 逐条核对

- [x] 合法模型输出 → task-engine `Plan` / `PlanStep`，`Plan::validate()` 通过并成功交给 `TaskEngine::create_task`。
- [x] 六类负向用例均有独立测试：重复 id / 缺依赖 / 环 / 非法工具名 / 写步骤缺 postcondition / L3 未标 point-of-no-return。
- [x] 不可解析、缺字段、类型错误、空 steps、tool-call 输出、无 Stop finish 均 fail-closed；不产生空计划。
- [x] `core` 依赖在 ADR-0053 D2 白名单内；新增 `arch_dependencies.rs` 做机器校验。
- [x] Planner 测试用注入 `ModelProvider`，零真实 IO / 网络 / 时钟。
- [x] `assistant-core` 行覆盖 88.60% ≥ 85%。
- [x] 既有测试零改动通过。
- [x] 14 条验收命令全绿；`hygiene` / `card-check` / `docscan` warning 未新增。
- [x] §11.1 进度同步已落地。
- [x] 新 FACT 已追加到 `docs/memory/facts.md`。

### 5. 偏差

none。`Cargo.lock` 是 `Cargo.toml` 新增 workspace 依赖后由 Cargo 自动生成的依赖边更新，无新 package、无第三方引入、无版本变化。

### 6. 更合理做法

新增 `arch_dependencies.rs`，把 ADR-0053 的 workspace 依赖白名单从 Review 人工核对提升为 `cargo test -p assistant-core arch::` 的机器断言；同时保留合成坏样本，避免扫描器变成恒真断言。

### 7. 遗留问题

无阻塞。Planner 按本卡契约直接消费 `ModelProvider`，不承担路由、重试、降级、预算或工具执行；这些仍由 `model-gateway` 与后续装配层负责。

### 8. 新增长期记忆

`docs/memory/facts.md` 新增：TASK-207 Planner 的公开基线 = 模型只输出 `steps`、调用方持有 plan/task identity、严格 JSON、`Plan::validate()` + 工具目录双重校验、单次 poll 可取消。

### 9. 给审阅者的关注点

1. 最该审的是严格输出契约：顶层只接受单一 `steps` 字段，不做 Markdown / 缺字段修复；这符合 fail-closed，但要求 Provider 忠实遵守 JSON 请求。
2. `UnknownPlannerTool` 在 `Plan::validate()` 之后检查，因此非法语法与目录外工具是两个不同错误；确认这是期望分层。
3. `Planner` 直接消费 `ModelProvider`，因此不继承 `ModelGateway` 的 retry/fallback；确认这与 ADR-0053 D2 的本卡边界一致。
