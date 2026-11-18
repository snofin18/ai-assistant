# spec: Tool/Adapter/审计事件 schema 单一事实源

> 摘要：Tool / Adapter / 审计事件 JSON Schema 规范。本 spec 是本项目**契约层**的一部分，由 ADR 批准后即作为 xtask 卡实施 + CI 机器校验的权威。
> 状态：Draft（ADR-0055、ADR-0104 与 ADR-0105 已批准字段变更；schema 1.1 与生成类型已实现，通用只读 Adapter validator 待后续卡）　版本：0.6　日期：2026-11-18
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
    version: String,         // canonical ToolSchema meta-schema version
    name: String,            // 例: "notepad.file.read"
    description: String,
    input: JsonSchema,
    output: EnvelopeSchema,
    effect: read | write,
    reversibility: l0_undo_stack | l1_snapshot | l2_compensation |
                   l3_irreversible | none_readonly,
    risk_level: low | medium | high | critical,
    requires_approval: bool,
    idempotent: bool,
    tags: Vec<String>,
}

struct AdapterToolEnvelopeV1 {
    schema: ToolSchema,
    capabilities: Vec<CapabilityId>,       // >=1，来自 capability catalog
    postconditions: Vec<Postcondition>,    // >=1，来自 assertion/error-code 契约
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
2. **capability 子集**：Adapter tool envelope 声明的 capability 必须是 capability catalog 中**已存在**的非空唯一集合；否则报 ErrorCode::CapabilityUndeclared。
3. **postcondition 必填**：每个 Adapter tool envelope 至少 1 条可执行 postcondition（= 强制可校验）；只把 postcondition 写在注释或模型参数中不算。
4. **risk_level 单调**：high/critical 或 L3 write 必须 `requires_approval=true`（policy 引擎强制）。
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
9. **行为元数据权威**：`effect` / `reversibility` 是 ToolSchema 必填字段，只能由工具作者声明。
   Planner 不得采用模型自报的这两个字段；模型输出若包含它们必须拒绝，并由 Planner 从工具目录注入。
   依据：**ADR-0055**、TASK-207 独立 review、`DRIFT-207-1`。
10. **Adapter envelope**：`capabilities` 与 `postconditions` 属于 `adapter_tool_envelope.v1`
    外层，不进入 canonical ToolSchema；envelope 与 ToolSchema 均拒绝未知字段。
    依据：**ADR-0104**、TASK-374、TASK-376。
11. **`none_readonly` 受控**：只允许 `effect=read`；任何 write 使用它都拒绝。
    依据：**ADR-0104** D4。

### 4.1 Adapter bundle 与 Tool 文件

`adapter.toml` 的 `[bundle].tools` 必须指向同一 bundle root 内的
`tools/tools.json`。该文件顶层为 `schema_version/app_id/tools`；每个 `tools[]` 项必须符合
`adapter_tool_envelope.v1`。六个 bundle 路径全部必填、路径 containment、未知版本/字段和
悬空引用规则见 **ADR-0104** D1/D4。

### 4.2 兼容窗口

当前 `protocol/tool-schema/tool-schema-1.0.json` 尚未包含 `none_readonly` 和
Adapter envelope。迁移顺序固定为：

1. ADR-0104/spec（本文件）冻结目标契约；
2. protocol schema 1.1 + 生成类型（TASK-380 已实现）+ 通用只读 validator（待后续卡）；
3. Notepad/Paint、Edge/Chrome 两批收敛；
4. Excel Adapter pack。

ToolSchema 1.1 与 `AdapterToolEnvelopeV1` 生成类型已由 TASK-380 落地；ToolSchema 1.0 作为
一个版本的兼容窗口继续保留。通用只读 Adapter validator、运行时 loader/registry 消费与现有
pack 迁移尚未完成，因此 binary 对 `read + none_readonly` 的归一化仍只是兼容路径，不是
运行时已支持 ADR-0104 全 envelope 的证据。

### 4.3 Validator catalog 与 envelope 机器契约

`docs/spec/adapter-bundle.md` 是 Adapter bundle validator 的机器契约：它冻结 envelope
unknown-field rejection、ToolSchema 1.0/1.1 兼容窗口、read/write postcondition matrix 和
跨文件引用规则。

validator 的 capability、ErrorCode 与 postcondition kind 只能引用以下共享事实源：

- capability：`protocol/capability-matrix/capability-1.0.json`
- ErrorCode：`protocol/error-codes/error-codes-1.0.json`
- postcondition：由现有 assertion 契约与 `crates/verify/src/postcondition.rs` 生成的 reviewed
  machine catalog

validator MUST NOT 手写第三份 kind、ErrorCode 或 capability 枚举。catalog 的生成命令、产物路径和
`codegen --check` 集成由紧随其后的 validator implementation 卡实现；在实现卡合并前，不得把
本节写成“catalog 已生成”或“validator 已通过现有 pack”。

`effect=read` 至少需要一个 canonical `state_unchanged`；输出字段存在、`element_exists` 或自由文本
描述都不能单独证明无状态变化。`effect=write` 至少需要一个可执行的 canonical postcondition。
每个 envelope postcondition 必须显式包含 `kind`、非空 `description` 和合法 `error_code`，并按
kind 的封闭字段集合校验。

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
| 0.3 | 2026-09-27 | §3 增加权威 `effect` / `reversibility`，§4 增加不变量 9；依据 **ADR-0055**，关闭 TASK-207 `DRIFT-207-1` |
| 0.4 | 2026-11-17 | §3 对齐 canonical ToolSchema 并把 capabilities/postconditions 归入 `adapter_tool_envelope.v1`；§4 新增不变量 10/11、bundle 与兼容窗口；依据 **ADR-0104** / TASK-376 |
| 0.5 | 2026-11-17 | ToolSchema 1.1 的 `none_readonly` 与 `AdapterToolEnvelopeV1` 生成类型已实现并纳入 `verify-schemas` / `codegen`；通用只读 validator 明确仍未实现；依据 ADR-0104 / TASK-380 |
| 0.6 | 2026-11-18 | 增加 validator catalog 唯一来源、ToolSchema 1.0/1.1 窗口与 read/write postcondition matrix；机器契约转交 `docs/spec/adapter-bundle.md`；依据 ADR-0105 / TASK-383 |
