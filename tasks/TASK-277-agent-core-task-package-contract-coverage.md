# TASK-277　agent-core task-package provider 公共契约覆盖率补齐（章程 §2 P2）

- 状态：**Done**
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**（ADR-0037 号段 200~299）　依赖：无　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**；`- 状态：` 行按 ADR-0083 为唯一例外）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：无　**预估**：S　**难度**：S
- **write scope**：`apps/agent-core/tests/task_package_contract.rs`、`tasks/TASK-277-agent-core-task-package-contract-coverage.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/memory/facts.md`、`MEMORY.md`、`docs/automations/2026-10-13-round-3.md`、`docs/automations/2026-10-13-report.md`
- **关联**：`docs/automation-charter.md` §2 P2 / §5.3、`docs/adr/0058-production-composition-root-and-plan-source.md`、`docs/spec/runtime-execution.md`

**目标**

仅为 `assistant-agent-core` 的确定性 task-package Plan source 增加 provider 级公开契约测试，把
`apps/agent-core/src/task_package.rs` 的行覆盖率从 75.00% 提升到 ≥90%（crate 独立行覆盖率保持
≥75%）。重点覆盖 provider 元数据（`model_id` / `capabilities` / `pricing` / `count_tokens`）、
声明身份访问器（`task_id` / `declared_task_id` / `goal`）、缺省 goal 生成、`from_package_file`
的读取失败，以及 `complete` 的取消边界。不修改生产逻辑，不启动真实模型 / 网络 / GUI / 凭据。

**背景**

2026-10-13 round 3 的 `apps/agent-core/src/task_package.rs` 行覆盖率为 75.00%。既有
`tests/task_package.rs` 覆盖了渲染出的 plan / dataflow / 断言表与错误分支，但没有覆盖 provider
元数据、身份访问器、缺省 goal、文件读取失败与取消边界。候选来自
`docs/automations/2026-10-12-report.md` §③（`apps/agent-core` 按模块拆卡）、按 N=3 派单。

**步骤**

1. 新建 `apps/agent-core/tests/task_package_contract.rs`。
2. 覆盖 provider 元数据：`model_id` / `capabilities`（streaming + context）/ `pricing` / `count_tokens`。
3. 覆盖身份访问器与缺省 goal（缺失 / 空白 goal → 确定性生成）。
4. 覆盖 `from_package_file` 的 `Read` 失败与 `complete` 的取消边界。
5. 复测 `cargo llvm-cov -p assistant-agent-core --tests --fail-under-lines 75` 并跑全部全局硬门禁。

**DoD**

- [ ] 生产逻辑零改动（仅新增测试文件）
- [ ] 新增测试只调用公开 API，覆盖 provider 契约
- [ ] `cargo llvm-cov -p assistant-agent-core --tests --fail-under-lines 75` 通过，且 `task_package.rs` 行覆盖率 ≥90%
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

【任务】TASK-277 agent-core task-package provider 公共契约覆盖率补齐
【目标】仅新增集成契约测试，把 `task_package.rs` 行覆盖率从 75.00% 提升到 ≥90%
【write scope】仅：新建 `apps/agent-core/tests/task_package_contract.rs` 与本卡列出的状态同步文件
【铁律】测试不改生产逻辑；无静默失败；不扩 scope；热点文件先 guard
【禁止】改公共接口、依赖、CI/lint、生产行为；启动真实模型 / 网络 / GUI / 凭据
【验收】见「验收命令」→ crate 覆盖率 + 模块覆盖率 + 全部硬门禁
【依赖】无；章程 §2 P2 / §5.3；运行日 2026-10-13 round 3
【疑问】无

### 2. 实际改动文件

- `apps/agent-core/tests/task_package_contract.rs`（新增 6 个公开契约测试）
- `tasks/TASK-277-agent-core-task-package-contract-coverage.md`
- `LEDGER.md`、`docs/memory/facts.md`、`MEMORY.md`
- `PLAN.md`、`README.md`、`plans/stage-1-pilots.md`
- `docs/automations/2026-10-13-round-3.md`、`docs/automations/2026-10-13-report.md`

### 3. 验收输出摘要

- 改动前独立实测：`apps/agent-core` 行覆盖率 75.04%，`task_package.rs` 75.00%（region 73.05%）。
- `cargo test -p assistant-agent-core --test task_package_contract`：6 passed。
- `cargo llvm-cov -p assistant-agent-core --tests --fail-under-lines 75`：EXIT 0；
  crate 行覆盖率 **75.32%**；`task_package.rs` 行覆盖率 **94.35%**（region 94.33%）。
- `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、
  `cargo test --workspace`：EXIT 0。
- xtask：`hygiene` scanned=409 / 0E / 128W / PASSED，`docscan` 0E/264W，
  `memory-counts`、`adr-index`、`check-ledger`、`card-check`、`refscan`(0E/0W)、
  `check-comments`、`verify-schemas`、`check-migrations` 全部 PASSED。
- PR #350；pull_request CI = **11/11 SUCCESS**；合并前 base=`main`、mergeable=`MERGEABLE`、
  mergeStateStatus=`CLEAN`；merge `441212f`。

### 4. DoD 逐条核对

- 生产逻辑零改动：满足；仅新增 `apps/agent-core/tests/task_package_contract.rs`。
- 新增测试只调用公开 API 覆盖 provider 契约：满足；6 个测试覆盖元数据、身份访问器、
  缺省 goal、文件读取失败与取消边界。
- `cargo llvm-cov -p assistant-agent-core --tests --fail-under-lines 75` 且 `task_package.rs` ≥90%：
  满足，crate 75.32%、模块 94.35%。
- `cargo test -p assistant-agent-core` 全绿：满足。
- `cargo fmt --all --check` / `clippy --all-targets -- -D warnings` / `test --workspace` 全绿：满足。
- 全部现行 xtask 硬门禁 PASSED：满足。
- 热点文件 guard：满足；`LEDGER.md` / `docs/memory/facts.md` / `MEMORY.md`（收口）与
  `PLAN.md` / `README.md` / `plans/stage-1-pilots.md`（回填）先 acquire 后 release。

### 5. 偏差

- 无。

### 6. 更合理做法

- provider 级契约单独放 `task_package_contract.rs`，与既有的 plan / dataflow / 断言表用例分开，
  便于独立回归元数据、访问器、缺省值与取消边界。

### 7. 遗留问题

- `reserved_host.rs`（55.15%）与 `ui_server.rs`（67.20%）仍低，但前者经 `reserved_invoker` 私有嵌套、
  公开 API 不可达，后者属 UI 传输层；如需继续应由后续卡按模块单独评估。

### 8. 新增长期记忆

- `docs/memory/facts.md`：记录 `task_package.rs` 行覆盖率 75.00% → 94.35%、crate 75.04% → 75.32%，
  并同步 `MEMORY.md` 规模表。

### 9. 给审阅者的关注点

1. 新增测试是否全部只调用公开 API，未改变生产控制流或公共接口。
2. provider 元数据 / 访问器 / 缺省 goal / 取消边界是否都断言了可观察结果。
3. 覆盖率数字是否由同一提交上的独立 `cargo llvm-cov -p assistant-agent-core` 实跑得到。
