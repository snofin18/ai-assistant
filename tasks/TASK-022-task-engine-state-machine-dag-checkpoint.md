# TASK-022　`task-engine`：12 状态机 + Plan/Step DAG + 检查点 + 恢复 + 取消 + 预算 + 看门狗

- 状态：**Done**（2026-09-25）
- 阶段：1　子阶段：**1a**　批次：**A2**　依赖：011,012　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：011（`crates/protocol` 的错误分类与协议类型）、012（`crates/storage` 的 W1 任务/步骤/检查点记录 API）
- **预估**：M　**难度**：M
- **write scope**：`crates/task-engine/**`（新 crate `assistant-task-engine`）、`docs/DEPENDENCIES.md`（**仅追加**本卡实际使用的依赖列）
- **关联**：`plans/stage-1-pilots.md` 批次表 A2（1a）、`docs/wbs-overview.md` §6；架构 v2 **§8.1 状态集合 / §8.2 六阶段执行模型 / §8.3 Plan-Step DAG / §8.4 中断暂停取消接管 / §8.5 崩溃恢复 / §8.9 超时预算看门狗**；`docs/storage-design.md` §3.1/§4/§6/§8；`docs/spec/error-codes.md`；**铁律 1 / 2 / 4 / 6 / 9 / 10**

**目标**

新建 `crates/task-engine`，把架构 v2 §8 的任务引擎落成确定性领域逻辑：

- **Task 12 状态机**：`Draft`、`AwaitingApproval`、`Running`、`Paused`、`TakenOver`、`Resuming`、`Cancelled`、`Completed`、`CompletedWithWarnings`、`Failed`、`Blocked`、`NeedsHuman`。
- **Step 状态机**：`Pending`、`Prechecking`、`Approved`、`Executing`、`Verifying`、`Committed`、`PolicyDenied`、`Failed`、`Retrying`、`RolledBack`、`Skipped`。
- **Plan / Step DAG**：校验重复 id、缺失依赖、自环与环；按依赖与步骤状态计算可执行 Frontier。
- **检查点与恢复**：每次成功状态变更先持久化完整的最小可恢复快照；恢复时对 `Executing` / `Verifying` 步骤接收外部证据，只有 `Completed` / `NotCompleted` / `Unknown` 三种结论；`Unknown` 必须进入 `NeedsHuman`，绝不猜测。
- **取消 / 暂停 / 接管**：暂停在当前步骤完成后生效；取消停止后续步骤；接管立即停手并保留恢复入口。
- **预算与看门狗**：步数、总时长、token、成本四类预算；resolve / execute / verify 三类超时；预算耗尽或看门狗超时进入 `NeedsHuman` 或 `Failed`，不无限重试。

**In scope（本卡交付物）**

| # | 交付物 | 依据 |
|---|---|---|
| 1 | `crates/task-engine/Cargo.toml`（`assistant-task-engine`）+ `src/lib.rs` + `README.md`（职责 / 边界 / 不变量 / 已知限制） | gov §5.4、阶段 1 DoD |
| 2 | 强类型 `TaskId` / `StepId` + Task / Step 状态枚举与受控迁移函数 | 架构 v2 §8.1、`docs/spec/naming.md` |
| 3 | `Plan` / `PlanStep` / `DependsOn` DAG + 结构校验 + 确定性可执行 Frontier | 架构 v2 §8.3 |
| 4 | `TaskSnapshot` / `StepSnapshot` / `Budget` / `BudgetUsage` / `StepTimeouts` / 看门狗判定 | 架构 v2 §8.3 / §8.9 |
| 5 | 检查点仓库 trait + 内存实现 + 基于 `assistant-storage` 公开记录的 SQLite 实现 | 架构 v2 §8.5、storage-design §3.1 / §4 / §8 |
| 6 | 恢复分类：`StepRecoveryEvidence::{Completed,NotCompleted,Unknown}` → 跳过 / 重做 / `NeedsHuman` | 架构 v2 §8.5 硬规则 |
| 7 | 取消、暂停、接管、恢复、预算耗尽与看门狗超时的状态迁移 | 架构 v2 §8.4 / §8.9 |
| 8 | 正向与负向测试：12 状态覆盖、非法迁移拒绝、DAG 环拒绝、SQLite 崩溃恢复、未知结论 `NeedsHuman`、预算 / 看门狗硬边界 | ADR-0019 N1、gov §5.5、覆盖率 ≥85% |
| 9 | `docs/DEPENDENCIES.md` 追加 `serde` / `thiserror` 的 `crates/task-engine` 使用方（若最终无新增则显式记录零新增） | 登记规则 1 / 2 |

