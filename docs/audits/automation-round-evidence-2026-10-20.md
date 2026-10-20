# Automation round evidence audit — 2026-10-20

## 1. 范围与基线

- 起点：`origin/main = 9e990f6cda5977ff22cd03a81910fd7a00add50e`
- 运行日：`2026-10-20`（本地 19:30 触发，按 ADR-0088 归当日）
- 轮次：`2`（`origin/main` 上 `docs/automations/2026-10-20-round-*.md` 为 1 个）
- 闸门：最近 human 轮次为 `2026-10-14-round-2`；此后已有 22 个 automation 轮次，
  本轮计入后 `N_auto = 23 > 5`，按 ADR-0089 只做 docs-only 证据收口。
- 非范围：产品代码、测试断言、schema、公共接口、ADR、spec、依赖、真实 GUI /
  浏览器 / 网络 / 凭据操作。

## 2. PR 事实

| PR | 标题 | base | merge commit | CI |
|---|---|---|---|---|
| #500 | `docs(governance): close stage 1 status consistency` | `main` | `5e9f45301fce98b10be3fc28197e7572328821bc` | 22 checks（21 success + 1 skipped） |
| #501 | `docs(automation): backfill TASK-321 merge evidence` | `main` | `66711a4dfc95d621dd2cb4bc37c0ec82a7ccee6c` | 22 checks（21 success + 1 skipped） |
| #502 | `docs(automation): correct shell deviation count` | `main` | `9e990f6cda5977ff22cd03a81910fd7a00add50e` | 22 checks（21 success + 1 skipped） |

## 3. 发现的证据缺口

1. `docs/automations/2026-10-20-round-1.md` 只记录 PR #500，未记录随后互相同批回填的
   PR #501 与实际合入 `main` 的 PR #502。
2. TASK-321 的执行记录停在 PR #500 的 merge hash，未记录 PR #501 / #502 对
   `LEDGER.md` 与 shell 偏差计数的后续更正。
3. 既有轮次与卡片内容属于已落地历史记录；本轮只允许追加，不改写原文。
4. 当前 prompt 与操作手册均要求每条命令显式 `shell: "powershell"`，但：
   - round 1 已记录 7 条 shell 偏差；
   - round 2 开工核对阶段又发生 4 条只读命令误用 `shell: "bash"`，全部因目标命令不存在而失败，
     未修改仓库内容。

## 4. 处置

- 在 round 1 追加 PR #501 / #502 的合并后证据段，保留首轮原文。
- 在 TASK-321 执行记录区追加 round 2 证据，不改卡片正文或状态。
- 新增本审计与 TASK-322，并用追加式 LEDGER 行记录本轮。
- 不改产品代码、schema、ADR、spec 或依赖。

## 5. 验收证据

- `git diff --check` → exit 0。
- `cargo run -p xtask -- hygiene` → PASSED；438 files，0 error，136 existing warnings。
- `cargo run -p xtask -- card-check` → PASSED；216 files，0 error，35 existing warnings。
- `cargo run -p xtask -- docscan` → PASSED；517 files，0 error，229 existing warnings。
- `cargo run -p xtask -- check-ledger` → PASSED；`plan_date=2026-10-20`、
  `ledger_last_date=2026-10-20`、`current_stage=阶段 1`。
- `cargo run -p xtask -- refscan` → PASSED；984 files，0 error，0 warning。
- `cargo run -p xtask -- memory-counts` → PASSED；9 files，0 error，1 existing
  `paint.md` archive-threshold warning。
- `cargo run -p xtask -- adr-index` → PASSED；87 files，0 error，0 warning。
