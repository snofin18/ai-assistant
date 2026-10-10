# TASK-261　capture 契约测试覆盖率加固（章程 §5.3）

- 状态：**Review**
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**（ADR-0037 号段 200~299）　依赖：无　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**；`- 状态：` 行按 ADR-0083 为唯一例外）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：无　**预估**：S　**难度**：S
- **write scope**：`crates/capture/tests/**`、`tasks/TASK-261-capture-coverage-hardening.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/memory/facts.md`、`MEMORY.md`、`docs/automations/2026-10-10-round-5.md`
- **关联**：`docs/automation-charter.md` §2 P2 / §5.3、`crates/capture/README.md`

**目标**

仅为 `crates/capture` 增加白盒契约测试，把该 crate 的行覆盖率提升到 ≥90%，重点覆盖
`CaptureError` 的稳定错误码、Display/source 链、平台错误保真，以及
`ScrollCleanupError` 的稳定错误码与 Display；不修改生产逻辑。

**背景**

自动化运行日 2026-10-10 round 5 的实测基线为 `assistant-capture` 行覆盖率
71.58%（95 行中覆盖 68 行）。该 crate 是 ADR-0071 冻结的截图隐私边界，
`error.rs` 当前 0% 覆盖，滚动计划错误也只验证了 `matches!` 而未验证稳定错误码和展示文本。
当前 Ready 队列均需真实 GUI 或人工安全设计判断，故按 §2 P2 做可机器验证的测试强化。

**步骤**

1. 新增 `crates/capture/tests/coverage_contract.rs`，只调用现有公共 API。
2. 覆盖 `CaptureError::code` 对 redaction 与 platform 两类错误的分类。
3. 覆盖 `CaptureError` 的 `Display` 与 `std::error::Error::source` 链。
4. 覆盖平台错误穿过 pipeline 后仍保留原始 `ErrorCode`。
5. 覆盖 `ScrollCleanupError::code` 与两种错误的 `Display` 文本。
6. 复测 `cargo llvm-cov -p assistant-capture --tests`，确认 crate 行覆盖率 ≥90%，
   再跑全局门禁。

**DoD**

- [ ] 生产代码 `crates/capture/src/**` 零改动
- [ ] 新增测试覆盖上述五类契约，正常 / 边界 / 错误路径均存在
- [ ] `cargo llvm-cov -p assistant-capture --tests --fail-under-lines 90` 通过
- [ ] `cargo test -p assistant-capture` 全绿
- [ ] `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全绿
- [ ] 全部现行 xtask 硬门禁 PASSED
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 与最终状态同批同步

**验收命令**

```powershell
cargo llvm-cov -p assistant-capture --tests --fail-under-lines 90
cargo test -p assistant-capture
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

【任务】TASK-261 capture 覆盖率加固
【目标】仅新增契约测试，把 `crates/capture` 行覆盖率提升到 ≥90%
【write scope】仅：本卡正文所列文件
【铁律】测试不改生产逻辑；无静默失败；不扩 scope；热点文件先 guard
【禁止】改 `src/**`、契约、依赖、CI/lint；操作真实 GUI/网络
【验收】见「验收命令」→ crate 覆盖率 + 全部硬门禁
【依赖】无；章程 §2 P2 / §5.3
【疑问】无

### 2. 实际改动文件

- `crates/capture/tests/coverage_contract.rs`（新增，仅测试）
- `tasks/TASK-261-capture-coverage-hardening.md`
- `LEDGER.md`
- `PLAN.md`、`README.md`、`plans/stage-1-pilots.md`
- `docs/memory/facts.md`、`MEMORY.md`（规模表同步）
- `docs/automations/2026-10-10-round-5.md`

### 3. 验收输出摘要

- 改动前 workspace 实测 `assistant-capture`：95 行中覆盖 68 行，**71.58%**。
- `cargo test -p assistant-capture --test coverage_contract`：4 passed / 0 failed。
- `cargo llvm-cov -p assistant-capture --tests --fail-under-lines 90`：EXIT 0；
  95 行中覆盖 92 行，**96.84%**。
- `cargo test -p assistant-capture`：11 integration tests，全绿。
- `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、
  `cargo test --workspace --quiet`：EXIT 0。
- PR / CI / merge hash：待本轮 PR 收口。

### 4. DoD 逐条核对

- 生产源码零改动：满足，`crates/capture/src/**` 未改。
- 正常 / 边界 / 错误路径覆盖：满足，4 个契约测试覆盖 redaction / platform / scroll
  错误分类、稳定展示文案与 source 链。
- crate 行覆盖率 ≥90%：满足，实测 96.84%。
- `cargo test -p assistant-capture` 与全 workspace 门禁：满足。
- 热点文件 guard：满足，六个热点文件先 acquire 后编辑并 release。
- 最终状态：**Review**；PR / CI / merge hash 待回填。

### 5. 偏差

- 本轮曾重复把只读命令误发到 `shell=bash` 并得到 exit 127；随后均改用
  `shell=powershell` 重跑，失败发生在写入前，未污染工作区。

### 6. 更合理做法

- 新增测试独立成文件，覆盖错误枚举的公共契约而不是继续堆入既有截图流程测试，
  更容易审阅且不影响生产路径。

### 7. 遗留问题

- TASK-256 / TASK-257 / TASK-044 的真实 Paint GUI 验收仍由有人在场时执行。

### 8. 新增长期记忆

- `docs/memory/facts.md`：记录 `assistant-capture` 契约覆盖率从 71.58% 提升到 96.84%；
  同步更新 `MEMORY.md` 规模表并让 `memory-counts` 恢复 0E/0W。

### 9. 给审阅者的关注点

1. 新增文件是否仅调用现有公共 API，且没有改生产源码或放宽断言。
2. redaction / platform / scroll 三类错误码与展示文本是否覆盖稳定契约。
3. 覆盖率数字是否由 `cargo llvm-cov -p assistant-capture --tests` 在同一提交上实跑得到。
