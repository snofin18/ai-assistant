# ADR-0090　章程 §2 P6 的 `PITFALL` 落点：改为 `docs/memory/`（与 ADR-0021 对齐）

状态：Accepted　日期：2026-10-14　Supersedes：—　Superseded by：—
关联：ADR-0021（长期记忆分层：`MEMORY.md` 只做索引）、AGENTS.md §3 / §5.2、章程 §2 P6、
`docs/memory/README.md`

## 背景

章程 §2 P6（自动化白名单「记忆维护」）写的是「汇总代码中的 `PITFALL(...)` 标签进 **`MEMORY.md` §5**；
补录 `LEDGER.md`」，write scope 也写成 `MEMORY.md`、`LEDGER.md`。

但 **ADR-0021**（2026-09-18）已把 `MEMORY.md` 重构成 ≤150 行的 **L0 索引**：只剩「四种记录文件的分工 /
读取路由 / 规模表 / 条目格式 / 更新职责 / §1 快照 / §7 归档索引」，**没有 §5 这一节**；原来的
`[PITFALL]` 正文已迁到 `docs/memory/pitfalls.md`（跨应用）与 `docs/memory/apps/<app>.md`（应用专属）。
AGENTS.md §3 与 §5.2 已是这个口径（`// PITFALL(app=excel): …` → 汇总进 `docs/memory/apps/excel.md` §6；
跨应用进 `docs/memory/pitfalls.md`）。

后果（一手，2026-10-13 的无人值守批次）：round 9~12 四轮做 P6 时都判定「`MEMORY.md` §5 与 ADR-0021 冲突 →
不自行改 ADR/spec → 回退 P2」，并把冲突写进各自 round 文件与阶段报告。**P6 这条无人值守通道实际是死的。**

## 决策（一句话）

章程 §2 P6 的落点由 `MEMORY.md` §5 改为 **`docs/memory/`**：跨应用的 `PITFALL(...)` 汇总进
`docs/memory/pitfalls.md`；应用专属的汇总进 `docs/memory/apps/<app>.md` §6；并**补录 `LEDGER.md`**（只追加）。

细则：

1. **D1　落点**：`MEMORY.md` §5 → 上述两个 L1/L2 文件（依据 ADR-0021 分层 + AGENTS.md §5.2 的
   `PITFALL(app=…)` 约定）。
2. **D2　write scope**：P6 的 write scope 改为 `docs/memory/pitfalls.md`、`docs/memory/apps/<app>.md`、
   `LEDGER.md`（只追加）；**`MEMORY.md` 移出 P6 write scope**（L0 由 Orchestrator 维护，仅 §1 快照 /
   规模表变化时改）。
3. **D3　保留**：P6 的「补录 LEDGER」不变；只追加、不改写他人条目（`docs/memory/README.md` 三条写入
   规则不变）。
4. **D4　不改上游**：不改 ADR-0021、不改 AGENTS.md、不改 `docs/memory/README.md` —— 它们已是正确口径，
   本次只把章程 §2 P6 对齐过去。

## 考虑过的选项（含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | 保留 `MEMORY.md` §5，把 §5 加回 MEMORY.md | ❌ 否决 | 与 ADR-0021 的分层（L0 只做索引、≤150 行）直接冲突，等于回退重构 |
| 2 | **P6 落点改为 `docs/memory/pitfalls.md`（跨应用）+ `docs/memory/apps/<app>.md` §6（应用专属）** | ✅ 采纳 | 与 ADR-0021 / AGENTS.md §5.2 一致；不改上游文档；只是章程补齐 |
| 3 | 取消 P6，记忆维护改由人类 / Orchestrator 做 | ❌ 否决 | P6 是无人值守可做的低风险维护；取消会丢掉一条产能通道 |

## 影响

- `docs/automation-charter.md`：§2 P6 行 + 文件头 1.19 → **1.20** + §12 追加 1.20 行。
- `docs/adr/README.md`：登记 0090、下一个可用编号 0090 → 0091。
- `docs/memory/decisions.md`：追加 ADR-0090 索引行。
- 不改：ADR-0021 正文、AGENTS.md、`docs/memory/README.md`、历史 round 文件与既有 LEDGER 行。

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| P6 产物写进 L1 时与应用专属条目重复 | `docs/memory/README.md` 写入规则 1：应用专属只进 `apps/<app>.md`，L1 只留一行指针 |
| 自动改 `docs/memory/*` 与人类 / 其它 run 冲突 | 该目录属「公共热点文件」，写前必须先 `xtask guard acquire`（ADR-0028） |

## 验证方式

1. `cargo run -p xtask -- adr-index` → 0 error（0090 登记表 ↔ 文件 ↔ `decisions.md` 一致、下一个可用编号 = 0091）。
2. `cargo run -p xtask -- hygiene / docscan / check-ledger / memory-counts / card-check / refscan` → 全 PASSED。
3. 现行正文扫描：`git grep -n -F "MEMORY.md §5" -- docs AGENTS.md MEMORY.md` 应 0 命中（历史 round 文件除外）。

**重新评估的触发条件**：若 ADR-0021 的分层再次调整（例如新增 L1 文件），P6 落点随之更新。
