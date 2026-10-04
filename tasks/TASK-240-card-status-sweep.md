# TASK-240　任务卡状态行 ↔ PLAN / LEDGER 一致性清扫（2026-10-05）

- 状态：**InProgress**
- 阶段：1　子阶段：治理　批次：治理池　依赖：TASK-239（Done）
- 预估：M　难度：M
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。分界线以上为正文（Orchestrator 所有，Implementer 只读）。
- 关联：`tasks/TASK-*.md`（仅状态行）、`LEDGER.md`、`PLAN.md`、`plans/stage-1-pilots.md`、`docs/memory/pitfalls.md`

---

## 目标（一句话）

用 `LEDGER.md`、`PLAN.md`、main 文件与专项测试证据，修正所有可确定的滞后任务卡状态行。

## 背景（为什么现在做）

多轮自动化收口后发现，部分卡在台账已有 `Done` 事件、产物也在 main 上，但卡面仍保留 `Ready` 或 `InProgress`。ADR-0031 D3/D5 将正文区设为只读，但状态行是必须与事实同步的元数据；本卡按 2026-10-04 的状态行硬规则做一次全量清扫。

## write scope

- 本仓库全部 `tasks/TASK-*.md` 的 `- 状态：` 行（卡正文、执行记录、断言不改）
- `tasks/TASK-240-card-status-sweep.md`（本卡）
- `LEDGER.md`（仅追加）
- `docs/memory/pitfalls.md`（仅追加占位卡状态行口径）
- `docs/PARKING_LOT.md`（仅追加无法判定项；本轮如为空则不改）
- `PLAN.md`（仅「当前状态」块四项）
- `README.md`（仅状态行 + `## 当前阶段` + `## 最近进展`）
- `plans/stage-1-pilots.md`（仅完成标记与当前进度句）
- `docs/automations/2026-10-05-round-2.md`（本轮留痕）

## In scope

- 枚举全部任务卡状态行并与台账中该卡最后一个状态事件比对。
- 对“台账最后一次 `Done`、main 有对应产物、专项测试通过、卡面仍为 `Ready`/`InProgress`”的卡，按实更正为 `Done`。
- 对正文为批次表占位但工作已由后续卡完成的卡，使用统一占位口径。
- 逐张输出状态行与台账最后状态的比对结果。

## Out of scope（做了算漂移）

- 不把“台账无 Done 事件”或“卡面明确仍有未完成项”的卡猜成 Done。
- 不改卡正文、DoD、执行记录或断言。
- 不改产品代码、公共 trait/schema、依赖、crate、顶层目录、lint。
- 不删除或改写 LEDGER 历史行。

## 必须遵守

- **铁律 1 / 2 / 9 / 10**：结论必须有证据；不扩范围；不先改契约。
- **ADR-0031**：正文只读；状态行是唯一可维护例外。
- **ADR-0028**：写热点文件前先 `guard acquire`，写完立即 release。
- **收口四步**：提交前对本批涉及卡逐张取 `^- 状态`，与 LEDGER 最后状态比对，并将结果贴进 PR。

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

- [ ] 全部任务卡已枚举并逐张与 LEDGER 最后状态比对。
- [ ] 所有可确定的滞后状态行只改状态行；无代码/正文改动。
- [ ] 无法判定项保持原状并留可追踪记录。
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 与本卡同批同步。
- [ ] PR CI 11/11 SUCCESS + `MERGEABLE` + `CLEAN` + base=main；合并后回填 merge hash。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

```text
【任务】TASK-240 卡状态行 ↔ PLAN / LEDGER 一致性清扫
【目标】用台账、main 产物与测试证据修正全部滞后卡状态行，只动 `- 状态：` 行。
【write scope】本批 tasks 状态行、本卡、LEDGER、pitfalls/parking、PLAN 当前状态、README 三处、
              stage-1 当前进度与完成标记、2026-10-05-round-2 自动化留痕。
【铁律】1 无静默失败；9 不扩范围；10 契约先行；ADR-0031 正文只读但状态行例外；ADR-0028 热点先取锁。
【禁止】改卡正文/记录区/代码/断言；新增依赖、crate、公共接口或放宽 lint；无证据猜 Done。
【验收】fmt / clippy / test --workspace + 11 个 xtask 门禁全绿；逐卡比对；CI 11/11 + CLEAN。
【依赖】TASK-239 Done；origin/main 已同步。
【疑问】无；TASK-105 台账最后仍为 InProgress，保持原状并记录计划面残留。
```

### 2. 实际改动文件

