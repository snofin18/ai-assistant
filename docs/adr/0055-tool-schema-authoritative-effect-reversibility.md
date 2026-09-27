# ADR-0055　ToolSchema 的权威 effect / reversibility

状态：**Accepted**（2026-09-27，人类 chat「继续」= 授权按建议立 ADR 并实施）　日期：2026-09-27　Supersedes：—　Superseded by：—
关联：架构 v2 §5.2、`docs/spec/tool-schema.md`、`docs/spec/core-orchestration.md`、ADR-0021（受控词汇表）、ADR-0053（Core 编排边界）、TASK-207（Planner）DRIFT-207-1

## 背景（为什么现在要决定）

TASK-207 的独立 review 发现：模型在 Planner 输出中可以自报 `effect` / `reversibility`，而 ToolSchema 没有这两个字段。`Plan::validate()` 只能检查「模型自报字段彼此是否一致」，不能证明它们与真实工具有关。模型因此可以把 high/critical、实际会写入的工具标成 `read` 或较低可逆性，绕过写步骤 postcondition 要求。

该问题不能用默认值或风险级猜测修补：

- `risk_level` 不等于 `effect`：敏感读取可以是 high risk，写入也可以由 L0/L1 撤销。
- `requires_approval` 不等于不可逆性：审批范围与撤销能力是两个维度。
- 让 Planner 继续信任模型自报值，违反铁律 2「模型输出是不可信输入」。

## 决策（一句话）

**`effect` 与 `reversibility` 是 ToolSchema 的权威必填元数据；Planner 只从已校验的工具目录注入这两个字段，模型输出不得提供它们。**

## 决策细化

| # | 内容 |
|---|---|
| **D1** | `ToolSchema` 新增必填字段：`effect ∈ {read, write}`、`reversibility ∈ {l0_undo_stack, l1_snapshot, l2_compensation, l3_irreversible}`。字段名与 task-engine 的稳定序列化值保持一致。 |
| **D2** | `ToolDefinition::new` 必须显式接收 `effect` 与 `reversibility`；禁止按风险级推断默认行为。`critical` 或 `l3_irreversible` 默认 `requires_approval=true`。 |
| **D3** | Planner 的模型输出只接受 `id / sequence / tool / args / depends_on / postconditions / point_of_no_return / timeouts`。模型若输出 `effect` / `reversibility`，直接以 `ModelInvalidOutput` 拒绝。 |
| **D4** | Planner 在反序列化为 task-engine PlanStep 前，从该 `tool` 的权威 ToolSchema 注入 `effect` / `reversibility`；未知工具仍在进入 Plan 前拒绝。 |
| **D5** | `tool-bus` 的工具集指纹必须包含 `effect` / `reversibility`：行为元数据变化会让模型/策略所见的工具集事实变化。 |
| **D6** | 本次修改发生在项目公开前的单一 `tool-schema-1.0.json` 上：不新建第二份 schema，不保留缺字段的旧实例。理由是当前没有外部工具作者，且 codegen 仍只支持单一版本；发布冻结后任何同类变化必须走 version bump + 兼容期。 |
| **D7** | 同步 `docs/spec/tool-schema.md`、generated Rust 类型、tool-bus 注册期校验、Planner prompt/tests、任务卡 DRIFT-207-1 与进度事实源。 |

## 考虑过的选项

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | 继续让模型自报，仅由 policy 在更晚阶段纠正 | ❌ 否决 | Planner 已会据此跳过 postcondition 等结构校验；到 policy 再补会让错误 Plan 进入 task-engine |
| 2 | 给 ToolSchema 字段可选，缺省按 risk level 推断 | ❌ 否决 | risk 与 effect/reversibility 不是一个维度；默认值会制造新的静默降级 |
| 3 | 新建 `tool-schema-1.1` 并保留 1.0 | ❌ 本次不采纳 | codegen 目前单版本；项目尚未发布、无外部实例，先做 pre-freeze 修正。冻结后同类改动必须版本化 |
| 4 | ToolSchema 必填 + Planner 只信任目录注入 | ✅ 采纳 | 同时满足不可信输入、fail-closed、tool-bus 注册期校验和 task-engine Plan 结构 |

## 后果（正 / 负）

- **正**：模型不能再降级真实工具的行为语义；写步骤 postcondition 与 L3 `point_of_no_return` 检查基于权威事实。
- **正**：工具作者必须在注册期声明行为，不再依赖隐式推断。
- **负**：`ToolDefinition::new` 与 ToolSchema 是破坏性变更，现有内部调用点必须全部显式补字段。
- **负**：模型 prompt/test fixture 必须移除 `effect` / `reversibility`，否则 fail-closed。

## 关联与实施

- 落地实现：TASK-207 的 Planner、`crates/tool-bus` 注册与指纹、protocol codegen、`docs/spec/tool-schema.md`。
- 关闭：`DRIFT-207-1`。
