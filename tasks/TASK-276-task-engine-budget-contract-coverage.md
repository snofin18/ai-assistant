# TASK-276　task-engine 预算核算 fail-closed 契约覆盖率补齐（章程 §2 P2）

- 状态：**Done**
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**（ADR-0037 号段 200~299）　依赖：无　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**；`- 状态：` 行按 ADR-0083 为唯一例外）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：无　**预估**：S　**难度**：S
- **write scope**：`crates/task-engine/tests/budget_contract.rs`、`tasks/TASK-276-task-engine-budget-contract-coverage.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/memory/facts.md`、`MEMORY.md`、`docs/automations/2026-10-13-round-2.md`
- **关联**：`docs/automation-charter.md` §2 P2 / §5.3、`crates/task-engine/README.md` 不变量 6、`docs/spec/testing.md`

**目标**

仅为 `assistant-task-engine` 增加**预算核算 fail-closed** 的公开契约测试，把
`crates/task-engine/src/budget.rs` 的行覆盖率从 82.02% 提升到 100%（crate 独立行覆盖率从 90.40%
提升到 ≥91%）。重点覆盖 `Budget::new` 的参数校验、`BudgetUsage` 的计数器 / token / cost 溢出，
以及 `Budget::check` 的 token 溢出与 `Within` 分支。不修改生产逻辑，不启动真实 IO、网络或凭据。

**背景**

2026-10-13 round 2 的独立 crate 基线为行覆盖率 90.40%，`budget.rs` 仅 82.02%。既有
`tests/budget_watchdog.rs` 覆盖了 `Budget::check` 的四个超限维度，但没有覆盖构造期非法参数与
计数器 / token / cost 溢出这些 fail-closed 分支（README 不变量 6「不得静默饱和」）。
候选来自 `docs/automations/2026-10-12-report.md` §③（纯逻辑、无 GUI / keychain）、按 N=2 派单。

**步骤**

1. 新建 `crates/task-engine/tests/budget_contract.rs`。
2. 覆盖 `Budget::new` 拒绝零步数 / 零时长 / 负数 / 非有限 cost。
3. 覆盖 `BudgetUsage::total_tokens`、`record_step_started`、`record_usage` 的溢出与非法 cost 分支。
4. 覆盖 `Budget::check` 的 token 溢出（`Exceeded(Tokens)`）与 `Within` 分支。
5. 复测 `cargo llvm-cov -p assistant-task-engine --tests --fail-under-lines 91` 并跑全部全局硬门禁。

**DoD**

- [ ] 生产逻辑零改动（仅新增测试文件）
- [ ] 新增测试只调用公开 API，覆盖预算 fail-closed 契约
- [ ] `cargo llvm-cov -p assistant-task-engine --tests --fail-under-lines 91` 通过
- [ ] `cargo test -p assistant-task-engine` 全绿
- [ ] `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全绿
- [ ] 全部现行 xtask 硬门禁 PASSED
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 与最终状态同批同步

**验收命令**

```powershell
cargo llvm-cov -p assistant-task-engine --tests --fail-under-lines 91
cargo test -p assistant-task-engine
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene; cargo run -p xtask -- memory-counts; cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan; cargo run -p xtask -- docscan; cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger; cargo run -p xtask -- check-comments
cargo run -p xtask -- verify-schemas; cargo run -p xtask -- codegen --check; cargo run -p xtask -- check-migrations
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

【任务】TASK-276 task-engine 预算核算 fail-closed 契约覆盖率补齐
【目标】仅新增集成契约测试，把 `assistant-task-engine` 独立行覆盖率从 90.40% 提升到 ≥91%
【write scope】仅：新建 `crates/task-engine/tests/budget_contract.rs` 与本卡列出的状态同步文件
【铁律】测试不改生产逻辑；无静默失败；不扩 scope；热点文件先 guard
【禁止】改公共接口、依赖、CI/lint、生产行为；启动真实 IO / 网络 / 凭据
【验收】见「验收命令」→ crate 覆盖率 + 全部硬门禁
【依赖】无；章程 §2 P2 / §5.3；运行日 2026-10-13 round 2
【疑问】无

