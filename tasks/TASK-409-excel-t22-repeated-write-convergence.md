# TASK-409 Excel T2.2 repeated write convergence and watchdog evidence

- 状态：**Done**
- 阶段：2　子阶段：Stage 2 Excel T2.2　批次：6d　依赖：TASK-403、TASK-404、TASK-405、TASK-406、TASK-407、TASK-408、ADR-0063、ADR-0107、ADR-0108、ADR-0110、ADR-0111、ADR-0112

---

## write scope

- `tasks/TASK-409-excel-t22-repeated-write-convergence.md`
- `crates/platform/windows/src/excel_com_runner_tests.rs`
- `crates/platform/windows/src/excel_com_runner_repeat_tests.rs`
- `crates/platform/windows/README.md`
- `eval/tasks/excel/t22/test_excel_t22_repeated_write.ps1`
- `eval/tasks/excel/t22/README.md`
- `eval/tasks/excel/t22/evidence/2026-11-26-task-409/**`
- `docs/audits/stage-2-excel-t22-repeat-2026-11-26.md`
- `docs/memory/apps/excel.md`
- `MEMORY.md`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

在 test-only 边界内完成真实 Excel COM 重复写入收敛：连续 10 次独立进程完成本地临时
`.xlsx` 副本的 writable open、scalar write、Calculate、完整 `.Value2/.Formula` 双读、
Save、Close、stale handle/session 拒绝与进程归零；同时加入失败注入、ownership journal
和独立 watchdog，证明异常/超时路径只回收已复核身份的 owned Excel，不扩大生产 Tool、
公共 API 或 ToolBus surface。

## In scope

- 新增 test-only 真实 COM 重复写入验收；每次运行使用新 runner、新 owned Excel 进程和
  新本地临时工作副本，源 fixture 与 shadow copy 只读，工作副本保存后必须改变 digest。
- 每轮关闭后验证旧 workbook handle 对 write/recalculate/save/read 全部显式失败；每轮
  shutdown 后等待该 owned PID 退出，并断言全机 `EXCEL.EXE=0`。
- 在可能阻塞的 writable open / Close / Quit 之前写入仓库外 ownership journal，字段至少
  包含 `process_id`、`start_time_100ns`、`executable_path`、run 序号和工作副本路径。
- 新增外部 PowerShell watchdog：Rust 子进程超时时读取 journal，重新校验 PID、
  creation FILETIME、可执行路径三者，再只终止仍匹配的 Excel 进程树和本次 child tree。
- 新增可重复失败注入：真实 COM 在 after-write checkpoint 主动失败，验证进程最终归零；
  另以 after-open intentional hang 验证独立 watchdog 的 journal 读取和身份复核清理。
- 新增 test-only fake 负向覆盖：writable open、write、recalculate、save 注入失败后
  session 仍可显式 close/shutdown，关闭后的旧 handle 不再触达 backend。
- 证据写入 `eval/tasks/excel/t22/evidence/2026-11-26-task-409/**` 与审计文档。
- 同步 platform README、Excel app memory、MEMORY 规模表、Stage 2、PLAN、README、LEDGER。
- 独立 review、PR、CI、merge 与 post-merge main CI 收口；Done 状态另行 docs-only backfill。

## Out of scope

- 不填写 `adapters/com.microsoft.excel/tools/tools.json`、rollback recipe 或 assembly。
- 不注册 production ToolBus，不建立模型可见 `excel.range.write` Tool。
- 不修改 protocol/schema、公共 `platform/api` trait、ErrorCode、Policy、依赖或现有
  production `ExcelComRunner` 公开方法面。
- 不实现生产 ownership journal/watchdog、崩溃恢复、宏/VBA、公式写入、图表、格式、
  批量事务或其他用户的 Excel 会话。
- 不打开用户工作簿、OneDrive/SharePoint/UNC、`.xlsm/.xlsb` 或既有 Excel 实例。
- 不修改 `DisplayAlerts`，不静默覆盖文件，不把 fake、注入或一次异常清理冒充 10/10
  正常路径。

