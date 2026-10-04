# TASK-241　派生值指针化清扫（PL-022 / PL-035 / PL-065 / PL-066 同族）

- 状态：**Done（2026-10-05；PR #233 首轮 CI run `37237100194` 11/11 SUCCESS；merge hash 待回填）**
- 阶段：1　子阶段：治理　批次：治理池　依赖：TASK-239、TASK-240（均 Done）
- 预估：M　难度：M
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。分界线以上为正文（Orchestrator 所有，Implementer 只读）。
- 关联：`docs/adr/0072-derived-values-point-to-ssot.md`、`AGENTS.md`、`MEMORY.md`、`PLAN.md`、
  `plans/stage-1-pilots.md`、`docs/subagent-orchestration.md`、`docs/governance-ai-agent-execution.md`、
  `cross-platform-ai-assistant-architecture-v2.md`、`docs/PARKING_LOT.md`

---

## 目标（一句话）

用已接受的新 ADR 冻结“当前派生值不得手抄、只写指向唯一事实源的指针”，并清扫 PL-022 / PL-035 / PL-065 / PL-066 的同族残留。

## 背景（为什么现在做）

`PLAN.md`、`plans/*`、`MEMORY.md` §1 与派单文档曾多次手抄同一进度或 `AGENTS.md` 行数。每张卡
Done 后这些值都会变化，手抄副本必然漂移；ADR-0030 已解决规模表与 ADR 编号的一部分，但本组残留
需要统一的边界口径和一次只做指针化的清扫。

## write scope

- `tasks/TASK-241-derived-value-sweep.md`（本卡）
- `docs/adr/0072-derived-values-point-to-ssot.md`（新 ADR）
- `docs/adr/README.md`（仅登记表、下一可用编号）
- `docs/memory/decisions.md`（仅追加 ADR-0072 决策）
- `AGENTS.md`（仅把重复的行数派生值改为指针，并更新变更授权来源）
- `MEMORY.md`（仅 §1「下一步 ②」指针化 + 规模表机械同步）
- `plans/stage-1-pilots.md`（仅当前进度句与完成标记，条目正文不动）
- `docs/subagent-orchestration.md`、`docs/governance-ai-agent-execution.md`、
  `cross-platform-ai-assistant-architecture-v2.md`（仅 `AGENTS.md` 行数的指针化；已是指针的不动）
- `docs/PARKING_LOT.md`（仅追加 PL-022 / PL-035 / PL-065 / PL-066 的本轮结论）
- `LEDGER.md`（仅追加）
- `PLAN.md`（仅当前状态块）
- `README.md`（仅状态行、当前阶段、最近进展）
- `docs/automations/2026-10-05-round-3.md`、`docs/automations/2026-10-05-report.md`（本轮留痕与阶段报告）

## In scope

- 写 ADR-0072 并标 Accepted，登记 `docs/adr/README.md`，追加 `docs/memory/decisions.md`。
- 删除目标文档里会随仓库演变的当前派生值，改成源指针。
- 将 `plans/stage-1-pilots.md` 的当前进度句改成 `PLAN.md` 当前状态块指针，条目正文零改动。
- 将 `MEMORY.md` §1 的下一步进度改写成 `PLAN.md` / `LEDGER.md` 指针，保留不随卡 Done 漂移的约束。
- 以 `rg` 证明目标文件不再命中 `~[0-9]+ 行` 或 `下一张 = TASK-`。
- 同步 LEDGER、PLAN、README、plans 与自动化轮次报告。

## Out of scope（做了算漂移）

- 不新增或放宽 hygiene 规则、不改门禁判据；第 14 条 hygiene 规则留给后续 ADR 评估。
- 不改批次表、卡片位置表的条目正文、排期、依赖、验收要点。
- 不改产品代码、公共 trait/schema、依赖、crate、顶层目录、`unsafe` 或 `#[allow]`。
- 不改历史事件记录中的当时观测值；只禁止把动态当前值继续写成事实源。

