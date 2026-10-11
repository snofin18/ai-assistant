# TASK-265　assistant-hitl 契约测试覆盖率加固（章程 §2 P2）

- 状态：**Done**（2026-10-11）
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**（ADR-0037 号段 200~299）　依赖：无　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**；`- 状态：` 行按 ADR-0083 为唯一例外）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：无　**预估**：S　**难度**：S
- **write scope**：`crates/hitl/tests/coverage_contract.rs`、`tasks/TASK-265-hitl-coverage-hardening.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/memory/facts.md`、`MEMORY.md`、`docs/automations/2026-10-11-round-3.md`
- **关联**：`docs/automation-charter.md` §2 P2 / §5.3、`crates/hitl/README.md`

**目标**

仅为 `assistant-hitl` 增加白盒契约测试，把该 crate 的独立行覆盖率提升到 ≥85%，
重点覆盖审批请求构造 / 解析失败关闭、四维授权拒绝矩阵、稳定错误码映射与少量 diff 边界；
不修改生产逻辑，不读取真实桌面，不启动任何 GUI。

**背景**

2026-10-11 round 3 的独立实测基线为 `assistant-hitl` 行覆盖率 77.93%
（725 行中覆盖 565 行）；`approval.rs` 63.24%、`authorization.rs` 69.35%、
`error.rs` 50.00% 仍有多条公开契约分支未覆盖。当前 Ready 队列均为真实 GUI
或人工安全设计工作，按 §2 P2 做可机器验证的测试强化。

**步骤**

1. 新建 `crates/hitl/tests/coverage_contract.rs`。
2. 覆盖 `ApprovalScope` 解析 / 拼写、工具与目标维度校验、请求输入边界。
3. 覆盖审批拒绝 / 超时 / 未提供 scope / 重复 scope / TTL 与 uses 超限路径。
4. 覆盖四维授权拒绝矩阵、grant 消费失败与高风险 scope 校验。
5. 覆盖 `HitlError::error_code` 全部公开变体映射与 diff 构造边界。
6. 复测 `cargo llvm-cov -p assistant-hitl --tests --fail-under-lines 85`。
7. 跑 `cargo test -p assistant-hitl` 与全部全局硬门禁。

**DoD**

- [ ] 生产逻辑零改动（仅修改测试文件）
- [ ] 新增测试覆盖审批、授权、错误码与 diff 的公开失败关闭契约
- [ ] `cargo llvm-cov -p assistant-hitl --tests --fail-under-lines 85` 通过
- [ ] `cargo test -p assistant-hitl` 全绿
- [ ] `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全绿
- [ ] 全部现行 xtask 硬门禁 PASSED
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 与最终状态同批同步

**验收命令**

```powershell
cargo llvm-cov -p assistant-hitl --tests --fail-under-lines 85
cargo test -p assistant-hitl
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

【任务】TASK-265 assistant-hitl 契约测试覆盖率加固
【目标】仅新增测试，把 `assistant-hitl` 独立行覆盖率提升到 ≥85%
【write scope】仅：本卡正文所列文件
【铁律】测试不改生产逻辑；无静默失败；不扩 scope；热点文件先 guard
【禁止】改生产行为、契约、依赖、CI/lint；读取真实桌面 / GUI / 网络 / 凭据
【验收】见「验收命令」→ crate 覆盖率 + 全部硬门禁
【依赖】无；章程 §2 P2 / §5.3
【疑问】无

### 2. 实际改动文件

- `crates/hitl/tests/coverage_contract.rs`
- `tasks/TASK-265-hitl-coverage-hardening.md`
- `LEDGER.md`
- `PLAN.md`、`README.md`、`plans/stage-1-pilots.md`
- `docs/memory/facts.md`、`MEMORY.md`（规模表同步）
- `docs/automations/2026-10-11-round-3.md`

### 3. 验收输出摘要

- 改动前独立实测：`assistant-hitl` 行覆盖率 77.93%（725 行中覆盖 565 行）。
- `cargo llvm-cov -p assistant-hitl --tests --fail-under-lines 85`：EXIT 0；
  725 行中覆盖 667 行，行覆盖率 **92.00%**；`approval.rs` 95.68%、
  `authorization.rs` 86.93%、`control.rs` 90.53%、`diff.rs` 94.09%、
  `error.rs` 100%。
- `cargo test -p assistant-hitl`：新增 6 个 coverage 契约测试与既有 18 个测试全绿。
- `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、
  `cargo test --workspace`：EXIT 0，workspace 469 passed / 0 failed。
- `xtask` 现行硬门禁全部 PASSED；`cargo deny check` 为 advisories / bans / licenses / sources 全 ok；
  `cargo test -p assistant-core arch::` 10 passed；`replay --suite core` 2 steps / 0 mismatch。
- PR #308：pull_request CI run `38115879774` = **11/11 SUCCESS**；合并前 base=`main`、
  mergeable=`MERGEABLE`、mergeStateStatus=`CLEAN`；merge `4e9b573`。

### 4. DoD 逐条核对

- 生产逻辑零改动：满足；只新增 `crates/hitl/tests/coverage_contract.rs`；PR #308 / merge `4e9b573`。
- 审批、授权、错误码与 diff 的公开失败关闭契约：满足；新增 6 个公共 API 契约测试。
- crate 独立行覆盖率 ≥85%：满足，实测 92.00%。
- `cargo test -p assistant-hitl` 与全 workspace 门禁：满足。
- 全部现行 xtask 硬门禁：满足，见本卡验收输出与轮次报告。
- 热点文件 guard：满足，`LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` /
  `docs/memory/facts.md` / `MEMORY.md` 均先 acquire 后编辑并 release。

### 5. 偏差

- 初稿测试文件分别为 599 / 610 行，超过章程 §6 的单卡 400 行审阅预算；
  已压缩为 393 行表驱动版本，仍达到 92.00% 行覆盖率，未留下预算偏差。

### 6. 更合理做法

- 将稳定的错误码映射断言收进局部宏，并用表驱动覆盖输入边界，避免重复的长结构体构造。

### 7. 遗留问题

- PR #308 / CI run `38115879774` / merge `4e9b573` 已回填；真机 Paint 验收仍由 TASK-256 / TASK-044 的有人值守流程承接。

### 8. 新增长期记忆

- `docs/memory/facts.md`：记录 `assistant-hitl` 独立行覆盖率从 77.93% 提升到 92.00%，
  并同步 `MEMORY.md` 规模表。

### 9. 给审阅者的关注点

1. 新增测试是否全部只调用公开 API，生产源码零改动。
2. 审批与授权失败路径是否断言了具体错误变体和覆盖边界，而不是只断言“没有 panic”。
3. 覆盖率数字是否由同一提交上的独立 `cargo llvm-cov -p assistant-hitl` 实跑得到。