**Out of scope（做了算漂移）**

- `crates/lease/**`（租约获取、续租、抢占与死锁避免）→ TASK-025
- `crates/verify/**`（断言执行与指纹比较）→ TASK-023
- `crates/undo/**`（撤销锚点与回滚剧本）→ TASK-024
- `crates/hitl/**`（审批请求与授权范围）→ TASK-027
- `crates/core/**`（Planner、会话装配与真实模型循环）→ TASK-028
- Host / IPC / platform 调用：本卡只编排状态，不执行工具
- 修改 `protocol/**`、`crates/protocol/**`、`crates/storage/**` 的 schema 或公共接口
- 新增第三方 crate、顶层目录、`unsafe`、`#[allow]` 放宽 lint、后台线程或真实计时器

**必须遵守**

1. **无静默失败**：非法迁移、损坏检查点、DAG 不合法、预算已耗尽都必须返回带 `ErrorCode` 的错误或显式 `NeedsHuman`，不得落回默认状态。
2. **迁移前置条件**：每次 Task / Step 迁移都必须由显式事件触发；没有 `(state,event)` 规则即拒绝。
3. **检查点原子性**：先构造下一快照并成功持久化，再替换内存状态；持久化失败时内存必须保持原状态。
4. **恢复不猜测**：`Executing` / `Verifying` 步骤在崩溃后只能由外部证据判定；`Unknown` 一律 `NeedsHuman`。
5. **写操作语义**：状态引擎不调用平台 API、不访问网络、不执行工具；`postcondition` 与 `verify` 由 TASK-023 在提交前提供结果。
6. **取消 / 接管**：执行中的步骤不在中途强停；请求取消后等待合法步骤边界；接管时立即停止调度后续步骤。
7. **预算 / 看门狗**：预算判定与超时判定是纯函数，时钟通过参数或注入 trait 提供；禁止读系统时间或启动后台计时线程。
8. **持久化复用**：SQLite 实现只调用 `assistant-storage` 的公开 `insert_task` / `insert_checkpoint` / `load_latest_checkpoint`，禁止重写记录结构或直接拼接 SQL。
9. **规模与命名**：单文件 ≤ 400 行（软）/ 600（硬），函数 ≤ 80 行，参数 ≤ 6；使用 `Task` / `Step` / `Tool` / `Fingerprint` 等受控词。

**DoD**

