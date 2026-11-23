# TASK-394 Excel T2.1 read-only tool contract

- 状态：**Done**
- 阶段：2　子阶段：Stage 2 Excel T2.1　批次：3b　依赖：TASK-393、ADR-0104、ADR-0105、ADR-0107

---

## write scope

- `tasks/TASK-394-excel-t21-readonly-tool-contract.md`
- `docs/adr/0108-excel-t21-readonly-tool-contract.md`
- `docs/adr/README.md`
- `docs/spec/excel-t21-readonly-workflow.md`
- `docs/spec/capability-matrix.md`
- `docs/memory/decisions.md`
- `MEMORY.md`（仅 `decisions.md` 派生规模行）
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

在实现生产 Excel COM runner 前，冻结 T2.1 只读工作流的模型可见 Tool 名、输入输出、
session/ownership、生命周期清理、错误映射和测试矩阵；本卡只定义契约，不暴露未实现的 Tool。

## In scope

- 新增 ADR-0108 与 `docs/spec/excel-t21-readonly-workflow.md`。
- 冻结四个 Tool 名：`excel.workbook.open`、`excel.workbook.read`、
  `excel.range.read`、`excel.range.summarize`。
- 定义 opaque task-scoped `session_id`，禁止 COM handle 跨进程/跨任务。
- 定义 `.xlsx` local absolute path、read-only、单 session、无用户 Excel 进程、bounded range、
  显式 `value2|formula|text` 与 `.Formula`/`.Text` 语义。
- 定义 open 的 write/L3/approval/no_recovery 边界，以及三个 read Tool 的
  `none_readonly`/postcondition 映射。
- 定义任务结束时 Close(false) / Quit / COM release / owned PID reap 的清理责任。
- 同步 ADR 登记、decisions、capability-matrix 引用、Stage 2 与 LEDGER。

## Out of scope

- 不新增/修改 `adapters/com.microsoft.excel/tools/tools.json` 或 rollback recipes。
- 不实现 COM runner、handler、ToolBus 注册、ToolSchema JSON/生成类型、Policy 或 runtime。
- 不启动真实 Excel，不打开用户工作簿，不读取 `.xlsx`，不运行真实 GUI 验收。
- 不支持 `.xlsm/.xlsb`、宏、外部链接、Protected View 自动启用、公式写入、范围写入、Save As。
- 不新增依赖、crate、模块、公共 API、ErrorCode、IPC/DB schema 或 CI。

## 必须遵守

1. `excel.workbook.open` 必须是唯一声明 `com.workbook_open` 的 write/high/L3/approval/
   non-idempotent Tool；不得夹带任何读取 capability。
2. `session_id` 是 task-scoped opaque token，不是 COM handle；未知、过期、跨 task 一律失败。
3. T2.1 只接受本地绝对路径、`.xlsx`、只读打开；任何既有 `EXCEL.EXE` 在创建前失败。
4. 读取范围最多 4096 cells / 256 KiB；超限失败，不截断、不返回部分成功。
5. `value2` / `formula` / `text` 必须由调用方显式选一；汇总只允许 `value2`。
6. task 结束必须 Close(false) / Quit / COM release / 有界回收 owned PID，绝不终止用户进程。
7. 本卡不能把契约已定义写成 runner/Tool/真机已实现。

## 验收命令

```powershell
cargo run -p xtask -- adr-index
cargo fmt --all --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo run -p xtask -- refscan
cargo run -p xtask -- memory-counts
cargo run -p xtask -- check-ledger
git diff --check
```

预期：ADR/spec/decisions/capability cross-link 一致；所有文档门禁通过；
`adapters/**`、`protocol/**`、`crates/**`、`apps/**`、CI 与依赖零差异。

## DoD

