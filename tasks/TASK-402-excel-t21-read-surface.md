# TASK-402 Excel T2.1 read-only runner surface

- 状态：**Done**
- 阶段：2　子阶段：Stage 2 Excel T2.1　批次：4h　依赖：TASK-401、ADR-0108、ADR-0111、ADR-0112

---

## write scope

- `tasks/TASK-402-excel-t21-read-surface.md`
- `docs/adr/0112-excel-t21-reader-surface.md`
- `docs/adr/README.md`
- `docs/spec/excel-t21-readonly-workflow.md`
- `docs/memory/decisions.md`
- `docs/memory/apps/excel.md`
- `MEMORY.md`
- `crates/platform/windows/src/excel_com_runner.rs`
- `crates/platform/windows/src/excel_com_runner_api.rs`
- `crates/platform/windows/src/excel_com_runner_windows.rs`
- `crates/platform/windows/src/excel_process.rs`
- `crates/platform/windows/src/excel_com_runner_tests.rs`
- `crates/platform/windows/src/lib.rs`
- `crates/platform/windows/Cargo.toml`
- `crates/platform/windows/README.md`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

把 TASK-401 的 smoke handle 扩展为完整 Excel T2.1 只读平台面，让后续 agent-core
适配器可以在不接触 COM 指针的情况下实现现有 `ExcelT21Runtime`。

## In scope

- owned process identity、workbook/worksheet/named-range/calculation snapshot。
- `value2` / `formula` / `text` 三模式 range read。
- deterministic workbook fingerprint 与 ownership verification。
- fake tests、Windows 编译、真实 COM ignored 验收。
- 文档与状态同步。

## Out of scope

- 不实现 agent-core `ExcelT21Runtime` 适配（下一卡）。
- 不实现写入、保存、图表、rollback、watchdog、ownership journal 或重复收敛。
- 不填写 `tools.json`，不注册 ToolBus，不改 protocol/schema/依赖版本、Policy 或 UI。
- 不修改工作簿，不覆盖文件。

## 必须遵守

1. COM 对象只在 worker thread，公共 handle 不携带 COM。
2. 所有 range 在读取前检查 4096 cells；workbook metadata 有硬上限。
3. A1 边界与 agent-core 一致：最大 XFD1048576，不允许前导零/反向范围。
4. Excel error、空值、数字、布尔、文本必须映射为封闭 cell value type。
5. named range scope 解析失败必须显式失败，不猜。
6. fingerprint 只基于 metadata，并明确不冒充文件 digest。
7. 生产 Tool surface 保持空。

## 验收命令

```powershell
cargo test -p assistant-platform-windows excel_com_runner
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-core arch::
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo run -p xtask -- refscan
cargo run -p xtask -- memory-counts
cargo run -p xtask -- check-ledger
cargo run -p xtask -- adr-index
cargo run -p xtask -- check-comments
cargo deny check
cargo run -p xtask -- replay --suite core
pnpm lint; pnpm typecheck; pnpm test
cargo llvm-cov --workspace --fail-under-lines 75
```

真实 COM 验收：使用人工生成的单个本地 `.xlsx`，至少含 worksheet、A1 range 和
workbook-scoped named range；验证 metadata、三模式读取、fingerprint、ownership、
digest 不变与 `EXCEL.EXE=0`。

## DoD

- [ ] 公共 API 只含自包含数据，COM 不跨线程。
- [ ] workbook metadata 与 named ranges 上限/顺序/scope 均有正负测试。
- [ ] range 三模式、空值/数字/布尔/文本/error、bounds 均有测试。
- [ ] fingerprint 确定且 ownership 使用 PID + start ticks + EXE path。
- [ ] 真实 COM 人工验收通过并记录 digest/进程证据。
- [ ] 生产 tools/ToolBus/assembly/rollback 零注册。
- [ ] 全量门禁、独立 review、PR/CI/merge 闭环。

<!-- 记录区由 Implementer 填写；正文区只读，状态行按 ADR-0083 更新。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-402 Excel T2.1 read-only runner surface
【目标】扩展 platform runner 的完整只读 surface，不注册生产 Tool
【write scope】本卡、ADR-0112/索引/spec/decisions/excel memory、platform-windows runner/tests/README/Cargo feature、Stage 2/PLAN/README/LEDGER
【铁律】COM 不跨线程；payload 有界；无静默失败；不可信结果二次校验
【禁止】不实现 agent-core adapter/write/watchdog/repeat；不填 tools.json/不注册 ToolBus；不改依赖版本
【验收】专项/fake/真实 COM + 全量门禁
【依赖】TASK-401 Done；ADR-0108/0111/0112 Accepted
【疑问】sheet-scoped Name 先按 Name.Name `Sheet!Name` 解析；不成立则显式失败并记 DRIFT
```

### 2. 实际改动

- 新增 `docs/adr/0112-excel-t21-reader-surface.md`，并在 ADR 索引、spec、decisions 与
  Excel memory 中登记只读 runner surface。
- 新增自包含类型 `ExcelOwnedProcessIdentity`、workbook/worksheet/named-range/calculation
  snapshot、`ExcelReadMode`、cell/range snapshot 与 range selector；COM `IDispatch`
  仍只在 worker thread 内存在。
- `ExcelComRunner` 增加 `read_workbook`、`read_range`、`fingerprint_workbook`、
  `verify_ownership`；range 上限 4096 cells、payload 上限 256 KiB，A1 上限
  `XFD1048576`，fingerprint 明确只是 metadata SHA-256。
- Windows backend 增加 workbook metadata、named-range scope、value2/formula/text 读取和
  cell value type 映射；修复 Excel 集合 `Item` 需要 `DISPATCH_PROPERTYGET |
  DISPATCH_METHOD` 的真实 COM 差异。
- 将 workbook path/A1 校验移入 `excel_process`，把真机验收 guard 放入
  `excel_com_runner_api` 的 test-only 支撑区，三个 Excel runner 文件均低于 900 行硬门禁。
- 增加 fake/protocol/真实 COM 测试；真实 fixture 覆盖 numeric、boolean、text、blank、
  `#DIV/0!`、formula、named range、metadata、fingerprint 与 ownership。真实 COM 退出等待
  改为有界 20 秒，以容纳公式/error cell 的释放延迟。
