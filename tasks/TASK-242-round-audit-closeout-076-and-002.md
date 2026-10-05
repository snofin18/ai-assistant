# TASK-242　三连发自动化审计收口（TASK-076 状态行 + TASK-002 落点）

- 状态：**Done（2026-10-05）**
- 阶段：1　子阶段：治理　批次：治理池　依赖：TASK-239、TASK-240、TASK-241（均已 Done）
- 预估：S　难度：S
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。分界线以上为正文（Orchestrator 所有，Implementer 只读）。
- 关联：`tasks/TASK-076-b1-5-probe-08-failure-injection.md`、`tasks/TASK-002-spike-a-notepad-uia.md`、`docs/PARKING_LOT.md`、`LEDGER.md` L88、`PLAN.md` 阻塞项

---

## 目标（一句话）

把 2026-10-05 三连发（TASK-239 / 240 / 241）审计发现的两处残余就地收口：**补正 TASK-076 的
滞后状态行**，并**给 TASK-002 的三方状态不一致开一条停车位待裁决**。

## 背景（为什么现在做）

独立复核三连发产物时发现两处：

1. **TASK-076 状态行漏改（真实缺陷）**：`LEDGER.md` L88（2026-09-22）明写「收尾 TASK-076 …
   **状态 InProgress → Done** + §2-9 全填」，卡内也标「9 节执行记录填写完毕（… TASK-084 v2 收尾 = 2026-09-22）」，
   但 commit `0f05ef7` **实际没改状态行**（`git show 0f05ef7 -- tasks/TASK-076-*.md` 无状态行 diff），
   卡面至今仍为 `InProgress`。TASK-240 的清扫按「台账行的卡号主语」归属，漏掉了这条挂在
   **以 TASK-084 为主语**的台账行里的 Done 证据。
2. **TASK-002 三方不一致未登记**：卡面 `Done`（2026-09-25 wf1 收回时把状态行改过），但 `PLAN.md`
   阻塞项仍写「TASK-002 仍 Blocked」，`LEDGER.md` 无 TASK-002 的 Done 事件。TASK-240 在自己的
   记录区 §7 提了一句，但**没有按规则落到 `docs/PARKING_LOT.md`**（未决项必须有落点）。

## write scope

- `tasks/TASK-076-b1-5-probe-08-failure-injection.md`（**仅状态行**）
- `docs/PARKING_LOT.md`（**仅追加** PL-109 行）
- `tasks/TASK-242-round-audit-closeout-076-and-002.md`（本卡）
- `LEDGER.md` / `PLAN.md`（仅「当前状态」块）/ `README.md`（仅三处）/ `plans/stage-1-pilots.md`

## In scope

- TASK-076 状态行按实改为 `Done（2026-09-22；TASK-084 v2 收尾，LEDGER L88）`；卡正文其余部分与执行记录不动。
- `docs/PARKING_LOT.md` 追加 **PL-109（新提）**：记录 TASK-002 卡面 / PLAN / LEDGER 三方不一致，附证据与「需裁决」。
- 状态同步 + 全套门禁绿。

## Out of scope（做了算漂移）

- 不裁决 TASK-002 到底该是 Done 还是 Blocked（那是 PL-109 交人类的决定），本轮**不改** TASK-002 卡面与 `PLAN.md` 阻塞项。
- 不改任何产品代码、`crates/**`、`apps/**`、`xtask/**`、公共 trait/schema、依赖、crate、顶层目录、lint。
- 不重跑或改写 TASK-239 / 240 / 241 的正文与执行记录。
- 不改 `LEDGER.md` 历史行。

## 必须遵守

- **铁律 1 / 9**：结论必须有可机器验证的证据（`git show 0f05ef7` 无状态行 diff + LEDGER L88）；不扩范围。
- **ADR-0031**：TASK-076 正文只读，**只允许改状态行**。
- **ADR-0028**：写 `docs/PARKING_LOT.md` / `LEDGER.md` / `PLAN.md` / `README.md` / `plans/*` 前先 `guard acquire`，写完立即 release。
- **状态行收口**：提交前列出本批 `tasks/TASK-*.md`，逐张比对状态行与 LEDGER，结果贴进 PR。

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

- [ ] TASK-076 状态行为 `Done`（附 2026-09-22 与 TASK-084 v2 依据），正文与执行记录零改动。
- [ ] `docs/PARKING_LOT.md` 有 PL-109 行（原行未改），写明三方不一致与需裁决。
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 与本卡同批同步。
- [ ] 全门禁绿；PR CI 11/11 SUCCESS + `MERGEABLE` + `CLEAN` + base=main 后合并并回填 merge hash。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

