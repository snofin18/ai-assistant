# TASK-275　tool-bus 注册期「形状拒绝」契约覆盖率补齐（章程 §2 P2）

- 状态：**Done**
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**（ADR-0037 号段 200~299）　依赖：无　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**；`- 状态：` 行按 ADR-0083 为唯一例外）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：无　**预估**：S　**难度**：S
- **write scope**：`crates/tool-bus/tests/schema_shape_contract.rs`、`tasks/TASK-275-tool-bus-schema-shape-coverage.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/memory/facts.md`、`MEMORY.md`、`docs/automations/2026-10-13-round-1.md`
- **关联**：`docs/automation-charter.md` §2 P2 / §5.3、`crates/tool-bus/README.md`、`docs/spec/testing.md`、`docs/spec/tool-schema.md`

**目标**

仅为 `assistant-tool-bus` 增加**注册期形状拒绝**的公开契约测试，把
`crates/tool-bus/src/schema/registration.rs` 的行覆盖率从 73.63% 提升到 ≥90%（crate 独立行覆盖率
从 85.62% 提升到 ≥87%）。重点覆盖 `collect_unenforceable_constructs` 对**形状不合法**的受支持关键字
（`type` / `enum` / `required` / 数值边界 / `properties` / `additionalProperties` / `items` / 组合器）
的 fail-closed 拒绝。不修改生产逻辑，不启动真实 NamedPipe、GUI、网络或读取凭据。

**背景**

2026-10-13 round 1 的独立 crate 基线为行覆盖率 85.62%，`schema/registration.rs` 仅 73.63%。
既有 `tests/schema_keywords.rs`（TASK-204）只覆盖**合法形状**与关键字表；README 承诺的
「形状不对同样算无法强制」这条 fail-closed 判据没有回归证据。候选来自
`docs/automations/2026-10-12-report.md` §③（纯逻辑、无 GUI / keychain）、按 N=1 派单。

**步骤**

1. 新建 `crates/tool-bus/tests/schema_shape_contract.rs`。
2. 覆盖非对象 / 非布尔 schema、`type` / `enum` / `required` 畸形、数值边界畸形、
   子 schema 关键字畸形与组合器空数组。
3. 覆盖嵌套子 schema 的 JSON pointer 定位，以及合法形状（布尔 / 对象子 schema）不误拒。
4. 复测 `cargo llvm-cov -p assistant-tool-bus --tests --fail-under-lines 87`。
5. 跑 crate 测试与全部全局硬门禁。

**DoD**

- [ ] 生产逻辑零改动（仅新增测试文件）
- [ ] 新增测试只调用公开 API，覆盖形状拒绝契约
- [ ] `cargo llvm-cov -p assistant-tool-bus --tests --fail-under-lines 87` 通过
- [ ] `cargo test -p assistant-tool-bus` 全绿
- [ ] `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全绿
- [ ] 全部现行 xtask 硬门禁 PASSED
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 与最终状态同批同步

**验收命令**

```powershell
cargo llvm-cov -p assistant-tool-bus --tests --fail-under-lines 87
cargo test -p assistant-tool-bus
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

【任务】TASK-275 tool-bus 注册期形状拒绝契约覆盖率补齐
【目标】仅新增集成契约测试，把 `assistant-tool-bus` 独立行覆盖率从 85.62% 提升到 ≥87%
【write scope】仅：新建 `crates/tool-bus/tests/schema_shape_contract.rs` 与本卡列出的状态同步文件
【铁律】测试不改生产逻辑；无静默失败；不扩 scope；热点文件先 guard
【禁止】改公共接口、依赖、CI/lint、生产行为；启动真实 NamedPipe / GUI / 网络 / 凭据
【验收】见「验收命令」→ crate 覆盖率 + 全部硬门禁
【依赖】无；章程 §2 P2 / §5.3；运行日 2026-10-13 round 1
【疑问】无

### 2. 实际改动文件

- `crates/tool-bus/tests/schema_shape_contract.rs`（新增 6 个公开契约测试）
- `tasks/TASK-275-tool-bus-schema-shape-coverage.md`
- `LEDGER.md`、`docs/memory/facts.md`、`MEMORY.md`
- `PLAN.md`、`README.md`、`plans/stage-1-pilots.md`
- `docs/automations/2026-10-13-round-1.md`

### 3. 验收输出摘要

- 改动前独立实测：`assistant-tool-bus` 行覆盖率 85.62%，`schema/registration.rs` 73.63%
  （region 81.21%）。
- `cargo test -p assistant-tool-bus --test schema_shape_contract`：6 passed。
- `cargo llvm-cov -p assistant-tool-bus --tests --fail-under-lines 87`：EXIT 0；
  crate 行覆盖率 **87.93%**（region 89.59%）；`schema/registration.rs` 行覆盖率
  **95.02%**（region 98.58%）。
- `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、
  `cargo test --workspace`：EXIT 0。
- xtask：`hygiene` scanned=409 / 0E / 128W / PASSED，`docscan` 0E/264W，
  `memory-counts`、`adr-index`、`check-ledger`、`card-check`、`refscan`(0E/0W)、
  `check-comments`、`verify-schemas`、`codegen --check`、`check-migrations` 全部 PASSED。
- PR #346；pull_request CI = **11/11 SUCCESS**；合并前 base=`main`、mergeable=`MERGEABLE`、
  mergeStateStatus=`CLEAN`；merge `85973c5`。

### 4. DoD 逐条核对

- 生产逻辑零改动：满足；仅新增 `crates/tool-bus/tests/schema_shape_contract.rs`。
- 新增测试只调用公开 API 覆盖形状拒绝契约：满足；6 个测试覆盖非对象 / 非布尔 schema、
  `type` / `enum` / `required` 畸形、数值边界畸形、`properties` / `additionalProperties` /
  `items` 畸形、组合器非数组 / 空数组、嵌套 JSON pointer 与合法形状放行。
- `cargo llvm-cov -p assistant-tool-bus --tests --fail-under-lines 87`：满足，实测 87.93%。
- `cargo test -p assistant-tool-bus` 全绿：满足。
- `cargo fmt --all --check` / `clippy --all-targets -- -D warnings` / `test --workspace` 全绿：满足。
- 全部现行 xtask 硬门禁 PASSED：满足。
- 热点文件 guard：满足；`LEDGER.md` / `docs/memory/facts.md` / `MEMORY.md`（收口）与
  `PLAN.md` / `README.md` / `plans/stage-1-pilots.md`（回填）先 acquire 后 release。

### 5. 偏差

- 无。

### 6. 更合理做法

- 形状拒绝契约单独放 `schema_shape_contract.rs`，与 TASK-204 的合法形状 / 关键字表用例分开，
  便于独立回归 fail-closed 判据。

### 7. 遗留问题

- `bus.rs`（75.82%）与 `handler.rs`（75.38%）的剩余缺口分别是私有错误映射（需真实 transport）
  与平凡访问器，未纳入本卡；如需继续可用后续 P2 卡承接。

### 8. 新增长期记忆

- `docs/memory/facts.md`：记录 `assistant-tool-bus` 独立行覆盖率 85.62% → 87.93%、
  `schema/registration.rs` 73.63% → 95.02%，并同步 `MEMORY.md` 规模表。

### 9. 给审阅者的关注点

1. 新增测试是否全部只调用公开 API，未改变生产控制流或公共接口。
2. 每个畸形形状是否都断言了精确可读的拒绝原因与 JSON pointer。
3. 覆盖率数字是否由同一提交上的独立 `cargo llvm-cov -p assistant-tool-bus` 实跑得到。
