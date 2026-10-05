# TASK-243　PL-109 裁决收口：TASK-002 = Done，清除各处不一致

- 状态：**Done（2026-10-05）**
- 阶段：1　子阶段：治理　批次：治理池　依赖：TASK-242（Done）
- 预估：S　难度：S
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。分界线以上为正文（Orchestrator 所有，Implementer 只读）。
- 关联：`docs/PARKING_LOT.md` PL-109、`tasks/TASK-002-spike-a-notepad-uia.md`、`PLAN.md` 阻塞项、`MEMORY.md` §1、`LEDGER.md`

---

## 目标（一句话）

按人类 2026-10-05 裁决 **TASK-002 = Done**，把「活的事实源」（卡面 / `PLAN.md` 阻塞项 /
`MEMORY.md` §1 / `LEDGER.md` / `docs/PARKING_LOT.md`）全部协调一致，并给两份日期化历史快照
加上前向标注，确保不再有「当前状态」层面的不一致。

## 背景（为什么现在做）

`PL-109` 记录了三方不一致：卡面 `Done` / `PLAN.md` 阻塞项 `Blocked` / `LEDGER.md` 无 Done 事件。
人类 2026-10-05 裁决：**TASK-002 就是 Done**（SPIKE-A 已归档 GO，B1.1~B1.7 与最终 go/no-go 均已落
`docs/spike-reports/SPIKE-A.md` §11~§15，main 上 `tasks/TASK-002` 状态行亦已是 Done）。本轮把其余落点补齐。

## write scope

- `PLAN.md`（**仅「当前状态」块**：阻塞项 + 更新日期）
- `MEMORY.md`（**仅 §1「下一步 ③」**）
- `docs/PARKING_LOT.md`（**仅追加** PL-109 闭环行）
- `LEDGER.md`（追加裁决事件行）
- `docs/spike-reports/SPIKE-A.md`（**仅顶部 banner 追加一行 `[supersedes:2026-10-05]`**，不改历史节）
- `docs/audits/stage-1a-reaudit-checklist-2026-09-30.md`（**仅该表行追加前向标注**）
- `tasks/TASK-243-pl109-adjudication-and-consistency.md`（本卡）
- `README.md`（三处）/ `plans/stage-1-pilots.md`

## In scope

- `PLAN.md` 阻塞项删去「TASK-002 仍 Blocked」，只保留真实剩余阻塞（gov #9 / #11 SOFT 门禁）。
- `MEMORY.md` §1「下一步 ③」改写为 TASK-002 = Done 的指针表述。
- `LEDGER.md` 追加裁决事件行；`docs/PARKING_LOT.md` 追加 `PL-109 已闭环`（原行不改）。
- SPIKE-A banner 与 2026-09-30 审计快照各加一行「以 2026-10-05 裁决为准」的前向标注（历史正文不动）。

## Out of scope（做了算漂移）

- **不改写任何历史记录**：旧 `LEDGER.md` 行、`SPIKE-A.md` §11~§15 正文、旧任务卡执行记录、日期化审计正文一律只读（ADR-0051 / ADR-0052）。
- 不改产品代码 / 公共 trait-schema / 依赖 / crate / 顶层目录 / lint。
- 不改 TASK-002 卡的状态行（已是 Done）与正文。
- 不重开 SPIKE-A 的技术结论（那是 `PL-079`/`TASK-079` 的既有裁决）。

## 必须遵守

- **铁律 1 / 9**：每处改动都要能指向权威证据（卡面 Done + SPIKE-A GO + 本裁决）；不扩范围。
- **ADR-0039 / ADR-0041**：`PLAN.md` 只改「当前状态」块；`plans/*` 只动进度句与完成标记。
- **ADR-0028**：写 `PLAN.md` / `MEMORY.md` / `LEDGER.md` / `README.md` / `docs/PARKING_LOT.md` / `plans/*` 前先 `guard acquire`，写完立即 release。
- **状态行收口**：提交前列出本批涉及的 `tasks/TASK-*.md`，逐张比对状态行与 LEDGER，结果贴进 PR。

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

- [ ] `rg` 证明活的事实源里不再有把 TASK-002 描述为 Blocked / InProgress 的当前状态表述（历史行除外，且已前向标注）。
- [ ] `PL-109` 在 `docs/PARKING_LOT.md` 标为已闭环（原行未改）。
- [ ] `LEDGER.md` 有裁决事件行；`PLAN.md` 阻塞项与 `MEMORY.md` §1 与卡面一致。
- [ ] 全门禁绿；PR CI 11/11 SUCCESS + `MERGEABLE` + `CLEAN` + base=main 后合并并回填 merge hash。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

