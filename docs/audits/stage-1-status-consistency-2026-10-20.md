# Stage 1 status consistency audit — 2026-10-20

## 1. 范围与基线

- 起点：`origin/main = d96ae4c412e2040f40ea5c70bcc89c77ac73e51b`
- 范围：2026-10-19~20 完成的 Paint / UIA 卡与其在任务卡、阶段计划、`PLAN.md`、
  `README.md`、`LEDGER.md` 中的状态。
- 本轮机制：automation；运行日 `2026-10-20` round 1。
- 闸门：最近 human 轮次为 `docs/automations/2026-10-14-round-2.md`；此后已有
  21 个 automation 轮次，本轮计入后 `N_auto = 22 > 5`，按 ADR-0089 只做
  docs-only 报告与状态收口。
- 非范围：产品代码、测试断言、schema、公共接口、ADR、spec、依赖、真实 GUI /
  浏览器 / 网络 / 凭据操作。

## 2. 事实源交叉核对

| 卡 | 任务卡状态 | LEDGER merge 证据 | 阶段计划处置 |
|---|---|---|---|
| TASK-044 | Done | `af005c9` / PR #487 | 批次行补 `✅` |
| TASK-045 | Done | `313af12` / PR #490 | 批次行补 `✅` |
| TASK-046 | Done | `015c5fe` / PR #492 | 批次行补 `✅` |
| TASK-047 | Done | `f59f7f9` / PR #494 | 批次行补 `✅` |
| TASK-256 | Done | `0fb655b` / PR #485 | 批次行补 `✅` |
| TASK-257 | Done | `069328b` / PR #496 | 新增完成行 |
| TASK-309 | Done | `043bc62` / PR #483 | 新增完成索引行 |
| TASK-317 | Done | `fdbd356` / PR #474；`043bc62` / PR #483 | 新增完成索引行 |
| TASK-318 | Done | `b5be949` / PR #476；`6aa3be8` / PR #477；`043bc62` / PR #483 | 新增完成索引行 |
| TASK-319 | Done | `ff8337d` / PR #479；`0bed1b1` / PR #480；`043bc62` / PR #483 | 新增完成索引行 |
| TASK-320 | Done | `235b171` / PR #489 | 新增完成索引行 |

## 3. 发现的漂移

1. `plans/stage-1-pilots.md` 的 1b 批次行仍把已完成卡显示为未完成；TASK-257
   和 TASK-309~320 完全没有进入索引表。
2. 同一文件的“当前进度”仍写 `TASK-257 Review` / `TASK-256 Blocked`，
   与任务卡及 merge 证据冲突。
3. `PLAN.md` 的“更新日期 / 当前阶段 / 当前任务卡 / 阻塞项 / 下一步动作”
   仍停在 TASK-319 InProgress、TASK-256 Review、TASK-044 等待十次运行的状态。
4. `README.md` 尚未记录本次状态一致性收口；其状态行仍以较早任务作为最新状态。

## 4. 处置

- 计划文件只新增完成标记、已完成任务行和“当前进度”内容；既有任务条目的
  名称、write scope、依赖、预估和验收要点不改（ADR-0041）。
- `PLAN.md` 只替换当前状态控制行；旧历史摘要继续保留，不把派生细节另立事实源。
- `README.md` 只改状态行、`## 当前阶段` 和 `## 最近进展` 三处（ADR-0039 D5）。
- 新任务使用独立卡 `TASK-321-stage1-status-consistency-closeout.md`，正文与记录区
  按 ADR-0031 分界。

## 5. 结论

Stage 1a 与 Stage 1b 已完成；Stage 1c Edge/Chrome 尚未完成。下一张产品卡候选为
TASK-048 / TASK-049，但当前 automation 已超过 ADR-0089 的代码轮次闸门，必须由
人类会话或闸门重置后的轮次派单。

## 6. 验收证据

- `git diff --check` → exit 0。
- `cargo run -p xtask -- hygiene` → PASSED；438 files，0 error，136 existing warnings。
- `cargo run -p xtask -- card-check` → PASSED；215 files，0 error，35 existing warnings。
- `cargo run -p xtask -- docscan` → PASSED；514 files，0 error，229 existing warnings。
- `cargo run -p xtask -- check-ledger` → PASSED；`plan_date=2026-10-20`、
  `ledger_last_date=2026-10-20`、`current_stage=阶段 1`。
- `cargo run -p xtask -- refscan` → PASSED；981 files，0 error，0 warning。
- `cargo run -p xtask -- memory-counts` → PASSED；9 files，0 error，1 existing
  `paint.md` archive-threshold warning。
- `cargo run -p xtask -- adr-index` → PASSED；87 files，0 error，0 warning。

## 7. 保留项

ADR-0041 禁止改写既有任务条目正文，因此部分旧索引说明仍保留 `Ready` / `Review`
字样。完成态以行首 `✅`、任务卡 `- 状态：` 行和 `LEDGER.md` merge 证据为准；本审计
不借收口之机重写条目正文。