### 2. 实际改动文件

- `crates/task-engine/tests/budget_contract.rs`（新增 6 个公开契约测试）
- `tasks/TASK-276-task-engine-budget-contract-coverage.md`
- `LEDGER.md`、`docs/memory/facts.md`、`MEMORY.md`
- `PLAN.md`、`README.md`、`plans/stage-1-pilots.md`
- `docs/automations/2026-10-13-round-2.md`

### 3. 验收输出摘要

- 改动前独立实测：`assistant-task-engine` 行覆盖率 90.40%，`budget.rs` 82.02%（region 89.04%）。
- `cargo test -p assistant-task-engine --test budget_contract`：6 passed。
- `cargo llvm-cov -p assistant-task-engine --tests --fail-under-lines 91`：EXIT 0；
  crate 行覆盖率 **91.58%**（region 88.59%），1354 行中 1240 行覆盖；`budget.rs` 行覆盖率 **100.00%**。
- `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、
  `cargo test --workspace`：EXIT 0。
- xtask：`hygiene` scanned=409 / 0E / 128W / PASSED，`docscan` 0E/264W，
  `memory-counts`、`adr-index`、`check-ledger`、`card-check`、`refscan`(0E/0W)、
  `check-comments`、`verify-schemas`、`check-migrations` 全部 PASSED。
- PR #348；pull_request CI = **11/11 SUCCESS**；合并前 base=`main`、mergeable=`MERGEABLE`、
  mergeStateStatus=`CLEAN`；merge `3b2081e`。

### 4. DoD 逐条核对

- 生产逻辑零改动：满足；仅新增 `crates/task-engine/tests/budget_contract.rs`。
- 新增测试只调用公开 API 覆盖预算 fail-closed 契约：满足；6 个测试覆盖 `Budget::new`
  参数校验、`BudgetUsage` 计数器 / token / cost 溢出、`record_usage` 非法 cost、
  `Budget::check` 的 token 溢出与 `Within`。
- `cargo llvm-cov -p assistant-task-engine --tests --fail-under-lines 91`：满足，实测 91.58%。
- `cargo test -p assistant-task-engine` 全绿：满足。
- `cargo fmt --all --check` / `clippy --all-targets -- -D warnings` / `test --workspace` 全绿：满足。
- 全部现行 xtask 硬门禁 PASSED：满足。
- 热点文件 guard：满足；`LEDGER.md` / `docs/memory/facts.md` / `MEMORY.md`（收口）与
  `PLAN.md` / `README.md` / `plans/stage-1-pilots.md`（回填）先 acquire 后 release。

### 5. 偏差

- 无。

### 6. 更合理做法

- 预算核算契约单独放 `budget_contract.rs`，与 TASK-061 的预算 / 看门狗集成用例分开，
  便于独立回归 fail-closed 的溢出与非法输入分支。

### 7. 遗留问题

- `scheduler.rs`（87.50%）与 `watchdog.rs`（87.50%）的剩余未覆盖行主要是**非公开**的
  `apply_step_event` / `mutable_step` / `elapsed_ms` 错误分支，公开 API 不可达；如需继续，
  应由 crate 内 `#[cfg(test)]` 单测或后续卡承接。
- `status.rs`（89.38%）的剩余分支为状态机负向转换，可用后续 P2 卡承接。

### 8. 新增长期记忆

- `docs/memory/facts.md`：记录 `assistant-task-engine` 独立行覆盖率 90.40% → 91.58%、
  `budget.rs` 82.02% → 100.00%，并同步 `MEMORY.md` 规模表。

### 9. 给审阅者的关注点

1. 新增测试是否全部只调用公开 API，未改变生产控制流或公共接口。
2. 每个溢出 / 非法输入是否都断言了精确的 `ErrorCode` 变体。
3. 覆盖率数字是否由同一提交上的独立 `cargo llvm-cov -p assistant-task-engine` 实跑得到。
