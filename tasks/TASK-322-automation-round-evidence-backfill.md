# TASK-322 Automation round evidence backfill

- 状态：**Done**
- 阶段：1　子阶段：跨阶段治理　批次：治理池　依赖：TASK-321 / PR #501 / PR #502　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。

---

- **write scope**：`tasks/TASK-322-automation-round-evidence-backfill.md`、`tasks/TASK-321-stage1-status-consistency-closeout.md`（仅记录区）、`docs/audits/automation-round-evidence-2026-10-20.md`、`docs/automations/2026-10-20-round-1.md`、`docs/automations/2026-10-20-round-2.md`、`plans/stage-1-pilots.md`、`PLAN.md`、`README.md`、`LEDGER.md`
- **关联**：ADR-0031、ADR-0039、ADR-0041、ADR-0054、ADR-0083、ADR-0088、ADR-0089

**目标**

把 2026-10-20 round 1 合并后的 PR #501 / #502 证据、TASK-321 的后续回填事实与本轮
round 2 留痕对齐，消除 automation 轮次文件与任务卡执行记录停在首个 merge 的状态缺口。

**In scope**

- 复核 PR #500 / #501 / #502 的 base、CI、merge hash 与实际改动。
- 在 `docs/automations/2026-10-20-round-1.md` 追加合并后证据，不改写既有段落。
- 在 TASK-321 执行记录区分界线以下追加后续证据，不改卡片正文。
- 新增本卡与本轮证据审计；追加 `LEDGER.md`、round 2 与计划 / 当前状态同步。
- 记录本轮 `N_auto > 5` 的 report-only 依据和实际 shell 偏差。

**Out of scope**

- 任何产品代码、测试断言、schema、公共接口、ADR、spec、依赖改动。
- 改写历史轮次正文、既有 LEDGER 行、任务条目的名称 / write scope / 依赖 / 预估 / 验收要点。
- 把 PR #501 / #502 冒充为产品实现；它们只是文档证据回填与更正。
- 真实 GUI、浏览器、网络、凭据或商业应用操作。

**必须遵守**

- 遵守 AGENTS.md 十条铁律与 ADR-0041：计划文件只改完成标记、新增任务行和当前进度内容。
- `N_auto > 5` 时只补 / 校阶段报告，不写业务代码（ADR-0089）。
- 每个热点文件写前取 guard 锁，写完立即释放；不得 `--force`。
- 所有证据必须来自实际命令输出；未运行项不得写成通过。
- 每条 `exec_command` 必须显式声明 `shell: "powershell"`；误用必须如实记录。

**验收命令（agent 必须全部执行并粘贴输出）**

```powershell
git diff --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo run -p xtask -- check-ledger
cargo run -p xtask -- refscan
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
```

**完成定义（DoD）**

