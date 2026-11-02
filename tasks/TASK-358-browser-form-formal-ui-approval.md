# TASK-358 Browser form formal UI approval session

- 状态：**Done**
- 阶段：1　子阶段：1c　批次：T5.2 前置
- 依赖：TASK-344、TASK-350、TASK-352、TASK-355、TASK-356、TASK-357
- 本文件 = **卡片正文 + 执行记录**（ADR-0031「一卡一文件」）。分界线以上是正文（Orchestrator 所有，Implementer 只读）；以下是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

## 目标

把 browser task runner 的 `form_submit` 终端审批入口推进到真实 one-shot UI IPC
session：暂停时启动 `serve_until_human_decision_with_events`，推送已知 snapshot 与
`step_approval_requested`；UI approve 后恢复同一 Prechecking step，deny 由同一
`TaskEngine` 停止，断连、错误 request 或未落地的 outcome 均 fail-closed。

本卡只闭合 Core 侧 browser runner 的正式 UI 审批后端，不实现 desktop-ui 页面或
Tauri 事件桥，也不把 TASK-057 / TASK-058 标为 Done。

## In scope

- `apps/agent-core/src/browser_task_runtime.rs`：新增 `--ui-approval` 编排；在
  `AwaitingApproval` 时复用暂停运行自身的 engine、pending registry 与 grant table；
  approve 恢复原 step，deny 在返回前核验 `Failed + PolicyDenied`。
- `apps/agent-core/src/browser_task_support.rs`：解析并校验 `--ui-approval`、
  `--ui-pipe`、`--ui-token-env`、`--ui-peer`，与 `--human-approval` 互斥。
- `apps/agent-core/src/browser_task_ui_approval.rs`（新增）：封装 combined snapshot /
  pending event source、`TaskControlHandler` 与正式 UI server 调用，便于独立测试。
- `apps/agent-core/src/browser_task_runtime_tests.rs`、
  `apps/agent-core/src/browser_task_ui_approval_tests.rs`（新增）：覆盖 approve、deny、
  无决策、错误 request、无 grant、重复恢复不重放，以及 CLI 参数正负边界。
- `tasks/TASK-358-browser-form-formal-ui-approval.md`、`LEDGER.md`、`docs/memory/facts.md`：
  Review 状态与证据同步；Done 后按常规回填 `PLAN.md`、`README.md`、
  `plans/stage-1-pilots.md`。

## Out of scope

- 不修改 `apps/desktop-ui/**` 页面、Tauri command 或事件订阅桥。
- 不修改 Tool/UI IPC/DB schema、ErrorCode、第三方依赖或平台 trait。
- 不自动批准、不预批准 `form_submit`、不允许 persistent/session/task scope。
- 不访问外部站点、真实账号、Cookie、文件上传、支付或验证码。
- 不改 TASK-057 / TASK-058 正文与状态。

## 必须遵守

1. only valid `ApprovalGranted` / `ApprovalDenied` outcome 可结束 one-shot session；
   断连、心跳失败、非法命令和普通 task control 命令都不得伪造 human decision。
2. approve 必须由同一 pending registry 写入同一 task/step 的 bounded once grant；
   deny 必须由暂停运行自身的 `TaskEngine` 执行。
3. approve 后必须从原 Prechecking step 恢复；若同一 request 再次暂停，必须显式失败。
4. UI session 阻塞期间 BrowserRuntime / ToolBus 生命周期不得被提前释放；失败路径仍要
   关闭 ToolBus、浏览器进程树并释放 pending/approval 状态。
5. `--ui-approval` 必须要求 pipe、token 环境变量和至少一个 allow-listed peer image；
   缺失或与 `--human-approval` 同时出现时在启动浏览器前失败。

## 验收命令

