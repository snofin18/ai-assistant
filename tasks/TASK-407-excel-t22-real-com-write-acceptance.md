# TASK-407 Excel T2.2 human-present real COM write acceptance

- 状态：**Done**
- 阶段：2　子阶段：Stage 2 Excel T2.2　批次：6c　依赖：TASK-403、TASK-404、TASK-405、TASK-406、ADR-0107、ADR-0108、ADR-0110、ADR-0111、ADR-0112

---

## write scope

- `tasks/TASK-407-excel-t22-real-com-write-acceptance.md`
- `crates/platform/windows/src/excel_com_runner.rs`
- `crates/platform/windows/src/excel_com_runner_dispatch.rs`
- `crates/platform/windows/src/excel_com_runner_write.rs`
- `crates/platform/windows/src/excel_com_runner_windows.rs`
- `crates/platform/windows/src/excel_com_runner_windows_variants.rs`
- `crates/platform/windows/src/excel_com_runner_windows_write.rs`
- `crates/platform/windows/src/excel_com_runner_tests.rs`
- `crates/platform/windows/src/excel_com_runner_write_tests.rs`
- `crates/platform/windows/src/excel_process.rs`
- `crates/platform/windows/README.md`
- `docs/memory/apps/excel.md`
- `MEMORY.md`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

在不扩大生产公共 API、不注册生产 Tool 的前提下，用真实 Excel COM 对一份本地临时
`.xlsx` 工作副本完成可写打开、单一 scalar range write、`Application.Calculate`、
完整 `.Value2` / `.Formula` 双读、`Workbook.Save`、关闭与进程收敛；随后用磁盘影子
副本恢复原文件，验证原 digest 和原始值可读回。

本卡只对人类在场的 test-only 校准负责，不把真实写入能力暴露给模型、ToolBus 或生产
assembly。

## In scope

- 在 `#[cfg(test)]` 范围增加 writable-open、scalar property put、recalculate、save
  和关闭后句柄失效验证。
- range 选择只复用既有 `ExcelRangeSelector`；scalar 只允许 null/boolean/finite number/
  bounded text，不接受公式、数组、对象或任意 property/method 名称。
- 真实验收只操作从显式本地 fixture 复制出的临时工作副本；shadow copy 必须在同一本地
  磁盘，恢复时覆盖临时工作副本，不覆盖源 fixture 或用户文件。
- 真实验收记录 original / written / restored 文件 digest，真实 `.Value2` / `.Formula`
  raw 形状，owned Excel PID、creation FILETIME、executable path、保存前后值和最终
  `EXCEL.EXE=0`。
- `.Formula` 对 number/boolean/text/blank 的断言必须先通过真实读取校准，不得沿用 fake
  port 的猜测。
- fake 测试覆盖 writable-open、write → recalculate → read → save 的 worker 顺序、
  stale/mismatched handle、scalar 边界、save 失败与 shutdown 收敛。
- 真实 COM 验收保持 `#[ignore]`，只由人类提供 fixture 路径并在本机运行。
- 同步平台 README、Excel app memory、Stage 2、PLAN、README、LEDGER 与任务记录。

## Out of scope

- 不填写 `adapters/com.microsoft.excel/tools/tools.json`、rollback recipe、assembly，
  不注册 production ToolBus 或模型可见 `excel.range.write`。
- 不修改 `platform/api` 公共 trait、protocol/schema、ErrorCode、Policy、依赖或生产
  `ExcelComRunner` 公开方法面。
- 不实现 general COM property/method 调用、公式写入、VBA、宏、图表、格式、批量事务、
  ownership journal、独立 watchdog 或重复运行 10/10。
- 不打开或保存用户工作簿、OneDrive/SharePoint/UNC 路径、既有 Excel 实例或宏工作簿。
- 不把 fake、单次真实写入或 shadow restore 冒充完整生产 T2.2 写入事务。

## 必须遵守

1. 所有 COM 对象、STA guard 和 property put 参数只在 dedicated worker 内创建、
   使用并释放。
2. 真实阶段开始前必须确认 `EXCEL.EXE=0`；结束后必须确认 owned PID 与全机
   `EXCEL.EXE=0`。
