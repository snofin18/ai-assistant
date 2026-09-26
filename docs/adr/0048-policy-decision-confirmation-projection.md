# ADR-0048　策略决策的确认投影必须无损

状态：**Accepted**（2026-09-26，人类授权 TASK-027 按 PL-082 走 ADR 路线）　日期：2026-09-26　Supersedes：—　Superseded by：—
关联：`cross-platform-ai-assistant-architecture-v2.md` §10.2 / §10.3 / §10.4 / §12.2、`docs/spec/audit-event.md`、`protocol/audit-event/audit-event-1.0.json`、`crates/policy`（TASK-021 `DRIFT-021-1`）、`docs/PARKING_LOT.md` PL-082、TASK-027 `hitl`

## 背景（为什么现在要决定）

TASK-021 的富策略决策包含：

```text
Allow
AllowWithConfirmation { scope_options, show_diff }
Deny { rule_id, reason }
```

但审计协议里的 `PolicyDecision` 只有 `allow` / `rule_id` / `reason`，因此
`AllowWithConfirmation` 的审批范围与是否需要差异预览在协议投影时被静默丢掉。
TASK-021 为避免把有损投影当完整决策，保留了 policy-local 富决策，并把缺口登记为
**PL-082 / DRIFT-021-1**。

TASK-027 的 `hitl` 必须把确认决策转成审批请求。若继续让协议投影有损，审批 UI
就无法从稳定协议获得“允许提供哪些授权范围、是否必须展示 diff”，只能反推或依赖
进程内类型，违反铁律 1 与铁律 10。

## 决策（一句话）

**在既有 `PolicyDecision` 上向后兼容地增加可选 `scope_options` 与 `show_diff`**：
`allow = true` 且出现非空 `scope_options` 时表示需要人工确认，此时 `show_diff`
必填；两者都缺席时表示无条件允许；`allow = false` 时两者都必须缺席。

## 决策细化

| # | 内容 |
|---|---|
| **D1** | `PolicyDecision` 增加两个可选字段：`scope_options: string[]`、`show_diff: boolean`。既有 `allow` / `rule_id` / `reason` 与序列化形状保持兼容。 |
| **D2** | **唯一确认判据** = `allow = true` 且 `scope_options` 存在且非空。此时 `show_diff` 必须存在；不得再增加与它同义的 `requires_confirmation` 布尔字段。 |
| **D3** | `scope_options` 的稳定字符串只能是架构 v2 §10.3 的五项：`once` / `this_step_pattern` / `this_task` / `this_app_session` / `persistent`；数组必须非空且无重复。 |
| **D4** | `allow = false` 时 `scope_options` / `show_diff` 必须缺席；拒绝原因仍由 `rule_id` + `reason` 表达。 |
| **D5** | `allow = true` 且 `scope_options` 缺席时是无条件允许，`show_diff` 也必须缺席。 |
| **D6** | policy-local 富 `Decision` 仍是 TASK-021 的求值结果；ADR 落地后 `to_protocol_decision()` 必须无损承载 confirmation 字段，不再以“只作审计摘要”为由丢字段。 |
| **D7** | 高风险（`RiskLevel::High` / `Critical` 或 `L3Irreversible`）仍只允许 `once`；`hitl` 在构造审批请求时 fail-closed，遇到策略给出的禁用范围直接返回错误，不得静默删项后继续。 |
| **D8** | 不新增 `ErrorCode`。协议形状错误继续映射既有 `ToolInvalidArgs` / `PolicyDenied` / `UserInteraction` 分类。 |
| **D9** | schema 顶层版本仍为 `1.0`：新增字段全部可选，旧生产者与旧消费者可继续工作；是否出现 `scope_options` 是判据，不依赖版本号分支。 |

## 考虑过的选项（至少 2 个，含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | **扩展现有 `PolicyDecision` 为可选无损投影（本 ADR）** | ✅ 采纳 | 单一契约、向后兼容、无需第二份“策略摘要”；审批请求可直接使用协议决策，不再反推 |
| 2 | 新建独立 `ApprovalDecision` schema，保留审计投影有损 | ❌ 否决 | 同一策略决策出现两份类型，必须同步且容易出现“审计看到的允许与执行看到的允许不同”；PL-082 的根因仍在 |
| 3 | 继续保留有损投影，要求 `hitl` 只接受进程内富类型 | ❌ 否决 | 跨进程 UI / 恢复 / 审计无法无损获得审批范围；调用方很容易误把 `allow=true` 当直接放行 |
| 4 | 增加 `requires_confirmation` 布尔字段 | ❌ 否决 | 与 `scope_options` 是否出现重复表达同一事实，违反 ADR-0030 的单一事实源原则 |

## 影响（需要改的文档与代码）

- `protocol/audit-event/audit-event-1.0.json`：扩展 `policy_decision.properties`，保持原有 required / additionalProperties 规则。
- `xtask/src/render.rs`：审计事件渲染器生成 `PolicyScopeOption` 与新的可选字段。
- `crates/protocol/src/generated/audit_event.rs` 与 `crates/protocol/src/lib.rs`：由 codegen 更新并导出新枚举。
- `docs/spec/audit-event.md`：补充 `policy_decision` 的确认投影语义与演进记录。
- `crates/policy/src/decision.rs`：`to_protocol_decision()` 保留 `scope_options` / `show_diff`。
- `crates/policy/tests/protocol_projection.rs` 与 README：把“已知有损”改为已由 ADR-0048 闭环。
- `docs/adr/README.md` / `docs/memory/decisions.md`：登记 ADR-0048。
- `docs/PARKING_LOT.md`：PL-082 闭环。
- TASK-027 `crates/hitl/**`：以新的协议确认字段作为审批请求输入，仍执行高风险范围收敛。
- **不改**：`ErrorCode` 分类、架构 v2 的五种授权范围、policy 规则 DSL 的既有两种可选范围。

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 旧消费者忽略新增字段，仍把 confirmation 当无条件 allow | 迁移期间所有使用方必须检查 `scope_options` 是否存在；TASK-027 的 `hitl` 只接受“有非空 scope”的决策构造审批请求，无条件 allow 返回明确错误 |
| `show_diff` 缺失但 scope 存在造成 UI 无法决定 | codegen 保持 `Option<bool>`，`hitl` 构造审批请求时校验缺失并返回 `ToolInvalidArgs` |
| 高风险策略错误地给出 `this_task` / `persistent` | `hitl` 不静默降级：发现禁用范围即拒绝请求并返回 `PolicyDenied`，让策略配置错误在测试中暴露 |
| 审计体积增加 | 字段只在需要确认时出现，且字符串枚举固定；无条件 allow / deny 仍保持原小形状 |

## 验证方式（怎么知道这个决策是对的；何时应重新评估）

1. `cargo run -p xtask -- verify-schemas` 与 `codegen --check` 绿；
2. `cargo test -p assistant-protocol` 覆盖新字段的 serde round-trip 与旧形状兼容；
3. `cargo test -p assistant-policy protocol_projection` 断言 `AllowWithConfirmation` 投影后仍保留 scope / `show_diff`；
4. TASK-027 测试覆盖：无条件 allow 不能构造审批请求、高风险非 `once` 范围被拒绝、TTL 到期硬边界；
5. **重新评估触发**：若未来审批范围增加模型可自定义字段或需要跨版本协商，必须新 ADR 取代本 ADR；不得在本字段上增加未声明字符串。
