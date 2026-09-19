# spec: 审计事件 schema

> 摘要：审计事件（audit event）结构与字段约束。本 spec 是本项目**契约层**的一部分，由 ADR 批准后即作为 xtask 卡实施 + CI 机器校验的权威。
> 状态：Draft（待 ADR 批准）　版本：0.1　日期：2026-09-20
> 上位：`AGENTS.md` §5、`docs/governance-ai-agent-execution.md` §5/§6
> 强制性：**本文档是契约**。违反即 CI 失败（`xtask` 相关子命令）或 Reviewer 拒绝合并。
> 变更门槛：新增字段或修改类型 → 需 ADR。

---

## 1. 目标

## 2. 范围

## 3. 类型定义

所有受 xtask 治理的动作必须产出一条 audit event（append-only + hash chain）。

```rust
struct AuditEvent {
    event_id: Uuid,            // 全局唯一 = 跨事件去重锚
    timestamp: Iso8601,        // UTC + RFC 3339
    sequence: u64,             // 本会话内单调递增（= 排序用）
    prev_hash: Hash,            // 前一条 event 的 hash（= hash chain）
    payload_hash: Hash,        // 本条 payload 的 SHA-256
    actor: ActorId,             // 哪条 Tool / Agent / 人类
    subject: TargetDescriptor,  // 被操作目标
    action: String,             // 例 "tool.invoked" / "approval.granted" / "policy.denied"
    outcome: Outcome,           // Success{code,return_value} | Error{code,message}
    metadata: serde_json::Value, // 自由扩展 = adapter 自定义字段
    redacted_fields: Vec<String>, // 数据脱敏字段路径（= DLP 三档）
}

enum Outcome {
    Success { code: u16, return_value: serde_json::Value },
    Error { code: u16, message: String },
}
```

## 4. 不变量

1. **append-only**：任何 event 一旦 append = 永不可修改（仅 hash chain 重算可检测篡改）。
2. **prev_hash 单调链**：`prev_hash` = 前一条 `payload_hash + prev_hash` 的拼接哈希（= tamper-evident）。
3. **sequence 单调**：同一 actor 会话内 `sequence` 单调递增，跨会话不连续（= 不同 actor 独立 sequence 空间）。
4. **code ∈ ErrorCode**：`outcome.code` 必须是 `docs/spec/error-codes.md` 中定义的 u16。

---

## 5. 与其他 spec 的关系

(本节 = envelope 的 PayloadKind::AuditEvent + tool-schema 的 outputs + error-codes::Success/Error)

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
