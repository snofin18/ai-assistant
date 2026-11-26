# TASK-410 Excel T2.2 production cleanup boundary audit

- 状态：**Done**
- 阶段：2　子阶段：Stage 2 Excel T2.2　批次：6e　依赖：TASK-401、TASK-402、TASK-403、TASK-409、ADR-0108、ADR-0110、ADR-0111、ADR-0112

---

## write scope

- `tasks/TASK-410-excel-t22-production-cleanup-boundary-audit.md`
- `docs/audits/stage-2-excel-t22-production-cleanup-boundary-2026-11-26.md`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

只读审计 TASK-409 的 test-only ownership journal、Job Object 与外部 watchdog 证据，
对照 ADR-0108 D8 的 production task-end cleanup 契约，明确哪些语义可以直接进入
`assistant-platform-windows`、哪些需要新的 ADR 裁决，并给出下一实现卡的顺序与门禁。

## In scope

- 审计 `ExcelComRunner::shutdown`、`Drop`、`RunnerState::shutdown` 与 Windows backend
  的 Close/Quit/identity 路径。
- 审计 `PlatformExcelT21Runtime` 是否已有 production caller、task-end cleanup 与可观测
  的进程收敛结果。
- 对照 TASK-409 的 Rust journal、PowerShell Job Object、watchdog 身份复核与资源收敛证据。
- 输出 ADR-0108 D8 逐项覆盖矩阵、P0-P3 findings、GO/NO-GO 和下一实现卡建议。
- 同步 Stage 2、PLAN、README、LEDGER。
- 完成独立 review、PR、CI、merge 与 post-merge main CI 收口。

## Out of scope

- 不实现 production ownership journal、watchdog、process-tree kill 或 task-end cleanup。
- 不修改 `ExcelComRunner` 公共方法面、`agent-core` runtime、ToolBus、schema、ErrorCode、
  `tools.json`、rollback recipe 或 production assembly。
- 不新增依赖、子进程 helper、Job Object 生产接线、后台服务或持久化路径。
- 不启动真实 Excel，不重跑 TASK-409 真机场景，不把 test-only 证据冒充 production。
- 不决定 production Tool registration；该动作仍被 ADR-0110 gate 阻塞。

## 必须遵守

1. 只读审计产品代码；任何实现建议必须明确标为后续卡。
2. 必须区分 ADR 已冻结的契约、当前 production 行为和 test-only harness 证据。
3. 必须指出“独立线程”与“独立进程”在 Host 崩溃场景下的覆盖差异；如契约不足，
   记录为 ADR clarification 需求，不得自行扩大解释。
4. 不得把 PowerShell harness、Job Object 或测试 checkpoint 直接声明为 production 组件。
5. 任何 process termination 建议必须先复核 PID、creation FILETIME 与 executable path。
6. 不得为了让审计通过而降低 ADR-0108 的 cleanup、错误映射或资源收敛要求。

## 验收命令

```powershell
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-comments
git diff --check
```

预期：全部 PASSED；审计文档包含逐项矩阵、findings、决策与后续卡顺序；
production 代码、公共 API、schema、依赖和 `tools.json` 零改动。

## DoD

- [ ] ADR-0108 D8 的 10 个 cleanup 步骤逐项对照 current production 与 TASK-409 harness。
- [ ] 至少给出 durable journal、watchdog、unbounded join、cleanup 错误映射、
  timeout 后进程收敛、test-only adapter 六类 production gap，并按 P0-P3 定级。
- [ ] 明确 `excel_process` 现有 identity 能力、creation race、OpenProcess 错误折叠与
  缺失的 verified termination 边界。
- [ ] 明确 test-only runtime 未先进入 closing、cleanup 失败仍删除 active session 的
  状态机缺口。
- [ ] 明确“独立线程”不足以覆盖 Host 进程整体崩溃，记录是否需要 ADR clarification。
- [ ] 明确 journal publish 的唯一路径/phase 命名约束，不把 rename 当 replace。
- [ ] 给出 NO-GO：当前不得注册 production Excel Tool/ToolBus/rollback/assembly。
- [ ] 给出可执行的后续卡顺序，且不新增依赖或公共接口。
- [ ] 文档与治理门禁全绿，独立 review 完成，PR/CI/merge/main CI 证据落记录区。

