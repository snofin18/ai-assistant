# TASK-278　agent-core runtime dataflow 公共契约覆盖率补齐（章程 §2 P2）

- 状态：**Done**
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**（ADR-0037 号段 200~299）　依赖：无　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**；`- 状态：` 行按 ADR-0083 为唯一例外）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：无　**预估**：S　**难度**：S
- **write scope**：`apps/agent-core/tests/runtime_dataflow_contract.rs`、`tasks/TASK-278-agent-core-runtime-dataflow-contract-coverage.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/memory/facts.md`、`MEMORY.md`、`docs/automations/2026-10-13-round-4.md`、`docs/automations/2026-10-13-report.md`
- **关联**：`docs/automation-charter.md` §2 P2 / §5.3、`docs/adr/0061-runtime-task-dataflow-and-resumable-approval.md`、`docs/spec/runtime-execution.md`

**目标**

仅为 `assistant-agent-core` 的 binary-layer task dataflow（ADR-0061）增加公开契约测试，把
`apps/agent-core/src/runtime_dataflow.rs` 的行覆盖率从 77.12% 提升到 ≥90%（crate 独立行覆盖率保持
≥75%）。重点覆盖 `RuntimeDataflowPlan` 的重复 id / 访问器、`parse_condition` 的闭集拒绝、
比较运算符全矩阵与 `TypeMismatch`、布尔谓词的非布尔失败、`resolve_references` /
`collect_references` 的失败关闭路径。不修改生产逻辑，不启动真实 IO / 网络 / GUI / 凭据。

**背景**

2026-10-13 round 4 的 `runtime_dataflow.rs` 行覆盖率为 77.12%。既有 in-crate 单测
（`runtime_dataflow_tests.rs`）只覆盖了整值引用、部分插值拒绝、闭集解析、求值与递归收集的**happy path**；
重复 step、比较运算符矩阵、类型/布尔失败、引用解析失败等分支无回归证据。候选来自
`docs/automations/2026-10-12-report.md` §③（`apps/agent-core` 按模块拆卡）、按 §2 P2 派单。

**步骤**

1. 新建 `apps/agent-core/tests/runtime_dataflow_contract.rs`。
2. 覆盖 `RuntimeDataflowPlan::{new, insert, get, step_ids}`（含重复 id → `DuplicateStep`）。
3. 覆盖 `parse_condition` 的闭集拒绝、六个比较运算符、`TypeMismatch`、非布尔与未知引用。
4. 覆盖 `resolve_references` / `collect_references` 的递归、字面量直通与失败关闭。
5. 复测 `cargo llvm-cov -p assistant-agent-core --tests --fail-under-lines 75` 并跑全部全局硬门禁。

**DoD**

- [ ] 生产逻辑零改动（仅新增测试文件）
- [ ] 新增测试只调用公开 API，覆盖 dataflow 契约
- [ ] `cargo llvm-cov -p assistant-agent-core --tests --fail-under-lines 75` 通过，且 `runtime_dataflow.rs` 行覆盖率 ≥90%
- [ ] `cargo test -p assistant-agent-core` 全绿
- [ ] `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全绿
- [ ] 全部现行 xtask 硬门禁 PASSED
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 与最终状态同批同步

**验收命令**

```powershell
cargo llvm-cov -p assistant-agent-core --tests --fail-under-lines 75
cargo test -p assistant-agent-core
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

【任务】TASK-278 agent-core runtime dataflow 公共契约覆盖率补齐
【目标】仅新增集成契约测试，把 `runtime_dataflow.rs` 行覆盖率从 77.12% 提升到 ≥90%
【write scope】仅：新建 `apps/agent-core/tests/runtime_dataflow_contract.rs` 与本卡列出的状态同步文件
【铁律】测试不改生产逻辑；无静默失败；不扩 scope；热点文件先 guard
【禁止】改公共接口、依赖、CI/lint、生产行为；启动真实 IO / 网络 / GUI / 凭据
【验收】见「验收命令」→ crate 覆盖率 + 模块覆盖率 + 全部硬门禁
【依赖】无；章程 §2 P2 / §5.3；运行日 2026-10-13 round 4（**机制 = human**，ADR-0089 后人类会话不计入 `N_auto`）
【疑问】无