- [ ] 审计报告列出 PR #500 / #501 / #502 的 merge hash、CI 数量与缺失记录。
- [ ] round 1 以追加方式记录 PR #501 / #502，既有轮次内容不被改写。
- [ ] TASK-321 执行记录区追加 PR #501 / #502 证据，正文区不变。
- [ ] `plans/stage-1-pilots.md` 只新增完成标记 / 任务行 / 当前进度内容，既有条目正文不变。
- [ ] `PLAN.md` 只更新当前状态块，`README.md` 只更新允许的三处。
- [ ] `LEDGER.md` 追加 pending 行；合并后追加实际 merge hash 回填行。
- [ ] 八条验收命令全部实际执行并记录结果。
- [ ] 无任何产品代码、测试断言、schema、ADR、spec 或依赖改动。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-322 automation round evidence backfill
【目标】对齐 round 1 的 PR #501/#502 后续证据与 TASK-321 执行记录，并留下 round 2 审计
【write scope】仅本卡、TASK-321 记录区、证据审计、round 1/2、阶段计划、PLAN、README、LEDGER
【铁律】证据不伪造；不得静默扩大范围；计划条目正文只读；公共热点文件写前取 guard；N_auto>5 只做报告类工作
【禁止】产品代码 / 测试断言 / schema / 公共接口 / ADR / spec / 依赖；真实 GUI / 网络 / 凭据；覆盖历史轮次
【验收】git diff --check；xtask hygiene/card-check/docscan/check-ledger/refscan/memory-counts/adr-index
【依赖】ADR-0031/0039/0041/0054/0083/0088/0089；origin/main=9e990f6
【疑问】无
```

### 2. 实际改动文件

- `tasks/TASK-322-automation-round-evidence-backfill.md`
- `tasks/TASK-321-stage1-status-consistency-closeout.md`（仅追加记录区）
- `docs/audits/automation-round-evidence-2026-10-20.md`
- `docs/automations/2026-10-20-round-1.md`（仅追加）
- `docs/automations/2026-10-20-round-2.md`
- `plans/stage-1-pilots.md`（仅新增完成行与当前进度）
- `PLAN.md`（仅当前状态块）
- `README.md`（仅状态行 / 当前阶段 / 最近进展）
- `LEDGER.md`（仅追加）

### 3. 验收输出摘要

- `git diff --check` → exit 0。
- `cargo run -p xtask -- hygiene` → PASSED；438 files，0 error，136 existing warnings。
- `cargo run -p xtask -- card-check` → PASSED；216 files，0 error，35 existing warnings。
- `cargo run -p xtask -- docscan` → PASSED；517 files，0 error，229 existing warnings。
- `cargo run -p xtask -- check-ledger` → PASSED；`plan_date=2026-10-20`、
  `ledger_last_date=2026-10-20`、`current_stage=阶段 1`。
- `cargo run -p xtask -- refscan` → PASSED；984 files，0 error，0 warning。
- `cargo run -p xtask -- memory-counts` → PASSED；9 files，0 error，1 existing
  `paint.md` archive-threshold warning。
- `cargo run -p xtask -- adr-index` → PASSED；87 files，0 error，0 warning。
- PR #503 合并回填：`base=main`、22 checks（21 success + 1 skipped）、pending 0、
  `mergeable=MERGEABLE`、`mergeStateStatus=CLEAN`；merge commit `247d9c0`。

### 4. DoD 逐条核对

- [x] 审计报告列出 PR #500 / #501 / #502 的 merge hash、CI 数量与缺失记录。
- [x] round 1 以追加方式记录 PR #501 / #502，既有轮次内容未被改写。
- [x] TASK-321 执行记录区追加 PR #501 / #502 证据，正文区不变。
- [x] `plans/stage-1-pilots.md` 只新增完成标记 / 任务行 / 当前进度内容，既有条目正文不变。
- [x] `PLAN.md` 只更新当前状态块，`README.md` 只更新允许的三处。
- [x] `LEDGER.md` 已追加 pending 行；合并后追加实际 merge hash 回填行。
- [x] 八条验收命令全部实际执行并通过。
- [x] 无任何产品代码、测试断言、schema、ADR、spec 或依赖改动。

### 5. 偏差

- 开工核对阶段有 4 条只读命令误用 `shell: "bash"`：1 条 README 读取、2 组 `rg`
  查询、1 条 `git show`。四次均在 WSL bash 中因命令不存在而失败，未修改仓库内容；
  之后所有命令均显式使用 `shell: "powershell"`。该偏差不是 PowerShell 通道故障。

### 6. 更合理做法

- 对已经合并的 automation 轮次，后续回填应按“原轮次追加段 + 任务卡记录区追加段”
  处理，而不是改写首轮正文；这样既保留历史快照，也能让下一轮看到完整 PR 链。

### 7. 遗留问题

- Stage 1c Edge/Chrome、真实 GUI / 浏览器专用 profile 与最终安全验收仍未完成。
- 当前 `N_auto = 23 > 5`，后续 automation 只能继续补 / 校报告，直到人类会话重置。
- 等本轮 PR 合并后，另开只追加回填 PR 记录实际 merge hash。

### 8. 新增长期记忆

- 无新增仓库长期记忆。shell 严格规则已由 `docs/memory/pitfalls.md` 的
  2026-10-18 条目和自动化操作手册 §4.1.c-1 覆盖；本卡只追加新的偏差证据。

### 9. 给审阅者的关注点

1. 只检查历史记录区的追加是否有事实依据；PR #501 / #502 的 merge hash 与 CI 数量
   均来自 GitHub / git 实际输出。
2. `N_auto=23 > 5`，本卡必须保持 docs-only；重点确认没有产品代码、schema、ADR、
   spec 或依赖变化。
3. TASK-321 分界线以上正文没有变化；只允许执行记录区出现新增内容。
