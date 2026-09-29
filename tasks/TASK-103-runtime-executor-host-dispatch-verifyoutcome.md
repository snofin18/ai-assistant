# TASK-103　真实任务执行器：Host 分发、上下文装配与 VerifyOutcome 接线

- 状态：**Ready**
- 阶段：1　子阶段：1a 补救　批次：A5-REMEDIATION　依赖：102
- 预估：L　难度：L
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。
- 关联：TASK-102 的 ADR/spec、`apps/agent-core/**`、`apps/automation-host/**`、`crates/task-engine/**`

## 目标

按 TASK-102 的契约实现一条真实的后端执行链：从 `Plan` / `Step` 进入 task-engine，
经 Policy 放行、ToolBus 调用、Host 分发、结果校验与 VerifyOutcome，再按结果推进状态；
失败、取消、超时和未知结果必须 fail-closed。

## In scope

- `apps/agent-core/src/**` 的执行器与装配代码。
- `apps/automation-host/src/**` 的请求分发。
- `crates/task-engine/**` 的 `VerifyOutcome` 接线。
- `crates/ipc/**` 的必要测试适配。
- 相关 README、测试与本卡记录。

## Out of scope

- UI、审批卡、桌面端 typed command。
- 真实 Notepad 运行和 10 次成功率报告。
- 新 Provider、新 MCP server 或新公共 schema。

## 必须遵守

- 以一个已声明步骤为最小闭环，不得一次实现多渠道抽象。
- task-engine 不得在缺 VerifyOutcome 时进入成功终态。
- Host 断连、未知响应、校验失败都返回带 ErrorCode 的失败。
- 所有超时可取消，取消后不得继续写。
- 不在 core 直接依赖平台实现或 IPC。

