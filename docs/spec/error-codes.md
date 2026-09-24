# spec: ErrorCode 枚举规范

> 摘要：错误码枚举（ErrorCode）规范。本 spec 是本项目**契约层**的一部分，由 ADR 批准后即作为 xtask 卡实施 + CI 机器校验的权威。
> 状态：Draft（待 ADR 批准）　版本：0.1　日期：2026-09-20
> 上位：`AGENTS.md` §5、`docs/governance-ai-agent-execution.md` §5/§6
> 强制性：**本文档是契约**。违反即 CI 失败（`xtask` 相关子命令）或 Reviewer 拒绝合并。
> 变更门槛：新增字段或修改类型 → 需 ADR。

---

## 1. 目标

定义本项目**唯一的错误码枚举** `ErrorCode`（u16 + 命名空间分组，借鉴 RFC 7807 的分组思路），使错误在 Host ↔ Agent ↔ Platform 之间稳定可比、可审计、可国际化。

## 2. 范围

**管**：错误码的数值区间与分组语义、跨进程稳定性要求、错误实例必带 `message` 的约束、新增错误码的门槛。

**不管（不做清单）**：
- 不管错误到**领域错误类型**的映射（各 crate 的 `error.rs` 负责映射，本 spec 只给枚举）
- 不管重试 / 降级 / 审批策略（归 `docs/spec/capability-matrix.md` 的风险级与 policy 引擎）
- 不管 HTTP 状态码或 RFC 7807 的具体载体（本 spec 只借其分组思路）
- 不管日志与事件格式（归 `docs/spec/audit-event.md`）

## 3. 类型定义

错误码枚举 = `ErrorCode` = u16 整数 + 命名空间分组（参考 RFC 7807 风格）。

```rust
#[repr(u16)]
enum ErrorCode {
    // 1000-1999: 通用 / IO / 工具自身缺陷
    Unknown = 1000,
    Io = 1001,
    InternalBug = 1002,
    // 2000-2999: 用户输入 / 契约违反
    InvalidArgument = 2000,
    SchemaVersionMismatch = 2001,
    CapabilityUndeclared = 2002,
    // 3000-3999: 目标应用 / 系统资源
    TargetNotFound = 3000,
    TargetUnresponsive = 3001,
    TargetVersionMismatch = 3002,
    // 4000-4999: 策略 / 权限
    PolicyDenied = 4000,
    ApprovalRequired = 4001,
    ApprovalExpired = 4002,
    // 5000-5999: 自动化 / 资源锁
    LockTimeout = 5000,
    QuotaExceeded = 5001,
    // 6000-6999: 用户主动 / 取消
    UserCancelled = 6000,
    UserRejected = 6001,
    // 9000+: 不可恢复 / 致命
    Fatal = 9000,
}
```

## 4. 不变量

1. **错误码 u16 单调**：1000-9999 内部按"严重度+类别"分组；预留 9000+ 不可恢复。
2. **错误码与文档同步**：本枚举必须有 ADR 批准才能新增；`xtask verify-schemas` 强制 schema 与代码一致。
3. **错误码跨进程稳定**：u16 数值在所有跨进程边界（Host ↔ Agent ↔ Platform）保持不变；命名空间允许重排但数值不变。
4. **错误必带 message**：所有 ErrorCode 实例化时必带人可读 `message` 字段（= 多语言友好）。
5. **错误用 ErrorCode 枚举**：`outcome` / `postconditions` / 协议错误等**一切**错误字段只能用本枚举；禁止字符串自定义错误码

---

## 5. 与其他 spec 的关系

| 引用方向 | spec | 关系 |
|---|---|---|
| 被依赖 | `docs/spec/tool-schema.md` | Tool 的 `postconditions` 用 ErrorCode |
| 被依赖 | `docs/spec/envelope.md` | `PayloadKind::Error` 的 code 用 ErrorCode |
| 被依赖 | `docs/spec/audit-event.md` | `outcome.code` 必须是本枚举的 u16 |
| 相关 | `docs/spec/capability-matrix.md` | `PolicyDenied` / `ApprovalRequired` 由矩阵的风险级与 Approval 列触发 |

---

## 附录：演进记录

| 版本 | 日期 | 变更 |
|---|---|---|
| 0.1 | 2026-09-20 | 初稿（项目进度督察后批量补齐 stage-0 DoD #5）|