3. 真实测试只能对本地临时 `.xlsx` 副本执行；fixture、shadow copy 和工作副本不得
   位于 UNC、映射网络盘或用户业务目录。
4. 写入顺序固定为 writable open → scalar write → calculate → complete Value2 read →
   complete Formula read → save → close → shutdown。
5. 任一写、重算、双读、保存、关闭或进程收敛失败都必须显式失败；不得伪造 digest、
   属性值或进程结果。
6. 保存后 digest 必须改变；shadow restore 后 digest 必须逐字节回到 original。
7. 恢复后的文件必须再次以 read-only 模式打开并读回原始值。
8. `DisplayAlerts=false` 永久禁止；不得静默覆盖用户确认。
9. `.Formula` 精确断言只能在真实 raw 结果落盘后编码。

## 验收命令

```powershell
cargo test -p assistant-platform-windows excel_com_runner
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo clippy --target x86_64-unknown-linux-gnu -p assistant-platform-windows --all-targets -- -D warnings
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
cargo run -p xtask -- replay --suite core
cargo llvm-cov --workspace --fail-under-lines 75
git diff --check
```

真实 COM 验收：

```powershell
$env:EXCEL_T22_ACCEPTANCE_WORKBOOK = '<explicit local absolute .xlsx fixture path>'
cargo test -p assistant-platform-windows test_windows_excel_real_com_write_calibration -- --ignored --nocapture
```

预期：fake 与全量门禁全绿；真实阶段保存前/后/恢复后 digest 符合预期，raw
`.Value2/.Formula` 已记录并按校准结果断言，最终 `EXCEL.EXE=0`；生产 Excel surface
仍为空。

## DoD

- [ ] writable-open/write/calculate/save 均为 `#[cfg(test)]`，生产公开面与 Tool 注册零扩大。
- [ ] scalar property put 使用 `DISPATCH_PROPERTYPUT` 与 `DISPID_PROPERTYPUT`，所有
  `VARIANT` 恰好清理一次，参数在同步 `Invoke` 期间保持有效。
- [ ] fake 测试覆盖操作顺序、scalar 边界、stale handle、保存失败与 worker 收敛。
- [ ] 人类在场真实 COM 验收完成，记录 original/written/restored digest、raw
  `.Value2/.Formula`、PID/identity 与最终 `EXCEL.EXE=0`。
- [ ] 保存后 digest 改变；shadow restore 后逐字节恢复，并 read-only 读回原始值。
- [ ] `.Formula` 断言以真实读取结果为准，不保留未经验证的 fake 猜测。
- [ ] 生产 tools/ToolBus/rollback/assembly、schema、公共 API、ErrorCode、依赖零改动。
- [ ] 专项、全量门禁、coverage 与独立 review 完成；PR/CI/merge/post-merge main CI
  证据落到记录区，Done 状态另行 docs-only backfill。

<!-- 分隔线以下由 Implementer 填写；正文区只读，状态行按 ADR-0083 更新。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-407 Excel T2.2 human-present real COM write acceptance
【目标】test-only 真实 Excel 临时副本完成 writable open、scalar write、Calculate、Value2/Formula 双读、Save、shadow restore 与进程收敛
【write scope】仅：本卡、Windows Excel runner 与新增 test-only calibration、专项测试、platform README、Excel memory、MEMORY 规模表、Stage 2、PLAN、README、LEDGER
【铁律】COM 只在 dedicated worker；只碰临时副本；写后真实回读；无静默失败；资源有界并收敛；不伪造证据
【禁止】不碰用户工作簿；不注册 Tool/ToolBus/rollback/assembly；不改 schema/公共 API/ErrorCode/依赖；不执行宏/VBA/公式写入
【验收】runner fake 与全量门禁全绿；真实 COM 记录 digest、raw Value2/Formula、PID identity、EXCEL.EXE=0
【依赖】TASK-403/404/405/406 Done；ADR-0110/0111/0112 Accepted；LEDGER 已核对
【疑问】无；Formula 精确形状由真实读取校准后编码
```

### 2. 实际改动

- 新增 test-only `excel_com_runner_write`：closed scalar（blank/boolean/finite number/
  bounded non-formula text）、writable open、write、recalculate、save 请求，复用现有
  runner/session 身份校验与容量 1 worker 队列。
- Windows backend 新增 `Range.Value2` property put：`DISPATCH_PROPERTYPUT` +
  `DISPID_PROPERTYPUT`，`cArgs=1/cNamedArgs=1`，参数 VARIANT 在同步 `Invoke` 期间保持
  有效并恰好 `VariantClear` 一次；`Application.Calculate` 与 `Workbook.Save` 走既有
  method helper。
- 真实 COM 验收只复制显式 fixture 到本地临时工作副本与 shadow copy，验证 writable
  open、单 cell number/boolean/text/blank、2-cell numeric range、完整 `.Value2/.Formula`
  双读、保存后 digest 改变、关闭后 handle 失效、owned PID/全机 `EXCEL.EXE=0`、shadow
  restore 回原 digest 与恢复后 read-only 读回。
- 为保持 900 行硬门禁，将 worker response/terminal helpers 拆到
  `excel_com_runner_dispatch.rs`，将 closed VARIANT constructors/collection helper 拆到
  `excel_com_runner_windows_variants.rs`；未使用 `#[allow]`。
