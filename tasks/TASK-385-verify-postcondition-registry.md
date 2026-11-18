# TASK-385 Verify postcondition machine registry

- 状态：Done
- 阶段：2　子阶段：2b Adapter contract implementation　批次：registry prerequisite　依赖：TASK-383、TASK-384
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031「一卡一文件」）。

---

## write scope

- `tasks/TASK-385-verify-postcondition-registry.md`
- `crates/verify/src/postcondition-registry-1.0.json`
- `crates/verify/src/postcondition_registry.rs`
- `crates/verify/src/postcondition.rs`
- `crates/verify/src/lib.rs`
- `crates/verify/tests/postcondition_registry.rs`
- `crates/verify/README.md`
- `docs/spec/adapter-bundle.md`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

为 `assistant-verify` 建立版本化、机器可读的 postcondition 注册表，并让 Adapter generated
catalog 只从该表生成。解决 `DRIFT-384-1` 的“xtask 内第二套 kind/字段表”问题。

## In scope

- 新增 `postcondition-registry-1.0.json`，覆盖当前 Adapter catalog 支持的 11 个 kind。
- 在 `assistant-verify` 内通过 `include_str!` 暴露 registry，并以现有
  `parse_postconditions` 对每个 kind 生成最小合法样例做同步验证。
- `parse_kind` 的 allowed/required/basic type 与机器 constraints 读取 registry；
  未知 kind 的 `UnsupportedPostconditionKind` 行为保持不变。
- 对 registry 执行封闭结构检查：kind 唯一、数组去重有序、required ⊆ allowed、
  field_types key 与 allowed 对应、effect_eligibility 仅 read/write。
- 显式记录 `visual_assert` 暂不进入 Adapter catalog 的原因，避免静默遗漏。
- 在 `adapter-bundle` spec 固定 registry 路径与 Adapter catalog 的派生关系；
  xtask 改消费 registry 属于 TASK-384 的恢复步骤。

## Out of scope

- 不改变 `Postcondition` 枚举、评估器或 visual runtime；parser 只把结构来源切换到 registry。
- 不在本卡实现 Adapter `visual_assert` 支持；该分支需要独立 schema/validator 卡。
- 不迁移现有 Adapter pack、不改 runtime loader、Policy、ToolBus、CI 或依赖。
- 不把 `adapter-validate` 接入 CI 硬门禁。
- 不修改 xtask 或 generated Adapter catalog；这些由 TASK-384 在 TASK-385 合并后完成。

## 必须遵守

1. registry 是 Adapter postcondition kind/字段/类型/effect 的唯一机器来源。
2. registry 任一 kind 必须能被现有 `parse_postconditions` 接受；不一致即测试失败。
3. Adapter 专属字段不写入 Verify registry；它们由 TASK-384 生成 catalog 时显式合并。
4. `visual_assert` 只能作为显式 excluded kind 记录，不能静默当作 11 项之一。

## 验收命令

```powershell
cargo fmt --all --check
cargo test -p assistant-verify postcondition_registry
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo run -p xtask -- refscan
cargo run -p xtask -- check-ledger
```

预期：registry 与 Verify parser 同步；generated catalog 0 drift；全量测试/Clippy/文档门禁全绿。

## DoD

- [ ] `postcondition-registry-1.0.json` 含 11 个 Adapter-supported kind 与显式 excluded kind。
- [ ] runtime parser 消费 registry 结构/constraints，且每个 kind 样例通过。
- [ ] spec 固定 registry 路径，并声明 Adapter 专属字段在 catalog 生成时合并。
- [ ] workspace tests、Clippy 与文档门禁全绿。
- [ ] 不改 runtime 语义、adapters、CI、Policy 或依赖。

---

## 执行记录

<!--
  由 Implementer 在分界线以下填写。
  以下由 Implementer 填写。改动分界线以上任何正文行 = 漂移触发器 ⑤；
  状态行仅按 ADR-0083 与产生该状态的提交同批更新。 -->

### 1. 约束回执

