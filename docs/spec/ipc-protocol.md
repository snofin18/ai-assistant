# spec: Host IPC 协议

> 摘要：Host / Agent / Platform 三方 IPC 协议（命名管道 / 共享内存 + 序列化）。本 spec 是本项目**契约层**的一部分，由 ADR 批准后即作为 xtask 卡实施 + CI 机器校验的权威。
> 状态：Draft（待 ADR 批准）　版本：0.1　日期：2026-09-20
> 上位：`AGENTS.md` §5、`docs/governance-ai-agent-execution.md` §5/§6
> 强制性：**本文档是契约**。违反即 CI 失败（`xtask` 相关子命令）或 Reviewer 拒绝合并。
> 变更门槛：新增字段或修改类型 → 需 ADR。

---

## 1. 目标

定义 Host / Agent / Platform 三方之间的**线上协议**（wire format + 握手 + 消息流 + 失败模式），使跨进程通信有唯一可实现的形状。

## 2. 范围

**管**：字节布局（magic / `envelope_size` / CRC32）、握手（ClientHello / ServerHello / Heartbeat）、消息流方向与 correlation、失败模式（断连 / 超时 / 版本不匹配）。

**不管（不做清单）**：
- 不管 envelope **内部字段**（归 `docs/spec/envelope.md`）
- 不管 capability **取值**（握手只传 capability 列表，取值归 `docs/spec/capability-matrix.md`）
- 不管进程**权限模型**与 token / 对端身份校验的实现（归 TASK-019 `apps/automation-host`）
- 不定义 macOS / Linux 的传输实现细节（平台层归各自卡）

## 3. 类型定义

Host / Agent / Platform 三方通过命名管道（Windows）/ 共享内存（Linux/macOS）+ 序列化通信。

```
On wire format:
  [4 bytes: magic 0xC0DECAFE]
  [4 bytes: envelope_size (LE u32)]
  [N bytes: envelope (per docs/spec/envelope.md)]
  [4 bytes: envelope CRC32 (LE u32)]

Handshake (连接建立):
  Client -> Server: ClientHello { version: u32, capabilities: Vec<CapabilityId> }
  Server -> Client: ServerHello { version: u32, capabilities: Vec<CapabilityId>, session_id: Uuid }
  Both -> Both: Heartbeat { session_id, timestamp, alive }

Message flow:
  Client -> Server: Request { correlation_id, target, tool_invoke }
  Server -> Client: Response { correlation_id, result | error }
  Server -> Client: AuditEvent (单向 fire-and-forget, 无 correlation_id)

Failure modes:
  - 断连：客户端用 `guard acquire` 锁文件记录断连 + 重连尝试（per ADR-0028）。
  - 超时：`envelope_size` 超 16 MB = 协议错误 = 强制断连。
  - schema_version 不匹配：握手阶段拒绝 = 永不进入正常流。
```

## 4. 不变量

1. **magic 必填**：任何首 4 字节 != 0xC0DECAFE = 协议错（= 客户端不可能是本系统）。
2. **CRC32 必填**：任何 CRC 不匹配 = 数据错 = 丢弃 + 记 audit。
3. **envelope_size 上限 16 MB**：防止恶意客户端传超长消息撑爆管道。
4. **握手协议无副作用**：ClientHello / ServerHello / Heartbeat 都不产 audit event（= 心跳噪声不污染日志）。

---

## 5. 与其他 spec 的关系

| 引用方向 | spec | 关系 |
|---|---|---|
| 依赖 | `docs/spec/envelope.md` | 线上载荷 = envelope 的序列化字节 |
| 依赖 | `docs/spec/capability-matrix.md` | 握手阶段交换 capability 列表 |
| 依赖 | `docs/spec/audit-event.md` | 服务端可单向推送 audit event（fire-and-forget） |
| 依赖 | `docs/spec/tool-schema.md` | 请求 / 响应消息体是 Tool 输入 / 输出 schema 实例 |
| 依赖 | `docs/spec/error-codes.md` | 协议错误（magic / CRC / 超长）用 ErrorCode 表达 |

---

## 附录：演进记录

| 版本 | 日期 | 变更 |
|---|---|---|
| 0.1 | 2026-09-20 | 初稿（项目进度督察后批量补齐 stage-0 DoD #5）|