- 生产 `tools.json`、ToolBus/rollback/assembly、protocol/schema、`platform/api`、
  ErrorCode 与依赖零改动。

### 3. 门禁输出摘要

- `cargo test -p assistant-platform-windows excel_com_runner` → **21 passed / 2 ignored**。
- 真实 COM final：**1 passed / 0 failed / 7.70 s**；PID `32060`，creation FILETIME
  `134401327988182498`，EXE `C:\Program Files\Microsoft Office\root\Office16\EXCEL.EXE`。
- digest：original/restored
  `0f2d5a65600ac1e17bd97d722c9f6a376133c848bb12639bf507503b874c2b54`；written
  `10bcdafa0154b14dd39f03959c0d345965c2f196bb2ae470aa9fb73a136f8abe`。
- raw `.Formula`：number `41` -> `"41"`、number `1` -> `"1"`、boolean `false` ->
  `"FALSE"`、text `TASK-407` -> `"TASK-407"`、blank -> `Blank`；公式 cell 保持
  `=A1+B1`。2-cell range 的 `.Value2` 为 number `9`/`9`，`.Formula` 为 text
  `"9"`/`"9"`。
- `cargo fmt --all --check`、workspace Clippy、Linux-target platform Clippy → PASSED；
  仅仓库既有 `clippy::assert_is_empty` unknown-lint warning。
- `cargo test --workspace` → PASSED（agent-core lib 183 passed / 1 ignored；最终 workspace
  复跑仍全绿，xtask 587 passed，其余 crate/doctest 全绿）。
- arch、schema/codegen、hygiene、card-check、docscan、refscan、memory-counts、
  adr-index、check-comments、`cargo deny check`、core replay → PASSED；hygiene 0 error。
- `cargo llvm-cov --workspace --fail-under-lines 75` → workspace line coverage **77.87%**。
- `git diff --check` → PASSED。仓库无 `package.json`，`pnpm lint/typecheck/test` 不适用。

### 4. DoD 核对

- [x] writable-open/write/calculate/save 均为 `#[cfg(test)]`，生产公开面与 Tool 注册零扩大。
- [x] property put 使用 `DISPATCH_PROPERTYPUT` / `DISPID_PROPERTYPUT`，VARIANT 清理与
  同步 Invoke 参数生命周期有实现注释和真实 COM 证据；`ManuallyDrop` 保证显式
  `VariantClear` 恰好一次。
- [x] fake 测试覆盖 closed sequence、scalar 边界、stale handle、save 失败与 worker 收敛。
- [x] 人类在场真实 COM 验收完成；PID/identity、原/改/恢复 digest、raw
  `.Value2/.Formula`、最终 `EXCEL.EXE=0` 与临时文件 0 已记录。
