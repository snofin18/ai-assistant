# docs/memory/archive/ — L3 归档层

**归档清单的唯一落点 = `MEMORY.md` §7「归档索引」**（本文件只写规则，**不抄第二份清单**）。

> 2026-10-18（TASK-314 / PL-116）：本句原文是「**当前为空**（还没有任何 L1/L2 文件超过 400 行）」，
> 而 `docs/memory/archive/paint-early-probes.md`（429 行）已于 2026-10-17 落地 —— 同一事实手写两处
> 必然漂移（ADR-0072「动态派生值只指向唯一事实源」）。故改为指针：**有哪些归档、从哪来、为什么**，
> 一律看 `MEMORY.md` §7。

## 归档规则（ADR-0021）

- **触发条件**：某个 `docs/memory/*.md` 或 `docs/memory/apps/*.md` **超过 400 行**。
- **拆分方式**：**按主题**拆出（例如把 `pitfalls.md` 的「平台与生态」节整体移入
  `archive/pitfalls-platform.md`），**不按时间**拆。
  理由：记忆价值不按时间衰减，按时间归档会先扔掉最稳定的结论。
- **拆出后必须做**：① 在原文件对应位置留一行指针（`→ 见 archive/<file>.md`）；
  ② 在 `MEMORY.md` 的归档索引里登记一行；③ 在 `LEDGER.md` 追加一行。
- **命名**：`archive/<来源文件名>-<主题>.md`，例如 `archive/pitfalls-platform.md`。
- **不得**在归档时改写条目内容（归档 = 移动，不是重写）。
