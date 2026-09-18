# docs/memory/archive/ — L3 归档层

**当前为空**（还没有任何 L1/L2 文件超过 400 行）。

## 归档规则（ADR-0021）

- **触发条件**：某个 `docs/memory/*.md` 或 `docs/memory/apps/*.md` **超过 400 行**。
- **拆分方式**：**按主题**拆出（例如把 `pitfalls.md` 的「平台与生态」节整体移入
  `archive/pitfalls-platform.md`），**不按时间**拆。
  理由：记忆价值不按时间衰减，按时间归档会先扔掉最稳定的结论。
- **拆出后必须做**：① 在原文件对应位置留一行指针（`→ 见 archive/<file>.md`）；
  ② 在 `MEMORY.md` 的归档索引里登记一行；③ 在 `LEDGER.md` 追加一行。
- **命名**：`archive/<来源文件名>-<主题>.md`，例如 `archive/pitfalls-platform.md`。
- **不得**在归档时改写条目内容（归档 = 移动，不是重写）。
