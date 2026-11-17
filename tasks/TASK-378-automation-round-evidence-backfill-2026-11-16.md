# TASK-378 2026-11-16 自动化轮次证据回填

- 状态：Review
- 阶段：2　子阶段：2b Adapter contract decision　批次：治理池　依赖：TASK-373、TASK-374、TASK-375、TASK-377、TASK-376 Blocked
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031「一卡一文件」）。

---

## write scope

- `tasks/TASK-378-automation-round-evidence-backfill-2026-11-16.md`
- `docs/audits/automation-round-evidence-2026-11-16.md`
- `docs/automations/2026-11-16-round-1.md`
- `docs/automations/2026-11-16-round-2.md`
- `docs/automations/2026-11-16-round-3.md`
- `docs/automations/2026-11-16-round-4.md`
- `docs/automations/2026-11-16-round-5.md`
- `docs/automations/2026-11-16-report.md`（仅追加）
- `plans/stage-2-excel.md`（仅新增任务行、完成标记与当前进度）
- `PLAN.md`（仅当前状态块）
- `README.md`（仅状态行、当前阶段、最近进展）
- `LEDGER.md`（仅追加）

## 目标

依据 2026-11-16 的五个永久 JSONL、GitHub PR/CI/merge 元数据和当前机器输出，补齐
`docs/automations/2026-11-16-round-{1..5}.md`，解释 `automation-rounds` 为何在五个真实
automation 运行后仍返回零轮次，并给出后续防止同类遗漏的可执行方案。

## In scope

- 新建五份轮次文件，逐槽记录机制、automation id、任务、PR、CI、merge、偏差、
  shell 误用、锁释放、自删与遗留。
- 新建运行日证据审计，区分五个外部 JSONL 事实、仓库轮次文件缺失事实和机器派生值。
- 在既有运行日报告末尾追加回填说明，不改写历史段落。
- 验证回填后 `automation-rounds` 从 `0 / 1 / 0` 变为 `5 / 6 / *`；
  用真实 automation rollout 来源验证 `n_auto=5`，人类会话则验证 ADR-0099 的自动归零。
- 仅按 ADR-0039 / ADR-0041 同步 Stage 2 计划、`PLAN.md`、`README.md` 与 `LEDGER.md`。
- 给出复发防护建议：在 automation 最终审计前强制检查本轮 `round-N.md` 已在 `main`。

## Out of scope

- 不修改 `xtask automation-rounds` 算法、ADR-0054/0089/0099 或任何门禁判据。
- 不修改 `protocol/**`、`adapters/**`、`crates/**`、`apps/**`、schema、测试断言或依赖。
- 不创建 ADR/spec，不代答 TASK-376 的 D1-D6，不解除 Stage 2 公共契约阻塞。
- 不改写五份永久 JSONL、既有 PR/CI/merge 结果、历史任务卡正文或既有 LEDGER 行。
- 不启动真实 Excel、Edge 或其他 GUI，不访问外部业务数据。

## 必须遵守

1. `automation-rounds` 仍以 `main` 上的 `round-*.md` 为唯一事实源；不得为让输出变化而
   修改算法、放宽判据或手工伪造 reset。
2. 每份轮次文件只写 JSONL、GitHub 元数据或当前只读复核能证明的事实；未知 base/run id
   明确写“日志未记录”，不得推测。
3. 既有 `2026-11-16-report.md` 只追加；五份新轮次文件创建后不得覆盖。
4. 写 `plans/stage-2-excel.md`、`PLAN.md`、`README.md`、`LEDGER.md` 前取 guard，
   写完立即释放；超时必须通报，不得 `--force`。
5. TASK-376 保持 Blocked；本卡只记录六项建议，不把建议升级为 Accepted 契约。
6. 本会话 PowerShell 零输出超时后的 `cmd` 回退与 1 次 `bash` 误填必须如实记录。

## 验收命令

```powershell
cargo run -p xtask -- automation-rounds --run-date 2026-11-16
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo run -p xtask -- refscan
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- check-ledger
git diff --check
```