## 必须遵守

1. 正常验收开始前必须确认全机 `EXCEL.EXE=0`；任何既有 Excel 都 fail-closed。
2. 只对显式本地 `.xlsx` fixture 的临时副本执行写入；源 fixture 和 shadow copy 不得
   被写入，源 digest 在整轮结束后必须不变。
3. ownership journal 必须先于 writable open、Close 和 Quit 落盘；watchdog 未同时匹配
   PID、creation FILETIME 和 executable path 时禁止 kill。
4. 任一轮 write/recalculate/double-read/save/close/shutdown/process convergence 失败都
   必须保留真实错误；不得用重试掩盖或把失败轮计为成功。
5. 10/10 只统计真实正常路径；failure injection 与 watchdog timeout 分开记录，不能
   混入正常成功率。
6. fake 测试、真实 COM 正常路径、失败注入和超时清理必须分别输出证据，不得互相冒充。
7. 所有等待有界；超时后必须显式失败，除非外部 watchdog 场景本身预期 timeout 且完成
   已复核身份清理。

## 验收命令

```powershell
cargo test -p assistant-platform-windows excel_com_runner
pwsh -NoProfile -File eval/tasks/excel/t22/test_excel_t22_repeated_write.ps1 `
  -SourceWorkbook '<explicit local .xlsx fixture>' `
  -EvidenceRoot eval/tasks/excel/t22/evidence/2026-11-26-task-409
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

预期：fake/真实正常路径全绿；真实正常路径 10/10，每轮结束 owned PID 与全机
`EXCEL.EXE=0`，源 fixture digest 不变；after-write failure injection 最终归零；
after-open watchdog timeout 只终止 journal 身份仍匹配的 Excel；production Excel
surface 仍为空。

## DoD

- [ ] 真实 COM 正常路径连续 10/10 通过，每轮新 runner/session、旧 handle 失效、
  `EXCEL.EXE=0`，无失败轮混入成功率。
- [ ] 每轮工作副本保存后 digest 改变，shadow restore 后回到原 digest，源 fixture
  digest 不变。
- [ ] ownership journal 在危险操作前落盘，watchdog 只对 PID + creation FILETIME +
  executable path 全匹配的 Excel 执行有界回收。
- [ ] after-write failure injection 与 after-open watchdog timeout 均有独立证据和
  最终进程/临时文件收敛结论。
- [ ] fake 注入覆盖 open/write/recalculate/save 失败，并证明 close/shutdown 与 stale
  handle 拒绝路径。
- [ ] production tools/ToolBus/rollback/assembly、schema、公共 API、ErrorCode、依赖零改动。
- [ ] 专项、全量门禁、coverage 与独立 review 完成；PR/CI/merge/post-merge main CI
  证据落到记录区，Done 状态另行 docs-only backfill。

<!-- 分隔线以下由 Implementer 填写；正文区只读，状态行按 ADR-0083 更新。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-409 Excel T2.2 repeated write convergence and watchdog evidence
【目标】test-only 真实 COM 10/10 重复写入、失败注入、进程/会话/句柄收敛、ownership journal 与独立 watchdog
【write scope】仅：本卡、Windows Excel repeat tests/入口、platform README、Excel T2.2 脚本/证据、审计、Excel app memory、MEMORY、Stage 2、PLAN、README、LEDGER
【铁律】真实证据不得伪造；写后必须回读；资源有界并收敛；完整身份才可 kill；public surface 不得扩大；热门文档写前 guard
【禁止】不注册 production Tool/ToolBus/rollback/assembly；不改 schema/公共 API/ErrorCode/依赖；不碰用户/网络/宏工作簿；不修改 DisplayAlerts
【验收】fake 与真实 COM 专项、10/10、after-write 失败注入、20 秒 watchdog、全量门禁、coverage、review、PR/CI/merge
【依赖】TASK-403/404/405/406/407/408 Done；ADR-0063/0107/0108/0110/0111/0112 已核对；N_auto=0
【疑问】无；按预授权推进，真实测试只使用显式本地 fixture 的临时副本
```

