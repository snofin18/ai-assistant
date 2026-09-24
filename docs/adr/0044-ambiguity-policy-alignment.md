# ADR-0044　歧义策略与架构 v2 §6.6 对齐：平台层只保留 fail-closed 的一种语义

状态：**Accepted**（2026-09-24，人类 chat「按你的建议去做」授权）　日期：2026-09-24　Supersedes：—　Superseded by：—
关联：架构 v2 §6.3 / §6.6、`crates/platform/api/src/target.rs`、`crates/platform/windows/src/selector.rs`、`docs/PARKING_LOT.md` PL-069

## 背景（为什么现在要决定）

架构 v2 §6.6 定义 **4** 种多匹配策略：`error_and_ask`（默认）/ `first_by_order` /
`require_unique` / `disambiguate_by`，并明确「**禁止默默取第一个**」。

TASK-016 冻结的 `OnAmbiguous` 却只有 **2** 个变体：`ErrorAndAsk` / `HighestScore`。
TASK-017 落地时发现 `HighestScore` **在候选链模型里没有定义**：

- 候选链的 `score` 属于**候选**（selector；架构 v2 §6.2 的 `score` = 经验值 + 学习值），
  **不属于**命中元素；
- 因此同一个候选命中的 n 个元素，其「命中分数」**恒等** → 「最高分并列」必然成立
  → `decide_selection` 传 `HighestScore` 时**永远**走 `Ambiguous`
  （实测：`crates/platform/windows/src/selector.rs` 的两条用例，一条 `Unique`、一条 `Ambiguous`）。

结论：`HighestScore` 是一个**声明了却永远只能报错**的选项 —— 它既不在 §6.6 里，
也不能表达「取最高分」。这正是 PL-069 登记的口径缺口。

## 决策（一句话）

**平台层的歧义语义收敛为唯一一种：`error_and_ask`（fail-closed）**；
删掉 `OnAmbiguous::HighestScore`；§6.6 其余 3 种策略的**归属层**写清（它们不是平台层的职责）。

## 决策细化

| # | 内容 |
|---|---|
| **D1** | `OnAmbiguous` 只保留 `ErrorAndAsk`。它是**唯一**的平台层行为：多命中 → `TargetAmbiguous`（铁律 1：不猜） |
| **D2** | **删除 `HighestScore`** —— 它在候选链模型里不可实现（见背景），且不是 §6.6 的策略。删除是**缩窄**契约（`#[non_exhaustive]` 已预留扩展位），不是放宽判据 |
| **D3** | **`require_unique`（§6.6）= 调用方把平台层的 `TargetAmbiguous` 直接判失败**。也就是说它在平台层的**行为**与 `error_and_ask` 相同，差别在**上层怎么处置**这个错误（问模型 / 问用户 vs 直接失败）→ 归 Host / task-engine（TASK-019 / 022），**不进**平台层枚举 |
| **D4** | **`first_by_order`（§6.6）本层不实现**。它要求「按树顺序取第一个」**并且**在审计里标记「使用了歧义解析」；平台层**没有**审计通道（审计是 `crates/audit`，TASK-013），先实现「取第一个」再补审计 = 一段时间内**静默地**猜（违反铁律 1 与 §6.6 明文）。要引入时：连同审计标记一起，由持有审计通道的 Host 层实现，另开 ADR |
| **D5** | **`disambiguate_by`（§6.6）= Adapter 用更强的候选链表达**（`RoleAndParent` 已实现「在某父候选的子树内搜索」；索引 / 可见性条件归 `ClassAndRole` 与后续候选种类）。它是**候选编写**问题，不是运行时策略 → 不需要枚举变体 |
| **D6** | 本 ADR **授权**：① 改 `crates/platform/api/src/target.rs` 的 `OnAmbiguous`；② 改 `crates/platform/windows/src/selector.rs` 的 `decide_selection` 与那两条用例；③ 在 架构 v2 §6.6 的表下补一段「各策略的归属层」 |

## 考虑过的选项（至少 2 个，含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | 保留 `HighestScore`，把它文档化成「等价于报歧义」 | ❌ 否决 | 一个「存在但只能报错」的选项会让调用方以为它有用（PL-069 的原始误读就是这么来的）；契约必须表达真实行为 |
| 2 | 引入 §6.6 的 `first_by_order` 并在审计里标记 | ❌ 本批否决 | 平台层没有审计通道 → 只能实现「取第一个」而**没有**标记 = 静默猜（违反 §6.6 明文与铁律 1）。等 Host 层（TASK-019）有审计通道时再按 D4 另开 ADR |
| 3 | 把 §6.6 的 4 种全部搬进 `OnAmbiguous` | ❌ 否决 | 其中 2 种（`require_unique` / `disambiguate_by`）不是平台层能表达的（见 D3 / D5）；搬进来只会制造第 2 个「声明了却做不到」的变体 |
| 4 | **收敛为 `ErrorAndAsk` + 写清各策略归属层（本 ADR）** | ✅ **采纳** | 契约只声明平台层真正能做到的事；§6.6 的另外 3 种各自有明确归属，且没有一条被静默丢弃 |

## 影响（需要改的 spec / 代码 / 文档 / 任务卡）

- `crates/platform/api/src/target.rs`：`OnAmbiguous` 删掉 `HighestScore`（**改公共接口**，本 ADR 授权）
- `crates/platform/windows/src/selector.rs`：`decide_selection` 去掉 `HighestScore` 分支 + 删掉测它的 2 条用例（**不是**为过门禁改断言 —— 被测的变体已不存在）
- `crates/platform/windows/README.md`：「已知限制」里的歧义条目改为「已按 ADR-0044 收敛」
- 架构 v2 §6.6：补「各策略的归属层」一段
- `docs/PARKING_LOT.md`：PL-069 处置行；`tasks/TASK-017-*.md` §5：登记裁决
- **不改**：`ResolutionPolicy` 的字段形状（`on_ambiguous` 仍是 `OnAmbiguous`，序列化值仍是 `error_and_ask`）、`SelectorChain`、任何排期（ADR-0041 D2）

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 删掉公共枚举变体破坏外部调用方 | 阶段 1a 尚无 crate 外调用方（`OnAmbiguous` 只在 `assistant-platform-api` / `assistant-platform-windows` 内使用，已 grep 全仓）；`#[non_exhaustive]` 保留未来扩展位 |
| 未来需要「多命中取一个」时又要改回 | D3 / D4 已写明各自的**正确**归属层与前置条件（审计标记）；届时按 D4 另开 ADR，而不是把 `HighestScore` 加回来 |
| 「收敛成 1 个变体」被误读成「平台层不管歧义」 | 恰恰相反：平台层**唯一**的歧义行为就是 fail-closed 报 `TargetAmbiguous`；D3 明确上层如何把它变成 `require_unique` |

## 验证方式（怎么知道这个决策是对的；何时应重新评估）

1. **立即验证（落地时）**：`cargo test --workspace` 全绿；`HighestScore` 在 `crates/` 下 **0 命中**；
   `cargo clippy --all-targets -- -D warnings` exit 0。
2. **重新评估触发条件**：Host 层落地（TASK-019）后若要支持 §6.6 的 `first_by_order`（含审计标记）；
   或出现「平台层必须自己消歧」的真实用例（那时要有 ADR + 审计通道）。

## 相关 ADR

- ADR-0022（UIA selector 稳定性 —— 候选链 `score` 的语义来自 §6.2 / §6.3）
- ADR-0043（元素解析 scope —— 同一批 TASK-017 遗留裁决）
