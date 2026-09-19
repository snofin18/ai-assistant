# spec: Host IPC 协议

> 摘要：Host / Agent / Platform 三方 IPC 协议（命名管道 / 共享内存 + 序列化）。本 spec 是本项目**契约层**的一部分，由 ADR 批准后即作为 xtask 卡实施 + CI 机器校验的权威。
> 状态：Draft（待 ADR 批准）　版本：0.1　日期：2026-09-20
> 上位：`AGENTS.md` §5、`docs/governance-ai-agent-execution.md` §5/§6
> 强制性：**本文档是契约**。违反即 CI 失败（`xtask` 相关子命令）或 Reviewer 拒绝合并。
> 变更门槛：新增字段或修改类型 → 需 ADR。

---

## 1. 目标

## 2. 范围

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

(本节 = envelope 的 wire format + capability-matrix 握手 + audit-event 单向流 + tool-schema 消息体)

### 字段
### 字段

| 字段 | 类型 | 必选 | 说明 |
|---|---|---|---|
| `schema_version` | integer | ✓ | 当前 = 1（schema 升级时递增） |

### 命名空间

- Tool：`tool.<app>.<domain>.<action>`（与 ADR-0021 受控词一致）
- Adapter：`adapter.<app>.<version>`
- AuditEvent：`audit.<event_kind>`

---

## 4. 不变量

1. **schema_version 单调递增**：从 1 起；任何字段重命名/类型变化 → 新增 version，旧字段标记 `@deprecated` 保留 ≥2 个版本。
2. **必选字段不可为空**：`required` 列表中的字段在所有实例中**非 null / 非空字符串 / 非空数组**。
3. **时间戳用 ISO-8601**：所有时间字段（`captured_at` / `occurred_at` / `timestamp`）= UTC + RFC 3339（= `2026-09-19T12:34:56Z` 形式）。
4. **ID 用 u64**：所有 `id` 字段类型 = unsigned 64-bit（= 本机跨进程传递稳定）。
5. **错误用 ErrorCode 枚举**：见 `docs/spec/error-codes.md`；禁止字符串自定义错误码。

---

## 5. 与其他 spec 的关系

| 引用方向 | 来源 spec | 关系 |
|---|---|---|
| 依赖 | `docs/spec/error-codes.md` | 所有错误字段用 ErrorCode 枚举 |
| 依赖 | `docs/spec/envelope.md` | 大消息包 `envelope` 内含本 schema 实例 |
| 依赖 | `docs/spec/capability-matrix.md` | Tool schema 必须含 capability 字段声明 |
| 依赖 | `docs/spec/audit-event.md` | Tool 调用结果必须产出 audit event |
| 依赖 | `docs/spec/ipc-protocol.md` | 跨进程传输用 envelope 包 Tool 输入/输出 |
| 依赖 | `docs/spec/naming.md` | 字段命名遵守命名规范 |
| 依赖 | `docs/spec/testing.md` | 测试用例覆盖 schema 边界 |

---

## 附录：演进记录

| 版本 | 日期 | 变更 |
|---|---|---|
| 0.1 | 2026-09-20 | 初稿（项目进度督察后批量补齐 stage-0 DoD #5）|
