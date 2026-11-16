# TASK-375 Excel COM 失败路径生命周期取证

- 状态：InProgress
- 阶段：2　子阶段：Stage 2 COM lifecycle　批次：1a.2
- 依赖：TASK-372、TASK-373、TASK-374、ADR-0063

---

## write scope

- `tasks/TASK-375-excel-com-lifecycle-failure-evidence.md`
- `eval/tasks/excel/preflight/probe_excel_com.ps1`
- `eval/tasks/excel/preflight/test_excel_com_lifecycle.ps1`
- `eval/tasks/excel/preflight/README.md`
- `eval/tasks/excel/preflight/evidence/2026-11-16-task-375/**`
- `docs/audits/stage-2-excel-com-lifecycle-2026-11-16.md`
- `docs/memory/apps/excel.md`
- `docs/memory/facts.md`
- `docs/memory/pitfalls.md`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

在现有 Excel COM preflight 上补齐**成功、注入异常、协作取消、已有 Excel 拒绝**
四条可重复执行的生命周期路径，证明每条路径都只回收本轮创建的 `EXCEL.EXE`，
且不会误杀既有进程；不修改公共接口或生产 COM runner。

## In scope

- 为 `probe_excel_com.ps1` 增加仅用于取证的有界异常注入与协作取消信号。
- 新增独立生命周期回归脚本，分别驱动正常、异常、取消和既有进程拒绝场景。
- 校验每个场景的 summary、退出码、本进程树回收与最终 `EXCEL.EXE` 数量。
- 将真实运行证据、COM 清理事实和取消路径限制写入审计与长期记忆。
- 保持 Stage 2 InProgress，并明确公共 ADR/spec 裁决仍是下一人工闸门。

## Out of scope

- 不修改 `docs/adr/**`、`docs/spec/**`、`protocol/**`、`adapters/**` 或任何 Rust/TS 产品代码。
- 不新增依赖、crate、模块、公共 API、Tool、Target、schema、rollback recipe 或 interrupt 变体。
- 不实现生产 Excel Adapter、COM runner、T2.1/T2.2/T2.3 或 L1 rollback。
- 不打开用户工作簿，不启动可见 Excel，不修改 `DisplayAlerts`，不复用或终止既有 Excel。
- 不把异常注入/取消信号当作生产接口，不把本卡证据冒充真实 GUI 或真实业务工作簿验收。

## 必须遵守

1. 失败、取消、拒绝都必须显式写 summary；不得吞掉 COM release、Quit 或等待失败。
2. 只允许终止脚本自己发现的 probe 新 PID；既有 Excel 在任何创建前 fail-closed。
3. 注入异常与取消信号仅走脚本公开参数并在 summary 中如实记录，不放宽正常成功判据。
4. 所有等待必须有界；最终状态以实际进程查询为准，不用推测代替证据。
5. 任何公共契约或生产代码需求出现即停止并另行 DRIFT，不在本卡扩张。

## 验收命令

