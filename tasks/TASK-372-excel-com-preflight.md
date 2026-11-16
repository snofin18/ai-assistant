# TASK-372 Excel COM preflight

- 状态：**Review**
- 阶段：2　子阶段：Stage 2a Excel L1 preflight　批次：Stage 2 kickoff
- 依赖：Stage 1 closeout、TASK-371、ADR-0063、feasibility P2
- 本文件 = 卡片正文 + 执行记录（ADR-0031）。

---

## write scope

- `tasks/TASK-372-excel-com-preflight.md`
- `plans/stage-2-excel.md`
- `eval/tasks/excel/preflight/probe_excel_com.ps1`
- `eval/tasks/excel/preflight/README.md`
- `eval/tasks/excel/preflight/evidence/2026-11-16-task-372/**`
- `docs/audits/stage-2-excel-com-preflight-2026-11-16.md`
- `docs/memory/apps/excel.md`
- `docs/memory/facts.md`
- `docs/memory/pitfalls.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`
- `MEMORY.md`

## 目标

建立 Stage 2（Excel L1 COM + Adapter 规范化）可执行计划，并用本机真实 Excel 做一次隔离、
可回收的 COM 预检：确认版本/通道、公式与三种读法、程序化修改后的 undo 可观测状态、
Protected View/AutoSave 可读取性，以及退出后 `EXCEL.EXE` 进程归零。

## In scope

- 新建 `plans/stage-2-excel.md`，冻结 Stage 2 目标、任务顺序、DoD 与 Out of scope。
- 使用 PowerShell COM 创建**新的未保存工作簿**，只写临时单元格，不打开用户文件、不保存。
- 记录 Excel version/build/path、Protected View 数量、AutoSave 状态与 `DisplayAlerts` 当前值。
- 验证 `.Value2`、`.Formula`、`.Text` 三种读取口径，并验证公式 `A3=A1+A2` 计算为 `5`。
- 记录连续 COM 写入前后的 `CanUndo` 与一次 `Application.Undo()` 观测结果。
- 无论成功失败都执行 `Close(false)`、`Quit()`、COM release 和有界进程回收。
- 把 Adapter schema 缺口与后续 COM 平台化前置条件写入审计和 Stage 2 批次表。

## Out of scope

- 不新增 COM crate、平台模块、依赖或 `unsafe`。
- 不修改协议、Tool/UI/DB schema、Policy、Adapter 现有文件或产品运行时。
- 不打开、不修改、不保存任何用户工作簿；不创建 Protected View/OneDrive/AutoSave 场景。
- 不做 T2.1/T2.2/T2.3 任务、图表导出、宏、VBA、外部链接或真实业务数据。
- 不设置 `DisplayAlerts=false`；只读取当前值。

## 必须遵守

1. 检测到任何现有 `EXCEL.EXE` 时在创建 COM 前失败，禁止复用或终止用户实例。
2. COM 失败路径也必须关闭工作簿、`Quit`、release，并只回收本轮新建 PID。
3. 公式/字段读取失败必须显式失败；不得用空字符串或默认值冒充结果。
4. undo 观测只记录真实返回值；即使 `CanUndo=false` 也不得手工推断为成功。
5. 临时工作簿只能 `Close(false)` 丢弃，禁止保存到磁盘。

## 验收命令

```powershell
pwsh -NoProfile -File eval/tasks/excel/preflight/probe_excel_com.ps1 `
  -EvidenceRoot eval/tasks/excel/preflight/evidence/2026-11-16-task-372
python -m json.tool eval/tasks/excel/preflight/evidence/2026-11-16-task-372/summary.json
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo run -p xtask -- refscan
cargo run -p xtask -- memory-counts
cargo run -p xtask -- check-ledger
git diff --check
```

预期：真实 Excel probe `result=passed`；公式结果 `5`；工作簿未保存；
完成后 `excel_processes_after=0`；全部文档门禁通过。

## DoD

- [ ] 本机 Excel COM 可创建隔离实例并读取 version/build/path。
- [ ] `Value2` / `Formula` / `Text` 与公式计算取证完成，公式结果等于 5。
- [ ] 连续程序化写入前后的 undo 状态和一次 Undo 行为均如实记录。
- [ ] Protected View、AutoSave、`DisplayAlerts` 当前状态有结构化证据。
- [ ] 成功/失败清理逻辑覆盖 Close、Quit、release、bounded wait 与新建 PID 回收。
- [ ] Stage 2 计划、Excel app memory、审计、PLAN/README/LEDGER 同步。
- [ ] 所有文档与仓库门禁通过。

<!-- ══ 分界线：以上为卡片正文，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线上方任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-372 Excel L1 COM preflight
【目标】建立 Stage 2 计划，并用真实 Excel COM 证明版本、公式读回、undo 观测与进程生命周期
【write scope】仅：本卡、stage-2-excel 计划、Excel preflight 脚本/证据、审计、excel app memory、PLAN/README/LEDGER/MEMORY 状态
【铁律】无静默失败；Adapter/COM 结果按不可信输入；写入必须 L1 快照；子进程/COM 必须回收；不改公共 schema/API
【禁止】不碰用户现有工作簿；不改 DisplayAlerts；不保存临时工作簿；不引入依赖或 COM 产品代码
【验收】真实 Excel probe passed；公式结果=5；完成后 EXCEL.EXE=0；全部文档/仓库门禁通过
【依赖】Stage 1 closeout、TASK-371、ADR-0063、feasibility P2 已核对
【疑问】无；Excel 2024 x64 已安装且当前无运行实例
```

