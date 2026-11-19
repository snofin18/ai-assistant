# TASK-388 interrupt optional match fields

- 状态：**Done**
- 阶段：2　子阶段：Adapter pack convergence A　批次：contract repair
- 依赖：ADR-0104、ADR-0105、TASK-383、TASK-384、TASK-386

---

## write scope

- `tasks/TASK-388-interrupt-optional-match-fields.md`
- `xtask/src/adapter_validate/interrupts.rs`
- `xtask/src/adapter_validate_interrupt_tests.rs`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

修复 Adapter interrupt match validator 把可选字段误当成必填字段的实现偏差，
使 `class_name`、`automation_id`、`automation_id_candidates`、`title_candidates`、
`name_candidates`、`class_name_contains` 按 ADR-0105 的 union 语义校验。

## In scope

- 只校验 `match` 中实际出现的字符串字段；缺失可选字段不报错。
- 保留“match 非空”和“title/name 文本候选不得单独成立”的既有拒绝条件。
- 保留候选数组非空、去重、内容非空和 `locale_dependent=true` 校验。
- 为 `class_name + title_candidates` 与
  `automation_id_candidates + class_name_contains` 两个合法组合补正向测试。
- 为已出现但空白的 `class_name`、`automation_id`、`class_name_contains` 补负向测试。

## Out of scope

- 不改 `docs/adr/**`、`docs/spec/**`、Adapter pack 数据、公共 schema、产品运行时、
  CI 或依赖。
- 不增加新的 match/action/classification 语法。
- 不修复与本偏差无关的 validator 行为。
- 不做真实 Notepad GUI 验收。

## 必须遵守

1. 不通过向 Adapter pack 添加未经实测的 class/id 字段绕过 validator。
2. 文本候选仍必须与至少一个非文本 identity 字段同时出现。
3. 字段存在但类型错误、空字符串或纯空白时继续 fail-closed。
4. 修复必须有直接单测覆盖合法 union 子集和空白字段拒绝。

## 验收命令

```powershell
cargo test -p xtask adapter_validate
cargo test -p xtask --no-fail-fast
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo run -p xtask -- refscan
cargo run -p xtask -- memory-counts
cargo run -p xtask -- check-ledger
git diff --check
```

预期：两类合法 match 子集通过；空白已声明字段和纯文本 match 失败。TASK-386
重基本卡后，再以 Notepad pack 全量 validator 作为下游验收。

## DoD

- [ ] 缺失的可选 match 字段不再产生 finding。
- [ ] 已出现但空白的 identity 字符串字段仍失败。
- [ ] 两类合法 union 子集有正向单测。
- [ ] 纯文本候选不得单独成立、空候选数组/重复候选规则不回归。
- [ ] TASK-386 重基后 Notepad pack 的 interrupt finding 为零，且未修改其他 pack。
- [ ] PR checks、merge 与 post-merge main CI 回填。

<!-- ─── 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ───
     以下由 Implementer 填写。改动分界线上方任何一行 = 漂移触发器 ⑤。