预期：`automation-rounds` 输出 `existing_rounds_for_run_date=5`、`next_round=6`；
真实 automation rollout 来源下 `n_auto=5`，当前人类会话下 `n_auto=0` 且
`reset_source=human-session`。其余门禁 0 error；产品代码、schema、ADR、spec 与依赖
零差异；PR checks 11/11 后合并，并回填 post-merge main CI。

## DoD

- [ ] 五份 `2026-11-16-round-{1..5}.md` 均包含 JSONL 可证实的事实与证据边界。
- [ ] 审计明确根因是轮次产物遗漏，不是 `automation-rounds` 算法错误。
- [ ] 机器输出在回填前后形成可复现对照；automation 来源为 `5 / 6 / 5`，
  人类会话为 `5 / 6 / 0 + human-session reset`。
- [ ] 运行日报告以追加方式说明回填，不改写既有段落。
- [ ] TASK-376 的 D1-D6 建议与人工裁决闸门保持清晰分离。
- [ ] 十项 xtask/文档门禁与 `git diff --check` 全绿。
- [ ] 记录 `automation-rounds` 当前扫描工作区而非 `origin/main` 的 ADR-0054 D3
  实现偏差，并给出独立修复方案。
- [ ] 产品代码、schema、ADR、spec、adapters 与依赖零差异。
- [ ] PR checks、merge 与 post-merge main CI 证据回填后关闭本卡。

<!-- ══ 分界线：以上是卡片正文，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上任何正文行 = 漂移触发 ⑤；
     状态行仅按 ADR-0083 与产生该状态的提交同批更新。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-378 2026-11-16 自动化轮次证据回填
