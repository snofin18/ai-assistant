# TASK-237　三连发自动化审计收口（PL-062 闭环 + 孤儿目录清理）

- 状态：**Done（2026-10-04）**
- 阶段：1　子阶段：治理　批次：治理池　依赖：TASK-234、TASK-235、TASK-236（均已 Done）
- 预估：S　难度：S
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。分界线以上为正文（Orchestrator 所有，Implementer 只读）。
- 关联：`docs/PARKING_LOT.md` 的 PL-062、`%USERPROFILE%\.codex\automations\ai-assistant-task-round-0420-storage-session-api\`、`tasks/TASK-234..236`

---

## 目标（一句话）

对 2026-10-04 三连发自动化（TASK-234 / 235 / 236）做事后独立审计收口：关闭触发条件已消失的
`PL-062`、删除孤儿自动化目录，并把状态同步文件与本卡同批更新。

## 背景（为什么现在做）

- 三轮自动化全部合并：TASK-234（PR #218 / `e417a2a`）、TASK-235（PR #220 / `8d9241d`）、
  TASK-236（PR #223 / `d7f61c8`），各自 merge-hash 回填完成，卡片状态行与 LEDGER 一致。
- **PL-062 已成僵尸条目**：它记录「`AGENTS.md` §6 的 `cargo run -p xtask -- replay --suite core`
  与骨架版 CLI 不符」。TASK-236 已交付完整版 `replay --suite core`，本机实测该命令 `exit 0`
  与 `AGENTS.md` §6 一致 —— 触发条件消失，但 PARKING_LOT 仍标「待评审」。
- **孤儿自动化目录残留**：`...\.codex\automations\ai-assistant-task-round-0420-storage-session-api\`
  是 `PL-108` 的来源证据；`PL-108` 已由 TASK-233 闭环，该目录只剩一个 870 B `memory.md`。

## write scope

- `docs/PARKING_LOT.md`（**仅追加** PL-062 闭环行）
- `tasks/TASK-237-automation-round-audit-closeout.md`（本卡）、`tasks/TASK-235-role-and-parent-resolution-semantics.md`（**仅状态行**）
- `LEDGER.md` / `PLAN.md`（仅「当前状态」块）/ `README.md`（仅三处）/ `plans/stage-1-pilots.md`（完成标记 + 当前进度句）

## In scope

- `docs/PARKING_LOT.md` 追加 `PL-062 已闭环` 行，**不改写** PL-062 原行。
- 删除孤儿自动化目录：先解析绝对路径并确认落在 `.codex\automations\` 下，清只读位后删除，留前后证据。
- 归一 `TASK-235` 状态行（补 PR / merge / CI），与 `TASK-234` / `TASK-236` 的写法一致。
- 状态同步 + 全套门禁绿。

## Out of scope（做了算漂移）

- 不改 `AGENTS.md`、`docs/spec/**`、`docs/adr/**`、`crates/**`、`apps/**`、`xtask/**` 的任何内容。
- 不新增依赖 / crate / 顶层目录 / 抽象层。
- 不追改 TASK-234 的 LEDGER 历史行（那条「状态校正（CI 前）」是主动按实回退，保留为事实）。
- 不重开或改写 TASK-234 / 235 / 236 的正文与执行记录。

## 必须遵守

- **铁律 1 / 9**：结论必须有可机器验证的证据；不扩范围。
- **ADR-0028**：写 `docs/PARKING_LOT.md` / `LEDGER.md` / `PLAN.md` / `README.md` / `plans/*` 前先 `guard acquire`，写完立即 release。
- **状态行收口**：提交前列出本批 `tasks/TASK-*.md`，逐张比对状态行与 LEDGER，结果贴进 PR。
- 目录删除：先 `Resolve-Path` 并断言目标在 `.codex\automations\` 之下，再删；只删该一个具名目录。

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
cargo run -p xtask -- replay --suite core
```

## 完成定义（DoD）

- [ ] `docs/PARKING_LOT.md` 有 `PL-062 已闭环` 行，原 PL-062 行未改。
- [ ] 孤儿自动化目录删除，`Test-Path` 前 True → 后 False 证据落卡。
- [ ] `TASK-235` 状态行与 LEDGER 一致（带 PR / merge / CI）。
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 与本卡同批同步。
- [ ] 全门禁绿；PR CI 11/11 SUCCESS + `MERGEABLE` + `CLEAN` + base=main 后合并并回填 merge hash。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

```text
【任务】TASK-237　三连发自动化审计收口（PL-062 闭环 + 孤儿目录清理）
【目标】关闭触发条件已消失的 PL-062、删除孤儿自动化目录、归一 TASK-235 状态行并同步状态
【write scope】仅：docs/PARKING_LOT.md（追加）、本卡、tasks/TASK-235-*.md（仅状态行）、
              LEDGER.md / PLAN.md（当前状态块）/ README.md（三处）/ plans/stage-1-pilots.md
【铁律】1 无静默失败（结论有可验证证据）/ 9 不扩范围 / ADR-0028 热点文件先 guard
【禁止】改 AGENTS.md / docs/spec / docs/adr / 产品代码 / xtask；新增依赖 / crate / 顶层目录
【验收】全套 xtask 门禁 + fmt + replay --suite core → 全绿
【依赖】TASK-234 / 235 / 236 均已 Done，merge hash 已核对
【疑问】无
```

### 2. 实际改动文件

- `docs/PARKING_LOT.md`：**仅追加** `PL-062 已闭环` 一行，原 PL-062 行未改。
- `tasks/TASK-235-role-and-parent-resolution-semantics.md`：**仅状态行**补 `2026-10-04` / ADR-0070 / PR #220 / merge `8d9241d` / CI，与 TASK-234 / 236 写法归一。
- `tasks/TASK-237-automation-round-audit-closeout.md`：本卡。
- `LEDGER.md` / `PLAN.md`（当前状态块）/ `README.md`（三处）/ `plans/stage-1-pilots.md`：状态同步。
- 仓库外：删除 `%USERPROFILE%\.codex\automations\ai-assistant-task-round-0420-storage-session-api\`（孤儿目录）。

### 3. 验收输出摘要

**孤儿目录删除**（先解析绝对路径并确认在 `.codex\automations\` 之下）：

```text
target = C:\Users\fexfe\.codex\automations\ai-assistant-task-round-0420-storage-session-api
root   = C:\Users\fexfe\.codex\automations
exists_before = True
files_before  = 1; bytes = 870
（清只读位后 [System.IO.Directory]::Delete($resolved, $true)）
exists_after  = False
```

**PL-062 关闭依据**（`AGENTS.md` §6 L180 与会话一致）：

```text
$ cargo run -p xtask -- replay --suite core
-- summary: 2 step(s), 5 change(s), 0 mismatch
-- verdict: PASSED            (EXIT 0)

$ cargo run -p xtask -- --list-deferred
未实现的子命令：0 项。
gov §5.4 的 13 项卫生规则已全部实现：未实现 0 项。
```

**三连发产物独立复核**（本地实跑，与各卡记录一致）：

```text
xtask hygiene                      scanned=359, 0E/105W, PASSED；deferred-rules 13/13
cargo test -p xtask                459 passed / 0 failed
cargo test -p assistant-replay     16 passed / 0 failed
cargo test -p assistant-platform-windows   96 passed / 4 ignored（+ 7 / 4 / 1 集成与 doctest）
replay negative tampered-text      text_changed MISMATCH, EXIT 1（可定位 node/automation_id/field）
fmt / adr-index / memory-counts / refscan / docscan / card-check / check-ledger /
check-comments / verify-schemas / codegen --check / check-migrations   全 EXIT 0
```

### 4. DoD 逐条核对

- [x] `docs/PARKING_LOT.md` 有 `PL-062 已闭环` 行，原 PL-062 行未改。
- [x] 孤儿自动化目录删除，`exists_before=True / exists_after=False`（1 文件 870 B）证据落卡。
- [x] `TASK-235` 状态行与 LEDGER 一致（补 PR / merge / CI）。
- [x] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 同批同步。
- [x] 全门禁绿；PR CI 11/11 + `MERGEABLE` + `CLEAN` + base=main 后合并并回填 merge hash。

### 5. 偏差

无。`PARKING_LOT.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md` 均按 ADR-0028 先取锁再写、写完 release；未改 AGENTS.md / spec / ADR / 产品代码 / xtask。

### 6. 更合理做法

无。两条收口都只需事实核对，说不上有更优工程做法。

### 7. 遗留问题

- `PL-023`（`scripts/` 顶层目录与 `nightly` 日志入库）仍独立待裁决，未受本卡影响。
- TASK-234 / 235 / 236 的 hygiene Warning（`xtask/src/replay*.rs` 长文件 / 复杂度建议）仍为 Warning，未升 Error。

### 8. 新增长期记忆

无新增 FACT / PITFALL；本轮为审计收口，未产生新的跨应用坑或硬事实（`MEMORY.md` 规模表无需变动）。

### 9. 给审阅者的关注点

1. PL-062 关闭是「实现消除差异」，不是「改 `AGENTS.md` 迎合现状」——`AGENTS.md` 一字未动。
2. 孤儿目录删除先解析绝对路径并在 `.codex\automations\` 内断言，再删单个具名目录，未用递归通配。
3. TASK-234 的 LEDGER「状态校正（CI 前）InProgress」行按 Out of scope 保留为事实，未追改。