```text
【任务】TASK-242　三连发自动化审计收口（TASK-076 状态行 + TASK-002 落点）
【目标】补正 TASK-076 的滞后状态行；把 TASK-002 三方不一致开成停车位待裁决
【write scope】仅：tasks/TASK-076-*.md（仅状态行）、docs/PARKING_LOT.md（追加 PL-109）、本卡、
              LEDGER / PLAN（状态块）/ README（三处）/ plans
【铁律】1 无静默失败（证据可机器验证：git show 0f05ef7 无状态行 diff + LEDGER L88）；9 不扩范围；ADR-0031 正文只读；ADR-0028 热点先 guard
【禁止】裁决 TASK-002 本身（Done/Blocked 是 PL-109 交人类的决定）；改产品代码 / schema / 依赖 / 历史台账行
【验收】fmt + 十一项 xtask 门禁 → 全绿
【依赖】TASK-239 / 240 / 241 均已 Done（已核对 LEDGER）
【疑问】无
```

### 2. 实际改动文件

- `tasks/TASK-076-b1-5-probe-08-failure-injection.md`：**仅状态行** `InProgress` → `Done（2026-09-22；TASK-084 v2 收尾，LEDGER L88；probe-08 v2 数据已填入 §2-9）`。
- `docs/PARKING_LOT.md`：**仅追加** 1 行 `PL-109（新提）`（TASK-002 三方不一致，待裁决），原行未改。
- `tasks/TASK-242-round-audit-closeout-076-and-002.md`：本卡。
- `LEDGER.md` / `PLAN.md`（当前状态块）/ `README.md`（三处）/ `plans/stage-1-pilots.md`：状态同步。

### 3. 验收输出摘要

```text
cargo fmt --all --check                                   EXIT 0
xtask hygiene          scanned=361, 0E/105W, PASSED；deferred-rules 13/13
xtask memory-counts    EXIT 0（0E/0W）
xtask adr-index        EXIT 0（scanned=60, 0E/0W）
xtask refscan          EXIT 0（0E/0W）
xtask docscan / card-check / check-ledger / check-comments /
      verify-schemas / codegen --check / check-migrations   全 EXIT 0

根因证据：git show 0f05ef7 -- tasks/TASK-076-b1-5-probe-08-failure-injection.md
          → 只见执行记录追加，**无状态行 diff**（LEDGER L88 声称的
          「状态 InProgress → Done」从未落盘）。
```

### 4. DoD 逐条核对

- [x] TASK-076 状态行为 `Done`（附 2026-09-22 与 TASK-084 v2 依据），正文与执行记录零改动。
- [x] `docs/PARKING_LOT.md` 有 PL-109 行（原行未改），写明三方不一致与需裁决。
- [x] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 同批同步。
- [x] 全门禁绿；PR CI 11/11 + MERGEABLE + CLEAN + base=main 后合并并回填（见 merge-hash 回填行）。

### 5. 偏差

无。两处都只动状态行 / 追加停车位行；未裁决 TASK-002、未改产品代码或历史台账。`git show 0f05ef7 -- tasks/TASK-076-b1-5-probe-08-failure-injection.md` 的 diff **只见执行记录追加、不含状态行**，即「LEDGER 声称改了状态行但实际没改」的直接证据。

### 6. 更合理做法

停车位复核（TASK-239）与状态行清扫（TASK-240）应共用同一份「卡号 → LEDGER 末状态」索引，且**按台账行里出现的每个卡号**归属（而不是只取行首主语）——TASK-076 的 Done 证据恰好挂在以 TASK-084 为主语的台账行里，才被漏掉。该改进留给后续护栏卡。

### 7. 遗留问题

- **PL-109**（TASK-002 三方不一致）仍待人类裁决：以卡面 `Done` 为准同步 `PLAN.md` 阻塞项并补 LEDGER 说明，还是以 `Blocked` 为准回退卡面。
- `PL-022`（`TOTAL_HYGIENE_RULE_COUNT` 是否从 gov §5.4 派生）等机器派生余项仍开放（TASK-241 已如实保留）。

### 8. 新增长期记忆

无新增 FACT / PITFALL。本轮的两条结论（TASK-076 漏改、TASK-002 三方不一致）已分别落在卡记录与 PL-109，不需另开记忆条目；`MEMORY.md` 规模表不因本卡变化。

### 9. 给审阅者的关注点

1. TASK-076 只应出现状态行 1 行 diff；若正文或执行记录有变化应拒绝合并。
2. `docs/PARKING_LOT.md` 必须只追加 1 行、原 267 行逐字未变。
3. PL-109 是**待裁决**项，不是闭环——审阅时不要把「开条」当「已解决」。