-->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-388 interrupt optional match fields
【目标】修复可选 match 字段被误判为必填，解除 TASK-386 的 DRIFT-386-2
【write scope】仅：本卡、interrupts validator、新增外置测试、Stage 2/PLAN/README/LEDGER
【铁律】契约先行；不可信输入先校验；默认拒绝；不改测试断言规避；证据不得伪造
【禁止】不改 spec/ADR、Adapter pack、公共 schema、产品代码、CI 或依赖
【验收】xtask 专项/全量 + fmt/clippy + Notepad adapter-validate + 文档门禁全绿
【依赖】TASK-383/384 Done；TASK-386 因 DRIFT-386-2 等待本卡
【疑问】无；严格按 spec §8.2 的可选 union 修复
```

### 2. 实际改动文件（逐个核对是否在 In scope 内）

- `tasks/TASK-388-interrupt-optional-match-fields.md`
- `xtask/src/adapter_validate/interrupts.rs`
- `xtask/src/adapter_validate_interrupt_tests.rs`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

未修改 Adapter pack、spec/ADR、公共 schema、产品运行时、CI 或依赖。

### 3. 验收输出摘要（命令 → 结果，全绿 / 失败项）

- `cargo test -p xtask adapter_validate --no-fail-fast` → 37 passed / 0 failed。
- `cargo test -p xtask --no-fail-fast` → 584 passed / 0 failed。
- `cargo fmt --all --check` → PASSED。
- `cargo clippy --workspace --all-targets -- -D warnings` → exit 0；仅有仓库既存
  `unknown lint: clippy::assert_is_empty` 提示。
- `cargo test --workspace --no-fail-fast` → 全部 test target 与 doctest 通过，0 failed；
  仅保留需要真实桌面/Excel/Paint/Edge 的人类在场 `ignored` 用例。
- `cargo run -p xtask -- hygiene` → PASSED，500 files，0 error / 195 既存 warning。
- `cargo run -p xtask -- check-comments` → PASSED，0 error。
- `cargo run -p xtask -- card-check` → PASSED，290 files，0 error / 144 既存 warning。
- `cargo run -p xtask -- docscan` → PASSED，694 files，0 error / 175 既存 warning。
- `cargo run -p xtask -- refscan` → PASSED，1235 files，0 error / 0 warning。
- `cargo run -p xtask -- memory-counts` → PASSED，0 error / 1 既存 Paint warning。
- `cargo run -p xtask -- check-ledger` → PASSED，PLAN/LEDGER 均为 2026-11-19。
- `cargo deny check` → advisories / bans / licenses / sources 全部 ok。
- `git diff --check` → PASSED。
- PR #779 当前 head checks **11/11 success**，pending 0 / failed 0，base=`main`，
  `MERGEABLE` / `CLEAN`。
- merge `9d6d5584843c934c300c877f044e83883858b064`。
- post-merge main CI `44490394734` → success。
- TASK-386 重基本合并后，`adapter-validate adapters/com.microsoft.notepad
  --report-only` → `scanned_files=7`、0 error / 0 warning、PASSED。

### 4. DoD 逐条核对

- [x] 缺失的可选 match 字段不再产生 finding。
- [x] 已出现但空白的 identity 字符串字段仍失败。
- [x] `class_name + title_candidates` 与
  `automation_id_candidates + class_name_contains` 两类合法 union 子集有正向单测。
- [x] 纯文本候选不得单独成立、候选数组和既有 human matrix 测试未回归。
- [x] TASK-386 重基后 Notepad pack validator 为 7 files / 0 error / 0 warning / PASSED。
- [x] PR #779 checks 11/11 success、merge `9d6d558`、post-merge main CI
  `44490394734` success 已回填。

### 5. 偏差

- PowerShell 最小读取再次持续零输出；终止后回退 `cmd`，未产生仓库写入。

### 6. 更合理做法（实施中发现，非漂移）

- 测试按职责外置到 `adapter_validate_interrupt_tests.rs`，避免继续推高
  `adapter_validate_tests.rs` 的 900 行硬上限。
- 直接测 `interrupts::validate` 的 match union，不复制完整 Adapter fixture。

### 7. 遗留问题

- 合并 TASK-388 后需重基 TASK-386，并确认 `adapter-validate
  adapters/com.microsoft.notepad --report-only` 只剩/清零下游 finding。

### 8. 新增长期记忆

- 无；这是实现与 spec 对齐缺陷，不需要新增 ADR/spec 决策。

### 9. 给审阅者的关注点（风险最高的 1~3 处）

1. 确认缺失的 `class_name` / `automation_id` / `class_name_contains` 不再被误报，
   但显式空值和错误类型仍失败。
2. 确认文本候选仍不能脱离非文本 identity 字段单独命中。
3. 确认测试外置没有改变 `adapter_validate_tests` 模块的可见性边界。
