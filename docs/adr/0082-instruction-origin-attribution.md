# ADR-0082　指令来源归因的 core 纯模型

状态：**Accepted**（2026-10-07，人类「授权你先立 ADR-0082 + 最小扩权，然后继续 TASK-052」）　日期：2026-10-07　
Supersedes：—　Superseded by：—
关联：ADR-0053、ADR-0080、ADR-0081、`crates/core/src/origin.rs`、
`apps/desktop-ui/src/features/approval/**`、
`docs/spec/core-orchestration.md`、架构 v2 §10.5 / §12.4

## 背景（为什么现在要决定）

架构 v2 §10.5 要求每个待批准动作都能回答“这个动作是谁要求的”，并把来源分为
`user_request` / `plan_derived` / `app_content` / `tool_suggestion` 四类；其中
`app_content` 是提示注入高危来源，审批卡片必须标红并默认拒绝，需用户显式覆盖。

当前 `apps/desktop-ui/src/features/approval/**` 已经定义了同名的 TypeScript
`InstructionOrigin`、`OriginBadge` 和 `app_content` 默认拒绝逻辑，但 `core` 侧还没有
canonical 值类型和归因记录。TASK-052 的占位卡写了 `crates/core/src/origin**`，属于
`core` 新增公开组件；`docs/spec/core-orchestration.md` 明确规定新增公开组件必须先有
ADR，因此本 ADR 冻结最小接口后再实现。

## 决策（一句话）

**在 `assistant-core` 新增纯模型 `InstructionOrigin` 与 `InstructionAttribution`：
用稳定 token 固定四类来源、校验每类所需 / 禁止的元数据，并把 `app_content` 标记为
高风险来源；它只提供可验证的归因记录，不判定权限、不写审计、不改 protocol /
`PolicyDecision` / `ApprovalRequest` / DB schema。**

## 决策细化

| # | 内容 |
|---|---|
| **D1** | `InstructionOrigin` 是 `core` 的公开枚举，稳定 token 固定为 `user_request` / `plan_derived` / `app_content` / `tool_suggestion`；未知 token 解析必须失败，禁止默认成 `user_request`。 |
| **D2** | `InstructionAttribution` 是归因记录：含 `origin`，以及按来源类型有条件的 `parent_goal` / `source_ref`。`plan_derived` 必须有父目标；`app_content` / `tool_suggestion` 必须有来源引用；`user_request` 不得携带这两类元数据。 |
| **D3** | `InstructionAttribution::is_high_risk()` 只把 `app_content` 标记为高风险信号。该信号供 policy / UI 使用，但**不自行放行或拒绝**；最终拒绝仍由 policy / HITL 与用户显式覆盖流程决定。 |
| **D4** | 值类型和字符串必须经过校验：非空、长度有界、无 unsupported control；错误返回 `ToolInvalidArgs` 与稳定 reason code。模型输出、IPC、被读文档进入归因前仍按铁律 2 校验。 |
| **D5** | UI 的 `instructionOrigins` token 集合必须与 core 的稳定 token 集合逐项一致；`app_content` 在审批模型中继续默认拒绝，需显式覆盖。UI 仍不授权动作，只展示与上报归因。 |
| **D6** | 本 ADR 只扩展 `core` 公开组件面，不改 `PlanStep` / `PolicyDecision` / `ApprovalRequest` / audit-event schema / IPC / DB。把归因写入审计事件或审批请求跨层字段需要后续 ADR + schema 变更。 |
| **D7** | 允许的最小文档 / 状态改动：本 ADR、ADR 登记表、decisions、core README、`core-orchestration` spec、`crates/core/src/lib.rs` 注册与 TASK-052 Done 同步文件。 |

## 考虑过的选项（至少 2 个，含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | 直接把 `instruction_origin` 加进 `ApprovalRequest` / `PolicyDecision` / audit-event schema | ❌ 否决 | 会同时改 Rust 公共形状、schema、codegen、IPC 与迁移；远超 TASK-052 的最小范围，且需要另一轮跨层契约。 |
| 2 | 只保留 UI 里的 TypeScript union | ❌ 否决 | UI 有展示，但不能给 core / 后续审计提供稳定 canonical 值和校验；同一概念会出现第二事实源。 |
| 3 | **在 core 增加纯值类型 + 归因记录，UI 消费同一 token 集合（本 ADR）** | ✅ 采纳 | 满足来源归因的最小可测试面，不污染策略、协议、审计或 DB；后续跨层接线有明确落点。 |

## 影响

- 新增 `crates/core/src/origin.rs`，公开 `InstructionOrigin`、
  `InstructionAttribution`、`OriginError` / `OriginResult`。
- `crates/core/src/lib.rs` 注册并导出模块；`crates/core/README.md` 同步职责、边界与不变量。
- `docs/spec/core-orchestration.md` 增加 InstructionOrigin 组件行与 ADR-0082 关联。
- UI approval 测试补足四类 token 契约与 `plan_derived` / `tool_suggestion` 展示路径；既有
  `app_content` 默认拒绝逻辑保持不变。
- 不新增依赖、crate、protocol / IPC / DB schema，不改 `PlanStep` 或审批公共类型。

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 归因 token 在 core / UI 漂移 | 两侧都测试精确四元组；未知 token 一律失败，不做默认值。 |
| `plan_derived` 只写来源却看不到父目标 | `InstructionAttribution` 要求 `plan_derived` 携带非空 `parent_goal`。 |
| `app_content` 被调用方当成已获授权 | `is_high_risk` 只是信号；ADR 明写 policy / HITL 仍是唯一放行点，UI 仍需显式覆盖。 |
| 本卡被误认为已完成 audit / IPC 落盘 | ADR 明写跨层 schema 属后续卡；TASK-052 只交付 canonical core 值 + UI 显示与校验。 |

## 验证方式（怎么知道这个决策是对的；何时应重新评估）

1. `cargo test -p assistant-core origin::` 全绿，覆盖四类 token、非法 token、元数据组合和 `app_content` 高风险标记。
2. UI approval 测试覆盖四类 origin token 一致性、`plan_derived` / `tool_suggestion` 可展示、`app_content` 默认拒绝与显式覆盖。
3. `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全绿。
4. `cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check` 全 PASSED，且 `adr-index` 登记 0082。
5. **何时重新评估**：需要在 audit event / IPC / ApprovalRequest 中持久化归因，或需要新增第五种来源时，另立 ADR。