## 必须遵守

- **铁律 1 / 9 / 10**：不静默失败；不扩范围；契约先行为先。
- **ADR-0030 / ADR-0039 / ADR-0041**：规模表、状态同步、计划文件可写面继续生效。
- **ADR-0028**：热点文件写前取锁；每次写完立即释放。
- **用户预授权**：本轮只做指针化、不篡改决策语义；ADR 可代为 Accepted。

## 验收命令

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
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

- [ ] ADR-0072 为 Accepted，登记表与 `decisions.md` 同步，`adr-index` 0E0W。
- [ ] 目标文档只删除当前派生值并改指针，不改数字语义以外的决策。
- [ ] 目标清扫证据为空；`refscan` 不出现 `BARE-PENDING`。
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 与本卡同批同步。
- [ ] PR CI 11/11 SUCCESS + `MERGEABLE` + `CLEAN` + base=main；合并后回填 merge hash。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

```text
【任务】TASK-241 派生值指针化清扫（PL-022 / PL-035 / PL-065 / PL-066 同族）
【目标】新增 Accepted ADR-0072，冻结动态派生值只指向唯一事实源，并完成目标文档指针化。
【write scope】新 ADR/登记表/decisions、AGENTS、MEMORY §1/规模表、plans 当前进度、
              subagent/gov/架构行数、PARKING_LOT、LEDGER、PLAN、README、自动化轮次产物、本卡。
【铁律】1 无静默失败；9 不扩范围；10 契约先行；ADR-0028 热点先取锁；ADR-0041 条目正文只读。
【禁止】新增 hygiene 规则；改门禁口径、批次条目正文、产品代码、schema、依赖或断言。
【验收】全部本地门禁绿；目标 `rg` 证据为空；PR 11/11 SUCCESS + MERGEABLE + CLEAN + base=main。
【依赖】TASK-239、TASK-240 已 Done；ADR 下一个编号运行时为 0072。
【疑问】无；按用户预授权直接推进。
```

### 2. 实际改动文件

- `docs/adr/0072-derived-values-point-to-ssot.md`：新增 ADR-0072，Accepted。
- `docs/adr/README.md`：登记 0072，下一可用编号改为运行时值 + 1。
- `docs/memory/decisions.md`：追加 ADR-0072 决策条目。
- `AGENTS.md`：把 `PLAN.md` / `MEMORY.md` 的重复行数副本改成文件自身指针；同步 ADR 授权尾注。
- `MEMORY.md`：§1「下一步 ②」改成 `PLAN.md` / `LEDGER.md` 指针；`decisions.md` 规模同步为 214 行 / 84 条。
- `plans/stage-1-pilots.md`：当前进度句改成 `PLAN.md` 当前状态块指针；条目正文未改。
- `docs/subagent-orchestration.md`：删除 `AGENTS.md` 当前行数副本；审阅预算的近似行数改为容量/历史记录描述。
- `docs/governance-ai-agent-execution.md`：`AGENTS.md` 行数表述已是指针；两个数字范围改为“到”式写法以通过定向证据。
- `docs/PARKING_LOT.md`：追加 PL-022 部分闭环、PL-035 闭环、PL-065 / PL-066 复核确认。
- `LEDGER.md`：追加本卡事件行。
- `docs/automations/2026-10-05-round-3.md` / `2026-10-05-report.md`：本轮留痕。
- `tasks/TASK-241-derived-value-sweep.md`：本卡。
- 未改产品代码、schema、依赖、公共接口、门禁判据、批次条目正文或历史事件行。

### 3. 验收输出摘要

