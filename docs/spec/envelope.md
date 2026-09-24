# spec: Envelope / 跨进程消息包装

> 摘要：跨进程 / 跨 crate / 跨语言消息包装（envelope）规范。本 spec 是本项目**契约层**的一部分，由 ADR 批准后即作为 xtask 卡实施 + CI 机器校验的权威。
> 状态：Draft（待 ADR 批准）　版本：0.1　日期：2026-09-20
> 上位：`AGENTS.md` §5、`docs/governance-ai-agent-execution.md` §5/§6
> 强制性：**本文档是契约**。违反即 CI 失败（`xtask` 相关子命令）或 Reviewer 拒绝合并。
> 变更门槛：新增字段或修改类型 → 需 ADR。

---

## 1. 目标

定义 Host / Agent / Platform 之间**唯一**的消息包装（envelope）：单根 envelope = `header` + `payload`，payload 由 `header.kind` 判别。
目标：任何跨进程 / 跨 crate / 跨语言的消息都走同一形状，使调试追踪、审计关联、版本协商有统一落点。

## 2. 范围

**管**：envelope 的 `header` 字段与语义、`PayloadKind` 判别方式、关联 ID（`correlation_id`）规则、schema 版本的拒绝策略。

**不管（不做清单）**：
- 不管**传输层**：字节布局 / magic / CRC32 / 握手 归 `docs/spec/ipc-protocol.md`
- 不管 payload **内容**：Tool 输入输出归 `docs/spec/tool-schema.md`，事件归 `docs/spec/audit-event.md`
- 不管错误码取值（归 `docs/spec/error-codes.md`）
- 不管加密 / 压缩 / 分片（后续卡）

## 3. 类型定义

跨进程 / 跨 crate / 跨语言消息统一 envelope = 单根 envelope 含 `header` + `payload`，payload 由 `kind` 字段判别。

```rust
struct Envelope {
    header: Header,
    payload: Payload,
}

struct Header {
    schema_version: u32,       // 当前 = 1
    envelope_id: Uuid,         // 全局唯一 = 调试/审计追踪
    correlation_id: Option<Uuid>, // 关联请求 ID（请求/响应匹配）
    timestamp: Iso8601,
    source: ActorId,            // 发起方
    target: ActorId,            // 接收方
    kind: PayloadKind,          // 判别 payload 类型
}

enum PayloadKind {
    ToolInvoke,
    ToolResult,
    AuditEvent,
    Error,
    Heartbeat,
}

struct Payload {
    kind: PayloadKind,
    body: serde_json::Value,   // = tool schema / result / event 等
}
```

## 4. 不变量

1. **schema_version 必填**：`<= 0` = 拒绝接收；`> 当前` = 拒绝（不支持前向兼容）。
2. **envelope_id 全局唯一**：UUID v7 = 时间有序（= 利于审计追踪）。
3. **correlation_id 必填**（请求-响应场景）：缺失 = 当成孤儿消息（记 audit 后丢弃）。
4. **payload.body 自描述**：由 `header.kind` 决定解析路径，**禁止**基于 payload 内部字段类型反推 kind。

---

## 5. 与其他 spec 的关系

| 引用方向 | spec | 关系 |
|---|---|---|
| 被依赖 | `docs/spec/ipc-protocol.md` | 本 envelope 是该协议的线上载荷（字节布局由该 spec 定义；本 spec §2 明确不管传输层） |
| 依赖 | `docs/spec/tool-schema.md` | `PayloadKind::ToolInvoke` / `ToolResult` 的 body 是 Tool schema 实例 |
| 依赖 | `docs/spec/audit-event.md` | `PayloadKind::AuditEvent` 的 body 是 audit event |
| 依赖 | `docs/spec/error-codes.md` | `PayloadKind::Error` 的 code 只用 ErrorCode 枚举 |
| 相关 | `docs/spec/capability-matrix.md` | 握手交换的 capability 列表取值由矩阵定义 |

---

## 附录：演进记录

| 版本 | 日期 | 变更 |
|---|---|---|
| 0.1 | 2026-09-20 | 初稿（项目进度督察后批量补齐 stage-0 DoD #5）|