- [ ] 架构 v2 §8.1 的 12 个 Task 状态与全部 Step 状态都有可执行定义和测试覆盖
- [ ] Plan DAG 对重复 id / 缺失依赖 / 自环 / 环给出可读错误，并对合法 DAG 返回确定性 Frontier
- [ ] 每个成功状态变更写入检查点；SQLite 实现可在重新打开后恢复最近快照
- [ ] `Executing` / `Verifying` 恢复的三条证据路径均有测试：`Completed` → 跳过、`NotCompleted` → 重做、`Unknown` → `NeedsHuman`
- [ ] 暂停、取消、接管、预算耗尽、看门狗超时均有状态迁移测试，非法迁移有负向测试
- [ ] `crates/task-engine/README.md` 含职责 / 边界 / 不变量 / 已知限制
- [ ] `cargo llvm-cov -p assistant-task-engine --fail-under-lines 85` PASS
- [ ] `cargo fmt --all --check` 0 diff
- [ ] `cargo clippy --all-targets -- -D warnings` 退出码 0
- [ ] `cargo test --workspace` 全绿
- [ ] `cargo test -p assistant-core arch::` 全绿
- [ ] `xtask verify-schemas / codegen --check / hygiene / memory-counts / adr-index / docscan / card-check / check-ledger / check-migrations` 全部 PASSED
- [ ] `refscan` 只允许既有 baseline（PL-058），不得新增
- [ ] LEDGER.md 追加一行；如新增事实/坑则追加 `docs/memory/{facts,pitfalls}.md`
- [ ] 无任何 Out of scope 文件被修改

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-task-engine
cargo test -p assistant-core arch::
cargo llvm-cov -p assistant-task-engine --fail-under-lines 85
cargo run -p xtask -- verify-schemas / codegen --check / hygiene / memory-counts / adr-index / docscan / card-check / check-ledger / check-migrations
```

**夜间自动化补充约束（人类 2026-09-25 授权）**

1. 分支 = `task/TASK-022-task-engine-state-machine-dag-checkpoint`，从最新 `origin/main` 切出；PR base 必须为 `main`。
2. 需要裁决的项按最优解自决，但必须写入本卡 §5 与 `LEDGER.md`。
3. 合并需同时满足 CI 全绿、`mergeable_state == clean`、base = `main`；否则只开 PR。
4. 结束后同步 `PLAN.md` 当前状态块、`README.md` 三处、`plans/stage-1-pilots.md` 进度句与完成标记、`LEDGER.md`。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

【任务】TASK-022 `task-engine`：12 状态机 + Plan/Step DAG + 检查点 + 恢复 + 取消 + 预算 + 看门狗
【目标】新建 `assistant-task-engine`，把架构 v2 §8 的确定性状态机、DAG、预算、看门狗与崩溃恢复语义落成可持久化、可回放的领域逻辑。
【write scope】`crates/task-engine/**`、`docs/DEPENDENCIES.md`（追加使用方），以及任务卡记录区、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、必要 memory / PARKING_LOT。
【铁律】1 无静默失败；2 不可信输入先校验；4 确定性提交；6 不可逆动作禁止无人值守；9 不静默扩大范围；10 契约先行。
【禁止】不实现平台 / Host / IPC / lease / verify / undo / hitl；不改 `protocol` / `storage` schema；不加新第三方 crate、`unsafe`、`#[allow]`；未知恢复结果不得猜测。
【验收】fmt / workspace clippy / workspace test / task-engine 专项 / core arch / 文档门禁 / `cargo deny` / 行覆盖 ≥85% → 全绿。
【依赖】TASK-011 / 012 已 Done 并进入 `main`；当前 `main` 干净且无 `.nightly.lock`。
【疑问】卡面正文为占位，按人类授权代 Orchestrator 展开；SQLite 实现只复用 `assistant-storage` 公开记录 API。

### 2. 实际改动文件

- 新增 `crates/task-engine/**`（`Cargo.toml`、README、10 个 `src` 模块、8 个集成测试文件）。
- `docs/DEPENDENCIES.md`：把 `crates/task-engine` 追加为 `serde` / `thiserror` 的使用方。
- `Cargo.lock`：workspace 自动登记 `assistant-task-engine` package（DRIFT-022-2）。
- 文档同步：本卡、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`MEMORY.md` 规模表、`docs/memory/{facts,pitfalls}.md`、`docs/PARKING_LOT.md`。

### 3. 验收输出摘要

- `cargo fmt --all --check` → 0 diff。
- `cargo clippy --all-targets -- -D warnings` → exit 0。
- `cargo test --workspace` → 全绿；`assistant-task-engine` 新增 43 个库/集成测试 + 1 个 doctest。
- `cargo test -p assistant-core arch::` → 5 passed。
- `cargo llvm-cov -p assistant-task-engine --fail-under-lines 85` → PASS，行覆盖 **87.03%**。
- `xtask verify-schemas / codegen --check / hygiene / memory-counts / adr-index / docscan / card-check / check-ledger / check-migrations` → 全部 PASSED。
- `cargo deny check` → advisories / bans / licenses / sources 全 ok。
- `xtask refscan` → 既有 baseline **151 error**；`task-engine` 命中 **0**（PL-058）。
- GitHub PR #30：**16/16 check-runs success**；合并前 base=`main`、`mergeable_state=clean`；merge commit `9f41e92`。

### 4. DoD 逐条核对

- [x] 12 个 Task 状态与全部 Step 状态都有定义和测试。
- [x] DAG 重复 id / 缺失依赖 / 环 / 非法工具名 / 写步骤缺 postcondition 均拒绝；Frontier 确定性排序。
- [x] 每次成功状态变更先写检查点；SQLite 实现重开后恢复。
- [x] `Completed` / `NotCompleted` / `Unknown` 三条恢复路径均测试；缺证据也进入 `NeedsHuman`。
- [x] 暂停、取消、接管、预算耗尽、看门狗超时均覆盖。
- [x] README 含职责 / 边界 / 不变量 / 已知限制。
- [x] fmt / clippy / workspace tests / arch / 文档门禁 / deny / 覆盖率均通过。
- [x] LEDGER 与长期记忆已同步。
- [ ] “无任何 Out of scope 文件被修改”不完全成立：`Cargo.lock` 自动更新，见 DRIFT-022-2。

### 5. 偏差

**DRIFT-022-1（卡片状态行由 Ready → Done）**
- 现象：卡面正文区 `- 状态：` 行由 Implementer 更新。
- 影响：与 ADR-0031 的“正文只读”字面冲突。
- 处置：沿用 TASK-015 ~ 021 的既有做法，保证下一会话能按状态选卡；PL-073 仍在跟踪状态行归属机制。

**DRIFT-022-2（`Cargo.lock` 自动更新）**
- 现象：新增 `crates/task-engine` 后 lockfile 增加 workspace package。
- 影响：该文件不在原卡 write scope，但不更新会让 workspace 不一致。
- 处置：自裁决接受；没有新增第三方 crate，只引用既有已批准依赖。

### 6. 更合理做法

- 检查点采用“先构造下一快照并成功持久化，再返回新快照”，避免内存状态领先于持久状态。
- SQLite 适配器只调用 `assistant-storage` 的公开记录 API，不复制表结构、不直接写 SQL。
- `engine.rs` 拆分出 `scheduler.rs`，把 DAG Frontier 与 Step 变更辅助独立，单文件回到 hygiene 建议线内。

### 7. 遗留问题

- **PL-083**：`assistant-storage` 无 task 行更新 API，`tasks.status` 保持创建时值；最新状态以 checkpoints 为准。
- **PL-058**：`refscan` 既有 151 error 仍非绿，不在本卡范围，本卡新增 0。

### 8. 新增长期记忆

- `docs/memory/facts.md`：新增 TASK-022 的 12/11 状态、持久化优先、44 项测试与 87.03% 覆盖事实。
- `docs/memory/pitfalls.md`：新增“task 行不更新，禁止把 `tasks.status` 当当前状态”的坑。
- `docs/PARKING_LOT.md`：新增 PL-083。

### 9. 给审阅者的关注点

1. **恢复不猜测**：重点检查 `assess_recovery`；缺证据与 `Unknown` 必须保持活动 Step 并进入 `NeedsHuman`。
2. **持久化原子性**：检查 `TaskEngine::persist` 是否始终在返回新快照前成功写检查点，以及 SQLite 初始 task+checkpoint 是否同事务。
3. **状态机边界**：检查取消/暂停只在目标 Step 边界生效，`L3Irreversible` 必须 `point_of_no_return`，写 Step 必须有 postcondition。
