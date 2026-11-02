# TASK-355 Browser form approval runtime registration

- 状态：**Review**
- 阶段：1　子阶段：1c　批次：T5.2 前置
- 依赖：TASK-354
- 本文件 = **卡片正文 + 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

## 目标

把 TASK-354 已构造并校验的 `step_approval_requested` 登记到现有
`PendingApprovals`，让 UI 看到的 `request_id`、task、step 与 once scope
命中同一份待审批记录；同时提供有界、一次性 drain 的 UI event source。
本卡只完成登记与实时事件源，不执行 `form_submit`，不消费
`ApprovalGrants` 的运行时提交路径。

## In scope

- `apps/agent-core/src/ui_control.rs`：有界 pending registry、事件登记、过期拒绝、
  request/task/step/scope 一致性校验与 approve/deny 清理。
- `apps/agent-core/src/ui_events.rs`：从共享 `PendingApprovals` 一次性 drain
  `step_approval_requested` 的 `UiEventSource`。
- `apps/agent-core/src/browser_task_runtime.rs` 与
  `apps/agent-core/src/browser_task_approval_event.rs`：在 `form_submit` policy
  求值前把已构造事件登记到 recorder 共享的 pending registry。
- `apps/agent-core/src/browser_task_runtime_tests.rs`：登记、事件 drain、
  request id 对齐、重复/过期/非法组合负向测试。
- `apps/agent-core/src/lib.rs`：导出新的 event source。
- `apps/agent-core/tests/ui_ipc.rs`：UI approve 命中同一 task/step 的契约测试。
- `tasks/TASK-355-browser-form-approval-runtime-registration.md`、`LEDGER.md`、
  `PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/memory/facts.md`、
  `MEMORY.md`（仅计数变化时）：状态与进展同步。

## Out of scope

- 不执行 `form_submit`、不启动真实 Edge、不跑 loopback 任务链。
- 不修改 `ApprovalGrants` 的 grant/consume 语义，不新增通用点击工具。
- 不修改 `form_read` / `form_fill` / `form_submit` Tool schema、DB schema、
  ErrorCode 或 `UiEvent` 线上形状。
- 不新增 ADR、第三方依赖、数据库表或持久化审批。
- 不修改 TASK-057 / TASK-058 的正文或状态。

## 必须遵守

1. 只有 `StepApprovalRequested` 可登记；`request_id` 必须等于
   `runtime:<task_id>:<step_id>`，且 task/step 必须能解析为现有 ID。
2. 只接受 `risk=high`、`reversibility=l3_irreversible`、
   `scope_options=["once"]`、非空字段 diff 和未来 expiry。
3. pending registry 与事件队列必须有硬上限；达到上限时拒绝新请求，不得静默
   淘汰仍在等待的决定。
4. 重复 `request_id`、过期事件、非 once scope、字段组合不合法一径 fail-closed。
5. approve/deny 必须清理同一 request 的 queued event；UI event source 每次只
   drain 一次，不把同一卡片重复推送。
6. UI approve 仍只生成既有 `ApprovalGrants` 的一次性 grant；Policy 仍是唯一
   放行点，本卡不得在 event source 或 recorder 中直接放行工具。

## 验收命令

```powershell
cargo test -p assistant-agent-core browser_task_approval
cargo test -p assistant-agent-core browser_task_runtime
cargo test -p assistant-agent-core --test ui_ipc
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
pnpm --dir apps/desktop-ui lint
pnpm --dir apps/desktop-ui typecheck
pnpm --dir apps/desktop-ui test
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-comments
cargo deny check
cargo run -p xtask -- replay --suite core
```

## DoD

- [ ] `step_approval_requested` 在 form submit policy 求值前登记到有界 pending registry。
- [ ] UI event source 能 drain 同一事件一次，approve/deny 后不再保留 stale event。
- [ ] 重复 request id、过期事件、非 once scope、request/task/step 不一致全部 fail-closed。
- [ ] UI approve 仍只写入既有一次性 grant，未绕过 Policy 或执行提交。
- [ ] 全部门禁命令通过；TASK-057 / TASK-058 状态与正文未被本卡改写。

<!-- ══ 分界线：以上是**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下是 Implementer 填写。改动分界线以上任何一行 = 漂移触发器 ⑤，
     并会被 xtask card-check 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写，9 节骨架见 gov §3.4 / ADR-0031 D4）
### 1. 约束回执