- [ ] ADR-0108 Accepted，四个 Tool 名、session、ownership、输入输出与错误映射完整。
- [ ] open Tool 的 capability/effect/reversibility/risk/approval/postcondition/rollback 唯一映射完整。
- [ ] 三个 read Tool 的 `none_readonly`、state_unchanged 与 bounded payload 契约完整。
- [ ] `.xlsx` 只读打开、无既有进程、无 session 泄漏、无未保存工作簿写入的真机门禁写清。
- [ ] capability-matrix、decisions、ADR 索引与 Stage 2 同步。
- [ ] 文档门禁、PR、merge 与 post-merge main CI 回填。
- [ ] protocol/adapters/Rust/TS 产品代码与依赖零差异。

<!-- ───── 分界线：以上是卡片正文，Orchestrator 所有，Implementer 只读 ─────
     以下由 Implementer 填写。改动分界线以上任何正文行 = 漂移触发器 ⑤；
     状态行仅按 ADR-0083 与产生该状态的提交同批更新。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-394 Excel T2.1 read-only tool contract
【目标】实现前冻结只读 open/read/summarize Tool、session 与生命周期契约
【write scope】仅：本卡、ADR-0108/登记、excel T2.1 spec、capability spec、decisions/MEMORY、Stage 2/PLAN/README/LEDGER
【铁律】契约先行；不可信输入先校验；无静默失败；资源有界；不改公共 schema/API
【禁止】不写 tools.json/rollback；不实现 runner；不启动 Excel；不改 protocol/adapters/产品代码/依赖
【验收】ADR/spec/cross-link 完整；文档门禁全绿；适配器与产品路径零差异
【依赖】TASK-393、ADR-0104/0105/0107 已核对 Done/Accepted
【疑问】无；按“继续下一步”先落 T2.1 契约，真实 COM 在后续 implements card
```

### 2. 实际改动文件

- `tasks/TASK-394-excel-t21-readonly-tool-contract.md`：任务卡与执行记录，in scope。
- `docs/adr/0108-excel-t21-readonly-tool-contract.md`：四 Tool/session/ownership/cleanup ADR，in scope。
- `docs/adr/README.md`：ADR-0108 登记与下一个可用编号 0109，in scope。
- `docs/spec/excel-t21-readonly-workflow.md`：输入输出、bounds、错误与实现门禁，in scope。
- `docs/spec/capability-matrix.md`：v0.5 T2.1 concrete consumer cross-link，in scope。
- `docs/memory/decisions.md`：ADR-0108 decision，in scope。
- `MEMORY.md`：仅更新 `decisions.md` 派生规模 334 / 116，in scope。
- `plans/stage-2-excel.md`：TASK-394 批次行与 Review 进度，in scope。
- `LEDGER.md`：追加 TASK-394 Review 事件，in scope。

### 3. 验收输出摘要

- `cargo run -p xtask -- adr-index` → PASSED，97 files / 0 error / 0 warning。
- `cargo fmt --all --check` → PASSED。
- `cargo run -p xtask -- hygiene` → PASSED，502 files / 0 error / 194 既有 warning。
- `cargo run -p xtask -- card-check` → PASSED，297 files / 0 error / 207 warning。
- `cargo run -p xtask -- docscan` → PASSED，711 files / 0 error / 175 既有 warning。
- `cargo run -p xtask -- refscan` → PASSED，1254 files / 0 error / 0 warning。
- `cargo run -p xtask -- memory-counts` → PASSED，0 error / 1 既有 paint archive warning。
- `cargo run -p xtask -- check-ledger` → PASSED；PLAN / LEDGER 均为 2026-11-23。
- `git diff --check` → PASSED。
- 独立 review `Hume`（`01a20742-4c5e-7cb2-b61c-492795576fac`）：无 P0；3 个 P1、3 个 P2 与
  1 个 P3 均已在 ADR/spec/执行记录中处理。
- PR #806 checks **11/11 success**，pending 0 / failed 0，base=main，`MERGEABLE` / `CLEAN`；
  merge `614062a8ea8717fab100853516ab11f67a9b15e0`；post-merge main CI `45133625235` success。

### 4. DoD 逐条核对

- [x] ADR-0108 Accepted，四个 Tool 名、session、ownership、输入输出与错误映射完整。
- [x] open Tool 的 capability/effect/reversibility/risk/approval/postcondition/rollback 唯一映射完整。
- [x] 三个 read Tool 的 `none_readonly`、state_unchanged 与 bounded payload 契约完整。
- [x] `.xlsx` 只读打开、无既有进程、无 session 泄漏、无未保存工作簿写入的真机门禁写清。
- [x] capability-matrix、decisions、ADR 索引与 Stage 2 同步。
- [x] 文档门禁、PR、merge 与 post-merge main CI 回填。
- [x] protocol/adapters/Rust/TS 产品代码与依赖零差异。

### 5. 偏差

- 无功能/契约 DRIFT。
- 首次 capability-matrix 表格在 inline code 内使用未转义 `|`，`docscan` 报 table-broken；
  已改为 `value2` / `formula` / `text` 并复跑通过。
- 独立 review 的 7 项 findings 已全部修复：cleanup 改为先释放 Range/Worksheet 子对象，
  再 `Workbook.Close(false)`、`Application.Quit()`，最后释放 Workbook/Application；
  在可能阻塞的 Open/Close/Quit 前写仓库外 ownership journal，并由不共享 STA 阻塞路径的
  独立 watchdog 仅回收经 PID/start ticks/path 复核的 owned 进程树。
- session 改为绑定 `(tool_bus_session_id, task_id, owned_process_identity, workbook_identity)`；
  ToolBus session id 与传给 `ToolBusConfig::new` 的值同源、由 assembly 注入 registry，
  不新增 `CallContext` 公共字段。
- `workbook.read` 补齐 262144 bytes / 1024 worksheets / 2048 named ranges 与 session_id
  32..=128 bytes 上限；错误矩阵补齐文件不存在、权限、unsupported extension、损坏工作簿、
  summarize 无数值、未知 Excel error、session 格式、workbook identity 漂移与 cleanup 分类。
- token 语义冻结为 TTL 内可多次 read/summarize，不是单次消费；dedicated Excel STA thread
  按到达顺序线性化，task end / cleanup start / TTL expiry 后立即失效。
- Stage 2 历史 TASK-392 排期行仍写“COM tools 等待 capability ADR”；按 ADR-0041
  不改历史条目正文，该状态已由 ADR-0107/0108、TASK-393/394 与 Stage 2 当前进度 supersede。
- `PROCESS-TASK394-SHELL-MISUSE`：最终机器计数 **43** 次（主实现 26 + 独立 review 17），
  均为 rollout 中显式 `shell="bash"` 的调用；未造成意外产品文件写入或外部副作用。
  其中合并前 Review 记录冻结为 40，合并后只读状态/结构/门禁查询使最终值升为 43。

### 6. 更合理做法

- 先把 Tool/session/lifecycle 契约与实现分开，避免为了推进 T2.1 提前把空 runner 暴露到
  `tools.json` 或 ToolBus。
- session token 采用 task-scoped opaque value + 内存 registry，不让 COM handle/PID/HWND 进入模型。
- session registry 用 ToolBus session + task + owned process/workbook identity 四元组隔离；
  token 是可复用 capability，不是一次性 nonce。
- read 使用 tagged `value_text`，避免 JSON 浮点精度和 Excel blank/error 被默认值吞掉。

### 7. 遗留问题

- TASK-395 或等价实现卡：Excel COM runner、session registry、Tool handlers、rollback row、
  独立 watchdog、假平台测试与人类在场 `.xlsx` 验收；在这些完成前四个 Tool 保持未实现、未注册。

### 8. 新增长期记忆

- 新增 ADR-0108 decision：T2.1 只读 Excel 使用四个 Tool 与 task-scoped opaque session；
  契约先于 tools.json/runner。

### 9. 给审阅者的关注点

- 重点检查 open Tool 是否仍保持 sole capability 与 L3 approval。
- read Tool 已由独立 review 检查：不能隐藏 open/写入，token 不能跨 task/跨 ToolBus 复用。
- 4096 cells / 256 KiB、workbook read bounds、完整错误矩阵与 task-end watchdog cleanup
  已补齐；PR checks、merge state 与 post-merge main CI 已全部回填。
