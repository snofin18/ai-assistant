# TASK-321 Stage 1 status consistency closeout

- 状态：**Done**
- 阶段：1　子阶段：跨阶段治理　批次：治理池　依赖：TASK-309 / TASK-317 / TASK-318 / TASK-319 / TASK-320　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。

---

- **write scope**：`tasks/TASK-321-stage1-status-consistency-closeout.md`、`docs/audits/stage-1-status-consistency-2026-10-20.md`、`plans/stage-1-pilots.md`、`PLAN.md`、`README.md`、`LEDGER.md`、`docs/automations/2026-10-20-round-1.md`
- **关联**：ADR-0031、ADR-0039、ADR-0041、ADR-0054、ADR-0088、ADR-0089

**目标**

把 2026-10-19~20 已完成的 Paint / UIA 卡片与阶段计划、全局状态、README、LEDGER
的事实源对齐，并留下可复核的状态差异审计报告；本轮只做 docs-only 收口。

**In scope**

- 核对任务卡状态、`LEDGER.md` 合并证据与 `plans/stage-1-pilots.md` 完成标记 / 当前进度。
- 在计划文件只新增完成标记、任务行和当前进度内容；不改既有任务条目正文或排期。
- 更新 `PLAN.md` 当前状态块、`README.md` 允许的三处状态内容。
- 新增状态一致性审计报告与本 automation 轮次留痕。

**Out of scope**

- 任何产品代码、测试断言、依赖、schema、公共接口、ADR 或 spec 改动。
- 改写任务卡正文，或借状态同步修改计划条目的名称、write scope、依赖、预估和验收要点。
- 真实 GUI、浏览器、网络、凭据或商业应用操作。
- 宣称 Stage 1 已完成；当前仍只收口到 Stage 1b，Stage 1c 尚未完成。

**必须遵守**

- 遵守 AGENTS.md 十条铁律与 ADR-0041：计划文件只改完成状态和当前进度，禁止改条目正文。
- `N_auto > 5` 时只补 / 校阶段报告，不写业务代码（ADR-0089）。
- 每个热点文件写前取 guard 锁，写完立即释放；不得 `--force`。
- 所有证据必须来自实际命令输出；未运行项不得写成通过。

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

