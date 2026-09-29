# spec: 运行执行链路（runtime-execution）

> 摘要：定义 binary 装配层 `RuntimeExecutor` 的单步执行契约，串联 TaskEngine、
> Policy、HITL、Lease、ToolBus、Host、Verify、Undo 与 Audit。
> 状态：Draft（ADR-0056 Proposed）　版本：0.1　日期：2026-09-29
> 上位：`AGENTS.md` §2、架构 v2 §7 / §8 / §9 / §12、ADR-0053、ADR-0056、ADR-0055
> 强制性：ADR-0056 转 Accepted 后即作为实现契约；违反由 Reviewer 拒绝合并，能机器化的
> 部分由 TASK-103/105 的测试固定。
> 变更门槛：新增公开接口、改变提交门或错误语义 → 需 ADR。

---

## 1. 目标

1. 让 Plan 中的每个副作用 Step 只能沿唯一路径执行、验证和提交。
2. 让模型、UI、Host 和工具 handler 都不能绕过 Policy 或伪造验证成功。
3. 取消、超时、接管、Host 断连和崩溃恢复都以显式状态收场。
4. 让阶段 1a 的真实 Notepad 闭环可以在同一确定性执行器上复现和取证。

## 2. 范围

**管**：

- `RuntimeExecutor` 的步骤顺序、提交门和错误映射；
- VerifyReceipt 与 task-engine 提交关系；
- Lease/Anchor/Audit/HITL 的调用边界；
- Pause/Cancel/TakeOver/Recovery 的运行时语义；
- UI/IPC 可观察到的事件。

**不管**：

- 具体 UI 布局、文案与 i18n；
- 具体平台 provider / Adapter 实现；
- ToolBus 的参数 schema 校验；
- storage SQL、audit 表结构和模型 Provider 网络细节；
- 多任务并行调度和无人值守模式。

## 3. 组件所有权

| 组件 | 拥有什么 | 不拥有什么 |
|---|---|---|
| `RuntimeExecutor` | 运行顺序、组件调用、提交门、运行事件投影 | 不定义 Policy、不执行平台动作、不写 SQL |
| `TaskEngine` | Task/Step 状态、DAG、checkpoint、恢复分类 | 不执行工具、不判定 Policy、不产生观测 |
| `Policy` | 唯一 allow/deny/confirmation 决定 | 不执行动作、不处理审批 UI |
| `HITL` | 审批请求、授权范围、接管基线与决策 | 不放行 Policy deny、不执行工具 |
| `Lease` | 目标写/读租约与冲突 | 不定位目标、不写应用 |
| `ToolBus` | MCP 工具调用与信封 | 不做权限判断、不直接操作 UI |
| `Host` | 目标定位、平台动作、Observer 收集 | 不判定权限、不推进 Task 状态 |
| `Verify` | `VerifyOutcome` 与不可伪造 receipt | 不执行工具、不提交任务 |
| `Undo` | Anchor、rollback recipe、冲突与 incident | 不决定是否放行、不执行平台动作 |
| `Audit` | append-only 事件持久化 | 不改回状态、不替 RuntimeExecutor 决策 |

## 4. 运行前置条件

1. 任务已由 Planner 产生 `Plan` 并通过 `Plan::validate`。
2. 任务处于 `Running`，或由显式 Resume/Recovery 事件进入 `Running`。
3. Tool 目录、Policy RuleSet、Host handlers、Clock、IdGenerator、Audit sink 已注入。
4. 当前没有未处理的中断、待审批请求或活动 Step。
5. 目标应用绑定和 Capability Matrix 可用；缺失能力必须显式失败。

## 5. 单步执行契约

### 5.1 Resolve / Precheck

- Host 重新解析目标，不得复用跨进程 handle。
- 获取 pre-fingerprint；无法获取时必须显式 `NotEvaluable`，不能省略。
- 检查 Tool 前置条件、能力、预算、前台/焦点、目标版本与 interrupt。

### 5.2 Policy / Approval

- `EvaluationContext` 至少包含 effect、risk、reversibility、unattended、tainted、
  target_app、egress。
- `AllowWithConfirmation` 必须经过 HITL；`once`/scope/TTL 必须满足。
- `Deny` 路径的工具调用数必须为 0。

### 5.3 Lease / Anchor

- Read Step 可持 shared lease；Write/Destroy/Send 步骤必须持 exclusive lease。
- 需要跨多个资源的 Step 按稳定 key 顺序获取，失败时释放已获取租约。
- L0/L1/L2 写步骤在执行前建立 Anchor；L3 只允许 point-of-no-return 证据。
- Anchor/Lease 创建失败则不进入 Execute。