## 验收命令

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-agent-core
cargo test -p assistant-task-engine
cargo run -p xtask -- check-migrations
cargo run -p xtask -- hygiene
```

## 完成定义（DoD）

- [ ] Agent-core 能装配并执行一个真实 `Plan`。
- [ ] Host 能接收并分发工具调用，返回结构化结果。
- [ ] task-engine 强制接收 `VerifyOutcome` 后才推进成功。
- [ ] 断连、超时、拒绝、验证失败均有负向测试。
- [ ] 无静默失败、无句柄跨进程。
- [ ] README 与测试同步，验收全绿。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

```text
【任务】TASK-103 真实任务执行器：Host 分发、上下文装配与 VerifyOutcome 接线
【目标】实现一条真实后端执行链：Plan/Step → task-engine → Policy → ToolBus → Host → VerifyOutcome → 状态推进，失败/取消/超时/未知结果 fail-closed
【write scope】仅：apps/agent-core/src/**、apps/agent-core/tests/**、apps/agent-core/README.md、apps/automation-host/src/**、apps/automation-host/README.md、crates/task-engine/**、crates/ipc/**、crates/verify/**、本卡记录区、LEDGER.md、PLAN.md、README.md、plans/stage-1-pilots.md
【铁律】1 无静默失败（工具 ok=false 必须失败而非 verify）；3 Policy 唯一放行点（Host 不自行判断）；4 写操作必须有 postcondition；8 element/句柄不跨进程（只传可序列化信封）；10 契约先行（ADR-0056 / runtime-execution spec）
【禁止】UI、审批卡、桌面端 typed command；真实 Notepad 运行与 10 次成功率报告；新 Provider / 新 MCP server / 新公共 schema
【验收】cargo fmt/clippy/test --workspace、cargo test -p assistant-agent-core、-p assistant-task-engine、check-migrations、hygiene → 全绿
【依赖】TASK-102（ADR-0056 Accepted，已核对 LEDGER：Done）
【疑问】无；ADR-0056 三项关键设计已由人类接受，实现细节按 spec 默认处理
```

### 2. 实际改动文件

| 文件 | 改动 |
|---|---|
| `crates/verify/src/verdict.rs` | 新增不透明 `VerificationReceipt`（字段私有 / 无公开构造器 / 不实现反序列化）与 `verify_postconditions_with_receipt`；只有 `Verified` 才返回 receipt |
| `crates/verify/src/lib.rs` | 导出 `VerificationReceipt` 与 `verify_postconditions_with_receipt` |
| `crates/verify/tests/postcondition_evaluation.rs` | 补 receipt 只能由 `Verified` 铸出、`Violated` / `Inconclusive` 返回原 outcome 的负向用例 |
| `crates/task-engine/src/commit.rs`（新增） | `StepCommit` 值对象：私有字段 + 消费式 receipt；`into_parts` 仅 crate 内可见 |
| `crates/task-engine/src/engine.rs` | `commit_step` 改收 `StepCommit`（消费 receipt）；保留 `is_verified` 防御性复核 → `UnverifiedCommit` |
| `crates/task-engine/src/error.rs` | 新增 `UnverifiedCommit` 变体 → `ErrorCode::VerifyFailed` |
| `crates/task-engine/src/lib.rs` | 声明 `mod commit` 并导出 `StepCommit` |
| `crates/task-engine/tests/{engine_commands,budget_watchdog}.rs`、`tests/common/mod.rs` | 改用 `StepCommit` + 真实 `verified_receipt()` fixture |
| `crates/ipc/src/handshake.rs` | 为 `RequestMessage` / `ResponseMessage` 增加构造器，绕过 `#[non_exhaustive]` 的外部构造限制 |
| `apps/automation-host/src/lib.rs` | 新增 `RequestDispatcher` trait、`run_host_with_dispatcher`、`process_request`（校验 correlation）；会话循环对 `Request` 回 `Response`，其余消息 fail-closed；`run_session`/`send_heartbeat` 泛化为 `T: Transport` |
| `apps/automation-host/tests/*`、`src/lib.rs` 测试 | 新增分发匹配 / correlation 不匹配 / 真实会话循环（脚本化 Transport）三个用例 |
| `apps/agent-core/src/runtime.rs`（新增） | `RuntimeExecutor`：Policy → Approval → Execute → Observe → Verify → Commit 单步顺序；`ToolInvoker`（异步 `impl Future`）、`ObservationCollector`、`StepPolicy`；真实 `ToolBusInvoker` 与 `EnvelopeObservationCollector`；`StepExecutionOutcome`（含 `ToolFailed` / `NeedsHuman`） |
| `apps/agent-core/src/lib.rs` | 导出 runtime 模块类型 |
| `apps/agent-core/tests/runtime_execution.rs`（新增） | 6 个契约用例：成功、Policy deny、审批等待、验证失败、未知工具结果、工具错误信封 |
| `apps/agent-core/tests/runtime_toolbus.rs`（新增） | 真实端到端：真 Plan → 真 TaskEngine → 真 ToolBus（MCP 往返）→ 真 receipt → commit |
| 四个 `README.md` | agent-core / automation-host / task-engine / verify 职责与不变量同步 |

### 3. 验收输出摘要

```text
cargo fmt --all --check                         → PASS（0 diff）
cargo clippy --all-targets -- -D warnings       → PASS（exit 0）
cargo test --workspace                          → PASS（999 passed / 0 failed）
cargo test -p assistant-agent-core              → PASS（含 runtime_execution 6 + runtime_toolbus 1）
cargo test -p assistant-task-engine             → PASS
cargo run -p xtask -- check-migrations          → PASS（4 文件 / 4 登记）
cargo run -p xtask -- hygiene                   → PASS（scanned=286，0 error，4 warning —— 与基线一致）
cargo run -p xtask -- verify-schemas / codegen --check → PASS
memory-counts / adr-index / check-ledger / card-check / docscan / refscan → 全 PASS（0 error）
```

合并证据：PR #98（base `main`）—— push run `36519313243` 与 pull_request run `36519337842` 均 `completed/success`，各 9/9 job success，PR 汇总 18 个 status context 全 success；合并前 `mergeable=MERGEABLE` / `merge_state_status=CLEAN`；merge commit `16c9709`。

### 4. DoD 逐条核对

- [x] **Agent-core 能装配并执行一个真实 `Plan`**：`runtime_toolbus.rs` 用真实 `ToolBus`（`rmcp` 同进程 MCP 往返，handler 计数断言恰好 1 次）驱动真实 `TaskEngine`，验证 `Committed` 且写入 post fingerprint。
- [x] **Host 能接收并分发工具调用，返回结构化结果**：`RequestDispatcher` + `process_request` 校验 correlation；会话循环对 `Request` 回 `Response`（脚本化 Transport 用例覆盖整条 recv→dispatch→send）。
- [x] **task-engine 强制接收 `VerifyOutcome` 后才推进成功**：`commit_step` 只接受持有 `VerificationReceipt` 的 `StepCommit`；receipt 字段私有、无公开构造器、不可反序列化，只有 `Verified` 能铸出。
- [x] **断连、超时、拒绝、验证失败均有负向测试**：Transport 失败 → `NeedsHuman`（无第二次调用）；工具 `ok=false` → `ToolFailed`（不进入 verify）；Policy deny / 审批等待 → 0 次工具调用；验证失败 → 不 commit。
- [x] **无静默失败、无句柄跨进程**：所有失败路径返回带 `ErrorCode` 的 `TaskEngineError` / `RuntimeExecutionError`；跨边界只传 `ToolEnvelope`（可序列化），无平台句柄。
- [x] **README 与测试同步，验收全绿**：四个 README 与新增模块/不变量同步；上表命令全绿。

### 5. 偏差

none。ADR-0056 的三项关键设计（执行器归 binary 装配层 / `VerificationReceipt` 唯一提交凭据 / 未知边界强制 `NeedsHuman`）按 spec 实现，未新增 ADR 级公共接口语义。

### 6. 更合理做法

`StepCommit` 消费式 receipt 比"传 `&VerificationReceipt` + 布尔"更强：同一份证明无法重放第二次提交。`ToolInvoker` 用显式 `impl Future` 而不是 `async fn` in trait，避免公共 trait 触发 `async_fn_in_trait` 而被迫加 `#[allow]`。

### 7. 遗留问题

- `VerificationReceipt` 未绑定 task/step，理论上可跨步骤重放同一份 receipt（当前由执行器单步顺序缓解）；如后续需要，按 ADR 扩展 receipt 载荷。
- 真实 Host handler 尚未注入 `automation-host` 二进制：默认 `run_host` 对 `Request` fail-closed，装配层须经 `run_host_with_dispatcher` 注入 handler（TASK-104/105）。
- 审批 / 租约 / 撤销 / 审计事件尚未接入 `RuntimeExecutor`（属 TASK-104 及后续）；本卡只闭合最小单步链路。
- `agent-core` 的 `HostComponents` 尚未暴露"用已装配 ToolBus 构造 `RuntimeExecutor`"的入口，端到端装配留待 TASK-104/105 收口。

### 8. 新增长期记忆

无新增 `docs/memory/*` 条目；实现边界已写入 ADR-0056 / `docs/spec/runtime-execution.md` 与四个 crate README。

### 9. 给审阅者的关注点

1. `commit_step` 的公共签名从 `&VerificationReceipt` 改为消费 `StepCommit` —— 是否认可这是"无未验证提交"的最小充分门。
2. `RuntimeExecutor::advance` 为异步（await 真实 `ToolBus`）；请确认调用侧（TASK-104）在 tokio 运行时内驱动。
3. 工具 `ok=false` 映射为 `ToolFailed`（带信封 `ErrorCode`）而非 `NeedsHuman`；未知/传输失败才升级人工 —— 请确认这与 spec §8 的错误映射一致。
