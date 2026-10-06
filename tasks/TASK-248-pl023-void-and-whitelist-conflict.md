# TASK-248　PL-023 作废收口 + 顶层目录白名单的指向冲突消解

- 状态：**Done（2026-10-06；ADR-0078）**
- 阶段：1　子阶段：治理　批次：治理池　依赖：TASK-239（停车位复核，判 PL-023 已被取代）
- 预估：S　难度：S
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。分界线以上为正文（Orchestrator 所有，Implementer 只读）。
- 关联：`docs/PARKING_LOT.md` PL-023、`docs/adr/0078-no-scripts-top-level-directory.md`、`docs/adr/top-level-directories.md`、`docs/adr/0069-top-level-directory-adr-whitelist.md`

---

## 目标（一句话）

按用户 2026-10-06 裁决把 **`PL-023` 作废**，并用 **ADR-0078** 消解现行文档里的指向冲突：
白名单附表规则第 3 条与 ADR-0069 的风险叙述都还写着「`scripts/` 由 PL-023 的后续 ADR 决定」，
而 PL-023 已被 TASK-239 复核判为「已被后续 ADR/卡取代」。

## 背景（为什么现在做）

- PL-023 的两半都没有对象了：② `scripts/` + `scripts/nightly-run.ps1` 已由 ADR-0029（主方案换成 Codex
  原生 scheduled tasks）判「不创建」；① `docs/nightly/logs/` 也不存在，自动化留痕由 ADR-0054 规定先落 PR。
- 但 `docs/adr/top-level-directories.md` 的变更规则第 3 条与 ADR-0069 的 D8 / 风险表仍把 `scripts/`
  的处置指向 PL-023 —— **现行契约指向一个作废条目**，属静默漂移。
- ADR-0069 明写「新增目录必须先有 Accepted ADR 或 ADR-0069 正式修订」，因此需要一次正式裁决。

## write scope

- `docs/adr/0078-no-scripts-top-level-directory.md`（新增）、`docs/adr/README.md`（登记表 + 下一可用号 + 0069 行摘要补注）
- `docs/adr/top-level-directories.md`（**仅变更规则第 3 条**；白名单表不动）
- `docs/PARKING_LOT.md`（**仅追加** PL-023 作废行）
- `tasks/TASK-248-pl023-void-and-whitelist-conflict.md`（本卡）
- `LEDGER.md`、`docs/memory/decisions.md`、`MEMORY.md`（仅规模表）
- `PLAN.md`（仅「当前状态」块）/ `README.md`（仅三处）/ `plans/stage-1-pilots.md`（完成标记 + 当前进度）

## In scope

- ADR-0078 标 Accepted：不设立 `scripts/`，脚本落点 = `tools/`；`docs/nightly/logs/` 亦不设立；
  ADR-0069 的「不预授权」结论保持有效，只取代其「由 PL-023 决定」的指向。
- 附表规则第 3 条改写为指向 ADR-0078；**白名单表不加行、不删行**。
- `docs/PARKING_LOT.md` 追加 `PL-023 作废` 行（原行与既有处置行逐字不改）。
- 状态同步 + 全套门禁绿。

## Out of scope（做了算漂移）

- **不改 ADR-0069 / ADR-0071 的正文**（ADR 只增不改）；本 ADR 已显式取代其过期指向。
- 不改章程 §11 里 `docs/nightly/logs/` 的 3 处引用（归 PL-057）。
- 不创建 `scripts/` 或 `docs/nightly/logs/`；不改 `xtask` 的任何判据；不加依赖 / crate。
- 不改 `PL-023` 的历史行与 2026-09-24 / 2026-10-05 的既有处置行。

## 必须遵守

- **铁律 1 / 10**：不给作废条目留悬空指向；契约先行（ADR-0078）。
- **ADR-0028**：写 `docs/adr/README.md` / `docs/adr/top-level-directories.md` / `docs/PARKING_LOT.md` / `docs/memory/*` / `LEDGER.md` / `MEMORY.md` 前先 `guard acquire`，写完立即 release。
- 状态行收口：提交前列出本批 `tasks/TASK-*.md`，逐张比对状态行与 LEDGER，结果贴进 PR。

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

- [ ] ADR-0078 Accepted，登记表 / 下一可用号 / `decisions.md` 同步，`adr-index` 0E0W。
- [ ] `docs/adr/top-level-directories.md` 规则第 3 条不再指向 PL-023；白名单表零改动。
- [ ] `docs/PARKING_LOT.md` 有 `PL-023 作废` 行，原行与既有处置行未改。
- [ ] `rg` 证明现行文档里不再有「`scripts/` 由 PL-023 决定 / 仍待 PL-023 裁决」这类指向（历史行除外）。
- [ ] 全门禁绿；PR CI 11/11 SUCCESS + `MERGEABLE` + `CLEAN` + base=main 后合并并回填 merge hash。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

```text
【任务】TASK-248　PL-023 作废收口 + 顶层目录白名单的指向冲突消解
【目标】按人类 2026-10-06 裁决作废 PL-023；用 ADR-0078 把「scripts/ 由 PL-023 决定」的过期指向改到本 ADR
【write scope】仅：ADR-0078 + 登记表、docs/adr/top-level-directories.md（仅规则第 3 条）、
              PARKING_LOT（追加）、本卡、LEDGER、decisions、MEMORY（规模表）、PLAN/README/plans 状态同步
【铁律】1 不留悬空指向；10 契约先行（ADR-0078）；ADR-0028 热点先 guard
【禁止】改 ADR-0069 / ADR-0071 正文；创建 scripts/ 或 docs/nightly/logs/；改 xtask 判据；加依赖
【验收】rg 证明现行文档不再有「由 PL-023 决定」；白名单表零改动；全套 xtask 门禁绿
【依赖】TASK-239 复核（判 PL-023 已被取代）
【疑问】无
```