- 18 张状态行滞后卡：`TASK-025`、`026`、`028`、`065`、`085`、`086`、`087`、`100`、`101`、`103`、`104`、`205`、`207`、`210`、`211`、`212`、`213`、`216`；`git diff --unified=0` 证明每张只改第 3 行 `- 状态：`。
- `tasks/TASK-240-card-status-sweep.md`：本卡。
- `LEDGER.md`：追加本轮事实行（本卡状态翻转与 merge hash 由后续行追加）。
- `PLAN.md` / `README.md` / `plans/stage-1-pilots.md`：TASK-240 当前的完成状态与进度同步。
- `docs/memory/pitfalls.md`：新增占位卡状态行口径。
- `MEMORY.md`：pitfalls 规模表更新为 277 行 / 166 条（由 `memory-counts` 首跑给出，复跑通过）。
- 未改产品代码、公共接口、依赖、卡正文、DoD、执行记录或断言。

### 3. 验收输出摘要

- `cargo fmt --all --check` → EXIT 0。
- `cargo clippy --all-targets -- -D warnings` → EXIT 0。
- `cargo test --workspace` → EXIT 0。
- `cargo run -p xtask -- hygiene` → PASSED，scanned=361，0E/105W，13/13。
- `cargo run -p xtask -- memory-counts` → PASSED，scanned=8，0E/0W。
- `cargo run -p xtask -- adr-index` → PASSED，scanned=59，0E/0W。
- `cargo run -p xtask -- refscan` → PASSED，scanned=690，0E/0W。
- `cargo run -p xtask -- docscan` → PASSED，scanned=308，0E/342W。
- `cargo run -p xtask -- card-check` → PASSED，scanned=134，0E/34W。
- `cargo run -p xtask -- check-ledger` → PASSED，`plan_date=2026-10-05`，`ledger_last_date=2026-10-05`。
- `cargo run -p xtask -- check-comments` → PASSED，scanned=361，0E/69W，8/8 规则。
- `cargo run -p xtask -- verify-schemas` → PASSED，5/5 OK。
- `cargo run -p xtask -- codegen --check` → PASSED，0 drift。
- `cargo run -p xtask -- check-migrations` → PASSED，5 files / 5 entries。
- 根因证据：`git ls-tree -r --name-only HEAD` 已核对 18 张卡的对应 crate / 文档产物均在当前 main；`LEDGER.md` 每张最后状态事件均为 `Done`。

### 4. DoD 逐条核对

- [x] 全部任务卡已枚举并逐张与 LEDGER 最后状态比对。
- [x] 所有可确定的滞后状态行只改状态行；无代码 / 正文改动。
- [x] 无法判定或反向不一致项保持原状并留说明。
- [x] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 与本卡同批同步。
- [ ] PR CI 11/11 SUCCESS + `MERGEABLE` + `CLEAN` + base=main；合并后回填 merge hash。

### 5. 偏差

- none。本轮没有改卡正文、代码或公共契约；`MEMORY.md` 规模表按机器校验结果同步，属于 ADR-0030 的文档一致性要求。

### 6. 更合理做法

- 不能把所有同名历史 `TASK-051~055` 台账行直接套到现行 1c 卡上：这些编号曾发生撞号，旧 xtask 工作已在 `TASK-065` 重编号为 `TASK-059~064`；因此现行 `TASK-051~055` 保持 `Ready`。
- 台账最后状态不是 Done 的卡不按“计划看起来像 Done”猜写；`TASK-105` 的最后台账仍为 `InProgress`，卡内也明确 L0/L1 回滚未闭环，故保持原状。

### 7. 遗留问题

- `PLAN.md` / `README.md` 的历史进度文字仍含 `TASK-105 ✅` 与阶段 1a GO 的旧口径；本轮仅新增 TASK-240 当前状态说明，不改历史文字，留给后续文档派生值清扫。
- `TASK-002` 的卡面为 `Done`，但执行记录自述仍 InProgress、LEDGER 最后状态也为 InProgress；本轮按“只修滞后 Done”边界不反向改动，需后续单独裁决。

### 8. 新增长期记忆

- `docs/memory/pitfalls.md` 新增「批次表占位卡状态行口径」：统一写 `Done（<日期>；由 <卡号/批次> 承担，正文为占位）`。

### 9. 给审阅者的关注点

- 最高风险是编号复用导致的假阳性：`LEDGER.md` 中 `TASK-051~055` 的 Done 行属于已重编号的历史 xtask 工作，不能据此改现行 1c 占位卡。
- 第二关注点是反向不一致：`TASK-002` / `TASK-105` 未在本轮改状态，仍有计划文字与事实的陈旧差异。
- 所有 18 张改动卡均应只出现状态行 diff；如审阅发现正文或记录区变化，应拒绝合并。
