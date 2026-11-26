# TASK-412 Excel T2.2 platform-private watchdog and verified termination

- 状态：**Done**
- 阶段：2　子阶段：Stage 2 Excel T2.2　批次：6g　依赖：TASK-401、TASK-402、TASK-403、TASK-409、TASK-410、TASK-411、ADR-0108、ADR-0110、ADR-0111、ADR-0112

---

## write scope

- `tasks/TASK-412-excel-t22-platform-private-watchdog.md`
- `crates/platform/windows/src/excel_process.rs`
- `crates/platform/windows/src/excel_process_watchdog.rs`
- `crates/platform/windows/src/excel_process_watchdog_windows.rs`
- `crates/platform/windows/src/excel_process_watchdog_tests.rs`
- `crates/platform/windows/src/excel_com_runner.rs`
- `crates/platform/windows/src/excel_com_runner_state.rs`
- `crates/platform/windows/src/excel_com_runner_windows.rs`
- `crates/platform/windows/src/excel_com_runner_windows_startup.rs`
- `crates/platform/windows/src/excel_com_runner_tests.rs`
- `crates/platform/windows/src/excel_com_runner_journal_tests.rs`
- `crates/platform/windows/README.md`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

在 TASK-411 的 platform-private ownership journal 之上实现不共享 Excel STA 阻塞路径的
有界 watchdog，并只对仍匹配 PID、creation FILETIME 与 executable path 的 owned
Excel 进程树执行 verified termination；response wait、worker join 与 termination wait
必须分别有界。

本卡只闭合 `assistant-platform-windows` 内的 process lifecycle primitives，不接真实
task runtime、production Tool 或 Host 进程崩溃后的外进程恢复。

## In scope

- 新增 platform-private watchdog controller/supervisor 与可替换的 test-only process
  operations；watchdog 线程不初始化 COM、不调用 STA，也不共享 COM 对象。
- 在 owned identity 捕获后 arm watchdog；Excel 完整退出后 disarm；shutdown 超时、
  worker disconnect 或 panic 路径可显式请求 verified force termination。
- 终止前通过同一个 process handle 复核 PID、creation FILETIME、executable path；
  禁止裸 PID 或 `taskkill /PID`。
- 有界枚举所持进程树，为每个候选持有 process handle 并复核快照中的 PID、
  parent id、creation FILETIME；无法复核的候选不得终止。
- response wait、worker join/reap、termination polling 三层分别使用独立 deadline；
  任何一层超时均显式失败，不能转成成功。
- 区分 `OpenProcess` 的“进程已不存在”和“访问拒绝/权限不足”，不得继续把任意
  `OpenProcess` 失败折叠为 `None`。
- cleanup 公共结果遵循 ADR-0108 D9：`UserInteraction` 保持；结构、身份终止或
  reap 失败映射为 `Fatal`；不新增 `ErrorCode`。
- fake tests 覆盖 arm/disarm、stale identity、watchdog disconnect、termination
  failure、bounded timeout；Windows 专项测试覆盖受控子进程树终止与 reap。
- 同步 crate README、Stage 2、PLAN、README、LEDGER，并完成独立 review、PR、CI、
  merge 与 post-merge main CI。

## Out of scope

- 不实现 Host 进程整体崩溃后的独立 helper/watchdog process 或下一次启动恢复。
- 不接 production task-end caller、active-session registry、incident 发布或
  agent-core runtime。
- 不新增 production Tool、`tools.json`、ToolBus、rollback recipe、assembly 或真实
  task 调用路径。
- 不修改 `assistant-platform-api`、protocol schema、Tool schema、ErrorCode、公共
  trait 或第三方依赖。
- 不启动真实 Excel，不操作真实工作簿，不触碰用户 Excel 进程，不进行 GUI 操作。
- 不把 PowerShell TASK-409 harness 打包为产品依赖。

## 必须遵守

1. watchdog 只能使用完整 `ExcelOwnedProcessIdentity`；PID 相同但 start time 或
   executable path 不同必须视为非 owned 进程，终止调用必须失败而不 kill。
2. `OpenProcess` 返回 access denied 时必须保留为显式权限/结构失败；只有明确的不存在
   错误才可视为已收敛。
3. 每个 process-tree handle 在终止前必须与 snapshot 的 PID、parent id 和 creation
   FILETIME 交叉复核；snapshot 条目数必须有硬上限。
4. 禁止 `worker.join()`、watchdog response、process wait 任一无限等待。
5. watchdog 保留 armed identity 时不得静默关闭；必须 disarm 或显式终止并确认收敛。
6. 失败 cleanup 必须保留 TASK-411 journal；不得为了让 shutdown 返回成功而删除证据。
7. 测试不得修改断言规避缺陷，不得新增 `#[allow]`、`unsafe` 豁免或第三方依赖。

## 验收命令

