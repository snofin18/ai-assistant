# spec: 能力矩阵（capability matrix）

> 摘要：Capability matrix 规范（Host / Skill / Platform 三层 × 资源访问 / 副作用 / 风险级）。本 spec 是本项目**契约层**的一部分，由 ADR 批准后即作为 xtask 卡实施 + CI 机器校验的权威。
> 状态：Draft（待 ADR 批准）　版本：0.1　日期：2026-09-20
> 上位：`AGENTS.md` §5、`docs/governance-ai-agent-execution.md` §5/§6
> 强制性：**本文档是契约**。违反即 CI 失败（`xtask` 相关子命令）或 Reviewer 拒绝合并。
> 变更门槛：新增字段或修改类型 → 需 ADR。

---

## 1. 目标

定义**能力矩阵**：每个 capability 的资源访问类别、副作用、风险级与审批要求，作为策略引擎（**默认拒绝**）的唯一放行依据。

> **名称澄清（ADR-0042）**：本 spec 的「能力矩阵」= **每条能力的风险·审批声明**（Rust `CapabilityEntry`）， > 它是**策略引擎的输入**（TASK-021）。它与另外两个**同名但不同物**的概念不是一件事： > ① **`CapabilityCatalog`** = `protocol/capability-matrix/capability-1.0.json` 的**稳定能力标识目录** > （`<layer>.<capability>` 2 段式 + `stability` + `version`，**不含**风险/审批列）； > ② **`CapabilityMatrix`** = 架构 v2 §13.1.2 的**运行时探测结果**（Rust `assistant_platform_api::CapabilityMatrix`： > `probed_at` / `platform` / `session` / `channels` / `degradations`）。

## 2. 范围

**管**：capability 的三个维度（Resource / SideEffect / Risk）+ Approval 列、风险单调升级规则、Resource 与 Risk 的枚举白名单。

**不管（不做清单）**：
- 不管**策略规则 DSL** 与判定算法（归 TASK-021 `crates/policy`；本 spec 只给矩阵数据与不变量）
- 不管 Tool 与 capability 的**绑定**（归 `docs/spec/tool-schema.md` 的 `capabilities` 字段）
- 不管审批 UI / 人工确认交互（归产品层）
- 不定义 L5（资金 / 对外发布类）之外的业务语义；L5 **永久禁止自动化**（铁律 6）

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

| 引用方向 | spec | 关系 |
|---|---|---|
| 被依赖 | `docs/spec/tool-schema.md` | Tool 声明的 `capabilities` 必须是本矩阵中**已存在**的项 |
| 被依赖 | `docs/spec/error-codes.md` | 矩阵的 Risk / Approval 列对应 `PolicyDenied` / `ApprovalRequired` |
| 被依赖 | `docs/spec/ipc-protocol.md` | 握手阶段交换 capability 列表用于权限协商 |
| 相关 | `docs/spec/audit-event.md` | 策略判定结果（deny / approval）必须落 audit event |

---

## 附录：演进记录

| 版本 | 日期 | 变更 |
|---|---|---|
| 0.1 | 2026-09-20 | 初稿（项目进度督察后批量补齐 stage-0 DoD #5）|