### 5.4 Execute

- 只经 ToolBus 调用注册工具。
- Host 在进程内定位和执行；跨进程只传可序列化参数/结果。
- 任何 ToolEnvelope 的 `untrusted`/`truncated` 标记必须保留。
- 到达执行边界后，缺少结果只能进入未知/交人，不得当成未执行。

### 5.5 Verify

- `verify_postconditions` 是所有成功提交的唯一验证入口。
- `Verified` 产生不透明的 `VerificationReceipt`。
- `Violated` / `Inconclusive` 不允许成功提交，并按 `on_violation` 分派。
- 验证所需 Observation 由 Host/Reader 提供；缺失即 `Inconclusive`。

### 5.6 Commit

- 成功提交接口必须接受 `VerificationReceipt`，不得同时提供绕过它的重载。
- 提交前再次校验 lease ownership、cancel/pause/takeover 状态和 fingerprint。
- 提交后记录 post fingerprint、验证 receipt 摘要、证据引用和 idempotency 信息。
- 审计写失败时任务不得对外报告成功。

## 6. 状态与事件

RuntimeExecutor 至少投影以下运行事件：

- `task_started` / `task_completed` / `task_failed` / `task_needs_human`
- `step_precheck_started` / `step_policy_decided` / `step_approval_requested`
- `step_execution_started` / `tool_result_received`
- `step_verification_started` / `step_verified` / `step_verification_failed`
- `step_committed` / `step_rolled_back`
- `lease_acquired` / `lease_released` / `lease_conflict`
- `anchor_created` / `rollback_started` / `incident_reported`
- `user_took_over` / `user_returned` / `task_paused` / `task_cancelled`

事件必须包含 task_id、step_id、时间、结果 code 与证据引用；不得包含凭据或未脱敏内容。

## 7. 控制语义

| 控制 | 语义 |
|---|---|
| Pause | 当前不可分割动作结束后停；不得在动作中途停 |
| Cancel | 阻止后续 Step；可逆已完成步骤按策略询问回滚 |
| TakeOver | 立即停手，释放输入/租约，进入 `TakenOver` |
| ReturnControl | 进入 `Resuming`，重新解析目标和指纹，不直接继续旧 handle |
| Timeout | 按 resolve/execute/verify 分阶段分类；未知边界交人 |
| Crash | 从 checkpoint 恢复；未知是否执行必须 `NeedsHuman` |

## 8. 错误映射与安全底线

- 所有错误必须带 `ErrorCode` 和可读说明。
- 未验证、未审批、lease 失败、句柄跨进程、审计失败一律 fail-closed。
- 不可逆动作永久禁止无人值守。
- Shell/通用代码执行不在工具目录内，RuntimeExecutor 无旁路执行入口。
- 验证失败不能被转化成 `CompletedWithWarnings`。

## 9. 与其他 spec 的关系

| 本 spec | 相关 spec | 关系 |
|---|---|---|
| Policy/Approval | `audit-event.md` §4.1 | PolicyDecision 的 confirmation 投影必须无损 |
| Execute | `envelope.md` | ToolEnvelope 是唯一工具返回形状 |
| Commit | `tool-schema.md` | 写步骤必须有 postconditions；effect/reversibility 来自 schema |
| Errors | `error-codes.md` | 不新增字符串错误码或 ErrorCategory |
| Tests | `testing.md` | 单元/契约/回放分层，禁止真实 IO 进单元测试 |

## 10. 验证清单

1. Policy deny / approval reject：ToolBus 调用数 = 0。
2. Verified：execute 先于 verify，verify 先于 commit。
3. Violated/Inconclusive：commit 调用数 = 0。
4. Lease 冲突/超时：动作不执行，错误可读。
5. Host 断连：边界前可重试，边界后 `NeedsHuman`。
6. Pause/Cancel/TakeOver：状态与事件一致，无隐藏继续执行。
7. Rollback：仅对 anchor/recipe 匹配步骤执行；冲突或失败产生 incident。
8. Audit：每次状态迁移和工具调用有对应事件，顺序可复核。
9. Replay：同一录制输入得到同一状态序列。

## 11. 变更历史

| 日期 | 变更 | 依据 |
|---|---|---|
| 2026-09-29 | 建立单步运行执行链路契约 | TASK-102 / ADR-0056（Proposed） |
