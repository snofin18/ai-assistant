# TASK-363 desktop-ui single-session approval transport

- 状态：**Review**
- 阶段：1　子阶段：1c　批次：T5.2 产品化前置
- 依赖：ADR-0057、TASK-357、TASK-358、TASK-359、TASK-361
- 本文件 = 卡片正文 + 执行记录（ADR-0031）。

---

## write scope

- `apps/desktop-ui/src-tauri/src/core_pipe.rs`
- `apps/desktop-ui/src-tauri/src/core_pipe_session.rs`
- `apps/desktop-ui/src-tauri/src/core_pipe_session_tests.rs`
- `apps/desktop-ui/src-tauri/src/event_bridge.rs`
- `apps/desktop-ui/src-tauri/src/lib.rs`
- `apps/desktop-ui/README.md`
- `tasks/TASK-363-desktop-ui-single-session-approval-transport.md`
- `docs/memory/facts.md`
- `docs/memory/pitfalls.md`
- `PLAN.md`
- `plans/stage-1-pilots.md`
- `README.md`
- `LEDGER.md`

## 目标

修复 desktop-ui 正式审批链路的会话边界：事件订阅、心跳和审批命令必须复用同一条已经
完成握手与鉴权的 NamedPipe 会话，使 `step_approval_requested` 到达 UI 后，UI 的
approve/deny 命令能回到同一个 Core 审批会话。不得通过另开连接、自动重试或放宽 Core
审批判据绕过该缺口。

## In scope

- 在 desktop-ui 进程内维护一个持久 Core UI 会话，统一处理 Heartbeat、UiEvent 和 UiResponse。
- 事件流保持单实例；同一时刻最多一个审批命令在途，不存在的 second command 显式拒绝。
- 心跳按已协商 session id 回送；错误 session id、未知帧和错 correlation fail-closed。
- 命令请求发送后只等待同一 correlation 的响应；超时、断连和错 correlation 不自动重放。
- 用真实 NamedPipe 测试证明事件先于响应、命令与事件共用同一连接、心跳不会终止事件流。
- 更新 desktop-ui README 与长期记忆，明确“单会话双向复用”不变量。

## Out of scope

- 不修改 `UiIpcRequest` / `UiIpcResponse` / `UiIpcEvent` wire schema、字段或版本。
- 不修改 Core 的 `UiServerConfig`、审批状态机、Policy、Tool schema、DB schema 或 ErrorCode。
- 不启动真实浏览器、不填写或提交真实表单、不执行 L3 审批。
- 不改变 TASK-057 / TASK-058 的状态或正文。
- 不新增第三方依赖。

## 必须遵守

1. 事件、命令和心跳只能通过同一个 authenticated session；断连后不得自动重连并重放命令。
2. 只接受当前 session id 的 live heartbeat；UiResponse 必须匹配唯一在途 correlation id。
3. 事件流终止、心跳失败、错帧和超时都必须显式上报，不得伪装成 idle。
4. token 只能从环境变量读取，不进入日志、错误消息、事件或命令行导出。
5. 不改变 Core 的单审批会话语义；UI 侧适配该语义，而不是要求 Core 放宽。

## 验收命令

```powershell
cargo fmt --manifest-path apps/desktop-ui/src-tauri/Cargo.toml --all --check
cargo clippy --manifest-path apps/desktop-ui/src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path apps/desktop-ui/src-tauri/Cargo.toml
pnpm --dir apps/desktop-ui lint
pnpm --dir apps/desktop-ui typecheck
pnpm --dir apps/desktop-ui test
pnpm --dir apps/desktop-ui build
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-comments
cargo deny check
```

期望：同一 NamedPipe 会话上心跳持续、审批事件可投递、approve/deny 响应可回送；
断连、错 correlation、错 session id 与超时均显式失败；全部门禁通过。

## DoD

- [ ] desktop-ui 只建立一条 authenticated Core UI 会话，事件与命令共用该会话。
- [ ] 心跳、事件、响应的分发均有 focused test，错误消息不含 token。
- [ ] 真实 NamedPipe 集成测试证明事件与命令同连接且 correlation 精确匹配。
- [ ] desktop-ui 专项、UI checks、workspace 与全部文档/安全门禁通过。
- [ ] PR 合并并记录 checks / merge / post-merge main CI；进度文件同步 Done。