<!-- 分隔线以下由 Implementer 填写；正文区只读，状态行按 ADR-0083 更新。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-410 Excel T2.2 production cleanup boundary audit
【目标】对照 ADR-0108 D8 审计 production runner 与 TASK-409 harness，决定下一实现边界
【write scope】仅：本卡、审计文档、Stage 2、PLAN、README、LEDGER
【铁律】只读产品代码；区分契约/现状/测试证据；完整身份才可清理；不得静默成功；热门文档写前 guard
【禁止】不实现 journal/watchdog/kill/helper/依赖；不改公共 API/schema/ErrorCode/ToolBus/tools.json
【验收】hygiene、card-check、docscan、memory-counts、adr-index、check-ledger、check-comments、diff-check
【依赖】TASK-401/402/403/409 Done；ADR-0108/0110/0111/0112 已核对；N_auto=0
【疑问】ADR D8 写“独立线程”，未说明 Host 整体崩溃；默认记录为后续 ADR clarification，不在本卡实现
```

### 2. 实际改动

- 新增 `docs/audits/stage-2-excel-t22-production-cleanup-boundary-2026-11-26.md`。
- 同步 Stage 2 批次表与当前进度、PLAN、README、LEDGER。
- 未修改产品 Rust 代码、公共 API、schema、依赖、production Tool 或真实 Excel 状态。

### 3. 门禁输出摘要

- `hygiene` → PASSED（0 error；252 个仓库既有 warning）。
- `card-check`、`docscan`、`memory-counts`、`adr-index`、`check-comments` → PASSED。
- `git diff --check` → PASSED。
- 独立 review 首轮：2 P1 + 3 P2 + 1 P3；已修正 D9 cleanup 错误映射、无界 join、
  test-only closing 顺序、ownership creation race、LEDGER 缺失与 journal 唯一路径/phase。
- 第二轮 review 又修正公共 ownership surface、内外层错误映射与 journal/watchdog
  scope 歧义；最终复审 **No findings (P0-P2)**。
- implementation PR #876：https://github.com/snofin18/ai-assistant/pull/876
  checks **22（21 SUCCESS + 1 SKIPPED）**，pending 0 / failed 0，base=main，
  `MERGEABLE` / `CLEAN`。
- merge `e6a66965249feaca3d1272a84e706f02e8fa9a9c`；post-merge main CI
  `45783616793` success。

### 4. DoD 核对

- [x] 已逐项对照 ADR-0108 D8/D9、current production 与 TASK-409 harness。
- [x] 已定级 durable journal、watchdog、unbounded join、cleanup 错误映射、timeout
  收敛与 test-only adapter 缺口。
- [x] 已明确现有 identity capture、creation race、OpenProcess 错误折叠与缺失的
  verified termination。
- [x] 已明确 test-only runtime 的 closing 顺序与 cleanup 失败时 active-session 删除缺口。
- [x] 已记录独立线程无法覆盖 Host 整体崩溃的 ADR clarification 需求。
- [x] 已明确 journal 原子 publish 必须使用唯一路径/phase 文件。
- [x] 已给出当前 production Excel Tool/ToolBus/rollback/assembly 的 NO-GO。
- [x] 已给出后续实现卡顺序。
- [x] 文档治理门禁、独立 review、PR/CI/merge/main CI 已完成。

### 5. 偏差

- 首轮审计低估了 cleanup 错误映射、无界 join 与 ownership creation race；已按独立
  review 修正，未修改产品行为。

### 6. 新增长期记忆

- 无新增；现有 `docs/memory/apps/excel.md` 与 TASK-409 记录已明确 production
  ownership journal/watchdog 尚未实现。

### 7. 遗留与下一步

- TASK-411 应先实现 platform-private durable ownership journal、唯一路径/phase 命名、
  cleanup 错误映射与 bounded cleanup journal schema。
- TASK-412 再实现不共享 STA 阻塞路径的 watchdog contract、完整身份复核与 verified
  process-tree termination，并解决 creation race、OpenProcess 错误折叠和 unbounded
  join/reap。
- 后续卡完成前继续禁止 production Excel Tool/ToolBus/rollback/assembly registration。

### 8. 外部证据

- TASK-409 审计：
  `docs/audits/stage-2-excel-t22-repeat-2026-11-26.md`。
- TASK-409 证据根：
  `eval/tasks/excel/t22/evidence/2026-11-26-task-409/20261126T063537240Z`。
- ADR-0108：
  `docs/adr/0108-excel-t21-readonly-tool-contract.md`。
- Spec：
  `docs/spec/excel-t21-readonly-workflow.md`。
- implementation PR #876：https://github.com/snofin18/ai-assistant/pull/876
  checks **22（21 SUCCESS + 1 SKIPPED）**，pending 0 / failed 0，base=main，
  `MERGEABLE` / `CLEAN`。
- merge `e6a66965249feaca3d1272a84e706f02e8fa9a9c`；post-merge main CI
  `45783616793` success。

### 9. 给审阅者的关注点

- 重点核对审计是否把 TASK-409 外部 watchdog 误当成 production 实现。
- 重点核对“独立线程”与 Host 崩溃场景的契约边界是否记录清楚。
- 重点核对 D9 cleanup 错误映射、无界 join、closing 状态顺序与 ownership creation race
  是否可执行。
- 重点核对后续卡顺序是否仍被 ADR-0110 的 production registration gate 阻塞。
