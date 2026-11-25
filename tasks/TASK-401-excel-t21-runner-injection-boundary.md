# TASK-401 Excel T2.1 runner injection boundary and real-COM acceptance harness

- 状态：**Review**
- 阶段：2　子阶段：Stage 2 Excel T2.1　批次：4g　依赖：TASK-391、TASK-395、TASK-398、TASK-399、ADR-0108、ADR-0110

---

## write scope

- `tasks/TASK-401-excel-t21-runner-injection-boundary.md`
- `docs/adr/0111-excel-t21-runner-injection-boundary.md`
- `docs/adr/README.md`
- `docs/spec/excel-t21-readonly-workflow.md`
- `docs/memory/decisions.md`
- `docs/memory/apps/excel.md`
- `MEMORY.md`
- `crates/platform/windows/src/lib.rs`
- `crates/platform/windows/src/excel_com_runner.rs`
- `crates/platform/windows/src/excel_com_runner_windows.rs`
- `crates/platform/windows/src/excel_process.rs`
- `crates/platform/windows/Cargo.toml`
- `crates/platform/windows/src/excel_com_runner_tests.rs`
- `crates/platform/windows/README.md`
- `apps/agent-core/tests/excel_t21_runner_boundary.rs`
- `apps/agent-core/README.md`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

在真实 COM 和人类在场 `.xlsx` 验收前，冻结 Excel T2.1 private runner 到 agent-core
之间的安全注入边界。将 TASK-399 的 test-only runner 提升为 Windows 平台上的窄公共
handle，并增加显式 ignored 的真实 COM 验收路径，同时保持生产 tools、rollback、
ToolBus 和模型可见能力为空。

## In scope

- 新增 ADR-0111，明确 `platform-windows` 持有 COM worker，agent-core 持有领域适配。
- 暴露仅限 Windows 的公共 `ExcelComRunner` handle 和自包含结果类型。
- 支持 create application、owned PID、read-only `.xlsx` open、active sheet text read、
  workbook close 与 bounded shutdown。
- fake backend 覆盖 open/read/close 的顺序、错误、重复关闭与 shutdown。
- 新增 ignored Windows 真实 COM 验收测试；仅在人工提供 `.xlsx` 路径且无既有 Excel 时运行。
- 新增 agent-core compile-boundary test，证明该 handle 可从 app 层引用；完整
  `ExcelT21Runtime` 域映射仍留给后续卡。
- 同步 ADR/spec/平台与 app README、Stage 2/PLAN/README/LEDGER。
- 独立 review、PR、CI、merge 与 post-merge main CI 收口。

## Out of scope

- 不填写 `adapters/com.microsoft.excel/tools/tools.json`，不注册四 Tool 或任何生产 ToolBus。
- 不实现 or 修改 rollback recipe、production assembly、protocol schema、公共 ErrorCode、依赖、
  Policy 或 UI。
- 不把 smoke read 冒充完整 T2.1 workbook/range/summarize 域能力。
- 不自动启动 Excel、不无人值守读取真实工作簿、不覆盖或保存用户文件。
- 不实现 ownership journal、独立 watchdog、process-tree kill、重复收敛或 named range。
- 不改测试断言来掩盖失败，不放宽 `tools.json` 为空或真实边界判据。

## 必须遵守

1. COM object / STA guard / IDispatch 只能在 dedicated worker thread 生命周期内存在。
2. 公共 handle 必须 `Send + Sync`，失败必须是 typed `PlatformError`，不得 silent fallback。
3. 同一 runner 最多一个 application 与一个 open workbook；重复/越序调用显式失败。
4. `.xlsx` 只接受本地绝对路径、canonical file、read-only open；既有 Excel 进程不得附着。
5. close/shutdown 必须按 workbook → application 顺序清理；超时或失败不得伪装成功。
6. ignored 验收必须记录输入路径、owned PID、前后 digest、open/read/close 结果与进程归零。
7. 真实 COM 未运行或环境不满足时，卡保持 Review/InProgress，不写 Done。
8. 保持生产 `tools.json`、ToolBus、rollback、assembly 零注册。

## 验收命令

```powershell
cargo test -p assistant-platform-windows excel_com_runner
cargo test -p assistant-platform-windows
cargo fmt --all --check
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
cargo run -p xtask -- adr-index
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-comments
cargo deny check
git diff --check
```

真实 COM 人工验收：

```powershell
$env:EXCEL_T21_ACCEPTANCE_WORKBOOK = '<explicit local absolute .xlsx path>'
cargo test -p assistant-platform-windows test_windows_excel_real_com_single_workbook_acceptance -- --ignored --nocapture
Remove-Item Env:\EXCEL_T21_ACCEPTANCE_WORKBOOK
```

预期：fake/边界测试全绿；生产无 Tool 注册；人工验收时 owned Excel 进程归零、文件 digest
不变；未提供环境或存在既有 Excel 时明确失败/Blocked，不伪造通过。

## DoD

- [x] ADR-0111 与 spec 固化窄公共 handle 边界，未新增 Excel 专用 `platform/api` trait。
- [x] runner 公共 handle 只在 Windows 提供，COM 状态不跨线程。
- [x] create/open/read/close/shutdown 成功与失败路径均有 fake tests。
- [x] read-only open 只接受本地 canonical `.xlsx`，同一 runner 不并发 open。
- [x] ignored Windows 验收测试可由人工显式运行，并检查 digest 与 owned PID 收敛。
- [x] agent-core compile-boundary test 能从 app 层引用平台 handle；不注册 ToolBus。
- [x] `tools.json`、rollback、production assembly、protocol/schema 与依赖零改动。
- [ ] 全量门禁、独立 review、PR/CI/merge/post-merge main CI 闭环。

