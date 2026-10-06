# TASK-245　四连发自动化审计收口（LEDGER 去重 + WIP 落点 + ADR 号冲突处置）

- 状态：**Done（2026-10-06）**
- 阶段：1　子阶段：治理　批次：治理池　依赖：TASK-041（拆分 A）、TASK-244（均已 Done）
- 预估：S　难度：S
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。分界线以上为正文（Orchestrator 所有，Implementer 只读）。
- 关联：`LEDGER.md`、`docs/PARKING_LOT.md`、PR #241（TASK-042 WIP）、PR #242（Windows capture WIP）、`docs/adr/README.md`

---

## 目标（一句话）

把 2026-10-06 四连发自动化审计发现的三处账面问题就地收口：**LEDGER 重复行去重**、
**把两个 WIP 的 DRIFT / 待裁决项落进停车位**、**处置 ADR-0074 号被两个未合并 PR 同时占用**。

## 背景（为什么现在做）

四连发（00:30 / 03:00 / 05:30 / 08:00）两轮已合并、两轮按规则留了 WIP。审计另发现：

1. **LEDGER 重复**：`TASK-041 **merge hash 回填**`（`9c30ba2` / PR #239）这一行在 `LEDGER.md` 里
   **出现了 5 次、内容逐字节相同**，且被插在 2026-10-04 的行之间 —— 破坏「只追加 + 时间序」。
   根因是 00:30 那轮反复 rebase 时重复追加。`check-ledger` 只查新鲜度，检不出重复。
2. **WIP 的落点缺失**：`DRIFT-041-2`（storage sink 未接入）、`DRIFT-042-1` / `PL-110`
   （`visual_assert` 未接入 `Postcondition`）目前**只存在于未合并的 PR 分支**，main 上没有落点。
3. **ADR-0074 号冲突**：PR #241（视觉断言）与 PR #242（GDI 截图）各自占用了 **0074**；
   main 上 `docs/adr/` 现为 0073 → 0075（0074 空缺），README 下一可用号 = 0076。

## write scope

- `LEDGER.md`（**去重**：删掉 4 条重复拷贝、把该事件行挪回文件尾部；追加本卡事件行）
- `docs/PARKING_LOT.md`（**仅追加** WIP 落点行）
- `tasks/TASK-245-round4-audit-closeout.md`（本卡）
- `PLAN.md`（仅「当前状态」块）/ `README.md`（仅三处）/ `plans/stage-1-pilots.md`

## In scope

- LEDGER 去重：同一事件（TASK-041 merge-hash 回填）在文件里只保留 **1** 条，且位于 **2026-10-06 的时间段**（文件尾部）；其余 4 条逐字节相同的拷贝删除。
- `docs/PARKING_LOT.md` 追加：`DRIFT-041-2`（storage sink 待人类裁决，附 PR #242）、`DRIFT-042-1` / `PL-110`（visual_assert 接入，附 PR #241）、ADR-0074 号冲突处置（建议 #241 留 0074、#242 改 0076）。
- 状态同步 + 全套门禁绿。

## Out of scope（做了算漂移）

- **不裁决 `DRIFT-041-2` 的 storage sink 形状**（那是人类决定；本轮只登记落点）。
- **不合并 PR #242**（它明确写着「未裁决前不得合并」）；不删除任何 automation。
- 不改产品代码 / `crates/**` / `apps/**` / `xtask/**` / 公共 trait-schema / 依赖。
- 不改 2026-10-04 及更早的 LEDGER 历史行（只动本次查出的 5 条重复行里多余的 4 条）。

## 必须遵守

- **铁律 1 / 9**：去重依据必须可机器复核（逐字节相同的行）；不扩范围。
- **ADR-0028**：写 `LEDGER.md` / `docs/PARKING_LOT.md` / `PLAN.md` / `README.md` / `plans/*` 前先 `guard acquire`，写完立即 release。
- **状态行收口**：提交前列出本批 `tasks/TASK-*.md`，逐张比对状态行与 LEDGER，结果贴进 PR。
- 去重后必须复核：`TASK-041 **merge hash 回填**` 只剩 1 条，且其它历史行逐字未变。

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

- [ ] LEDGER 里 `TASK-041 **merge hash 回填**` 只剩 1 条且位于 2026-10-06 段；其余有内容行逐字未变。
- [ ] `docs/PARKING_LOT.md` 有 `DRIFT-041-2` / `DRIFT-042-1`+`PL-110` / ADR-0074 冲突的落点行（原行未改）。
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 与本卡同批同步。
- [ ] 全门禁绿；PR CI 11/11 SUCCESS + `MERGEABLE` + `CLEAN` + base=main 后合并并回填 merge hash。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

