# TASK-239　停车位存量复核收口（2026-10-05）

- 状态：**Done（2026-10-05）**
- 阶段：1　子阶段：治理　批次：治理池　依赖：TASK-237、TASK-238（均已 Done）
- 预估：M　难度：M
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。分界线以上为正文（Orchestrator 所有，Implementer 只读）。
- 关联：`docs/PARKING_LOT.md`、`docs/audits/parking-lot-review-2026-10-05.md`、`LEDGER.md`

---

## 目标（一句话）

对 `docs/PARKING_LOT.md` 中仍开放的存量条目做一次机器证据复核，逐条追加“已闭环 / 已被取代 / 复核保留”结论，并产出可审阅的复核报告。

## 背景（为什么现在做）

停车位从 2026-09-16 累积到 2026-10-04，已有大量条目被后续 ADR、任务卡和门禁实现实际闭环，但原表仍保留旧状态。与此同时，也有一批条目的状态列看似关闭、正文或后续行却明确留下未完成的剩余项。本轮目标不是清空停车位，而是把它恢复成可信的待办清单。

## write scope

- `docs/PARKING_LOT.md`（**仅追加**复核行，不改写原始行）
- `docs/audits/parking-lot-review-2026-10-05.md`（新建）
- `tasks/TASK-239-parking-lot-review-2026-10-05.md`（本卡）
- `LEDGER.md` / `PLAN.md`（仅「当前状态」块）/ `README.md`（仅三处）
- `plans/stage-1-pilots.md`（完成标记 + 当前进度句）
- `docs/automations/2026-10-05-round-1.md`（本轮留痕）

## In scope

- 盘点截至 2026-10-05 仍未被后续事实闭环的停车位条目。
- 用仓库现有命令、文件存在性、ADR 登记表、任务卡状态与源码扫描做机器验证。
- 对每条给出三类结论之一：`已闭环（日期，本轮）`、`已被后续 ADR/卡取代，无需再做`、`复核保留（2026-10-05）`。
- 报告逐条列出原状态、复核结论、证据命令、结果摘要，并明确标注机器验证与人工判断。

## Out of scope（做了算漂移）

- 不改写或删除任何既有停车位行。
- 不放宽、删除或重解释任何门禁判据。
- 不改产品代码、公共 trait/schema、依赖、crate、顶层目录、lint。
- 不为让条目变绿而补造证据；无法机器验证的必须标“需人工”。

## 必须遵守

- **铁律 1 / 2 / 9 / 10**：结论必须有证据；文档输入必须先复核；不扩范围；涉及新契约时先停。
- **ADR-0028**：写 `docs/PARKING_LOT.md` / `LEDGER.md` / `PLAN.md` / `README.md` / `plans/*` / `docs/memory/*` 前先 `guard acquire`，写完立即 release。
- **状态行收口**：提交前列出本批涉及的 `tasks/TASK-*.md`，逐张比对 `^- 状态` 与 `LEDGER.md` 最后一行。
- **只追加**：停车位原行与历史行保持逐字不变。

## 验收命令

```powershell
cargo fmt --all --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-comments
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- check-migrations
```

## 完成定义（DoD）

- [ ] 当前开放条目逐条复核，报告中结论、证据命令与结果齐全。
- [ ] `docs/PARKING_LOT.md` 仅追加复核行，原行未改。
- [ ] 所有机器验证项重跑通过；无法机器验证项明确标“需人工”。
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 与本卡同批同步。
- [ ] PR CI 11/11 SUCCESS + `MERGEABLE` + `CLEAN` + base=main；合并后回填 merge hash。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

```text
【任务】TASK-239　停车位存量复核收口（2026-10-05）
【目标】逐条复核 PARKING_LOT 仍开放或存在部分关闭余项的条目，追加机器证据结论并产出审计报告。
【write scope】docs/PARKING_LOT.md（只追加）、docs/audits/parking-lot-review-2026-10-05.md、
              本卡、LEDGER.md、PLAN.md 当前状态、plans/stage-1-pilots.md 当前进度/完成标记、
              README.md 三处、docs/automations/2026-10-05-round-1.md。
【铁律】1 无静默失败；2 文档输入先复核；9 不扩范围；10 契约先行；ADR-0028 热点文件先取 guard 锁。
【禁止】不改原停车位行；不放宽判据；不改产品代码、公共 schema、依赖、顶层目录。
【验收】cargo fmt --all --check；xtask hygiene / memory-counts / adr-index / refscan / docscan /
        card-check / check-ledger / check-comments / verify-schemas / codegen --check /
        check-migrations 全绿；PR CI 11/11 + MERGEABLE + CLEAN + base=main。
【依赖】TASK-237、TASK-238 已 Done；main 已同步 origin/main。
【疑问】自动化未给既有卡号；按治理池新建 TASK-239 作为本卡，并代表 Orchestrator 完成文档状态同步。
```