```powershell
cargo test -p assistant-agent-core browser_task_ui_approval
cargo test -p assistant-agent-core browser_task_runtime
cargo test -p assistant-agent-core ui_server
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

期望：scripted UI approve 后 `form_submit` 恰好调用一次并完成任务；deny 调用为零；
断连、错误 request、无 grant、伪造 deny 与同 request 再暂停全部显式失败。

## DoD

- [ ] browser runner 可在 `form_submit` 前启动 one-shot formal UI session。
- [ ] approve 复用同一 engine/pending/grants 并恢复原 Prechecking step。
- [ ] deny 由同一 engine 落地为 `Failed + PolicyDenied`，提交调用为零。
- [ ] UI 参数 fail-closed，且现有 `--human-approval` / `--approve-step` 行为不回归。
- [ ] 全部门禁通过；desktop-ui、Tool/DB/IPC schema、依赖与 TASK-057/058 未被改写。

<!-- ══ 分界线：以上是**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下是 Implementer 填写。改动分界线以上任何一行 = 漂移触发器 ⑤，
     并会被 xtask card-check 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写，9 节骨架见 gov §3.4 / ADR-0031 D4）
### 1. 约束回执

```text
【任务】TASK-358 Browser form formal UI approval session
【目标】让 browser task runner 在 form_submit 暂停后启动真实 one-shot UI IPC session，并用同一 engine 恢复或拒绝。
【write scope】仅：`apps/agent-core/src/{browser_task_runtime.rs,browser_task_support.rs,browser_task_ui_approval.rs,browser_task_runtime_tests.rs,browser_task_ui_approval_tests.rs}`、本卡、`LEDGER.md`、`docs/memory/facts.md`、`MEMORY.md`。
【铁律】无静默失败；UI 输入先校验；Policy 是唯一放行点；L3 仅 once 人工确认；同一 engine/pending/grants；断连不得伪造决策。
【禁止】不改 desktop-ui/Tauri、Tool/DB/IPC schema、ErrorCode、依赖或平台 trait；不访问外部站点；不改 TASK-057/058。
【验收】browser UI approval 7/7、browser runtime 50/50、ui_server 17/17、workspace、fmt/clippy、UI lint/typecheck/test、schema/codegen/hygiene/memory/adr/refscan/docscan/card/ledger/comments、deny、replay core。
【依赖】TASK-344 / 350 / 352 / 355 / 356 / 357 已 Done。
【疑问】无；TASK-057 仍为旧占位卡，本卡不修改其正文或状态。
```

### 2. 实际改动文件

- `apps/agent-core/src/browser_task_support.rs`：新增 `--ui-approval`、`--ui-pipe`、`--ui-token-env`、`--ui-peer` 参数与 fail-closed 校验；与终端审批互斥。
- `apps/agent-core/src/browser_task_ui_approval.rs`（新增）：封装 terminal/UI approval mode、combined event source、正式 UI server 调用与 executor 重建。
- `apps/agent-core/src/browser_task_runtime.rs`：暂停时复用同一 engine/pending/grants；UI approve 后从原 Prechecking step 恢复，deny 核验 `Failed + PolicyDenied`。
- `apps/agent-core/src/browser_task_runtime_tests.rs`：扩展 scripted browser form 输出与 UI test seam，保持无浏览器、无 pipe。
- `apps/agent-core/src/browser_task_ui_approval_tests.rs`（新增）：覆盖 approve、deny、无 grant、错误 request、断连和参数互斥。
- `tasks/TASK-358-browser-form-formal-ui-approval.md`、`LEDGER.md`、`docs/memory/facts.md`、`MEMORY.md`：Review 状态与长期事实同步。

### 3. 验收输出摘要

- `cargo test -p assistant-agent-core browser_task_ui_approval` -> 7 passed / 0 failed。
- `cargo test -p assistant-agent-core browser_task_runtime` -> 50 passed / 0 failed。
- `cargo test -p assistant-agent-core ui_server` -> 17 passed / 0 failed。
- `cargo test --workspace` -> exit 0；agent-core 98 + 61 passed，xtask 510 passed，全部 doctest 通过。
- `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings` -> exit 0；保留仓库既有 `unknown lint: clippy::assert_is_empty` 提示。
- UI `lint` / `typecheck` / `test` -> 全绿；model 85/85、DOM 3/3。
- schema/codegen/hygiene/memory/adr/refscan/docscan/card/ledger/check-comments/deny/replay core -> 均 PASSED。
- Shell misuse：同一 user turn 机器统计 `shell="bash"` **9**、`shell="cmd"` **0**；均为只读检索或门禁命令，未通过误填 shell 写入仓库。

### 4. DoD 逐条核对

- [x] browser runner 在 `form_submit` 前可启动 authenticated one-shot formal UI session。
- [x] approve 复用同一 engine、pending registry 与 grant table，并从原 Prechecking step 恢复。
- [x] deny 由同一 engine 落地为 `Failed + PolicyDenied`，不会调用提交工具。
- [x] UI 参数 fail-closed；terminal 模式与既有 preapproval 行为无回归。
- [x] 全部门禁通过；desktop-ui、schema、依赖与 TASK-057/058 未改。

### 5. 偏差

- none。未超出已批准 write scope，未修改公共 wire schema、依赖或桌面 UI。

### 6. 更合理做法

- 将 terminal 与 formal UI 的暂停处理集中到 `browser_task_ui_approval.rs`，避免主 runner 文件超过硬上限。
- formal UI 在暂停时重新发布当前 snapshot，再 drain pending approval event，确保 UI 看到的是同一个冻结 step。
- 用现有 bus/dataflow/recorder/binding state 重建 executor，不新增 `RuntimeExecutor` 公共 API。

### 7. 遗留问题

- desktop-ui 尚未订阅此 one-shot event stream，也没有 Tauri command/event bridge；需要下一张卡承接真实页面闭环。
- 本卡只证明 Core 侧浏览器 runner 的 formal UI approval 后端，不将 TASK-057/058 标为 Done。

### 8. 新增长期记忆

- `FACT`：browser task runner 可在 `--ui-approval` 下暂停到正式 one-shot UI 决策；approve/deny 使用同一 engine、pending 与 grants，并通过同一 bus/dataflow 重建 executor。
- `MEMORY.md` 的 `facts.md` 行数与条目数已同步。

### 9. 给审阅者的关注点

- 重点审查 `browser_task_ui_approval.rs` 的 executor 重建是否遗漏 runtime 组件或放宽 approval 条件。
- 重点审查 UI 与 terminal 模式互斥、peer/token 参数在启动浏览器前失败。
- 重点审查 deny 的 `Failed + PolicyDenied` 核验以及无 grant 不会导致 form_submit 重放。

---

**2026-11-02 merge evidence**

- PR #689 checks **22**（21 success + 1 skipped，pending 0），Mergeable / Clean，base `main`；merge `44d2ce4`。
- post-merge main CI `41404030945` = **success**（Windows 11m36s / macOS 3m5s / Ubuntu 3m45s，0 failed）。
- 同一 user turn 最终 shell misuse：`shell="bash"` **9**、`shell="cmd"` **0**；均为只读或门禁命令，未通过误填 shell 写入仓库。
