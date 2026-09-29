# ADR-0056　运行执行链路契约：装配层执行器 + VerifyReceipt 提交门

状态：**Proposed**（等待人类确认后转 Accepted；TASK-103 不得在转正前开工）
日期：2026-09-29
Supersedes：—
Superseded by：—
关联：TASK-102、TASK-103、TASK-039 集成审计、架构 v2 §7 / §8 / §9 / §12、
ADR-0038、ADR-0048、ADR-0053、ADR-0055

---

## 背景

阶段 1a 的组件已分别存在：Planner 能产生 `Plan` / `PlanStep`，task-engine 有 12 状态与
DAG/检查点，Policy 有唯一放行点，ToolBus 有 MCP 信封，Host 有 IPC 传输，verify 能产生
`VerifyOutcome`，undo 能执行回滚，hitl 能表达授权与接管。但当前没有唯一执行者把这些
组件按确定顺序串起来：

- `task-engine` 的 `commit_step` 只接收 `post_fingerprint: Option<String>` 和
  `had_warning`，没有验证结果前置条件；
- `automation-host` 尚未分发工具调用；
- UI/Core/Host 尚无完整执行接线；
- TASK-039 因此判定阶段 1a **NO-GO**。

在没有新契约时直接实现，很容易把编排逻辑塞进 `core`、让 UI/Host 自行判断权限，或把
“验证失败”降级成布尔值/字符串。三者都违反既有铁律。

## 决策（一句话）

**运行执行由一个位于 binary 装配层的 `RuntimeExecutor` 独占编排；它逐步驱动
TaskEngine、Policy、HITL、Lease、ToolBus、Host、Verify 与 Undo，并且
`task-engine` 只能通过 verify 产生的不透明 `VerificationReceipt` 提交步骤。**

首次实现范围只要求一个可运行的 `PlanStep` 闭环，不一次抽象多渠道或通用工作流。

## 决策细化

| # | 内容 |
|---|---|
| **D1 执行器归属** | `RuntimeExecutor` 属于 `apps/agent-core` 的 binary 装配层，不进入 `assistant-core`。它持有各组件 trait/接口并编排顺序；`core` 仍只提供 Planner/Session/Context/Memory 组件。 |
| **D2 单写者** | 只有 `RuntimeExecutor` 能推进一个运行中任务的 Step 状态。UI、Host、Tool handler 和模型都不能直接推进 task-engine 状态。 |
| **D3 单步闭环** | 每次 `advance` 最多处理一个 ready `PlanStep`：Resolve → Precheck → Policy → Approval → Lease → Anchor → Execute → Verify → Commit/Escalate。不并发执行同一任务的多个副作用步骤。 |
| **D4 唯一放行点** | 任何工具调用前都必须从 ToolSchema 的权威 `effect` / `reversibility` + 运行时上下文构造 Policy 输入并取得决策。`RuntimeExecutor` 只能执行无条件 `Allow` 或已满足确认要求的调用；`Deny` 不得通过 UI/Host 绕过。 |
| **D5 验证提交门** | verify 新增不透明 `VerificationReceipt`：字段私有、无公开构造器、不实现反序列化，只能由 `verify_postconditions` 在 `Verified` 时返回。task-engine 的成功提交接口必须消费该 receipt；`VerifyOutcome::Violated` / `Inconclusive` 在类型上无法进入成功提交。 |
| **D6 未知边界** | 执行边界之前失败可安全重试；越过执行边界后若无法判定动作是否发生，必须进入 `NeedsHuman`，不得自动重试或声称失败未生效。 |
| **D7 取消/超时/接管** | Pause 只在 Step 边界生效；Cancel 阻止后续 Step，并对已执行的可逆 Step 询问/执行回滚；TakeOver 立即停手、释放输入/租约，归还控制后必须重新解析目标并重算指纹。 |
| **D8 撤销与审计** | 可逆写步骤在 Execute 前捕获 Anchor；Verify 成功后才能记录 post fingerprint。失败、回滚和 rollback incident 都进入审计。RuntimeExecutor 不直接写 SQLite，使用注入的 audit/storage 接口。 |
| **D9 UI/IPC 投影** | UI 只能提交用户意图、审批决定、暂停/取消/接管请求；执行状态与证据由 RuntimeExecutor 通过结构化事件投影，UI 不自行推断权限或成功。 |
| **D10 可回放** | 时间、id、网络、Host IO、模型 Provider 和存储都由 trait 注入；回放执行器通过录制的 Observation/ToolEnvelope 驱动，不调用真实平台。 |

## 规范流程

`RuntimeExecutor::advance` 的固定顺序：

