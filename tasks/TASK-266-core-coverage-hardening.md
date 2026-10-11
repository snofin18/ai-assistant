# TASK-266　assistant-core 契约测试覆盖率加固（章程 §2 P2）

- 状态：**Review**
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**（ADR-0037 号段 200~299）　依赖：无　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**；`- 状态：` 行按 ADR-0083 为唯一例外）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：无　**预估**：S　**难度**：S
- **write scope**：`crates/core/tests/coverage_contract.rs`、`tasks/TASK-266-core-coverage-hardening.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/memory/facts.md`、`MEMORY.md`、`docs/automations/2026-10-11-round-4.md`
- **关联**：`docs/automation-charter.md` §2 P2 / §5.3、`crates/core/README.md`

**目标**

仅为 `assistant-core` 增加白盒契约测试，把该 crate 的独立行覆盖率提升到 ≥90%，
重点覆盖内存会话存储的插入 / 更新 / 缺失分支、核心错误码与 Display 契约、
clean-context review 的输入边界、解析失败关闭和 provider 事件失败关闭；
不修改生产逻辑，不读取真实桌面，不启动任何 GUI。

**背景**

2026-10-11 round 4 的独立实测基线为 `assistant-core` 行覆盖率 86.16%
（2479 行中覆盖 2136 行）；`store.rs` 70.49%、`verify_review.rs` 68.35%、
`planner.rs` 80.00%、`error.rs` 87.83% 仍有公开契约分支未覆盖。当前 Ready
队列均为真实 GUI 或人工安全设计工作，按 §2 P2 做可机器验证的测试强化。

**步骤**

1. 新建 `crates/core/tests/coverage_contract.rs`。
2. 覆盖 `MemorySessionStore` 的插入、读取、更新、重复插入、缺失更新与 revision 冲突。
3. 覆盖 `CoreError` 的稳定 error_code / reason_code / Display 契约。
4. 覆盖 `HighRiskStepSummary` 与 reviewer 超时参数的全部拒绝边界。
5. 覆盖 clean-context review 的空输出、非对象、字段缺失 / 多余、错误类型与非法 verdict。
6. 覆盖 provider 空流、非 Stop 终止、非法事件和正常 Stop 的失败关闭 / 成功路径。
7. 复测 `cargo llvm-cov -p assistant-core --tests --fail-under-lines 90`。
8. 跑 `cargo test -p assistant-core` 与全部全局硬门禁。

**DoD**

- [ ] 生产逻辑零改动（仅新增测试文件）
- [ ] 新增测试覆盖会话存储、核心错误与 clean-context review 的公开失败关闭契约
- [ ] `cargo llvm-cov -p assistant-core --tests --fail-under-lines 90` 通过
- [ ] `cargo test -p assistant-core` 全绿
- [ ] `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全绿
- [ ] 全部现行 xtask 硬门禁 PASSED
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 与最终状态同批同步

**验收命令**

```powershell
cargo llvm-cov -p assistant-core --tests --fail-under-lines 90
cargo test -p assistant-core
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

【任务】TASK-266 assistant-core 契约测试覆盖率加固
【目标】仅新增测试，把 `assistant-core` 独立行覆盖率提升到 ≥90%
【write scope】仅：本卡正文所列文件
【铁律】测试不改生产逻辑；无静默失败；不扩 scope；热点文件先 guard
【禁止】改生产行为、契约、依赖、CI/lint；读取真实桌面 / GUI / 网络 / 凭据
【验收】见「验收命令」→ crate 覆盖率 + 全部硬门禁
【依赖】无；章程 §2 P2 / §5.3
【疑问】无

### 2. 实际改动文件

- `crates/core/tests/coverage_contract.rs`
- `tasks/TASK-266-core-coverage-hardening.md`
- `LEDGER.md`
- `PLAN.md`、`README.md`、`plans/stage-1-pilots.md`
- `docs/memory/facts.md`、`MEMORY.md`
- `docs/automations/2026-10-11-round-4.md`

### 3. 验收输出摘要

- 改动前独立实测：`assistant-core` 行覆盖率 86.16%（2479 行中覆盖 2136 行）。
- `cargo llvm-cov -p assistant-core --tests --fail-under-lines 90`：EXIT 0；
  2479 行中覆盖 2232 行，行覆盖率 **90.04%**；`store.rs` 80.33%、
  `verify_review.rs` 91.92%、`error.rs` 93.65%、`message.rs` 92.22%。
- `cargo test -p assistant-core`：新增 8 个 coverage 契约测试与既有测试全绿。
- `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、
  `cargo test --workspace`：EXIT 0，workspace 469 passed / 0 failed。

### 4. DoD 逐条核对

- 生产逻辑零改动：满足；只新增 `crates/core/tests/coverage_contract.rs`。
- 会话存储、核心错误与 clean-context review 的公开失败关闭契约：满足；
  8 个契约测试覆盖成功、重复、缺失、revision 冲突 / 溢出、错误码、解析与 provider 事件。
- crate 独立行覆盖率 ≥90%：满足，实测 90.04%。
- `cargo test -p assistant-core` 与全 workspace 门禁：满足。
- 全部现行 xtask 硬门禁：待本提交运行后填写。
- 热点文件 guard：满足，六个公共热点文件均先 acquire 后编辑。

### 5. 偏差

- 初稿 `test_core_error_codes_reason_codes_and_display_are_stable` 单函数 224 行，
  触发 `clippy::too_many_lines`；已拆成三个 case helper，保持测试语义与覆盖率不变。
- `crates/core/tests/coverage_contract.rs` 最终 719 行，超过章程 §6 的单卡 400 行
  审阅预算；测试覆盖保持 90.04%，未删断言。`hygiene` 因此从 123 → 124 Warning（0 Error，PASSED）。

### 6. 更合理做法

- 把错误矩阵按输入 / 规划 / 记忆三组拆分，既降低单函数长度，也让失败语义更容易审阅。

### 7. 遗留问题

- PR / CI / merge hash 待本轮提交与合并后回填；真机 Paint 验收仍由 TASK-256 / TASK-044
  的有人值守流程承接。

### 8. 新增长期记忆

- `docs/memory/facts.md`：记录 `assistant-core` 独立行覆盖率从 86.16% 提升到 90.04%，
  并同步 `MEMORY.md` 规模表。

### 9. 给审阅者的关注点

1. 新增测试是否只调用公开 API，生产源码零改动。
2. 会话存储的重复 / 缺失 / revision 冲突 / 溢出路径是否断言了稳定 reason code。
3. clean-context review 的非法输出与 provider 事件是否保持 fail-closed，而不是把空输出当作成功。