```text
【任务】TASK-245　四连发自动化审计收口（LEDGER 去重 + WIP 落点 + ADR 号冲突处置）
【目标】修 LEDGER 重复行；把两个 WIP 的 DRIFT/PL 落进停车位；登记 ADR-0074 号冲突
【write scope】仅：LEDGER.md（去重 + 追加）、docs/PARKING_LOT.md（追加）、本卡、
              PLAN.md（当前状态块）/ README.md（三处）/ plans/stage-1-pilots.md
【铁律】1 无静默失败（去重依据 = 逐字节相同的行，可机器复核）；9 不扩范围；ADR-0028 热点先 guard
【禁止】裁决 storage sink、合并 PR #242、改产品代码 / 依赖、改 2026-10-04 及更早的 LEDGER 历史行
【验收】fmt + 十一项 xtask 门禁全绿；去重后「重复行组 = 0」
【依赖】TASK-041 拆分 A 与 TASK-244 均已 Done（已核对 LEDGER）
【疑问】无
```

### 2. 实际改动文件

- `LEDGER.md`：删除 `TASK-041 **merge hash 回填**`（`9c30ba2` / PR #239）重复拷贝 **4** 条（原 L377 / L386 / L390 / L394），保留 **1** 条（原 L402，现 L398，位于 2026-10-06 段）；追加本卡事件行。文件 **404 → 400 行**。
- `docs/PARKING_LOT.md`：**仅追加 1 行** `PL-111`（`DRIFT-041-2` storage sink 待裁决 + ADR-0074 号冲突 + `DRIFT-042-1` / `PL-110` 落点）。
- `tasks/TASK-245-round4-audit-closeout.md`：本卡。
- `PLAN.md`（最新完成卡 / 下一步动作）/ `README.md`（状态块 / 当前阶段 / 最近进展）/ `plans/stage-1-pilots.md`：状态同步。

### 3. 验收输出摘要

```text
去重前：TASK-041 merge-hash 行 = 5 条（逐字节相同），其中 4 条插在 2026-10-04 行之间
去重后：= 1 条（L398，2026-10-06 段）；文件 404 → 400 行；完全重复行组 = 0

cargo fmt --all --check                                   EXIT 0
xtask hygiene / memory-counts / adr-index / refscan / docscan /
      card-check / check-ledger / check-comments / verify-schemas /
      codegen --check / check-migrations                   全 EXIT 0
```

### 4. DoD 逐条核对

- [x] LEDGER 里 `TASK-041 **merge hash 回填**` 只剩 1 条且位于 2026-10-06 段；其余有内容行逐字未变。
- [x] `docs/PARKING_LOT.md` 有 `DRIFT-041-2` / `DRIFT-042-1`+`PL-110` / ADR-0074 冲突的落点行（原行未改）。
- [x] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 同批同步。
- [x] 全门禁绿；PR CI 11/11 + MERGEABLE + CLEAN + base=main 后合并并回填（见 merge-hash 回填行）。

### 5. 偏差

无。去重只删 4 条**逐字节相同**的多余拷贝（同一事件），保留 1 条并归位到 2026-10-06 段——这是对「重复追加」这类真实损坏的修复，没有改写任何不同的历史事件。**未**裁决 storage sink、**未**合并 PR #242。

### 6. 更合理做法

`check-ledger` 只查「PLAN 日期 ≥ LEDGER 末行日期 + README 阶段名」，检不出**重复行**与**时间序错位**。建议后续给 `check-ledger`（或新护栏）加一条：**同一事件行不得逐字节重复**，且 2026-10 之后的追加必须落在文件尾部（可选 Warning 起步）——归后续护栏卡。

### 7. 遗留问题

- **`DRIFT-041-2`（`PL-111`）待人类裁决**：storage sink 形状（装配层注入 writer，还是 `ImageRef` 改内容地址 token）。PR #242 保持 WIP。
- **ADR-0074 号冲突**：PR #241 与 #242 各自占用；建议 #241 留 0074、#242 改 0076。两个 PR 均与 main 冲突，需各自 rebase。
- `DRIFT-042-1` / `PL-110`：`visual_assert` 尚未接入 `Postcondition` / `Observation`（需改公共形状）。

### 8. 新增长期记忆

无新增 FACT / PITFALL；本轮为账面修复与落点补齐，不改任何技术结论。

### 9. 给审阅者的关注点

1. LEDGER 只删了 4 条**完全相同**的多余行，保留 1 条；请用 `git diff --numstat` 确认是 4 行删除、其余历史行零改动。
2. `PL-111` 是**待裁决**项，不是闭环——不要把「开条」当「已解决」。
3. 本次**没有**动 PR #241 / #242 分支，也**没有**动 `crates/**` 产品代码。