【目标】补齐 round 1~5 的仓库事实源，纠正运行日计数可观测缺口，并给出复发防护方案
【write scope】仅：TASK-378 本卡、运行日 round 1~5、运行日报告追加段、证据审计、Stage 2 计划/PLAN/README 的允许区域、LEDGER
【铁律】证据不得伪造；历史事实只追加不改写；公共热点写前取 guard；公共契约未裁决不得自行放行；计划条目正文只读
【禁止】改 automation-rounds 算法、ADR/spec、protocol/adapters/产品代码/依赖；代答 TASK-376 D1-D6；启动真实 GUI
【验收】automation-rounds 从 0/1/0 变为 5/6；automation 来源 n_auto=5、人类来源 n_auto=0；hygiene/card-check/docscan/refscan/memory-counts/check-ledger/adr-index/verify-schemas/codegen 与 diff check 全绿；PR 11/11 + main CI
【依赖】ADR-0054、ADR-0088、ADR-0089、ADR-0099；TASK-373/374/375/377 Done，TASK-376 Blocked
【疑问】无；TASK-376 的六项人类裁决继续保留
```

### 2. 实际改动文件（逐个核对是否在 In scope 内）

- `tasks/TASK-378-automation-round-evidence-backfill-2026-11-16.md`
- `docs/audits/automation-round-evidence-2026-11-16.md`
- `docs/automations/2026-11-16-round-1.md`
- `docs/automations/2026-11-16-round-2.md`
- `docs/automations/2026-11-16-round-3.md`
- `docs/automations/2026-11-16-round-4.md`
- `docs/automations/2026-11-16-round-5.md`
- `docs/automations/2026-11-16-report.md`（仅追加）
- `plans/stage-2-excel.md`（仅允许区域）
- `PLAN.md`（仅当前状态块）
- `README.md`（仅三处）
- `LEDGER.md`（仅追加）

### 3. 验收输出摘要（命令 → 结果）

- `cargo run -p xtask -- automation-rounds --run-date 2026-11-16`（当前人类会话）
  → `existing_rounds_for_run_date=5`、`next_round=6`、`n_auto=0`、
  `reset_source=human-session`、`session_source=human`，PASSED。
- 同一命令使用 Slot 1 的真实 automation rollout（只读临时 `CODEX_HOME`）
  → `existing_rounds_for_run_date=5`、`next_round=6`、`n_auto=5`、
  `reset_source=daily-0800`、`session_source=automation`，PASSED。
- `cargo run -p xtask -- verify-schemas` → PASSED；5 schemas，0 error。
- `cargo run -p xtask -- codegen --check` → PASSED；0 drift，0 error。
- `cargo run -p xtask -- hygiene` → PASSED；486 files，0 error，168 条既有 warning。
- `cargo run -p xtask -- card-check` → PASSED；281 files，0 error，88 条既有 warning。
- `cargo run -p xtask -- docscan` → PASSED；667 files，0 error，175 条既有 warning。
- `cargo run -p xtask -- refscan` → PASSED；1194 files，0 error，0 warning。
- `cargo run -p xtask -- memory-counts` → PASSED；10 files，0 error，1 条既有 paint warning。
- `cargo run -p xtask -- adr-index` → PASSED；92 files，0 error，0 warning。
- `cargo run -p xtask -- check-ledger` → PASSED；plan date 2026-11-17、ledger 末行
  2026-11-16、current stage=阶段 2，0 error。
- `git diff --check` → PASSED。

### 4. DoD 逐条核对

- [x] 五份 `2026-11-16-round-{1..5}.md` 均包含 JSONL 可证实的事实与证据边界。
- [x] 审计明确根因是轮次产物遗漏，不是 `automation-rounds` 算法错误。
- [x] 机器输出在回填前后形成可复现对照；automation 来源为 `5 / 6 / 5`，
  人类会话为 `5 / 6 / 0 + human-session reset`。
- [x] 运行日报告以追加方式说明回填，未改写既有段落。
- [x] TASK-376 的 D1-D6 建议与人工裁决闸门保持清晰分离。
- [x] 十项 xtask/文档门禁与 `git diff --check` 全绿。
- [x] 记录 `automation-rounds` 当前扫描工作区而非 `origin/main` 的 ADR-0054 D3
  实现偏差，并给出独立修复方案。
- [x] 产品代码、schema、ADR、spec、adapters 与依赖零差异。
- [ ] PR checks、merge 与 post-merge main CI 证据待合入后追加回填。

### 5. 偏差

- PowerShell 复核累计出现 4 次零输出超时：开工 `git status` / `git log` 各一次，
  以及两次 `automation-rounds` 验证；均终止后按 ADR-0100 回退 `cmd`。
- 本会话误填 `shell: "bash"` 2 次：第一次只读列出 TASK-378/378 不存在的文件并成功返回空；
  第二次只读搜索 guard 说明，因路径分隔符丢失产生 `xtask\src\...` 不存在错误。
  两次均未修改仓库，后续命令改回 `cmd`。
- guard acquire 的 `--intent` 含空格时被 shell 错误拆分，意外创建 17 把额外目标锁。立即
  `guard status` 核实全部 owner=`codex-01a17b28-task378`，用 release/reap 清理 17 把；
  随后用无空格 intent 仅重新获取计划、PLAN、README、LEDGER 四把锁并逐个释放。
  最终 `guard status` 回到 NONE，没有遗留锁，也没有产生仓库文件。

### 6. 更合理做法

- 每轮 automation 在最终审计前把“本轮 `round-N.md` 已在 `main`”作为自删硬前提，并在
  JSONL 写入该文件的路径和 merge commit；运行日报告再以五个轮次文件做机器汇总。

### 7. 遗留问题

- TASK-376 的 D1-D6 仍待人类一次性批准。
- 是否把轮次产物存在性检查接入 automation 最终审计，作为独立后续治理卡处理。

### 8. 新增长期记忆

- 无。该结论是 2026-11-16 运行日证据与 ADR-0054 D3 实现偏差，已由运行日审计和
  `LEDGER.md` 保存；未改 `MEMORY.md` 或 `docs/memory/*`。

### 9. 给审阅者的关注点

- 核对轮次文件只包含 JSONL 或 GitHub 能证明的事实，未知值没有被补造。
- 核对 `5 / 6 / 5` 的变化来自新增轮次文件，不是修改 `automation-rounds` 或 reset 文件。
- 核对本卡没有推进 TASK-376 的公共 Adapter 契约。
