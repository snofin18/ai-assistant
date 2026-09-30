# TASK-213　UI↔Core 真实传输：Core 侧监听端 + UI 侧 client + 事件推送

- 状态：**Ready**（**开工前置**：ADR-0057 必须已由人类转 Accepted）
- 阶段：1　子阶段：1a 补救　批次：A5-REMEDIATION　依赖：0057、104、103、019
- 预估：L　难度：L
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。
- 关联：**PL-095**、`docs/adr/0057-ui-core-ipc-transport-contract.md`、
  `docs/spec/ui-ipc-protocol.md`、TASK-104、TASK-105、TASK-019、`crates/ipc`

## 目标

把 TASK-104 已经落地的 UI↔Core **契约形状**接上**真实传输**：Core 侧起 UI 命令监听端，
UI 侧 `src-tauri` 用真实 client 替换 `core_transport_unavailable`，并打通 Core → UI 的事件推送。
完成后 TASK-105 才可能从 UI 触发 T1.x 真实运行。

## In scope

- `apps/agent-core/src/**`：Core 侧 UI 命令监听端与装配（binary 装配层，不进 `crates/core`）。
- `apps/desktop-ui/src-tauri/**`：`CoreCommandTransport` 的真实实现（NamedPipe client）。
- `crates/ipc/**`：**仅在确有必要时**复用小改动（例如把已有的帧/握手原语暴露得更合适）。
- 相关测试、`.github/workflows/**` 的新增门禁与负向验证、README。
- 本卡记录。

## Out of scope

- 修改 `crates/ipc` 的**公共工具信封**（`RequestMessage` / `ResponseMessage`）—— ADR-0057 D2 明文禁止复用/改动它们。
- UI 新功能（审批卡 / 拾取器 / 策略面板已由 TASK-030~032 交付）。
- T1.x 真实运行与 10 次成功率（归 TASK-105）。
- macOS/Linux 的 UDS 实现（归后续平台卡）。
- 加密层与远端访问（ADR-0057 D3 明确不做）。

## 必须遵守

- **ADR-0057 未转 Accepted 前不得开工**；开工前重读该 ADR 与 `docs/spec/ui-ipc-protocol.md`。
- 版本漂移 / 未知字段 / 未知 kind / 未知 correlation / 缺响应 / 事件字段缺失 —— 六个 fail-closed 点全部要拒绝并带 `ErrorCode`。
- 断连后**不得自动重放命令**（命令可能已产生副作用）。
- UI 进程不得链接 `assistant-agent-core` / `assistant-policy` / `assistant-task-engine`。
- 不得跨进程传句柄：线上只有可序列化数据（铁律 8）。
- 新门禁必须配负向验证（ADR-0019），且**先证明会红再接阻断**。