### 2. 实际改动文件

- `apps/agent-core/tests/runtime_dataflow_contract.rs`（新增 9 个公开契约测试）
- `tasks/TASK-278-agent-core-runtime-dataflow-contract-coverage.md`
- `LEDGER.md`、`docs/memory/facts.md`、`MEMORY.md`
- `PLAN.md`、`README.md`、`plans/stage-1-pilots.md`
- `docs/automations/2026-10-13-round-4.md`、`docs/automations/2026-10-13-report.md`

### 3. 验收输出摘要

- 改动前独立实测：`apps/agent-core` 行覆盖率 75.32%，`runtime_dataflow.rs` 77.12%（region 77.59%）。
- `cargo test -p assistant-agent-core --test runtime_dataflow_contract`：9 passed。
- `cargo llvm-cov -p assistant-agent-core --tests --fail-under-lines 75`：EXIT 0；
  crate 行覆盖率 **76.02%**；`runtime_dataflow.rs` 行覆盖率 **96.73%**（region 95.99%）。
- `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、
  `cargo test --workspace`：EXIT 0。
- xtask：`hygiene` scanned=409 / 0E / 128W / PASSED，`docscan` 0E/264W，
  `memory-counts`、`adr-index`、`check-ledger`、`card-check`、`refscan`(0E/0W)、
  `check-comments`、`verify-schemas`、`check-migrations` 全部 PASSED。
- PR #354；pull_request CI = **11/11 SUCCESS**；合并前 base=`main`、mergeable=`MERGEABLE`、
  mergeStateStatus=`CLEAN`；merge `719699a`。

### 4. DoD 逐条核对

- 生产逻辑零改动：满足；仅新增 `apps/agent-core/tests/runtime_dataflow_contract.rs`。
- 新增测试只调用公开 API 覆盖 dataflow 契约：满足；9 个测试覆盖计划重复 id / 访问器、
  闭集拒绝、比较运算符矩阵、类型 / 布尔失败、引用解析与收集的失败关闭。
- `cargo llvm-cov -p assistant-agent-core --tests --fail-under-lines 75` 且 `runtime_dataflow.rs` ≥90%：
  满足，crate 76.02%、模块 96.73%。
- `cargo test -p assistant-agent-core` 全绿：满足。
- `cargo fmt --all --check` / `clippy --all-targets -- -D warnings` / `test --workspace` 全绿：满足。
- 全部现行 xtask 硬门禁 PASSED：满足。
- 热点文件 guard：满足；`LEDGER.md` / `docs/memory/facts.md` / `MEMORY.md`（收口）与
  `PLAN.md` / `README.md` / `plans/stage-1-pilots.md`（回填）先 acquire 后 release。

### 5. 偏差

- 无。

### 6. 更合理做法

- dataflow 契约单独放 `runtime_dataflow_contract.rs`，与 in-crate happy-path 单测分开，
  便于独立回归闭集拒绝与失败关闭分支。

### 7. 遗留问题

- `storage_session_store.rs`（76.19%）与 `ui_server.rs`（67.20%）仍低；前者依赖真实存储、后者属 UI
  传输层，如需继续应由后续卡按模块单独评估。

### 8. 新增长期记忆

- `docs/memory/facts.md`：记录 `runtime_dataflow.rs` 行覆盖率 77.12% → 96.73%、crate 75.32% → 76.02%，
  并同步 `MEMORY.md` 规模表。

### 9. 给审阅者的关注点

1. 新增测试是否全部只调用公开 API，未改变生产控制流或公共接口。
2. 比较运算符矩阵 / 类型不匹配 / 非布尔 / 未知引用是否都断言了精确的 `DataflowError` 变体。
3. 覆盖率数字是否由同一提交上的独立 `cargo llvm-cov -p assistant-agent-core` 实跑得到。
