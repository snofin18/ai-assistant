# spec: 审计事件 schema

> 摘要：审计事件（audit event）结构与字段约束。本 spec 是本项目**契约层**的一部分，由 ADR 批准后即作为 xtask 卡实施 + CI 机器校验的权威。
> 状态：Draft（待 ADR 批准）　版本：0.1　日期：2026-09-20
> 上位：`AGENTS.md` §5、`docs/governance-ai-agent-execution.md` §5/§6
> 强制性：**本文档是契约**。违反即 CI 失败（`xtask` 相关子命令）或 Reviewer 拒绝合并。
> 变更门槛：新增字段或修改类型 → 需 ADR。

---

## 1. 目标

定义**审计事件**的结构：任何受治理的动作必须产出可追加、不可修改的事件记录，并通过 hash chain 使篡改可检测。

## 2. 范围

**管**：事件字段（`event_id` / `sequence` / `prev_hash` / `payload_hash` / `actor` / `subject` / `action` / `outcome` / `metadata` / `redacted_fields`）、append-only 与 hash chain 不变量、脱敏字段路径的表达。

**不管（不做清单）**：
- 不管**存储实现**：表结构 / flush 策略 / `durability` 归 TASK-013 `crates/audit`
- 不管错误码取值（归 `docs/spec/error-codes.md`）
- 不管事件的**投递通道**（单向流归 `docs/spec/ipc-protocol.md`）
- 不管日志轮转 / 冷归档 / 签名（后续卡）

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

| 引用方向 | spec | 关系 |
|---|---|---|
| 依赖 | `docs/spec/envelope.md` | `PayloadKind::AuditEvent` 是事件的传输载体 |
| 依赖 | `docs/spec/error-codes.md` | `outcome.code` 必须是 ErrorCode 枚举的 u16 |
| 相关 | `docs/spec/tool-schema.md` | Tool 的 `outputs` / `postconditions` 决定事件的 outcome |
| 相关 | `docs/spec/ipc-protocol.md` | 事件可作为单向流（fire-and-forget）推送 |

---

## 附录：演进记录

| 版本 | 日期 | 变更 |
|---|---|---|
| 0.1 | 2026-09-20 | 初稿（项目进度督察后批量补齐 stage-0 DoD #5）|