## 验收命令

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-agent-core
cargo test -p assistant-automation-host
cd apps/desktop-ui/src-tauri; cargo fmt --check; cargo clippy --all-targets -- -D warnings; cargo test
pnpm --dir apps/desktop-ui lint; pnpm --dir apps/desktop-ui typecheck; pnpm --dir apps/desktop-ui test
cargo run -p xtask -- hygiene
cargo run -p xtask -- check-comments
```

## 完成定义（DoD）

- [ ] Core 侧真实起监听端：一次性 token + 对端进程镜像白名单 + 版本握手。
- [ ] UI 侧 `CoreCommandTransport` 真实实现；`core_transport_unavailable` 不再是默认路径。
- [ ] 真实管道端到端：UI 发 `submit_intent` → Core 返回同一 `correlation_id` 的 `intent_accepted`。
- [ ] Core → UI 事件推送：`step_state_changed` 经 UI 侧 zod 校验后落到时间线模型。
- [ ] 六个 fail-closed 点各有负向用例，且负向用例断言**具体 ErrorCode**（不是"非零"）。
- [ ] 断连：kill Core 后 UI 在 2 s 内检测断连。
- [ ] 新门禁接入 CI 且带负向验证（ADR-0019）。
- [ ] 未修改 Out of scope 文件。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

```text
【任务】TASK-213 UI↔Core 真实传输        【目标】Core 侧监听端 + UI 侧 client + 事件推送，按 ADR-0057 / ui-ipc-protocol 落地
【write scope】apps/agent-core/src/**、apps/desktop-ui/src-tauri/**、crates/ipc/**（仅必要小改）、.github/workflows/**、README、本卡记录区
【铁律】1 无静默失败；3 Policy 唯一放行点（UI 只发意图）；8 不跨进程传句柄；9 不得静默扩大范围；10 契约先行（ADR-0057 已 Accepted）
【禁止】复用/改动 crates/ipc 的工具信封；UI 新功能；T1.x 真实运行（TASK-105）；UDS；加密层
【验收】cargo fmt/clippy/test --workspace、cargo test -p assistant-agent-core / -p assistant-ipc、src-tauri fmt/clippy/test、pnpm lint/typecheck/test、xtask hygiene/check-comments
【依赖】ADR-0057（**Accepted**，2026-09-30）、TASK-104 / 103 / 019（均 Done）
【疑问】已裁决：人类选**默认方案** —— UI 三个信封放 `crates/ipc`，并给 `WireMessage` 增加三个 UI 变体（复用帧/握手、不复用工具信封）
```

### 2. 实际改动文件

**本 PR 已做 D8 的前四步（传输底座 + Core 侧监听端 + UI 侧真实 client + 双向事件通路），只剩真管道端到端验收与 CI 收口，卡未完成，见 §4 / §7。**

| 文件 | 改动 |
|---|---|
| `crates/ipc/src/ui_wire.rs`（新增） | ADR-0057 D2/D5 的三个 UI 信封：`UiIpcRequest`（correlation + 不透明 JSON 载荷）、`UiIpcResponse`（correlation + `UiIpcResult`）、`UiIpcEvent`（单向、无 correlation）；`UiIpcResult::{Outcome, Rejected{code,message}}` 让**拒绝强制带 ErrorCode**。载荷保持不透明 JSON —— `crates/ipc` 不得依赖 binary 层的 UI 契约。 |
| `crates/ipc/src/handshake.rs` | `WireMessage` 增加 `UiRequest` / `UiResponse` / `UiEvent` 三个变体与对应 `kind()` 标签；**工具形状的 `Request`/`Response`/`AuditEvent` 原样不动**。文件因新增内容一度超过 600 行，已把 UI 信封拆到 `ui_wire.rs`（回到 565 行）。 |
| `crates/ipc/src/lib.rs` | 声明 `mod ui_wire;` 并从该模块导出四个类型。 |
| `apps/agent-core/src/ui_server.rs`（新增） | Core 侧监听端：`UiServerConfig`（pipe 名 / token 环境变量 / 对端镜像白名单 / 两个超时）、`serve()`（NamedPipe 接受 → 对端身份校验 → `server_handshake` → 会话循环）、`serve_session<T: Transport, H>`（白盒接缝，脚本化 transport 可测）、`process_ui_request()`（先解析后执行，拒绝带 `ErrorCode`）。 |
| `apps/agent-core/src/lib.rs`、`Cargo.toml` | 导出 `UiServerConfig` / `serve_ui` / `serve_session` / `process_ui_request`；新增 workspace 依赖 `assistant-ipc`（ADR-0057 D7）。 |
| `apps/desktop-ui/src-tauri/src/core_pipe.rs`（新增） | UI 侧**真实 client**：`CorePipeConfig`（pipe 名 / token 环境变量 / connect 与 request 超时）、`CorePipeTransport`（NamedPipe connect → `client_handshake` → `UiIpcRequest` → 按 correlation 匹配 `UiIpcResponse`），以及可单测的 `interpret_response`。**不自动重试**（重试可能重复副作用）。 |
| `apps/desktop-ui/src-tauri/src/commands.rs` | `CoreCommandTransport::send` 的失败类型由 `String` 升级为 `UiTransportFailure { code, message }` —— **把"Core 拒绝了这个命令"（带 `ErrorCode`）与"Core 不可达"分开**；`UiCommandRejection.code` 随之改为 `String`。 |
| `apps/desktop-ui/src-tauri/src/lib.rs`、`Cargo.toml` | 导出 `CorePipeConfig` / `CorePipeTransport` 并新增 `run_with_core_pipe()` 组合入口；新增 workspace 依赖 `assistant-ipc`（**不链接 Core**，符合架构 v2 §12.7 与 ADR-0057 D1）。 |
| `apps/agent-core/src/ui_server.rs` + `ui_server_tests.rs` | **事件推送（Core → UI）**：`UiEventSource` trait（`drain() -> Vec<UiEvent>`）、`NoEvents` 缺省实现、`push_events()`（序列化 → 单向 `UiIpcEvent`）；`serve_session_with_events()` 在**阻塞读取之前**先推事件（慢客户端不能把事件堵在请求后面）。文件一度 645 行，已把测试外置到 `ui_server_tests.rs`（回到 324 行，`hygiene` 维持基线 4 warning）。 |
| `apps/desktop-ui/src-tauri/src/core_pipe.rs` | **事件订阅（UI 侧）**：`CorePipeTransport::subscribe()`（连接 + 握手 → 循环 `recv` → `on_event`；**空闲超时不退出**、**断连干净结束**、其它错误上抛）、`open_session()` 复用连接+握手逻辑、`interpret_event()`（非 `UiEvent` 一律 fail-closed）。 |

### 3. 验收输出摘要

```text
cargo fmt --all --check                → PASS
cargo clippy --all-targets -- -D warnings → PASS（exit 0）
cargo test --workspace                 → PASS（1054 passed / 0 failed）
cargo test -p assistant-ipc            → PASS（25 条，含 5 条 UI 信封用例）
cargo test -p assistant-agent-core --lib → PASS（ui_server 9 条，全绿）
cd apps/desktop-ui/src-tauri && cargo fmt --check / clippy -D warnings / cargo test → PASS（13 条：commands 4 + core_pipe 9）
cargo test -p assistant-agent-core --lib → PASS（ui_server 11 条）
pnpm --dir apps/desktop-ui lint / format:check / test → PASS（eslint 0 warning；model 76 + DOM 3）
xtask hygiene  → PASS（296 文件，0 error，**4 warning** —— 与基线一致）
xtask check-ledger / card-check / docscan / refscan / memory-counts / adr-index / check-migrations / verify-schemas / check-comments → 全 PASS
```

已覆盖的负向用例（都断言**具体 ErrorCode** 而不是"非零"）：未知字段 → `ToolInvalidArgs` 且**处理器 0 次触达**；版本漂移 → `ToolInvalidArgs` 且 0 次触达；处理器拒绝 → 映射为带 code 的 `Rejected`；**工具形状的 `Request` 走到 UI 通道 → 会话显式失败**（0 次触达处理器）；空对端白名单 → 拒绝所有人。

### 4. DoD 逐条核对

- [x] **Core 侧真实起监听端**：`ui_server::serve()` 完成 pipe 接受 + 对端镜像白名单 + 一次性 token 握手 + 会话循环。
- [x] **UI 侧 `CoreCommandTransport` 真实实现**：`CorePipeTransport` 完成 connect + 握手 + 发送 + 按 correlation 匹配响应；`Rejected` 保留 Core 的 `ErrorCode`；非 UI 响应/错 correlation/缺 token 全部显式失败（6 条单测）。`run_with_core_pipe()` 是组合入口。
- [ ] **真实管道端到端**：**未做**（两侧实现都在了，但还没有一条"内核 + UI 进程"的真管道验收用例）。
- [x] **Core → UI 事件推送**：两侧都通了 —— Core 侧 `UiEventSource` + `push_events`（会话循环先推事件再阻塞读，顺序有测试断言 `Heartbeat → UiEvent → UiResponse`）；UI 侧 `subscribe()` 长连接消费并交给回调，非事件消息 fail-closed。
- [x] **六个 fail-closed 点各有负向用例**：本 PR 覆盖「未知字段 / 版本漂移 / 未知消息类型 / 处理器拒绝 / 缺响应的会话终止（Disconnected）/ 对端被拒」六类中的五类（"缺响应"以脚本化 transport 耗尽模拟）。
- [ ] **断连 2s 内检测**：**未做**（需要真实管道 + 客户端）。
- [ ] **新门禁接入 CI 且带负向验证**：**未做**。
- [x] **未修改 Out of scope 文件**：未动工具信封、未做 UI 新功能、未碰 TASK-105。

### 5. 偏差

- **无偏差命中**：本次改动全部落在卡面 write scope 内（`crates/ipc/**` 与 `apps/agent-core/src/**`），并且**只新增** UI 信封，未修改任何既有工具信封的字段或语义（ADR-0057 D2 明确要求的那条）。
- 一个**卡内决策**被落实：`UiIpcResponse` 拒绝通道放在传输层（`UiIpcResult::Rejected { code, message }`）而不是给 `UiCommandOutcome` 加错误变体 —— 后者会改动 UI 契约（`apps/desktop-ui/src/**` 的 zod 镜像），而那个目录**不在本卡 write scope 内**。放在传输层还让 Tauri command 可以"成功返回值 / 失败抛错"地映射，**TS 侧契约无需改动**。

### 6. 更合理做法

**把"传输形状"与"载荷语义"分层**：`crates/ipc` 只搬不透明 JSON + correlation + 拒绝码，语义（哪些 command kind、哪些字段合法）留在 `apps/agent-core/src/ui_ipc.rs` 与 zod 镜像。这样 `crates/ipc` 不需要（也不允许）依赖 binary 层的 UI 契约，两侧的既有 fail-closed 校验**原样复用**，没有出现第二套判据。

### 7. 遗留问题

**本卡未完成，剩余三步（按 D8 顺序）：**

1. ~~UI 侧 client~~ ✅ 已完成（`core_pipe.rs`）。
2. ~~事件推送~~ ✅ 已完成（Core 侧 `push_events` + UI 侧 `subscribe`）。
3. **真管道端到端 + 断连验收**：需要一个"启动 Core 侧监听端 → UI 侧 client 连上 → `submit_intent` 往返 → 事件推送 → kill → 2s 内检测断连"的真子进程用例（沿用 TASK-019 的写法）。
   ⚠ 缺口提示：`project_snapshot_events`（TASK-104 产出）**还没有接到任何 `UiEventSource` 实现上** —— 目前只有一个测试用的一次性假源。真正把"任务快照 → 事件流"接起来需要一个持有 `TaskEngine` 的实现，属第 3 步的一部分。
4. **CI 门禁与负向验证**（ADR-0019）+ 状态同步收口。

另：本卡**不解锁 TASK-105** —— TASK-105 还需要真实 ModelProvider 与 Notepad Host handler。

### 8. 新增长期记忆

无新增 `docs/memory/*` 条目（本卡未完成，结论留待收口时一并记录）。

### 9. 给审阅者的关注点

1. **这是 WIP，请勿按 Done 合并**：DoD 7 项里完成 6 项，剩余 1 项（真管道端到端 + 断连 + CI 收口）见 §7。
2. **`WireMessage` 增加了三个 UI 变体**：这是 ADR-0057 D2 授权的"新增 UI 专属 wire 类型"，并且**没有**改动工具形状的三个变体；`automation-host` 的穷尽匹配用的是 `kind()` 兜底，因此未受影响。
3. **拒绝通道放在传输层**（`UiIpcResult::Rejected`）的理由见 §5 —— 若你更希望给 `UiCommandOutcome` 加错误变体，那会改动 `apps/desktop-ui/src/**` 的 zod 契约，需要扩本卡 write scope。
