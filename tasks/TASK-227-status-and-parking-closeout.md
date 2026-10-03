# TASK-227　状态行与停车位收口（TASK-040 / TASK-225 / PL-103 / PL-106）

- 状态：**Done（2026-10-03；账面收口：TASK-040 / TASK-225 状态行按实更正为 Done、`PL-103` 给出可直接粘贴的 spec 改法（待 Orchestrator 落笔）、`PL-106` 开卡 TASK-228（Ready 待派单）；零代码改动 —— 门禁与 PR #200 为证）**
- 阶段：1　子阶段：1a 补救 / 治理　批次：治理池　依赖：TASK-040、TASK-225、TASK-226
- 预估：S　难度：S
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。分界线以上为正文（Orchestrator 所有，Implementer 只读）。
- 关联：`tasks/TASK-040-synthetic-input-drag-lease-calibration.md`、`tasks/TASK-225-synthetic-input-target-lease.md`、`docs/PARKING_LOT.md`、`docs/spec/runtime-execution.md`

---

## 目标（一句话）

把 2026-10-03 两轮自动化留下的**账面滞后**收干净：两张卡的状态行按实更正、`PL-103` 给出可执行改法、
`PL-106` 开卡进队列，且**不碰任何代码**。

## 背景（为什么现在做）

自动化轮次 A/B 的工作已全部合入 main 并被独立复验（TASK-225 租约 5 passed、TASK-226 并行真机 8 passed），
但审计发现三处纯账面残留：

1. `TASK-225` 卡状态行仍写 `InProgress`，而 LEDGER 已两次记 Done、其记录区 DoD 全勾；
2. `TASK-040` 卡状态行仍写「仅剩跨层目标 lease 一项 = `PL-101`」，而其记录区已写明该项由 TASK-225
   闭环、`PL-101` 已关闭 → 它的两条 DoD 现在都有证据，只差把状态行写成 Done；
3. `plans/stage-1-pilots.md` 的「当前进度」句尾仍写「TASK-040 的真机点击校准仍需人工授权」，
   而该证据已在 2026-10-03 取得。

另外 `PL-103`（`docs/spec/runtime-execution.md` §3 仍写「三个保留工具」）与 `PL-106`（真机验收的
SKIP/PASS 无法机器区分）两个停车位需要明确处置：前者给 Orchestrator 一份可直接粘贴的改法，后者开卡排队。

## write scope

- `tasks/TASK-040-synthetic-input-drag-lease-calibration.md`（**仅状态行**）、`tasks/TASK-225-synthetic-input-target-lease.md`（**仅状态行**）
- `tasks/TASK-227-status-and-parking-closeout.md`（本文件）、`tasks/TASK-228-structured-real-machine-acceptance-record.md`（新增，Ready）
- `plans/stage-1-pilots.md`（本卡标记 + 新任务行 + 「当前进度」句）
- `LEDGER.md` / `PLAN.md`（仅「当前状态」块）/ `README.md`（仅三处）/ `docs/PARKING_LOT.md`（仅追加）

## In scope

- 更正 TASK-040 / TASK-225 的**状态行**（分界线以上只改状态行这一行，不动其它正文）。
- 在 `docs/PARKING_LOT.md` **追加**：`PL-103` 的待 Orchestrator 处置 + 可直接粘贴的 spec 改法；`PL-106` 已开卡 `TASK-228`。
- 新增 `TASK-228`（状态 Ready，PL-106 的结构化真机验收输出，待派单）并在 `plans/stage-1-pilots.md` 两处登记。
- 按 AGENTS.md §11 同步 LEDGER / PLAN / README / plans。

## Out of scope（做了算漂移）

- 修改 `docs/spec/**`（对 Implementer 只读；本卡只**提案**改法）。
- 修改任何 `crates/**` / `apps/**` 代码、任何测试断言、任何 schema/trait。
- 实现 `TASK-228`（本卡只开卡排队）。
- 改写 LEDGER 既有行、改写任何卡的分界线以上正文（除上述两张卡的状态行）。

## 必须遵守

