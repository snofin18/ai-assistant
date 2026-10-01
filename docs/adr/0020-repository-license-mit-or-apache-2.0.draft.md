# ADR-0020　仓库采用 MIT OR Apache-2.0 双许可证

状态：**Draft**（自动工作单 W4，待人类批准）　日期：2026-10-02　Supersedes：decisions.md `[ADR:待建 0020]`　Superseded by：—
关联：`LICENSE`、`NOTICE`、`Cargo.toml`、`README.md`、`deny.toml`、`docs/memory/open.md` M5

## 背景（为什么现在要决定）

仓库骨架建立时，M5 的最终许可证尚未裁决；TASK-001 为解除主线阻塞，曾临时按
**MIT 单许可**落地，并明确该选择可逆、需在阶段 0 结束前正式确认。

2026-09-18 人类完成 M5 裁决：采用 Rust 生态常见的 **MIT OR Apache-2.0 双许可证**。
此后 `LICENSE`、`NOTICE`、`Cargo.toml` 与 README 均已按双许可更新。本 ADR 只把
这一已经生效的决定从 `decisions.md` 的待建条目落成正式记录。

## 决策（一句话）

**本项目按 `MIT OR Apache-2.0` 双许可证分发，使用者可任选其一；许可证文本、
Apache-2.0 要求的 NOTICE、Cargo 元数据与依赖许可证白名单保持一致。**

决策细化：

| # | 内容 |
|---|---|
| **D1** | 仓库许可证标识为 `MIT OR Apache-2.0`。 |
| **D2** | `LICENSE` 同时包含 MIT 与 Apache-2.0 全文，并明确使用者可任选其一。 |
| **D3** | 仓库保留 `NOTICE`，满足 Apache-2.0 §4(d) 对归属声明的硬要求。 |
| **D4** | `Cargo.toml` 的 workspace `license` 字段与运行时分发标识保持同一值。 |
| **D5** | 依赖许可证白名单至少允许 `MIT` 与 `Apache-2.0`；依赖引入仍需按依赖治理流程审批。 |

## 考虑过的选项（至少 2 个，含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | **MIT OR Apache-2.0 双许可（本 ADR）** | ✅ 采纳 | 保留 MIT 的低摩擦使用，同时提供 Apache-2.0 的明确专利授权条款。 |
| 2 | 只使用 MIT | ❌ 最终否决 | 实现简单，但不提供 Apache-2.0 的专利授权表达；M5 已改选双许可。 |
| 3 | 只使用 Apache-2.0 | ❌ 否决 | 增加 NOTICE 与合规负担，也放弃 MIT 更短、更宽松的采用路径。 |
| 4 | 暂不决定许可证 | ❌ 否决 | 发布、依赖合规与贡献者授权都会处于不确定状态。 |

> TASK-001 期间的 MIT 单许可是明确的临时、可逆决定；M5 裁决后由本双许可决定取代。

## 影响（需要改的 spec / 代码 / 文档 / 任务卡）

- `LICENSE`：双许可证全文与选择声明。
- `NOTICE`：Apache-2.0 §4(d) 要求的归属信息。
- `Cargo.toml`：workspace `license = "MIT OR Apache-2.0"`。
- `README.md`：许可证说明与当前状态。
- `deny.toml`：依赖许可证白名单包含两个许可证。
- `docs/memory/open.md`：M5 已关闭，历史条目保留。

本 ADR 不决定未来商业发行、贡献者协议或开源脱敏清单。

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 分发时遗漏 `NOTICE` 或 Apache-2.0 全文 | 打包检查与人工发布清单把 `LICENSE`、`NOTICE` 列为必带文件。 |
| Cargo 元数据、README 与 LICENSE 漂移 | `cargo deny check` 与发布前检查同时核对许可证标识。 |
| 引入白名单外许可证的依赖 | 新增依赖必须登记并经过依赖治理；不因本项目是双许可就放宽依赖白名单。 |
| 临时 MIT 决策被误当成现行决策 | `docs/memory/open.md` 的 M5 关闭条目与 `decisions.md` 的 M5 追加条目保留改选链路。 |

## 验证方式（怎么知道这个决策是对的；何时应重新评估）

1. `Cargo.toml` 的 workspace `license` 为 `MIT OR Apache-2.0`，与 README 一致。
2. `LICENSE` 同时包含两种许可证文本，`NOTICE` 存在且包含项目归属。
3. `cargo deny check` 的 licenses 检查通过。
4. M5 在 `docs/memory/open.md` 中仍保留“已裁决”的历史链。
5. **重新评估触发**：项目改变分发模式、需要 relicensing、或增加新的贡献者授权协议时，
   必须新 ADR 取代本 ADR，并审计已分发版本的既有授权。