- [x] 保存后 digest 改变；shadow restore 逐字节恢复，并 read-only 读回原始值。
- [x] `.Formula` 精确断言以真实读取结果为准。
- [x] 生产 tools/ToolBus/rollback/assembly、schema、公共 API、ErrorCode、依赖零改动。
- [x] 独立 review 复审 **No findings (P0-P2)**；PR #870 checks **11/11 SUCCESS**，
  pending 0 / failed 0，base=main，`MERGEABLE` / `CLEAN`；merge
  `fc024e4a2c6eb3f39213d13838b6664c9941a16e`；post-merge main CI
  `45743239854` success。Done 状态由 docs-only backfill 更新。

### 5. 偏差

- 首轮真实校准因把 2-cell range 的 Formula 类型按 Value2 猜成 number 而失败；guard 按
  完整 owned identity 清理 PID `34748`。真实读取校准为 Formula text `"9"` 后修正断言。
- 一次多 cell 写入后 Quit 已在 20 秒内未退出，PID `20864` 由 guard 清理；最终测试保留
  45 秒有界等待，随后同路径以 PID `11036` 在 7.20 秒通过，未放宽零残留判据。
- Clippy 首轮抓到 worker/test 函数超 80 行及私有模块过宽可见性；已拆 dispatcher、
  VARIANT helper 与测试 helper。拆分首版曾误改 response-loss 的 worker 退出语义，
  workspace 测试捕获后已恢复并定向回归通过。
- hygiene 首轮发现两个 runner 文件超过 900 行硬上限；已按职责拆出
  `excel_com_runner_dispatch.rs` 与 `excel_com_runner_windows_variants.rs`，未提上限或用 allow。
- 独立 review 首轮发现 3 个 P2 + 2 个 P3：`VARIANT` Drop 与显式清理可能重复、公式前缀
  门未覆盖 `+/-/@`、真实测试在 copy 前未证明本地盘、临时 cleanup 非硬断言，以及边界/
  foreign handle 覆盖不足。P2 与主要 P3 已修复：`ManuallyDrop`、完整公式前缀、
  source/temp directory 预校验、成功路径显式 cleanup，以及 NUL/256 KiB/INF/foreign
  handle 测试；复审 **No findings (P0-P2)**。
- 复审仍保留一个 P3：首次 copy 已执行但后续文件校验失败时，清理依赖 guard 的 best-effort
  `Drop`；成功路径、真实 COM 路径和本次证据不受影响，后续重复运行卡可继续收紧失败注入清理。

### 6. 新增长期记忆

- 已追加到 `docs/memory/apps/excel.md`：property-put named DISPID、真实
  number/boolean/text/blank Formula 形状，以及保存后 teardown 的有界慢退出观察。

### 7. 遗留与下一步

- 本卡只完成 test-only、人类在场单次真实写入校准与 shadow restore；重复运行 10/10、
  ownership journal、独立 watchdog、用户工作簿/网络盘、宏与生产 Tool registration 仍待后续。
- 生产 `tools.json`、rollback recipe、assembly 与 ToolBus 必须继续保持空，直到 ADR-0110
  的重复收敛和全部真机门槛通过。

### 8. 外部证据

- fixture：`C:\Users\fexfe\AppData\Local\Temp\task407-real-com-write-source.xlsx`
  （Sheet1：A1=2、B1=3、C1=TRUE、D1=`hello`、E1 blank、F1=`=A1+B1`）。
- 真实 COM final：1 passed / 0 failed / 7.70 s；PID `32060`；source digest 未变；
  `EXCEL.EXE=0`；`assistant-task407-*.xlsx` 临时文件 0。
- implementation PR #870：https://github.com/snofin18/ai-assistant/pull/870
  checks **11/11 SUCCESS**，pending 0 / failed 0，base=main，`MERGEABLE` / `CLEAN`。
- merge `fc024e4a2c6eb3f39213d13838b6664c9941a16e`；post-merge main CI
  `45743239854` success。

### 9. 给审阅者的关注点

- 最高风险是 test-only property put 的 COM ABI：named DISPID 指针、参数 VARIANT 和
  `VariantClear` 必须严格同步且恰好一次。
- 45 秒 teardown 窗口只覆盖有界等待，不应被误读为重复运行稳定性；两次异常退出均由
  owned identity guard 回收，必须在后续重复收敛卡继续观察。
- 真实验收依赖外部 openpyxl fixture；fixture 只含校准数据，未写入仓库，也未触碰用户工作簿。
