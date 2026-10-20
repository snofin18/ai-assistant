# TASK-324　已完成治理卡状态与计划索引对账

- 状态：Done
- 阶段：1　子阶段：跨阶段治理　批次：治理池　依赖：TASK-292 / 293 / 294 / 296 / 297 / 303 / 304　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。

---

- **write scope**：
  - `tasks/TASK-324-completed-governance-card-status-reconciliation.md`
  - `tasks/TASK-292-platform-geometry-view-contract.md`（仅状态行与执行记录区）
  - `tasks/TASK-293-windows-window-element-geometry.md`（仅状态行与执行记录区）
  - `tasks/TASK-294-paint-synthetic-click-primitive.md`（仅状态行与执行记录区）
  - `tasks/TASK-296-palette-view-fallback.md`（仅状态行与执行记录区）
  - `tasks/TASK-297-layer-panel-alt-l.md`（仅状态行与执行记录区）
  - `tasks/TASK-303-t3-1-real-canvas-inputs.md`（仅状态行与执行记录区）
  - `tasks/TASK-304-per-monitor-v2-dpi-awareness.md`（仅状态行与执行记录区）
  - `docs/audits/completed-governance-card-reconciliation-2026-10-20.md`
  - `docs/automations/2026-10-20-round-4.md`
  - `plans/stage-1-pilots.md`（仅新增任务行与当前进度）
  - `PLAN.md`（仅当前状态块）
  - `README.md`（仅状态行、`## 当前阶段`、`## 最近进展`）
  - `LEDGER.md`（只追加）
- **关联**：ADR-0031、ADR-0039、ADR-0041、ADR-0054、ADR-0083、ADR-0088、ADR-0089

**目标**

对账 7 张已经合并、已有静态/真机证据，但任务卡仍停在 `Review` 且未进入阶段计划索引的治理卡：
TASK-292 / 293 / 294 / 296 / 297 / 303 / 304。用真实 GitHub 元数据与现有 LEDGER 证据确认完成事实，
只更新状态行、执行记录补充、计划完成索引和报告，不触碰正文契约或产品代码。

**In scope**

- 核对上述 7 张卡对应 merged PR 的 base、merge commit、CI checks、pending 与 state。
- 核对 LEDGER 中的真机通过证据和后续 TASK-044 十次验收证据。
- 将 7 张卡的状态行从 `Review` 更新为 `Done`，并在各自执行记录区追加一条可追溯补充。
- 新增 `docs/audits/completed-governance-card-reconciliation-2026-10-20.md` 作为逐卡对账表。
- 在 `plans/stage-1-pilots.md` 新增 7 张已完成卡与 TASK-324 的计划行，并更新当前进度句。
- 同步 `PLAN.md` 当前状态块、`README.md` 三处、`LEDGER.md` 与本轮 automation 留痕。

**Out of scope（做了算漂移）**

- 任何产品代码、测试、schema、公共接口、ADR、spec 或依赖改动。
- 修改 7 张卡分界线以上除唯一状态行以外的任何正文。
- 修改既有 LEDGER 行、历史轮次正文或历史执行记录。
- 复跑真实 GUI；本轮只核对已落盘的合并与真机证据。
- 扩展 `PL-115` 的风险豁免，或处理 `PL-118` 号段裁决。

**必须遵守**

- `N_auto > 5` 时只补 / 校阶段报告，不写业务代码（ADR-0089）。
- 每个公共热点文件写前取 guard 锁，写完立即释放；不得 `--force`。
- 任务卡正文只读；状态行是唯一例外，更新必须与真实状态同批（ADR-0083）。
- 计划文件只允许新增任务行、完成标记与当前进度，不得改写既有条目正文（ADR-0041）。
- 每条 `exec_command` 必须显式声明 `shell: "powershell"`；误用必须如实记录。

**验收命令（agent 必须全部执行并记录结果）**

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