<!-- 分隔线以下由 Implementer 填写；正文区只读，状态行按 ADR-0083 更新。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-401 Excel T2.1 runner injection boundary and real-COM acceptance harness
【目标】冻结平台 handle 注入边界并增加人工真实 COM 验收路径，不开放生产 Tool
【write scope】仅：本卡、ADR-0111/索引/spec/decisions、platform-windows runner/README/tests、agent-core 边界测试/README、Stage 2/PLAN/README/LEDGER
【铁律】公共接口先 ADR；COM 不跨线程；显式失败；真实 GUI 人在场；不伪造证据
【禁止】不填 tools.json/不注册 ToolBus/不改 rollback/assembly/protocol/依赖；不自动启动 Excel
【验收】runner/workspace/arch/schema/codegen/文档门禁/deny/diff-check；真实 COM 仅人工
【依赖】TASK-391/395/398/399/400 Done，ADR-0108/0109/0110 Accepted
【疑问】无可自动解决项；完整域映射留给后续卡
```

### 2. 实际改动

- 新增 ADR-0111 与 spec 注入边界。
- Windows platform `excel_com_runner` 从 test-only prototype 提升为窄公共 handle；
  新增 `excel_com_runner_windows.rs` 承载 dedicated worker、`IDispatch` 与 COM 生命周期。
- 新增 fake open/read/close/shutdown、路径/canonical/非法地址、失败保真测试。
- 新增 ignored `test_windows_excel_real_com_single_workbook_acceptance`，人工提供单个
  `.xlsx`，验证 open/read/close、digest 不变与 owned Excel 进程归零。
- 新增 agent-core compile-boundary test；不注册 ToolBus 或 tools。

### 3. 门禁输出摘要

- `cargo test -p assistant-platform-windows excel_com_runner -- --nocapture`：11 passed,
  1 ignored（真实 COM 测试需显式 `--ignored`）。
- 真实 COM smoke：最终复跑 1 passed；输出 workbook、owned PID、start ticks、EXE path 与
  digest before/after。两次 digest 均为
  `1bf100d1aa95e9a5486083c87351c1166435597fa9aaede4b36c5a2396420cde`，最终 `EXCEL.EXE=0`。
- `cargo clippy --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：通过；xtask 587 passed。
- schema 7/7、codegen 0 drift、arch 10/10、deny ok、replay 2 steps/0 mismatch。
- 文档门禁全绿；UI 90 model + 5 Vitest；coverage **78.18%** lines。

### 4. DoD 核对

- [x] ADR-0111 与 spec 固化窄公共 handle 边界，未新增 Excel 专用 `platform/api` trait。
- [x] runner 公共 handle 只在 Windows 提供，COM 状态不跨线程。
- [x] create/open/read/close/shutdown 成功与失败路径均有测试。
- [x] read-only open 只接受本地 canonical `.xlsx`，同一 runner 不并发 open。
- [x] ignored Windows 验收测试可由人工显式运行，并检查 digest 与 owned PID 收敛。
- [x] agent-core compile-boundary test 能从 app 层引用平台 handle；不注册 ToolBus。
- [x] `tools.json`、rollback、production assembly、protocol/schema 与依赖零改动。
- [ ] 全量门禁、PR/CI/merge/post-merge main CI 闭环。

### 5. 偏差

- 第一次真实 open 因 Windows canonical path 带 `\\?\` 前缀返回 `0x800A03EC`；已把本地
  canonical path 规范为 Excel 可接受形式，重跑通过。
- 第一次验收在错误前置断言处 panic，测试进程退出后 Excel 仍在；已增加测试级 RAII
  guard，正常与 panic 路径最终都保证 owned PID 归零，真实重跑确认 `EXCEL.EXE=0`。
- 独立 review 后补齐生产路径既有 `EXCEL.EXE` fail-closed、Toolhelp inventory、
  PID + start ticks + executable path owned identity、runner-specific handle id、
  严格 single-A1 地址、canonical 后 `.xlsx` 复验、`.`/`..` 拒绝与本地盘检查。
- Windows backend 字段顺序改为 workbook 先于 application；参数 VARIANT 清理失败时也释放
  result VARIANT。

### 6. 新增长期记忆

- `docs/memory/apps/excel.md`：记录 `Workbooks.Open` 拒绝 `\\?\` verbatim path，以及
  panic 路径必须由测试级 owned identity guard 回收 Excel。

### 7. 遗留与下一步

- 后续卡把公共 handle 完整映射为 `ExcelT21Runtime` 的 workbook/range/summarize 域操作。
- ownership journal、独立 watchdog、真实 process-tree kill 和重复运行
  process/session/handle 收敛仍未实现。
- 生产 `tools.json`、rollback recipe、production assembly 与 ToolBus registration 继续为空。

### 8. 外部证据

- 本轮为人工会话，无 automation 外部 JSONL。
- 真实 COM 证据来自 ignored 测试 stdout：workbook path、PID、start ticks、EXE path、
  digest before/after 与最终 `EXCEL.EXE=0`。

### 9. 给审阅者的关注点

- 独立复审最终 **no findings**；上轮 Toolhelp fail-open、A1 列上限/前导零和计划状态均已关闭。
- Toolhelp 对既有 Excel 的检测在创建 COM 前执行；仍需在后续 watchdog 卡处理检测与用户
  并发启动 Excel 的最终竞态。
- `Drop` 只做 best-effort；显式 `shutdown()` 仍唯一可报告失败，后续 watchdog 需关闭该缺口。
- 当前公共面只做 smoke，不能据此注册四 Tool 或宣称完整 T2.1 已完成。