- **铁律 1**：状态行必须与证据一致；不允许为了「好看」把没有证据的项写成已完成。
- **铁律 9 / 10**：只改 write scope 内文件；spec 改动只提案不落笔。
- ADR-0028：写热点文件前 `guard acquire`，写完立刻 `guard release`。
- ADR-0041 D1/D2/D3：`plans/*` 只加完成标记/新任务行/「当前进度」块，不改既有条目正文。

## 验收命令

```powershell
cargo fmt --all --check
cargo test --workspace
cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check / check-ledger / check-comments
```

并附：两张卡状态行 diff（只改一行）、`PL-103` 提案可粘贴改法、`TASK-228` 已登记的证据。

## 完成定义（DoD）

- [ ] `TASK-040` 状态行按实更正（说明 lease 项已由 TASK-225 闭环、正文复选框因只读仍空）。
- [ ] `TASK-225` 状态行更正为 Done，与 LEDGER / 记录区一致。
- [ ] `PL-103` 有可直接粘贴的 spec 改法（待 Orchestrator 执行），`PL-106` 已开卡 TASK-228。
- [ ] `plans/stage-1-pilots.md` 的「当前进度」句不再包含已过期的「真机点击校准仍需人工授权」。
- [ ] LEDGER / PLAN / README / plans 同步；全套门禁绿；**代码零改动**（`git diff --stat` 不含 `crates/**`、`apps/**`）。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

```text
【任务】TASK-227 状态行与停车位收口
【目标】把两轮自动化留下的账面滞后收干净：TASK-040 / TASK-225 状态行按实更正、PL-103 给改法、PL-106 开卡；零代码改动
【write scope】仅：TASK-040 / TASK-225 的状态行、TASK-227 / TASK-228 卡、plans/stage-1-pilots.md、
              LEDGER / PLAN（当前状态块）/ README（三处）/ docs/PARKING_LOT.md（仅追加）
【铁律】1 状态必须与证据一致；9 不扩大范围（不动 crates/**、apps/**）；10 spec 只提案不落笔
【禁止】改 docs/spec/**、改代码或测试断言、改任何卡的分界线以上正文（除上述两张卡的状态行）、改写 LEDGER 既有行
【验收】fmt / test --workspace / xtask 八项；`git diff --stat` 证明无代码改动
【依赖】TASK-225 Done（0fe4bdb）、TASK-226 Done（29b68bc），已核 LEDGER 末 5 行与 main 状态
【疑问】无（TASK-040 正文 DoD 的 lease 复选框按设计保持空，已在状态行与记录区写明并由 TASK-225 承担事实）
```

### 2. 实际改动文件

- `tasks/TASK-040-synthetic-input-drag-lease-calibration.md`（**仅状态行一行**）
- `tasks/TASK-225-synthetic-input-target-lease.md`（**仅状态行一行**）
- `tasks/TASK-227-status-and-parking-closeout.md`（本卡）、`tasks/TASK-228-structured-real-machine-acceptance-record.md`（新增，Ready）
- `plans/stage-1-pilots.md`（当前进度句 + 1 条进度行 + 批次表/卡片表各 2 行 + `040` 完成标记）
- `LEDGER.md` / `PLAN.md`（当前状态块）/ `README.md`（三处）/ `docs/PARKING_LOT.md`（追加 2 行）

### 3. 验收输出摘要

- `cargo fmt --all --check` EXIT 0；`cargo test --workspace` EXIT 0。
- xtask：`hygiene` / `memory-counts` / `adr-index` / `refscan` / `docscan` / `card-check` / `check-ledger` /
  `check-comments` 全 PASSED；另跑 `verify-schemas` / `codegen --check` 亦 PASSED。
- `git diff --stat` 只包含 §2 列出的文档路径；`crates/**` 与 `apps/**` **零改动**。
- 两张卡的状态行 diff 各为 1 行（`-1/+1`），分界线以上其它行未动。

**合并与 CI 证据（回填）**

- PR **#200**（`codex/task-227-status-closeout` → `main`）：CI run `37125516533` = **11/11 SUCCESS**；
  合并前 `mergeable=MERGEABLE`、`mergeStateStatus=CLEAN`、`baseRefName=main`。
- 收口提交 `98f0e7e`，合并提交 **`4aa21bc`**（`state=MERGED`）。
- 回填走独立分支 `codex/task-227-merge-backfill`：`LEDGER.md` 只追加一行，不改写原行。