### 2. 实际改动

- 新增 `crates/platform/windows/src/excel_com_runner_repeat_tests.rs`：test-only 真实
  COM 重复写入、fresh runner/process/session/workbook copy、`.Value2/.Formula` 双读、
  Save、stale-handle 拒绝、逐轮 digest、failure/hang checkpoint、ownership journal 与
  fake 失败注入。
- `excel_com_runner_tests.rs` 只增加 Windows-only test module 接线，保持 900 行硬门禁。
- 新增 `eval/tasks/excel/t22/test_excel_t22_repeated_write.ps1`：独立 PowerShell
  父进程执行 normal/failure/watchdog 三场景；超时时读取 journal，以 PID + creation
  FILETIME + executable path 复核后回收，并清理本次 run token 的临时文件。
- 外部 watchdog 还独立轮询并保存 `watchdog-observed-excel.json`；cargo/test child
  通过 Job Object 终止和 reap，Excel 通过持有并复核的 process handle 终止，避免
  check-then-taskkill 的 PID 复用窗口。
- 正常路径正式次数在 Rust 与 PowerShell 两侧都固定为 10；`Save` 后另起 read-only
  runner 重新打开工作副本，验证持久化后的 `A1/F1` 双读值，再恢复 shadow。
- 新增 `eval/tasks/excel/t22/README.md` 与
  `docs/audits/stage-2-excel-t22-repeat-2026-11-26.md`。
- 更新 platform README、Excel app memory、MEMORY 规模表、Stage 2 当前进度与
  plan 新卡行。
- 未改 production `tools.json`、ToolBus、rollback、assembly、schema、公共 API、
  ErrorCode 或依赖。

### 3. 门禁输出摘要

- `cargo test -p assistant-platform-windows excel_com_runner` →
  **22 passed / 0 failed / 3 ignored**。
- 真实证据：10/10 normal passed；源 digest
  `75c44d93c9e5c8678d8585bf18347bb026fc7be0a2781a893a047ea79f4a1d12`，10 轮 written
  digest 均不同，10 轮 restored digest 全部回到源 digest；首/末 owned PID 为
  `37168` / `37072`，最终 `EXCEL.EXE=0`。
- after-write failure injection：cargo exit `101`，真实 owned PID `31132`，journal
  保留后由 test guard 回收，最终 `EXCEL.EXE=0`。
- after-open watchdog：20 秒超时，journal owned PID `39000`，creation FILETIME
  `134401487976242301`；Excel 通过已打开并复核的 process handle 终止，cargo/test
  child 由 Job Object 回收，临时文件 2 个清理，最终 `EXCEL.EXE=0`。
- `cargo fmt --all --check`、workspace Clippy、Linux-target platform Clippy →
  PASSED；仅有仓库既有 `clippy::assert_is_empty` unknown-lint warning。
- `cargo test --workspace` → PASSED（agent-core lib 183 passed / 1 ignored；xtask
  587 passed；其余 crate/doctest 全绿）。首次 workspace run 被 `handle_discipline`
  对新增 test-only `to_json` 词法扫描拦截，改名后定向回归与全量复跑全绿。
- arch、verify-schemas、codegen、hygiene、card-check、docscan、refscan、
  memory-counts、adr-index、check-ledger、check-comments、`cargo deny check`、
  core replay → PASSED；hygiene 0 error，coverage **77.88%**。
- PowerShell AST parse → `PARSE_OK`；最终 normal `EXCEL.EXE=0`、related process 0、
  TASK-409 temp file 0。
- `git diff --check` → PASSED。

### 4. DoD 核对

