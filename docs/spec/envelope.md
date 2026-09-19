# spec: Envelope / 跨进程消息包装

> 摘要：跨进程 / 跨 crate / 跨语言消息包装（envelope）规范。本 spec 是本项目**契约层**的一部分，由 ADR 批准后即作为 xtask 卡实施 + CI 机器校验的权威。
> 状态：Draft（待 ADR 批准）　版本：0.1　日期：2026-09-20
> 上位：`AGENTS.md` §5、`docs/governance-ai-agent-execution.md` §5/§6
> 强制性：**本文档是契约**。违反即 CI 失败（`xtask` 相关子命令）或 Reviewer 拒绝合并。
> 变更门槛：新增字段或修改类型 → 需 ADR。

---

## 1. 目标

## 2. 范围

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

(本节 = tool-schema + audit-event 的容器 + ipc-protocol 的 wire format)

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