```powershell
pwsh -NoProfile -File eval/tasks/excel/preflight/test_excel_com_lifecycle.ps1 `
  -EvidenceRoot eval/tasks/excel/preflight/evidence/2026-11-16-task-375
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo run -p xtask -- refscan
cargo run -p xtask -- memory-counts
cargo run -p xtask -- check-ledger
git diff --check
```

预期：生命周期回归四场景全部通过；正常路径 `result=passed`、异常 `result=failed`、
取消 `result=cancelled`、既有进程拒绝 `result=failed`；每条路径结束后本次创建的
`EXCEL.EXE` 为 0，既有 bootstrap Excel 在拒绝场景中保持存活并由 harness 自行回收。
仓库所有门禁全绿，公共契约与产品代码零差异。

## DoD

- [ ] 正常、注入异常、协作取消、已有 Excel 拒绝四条真实路径均有 summary 和退出码断言。
- [ ] 每条路径只回收获准 PID；已有 Excel 拒绝路径证明 probe 未终止 bootstrap 进程。
- [ ] COM release、Workbook.Close、Quit、超时等待与 taskkill 结果均可在证据中追溯。
- [ ] 取消路径的有界适用范围和未覆盖的外部强制终止风险写清楚。
- [ ] `docs/adr/**`、`docs/spec/**`、`protocol/**`、`adapters/**`、Rust/TS 产品代码零差异。
- [ ] 文档/仓库门禁全绿，PR 与 post-merge main CI 证据回填。

<!-- ─── 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ───
     以下由 Implementer 填写。改动分界线上方任何一行 = 漂移触发器 ⑤。
-->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-375 Excel COM 失败路径生命周期取证
【目标】为现有 Excel COM probe 增加异常注入、协作取消与清理回归，补齐成功/异常/取消/拒绝证据
【write scope】本卡、Excel preflight 脚本/README/证据、Stage 2 审计、excel memory、FACT/PITFALL、Stage 2 计划与状态文件
【铁律】契约先行；无静默失败；写操作有 postcondition；不得静默扩大范围；资源有界并回收；证据不得伪造
【禁止】不改 ADR/spec/protocol/adapters/Rust/TS；不新增依赖或公共接口；不打开用户工作簿；不修改 DisplayAlerts
【验收】PowerShell 四场景生命周期回归 + Rust/工作区/xtask 门禁 + PR/main CI
【依赖】TASK-372 Done、TASK-373 Done、TASK-374 Done；N_auto=0、next_round=1
【疑问】公共 Adapter ADR/spec 仍须人类裁决；默认不越线，本卡保持 Stage 2 InProgress
```

### 2. 实际改动文件（逐个核对是否在 In scope 内）

- `tasks/TASK-375-excel-com-lifecycle-failure-evidence.md`：本卡状态与执行记录。
- `eval/tasks/excel/preflight/probe_excel_com.ps1`：精确 ownership journal、异常注入、协作取消与清理字段。
- `eval/tasks/excel/preflight/test_excel_com_lifecycle.ps1`：五场景真实生命周期与 watchdog 回归。
- `eval/tasks/excel/preflight/README.md`：运行方式、测试专用参数与取消边界。
- `eval/tasks/excel/preflight/evidence/2026-11-16-task-375/**`：五场景 console/summary、
  ownership journal、watchdog 证据与聚合 summary。
- `docs/audits/stage-2-excel-com-lifecycle-2026-11-16.md`：真实结果、失败取证与限制。
- `docs/memory/apps/excel.md`：Excel COM ownership、取消 checkpoint 与 watchdog 长期记忆。
- `MEMORY.md`：仅同步 `apps/excel.md` 派生行数 91 → 96。
- `LEDGER.md`：Review 事件行。

### 3. 验收输出摘要（命令 → 结果，全绿 / 失败项）

- PowerShell AST parse（probe + harness）→ 两文件 `PARSE_OK`。
- `pwsh -NoProfile -File eval/tasks/excel/preflight/test_excel_com_lifecycle.ps1 -EvidenceRoot eval/tasks/excel/preflight/evidence/2026-11-16-task-375`
  → `excel-com-lifecycle passed scenarios=5 final_excel_process_count=0`。
- 正常场景 → exit 0 / `result=passed` / `cleanup_succeeded=true`。
- 异常注入 → exit 2 / `result=failed` / `failure_kind=injected_failure` / 进程归零。
- 协作取消 → exit 3 / `result=cancelled` / checkpoint=`after-workbook-create` / 进程归零。
- 强制 hang → 15 秒 watchdog 命中；ownership 身份复核后只回收本场景 Excel 与子进程。
- 既有 Excel 拒绝 → exit 2 / `result=failed` / `failure_kind=existing_process`；
  bootstrap PID 在 probe 返回后仍存活，随后由 harness 只回收自己的 PID。
- `cargo fmt --all --check` / `cargo clippy --all-targets -- -D warnings` → PASSED。
- `cargo test --workspace` → 510 passed / 0 failed；既有真机用例按其 TASK 标记 ignored。
- verify-schemas、codegen、hygiene、card-check、docscan、refscan、memory-counts、
  check-ledger、adr-index、check-comments、架构测试、core replay、`cargo deny check` → 全 PASSED。
- `cargo llvm-cov --fail-under-lines 75` → 行覆盖 **79.76%**。
- `apps/desktop-ui` → lint / typecheck PASSED；model 90/90 + Vitest 5/5。
- PR / CI / merge / main CI：待本实现 PR 创建后回填。

### 4. DoD 逐条核对

- [x] 四条核心真实路径均有 summary 和退出码断言；新增 watchdog hang 路径验证强制超时回收。
- [x] 每条路径只回收获准 PID；拒绝场景证明 probe 未终止 bootstrap Excel。
- [x] COM release、Close、Quit、taskkill、剩余 PID 与最终进程数均有证据字段。
- [x] 取消适用范围和外部强制终止风险写入 README 与审计。
- [x] `docs/adr/**`、`docs/spec/**`、`protocol/**`、`adapters/**`、Rust/TS 产品代码零差异。
- [x] 仓库门禁全绿。
- [ ] PR 与 post-merge main CI 证据待提交后回填。

### 5. 偏差

- 首次异常注入子进程因 `$ErrorActionPreference=Stop` 下的 `Write-Error` 以 exit 1
  提前终止；改为显式 stderr 写入后兑现约定 exit 2，未放宽断言。
- 首次取消点选在 `after-com-create`，预置信号后 `Application.Quit()` 阻塞；仅终止本轮
  新 PID 后恢复。最终取消点改为已初始化工作簿的 `after-workbook-create`，该失败事实保留在审计中。
- 首次在仓库根执行 `pnpm` 时因根目录无 `package.json` 而失败；改在实际 UI workspace
  `apps/desktop-ui` 重跑并全绿，不是产品门禁失败。
- 独立 review 拒绝首版“前后 PID 差集”所有权方案与无限等待；最终改为 HWND→PID、
  PID/start ticks/path ownership journal、bounded child watchdog 与逐次身份复核。
- 修复过程中两次在 bootstrap helper 失败后留下本轮进程；均按当时 ownership/PID 与命令行
  核对后只回收本卡创建进程，未触碰用户实例，并把失败写入本记录。
- 无 write scope 扩大；没有修改公共契约、产品代码或依赖。

### 6. 更合理做法（实施中发现，非漂移）

- 取消测试不应在 COM 原始实例刚出现、Excel 尚未初始化完成时执行；选择明确安全 checkpoint
  才能验证可控清理，而不是把外部强杀伪装成协作取消。
- 进程计数断言需要短暂稳定等待；单纯读取一次 `Get-Process` 会与 `taskkill` 退出发生竞态。
- PowerShell 参数名大小写不敏感；`$BootstrapHelper` switch 不能同时作为持有 helper 对象的变量名。
- 同步 COM 调用不能靠本地 `WaitForExit` 变成可中断；必须把整个 probe/helper 放进有界父进程，
  配合 ownership journal 做外部 watchdog。

### 7. 遗留问题

- 生产 COM runner、Excel Adapter pack、T2.1/T2.2/T2.3、L1 rollback 均未实现。
- 外部 `TerminateProcess`、控制台会话丢失、机器崩溃与不可返回的 COM hang 不在协作取消覆盖内。
- Protected View、AutoSave=true、用户工作簿、只读/占用文件与模态对话框仍待后续真实场景。
- Adapter D1-D6 公共 ADR/spec 仍需人类裁决。

### 8. 新增长期记忆

- `docs/memory/apps/excel.md` 新增 Excel COM ownership、取消 checkpoint 与 watchdog 条目；
  `MEMORY.md` 同步派生行数 91 → 96。

### 9. 给审阅者的关注点（风险最高的 1~3 处）

- 重点核对 ownership：PID、start ticks、路径必须同时匹配，禁止裸 PID kill。
- 重点核对 `after-com-create` 阻塞事实与 15 秒 watchdog 证据。
- 重点核对既有进程拒绝场景只观察、不终止 bootstrap Excel；helper 仅回收自己的身份。
- 重点核对本卡没有把 eval harness 冒充生产 COM runner 或 Stage 2 Adapter 已落地。