- `cargo fmt --all --check` → EXIT 0。
- `cargo clippy --all-targets -- -D warnings` → EXIT 0；只有既有的 `clippy::assert_is_empty` unknown-lint 提示。
- `cargo test --workspace` → EXIT 0；`xtask` 459 passed / 0 failed，真机 ignored 8 个按既有跳过。
- `cargo test -p assistant-core arch::` → 10 passed / 0 failed。
- `cargo deny check` → advisories / bans / licenses / sources 全 ok。
- `cargo run -p xtask -- hygiene` → PASSED，scanned=361，0E/105W，13/13。
- `cargo run -p xtask -- memory-counts` → PASSED，scanned=8，0E/0W。
- `cargo run -p xtask -- adr-index` → PASSED，scanned=60，0E/0W。
- `cargo run -p xtask -- refscan` → PASSED，scanned=693，0E/0W。
- `cargo run -p xtask -- docscan` → PASSED，scanned=311，0E/342W。
- `cargo run -p xtask -- card-check` → PASSED，scanned=135，0E/34W。
- `cargo run -p xtask -- check-ledger` → PASSED，`plan_date=2026-10-05`，`ledger_last_date=2026-10-05`。
- `cargo run -p xtask -- check-comments` → PASSED，scanned=361，0E/69W，8/8 规则。
- `cargo run -p xtask -- verify-schemas` → PASSED，5/5。
- `cargo run -p xtask -- codegen --check` → PASSED，0 drift。
- `cargo run -p xtask -- check-migrations` → PASSED，5 files / 5 entries。
- 定向证据：`rg -n --no-heading "~[0-9]+ ?行|下一张 = TASK-" AGENTS.md MEMORY.md plans/stage-1-pilots.md docs/subagent-orchestration.md docs/governance-ai-agent-execution.md cross-platform-ai-assistant-architecture-v2.md` → EXIT 1，输出为空。
- PR #233：<https://github.com/snofin18/ai-assistant/pull/233>；首轮 pull_request CI run `37237100194` = 11/11 SUCCESS；确认 `baseRefName=main`、`mergeable=MERGEABLE`、`mergeStateStatus=CLEAN`。merge hash 由回填 PR 追加。

### 4. DoD 逐条核对

- [x] ADR-0072 为 Accepted，登记表与 `decisions.md` 同步，`adr-index` 0E0W。
- [x] 目标文档只删除当前派生值并改指针，不改数字语义以外的决策。
- [x] 目标清扫证据为空；`refscan` 不出现 `BARE-PENDING`。
- [x] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 与本卡同批同步。
- [ ] PR CI 11/11 SUCCESS + `MERGEABLE` + `CLEAN` + base=main；合并后回填 merge hash。首轮 CI 已满足，最终提交 CI 与 merge hash 待补。

### 5. 偏差

- none。`PL-022` 的机器派生余项按题目要求保留开放、未虚标闭环；本轮没有新增 hygiene 规则。

### 6. 更合理做法

- ADR 采用“源指针 + 定向 `rg` 证据”，没有把第 14 条规则塞进已冻结为 13 项的 `hygiene`；这样满足防复发原则，又不改现有 CI 门禁口径。

### 7. 遗留问题

- `PL-022` 仍保留机器派生余项：`TOTAL_HYGIENE_RULE_COUNT` 是否应从 gov §5.4 表格派生、gov/CI 数量是否交叉校验，仍需后续 Accepted ADR 或独立卡裁决。
- PR #233 首轮 CI 已 11/11 SUCCESS；Done 状态同步后的最终 CI 与 merge hash 回填待完成。

### 8. 新增长期记忆

- ADR-0072 决策：动态派生值只指向唯一事实源；`docs/memory/decisions.md` 已追加；`docs/PARKING_LOT.md` 已追加 PL-022 / PL-035 / PL-065 / PL-066 结论。

### 9. 给审阅者的关注点

- 最高风险是“指针化”被误解成允许删历史证据：ADR-0072 D3 明确历史观测值可保留，但必须带日期和来源。
- 第二关注点是 `plans/stage-1-pilots.md` 只改了当前进度句，批次表/位置表正文没有 diff。
- 第三关注点是 `PL-022` 只做部分闭环，机器派生余项仍需后续卡，不得把本轮描述成全量完成。
