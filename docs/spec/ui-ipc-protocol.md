# spec: UI↔Core IPC 协议（ui-ipc-protocol）

> 摘要：定义桌面 UI 进程与 Core 进程之间的命令 / 响应 / 事件线上契约。
> 状态：Draft（ADR-0057 **Accepted**，2026-09-30）　版本：0.1　日期：2026-09-30
> 上位：`AGENTS.md` §2、架构 v2 §12.7 / §14、ADR-0057、ADR-0056、`docs/spec/ipc-protocol.md`
> 强制性：ADR-0057 已于 2026-09-30 转 **Accepted**，本文即作为实现契约；违反由 Reviewer 拒绝合并，能机器化的
> 部分由 TASK-213 的测试固定。
> 变更门槛：新增字段或修改类型 → 需 ADR（漂移触发器 ③）。

---

## 1. 目标

1. 让桌面 UI 能在**不链接 Core、不持有任何系统权限**的前提下触发任务、提交审批决定、
   暂停 / 取消 / 接管，并接收执行状态与证据。
2. 让 UI 与 Core 两侧对同一份契约做**双向运行时校验**（UI 是不可信输入源，Core 的返回同样不可信）。
3. 让失败可诊断：任何拒绝都带稳定 `ErrorCode`，不存在"客户端静默等待"的路径。

## 2. 范围

**管**：

- 传输选择、握手与鉴权形状；
- 三个信封（请求 / 响应 / 事件）的线上表示与 correlation 规则；
- 版本漂移与非法输入的处置；
- 断连、超时、缺响应的失败模式。

**不管**：

- 载荷内部字段的语义 —— 见 `apps/agent-core/src/ui_ipc.rs` 的 `UiCommand` /
  `UiCommandOutcome` / `UiEvent`（TASK-104 的实现即本 spec 的载荷权威）；
- 工具调用的信封 —— 见 `docs/spec/envelope.md` 与 `docs/spec/ipc-protocol.md`（**工具形状，与本协议分离**）；
- 具体平台传输实现细节（Windows NamedPipe 归 `crates/ipc`；UDS 归后续平台卡）；
- 加密与远端访问（本协议不做；见 ADR-0057 D3）。

## 3. 传输与握手

```text
Transport (Windows):  NamedPipe，帧格式与上限复用 crates/ipc（magic / 16 MiB / CRC32）
Transport (macOS/Linux): Unix Domain Socket（后续平台卡；帧格式不变）

Handshake:
  UI   -> Core: UiClientHello   { protocol_version: u32, ui_build: String }
  Core -> UI  : UiServerHello   { protocol_version: u32, session_id: Uuid, heartbeat_timeout_ms: u64 }
  Both -> Both: UiHeartbeat     { session_id, timestamp_unix_ms, alive: bool }
```

- 协议版本不匹配 → **握手阶段拒绝**（`ErrorCode::CapabilityMissing`），永不进入消息流。
- 心跳静默超时 → 显式失败（客户端不得假装会话仍存活）。

## 4. 消息形状

```text
UI   -> Core: UiIpcRequest  { correlation_id: String, command: UiCommandEnvelope }
Core -> UI  : UiIpcResponse { correlation_id: String, outcome: UiCommandOutcome }
Core -> UI  : UiIpcEvent    { event: UiEvent }          // 单向推送，无 correlation
```

| 规则 | 内容 |
|---|---|
| R1 | `UiCommandEnvelope` 必须携带 `version`（当前 `"1.0"`）与 `command`；`command.kind` 必须在本协议的允许集内。 |
| R2 | 未知字段一律拒绝（两侧都是 allow-list / `strictObject`），不得忽略。 |
| R3 | Core 必须回**同 `correlation_id`** 的响应；缺响应或错 id = 会话级失败，不是"静默重试"。 |
| R4 | 事件不带 `correlation_id`；事件字段缺失即拒收该事件（不得用默认值补齐）。 |
| R5 | 同一连接上命令可流水，但响应必须与请求 id 一一对应。 |

## 5. 鉴权与对端身份

- **token**：Core 启动时生成一次性 token，经环境变量（受保护渠道）交给 UI；两侧用常数时间比较。
- **对端身份**：Windows 用 NamedPipe 客户端 PID + 镜像路径白名单；对端无法验证即拒绝连接。
- **管道归属**：仅当前用户可连接；不做远端客户端、不做跨用户。
- 与 `docs/spec/ipc-protocol.md` 的握手不变量一致：握手与心跳**不产生审计事件**，不污染日志。

## 6. 错误映射

| 失败点 | 结果 |
|---|---|
| 版本漂移 / 未知字段 / 未知 kind | `ErrorCode::ToolInvalidArgs`（UI 侧在发送前就已拒绝） |
| 未知 task / 未知 approval / scope 未提供 | `ErrorCode::TargetNotFound` / `ErrorCode::UserInteraction`（见 `ui_ipc.rs` 的映射） |
| token 或对端身份不通过 | 拒绝连接，不进入消息流 |
| 心跳超时 / 断连 | 显式失败 + 客户端重连尝试（不在本协议内自动重放命令） |
| 会话级未知 correlation / 缺响应 | 显式失败，`ErrorCode::Fatal` |

## 7. 不变量

1. **UI 不可信**：所有命令在 Core 侧二次校验；UI 侧校验只是提前失败，不是信任依据。
2. **不做静默降级**：任何未知输入、未知版本、未知 correlation 都返回带 `ErrorCode` 的失败。
3. **不跨进程传句柄**：线上只有可序列化数据（铁律 8）。
4. **不自动重放**：断连后不自动重发命令 —— 命令可能已经产生副作用（幂等性由上层决定）。
5. **载荷权威唯一**：`UiCommand` / `UiCommandOutcome` / `UiEvent` 的字段定义只有一处
   （`apps/agent-core/src/ui_ipc.rs`），TypeScript 侧为镜像并由黄金样本机器校验。

## 8. 与其他 spec 的关系

| 本 spec | 相关 spec | 关系 |
|---|---|---|
| 传输 / 帧 | `docs/spec/ipc-protocol.md` | 复用帧与握手不变量；**不复用**工具信封 |
| 载荷 | `docs/spec/runtime-execution.md` §6 | `UiEvent` 投影自运行事件 |
| 错误 | `docs/spec/error-codes.md` | 不新增字符串错误码或 ErrorCategory |
| 工具 | `docs/spec/envelope.md` | 工具信封与 UI 信封**分离**，互不承载 |

## 9. 验证清单

1. 负向：错 token / 非白名单镜像 / 版本不匹配 / 未知字段 / 错 correlation → 全部拒绝且带 `ErrorCode`。
2. 正向：真实管道上 `submit_intent` 往返成功。
3. 事件：`step_state_changed` 推送经 zod 校验后落到时间线模型。
4. 断连：kill Core 后 UI 在 2 s 内检测断连。
5. 无静默失败：以上每条失败路径都有对应断言，不存在"看着还活着"的路径。

## 10. 变更历史

| 日期 | 变更 | 依据 |
|---|---|---|
| 2026-09-30 | 建立 UI↔Core 传输契约（Draft） | ADR-0057 / PL-095 |
| 2026-09-30 | ADR-0057 由 Proposed 转 **Accepted**（人类确认），本文成为实现契约 | 人类裁决 |