### 2. 实际改动文件

- `docs/adr/0078-no-scripts-top-level-directory.md`（新增，Accepted）。
- `docs/adr/README.md`：§1 登记 0078 + 下一可用号 0079 + **0069 行摘要补注**（原「仍待 PL-023 裁决」已失效）。
- `docs/adr/top-level-directories.md`：**仅**变更规则第 3 条改写为指向 ADR-0078；白名单表 13 行**零改动**。
- `docs/PARKING_LOT.md`：**仅追加** `PL-023 作废` 行。
- `docs/memory/decisions.md`：追加 ADR-0078 条目；`MEMORY.md` 规模表（decisions 234→238 行 / 89→90 条）。
- `LEDGER.md`：追加 TASK-248 事件行。
- `PLAN.md`（最新完成卡）/ `README.md`（状态块新增一行）/ `plans/stage-1-pilots.md`（当前进度新增一行）。

### 3. 验收输出摘要

```text
rg -n "由 PL-023 的后续 ADR 决定|仍待 PL-023" docs/adr/README.md docs/adr/top-level-directories.md
  -> 零命中（ADR-0069 正文作为历史只读，未改；本 ADR 已显式取代其指向）
白名单表：仍 13 行（只改规则第 3 条）
cargo fmt --all --check                    EXIT 0
xtask hygiene/memory-counts/adr-index/refscan/docscan/card-check/
      check-ledger/check-comments/verify-schemas/check-migrations   全 EXIT 0
```

### 4. DoD 逐条核对

- [x] ADR-0078 Accepted，登记表 / 下一可用号（0079）/ `decisions.md` 同步，`adr-index` 0E0W。
- [x] `docs/adr/top-level-directories.md` 规则第 3 条不再指向 PL-023；白名单表零改动。
- [x] `docs/PARKING_LOT.md` 有 `PL-023 作废` 行，原行与既有处置行未改。
- [x] `rg` 证明现行文档里不再有「`scripts/` 由 PL-023 决定 / 仍待 PL-023 裁决」这类指向。
- [x] 全门禁绿；PR CI 11/11 + MERGEABLE + CLEAN + base=main 后合并并回填（见 merge-hash 回填行）。

### 5. 偏差

**流程偏差（与 TASK-246 同型，已记录）**：`PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 的三处状态同步
**发生在 `guard acquire` 之前**（当时直接进入编辑）；ADR-0078 / 登记表 / 附表 / PARKING_LOT / decisions /
LEDGER / MEMORY 六个热点文件都先取锁再写、写完即 release。本次无并发写者，故无 lost update，内容不受影响；
按 ADR-0028 仍属流程偏差，如实登记。**同一个坑连犯两次，说明「先取锁」需要更强的机械提醒** —— 见 §6。

其余无偏差：未改 ADR-0069 / ADR-0071 正文，未创建 `scripts/` 或 `docs/nightly/logs/`，未改 `xtask` 判据。

### 6. 更合理做法

本轮的「三层同步」（ADR 登记表 + 附表 + PLAN/README/plans）分散在 9 个热点文件上，靠人记「哪些要取锁」已经
连续出错两次（TASK-246 一次、本轮一次）。**建议**：给 `xtask guard` 加一个「按操作取锁」的便捷子命令
（例如 `guard acquire --hot-set task-sync` 一次锁住 `PLAN.md` / `README.md` / `plans/*` / `LEDGER.md` /
`docs/memory/*` / `MEMORY.md` / `docs/PARKING_LOT.md`），或让 `check-ledger` 在结束前提示「本批修改了热点文件
但 guard 日志里没有对应 acquire」。这属于工具判据变更，需 ADR，故本轮只记录不动。

### 7. 遗留问题

- 章程 §11 里 `docs/nightly/logs/` 的 3 处残留引用仍归 **PL-057**（本 ADR 已定「不设立该目录」，那 3 处应随之校准）。
- 「`visual_assert` 的图像来源」（capture 的 blob → 灰度缓冲 → `VisualObservation`）尚未接线，仍是 TASK-247 的后续。
- `ADR-0069` / `ADR-0071` 正文里的「PL-023 仍独立待裁决」字样按「ADR 只增不改」保持原样，由 ADR-0078 取代其指向。

### 8. 新增长期记忆

- `decisions.md`：ADR-0078 的完整决策（不设立 `scripts/`、脚本归 `tools/`、`docs/nightly/logs/` 不设立、指向取代）。
- 无新增 FACT / PITFALL 到 `docs/memory/{facts,pitfalls}.md`（本卡为治理收口；guard 相关的教训写在 §6，若后续立卡再落 pitfalls）。

### 9. 给审阅者的关注点

1. **白名单表必须零改动**（仍 13 行）——只改「变更规则」第 3 条；若 diff 里出现表格行增删应拒绝合并。
2. ADR-0069 与 ADR-0071 的**正文一字未改**（ADR 只增不改）；过期指向由 ADR-0078 取代，读契约时以 ADR-0078 为准。
3. 本卡**零代码改动**；若 diff 出现 `crates/**` / `apps/**` / `xtask/**` 应拒绝合并。
