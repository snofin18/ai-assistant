# ADR-0080　污点追踪与权限衰减的运行时语义

状态：**Accepted**（2026-10-07，人类「可以继续进行下一步」授权）　日期：2026-10-07　
Supersedes：—　Superseded by：—
关联：ADR-0021、ADR-0028、ADR-0041、ADR-0053、ADR-0063、
`crates/policy/src/rule_set.rs`、`crates/core/src/session.rs`、
`cross-platform-ai-assistant-architecture-v2.md` §12.4

## 背景（为什么现在要决定）

策略引擎已经有 `EvaluationContext.tainted`，并且对污点上下文中的高风险 / L3 动作有强制拒绝。
TASK-051 还必须补齐两个缺口：

1. 谁维护会话级 taint 状态，以及它在消息树上的传播 / 清除规则；
2. 污点生效期间，宽授权如何降级，尤其是 `ThisTask` 是否可以继续保留。

当前 `SessionSnapshot` 与 storage 的 conversation 表已经冻结；为了一个运行时安全标记增加
数据库列或改公共快照形状，会把 storage schema / 迁移 / protocol 一起拖进本卡，代价过大。

## 决策（一句话）

**taint 是 `SessionManager` 维护的运行时状态：Tool 消息使会话置污，新的 User 消息或
`SessionManager::clear_taint` 才能清除；恢复会话时从消息角色序列保守重算。policy 在
`tainted=true` 时把所有确认范围降级为 `Once`，高风险 / L3 的强制拒绝保持不变。**

## 决策细化

| # | 内容 |
|---|---|
| **D1** | `SessionManager` 为每个已缓存会话维护私有 `TaintState`。新会话是 clean；该状态不进入 `SessionSnapshot` / protocol / DB。 |
| **D2** | 消息传播规则固定：`MessageRole::Tool` → tainted；`MessageRole::User` → clean；`System` / `Assistant` 不改变状态。Tool 结果按架构 §12.4 第 1 层一律视为不可信输入。 |
| **D3** | `SessionManager` 是唯一清除点：公开 `is_tainted()` 与 `clear_taint()`；`clear_taint()` 只对当前缓存中的活动会话生效，不修改消息历史。 |
| **D4** | 恢复会话时按消息树的 `sequence` 顺序重放角色序列并重算 taint。显式 `clear_taint()` 不持久化；进程重启后若历史最后仍是 Tool 消息，则恢复为 tainted，保持 fail-closed。 |
| **D5** | policy 保持 `tainted` 为输入；在构造 `AllowWithConfirmation` 决策时，如果 `tainted=true`，无论规则声明了 `Once`、`ThisTask` 还是两者，最终只提供 `Once`。 |
| **D6** | 高风险 / critical-risk / L3 的强制拒绝继续由 policy 的 mandatory safety floor 执行，不受规则集声明影响。 |
| **D7** | 本 ADR 不新增依赖、不改 `SessionSnapshot` / `SessionStore` / protocol / IPC / DB schema，也不新增 taint 持久化表。 |

## 考虑过的选项（至少 2 个，含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | 把 `tainted` 写进 `SessionSnapshot` 并迁移 storage | ❌ 否决 | 改公共快照 / DB schema / 迁移链，超出 TASK-051 的 policy + session 范围；且历史消息已足以保守重算。 |
| 2 | 只在调用方临时传 `tainted`，SessionManager 不维护状态 | ❌ 否决 | 会把安全状态交给每个调用点自行维护，容易漏标；架构 §12.4 明确要求会话维护标记。 |
| 3 | **运行时状态 + 恢复时从消息角色重算；policy 在 tainted 时降级为 `Once`（本 ADR）** | ✅ 采纳 | 不改公共 schema，clear 点唯一，恢复保持 fail-closed，权限衰减可直接在唯一放行点实现。 |

## 影响

- `crates/core/src/session/taint.rs` 新增私有 `TaintState`；`SessionManager` 维护并公开查询 / 清除入口。
- `crates/policy/src/taint.rs` 提供确认范围降级；`RuleSet::evaluate` 在 tainted 上下文应用。
- 新增 session / policy 单测覆盖 Tool→tainted、User→clean、恢复重算、显式清除和 `Once` 降级。
- 不新增 crate / 依赖 / schema / IPC 字段；不改变已有 clean 路径的决策形状。