```powershell
cargo fmt --all --check
cargo test -p assistant-platform-windows
cargo clippy --all-targets -- -D warnings
cargo clippy --target x86_64-unknown-linux-gnu -p assistant-platform-windows --all-targets -- -D warnings
cargo clippy --target aarch64-apple-darwin -p assistant-platform-windows --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-core arch::
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-comments
cargo run -p xtask -- docscan
cargo deny check
git diff --check
```

预期：专项 watchdog/runner fake tests 与 Windows 受控子进程树测试覆盖 DoD；上述门禁
除仓库既有 warning/skipped check 外全部通过。

## DoD

- [ ] watchdog 独立线程不初始化 COM，arm/disarm/force termination 的状态转移显式且有界。
- [ ] owned Excel shutdown 超时或 worker 失联时可请求 verified force termination，
      不再仅 detach 后返回文本错误。
- [ ] 终止前完整身份复核；stale PID/start/path 均拒绝 kill，并有负向测试。
- [ ] process-tree 枚举有硬上限，候选在 kill 前复核 PID、parent id、creation FILETIME；
      受控子进程树测试证明 root 与 descendant 均被 reap。
- [ ] response wait、worker join/reap、termination polling 三层分别有 deadline，
      超时测试不会无限阻塞。
- [ ] `OpenProcess` 的不存在与 access denied 分离，公共 shutdown 的
      `UserInteraction`/`Fatal` 映射符合 ADR-0108 D9。
- [ ] cleanup 失败保留 journal；只有 watchdog 与 worker 均收敛且 graceful cleanup
      成功后删除 journal。
- [ ] 不新增公共 API、schema、ErrorCode、依赖、production Tool/ToolBus/rollback/
      assembly，不启动真实 Excel/GUI。
- [ ] 专项与全量门禁、独立 review no P0-P2、PR/CI/merge/main CI 证据落入记录区。

