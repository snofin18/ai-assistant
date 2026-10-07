# ADR-0081　干净上下文复核组件的公开契约

状态：**Accepted**（2026-10-07，人类「授权你按你说的做」授权）　日期：2026-10-07　
Supersedes：—　Superseded by：—
关联：ADR-0021、ADR-0028、ADR-0041、ADR-0053、ADR-0080、
`crates/core/src/verify_review.rs`、`crates/core/README.md`、
`docs/spec/core-orchestration.md`、架构 v2 §12.4

## 背景（为什么现在要决定）

架构 v2 §12.4 的第 4 层要求：高风险动作前用**不含外部内容**的独立小模型复核
「该动作是否与用户原始请求一致」，不一致则拒绝并告警。TASK-054 已把该能力拆到
`crates/core/src/verify_review**` 与 `crates/model-gateway/**`。

但 `docs/spec/core-orchestration.md` 的变更门槛规定：**新增公开组件必须先有 ADR**。
当前 `core` 的公开面只有会话 / 上下文 / Planner / Memory 四组组件；直接在本卡里新增
组件会同时触发漂移触发器 ③（公共接口）与 ⑤（超出 write scope），因此由
`DRIFT-054-1` 升级为人类裁决。人类已授权本 ADR 与最小扩权。

## 决策（一句话）

**在 `assistant-core` 新增一个公开的 `CleanContextReview` 组件：它只接收
`SessionSnapshot::goal()` 中的用户原始请求与调用方构造的、经校验的高风险 Step 摘要，
用注入的独立 `ModelProvider` 做严格 JSON 复核；不一致时返回拒绝，调用失败或不合法输出
一律 fail-closed；它不写入会话、不改变 taint，也不替代 policy / HITL 的放行权。**

## 决策细化

| # | 内容 |
|---|---|
| **D1** | 新增公开组件 `CleanContextReview`，实现落点为 `crates/core/src/verify_review.rs`，并在 `crates/core/src/lib.rs` 注册。该组件扩展 ADR-0053 D4 的公开组件面；ADR-0053 D2/D3 的依赖白名单与黑名单不变，不新增 crate、第三方依赖、protocol / IPC / DB schema。 |
| **D2** | 复核输入只有两类：`SessionSnapshot::goal()` 提供的用户原始请求，以及调用方构造并校验的 `HighRiskStepSummary`。组件**不得**接收会话消息列表、Tool 结果、网页、文档原文或其他外部内容；prompt 中也不得出现 `MessageRole::Tool`。 |
| **D3** | `CleanContextReviewer` 接收注入的 `Arc<dyn ModelProvider>`。独立小模型的注册与路由由装配层负责；`core` 不自行路由、不重试、不降级，也不读取凭据或网络。 |
| **D4** | 复核请求固定为 `System` 指令 + 一个 JSON 数据载荷，`ToolChoice::None`、无工具、temperature `0.0`。模型必须返回恰好含 `verdict` 与 `reason` 的 JSON 对象；`verdict` 只允许 `consistent` / `inconsistent`，空输出、超限输出、ToolCall 输出、未知字段或非法 verdict 全部失败。 |
| **D5** | `review()` 返回 `Allowed` 或 `Rejected`；`ensure_allowed()` 把 `Rejected` 转成带原因的 typed `Inconsistent` 错误。非法输入映射 `ToolInvalidArgs`，非法模型输出映射 `ModelInvalidOutput`，不一致映射 `PolicyDenied`，Provider 错误保留其原有 `ErrorCode`。 |
| **D6** | 复核不是会话消息：不得追加到 `SessionManager`，不得写入 `SessionSnapshot` / storage / audit，不得调用 `clear_taint` 或改变运行时 taint。测试必须证明含注入文本的 Tool 消息不会进入复核请求，且复核前后 session taint 不变。 |
| **D7** | 本组件只是独立复核原语，不自行判断风险等级、不放行工具、不取代 policy / HITL；高风险 / L3 的强制策略与人工确认规则保持 ADR-0080 及既有 policy 契约不变。 |
| **D8** | 允许的最小文档改动：新增本 ADR、回填 `docs/adr/README.md`、追加 `docs/memory/decisions.md`、更新 `docs/spec/core-orchestration.md` 与 `crates/core/README.md` 的公开面和边界；不得顺手改 protocol、IPC、DB schema 或其他 crate 的公共 API。 |

## 考虑过的选项（至少 2 个，含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | 放进 `crates/verify`，由 core 调用 | ❌ 否决 | ADR-0053 D3 把 `assistant-verify` 列为 core 永久禁止依赖的 crate；会破坏「core 不依赖放行/验证实现」的边界。 |
| 2 | 在 core 里写一个私有 helper，不登记公开组件 | ❌ 否决 | 装配层无法消费，卡目标无法落地；后续卡仍要重新决定输入、输出和失败语义，形成第二份契约。 |
| 3 | **在 core 新增 `CleanContextReview` 公开组件，注入 Provider，只接受结构化摘要（本 ADR）** | ✅ 采纳 | 保持依赖单向、public 形态明确、可直接单测；复核输入与 taint / session 写入分离，不替代 policy 放行点。 |

## 影响

- 新增 `crates/core/src/verify_review.rs`，公开 `CleanContextReviewRequest` /
  `HighRiskStepSummary` / `CleanContextReviewer` / `CleanContextReviewDecision` /
  `CleanContextReviewError`。
- `crates/core/src/lib.rs` 注册并导出该组件；`crates/core/README.md` 补充职责、边界、
  不变量与已知限制。
- `docs/spec/core-orchestration.md` 把公开组件面从四组扩展为四组 + 独立复核组件，
  并写明其不能替代 policy / HITL。
- 新增专项测试：一致 / 不一致、外部内容隔离、taint 不变、空输出 / 非法 JSON /
  ToolCall 输出 / Provider 失败的 fail-closed 行为。
- 不新增依赖、crate 或顶层目录；不改 `SessionSnapshot` / `SessionStore` /
  protocol / IPC / DB schema；不新增审计落盘路径。

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| Step 摘要本身仍可能是注入载体 | 只作为 JSON 数据载荷，system prompt 明确只评估不执行；无工具和无外部内容；最终仍由 policy / HITL 的不可逆动作确认兜底。 |
| 模型返回“看起来一致”但仍错误 | 复核只是一层辅助；`Rejected` 必须拒绝，`Allowed` 不取消既有策略检查与人工确认。 |
| 不同调用点各自拼 prompt 导致绕过隔离 | 公开面只暴露结构化请求类型与 Reviewer，prompt 构造保留在组件内部，不暴露 raw message 拼装入口。 |
| core 公开面继续膨胀 | 本 ADR 只授权这一个组件；后续新增公开组件仍须按同一门槛另立 ADR。 |

## 验证方式（怎么知道这个决策是对的；何时应重新评估）

1. `cargo test -p assistant-core verify_review::` 全绿，且覆盖一致、不一致、外部内容隔离、taint 不变和四类 fail-closed 失败。
2. `cargo test -p assistant-core arch::` 全绿；`crates/core/Cargo.toml` 仍符合 ADR-0053 D2。
3. `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全绿。
4. `cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check` 全部 PASSED，且 `adr-index` 登记 0081。
5. **何时重新评估**：需要把原始外部内容而不是结构化 Step 摘要送入复核；需要改成非 Provider 的模型调用；或需要把复核结果持久化 / 跨进程传输时，另立 ADR。