- [x] 真实 COM 正常路径连续 10/10；每轮新 runner/session、旧 handle 失效、进程归零。
- [x] 每轮 saved digest 改变，shadow restore 回源 digest，源 fixture digest 不变。
- [x] ownership journal 在危险操作前落盘；watchdog 只按完整 owned identity 回收。
- [x] after-write failure injection 与 after-open watchdog 有独立证据并最终收敛。
- [x] fake 注入覆盖 open/write/recalculate/save 失败、close/shutdown 与 stale handle。
- [x] production tools/ToolBus/rollback/assembly、schema、公共 API、ErrorCode、依赖零改动。
- [x] 独立 review 复审 **No findings (P0-P2)**；PR #874 checks
  **22（21 SUCCESS + 1 SKIPPED）**，pending 0 / failed 0，base=main，
  `MERGEABLE` / `CLEAN`；merge `b18603c359e62f92c85791f5713a239a508051ad`；
  post-merge main CI `45773046164` success。Done 状态由 docs-only backfill 更新。

### 5. 偏差

- 首轮 smoke 在 watchdog 场景先执行了 journal cleanup，再检查 live identity，导致
  误报无 live identity；已改为先读取/复核 journal，再执行清理。
- 首轮正式 harness 仍允许 `-NormalRunCount 1`，且 watchdog 依赖可空路径与
  check-then-taskkill；独立 review 发现后已移除正常次数参数、严格拒绝非 10 轮，
  改为 Job Object + handle 复核终止 + 独立观测 journal。
- 上述首轮异常还暴露 harness 在异常路径没有回收 cargo/test child；已新增
  `Invoke-EmergencyCleanup`，并实际清理遗留 process tree 后重跑通过。
- `handle_discipline` 将 test-only `to_json` 方法名当作可序列化包装；改为
  `render_evidence_record`，不放宽扫描规则。
- 第二轮 review 发现 observed Excel 未绑定 Job 时有误杀风险、cargo 启动早于
  Job 绑定，以及 pre-journal Drop 仍可能 graceful Quit；已改为 wrapper 先入
  Job、observed identity 必须仍是 Job 成员、pre-journal 只 detach runner。
- 第三轮 review 发现 wrapper 已退出但 Job 仍有成员时未收敛；已改为
  `TerminateJobObject` + 有界轮询 `GetJobProcessIds` 至空，再关闭 handle。
- 真实 COM 证据来自当前本机 Office build 20527 和本地临时 fixture；不代表用户
  工作簿、网络盘、宏或 production task cleanup。

### 6. 新增长期记忆

- `docs/memory/apps/excel.md` 新增 TASK-409 10/10 独立进程写入、failure injection、
  ownership journal 与 20 秒 watchdog 证据；`MEMORY.md` 同步 `apps/excel.md`
  140 行规模。

### 7. 遗留与下一步

- production ownership journal/watchdog、真实 task-end cleanup、崩溃恢复与 production
  Tool registration 仍未实现。
- 下一张卡应先审阅本 harness 的失败/watchdog 语义，再决定是否把 journal/watchdog
  提升为 production runner 边界；不得直接把 test-only harness 宣称为生产事务。

### 8. 外部证据

- 证据根：
  `eval/tasks/excel/t22/evidence/2026-11-26-task-409/20261126T063537240Z`。
- normal：`normal/normal-summary.json`、`normal/runs.jsonl`。
- failure：`failure-injection/ownership-run-01-write.json`、`stderr.txt`。
- watchdog：`watchdog-injection/ownership-run-01-write.json`、
  `watchdog-observed-excel.json`、`summary.json`。
- 源 fixture：
  `C:\Users\fexfe\AppData\Local\Temp\task409-repeated-write-source.xlsx`。
- implementation PR #874：https://github.com/snofin18/ai-assistant/pull/874
  checks **22（21 SUCCESS + 1 SKIPPED）**，pending 0 / failed 0，base=main，
  `MERGEABLE` / `CLEAN`。
- merge `b18603c359e62f92c85791f5713a239a508051ad`；post-merge main CI
  `45773046164` success。

### 9. 给审阅者的关注点

- 重点核对 watchdog 是否在超时前只读 journal，且任何 terminate 前都重新验证三项身份。
- 重点核对 normal 10/10 的每轮 digest/摘要来自真实临时副本，而不是 fake 或单次
  smoke 统计。
- 重点核对 failure injection 的 panic 清理只被记录为异常证据，未混入 10/10；同时
  确认没有扩大 production public runner surface。