<!-- ══ 分界线：以上是**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下是 Implementer 填写。改动分界线以上任何一行 = 漂移触发器 ⑤。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-363 desktop-ui single-session approval transport
【目标】让事件、心跳和审批命令复用同一条 authenticated Core UI NamedPipe 会话
【write scope】仅：desktop-ui core_pipe/event_bridge/lib、本卡、真实管道测试、README、facts/pitfalls、PLAN/plans/README/LEDGER
【铁律】无静默失败；UI/Core 输入先校验；策略引擎唯一放行；L3 不自动批准；契约先行
【禁止】不改 wire schema、Core 审批状态机、依赖、TASK-057/058；不启动真实表单或代替人类批准
【验收】desktop-ui fmt/clippy/test + 真实 NamedPipe 测试；UI checks；workspace 与文档/deny 全绿
【依赖】ADR-0057、TASK-357/358/359/361 已核对为 Done
【疑问】无；按现有单会话协议修正 UI 客户端，不放宽 Core
```

### 2. 实际改动文件

- `apps/desktop-ui/src-tauri/src/core_pipe_session.rs`：新增单会话 worker；在一个已握手连接上
  分发 Heartbeat、UiEvent 与 UiResponse，心跳回送，命令有界串行且不自动重放。
- `apps/desktop-ui/src-tauri/src/core_pipe_session_tests.rs`：真实 NamedPipe 契约测试，
  覆盖心跳、审批事件、同连接 command/response 与决策后干净关闭。
- `apps/desktop-ui/src-tauri/src/core_pipe.rs`：传输句柄改为持有同一 `CorePipeSession`；
  `send` 与 `subscribe` 都委托给该会话。
- `apps/desktop-ui/src-tauri/src/event_bridge.rs`：决策后的干净关闭不再伪装成传输失败；
  非预期断连仍显式上报。
- `apps/desktop-ui/src-tauri/src/lib.rs`：注册私有 session 模块。
- `apps/desktop-ui/README.md`：记录单会话双向复用不变量。
- `plans/stage-1-pilots.md`、`docs/memory/facts.md`、`docs/memory/pitfalls.md`：
  登记 TASK-363 与单会话/双会话坑。

### 3. 验收输出摘要

- desktop-ui Rust tests：20 passed / 0 failed；其中真实 NamedPipe 用例覆盖事件、双向心跳、
  approve 响应与决策后干净关闭。
- desktop-ui fmt / clippy：通过；clippy 仅保留既有 MSRV 配置 warning。
- UI `lint` / `typecheck` / `test` / `build`：通过；model 89、DOM 5。
- root fmt / clippy / workspace 510 / arch 10 / schema / codegen：通过。
- hygiene / memory-counts / adr-index / refscan / docscan / card-check /
  check-ledger / check-comments / cargo deny：全部 PASSED。

### 4. DoD 逐条核对

- [x] desktop-ui 只建立一条 authenticated Core UI 会话，事件与命令共用该会话。
- [x] 心跳、事件、响应的分发均有 focused test，错误消息不含 token。
- [x] 真实 NamedPipe 集成测试证明事件与命令同连接且 correlation 精确匹配。
- [x] desktop-ui 专项、UI checks、workspace 与全部文档/安全门禁通过。
- [ ] PR 合并并记录 checks / merge / post-merge main CI；进度文件同步 Done。

### 5. 偏差

- 实现中发现 `NamedPipeTransport` 持有没有 `Send` 的 Windows 原生句柄，因此连接和握手必须在
  长期 worker 线程内完成，不能先在调用线程打开再移动；已按此边界实现，无契约变更。
- 当前 user turn 的最终 shell misuse 计数留到 Done 回填，避免提交前快照再次漂移。

### 6. 实施中发现的更合理做法

- 用有界命令/事件 channel 将 UI 命令串行化，而不是允许多个并发命令共享一个
  `NamedPipeTransport`；这避免响应错配，也满足资源生命周期要求。

### 7. 遗留问题

- TASK-057 的真实桌面 UI 审批点击仍需人类在场，本卡只解除传输层阻塞。

### 8. 新增长期记忆

- FACT：desktop-ui 的 Heartbeat / UiEvent / UiResponse 已收敛到单 authenticated 会话。
- PITFALL：事件订阅与审批命令不能各开一条 NamedPipe；连接必须在长期 worker 内建立。

### 9. 给审阅者的关注点

- 单会话 worker 是否会在事件、心跳、响应之间保持有界且不吞错误。
- correlation、session id、超时与断连是否都不会触发命令重放。
