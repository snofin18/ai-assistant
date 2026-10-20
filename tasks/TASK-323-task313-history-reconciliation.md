# TASK-323 TASK-313 history reconciliation

- 状态：**Done**
- 阶段：1　子阶段：跨阶段治理　批次：治理池　依赖：TASK-313 / PR #453 / PR #466 / PR #468 / PR #469　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。

---

- **write scope**：
  - `tasks/TASK-323-task313-history-reconciliation.md`
  - `tasks/TASK-313-selector-fallback-only-promotion.md`（仅执行记录区）
  - `docs/automations/2026-10-20-round-3.md`
  - `plans/stage-1-pilots.md`（仅完成标记、任务行与当前进度）
  - `PLAN.md`（仅当前状态块）
  - `README.md`（仅状态行、当前阶段、最近进展）
  - `LEDGER.md`（只追加）
  - `docs/PARKING_LOT.md`（只追加）
- **关联**：ADR-0031、ADR-0039、ADR-0041、ADR-0054、ADR-0083、ADR-0088、ADR-0089、ADR-0095、`PL-115`、`PL-117`

**目标**

依据已合并提交与真实 GitHub 元数据，收口同一 TASK-313 号下两段历史工作留下的
可追溯缺口：`deny.toml` 限期豁免（PR #453）与 `fallback_only` 运行时升格
（ADR-0095，PR #466 / #468 / #469）。

**In scope**

- 复核 PR #453、#466、#468、#469 的 base、merge commit、CI 与文件范围。
- 在现有 TASK-313 卡执行记录区追加同号历史核对，不改卡片正文。
- 新建本卡作为历史对账的正式留痕；不建第二张同号 TASK-313 卡。
- 在计划表补 TASK-313 / TASK-323 完成标记行，并更新当前进度句。
- 追加本轮 automation 留痕，关闭 `PL-117` 的“无可追溯落点”缺口，
  同时保留 `PL-115` 的限期豁免风险与复审条件。
- 同步 `PLAN.md` 当前状态块、`README.md` 允许三处与 `LEDGER.md`。

**Out of scope（做了算漂移）**

- 任何产品代码、测试、schema、公共接口、ADR、spec 或依赖改动。
- 修改 TASK-313 分界线以上正文、既有 LEDGER 行或历史轮次正文。
- 延长、删除或放宽 `PL-115` 的 `deny.toml` 豁免。
- 伪造一张不存在的原始 TASK-313 派单卡，或建立第二个同号卡文件。
- 真实 GUI、浏览器、网络凭据或商业应用操作。

**必须遵守**

- `N_auto > 5` 时只补 / 校阶段报告，不写业务代码（ADR-0089）。
- 每个公共热点文件写前取 guard 锁，写完立即释放；不得 `--force`。
- 计划文件的既有条目正文只读；只允许完成标记、新增任务行和当前进度块。
- 历史异常必须如实描述，不能用“已补卡”掩盖同号复用事实。
- 每条 `exec_command` 必须显式声明 `shell: "powershell"`；误用必须记录。

**验收命令（agent 必须全部执行并粘贴输出）**

```powershell
git diff --check
git diff --name-only
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo run -p xtask -- check-ledger
cargo run -p xtask -- refscan
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
```

**完成定义（DoD）**