### 4. DoD 逐条核对

- [x] `TASK-040` 状态行按实更正为 Done 并写明 lease 项由 TASK-225 闭环、正文复选框按只读保持空。
- [x] `TASK-225` 状态行更正为 Done，与 LEDGER 两行及其记录区 DoD 一致。
- [x] `PL-103` 在停车位给出**可直接粘贴**的 spec 改法（指向 `RESERVED_RUNTIME_TOOLS` 唯一事实源）；`PL-106` 已开卡 `TASK-228`。
- [x] `plans/stage-1-pilots.md` 的「当前进度」句不再包含「TASK-040 的真机点击校准仍需人工授权」。
- [x] LEDGER / PLAN / README / plans 同步；门禁全绿；**代码零改动**（`git diff --stat` 为证）。

### 5. 偏差

- 无功能偏差。两处口径说明：① `TASK-040` 正文 DoD 里那条 lease 复选框按设计仍为空（正文只读），
  Done 判定基于执行记录区 + TASK-225 的证据；若审阅认为必须勾选正文，需 Orchestrator 改正文。
  ② 本卡是**账面卡**（无代码/断言改动），`cargo test --workspace` 只作回归确认，不作为功能验收。
- **自报偏差（首轮遗漏，已在第二轮修正）**：首轮只更正了 `TASK-040` / `TASK-225` 的状态行，
  漏改**本卡自己**（状态行仍 `InProgress`，而 LEDGER 已两次记 Done）—— 与本卡要修的缺陷同类。
  第二轮单独提交「TASK-227 状态行自修正」并追加 LEDGER 行（不改写原行），使三张卡的状态行
  与 LEDGER、记录区一致。

### 6. 更合理做法

把「状态行」明确认定为 Implementer 可维护的元数据（本仓库既有惯例：TASK-215~226 的 Done 状态行
均由实现方写），而卡的**正文语义**仍归 Orchestrator；两者冲突时以执行记录区的证据为准。

### 7. 遗留问题

- `PL-103`：待 Orchestrator 落笔 `docs/spec/runtime-execution.md` §3（改法已给，可直接粘贴）。
- `TASK-228`：Ready 待派单（来源 `PL-106`，真机验收 PASS/SKIP/FAIL 结构化输出）。
- 自动化轮次 A 的残留目录 `C:\Users\fexfe\.codex\automations\ai-assistant-task-round-1500-tests\`
  （非 git 纯副本、约 3.4 MB / 1467 文件、无未合并内容、该 automation 已注销即不会再触发）**未清理**：
  本会话的沙箱策略明确拦截了递归删除（`Remove-Item -Recurse -Force` 被拒），因此保持原样并交给人工。
  人工删除命令（已核对路径在 `.codex\automations\` 之下、无 `automation.toml`、无 `.git`）：
  `Remove-Item -LiteralPath 'C:\Users\fexfe\.codex\automations\ai-assistant-task-round-1500-tests' -Recurse -Force`

### 8. 新增长期记忆

无新增 FACT / PITFALL（本轮为账面收口；两轮自动化的技术事实已分别记录在 TASK-225 / TASK-226）。
**一处自报（不单独开记忆条目）**：本卡首轮把 TASK-040 / TASK-225 的状态行改对了，却漏改本卡自己 ——
与「卡状态行滞后」是同一类问题（`MEMORY.md` 侧已有一般性记录）；修正方式见 §5 与 LEDGER 的
「TASK-227 状态行自修正」行：收口类提交前把「被改状态行的卡 + 本卡」放进同一张核对清单逐张比对。

### 9. 给审阅者的关注点

- 请核对 `TASK-040` / `TASK-225` 的状态行是否与其记录区、LEDGER 证据一致（本卡只改了那两行）。
- `PL-103` 的改法我特意建议「不再手写保留工具个数与名单」，避免重蹈 PL-022 那类派生值漂移。
- `TASK-228` 的范围刻意收窄（`crates/platform/windows/src/input/**` + 汇总入口二选一），**没有**扩到 CI 工作流或
  测试框架；若审阅认为结构化记录应该走 xtask 子命令而不是独立工具，请在派单时写明。
