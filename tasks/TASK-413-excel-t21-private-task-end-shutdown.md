# TASK-413 Excel T2.1 private task-end shutdown convergence

- 状态：**Done**
- 阶段：2　子阶段：Stage 2 Excel T2.1　批次：4j　依赖：TASK-403、TASK-411、TASK-412、ADR-0108

---

## write scope

- `tasks/TASK-413-excel-t21-private-task-end-shutdown.md`
- `apps/agent-core/src/excel_t21.rs`
- `apps/agent-core/src/excel_t21_model.rs`
- `apps/agent-core/src/excel_t21_handlers.rs`
- `apps/agent-core/src/excel_t21_lifecycle_tests.rs`
- `apps/agent-core/src/excel_t21_platform_runtime.rs`
- `apps/agent-core/src/excel_t21_platform_runtime_tests.rs`
- `apps/agent-core/README.md`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

在 test-only 的 Excel T2.1 handler 与 platform runtime 之间补齐统一 task-end cleanup：
先原子进入 closing 并立即失效全部 session/token，再按有界顺序关闭 workbook、shutdown
runner；cleanup 未收敛时保留 closing/session 证据并返回显式失败。

本卡只闭合私有生命周期状态机，不把 Excel handler、runtime 或 Tool 注册到生产装配。

## In scope

- 给私有 `ExcelT21Runtime` 增加显式 `shutdown` 生命周期方法；平台 adapter 与 fake
  runtime 保持同一错误语义。
- 给 `ExcelT21Handlers` 增加单次 task-end `shutdown`：CAS `running -> closing`，随后拒绝
  open/read/summarize/重复 shutdown。
- cleanup 开始时把全部 active session 移入 closing，token 立即失效；关闭成功的 session
  才释放容量，runner 整体收敛成功后才清理剩余 closing session。
- `PlatformExcelT21Runtime::shutdown` 在调用可能阻塞的 runner cleanup 前发布本地 closing
  状态并释放 runtime mutex，使并发调用立即失败而不是等待整段 cleanup。
- runner shutdown 失败时标记 terminal、保留 workbook/session 证据，不伪造收敛；成功后才清空
  workbook 映射与应用 ready 状态。
- 保持 `UserInteraction` 错误码；结构、身份、runner/reap 与状态机失败映射 `Fatal`。
- fake tests 覆盖立即失效、并发拒绝、全部 session close、close 失败、shutdown 失败保留与
  成功清理；Windows 真实验收命令只编译，不在本卡运行。
- 同步 agent-core README、Stage 2、PLAN、README、LEDGER，并完成独立 review、PR、CI、
  merge 与 post-merge main CI。

## Out of scope

- 不注册 `excel.*` production ToolBus Tool，不填写 `tools.json`、rollback recipe 或 assembly。
- 不新增 Host helper process、跨进程恢复协议、Job Object 生产接线或 GUI。
- 不启动真实 Excel，不打开或写入真实工作簿，不触碰用户 `EXCEL.EXE`。
- 不修改 protocol schema、`assistant-platform-api`、`assistant-platform-windows` 公共面、
  ErrorCode、Policy 或第三方依赖。
- 不把本卡的 fake lifecycle 测试当作 production Tool registration 或真实 task assembly 证据。

## 必须遵守

1. task-end 入口必须先用原子状态转移进入 closing，任何新调用都立即失败。
2. active session 移入 closing 后 token 立即失效；不得等待 runner cleanup 才失效。
3. workbook close 成功后才释放该 session 容量；runner 未确认收敛时不得删除剩余 closing 证据。
4. 不得持 session/runtime mutex 执行可能阻塞的 close、Quit 或 worker reap。
5. shutdown 只能执行一次；重复或并发入口显式失败，不返回伪成功。
6. cleanup 失败必须返回既有 ErrorCode；`UserInteraction` 不降级为成功，结构失败不得伪装成
   可重试目标错误。
7. 不新增公共 API、schema、ErrorCode、依赖、`#[allow]` 或测试断言放宽。

## 验收命令

```powershell
cargo test -p assistant-agent-core excel_t21_lifecycle
cargo test -p assistant-agent-core excel_t21_platform_runtime
cargo test -p assistant-agent-core excel_t21
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-core arch::
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-comments
cargo deny check
git diff --check
```