<!-- 分隔线以下由 Implementer 填写；正文区只读，状态行按 ADR-0083 更新。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-412 Excel T2.2 platform-private watchdog and verified termination
【目标】以独立有界 watchdog 复核完整 owned identity 后收敛 Excel 进程树
【write scope】仅本卡、Windows excel process/watchdog/runner/state/tests、crate README、Stage 2、PLAN、README、LEDGER
【铁律】无静默失败；完整身份才可终止；资源有界；不改公共 API/schema/ErrorCode/依赖；production Tool surface 保持空
【禁止】不启动真实 Excel/GUI、不触碰用户 Excel、不做 Host 崩溃 helper process、不接 production task/runtime/ToolBus/tools.json
【验收】watchdog/runner 专项 fake tests + Windows 受控子进程树测试 + fmt/clippy/test/arch/schema/codegen/doc gates/deny/diff-check
【依赖】TASK-409/410/411 Done；ADR-0108 D8/D9、ADR-0110/0111/0112 已核对；N_auto=0
【疑问】ADR 的“独立线程”不覆盖 Host 整体崩溃；默认只实现 Accepted D8 的进程内 watchdog，并保留后续 ADR clarification
```

### 2. 实际改动

- 新增 platform-private `ExcelProcessWatchdog` / controller：独立线程不进入 COM STA；
  Arm 时立即 capture，armed 期间每 50 ms 刷新 tracked descendants，并以 sticky monitor
  error 阻止“capture 不完整但仍 disarm/删 journal”。
- 新增 Windows verified process-tree backend：只从已打开 handle 复核完整 PID、creation
  FILETIME、executable path 后终止；tracked tree 有 256 descendants / 8 snapshots 上限，
  stable snapshot 复核 PID、parent 与 image name，snapshot-time FILETIME ceiling 拒绝
  snapshot 后创建/复用的 PID generation。
- runner state 在 identity 捕获后 arm；Quit 前同步 capture；Close/Quit/worker 失联、
  panic 或 timeout 路径可执行 armed-only force termination。response wait、worker reap、
  termination wait 与 watchdog thread reap 分别有独立 deadline。
- arm 失败保留 `created` / `owned_process` / journal；`OpenProcess` 区分不存在与
  access denied；`UserInteraction` 保持，结构/reap/身份失败映射 `Fatal`。
- startup timeout 将 `JoinHandle` 移交给最多 1 个 quarantine reaper；startup 入口使用
  全局 try-lock，避免 quarantine 上限 TOCTOU 和并发 startup 抖动。
- 新增 fake watchdog 状态机/周期 capture/sticky error、stale identity 不 kill、隐藏
  受控子进程树 root+descendant reap、arm 失败证据保留与 startup reaper 测试。
- 同步 crate README、Stage 2、PLAN、README、LEDGER；未改公共 API、schema、ErrorCode、
  第三方依赖、production Tool/ToolBus/rollback/assembly 或 `tools.json`。

### 3. 门禁输出摘要

- `cargo test -p assistant-platform-windows`：**196 passed / 0 failed / 8 ignored**。
- `cargo test --workspace`：全 workspace tests 与 doctests 通过。
- `cargo fmt --all --check`、`git diff --check`：PASSED。
- host / `x86_64-unknown-linux-gnu` / `aarch64-apple-darwin` Clippy：exit 0；仅仓库既有
  `clippy::assert_is_empty` unknown-lint warning。
- `cargo test -p assistant-core arch::`、`verify-schemas`、`codegen --check`：PASSED。
- `hygiene`、`card-check`、`docscan`、`memory-counts`、`adr-index`、`check-ledger`、
  `check-comments`：PASSED（仓库既有 warning 保留）。
- `cargo deny check`：advisories / bans / licenses / sources 全部 ok。
- 独立 review 经五轮增量修复后最终 **No findings (P0-P2)**；仍有两个 P3 测试补强建议：
  Force sticky 分支的 armed 保留直断言、snapshot FILETIME ceiling 边界矩阵。
- implementation PR #880 checks **22（21 SUCCESS + 1 SKIPPED）**，pending 0 / failed 0，
  base=main，`MERGEABLE` / `CLEAN`；merge
  `0571dce5295daef8d56a64bd9a5610f0e6cbf19d`；post-merge main CI `45823476783` success。

### 4. DoD 核对

- [x] watchdog 独立线程不初始化 COM；arm / capture refresh / disarm / force termination
      状态转移显式，response 与 thread reap 有界。
- [x] shutdown 超时、worker 失联和 panic 路径可请求 verified force termination，并保留
      未收敛的 armed supervisor 供 Drop 重试。
- [x] 终止前完整身份复核；stale PID/start/path 均拒绝 kill，且有 Windows 负向测试。
- [x] process-tree 有 256 handles / 8 snapshots 硬上限；snapshot-time 与 stable identity
      交叉复核；受控子进程树测试证明 root + descendant 均被 reap。
- [x] response wait、worker join/reap、termination polling 与 watchdog reap 各有 deadline；
      timeout 测试在有限时间内返回失败。
- [x] `OpenProcess` 不存在/access denied 分离；cleanup 公共结果符合
      `UserInteraction` 保持、结构失败 `Fatal`。
- [x] capture 失败时 sticky error 阻止 Converge/Force 成功及 journal 删除；只有重新 Arm
      成功才清空。
- [x] 未新增公共 API、schema、ErrorCode、依赖、production Tool/ToolBus/rollback/
      assembly，未启动真实 Excel/GUI。
- [x] 专项/全量门禁与独立 review 完成；PR #880 / CI / merge / main CI 已回填。

### 5. 偏差

- 首轮实现后独立 review 依次发现 natural root-first descendant window、arm failure
  证据丢失、snapshot PID generation race、worker handle early-drop、monitor error
  late propagation 与 startup quarantine TOCTOU；均已修复并新增/调整测试。
- 为满足 600/900 行硬门禁，新增 `excel_com_runner_windows_startup.rs` 并把 bounded join
  测试移入 journal lifecycle tests；未改变公共边界。
- 工具偏差：本轮误填 `shell: "bash"` **63** 次；精确值已写入外部 JSONL。

### 6. 新增长期记忆

- 无新增。TASK-412 的可复用事实由本卡、`crates/platform/windows/README.md` 与
  `LEDGER.md` 承载；未改 `MEMORY.md` / `docs/memory/*`。

### 7. 遗留与下一步

- Host 进程整体崩溃后的外进程恢复仍未实现，需后续 ADR clarification + helper process
  设计；不得把本卡进程内 watchdog 宣称为 Host-crash recovery。
- production task-end caller、真实 Excel/GUI hang 注入、production `tools.json` /
  ToolBus / rollback / assembly registration 仍未开放。
- 两个 P3 测试补强：Force sticky 的 armed 保留直断言、snapshot FILETIME ceiling 边界矩阵。

### 8. 外部证据

- implementation PR #880：https://github.com/snofin18/ai-assistant/pull/880
  checks **22（21 SUCCESS + 1 SKIPPED）**，pending 0 / failed 0，base=main，
  `MERGEABLE` / `CLEAN`。
- merge `0571dce5295daef8d56a64bd9a5610f0e6cbf19d`；post-merge main CI
  `45823476783` success。
- 运行证据：`C:\Users\fexfe\.codex\automations-logs\2026-11-26\ai-assistant-task-round-20261126-1930.jsonl`
  （最终 shell misuse 精确计数与 gates/work/pr/ci/merge/main_ci/final 事件将写在该文件）。

### 9. 给审阅者的关注点

- 核对 arm 后立即 capture + 50 ms refresh 的 tree tracking 是否能阻止 root-first
  descendant 在 disarm 后残留。
- 核对 sticky monitor error 是否确保 Converge/Force 不删除 journal，并保留 armed
  supervisor 供 Drop 重试。
- 核对 `OpenProcess`、snapshot FILETIME ceiling、open-handle full identity 复核三者共同
  约束下不存在裸 PID kill。
- 核对 startup quarantine 的 try-lock/CAS 语义不会引入死锁或无限等待。
