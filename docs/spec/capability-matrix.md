# spec: 能力矩阵（capability matrix）

> 摘要：Capability matrix 规范（Host / Skill / Platform 三层 × 资源访问 / 副作用 / 风险级）。本 spec 是本项目**契约层**的一部分，由 ADR 批准后即作为 xtask 卡实施 + CI 机器校验的权威。
> 状态：Draft（待 ADR 批准）　版本：0.1　日期：2026-09-20
> 上位：`AGENTS.md` §5、`docs/governance-ai-agent-execution.md` §5/§6
> 强制性：**本文档是契约**。违反即 CI 失败（`xtask` 相关子命令）或 Reviewer 拒绝合并。
> 变更门槛：新增字段或修改类型 → 需 ADR。

---

## 1. 目标

## 2. 范围

## 3. 类型定义

三层（Host / Skill / Platform）× 三维（资源访问 / 副作用 / 风险级）的矩阵。

```
| Capability           | Resource | Side Effect | Risk | Approval |
|---------------------|----------|-------------|------|----------|
| WindowEnumerate     | read     | none        | L1   | auto     |
| WindowRead          | read     | none        | L1   | auto     |
| WindowActivate      | invoke   | UI focus    | L2   | auto     |
| FileRead            | read     | none        | L1   | auto     |
| FileWrite           | write    | disk        | L2   | auto     |
| FileDelete          | destroy  | disk        | L3   | required |
| ProcessSpawn        | invoke   | proc        | L3   | required |
| NetworkEgress       | send     | net         | L4   | required |
| MoneySend           | send     | ext-system  | L5   | forbidden |
| ClipboardRead       | read     | clipboard   | L2   | auto     |
| ClipboardWrite      | write    | clipboard   | L2   | auto     |
```

## 4. 不变量

1. **单调升级**：L1 → L5 风险越升越高；不允许 downgrade（L3 不能声明成 L2）。
2. **Approval 强制**：L3+ 必须 `Approval = required`；L5 必须 `Approval = forbidden`（= 强制拒绝）。
3. **Resource 类型枚举**：read / write / send / invoke / destroy = 5 类，不允许模糊（例 "modify"）。
4. **SideEffect 必填**：必须明确声明 `none` / 列举副作用（= `focus change` / `proc spawn` / `net egress`）。

---

## 5. 与其他 spec 的关系

(本节 = tool-schema 的 capabilities 字段 + error-codes::PolicyDenied + ipc-protocol 权限协商)

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