预期：专项 lifecycle/fake tests 与全量门禁通过；production Excel Tool surface 保持空；
未运行真实 Excel/GUI，不声称 production task-end caller 已开放。

## DoD

- [ ] handler task-end 通过单次 `running -> closing` CAS 拒绝新 open/read/summarize 与重复 shutdown。
- [ ] cleanup 开始时全部 active session 立即不可 lookup；close 成功才释放容量。
- [ ] runner 整体 shutdown 成功后才清理剩余 closing session；失败时保留证据。
- [ ] platform runtime 在阻塞 cleanup 前进入 closing 并释放 mutex，失败时保留 workbook 映射。
- [ ] `UserInteraction` 保持；结构/runner 失败为 `Fatal`；无静默丢弃的 cleanup 错误。
- [ ] 专项测试覆盖成功、close 失败、shutdown 失败、并发调用和重复 shutdown。
- [ ] production `tools.json`、ToolBus、rollback、assembly、protocol/schema、公共 API 与依赖零改动。
- [ ] 全量门禁、独立 review no P0-P2、PR/CI/merge/main CI 证据落入记录区。
- [ ] 外部 automation JSONL 记录 start/gates/work/pr/ci/merge/main_ci/final 与 shell misuse。

<!-- 分隔线以下由 Implementer 填写；正文区只读，状态行按 ADR-0083 更新。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-413 Excel T2.1 private task-end shutdown convergence
【目标】为私有 Excel handler/runtime 增加原子 closing、token 立即失效与有界 task-end cleanup 收敛
【write scope】仅：本卡、agent-core Excel lifecycle/model/handler/platform runtime 与测试、README、Stage 2、PLAN、README、LEDGER
【铁律】无静默失败；cleanup 前先 closing；失败保留证据；ErrorCode 原样保真；热门文档写前 guard
【禁止】不填 tools.json、不注册 ToolBus/rollback/assembly、不改公共 API/schema/依赖、不运行真实 Excel/GUI
【验收】lifecycle/platform runtime 专项 + fmt/clippy/workspace/arch/schema/codegen/文档门禁/deny/diff-check
【依赖】TASK-403、TASK-411、TASK-412 Done；ADR-0108 D3/D8/D9 已核对；N_auto=0
【疑问】Host 整体崩溃后的 helper process 不在 Accepted 范围；本卡保持 Out of scope 并记录遗留
```

### 2. 实际改动

- `ExcelT21Handlers` 新增 `running -> closing -> terminal` 原子生命周期门；open 的最终
  lifecycle 校验与 session 注册置于同一 registry mutex 临界区，关闭开始后不得逃逸。
- `SessionRegistry` 新增全部 active session 转入 closing、runner 收敛后统一释放容量；
  open/read/summarize/cleanup 在 closing 或 terminal 状态立即失败。
- `PlatformExcelT21Runtime` 将 runner 放入独立 mutex；shutdown 先发布 closing、释放
  runtime-state mutex，再执行可能阻塞的 runner shutdown。失败时标记 terminal 并保留
  workbook mapping。
- cleanup 错误统一保留 `UserInteraction`，其余结构/runner/reap 失败映射 `Fatal`。
- 新增 lifecycle 专项：token 在阻塞 cleanup 前失效、open 与 shutdown 并发不逃逸、
  多 session 收敛、close 与 shutdown 双失败保留、UserInteraction 保留、重复 shutdown 拒绝。
- 为满足 600/900 行硬门禁，把 task-end 扩展与相关测试集中到
  `excel_t21_lifecycle_tests.rs`，并把两个既有 parser 测试移入该文件；未改测试判据。

### 3. 门禁输出摘要

- `cargo test -p assistant-agent-core excel_t21_lifecycle`：**6 passed**。
- `cargo test -p assistant-agent-core excel_t21_platform_runtime`：**12 passed / 1 ignored**。
- `cargo test -p assistant-agent-core excel_t21`：**88 passed / 1 ignored**。
- `cargo test --workspace`：全部测试与 doctest 通过。
- `cargo fmt --all --check`、workspace Clippy：PASSED；仅仓库既有
  `clippy::assert_is_empty` unknown-lint warning。
- `hygiene`：0 error / 257 warning，PASSED；Excel 私有文件均不超过 900 行硬上限。
- `verify-schemas`、`codegen --check`、arch、`card-check`、`docscan`、`memory-counts`、
  `adr-index`、`check-ledger`、`check-comments`、`cargo deny check`、`git diff --check`：
  PASSED（仓库既有 warning / skipped 保留）。

### 4. DoD 核对

- [x] handler task-end 通过单次 `running -> closing` CAS 拒绝新操作与重复 shutdown。
- [x] cleanup 开始时全部 active session 立即不可 lookup；close 成功才释放对应容量。
- [x] runner 整体 shutdown 成功后才清理剩余 closing session；失败时保留证据。
- [x] platform runtime 在阻塞 cleanup 前进入 closing，失败时保留 workbook mapping。
- [x] `UserInteraction` 保持；结构/runner 失败为 `Fatal`；cleanup 错误不被静默丢弃。
- [x] 专项覆盖成功、close 失败、shutdown 失败、并发 open/read 与重复 shutdown。
- [x] production `tools.json`、ToolBus、rollback、assembly、protocol/schema、公共 API
  与依赖零改动。
- [x] 全量门禁、独立 review no P0-P2、PR/CI/merge/main CI 证据落入记录区。
- [x] 外部 automation JSONL 的 start/gates/work/pr/ci/merge/main_ci 已写入。

### 5. 偏差

- 首轮实现把新增 lifecycle 方法放在主 handler 文件，触发 900 行硬门禁；已改为在
  已授权的 lifecycle 私有测试模块中实现扩展，并把两个既有 parser 测试移到该文件。
- 独立 review 首轮发现 1 P1（open/shutdown 会话逃逸）、1 P2（expired close 错误码降级）
  与 1 P3（runner mutex poison 状态）；已修复并补回归测试。复审 **No findings (P0-P2)**，
  并发测试重复 20/20 passed；仅保留 runner mutex poison 无专门单测的非阻断测试缺口。
- PR #882 首轮 macOS CI 暴露测试 gate 可移植性缺陷：同一 `Condvar` 先后绑定 entered/
  release 两个 mutex，pthread 实现 panic。已拆为两个独立 `Condvar`；该偏差仅在测试
  基础设施，未改变产品状态机或断言。

### 6. 新增长期记忆

无。稳定事实由本卡、agent-core README 与 LEDGER 承载；未改 `MEMORY.md` / `docs/memory/*`。

### 7. 遗留与下一步

- Host 整体崩溃后的独立 helper process / 下次启动恢复仍无 Accepted 契约，不在本卡实现。
- production task assembly、ToolBus/tools.json/rollback/assembly registration 仍保持未开放。
- 真实 task-end 与真实 Excel/GUI 未运行；本卡只提供 fake/contract 证据。

### 8. 外部证据

- 仓库外 automation JSONL：
  `C:\Users\fexfe\.codex\automations-logs\2026-11-26\ai-assistant-task-round-20261126-2200.jsonl`。
- 独立 review 首轮 findings：P1 open/shutdown 逃逸、P2 expired cleanup 错误码、
  P3 runner mutex poison closing；修复后复审 **No findings (P0-P2)**。
- implementation PR #882：checks **22（21 success + 1 skipped）**，pending 0 / failed 0，
  base=main，`MERGEABLE` / `CLEAN`；merge
  `58ceb69cf5660672f85de35d0dbaa32a7f2ea0ff`；post-merge main CI
  `45845203826` attempt 2 success（attempt 1 Windows 命中既有 TASK-412 PING 时序抖动，
  失败 job 重跑通过）。

### 9. 给审阅者的关注点

- 核对 open 的最终 lifecycle 检查与 session 注册是否在同一 registry mutex 下线性化。
- 核对 runtime shutdown 是否在 runner mutex 阻塞前先发布 closing，并在所有失败分支保留
  workbook/session 证据。
- 核对 `UserInteraction` 与结构失败的 ErrorCode 分级，以及 600/900 行拆分没有改变测试判据。