- [ ] 7 张卡的 merged PR、base、merge hash、checks、pending 与完成证据逐项留痕。
- [ ] 7 张卡只在正文区修改唯一状态行，未改其他正文。
- [ ] 7 张卡执行记录区均有 TASK-324 追加证据。
- [ ] `plans/stage-1-pilots.md` 新增 7 张完成卡与 TASK-324 行，并更新当前进度。
- [ ] `PLAN.md` / `README.md` / `LEDGER.md` 只改允许区域并保持新鲜度。
- [ ] 九条验收命令全部实际执行并记录结果。
- [ ] 无产品代码、测试、schema、ADR、spec 或依赖改动。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-324 已完成治理卡状态与计划索引对账
【目标】依据真实 merged PR / CI / 真机证据，把 7 张已完成但仍为 Review 的治理卡收口为 Done
【write scope】仅本卡、7 张卡状态行与记录区、审计、round 4、计划、PLAN、README、LEDGER
【铁律】证据不伪造；不改公共契约；计划条目正文只读；热点文件取 guard；N_auto>5 只做 docs-only
【禁止】产品代码 / 测试 / schema / ADR / spec / 依赖；真实 GUI；改写历史记录；扩大 scope
【验收】git diff --check / name-only；xtask hygiene/card-check/docscan/check-ledger/refscan/memory-counts/adr-index
【依赖】PR #402/#403/#404/#411/#412/#426/#428/#429 均 MERGED、base=main、11/11 success
【疑问】无
```

### 2. 实际改动文件

- `tasks/TASK-324-completed-governance-card-status-reconciliation.md`
- `tasks/TASK-292-platform-geometry-view-contract.md`（仅状态行 + 记录补充）
- `tasks/TASK-293-windows-window-element-geometry.md`（仅状态行 + 记录补充）
- `tasks/TASK-294-paint-synthetic-click-primitive.md`（仅状态行 + 记录补充）
- `tasks/TASK-296-palette-view-fallback.md`（仅状态行 + 记录补充）
- `tasks/TASK-297-layer-panel-alt-l.md`（仅状态行 + 记录补充）
- `tasks/TASK-303-t3-1-real-canvas-inputs.md`（仅状态行 + 记录补充）
- `tasks/TASK-304-per-monitor-v2-dpi-awareness.md`（仅状态行 + 记录补充）
- `docs/audits/completed-governance-card-reconciliation-2026-10-20.md`
- `docs/automations/2026-10-20-round-4.md`
- `plans/stage-1-pilots.md`（新增任务行 + 当前进度）
- `PLAN.md`（当前状态块）
- `README.md`（状态行 / 当前阶段 / 最近进展）
- `LEDGER.md`（追加 pending 行）

### 3. 验收输出摘要

- 真实 GitHub 元数据：PR #402 / #403 / #404 / #411 / #412 / #426 / #428 / #429 均
  `state=MERGED`、`base=main`、11 checks / 11 success / pending 0 / failure 0。
- 历史 PR 的合并后 `mergeable` / `mergeStateStatus` 返回 `UNKNOWN`；未伪写为合并时的
  `MERGEABLE` / `CLEAN`。
- `git diff --check` → exit 0。
- `git diff --name-only` → 只列允许的 11 个已跟踪文件；3 个新文件由 `git status` 核对。
- `cargo run -p xtask -- hygiene` → PASSED；438 files，0 error，136 warnings。
- `cargo run -p xtask -- card-check` → PASSED；218 files，0 error，35 warnings。
- `cargo run -p xtask -- docscan` → PASSED；522 files，0 error，229 warnings。
- `cargo run -p xtask -- check-ledger` → PASSED；`plan_date=2026-10-20`、
  `ledger_last_date=2026-10-20`、`current_stage=阶段 1`。
- `cargo run -p xtask -- refscan` → PASSED；989 files，0 error，0 warning。
- `cargo run -p xtask -- memory-counts` → PASSED；9 files，0 error，1 existing
  archive-threshold warning。
- `cargo run -p xtask -- adr-index` → PASSED；87 files，0 error，0 warning。
- PR #507：base `main`，22 checks = 21 success + 1 skipped，pending 0，
  `mergeable=MERGEABLE`、`mergeStateStatus=CLEAN`；merge
  `863ef36a4778009d4bdde77866feb060f6cfd1e8`。

### 4. DoD 逐条核对

- [x] 7 张卡的 merged PR、base、merge hash、checks、pending 与完成证据逐项留痕。
- [x] 7 张卡只在正文区修改唯一状态行，未改其他正文；`card-check` 0 error。
- [x] 7 张卡执行记录区均有 TASK-324 追加证据。
- [x] `plans/stage-1-pilots.md` 新增 7 张完成卡与 TASK-324 行，并更新当前进度。
- [x] `PLAN.md` / `README.md` / `LEDGER.md` 只改允许区域并保持新鲜度。
- [x] 九条验收命令全部实际执行并记录结果。
- [x] 无产品代码、测试、schema、ADR、spec 或依赖改动。

### 5. 偏差

- 全轮共 **21 条命令误用 `shell: "bash"`**：前 18 条为查询 / 校验，其中 17 条因命令不存在或
  语法失败，1 条 `git rev-parse` 成功读到只读哈希但后续 PowerShell cmdlet 失败；第 19 条是
  已授权的分支 `git push`；第 20/21 条是 fetch / sync 尝试，分别因 bash 语法和 TLS 失败。
  所有 21 条均在发现后以显式 `shell: "powershell"` 重跑，仓库最终内容无越界改动。

### 6. 更合理做法

优先新增一张治理卡承接跨卡状态对账，避免在同一轮修改 7 张卡的正文范围而不留统一审计依据。

### 7. 遗留问题

- Stage 1c Edge/Chrome、真实 GUI、浏览器专用 profile 仍需人类会话或闸门重置。
- `PL-115` 限期豁免与 `PL-118` 300+ 历史号段归属继续开放。
- 当前 `N_auto=25>5`，后续 automation 仍只能补 / 校报告。

### 8. 新增长期记忆

- 无。七张卡的完成事实已分别落到卡执行记录、审计、round 4 与 LEDGER，未形成新的通用规则。

### 9. 给审阅者的关注点

1. 确认 7 张卡分界线以上只有 `- 状态：` 一行变化。
2. 确认历史 merged PR 的 `UNKNOWN` mergeability 未被包装成当时的 `MERGEABLE` / `CLEAN`。
3. 确认计划表只新增任务行与当前进度，没有改写既有条目正文。
