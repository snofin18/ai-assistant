# TASK-357 Formal UI approval session bridge

- 状态：**Done**
- 阶段：1　子阶段：1c　批次：T5.2 前置
- 依赖：TASK-355、TASK-356
- 本文件 = **卡片正文 + 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

## 目标

把现有 `PendingApprovals` / `step_approval_requested` 从离线 handler 测试推进到真实的 Core UI
approval session 编排：生产运行在 `AwaitingApproval` 后启动一个 one-shot UI session，推送 snapshot 与
pending approval 事件；UI approve/deny 后返回 Core，approve 恢复同一个 `ProductionRun` 的 Prechecking
step，deny 通过同一 TaskEngine 停止该 step。会话在首个有效 human decision 后结束，不在后台遗留等待循环。

本卡只验证现有 Notepad T1.2 approval 产品链，不宣称 TASK-057 的 browser/desktop 页面已关闭。

## In scope

- `apps/agent-core/src/ui_server.rs`：新增 until-human-decision 的 session 入口与内部终止条件；保持现有
  `serve` / `serve_with_events` 行为不变。
- `apps/agent-core/src/ui_events.rs`：组合 snapshot source 与 `PendingApprovalEventSource`，保证同一轮
  先推送已知 task snapshot，再推送 pending approval 事件。
- `apps/agent-core/src/production.rs`：新增暂停运行到 UI human decision 的编排方法；复用同一
  `TaskControlHandler<MemoryCheckpointStore>`、pending registry 与 grant table。
- `apps/agent-core/src/production_run.rs`：仅在编排确实需要时提供 crate 内拆分/重组支持，不暴露 handle。
- `apps/agent-core/src/main.rs`、`apps/agent-core/src/lib.rs`：生产入口调用新编排并导出必要封闭类型。
- `apps/agent-core/src/ui_server_tests.rs`、`apps/agent-core/tests/production_root_t1_2.rs`：覆盖 approve、
  deny、错误 request id、无 grant 不恢复、重复审批不重放、已有非 approval session 行为不回归。
- `tasks/TASK-357-formal-ui-approval-session.md`、`LEDGER.md`、`PLAN.md`、`README.md`、
  `plans/stage-1-pilots.md`、`docs/memory/facts.md`：状态与证据同步。

## Out of scope

- 不修改 Tool schema、UiEvent wire shape、UiCommand wire shape、DB schema、ErrorCode 或第三方依赖。
- 不修改 `apps/desktop-ui/**` 页面布局，不实现长期运行 UI 客户端重连。
- 不实现 browser adapter 进入 `ProductionHost`；TASK-057 / TASK-058 正文和状态不改。
- 不自动批准、不产生 persistent/session/task 级 browser submit 授权。
- 不访问真实站点、真实账号、Cookie、文件上传、支付或验证码。

## 必须遵守

1. one-shot session 只能在已经校验并执行的 `ApproveRequest` / `DenyRequest` outcome 后返回；心跳、
   非法载荷、其他命令或断连不得伪造 human decision。
2. Core 必须使用暂停运行自身的 engine 处理 deny，不得创建空 engine 假成功；approve 必须由同一
   pending registry 写入同一 task/step 的 bounded grant。
3. approve 后必须恢复原 Prechecking step；若同一 request 再次暂停，必须 fail-closed，禁止循环重放。
4. 所有等待和审批数量必须有界；超过计划步骤数仍未收敛即显式失败。
5. 现有 UI server 事件先于响应、correlation 精确回显、未知字段 fail-closed 的契约不得回归。

## 验收命令