- 修复 Linux CI 条件编译：fake test 的 `super::*` 恢复为跨平台导入，
  `excel_process` 改为 `any(windows, test)`，仅 Windows 使用的 process/IDispatch helper
  按平台裁剪。
- Post-merge main CI `45563273943` 暴露 macOS temp fixture 竞态：同一纳秒/进程下两个
  fake test 可能生成同名 `.xlsx`，先结束的测试删除后另一个 canonicalize 失败。已追加
  进程内单调序号，未改产品路径校验语义。

### 3. 门禁输出摘要

- `cargo test -p assistant-platform-windows excel_com_runner` → **14 passed / 1 ignored**
  （真实 COM ignored）。
- `EXCEL_T21_ACCEPTANCE_WORKBOOK=... cargo test -p ... test_windows_excel_real_com_single_workbook_acceptance -- --ignored --nocapture`
  → **1 passed / 0 failed**；PID `34212`，digest before/after
  `c801004c5a350c5af9db06039821f826e46916e14bad97cbc242da74d76692ea`，
  最终 `EXCEL.EXE=0`。
- `cargo test --workspace`、`cargo clippy --all-targets -- -D warnings`、
  `cargo fmt --all --check`、arch、schema、codegen、hygiene、card/doc/ref/comment、
  memory-counts、adr-index、check-ledger、deny、core replay → 全部 PASSED。
- `cargo clippy --target x86_64-unknown-linux-gnu -p assistant-platform-windows --all-targets -- -D warnings`
  → PASSED，修复了 PR #853 首轮 Ubuntu fake-test 编译失败。
- `cargo llvm-cov --workspace --fail-under-lines 75` → workspace line coverage
  **77.59%**；根目录没有 `package.json`，`pnpm lint/typecheck/test` 不适用。

### 4. DoD 核对

- [x] 公共 API 只含自包含数据，COM 不跨线程。
- [x] workbook metadata 与 named-range scope 有 fake、真实 COM 与负向解析测试；上限检查
  由 snapshot/selector 构造和 runner 前置校验覆盖。
- [x] range 三模式、空值/数字/布尔/文本/error、A1 bounds 均有测试。
- [x] fingerprint 确定，ownership 使用 PID + creation FILETIME + EXE path。
- [x] 真实 COM 人工验收通过并记录 digest/进程证据。
- [x] 生产 tools/ToolBus/assembly/rollback 零注册。
- [x] 独立 review、PR、CI 与 merge 闭环。

### 5. 偏差

- none。真实 COM 首轮在 5 秒固定 poll 内未见退出，随后确认所有读取与校验已完成；
  guard 使用完整身份清理，未留下进程。将等待改为有界 20 秒属于测试稳定性修正，
  没有放宽任何成功判据或把未读取结果写成成功。

### 6. 新增长期记忆

- 已追加到 `docs/memory/apps/excel.md`：Excel collection `Item` 的真实 IDispatch
  flag 兼容要求；带 error cell 的工作簿需要更长的有界 Quit/process 收敛窗口。

### 7. 遗留与下一步

- 下一卡实现 agent-core `ExcelT21Runtime` 到本 runner 的私有领域映射。
- 重复运行、session/handle/process 收敛与生产 `tools.json`/ToolBus registration 仍需
  后续卡，不能以本次单次只读验收代替。

### 8. 外部证据

- fixture：`C:\Users\fexfe\AppData\Local\Temp\task402-real-com-read-surface.xlsx`
  （worksheet `Sheet1`；A1=2、B1=3、C1=TRUE、A2=`=A1+B1`、B2=`hello`、C2=`=1/0`；
  workbook-scoped `SalesData`=`=Sheet1!$A$1:$B$2`）。
- 最终真实 COM 日志：PID `34212`，EXCEL path
  `C:\Program Files\Microsoft Office\root\Office16\EXCEL.EXE`，前后 digest 相同，
  test result `1 passed`，进程清单归零。
- PR #853 checks **22（21 SUCCESS + 1 SKIPPED）**，pending 0 / failed 0，base=main，
  `MERGEABLE` / `CLEAN`；merge `d958b52b73f33e305e2441889f17a58363f311df`；
  post-merge main CI `45559864439` success。
- 后续 main CI `45563273943` 的 macOS fake-test 竞态由 fix PR #855 修复；PR checks
  **11/11 SUCCESS**，merge `a70c62559072d70f6accf36d43844a55e7242b42`，
  post-merge main CI `45567304098` success。修复前未把它误记为 main 全绿。

### 9. 给审阅者的关注点

- 最高风险是 `IDispatch` 集合/属性 flag 的真实 COM 兼容性；专项与真实验收现已覆盖
  `Worksheets.Item`、`Names.Item`、`Range.Cells`。
- 第二风险是 fingerprint 的语义边界：它只基于 metadata，不能当作文件内容 digest。
- 第三风险是 20 秒退出等待：仍是有界 fail-closed 验收，不是无限等待或强制静默成功。
