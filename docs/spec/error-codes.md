# spec: ErrorCode 枚举规范

> 摘要：错误码枚举（ErrorCode）规范。本 spec 是本项目**契约层**的一部分，由 ADR 批准后即作为 xtask 卡实施 + CI 机器校验的权威。
> 状态：Draft（待 ADR 批准）　版本：0.1　日期：2026-09-20
> 上位：`AGENTS.md` §5、`docs/governance-ai-agent-execution.md` §5/§6
> 强制性：**本文档是契约**。违反即 CI 失败（`xtask` 相关子命令）或 Reviewer 拒绝合并。
> 变更门槛：新增字段或修改类型 → 需 ADR。

---

## 1. 目标

## 2. 范围

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

---

## 5. 与其他 spec 的关系

(本节 = tool-schema 的 postconditions 字段 + envelope 的 PayloadKind::Error + audit-event 的 outcome)

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