```text
【任务】TASK-385 Verify postcondition machine registry
【目标】建立 crates/verify 版本化机器注册表，并让 Adapter catalog 只从该表生成
【write scope】仅：Verify registry/parser/tests/README、adapter-bundle spec、本卡与计划状态
【铁律】单一事实源；parse fail-closed；无静默默认；不改 runtime 语义；不新增依赖
【禁止】不实现 visual_assert Adapter 支持、不改 adapters/pack/CI/Policy 或手写第二套 kind 表
【验收】registry 逐 kind 被 parse_postconditions 接受；workspace tests/clippy/文档门禁全绿；TASK-384 后续消费 registry
【依赖】TASK-383 Done、TASK-384 Blocked 已核对
【疑问】无；visual_assert 作为显式 excluded kind 记录，后续单卡处理
```

### 2. 实际改动文件

- `tasks/TASK-385-verify-postcondition-registry.md`
- `crates/verify/src/postcondition-registry-1.0.json`
- `crates/verify/src/postcondition_registry.rs`
- `crates/verify/src/postcondition.rs`
- `crates/verify/src/lib.rs`
- `crates/verify/tests/postcondition_registry.rs`
- `crates/verify/README.md`
- `docs/spec/adapter-bundle.md`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

### 3. 验收输出摘要

- `cargo test -p assistant-verify` → PASSED，lib + 8 integration targets + 2 doctests 全绿。
- `cargo test -p assistant-verify --test postcondition_registry` → PASSED，9/9。
- `cargo test --workspace --no-fail-fast` → PASSED，所有 target 0 failed。
- `cargo clippy -p assistant-verify --all-targets -- -D warnings` → exit 0；仅既有
  `clippy::assert_is_empty` unknown-lint 提示。
- `cargo clippy --workspace --all-targets -- -D warnings` → exit 0；仅同一既有提示。
- `hygiene` / `card-check` / `docscan` / `refscan` / `check-ledger` → 均 exit 0。
- `git diff --check` → PASSED。
- PR #762：base=`main`，checks **21 success + 1 skipped**，pending 0 / failed 0，
  `MERGEABLE` / `CLEAN`；merge `4f92ed88120793a7383255863aa55319bba2f02a`，
  post-merge main CI `44256975571` success。

### 4. DoD 逐条核对

- [x] registry 含 11 个 Adapter-supported kind 与显式 `visual_assert` excluded 记录。
- [x] runtime parser 直接消费 registry 的 allowed/required/basic type/constraints，
      每个 kind 样例通过且未知 kind 错误变体不变。
- [x] spec 固定 registry 路径，并声明 Adapter 专属字段在 catalog 生成时合并。
- [x] workspace tests、Clippy 与文档门禁全绿。
- [x] 不改 runtime 语义、adapters、CI、Policy 或依赖。

### 5. 偏差

- 无 DRIFT。TASK-385 不直接修改 xtask；registry 合并后由 TASK-384 改为唯一输入。

### 6. 实施中发现的更合理做法

- registry 只保存 Verify parser 真正接受的字段；`description/error_code` 由 Adapter catalog
  generator 显式合并，避免把 envelope 字段伪装成 assertion 字段。
- runtime parser 的结构校验与跨字段 constraints 已改为读取 registry；typed variant 构造与
  evaluation 仍保留在 parser/engine，并由完整矩阵合同测试锁定。
- `visual_assert` 的 field/op/tolerance 组合不是当前 catalog 固定字段模型能表达的，
  因此先在 `excluded_kinds` 记录非空 reason，后续用独立卡扩展。

### 7. 遗留问题

- TASK-384 需在 TASK-385 合并后改为解析该 registry，并合并 Adapter 专属字段。
- Adapter `visual_assert` 支持仍未实现。

### 8. 新增长期记忆

- `crates/verify/src/postcondition-registry-1.0.json` 是 Adapter-supported postcondition
  kind/字段/类型/effect 的机器唯一来源。

### 9. 给审阅者的关注点

- 检查 11 个 kind 的 required/allowed/type 是否与 `postcondition.rs` 完全同步。
- 检查 `visual_assert` excluded reason 是否足以防止静默遗漏。
- 检查 spec 是否清楚区分 Verify registry 字段与 Adapter `description/error_code`。