- [ ] 留痕明确列出 PR #453 与 ADR-0095 三段 PR 的 base、merge hash、CI 与文件范围。
- [ ] 现有 TASK-313 卡只在执行记录区追加同号历史核对。
- [ ] 不建立第二个 TASK-313 同号卡；TASK-323 承接对账证据。
- [ ] `PL-117` 追加闭环记录，同时明确 `PL-115` 仍开放。
- [ ] `plans/stage-1-pilots.md`、`PLAN.md`、`README.md` 只改允许区域。
- [ ] `LEDGER.md` 追加 pending 行；合并后追加实际 merge hash 回填行。
- [ ] 九条验收命令全部实际执行并记录结果。
- [ ] 无产品代码、测试、schema、ADR、spec 或依赖改动。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-323 TASK-313 历史留痕一致性收口
【目标】依据真实 PR / ADR / LEDGER 证据收口 TASK-313 同号双工作流的历史缺口
【write scope】仅本卡、TASK-313 记录区、round 3、计划、PLAN、README、LEDGER、PARKING_LOT
【铁律】证据不伪造；不改公共契约；计划条目正文只读；热点文件取 guard；N_auto>5 只做报告类工作
【禁止】产品代码 / 测试 / schema / ADR / spec / 依赖；第二张同号卡；改写历史正文
【验收】git diff --check / name-only；xtask hygiene/card-check/docscan/check-ledger/refscan/memory-counts/adr-index
【依赖】PL-117；PR #453 / 7b81bfb；ADR-0095；PR #466 / 176b08b；PR #468 / 22b1c28；PR #469 / a04aa85
【疑问】历史号复用无法无痛拆成第二个同号卡；默认保留现有卡并在本卡对账
```

### 2. 实际改动文件

- `tasks/TASK-323-task313-history-reconciliation.md`
- `tasks/TASK-313-selector-fallback-only-promotion.md`（仅执行记录区）
- `docs/automations/2026-10-20-round-3.md`
- `plans/stage-1-pilots.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`
- `docs/PARKING_LOT.md`

### 3. 验收输出摘要

历史证据（本轮实际查询）：

- `gh pr view 453` → `MERGED`、base `main`、merge `7b81bfb`、
  CI 22 checks = 21 success + 1 skipped。
- `gh pr view 466/468/469` → 均 `MERGED`、base `main`，
  merge 分别为 `176b08b` / `22b1c28` / `a04aa85`；
  各 CI 22 checks = 21 success + 1 skipped。
- `git diff --stat <merge>^1 <merge>` → PR #453 为 5 files +41 / -2；
  selector 三段为 4 / 15 / 5 files，未越过 ADR-0095 的声明范围。

- `git diff --check` → exit 0。
- `git diff --name-only` → 仅列已跟踪的 TASK-313 记录区改动；未跟踪新文件由 `git status` 另行核对。
- `cargo run -p xtask -- hygiene` → PASSED；438 files，0 error，136 existing warnings。
- `cargo run -p xtask -- card-check` → PASSED；217 files，0 error，35 existing warnings。
- `cargo run -p xtask -- docscan` → PASSED；519 files，0 error，229 existing warnings。
- `cargo run -p xtask -- check-ledger` → PASSED；`plan_date=2026-10-20 (PLAN.md:15)`、
  `ledger_last_date=2026-10-20 (LEDGER.md:694)`、`current_stage=阶段 1`。
- `cargo run -p xtask -- refscan` → PASSED；986 files，0 error，0 warning。
- `cargo run -p xtask -- memory-counts` → PASSED；9 files，0 error，1 existing
  `paint.md` archive-threshold warning。
- `cargo run -p xtask -- adr-index` → PASSED；87 files，0 error，0 warning。

### 4. DoD 逐条核对

- [x] 留痕列出 PR #453 与 ADR-0095 三段 PR 的 base、merge hash、CI 与文件范围。
- [x] 现有 TASK-313 卡只在执行记录区追加同号历史核对。
- [x] 不建立第二个 TASK-313 同号卡；本卡承接对账证据。
- [x] `PL-117` 追加闭环记录，并明确 `PL-115` 仍开放。
- [x] `plans/stage-1-pilots.md`、`PLAN.md`、`README.md` 只改允许区域。
- [x] `LEDGER.md` 追加 pending 行；合并后另开只追加回填 PR 记录实际 merge hash。
- [x] 九条验收命令全部实际执行并记录结果。
- [x] 无产品代码、测试、schema、ADR、spec 或依赖改动。

### 5. 偏差

- 已有 11 条只读命令误用 `shell: "bash"`：其中 9 条是查询命令、
  1 条是 guard release 循环语法、1 条是 `adr-index`；均未修改仓库内容。
  每次发现后都以显式 `shell: "powershell"` 重新执行。该问题是本轮操作偏差，
  不是 PowerShell 通道故障。
- 历史事实与 `PL-117` 原文不完全相同：现有 TASK-313 卡确实存在，
  但它只覆盖后来的 `fallback_only` 工作；PR #453 的 `deny.toml` 工作仍没有独立卡。
  因此本轮按“同号历史对账”收口，而不是伪称原始卡已完整存在。

### 6. 更合理做法

- 对一个号下的两段工作，不建立第二个同号文件；保留可执行语义最完整的现有卡，
  用新的治理号保存编号冲突、真实 PR 链与处置结论，既满足一卡一文件，也不掩盖历史。

### 7. 遗留问题

- `PL-115` 继续开放，复审触发条件与 2026-12-31 兜底到期不变。
- `PL-118`（300+ 历史号段归属）仍需 ADR / Orchestrator 裁决。
- Stage 1c Edge/Chrome、真实 GUI / 浏览器 profile 仍等待人类会话或闸门重置。

### 8. 新增长期记忆

- 无新增长期记忆。该同号事件已由 `PL-117` 闭环记录、TASK-313 记录区与本轮留痕覆盖；
  尚未形成需要提升为通用 FACT/PITFALL 的新规则。

### 9. 给审阅者的关注点

1. 确认没有第二个 TASK-313 文件，也没有改写 TASK-313 分界线以上正文。
2. 确认 `PL-117` 的闭环准确表述为“历史可追溯缺口已收口”，而不是“原始 PR #453 从未缺卡”。
3. 确认 `PL-115` 仍开放，且本轮的表格行与轮次文件没有扩大 `deny.toml` 豁免。