```text
【任务】TASK-243　PL-109 裁决收口：TASK-002 = Done，清除各处不一致
【目标】按人类 2026-10-05 裁决，把 TASK-002 的活事实源统一为 Done，并给日期化历史快照加前向标注
【write scope】仅：PLAN.md（当前状态块）、MEMORY.md（§1）、docs/PARKING_LOT.md（追加）、LEDGER.md（追加）、
              docs/spike-reports/SPIKE-A.md（仅 banner）、docs/audits/stage-1a-reaudit-checklist-2026-09-30.md（前向标注）、
              本卡、README.md（三处）、plans/stage-1-pilots.md
【铁律】1 无静默失败；9 不扩范围；ADR-0051/0052 历史记录只读；ADR-0028 热点先 guard
【禁止】改写历史行 / SPIKE-A 章节 / 旧卡执行记录；改产品代码 / schema / 依赖；重开 SPIKE-A 技术结论
【验收】fmt + 十一项 xtask 门禁全绿；rg 证明活事实源不再把 TASK-002 述为当前 Blocked/InProgress
【依赖】TASK-242 Done（PL-109 由它开出）；人类 2026-10-05 裁决
【疑问】无
```

### 2. 实际改动文件

- `PLAN.md`：阻塞项删去「TASK-002 仍 Blocked」（只余 gov #9 / #11 SOFT）；当前任务卡 / 最新完成卡 / 下一步动作加入 TASK-243。
- `MEMORY.md`：§1「下一步 ③」改写为「TASK-002 已于 2026-10-05 裁决为 Done（SPIKE-A GO，§11~§15 在 main；PL-109 闭环）」。
- `docs/PARKING_LOT.md`：**仅追加** 1 行 `PL-109 已闭环`（原行未改）。
- `LEDGER.md`：追加裁决事件行（TASK-243）。
- `docs/spike-reports/SPIKE-A.md`：**仅顶部 banner** 追加一行 `[supersedes:2026-10-05]`（历史节不改）。
- `docs/audits/stage-1a-reaudit-checklist-2026-09-30.md`：在快照表后追加一行 `[supersedes:2026-10-05]` 前向标注。
- `README.md`（状态块 / 当前阶段 / 最近进展）/ `plans/stage-1-pilots.md`（当前进度 + 卡片位置行）；本卡。

### 3. 验收输出摘要

```text
cargo fmt --all --check                                   EXIT 0
xtask hygiene / memory-counts / adr-index / refscan / docscan /
      card-check / check-ledger / check-comments / verify-schemas /
      check-migrations                                   全 EXIT 0

定向证据（活事实源）：
  PLAN.md 阻塞项           -> 不再含「TASK-002 仍 Blocked」
  MEMORY.md §1「下一步 ③」 -> 「TASK-002 已于 2026-10-05 裁决为 Done」

剩余命中均为历史快照 / 修复描述：
  LEDGER 旧行（L29/L68/L69/L159）—— 只追加，不改写；
  SPIKE-A §11~§15 内「TASK-002 仍 InProgress」—— 顶部 banner 已加前向标注；
  2026-09-30 审计快照 —— 表后已加前向标注。
```

### 4. DoD 逐条核对

- [x] 活的事实源里不再把 TASK-002 述为当前 Blocked / InProgress（历史行已前向标注）。
- [x] `PL-109` 在 `docs/PARKING_LOT.md` 标为已闭环，原行未改。
- [x] `LEDGER.md` 有裁决事件行；`PLAN.md` 阻塞项与 `MEMORY.md` §1 与卡面（Done）一致。
- [x] 全门禁绿；PR CI 11/11 + MERGEABLE + CLEAN + base=main 后合并并回填（见 merge-hash 回填行）。

### 5. 偏差

无。历史记录（旧 `LEDGER.md` 行、`SPIKE-A.md` §11~§15、旧任务卡执行记录、2026-09-30 审计正文）一律**只读**，只在其上方/表后加前向标注——这是 ADR-0051 / ADR-0052「历史记录不改写」的既定做法，不是遗漏。

### 6. 更合理做法

无。裁决本身只需要把「活事实源」对齐；把日期化快照原样保留并在顶部指向当前权威，比逐处改写快照更可审计。

### 7. 遗留问题

- `PL-022`（`TOTAL_HYGIENE_RULE_COUNT` 是否从 gov §5.4 派生）等机器派生余项仍开放。
- `TASK-105` 的台账最后状态仍为 InProgress（与 `TASK-002` 不同：它确有未闭环的 L0/L1 回滚项），保持原状。

### 8. 新增长期记忆

无新增 FACT / PITFALL；本轮为裁决落地与一致性收口，不改变任何技术结论。`MEMORY.md` 只改了 §1 的「下一步 ③」表述。

### 9. 给审阅者的关注点

1. `SPIKE-A.md` 只加了一行 banner 标注；§11~§15 正文一字未改（历史证据保留）。
2. `PLAN.md` 阻塞项现在只余 gov #9 / #11 两条 SOFT 门禁。
3. 若审阅发现 `docs/PARKING_LOT.md` 有删除或改写原行，应拒绝合并。