### 2. 实际改动文件（逐个核对是否在 In scope 内）

- `tasks/TASK-372-excel-com-preflight.md`：本卡状态与执行记录。
- `plans/stage-2-excel.md`：Stage 2 目标、任务顺序、契约、DoD 与 TASK-372–379 批次表。
- `eval/tasks/excel/preflight/probe_excel_com.ps1`：隔离 Excel COM、公式/undo 观测、
  Close/Quit/release 与本轮 PID 收敛。
- `eval/tasks/excel/preflight/README.md`：运行方式、安全边界与“非生产 COM runner”声明。
- `eval/tasks/excel/preflight/evidence/2026-11-16-task-372/summary.json`：真实探针证据。
- `docs/audits/stage-2-excel-com-preflight-2026-11-16.md`：环境、结果、undo 结论与
  Adapter 契约缺口。
- `docs/memory/apps/excel.md`：Excel 身份、COM、UIA、数据口径、坑、通道与未测项。
- `docs/memory/facts.md`：追加真实 Excel COM preflight FACT。
- `docs/memory/pitfalls.md`：追加既有 Excel 进程复用与 `CanUndo=false` 坑。
- `MEMORY.md`：登记 Excel app memory，并同步 facts/pitfalls 规模表。

### 3. 验收输出摘要（命令 → 结果，全绿 / 失败项）

- `pwsh -NoProfile -File eval/tasks/excel/preflight/probe_excel_com.ps1 ...`：
  `excel-com-preflight passed version=16.0 build=20527 formula_value2=5 excel_processes_after=0`。
- Excel 安装：Office Home 2024 Retail x64，ClickToRun `16.0.20527.20196`。
- 公式：A1=2、A2=3、A3=`=A1+A2`；`.Value2=5.0`、`.Formula==A1+A2`、`.Text="5"`。
- Undo：写入前/第一次写入后/第二次写入后 `CanUndo` 均为 `false`，
  `Application.Undo()` 未调用。
- 状态：`DisplayAlerts=1` 且未修改；`Visible=false`；Protected View 0；新工作簿
  `AutoSaveOn=false`。
- 清理：`Close(false)`、`Quit()`、FinalReleaseComObject 全部成功；
  cleanup failure 0、taskkill 0；本轮 PID 剩余 0；最终 `EXCEL.EXE=0`。
- `python -m json.tool .../summary.json`：通过。
- `cargo run -p xtask -- memory-counts`：0 error / 1 warning，PASSED。

### 4. DoD 逐条核对

- [x] 本机 Excel COM 可创建隔离实例并读取 version/build/path。
- [x] `Value2` / `Formula` / `Text` 与公式计算取证完成，公式结果等于 5。
- [x] 连续程序化写入前后的 undo 状态和一次 Undo 行为均如实记录。
- [x] Protected View、AutoSave、`DisplayAlerts` 当前状态有结构化证据。
- [x] 成功清理逻辑覆盖 Close、Quit、release、bounded wait 与新建 PID 回收。
- [ ] Stage 2 计划、Excel app memory、审计、PLAN/README/LEDGER 尚待 Done closeout。
- [x] 真实探针与 `memory-counts` 已通过；其余门禁待提交前执行。

### 5. 偏差

- 新建 `docs/memory/pitfalls.md` 条目时首次误把 TASK-364 最后一条替换；`memory-counts`
  发现行数未增长后立即按 append-only 恢复原条目，再追加 Excel 条目。
- 探针未覆盖失败路径的现场触发；脚本中的失败 `finally` 与只清理本轮 PID 的逻辑已完成，
  但本次只有真实成功路径证据。

### 6. 更合理做法（实施中发现，非漂移）

- 将“既有 Excel 进程存在”作为创建 COM 前的硬失败条件，避免 COM 复用用户实例后错误回收。
- 只记录并回收探针新增 PID，而不是按进程名终止所有 Excel。
- undo 事实直接记录 `CanUndo`，不根据文档结论伪造一次 Undo 成功。

### 7. 遗留问题

- Protected View、AutoSave=true、只读/占用、模态对话框、真实工作簿读写尚未测。
- Adapter 六件套、产品 COM lifecycle runner 与 Excel Adapter pack 由后续 dispatch 卡承接。

### 8. 新增长期记忆

- `docs/memory/apps/excel.md`：本机 Office/Excel 身份、数据口径、COM、坑与未测项。
- `docs/memory/facts.md`：Excel COM 可用、公式读回和 `CanUndo=false` 实测。
- `docs/memory/pitfalls.md`：Excel COM 进程复用风险与 L1 回滚要求。

### 9. 给审阅者的关注点（风险最高的 1~3 处）

- 重点检查探针在已有 Excel 时必须创建前失败，不能附着用户实例。
- 重点检查 cleanup 只回收本轮新增 PID，并保留 Close/Quit/release 失败信息。
- undo 只作为观测事实；Stage 2 后续任何 Excel 写入都必须按 L1 快照实现。
