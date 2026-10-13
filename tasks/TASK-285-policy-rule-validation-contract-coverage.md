# TASK-285　policy 规则模型校验契约覆盖率补齐（章程 §2 P2）

- 状态：**Done**
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**　依赖：TASK-024 / ADR-0088 Done　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）；分界线以上正文只读，`- 状态：` 行按 ADR-0083 例外。

---

- **write scope**：`crates/policy/tests/model_validation_contract.rs`、`tasks/TASK-285-policy-rule-validation-contract-coverage.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/automations/2026-10-13-round-11.md`、`docs/automations/2026-10-13-report.md`
- **关联**：`docs/automation-charter.md` §2 P2 / §5、`crates/policy/src/model.rs`、`crates/policy/README.md`、`docs/spec/naming.md`

**目标**

仅为 `crates/policy/src/model.rs` 的纯规则模型（`PolicyRule` / `RuleConditions`）增加跨平台公共
契约测试，直接覆盖规则 id 形状、空条件列表、确认 scope 重复、deny 原因上下界，以及
`RuleConditions::matches` 的每个约束字段；把 policy crate 行覆盖率提升到 ≥85% 门槛以上，
不修改生产逻辑、不新增依赖。

**背景**

policy 是铁律 3「唯一放行点」的实现，`docs/spec/testing.md` 与章程 §5 要求 policy ≥85%。
实测 `model.rs` 仅 92/110 行（83.64%），DSL round-trip 测试无法触达直接构造的畸形规则形状。

**步骤**

1. 新建 `crates/policy/tests/model_validation_contract.rs`，只调用公开 API。
2. 覆盖 `PolicyRule::validate` 的正向与全部负向分支（空条件、空列表、非法 id、非法 target_app、
   空 / 重复 scope、deny 原因空或超长）。
3. 覆盖 `RuleConditions::matches` 的全约束命中、逐字段不命中、未约束字段与空列表语义。
4. 复测 crate / workspace 覆盖率，并跑全部全局硬门禁。

**DoD**

- [ ] 生产逻辑零改动；新增测试只调用公开 API、不新增依赖
- [ ] policy crate ≥85%、`model.rs` ≥90%
- [ ] `cargo llvm-cov -p assistant-policy --fail-under-lines 85` 与 `cargo test -p assistant-policy` 全绿
- [ ] `fmt` / `clippy -D warnings` / `test --workspace` / xtask 硬门禁 / `cargo deny check` 全绿
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 同批同步

**验收命令**

```powershell
cargo llvm-cov -p assistant-policy --fail-under-lines 85
cargo test -p assistant-policy
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings; cargo test --workspace
cargo run -p xtask -- hygiene; cargo run -p xtask -- check-comments; cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index; cargo run -p xtask -- refscan; cargo run -p xtask -- docscan
cargo run -p xtask -- card-check; cargo run -p xtask -- check-ledger; cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check; cargo run -p xtask -- check-migrations; cargo deny check
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══ -->

## 执行记录

### 1. 约束回执

【任务】TASK-285 policy 规则模型校验契约覆盖率补齐　【目标】仅追加测试，把 `model.rs`
行覆盖率提升到 ≥90%、crate 越过 85% 门槛　【write scope】仅本卡列出的测试与状态 / 轮次文件
【铁律】无静默失败；测试不改生产行为；不扩 scope；热点文件先 guard
【禁止】改公共接口、依赖、lint/CI、schema/DB/IPC；真实 GUI / 网络 / 凭据
【验收】见上；【依赖】TASK-024 / ADR-0088 Done；【疑问】无

### 2. 实际改动文件

- `crates/policy/tests/model_validation_contract.rs`（新增 9 个跨平台公共契约测试）
- `tasks/TASK-285-...md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`
- `docs/automations/2026-10-13-round-11.md`、`docs/automations/2026-10-13-report.md`

### 3. 验收输出摘要

- 改动前：`model.rs` 92/110 行（83.64%），policy crate 92.25%。
- `cargo test -p assistant-policy --test model_validation_contract`：9 passed。
- `cargo llvm-cov -p assistant-policy --summary-only`：crate 行覆盖率 **95.03%**；
  `model.rs` **110/110 = 100.00%**。
- `cargo llvm-cov --workspace --fail-under-lines 75`：EXIT 0；workspace 80.25% → **80.30%**。
- `fmt` / `clippy -D warnings` / `test --workspace` / `cargo deny check` / UI / arch / replay
  全部 EXIT 0；clippy 仅保留既有 unknown-lint 噪音。
- xtask：`hygiene` 0E/117W、`check-comments` 0E/67W、`docscan` 0E/264W，其余硬门禁 PASSED。
- PR #369；pull_request CI = 11/11 SUCCESS；合并前 base=main、mergeable=MERGEABLE、
  mergeStateStatus=CLEAN；merge `93173a3`。

### 4. DoD 逐条核对

- 生产逻辑零改动、测试只用公开 API、无新依赖：满足。
- policy crate ≥85%、model ≥90%、覆盖率命令与全门禁：满足（95.03% / 100.00%）。
- `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md`：同批同步完成。

### 5. 偏差

- 新建测试首次触发 `clippy::panic`；按仓库测试惯例加 `#![allow(...)]` 后复测全绿。
- 格式检查首轮报 `assert_invalid` 调用超行宽；`cargo fmt` 自动折行后复测 0 差异。

### 6. 更合理做法

- 用单个 policy-local integration test 直接驱动公开模型类型，负向用例按错误 fragment 断言，
  避免只堆砌成功路径。

### 7. 遗留问题

- `decision.rs` / `rule_set.rs` / `validation/*` 仍有少量未覆盖行，但均 ≥88%，不属本卡范围。

### 8. 新增长期记忆

- 无新增 FACT / PITFALL / REJECTED。

### 9. 给审阅者的关注点

1. 新增测试是否只调用公开 API，并覆盖畸形规则形状而非只堆成功路径。
2. `model.rs` 100% 是否来自可解释的 9 条契约用例，而非统计 artifact。