```text
【任务】TASK-355 Browser form approval runtime registration
【目标】把已验证的 step_approval_requested 登记为同一 request_id 的有界 pending approval，并提供一次性 UI event source。
【write scope】仅：本卡、agent-core ui_control/ui_events/browser_task_runtime/browser_task_approval_event/runtime tests/lib、ui_ipc test、状态同步文件。
【铁律】无静默失败；UI/IPC 输入不可信；Policy 唯一放行点；L3 不可逆仅 once；不得静默扩大范围；契约先行。
【禁止】不执行 form_submit、不启动真实 Edge、不改 Tool/DB/ErrorCode schema、不加依赖/ADR、不改 TASK-057/058 正文。
【验收】专项 event/registry/UI tests；workspace + fmt + clippy；schema/codegen/hygiene/card/doc/ledger/comment/deny/replay 全绿。
【依赖】TASK-354 Done；TASK-057/058 保持 Blocked。
【疑问】无；采用既有 PendingApprovals / ApprovalGrants / UiEventSource，不另造审批机制。
```

### 2. 实际改动文件

- `apps/agent-core/src/ui_control.rs`：有界 pending registry、事件登记、过期拒绝、
  request/task/step/scope 校验与 approve/deny 清理。
- `apps/agent-core/src/ui_events.rs`：新增 `PendingApprovalEventSource`，一次性
  drain `step_approval_requested`。
- `apps/agent-core/src/browser_task_runtime.rs`、
  `apps/agent-core/src/browser_task_approval_event.rs`：在 `form_submit` policy
  求值前登记同一事件。
- `apps/agent-core/src/browser_task_runtime_tests.rs`、`apps/agent-core/tests/ui_ipc.rs`：
  登记、drain、同 task/step 授权、过期、重复、非 once、registry 上限测试。
- `apps/agent-core/src/lib.rs`：导出 `PendingApprovalEventSource` 与
  `MAX_PENDING_APPROVALS`。
- `tasks/TASK-355-browser-form-approval-runtime-registration.md`、`LEDGER.md`、
  `docs/memory/facts.md`、`MEMORY.md`：卡状态、审计与事实计数同步。

### 3. 验收输出摘要

- `cargo test -p assistant-agent-core browser_task_approval` → 11 passed。
- `cargo test -p assistant-agent-core browser_task_runtime` → 38 passed。
- `cargo test -p assistant-agent-core --test ui_ipc` → 18 passed。
- `cargo test --workspace` → 510 passed / 0 failed；doctests 全绿。
- `cargo fmt --all --check` → exit 0。
- `cargo clippy --all-targets -- -D warnings` → exit 0；仅仓库既存
  `unknown lint: clippy::assert_is_empty` 提示。
- `pnpm --dir apps/desktop-ui lint / typecheck / test` → 全绿；model 85 + DOM 3 passed。
- `verify-schemas` / `codegen --check` / `hygiene` / `memory-counts` /
  `adr-index` / `refscan` / `docscan` / `card-check` / `check-ledger` /
  `check-comments` / `cargo deny check` / `replay --suite core` → 全部 PASSED。

### 4. DoD 逐条核对

- [x] `step_approval_requested` 在 form submit policy 求值前登记到有界 pending registry。
- [x] UI event source 能 drain 同一事件一次，approve/deny 后不再保留 stale event。
- [x] 重复 request id、过期事件、非 once scope、request/task/step 不一致全部 fail-closed。
- [x] UI approve 仍只写入既有一次性 grant，未绕过 Policy 或执行提交。
- [x] 全部门禁命令通过；TASK-057 / TASK-058 状态与正文未被本卡改写。

### 5. 偏差

- 同一 user turn 的显式 `shell="bash"` 误填机器审计值为 **36**，`shell="cmd"` 为 **0**；
  已知误填均为只读/门禁/状态查询命令，未通过 bash 写入仓库。后续命令全部显式 PowerShell。

### 6. 更合理做法

- 复用既有 `PendingApprovals` 与 `UiEventSource`，只把事件队列和有界校验放进同一把锁，
  避免维护两套审批真源。
- 对 event 来源的 pending approval 同时保存 expiry，并在 UI approve 时按剩余寿命授予
  once grant；过期请求在生成 grant 前被移除。

### 7. 遗留问题

- 本卡只完成 runtime 登记与 UI event source；真实 UI session 组装、once grant 在
  `form_submit` 路径的端到端消费、loopback 任务链仍由下一张卡承接。

### 8. 新增长期记忆

- `FACT`：browser form approval event 已接入有界 pending registry 与一次性 UI source；
  同一 request id 绑定 task/step/once/high/L3/expiry，approve 后清理未投递卡片。

### 9. 给审阅者的关注点

- 重点检查 `PendingApprovals::register_event` 是否只接受封闭 `step_approval_requested`
  组合，且 registry 达到上限时拒绝而不是淘汰仍在等待的决定。
- 重点检查 `TaskControlHandler::approve_request` 是否仍只写 `ApprovalGrants`，没有在
  UI source 或 recorder 中直接放行工具。
- 重点检查 runtime 是否在 policy 求值前登记事件，且 event source drain 不重复推送。