```powershell
cargo test -p assistant-agent-core ui_server
cargo test -p assistant-agent-core production_t1_2_pauses_and_resumes_with_ui_approval
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
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

期望：approve 只恢复一次并完成原计划；deny 由同一 engine 转为拒绝状态；错误 request、重复恢复、
无 grant 恢复均显式失败；现有 UI 与 production 测试全绿。

## DoD

- [ ] one-shot UI session 只在有效 approve/deny outcome 后返回，非法消息与断连 fail-closed。
- [ ] 生产编排复用暂停运行自身的 engine、pending registry 与 grant table，不用空 engine 处理 deny。
- [ ] approve 恢复同一 Prechecking step；deny 走同一 engine；重复或无 grant 恢复不得重放工具。
- [ ] snapshot + pending approval 事件组合顺序确定，现有 serve/serve_with_events 行为不回归。
- [ ] 全部自动化门禁通过；TASK-057 / TASK-058 正文与状态未被本卡改写。

<!-- ══ 分界线：以上是**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下是 Implementer 填写。改动分界线以上任何一行 = 漂移触发器 ⑤，
     并会被 xtask card-check 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写，9 节骨架见 gov §3.4 / ADR-0031 D4）
### 1. 约束回执

```text
【任务】TASK-357 Formal UI approval session bridge
【目标】把生产运行暂停、one-shot UI human decision、同一 Prechecking step 恢复与同一 engine deny 串成封闭链路。
【write scope】仅：`apps/agent-core/src/{ui_server.rs,ui_server_tests.rs,ui_events.rs,production.rs,production_ui_approval.rs,main.rs,lib.rs}`、`apps/agent-core/tests/production_root_t1_2.rs`、本卡与 `LEDGER.md` / `docs/memory/facts.md` / `MEMORY.md`。
【铁律】无静默失败；UI 输入先校验；Policy 是唯一放行点；L3 审批必须人工；approve/deny 必须绑定同一 task/step/request；断连不得伪造决策。
【禁止】不自动批准、不持久化 grant、不新增 Tool/DB/IPC schema 或依赖、不改 desktop UI 布局、不改 TASK-057/058 正文与状态。
【验收】UI server 17/17、production T1.2 9/9、workspace 全绿、fmt/clippy、schema/codegen/hygiene/memory/adr/refscan/docscan/card/ledger/comments、deny 与 replay core 全绿。
【依赖】TASK-355、TASK-356 已 Done；TASK-057/058 保持既有状态。
【疑问】无；按已批准范围完成，无 DRIFT。
```

### 2. 实际改动文件

- `apps/agent-core/src/ui_server.rs`：新增 until-human-decision 入口；内部 session loop 只在已执行的 approve/deny outcome 后结束，断连显式失败。
- `apps/agent-core/src/ui_server_tests.rs`：覆盖 approve、deny 与无决策断连三条 one-shot 边界。
- `apps/agent-core/src/ui_events.rs`：新增有序 `CombinedEventSource`，先 drain snapshot，再 drain pending approval 事件，并补顺序测试。
- `apps/agent-core/src/production_ui_approval.rs`：新增生产审批编排；复用暂停运行的 engine、pending registry 与 grant table，限制审批次数并校验 request。
- `apps/agent-core/src/production.rs`：挂载审批编排子模块并保持既有 production root 职责边界。
- `apps/agent-core/src/main.rs`：`--serve-ui` 在生产运行暂停时调用 one-shot human decision session。
- `apps/agent-core/src/lib.rs`：导出组合事件源与 until-human-decision UI session 入口。
- `apps/agent-core/tests/production_root_t1_2.rs`：approve 连续恢复两个不同 Prechecking step；deny 使用同一 engine；无 grant、request 不匹配与伪造 deny 均 fail-closed。
- `tasks/TASK-357-formal-ui-approval-session.md`、`LEDGER.md`、`docs/memory/facts.md`、`MEMORY.md`：Review 证据与长期事实同步。

### 3. 验收输出摘要

- `cargo test -p assistant-agent-core ui_server` -> 17 passed / 0 failed。
- `cargo test -p assistant-agent-core --test production_root_t1_2` -> 10 passed / 0 failed。
- `cargo test --workspace` -> exit 0；agent-core 98 + 54 passed，xtask 510 passed，全部 doctest 通过。
- `cargo fmt --all --check` -> exit 0。
- `cargo clippy --all-targets -- -D warnings` -> exit 0；保留仓库既有 `unknown lint: clippy::assert_is_empty` 提示。
- `verify-schemas`、`codegen --check`、`hygiene`、`memory-counts`、`adr-index`、`refscan`、`docscan`、`card-check`、`check-ledger`、`check-comments` -> 均 PASSED。
- `cargo deny check` -> advisories / bans / licenses / sources 全部 ok。
- `cargo run -p xtask -- replay --suite core` -> 2 steps、5 changes、0 mismatch。
- Shell misuse：同一 user turn 机器统计 `shell="bash"` **22**、`shell="cmd"` **0**；22 次均为只读检索、文件读取、门禁或 guard 命令，未通过误填 shell 写入仓库。

### 4. DoD 逐条核对

- [x] one-shot UI session 只在有效 approve/deny outcome 后返回；断连、非法载荷与其他命令均不构成 human decision。
- [x] 生产编排复用暂停运行的自身 engine、pending registry 与 grant table；deny 不创建空 engine，并核验最终状态确实为 `Failed + PolicyDenied`。
- [x] approve 恢复原 Prechecking step；无 grant 或 request 不匹配显式失败；同一 request 不会依靠 grant 循环重放。
- [x] snapshot 与 pending approval 事件顺序固定；既有 serve / serve_with_events 测试无回归。
- [x] 全部自动化门禁通过；TASK-057 / TASK-058 正文与状态未改。

### 5. 偏差

- none。未超出已批准 write scope，未新增依赖、公共 wire schema 或桌面 UI 页面改动。

### 6. 更合理做法

- 将审批编排拆入 `production_ui_approval.rs`，避免 production root 继续膨胀并保持锁顺序可见。
- 使用 `FloorClock` 把 handler 时间下限固定在暂停快照之后，避免固定测试时钟在 deny 时倒退。
- 审批上限直接复用计划 step budget，不新增第二套等待计数配置。

### 7. 遗留问题

- 本卡只闭合 Notepad T1.2 的 Core UI approval session 与生产编排；真实 desktop UI 页面、长期重连和 browser adapter 进入 ProductionHost 不在本卡范围。
- TASK-057/058 仍保持原状态；下一张需在正式 UI session 之上接 T5.2 产品化与安全验收。

### 8. 新增长期记忆

- `FACT`：one-shot approval session 只有 handler 实际返回 `ApprovalGranted` 或 `ApprovalDenied` 才结束；断连必须返回 `Disconnected`，不能当作拒绝或批准。
- `FACT`：审批恢复必须复用暂停运行自身的 `TaskEngine`；deny 由该 engine 执行，approve 后同一 request 再次暂停必须 fail-closed。
- `MEMORY.md` 的 `facts.md` 行数与条目数已随新增 FACT 同步。

### 9. 给审阅者的关注点

- 重点审查 `drive_ui_approvals` 对 grant、pending、request id、deny 最终状态与 engine 生命周期的绑定。
- 重点审查 `serve_session_with_events_inner` 的提前返回条件，确保既有常驻 session 行为不变。
- 重点审查 `FloorClock` 是否只用于审批 handler，是否影响生产运行其余时间语义。

---

**2026-11-02 merge evidence**

- PR #687 checks **22**（21 success + 1 skipped，pending 0），Mergeable / Clean，base `main`；merge `fb76fa1`。
- post-merge main CI `41396116377` = **success**。