### 2. 实际改动文件（逐个核对是否在 In scope 内）

- `docs/PARKING_LOT.md`：仅在末尾追加 48 行 2026-10-05 复核结论，`git diff --unified=0` 显示原 216 行未改。
- `docs/audits/parking-lot-review-2026-10-05.md`：新增复核报告，逐条列出开放与补记闭环条目。
- `tasks/TASK-239-parking-lot-review-2026-10-05.md`：本卡正文与执行记录。
- `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` / `docs/automations/2026-10-05-round-1.md`：待同批状态同步与留痕。

### 3. 验收输出摘要（命令 → 结果，全绿 / 失败项）

- `git diff --check` → EXIT 0。
- `cargo fmt --all --check` → EXIT 0。
- `cargo run -p xtask -- hygiene` → PASSED，scanned=361，0E/105W，13/13 规则已实现。
- `cargo run -p xtask -- memory-counts` → PASSED，scanned=8，0E/0W。
- `cargo run -p xtask -- adr-index` → PASSED，scanned=59，0E/0W。
- `cargo run -p xtask -- refscan` → PASSED，scanned=689，0E/0W。
- `cargo run -p xtask -- docscan` → PASSED，scanned=307，0E/342W；首次报出的报告表格破列已修复。
- `cargo run -p xtask -- card-check` → PASSED，scanned=133，0E/34W。
- `cargo run -p xtask -- check-ledger` → PASSED；`plan_date=2026-10-05`、`ledger_last_date=2026-10-05`。
- `cargo run -p xtask -- verify-schemas` → PASSED，5/5。
- `cargo run -p xtask -- codegen --check` → PASSED，0 drift。
- `cargo run -p xtask -- check-migrations` → PASSED，5 files / 5 entries。
- `cargo test --workspace` → EXIT 0；`cargo clippy --all-targets -- -D warnings` → EXIT 0；`cargo llvm-cov --fail-under-lines 75` → TOTAL 75.03%。
- PR #229 首轮 CI run `37218660609` → 11/11 SUCCESS；状态翻转后最终 PR CI run `37219322700` → 11/11 SUCCESS；PR merge `e37ce4f`。

### 4. DoD 逐条核对

- [x] 当前开放条目逐条复核，报告中结论、证据命令与结果齐全。
- [x] `docs/PARKING_LOT.md` 仅追加复核行，原行未改。
- [x] 机器验证项已在本机重跑；无法机器验证项明确标“需人工”。
- [x] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 与本卡同批同步。
- [x] PR CI 11/11 SUCCESS + `MERGEABLE` + `CLEAN` + base=main；PR #229 已合并（`e37ce4f`）。

### 5. 偏差

自动化 prompt 未提供既有任务卡号。按预授权“自行裁决并实施到底”，在治理池新建 TASK-239 作为本轮唯一卡号；未扩写卡片正文中的既有契约，未新增产品 crate / 依赖 / 公共接口。

### 6. 实施中发现的更合理做法

停车位的“处置”列不能直接当作权威状态：`PL-019` / `PL-022` 等条目已有“已关闭”字面，但正文仍保留未完成项。本轮改为把“历史关闭行”和“当前剩余项”分开复核，新增行只写当前事实。

### 7. 遗留问题

本轮确认 35 条仍需治理或人工裁决，详见 `docs/audits/parking-lot-review-2026-10-05.md` 的开放表；另补记 13 条可确认闭环/取代的条目。后续 03:00 与 05:30 自动化若继续运行，应以此报告为输入而不是重新解释旧状态列。

### 8. 新增长期记忆

无。本轮未修改 `docs/memory/*`，因此未触发 `MEMORY.md` 规模表同步。

### 9. 给审阅者的关注点

- 确认 `docs/PARKING_LOT.md` 只有末尾追加，且 48 行都保持 5 列表格。
- 重点检查 `PL-019` / `PL-022` / `PL-035` 这三类“历史状态字面与当前剩余事实不一致”的复核结论。
- 确认报告中标记“需人工”的条目没有被伪装成机器闭环。
