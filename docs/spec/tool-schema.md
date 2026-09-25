# spec: Tool/Adapter/审计事件 schema 单一事实源

> 摘要：Tool / Adapter / 审计事件 JSON Schema 规范。本 spec 是本项目**契约层**的一部分，由 ADR 批准后即作为 xtask 卡实施 + CI 机器校验的权威。
> 状态：Draft（待 ADR 批准）　版本：0.2　日期：2026-09-25
> 上位：`AGENTS.md` §5、`docs/governance-ai-agent-execution.md` §5/§6
> 强制性：**本文档是契约**。违反即 CI 失败（`xtask` 相关子命令）或 Reviewer 拒绝合并。
> 变更门槛：新增字段或修改类型 → 需 ADR。

---

## 1. 目标

Tool / Adapter / 审计事件三类实体的 **JSON Schema 单一事实源**：本项目所有跨 crate / 跨语言边界的类型都由这些 schema 生成或校验，禁止在 Rust / TS 侧手写重复结构体（ADR-0030 同源动机）。
本 spec 回答三个问题：① 三类实体各自有哪些字段；② 命名与 capability 如何受控；③ schema 自身如何演进（版本、必填、时间戳口径）。

## 2. 范围

**管**：Tool / Adapter / 审计事件的字段构成与命名空间、schema 演进规则（版本 / 必填 / 时间戳），以及 `protocol/**/*.json` 与其生成物的一致性口径。

**不管（不做清单）**：
- 不定义具体工具的**业务语义**：每个 Tool 的 `inputs` / `outputs` 由各 Adapter 的 schema 实例给出，本 spec 只管骨架与约束
- 不定义 capability 的**取值表**（归 `docs/spec/capability-matrix.md`）
- 不定义错误码取值（归 `docs/spec/error-codes.md`）
- 不定义消息包装（归 `docs/spec/envelope.md`）与传输（归 `docs/spec/ipc-protocol.md`）
- 不定义测试分层与覆盖率（归 `docs/spec/testing.md`）

## 3. 类型定义

Tool / Adapter / 审计事件三类的 schema 共享同一基类 `EntityRef`：

```rust
struct ToolSchema {
    name: String,            // 例: "notepad.file.read"
    version: SemVer,         // 例: 1.2.0
    capabilities: Vec<CapabilityId>,  // 来自 capability-matrix
    inputs: JsonSchema,      // 输入 JSON Schema
    outputs: JsonSchema,     // 输出 JSON Schema
    postconditions: Vec<Postcondition>,  // 来自 docs/spec/error-codes.md 的 ErrorCode 列表
    risk_class: RiskClass,    // L1 / L2 / L3 / L4
}

struct AdapterSchema {
    app: String,              // 例: "notepad.exe"
    version: SemVer,          // 适配的应用版本
    platform: PlatformId,     // 例: "windows"
    supported_capabilities: Vec<CapabilityId>,
    lifecycle: LifecycleSpec, // 安装 / 升级 / 卸载的版本兼容矩阵
}

struct AuditEventSchema {
    event_kind: String,        // 例: "tool.invoked"
    timestamp: Iso8601,
    actor: ActorId,            // 哪条 Tool 调用
    subject: TargetDescriptor, // 被操作的目标
    outcome: Outcome,
}
```

### 命名空间

- Tool：`tool.<app>.<domain>.<action>`（与 ADR-0021 受控词一致）
- Adapter：`adapter.<app>.<version>`
- AuditEvent：`audit.<event_kind>`

## 4. 不变量

1. **name 反向 DNS**：`name` 必须 `<app>.<domain>.<action>` 三段式；与 ADR-0021 §3 受控词一致。
2. **capability 子集**：Tool schema 声明的 capability 必须是 capability-matrix 中**已存在**的；否则报 ErrorCode::CapabilityUndeclared。
3. **postcondition 必填**：每个 Tool 至少 1 条 postcondition（= 强制可校验）。
4. **risk_class 单调**：L3+ 必有 `requires_human_approval=true` 字段（policy 引擎强制）。
5. **schema_version 单调递增**：schema 从 1 起；字段重命名 / 类型变化 → 新增 version，旧字段标 `@deprecated` 并保留 ≥2 个版本（= 无破坏性变更）
6. **必选字段不可为空**：`required` 列出的字段在所有实例中**非 null / 非空字符串 / 非空数组**
7. **时间戳用 ISO-8601**：所有时间字段一律 UTC + RFC 3339（形如 `2026-09-19T12:34:56Z`）
8. **校验关键字白名单**：`inputs` / `outputs` 的 schema 只允许 `crates/tool-bus` 校验器**显式支持**的
   draft-07 关键字子集（`SUPPORTED_KEYWORDS`，21 条）；在此之外的任何影响语义的关键字一律**拒绝注册**
   （fail-closed），且拒绝消息必须带**标签 + 一句理由 + schema 文档内 JSON pointer**
   （表见 `REJECTED_FOREVER_KEYWORDS` / `REJECTED_FOR_NOW_KEYWORDS` / `NON_DRAFT07_KEYWORDS`）。
   纯注解关键字（`ANNOTATION_KEYWORDS`，9 条）**忽略但不拒绝**。`$schema` **不是注解**：缺省 或
   draft-07 = 放行，声明**其它方言 = 拒绝**（不许用 draft-07 的语义冒充 2020-12 的文档）。
   依据：**TASK-204**（人类 chat 2026-09-25 授权）；理由与替代见 `crates/tool-bus/README.md`
   「已知限制」与 `docs/memory/rejected.md` 2026-09-25 各条。

---

## 5. 与其他 spec 的关系

| 引用方向 | spec | 关系 |
|---|---|---|
| 依赖 | `docs/spec/naming.md` | `name` 反向 DNS 三段式与受控词汇表（ADR-0021 D7） |
| 依赖 | `docs/spec/capability-matrix.md` | Tool 声明的 capability 必须是矩阵中**已存在**的项 |
| 依赖 | `docs/spec/error-codes.md` | `postconditions` 与一切错误字段只用 ErrorCode 枚举 |
| 被依赖 | `docs/spec/envelope.md` | Tool 输入 / 输出作为 envelope 的 payload 传输 |
| 被依赖 | `docs/spec/audit-event.md` | 每次 Tool 调用结果必须产出 audit event |
| 被依赖 | `docs/spec/ipc-protocol.md` | 跨进程时 Tool 消息体经 envelope 走该协议 |
| 相关 | `docs/spec/testing.md` | contract 测试覆盖 schema 边界（空 / null / 重复 ID / 超大值） |

---

## 附录：演进记录

| 版本 | 日期 | 变更 |
|---|---|---|
| 0.1 | 2026-09-20 | 初稿（项目进度督察后批量补齐 stage-0 DoD #5）|
| 0.2 | 2026-09-25 | §4 新增**不变量 8**（校验关键字白名单 + `$schema` 方言校验 + 「不支持即拒绝」的显式化）；依据 **TASK-204**（TASK-020 §9 关注点 3 的落地物）|
