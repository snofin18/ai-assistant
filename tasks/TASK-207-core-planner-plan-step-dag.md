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

（待填）

### 2. 实际改动文件

（待填）

### 3. 验收输出摘要

（待填）

### 4. DoD 逐条核对

（待填）

### 5. 偏差

（待填）

### 6. 更合理做法

（待填）

### 7. 遗留问题

（待填）

### 8. 新增长期记忆

（待填）

### 9. 给审阅者的关注点

（待填）