1. 读取最新 `TaskSnapshot`，确认任务 `Running` 且没有未完成的中断处理。
2. 选择 `ready_step_ids` 中的唯一步骤；多个 ready 时按现有确定性顺序选择一个。
3. `TaskEngine::begin_step`，进入 `Prechecking`。
4. Host/Platform 重新解析 `TargetDescriptor`，取得 pre-fingerprint 与 Observation 基础数据。
5. 构造 Policy `EvaluationContext`；Policy 决策：
   - `Deny` → 记录 policy denied，Step 不得执行；
   - `AllowWithConfirmation` → 创建 HITL request，等待匹配授权；
   - `Allow` → 继续。
6. 获取目标 Lease；写步骤必须 exclusive lease，冲突时按可读错误处理。
7. 写步骤在 Execute 前捕获 Undo Anchor，并把 anchor id 记入事件。
8. `TaskEngine::approve_step` 与 `begin_execute`。
9. 经 ToolBus 调用 Host handler；Host 在进程内完成定位与执行，只返回可序列化信封。
10. `TaskEngine::begin_verify`，根据 Host observation 调用 verify。
11. 只有收到 `VerificationReceipt` 才调用 `commit_verified_step`；否则按
    `on_violation` 进入 retry/fail/rollback/escalate，任何分支都不能 committed。
12. 成功提交后释放租约、记录 post fingerprint/证据/idempotency 信息，并推进
    task-level 完成或下一步。

## 错误映射

| 失败点 | 结果 |
|---|---|
| Policy deny | Step `PolicyDenied`；不调用 ToolBus；任务按策略升级为 `Failed` / `NeedsHuman` |
| Approval reject/expire | 不执行；返回 `UserRejected` / `ApprovalExpired`；任务状态显式终止或交人 |
| Lease conflict/timeout | 不执行；返回 `LockTimeout`；可按策略等待/改道 |
| ToolBus transport failure before boundary | 可重试；超预算后 `Failed` / `NeedsHuman` |
| ToolBus transport failure after possible side effect | `NeedsHuman`，保留 evidence；禁止重放 |
| Verify `Violated` | 不得 commit；按 `on_violation` 回滚、重试一次或不重试 |
| Verify `Inconclusive` | 不得 commit；默认升级人工 |
| Undo conflict/failure | 停止后续操作；生成 incident；`NeedsHuman` |

## 被否决的选项

| 选项 | 结论 | 理由 |
|---|---|---|
| 把 RuntimeExecutor 放进 `core` | ❌ | 与 ADR-0053 直接冲突，会让 core 依赖 policy/tool-bus/audit/hitl/verify/undo，破坏唯一装配点 |
| 用 `bool` / `String` / 自定义 JSON 表示验证结果 | ❌ | 可伪造、可漏传，无法在类型上阻止未验证提交 |
| 先 commit 再异步 verify | ❌ | 违反“写操作必须有 postcondition”，会产生已成功但未验证的状态 |
| Host 自行执行 Policy 判断 | ❌ | 违反铁律 3；Host 只能执行 Host 内定位与动作 |
| 未知执行边界后自动重试 | ❌ | 幂等性未知，可能重复写；必须交人 |

## 影响

- TASK-103 实现 `RuntimeExecutor`、Host 分发、VerifyReceipt 与 task-engine 提交门。
- TASK-104 在相同事件契约上接线 UI/IPC。
- TASK-105 用该链路运行 T1.1~T1.3 各 10 次。
- verify 新增 receipt 类型；task-engine 成功提交接口改为消费 receipt。
- 不改变 ToolBus 的“无策略判断”边界，也不改变 Policy 的“唯一放行点”。

## 验证方式

1. 没有 `VerificationReceipt` 时，代码无法调用 task-engine 成功提交。
2. Policy deny / Approval reject 路径断言 ToolBus 调用次数为 0。
3. 未知执行边界测试断言返回 `NeedsHuman` 且没有第二次工具调用。
4. Cancel、Pause、TakeOver 在 Step 边界有确定性测试。
5. Replay 测试只使用注入的时间、Host observation 与 ToolEnvelope。
6. TASK-105 的真实运行证据必须显示 verify 发生在 commit 之前。

## 重新评估触发条件

- 首个真实执行器无法在单进程 binary 装配层完成，需要独立 Host 调度器；
- ToolBus/Host 协议无法携带所需 evidence；
- VerifyReceipt 在持久化/恢复场景需要序列化；
- 出现同任务多步骤安全并发的明确需求。

## 相关 ADR

- ADR-0038：各 crate 声明自己的迁移，storage 只提供机制。
- ADR-0048：Policy confirmation 的无损投影。
- ADR-0053：core 依赖白名单与 binary 装配点。
- ADR-0055：ToolSchema 的 effect/reversibility 权威元数据。