- [ ] 审计报告列出 2026-10-19~20 完成卡在任务卡 / 计划 / PLAN / README 的差异与处置。
- [ ] `plans/stage-1-pilots.md` 只新增完成标记、任务行和当前进度内容，既有条目正文不变。
- [ ] `PLAN.md` 只更新当前状态块，`README.md` 只更新允许的三处。
- [ ] `LEDGER.md` 追加 pending 行；合并后追加实际 merge hash 回填行。
- [ ] 八条验收命令全部实际执行并记录结果。
- [ ] 本 automation 轮次文件记录 `N_auto = 22 > 5` 的 report-only 依据。
- [ ] 无任何产品代码、schema、ADR、spec 或依赖改动。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-321 Stage 1 状态一致性收口
【目标】把 2026-10-19~20 已完成卡与 PLAN/stage/README/LEDGER 的事实源对齐，并留下可审计报告
【write scope】仅 TASK-321 卡、状态审计报告、plans/stage-1-pilots.md、PLAN.md、README.md、LEDGER.md、本轮 automation 留痕
【铁律】无静默失败；不得静默扩大范围；计划文件只改完成标记/当前进度；证据不伪造；N_auto>5 只做报告类工作
【禁止】产品代码/schema/trait/依赖；ADR/spec；改写任务卡正文；改排期或任务条目正文；覆盖既有 automation 轮次
【验收】git diff --check；xtask hygiene/card-check/docscan/check-ledger/refscan/memory-counts/adr-index
【依赖】ADR-0031/0039/0041/0054/0088/0089；origin/main=d96ae4c
【疑问】无
```

### 2. 实际改动文件

- `tasks/TASK-321-stage1-status-consistency-closeout.md`
- `docs/audits/stage-1-status-consistency-2026-10-20.md`
- `plans/stage-1-pilots.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`
- `docs/automations/2026-10-20-round-1.md`

### 3. 验收输出摘要

- `git diff --check` → exit 0（LF 归一化后）
- `cargo run -p xtask -- hygiene` → PASSED；438 files，0 error，136 existing warnings
- `cargo run -p xtask -- card-check` → PASSED；215 files，0 error，35 existing warnings
- `cargo run -p xtask -- docscan` → PASSED；514 files，0 error，229 existing warnings
- `cargo run -p xtask -- check-ledger` → PASSED；plan_date / ledger_last_date 均为 `2026-10-20`
- `cargo run -p xtask -- refscan` → PASSED；981 files，0 error，0 warning
- `cargo run -p xtask -- memory-counts` → PASSED；9 files，0 error，1 existing
  `paint.md` archive-threshold warning
- `cargo run -p xtask -- adr-index` → PASSED；87 files，0 error，0 warning

### 4. DoD 逐条核对

- [x] 审计报告列出 2026-10-19~20 完成卡在任务卡 / 计划 / PLAN / README 的差异与处置。
- [x] `plans/stage-1-pilots.md` 只新增完成标记、任务行和当前进度内容。
- [x] `PLAN.md` 只更新当前状态控制行，`README.md` 只更新允许的三处。
- [x] `LEDGER.md` 已追加 pending 行；merge hash 待合并后追加回填。
- [x] 八条验收命令全部实际执行并通过。
- [x] 轮次文件记录 `N_auto = 22 > 5` 的 report-only 依据。
- [x] 无产品代码、schema、ADR、spec 或依赖改动。

### 5. 偏差

非漂移执行偏差：

- 共 7 条命令误用 `shell: "bash"`：6 条只读查询（1 条路径查询、3 条文档读取与
  2 条 `git diff`）和 1 条 `git switch`。其中前 4 条因所选 shell 中命令不存在而失败，
  后续 3 条成功但均未改仓库内容；全部改用显式 `shell: "powershell"`，并记录在本轮
  automation 留痕与外部日志。
- `README.md` 第一次写通道调用中，PowerShell 对 `List<string>.InsertRange`
  发生集合类型错误：状态行已写入，两段进展未插入。随后强类型化并断言只插入一次。
- 通过 PowerShell 原生管道调用 `xtask write` 时，行结束符被重新编码为 CRLF；
  `git diff --check` / `hygiene` 当场发现。改为 `ProcessStartInfo.StandardInput`
  写入 UTF-8 LF，三个热点文件实测 `CR_COUNT=0` 后门禁转绿。

### 6. 更合理做法

- 对需要保持 LF-only 的仓库文件，使用 `xtask write` 时应直接向子进程 stdin
  写入已归一化的 UTF-8 字符串，而不是依赖 PowerShell 原生管道；写后同时检查
  `CR_COUNT`、EOF 与 `hygiene/crlf-line-endings`。

### 7. 遗留问题

无新增 parking-lot 项。Stage 1c Edge/Chrome、真实 GUI / 浏览器专用 profile 与最终
阶段安全验收仍是既有后续范围。

### 8. 新增长期记忆

无新增。既有 `docs/memory/pitfalls.md` 已记录 PowerShell 写文件默认换行与
「恰好一个 LF」规则，本卡按既有记忆执行。

### 9. 给审阅者的关注点

1. 计划表的既有条目正文按 ADR-0041 保持只读；TASK-044/256/257 等行的旧说明文字
   可能仍写 `Ready` / `Review`，以行首完成标记、任务卡状态与 LEDGER merge 证据为准。
2. `N_auto=22 > 5`，本卡必须是 docs-only；请重点检查没有产品代码、schema、ADR、
   spec 或依赖变化。
3. Stage 1 仍未完成：本卡只收口到 Stage 1b，不宣称 Stage 1c 或阶段 DoD 已完成。

## 追加证据（2026-10-20 round 2；只追加，不改写原记录）

- PR #501 `docs(automation): backfill TASK-321 merge evidence`
  - merge commit：`66711a4dfc95d621dd2cb4bc37c0ec82a7ccee6c`
  - 在 `LEDGER.md` 追加 PR #500 / merge `5e9f453` 的回填行，并补充 round 1
    合并后证据。
- PR #502 `docs(automation): correct shell deviation count`
  - merge commit：`9e990f6cda5977ff22cd03a81910fd7a00add50e`
  - 将本卡与 round 1 的 shell 偏差计数从 3 更正为 7；本卡状态仍为 **Done**。
- 本节不改变卡片正文、DoD 或原验收结论，只补记完成后实际发生的 docs-only 回填。
