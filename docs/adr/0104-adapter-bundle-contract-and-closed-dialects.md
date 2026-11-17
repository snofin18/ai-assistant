# ADR-0104：Adapter 六件套 bundle 与封闭语法契约

状态：**Accepted**（2026-11-17，人类一次性批准 TASK-376 D1-D6）
日期：2026-11-17
Supersedes：无
Superseded by：无
关联：ADR-0022、ADR-0028、ADR-0031、ADR-0041、ADR-0055、ADR-0062、ADR-0083、
TASK-373、TASK-374、TASK-376
落点：`docs/spec/tool-schema.md`、`docs/spec/capability-matrix.md`、
`docs/spec/naming.md`

## 背景

TASK-373 证明 Notepad、Paint、Edge、Chrome 虽然都有六类声明文件，但缺少统一的
bundle envelope、owner、兼容和失败规则：`adapter.toml`、富 App Map、rollback pack 与
interrupt pack 没有 Rust consumer；Edge/Chrome 缺 core App Map projection 与 rollback；
selector 文件存在两套形状；tool 外层 `postconditions` 被当前 loader 忽略；
`none_readonly` 只靠 binary 私有归一化；部分 write 没有 rollback 覆盖；interrupt 仍含
`delegate_to_task_*` 历史动作。

TASK-374 把上述缺口收敛为 D1-D6 决策包。人类在 2026-11-17 明确批准六项全部建议：
六路径 bundle containment、富 App Map + 确定性 projection、两种显式 selector model、
Adapter tool envelope + read-only `none_readonly`、write rollback 全覆盖，以及 interrupt
五动作闭集。

## 决策

一句话：**Adapter 以 `adapter.toml` 的六路径 bundle 为唯一入口；App Map、selector、tool、
rollback 与 interrupt 各自只有一个 owner 和封闭可验证方言；未知版本、未知字段、越界路径、
悬空引用或未声明动作一律 fail-closed。**

本 ADR 冻结目标契约；`protocol/**`、loader、validator 与现有 Adapter pack 的迁移由后续卡
实现。本 ADR 不声明当前二进制已经执行全部新规则。

## D1　Bundle envelope

`adapter.toml` 顶层必须声明 `schema_version = 1`，并包含且只包含以下 `[bundle]` 键：

```toml
[bundle]
app_map = "app_map.json"
app_map_projection = "memory/app_map.v1.json"
app_map_projection_model = "core_app_map_v1"
selectors = "selectors/targets.json"
tools = "tools/tools.json"
rollback = "rollback/recipes.json"
interrupts = "interrupts/interrupts.json"
```

- 六个**路径槽**全部必填：`app_map`、`app_map_projection`、`selectors`、`tools`、
  `rollback`、`interrupts`；`app_map_projection_model` 是必填枚举，不是第七个路径槽。
- rollback 或 interrupts 暂无条目时仍必须提供合法空数组文件，不得省略路径。
- 每个路径相对 bundle root，使用 `/` 分隔；绝对路径、空路径、`.`、`..`、重复规范化目标、
  经 canonicalize 后越出 root、或通过 symlink 越出 root 均拒绝。
- 六个目标必须存在、可读、可解析；同一文件不得占用多个路径槽。
- `adapter.toml` 中的 `app_id` 必须与各文件顶层 `app_id` 逐字相等。
- 除 core App Map projection 的 `version/app_id/entries` 形状外，各文件顶层必须声明
  `schema_version = 1`。未知版本值拒绝。
- 所有文件按 `additionalProperties=false` 语义解析；未知字段拒绝，不从别名、其他字段或
  binary 私有归一化猜出版本。
- `app_map_projection_model` 当前唯一值为 `core_app_map_v1`；未知值拒绝。

## D2　App Map owner
- `app_map.json` 是人工维护的知识源，拥有 commands、ui_map、workflows、known_pitfalls、
  state machine、undo capability 与应用特有约定。
- `memory/app_map.v1.json` 是从富 App Map 确定性生成的 core loader 投影，只含
  `version/app_id/entries`；不得作为独立人工事实源。
- 投影中的每个 entry 必须有非空 `source_reference`；引用指向富 App Map、selector、
  rollback 或明确的应用/平台记忆锚点。悬空引用拒绝。
- selector/tool 等 bundle 引用必须可解析；不能进入 core projection 的知识必须在生成报告中
  明确说明原因，不得静默丢失。
- 生成器必须稳定排序、稳定缩进和固定末尾换行；validator 重新生成后必须与已提交投影字节一致。
- 运行时 provenance 由 `AppMapLoader` 基于投影路径生成
  `<projection>#/entries/<index>`；不得伪称该运行时路径可直接定位富 App Map。

## D3　Selector model
selector 文件必须有 `selector_model`，且只允许两个值：

### `uia_candidate_chain_v1`

- 适用于 Notepad/Paint 等 UIA target。
- 顶层包含 `schema_version/app_id/selector_model/app_version_range/policy/targets`。
- 每个 target 包含唯一 `id`、`scope` 和非空 `candidates`；candidate 使用显式 `kind/value/score`
  与 `locale_dependent`。
- policy 的歧义行为必须先整组尝试非 fallback 候选，全部失败后再尝试 fallback 候选；
  `TargetAmbiguous` 不得降级为任取首个。

### `shell_target_v1`

- 适用于 Edge/Chrome 的浏览器 shell 目标。
- 顶层包含 `schema_version/app_id/selector_model/targets`。
- target 只表达 shell 的 `id/role/class_name/automation_id/supports/probe_status` 等字段。
- 该 model 明确不提供 DOM/page target resolution。

共同规则：

- target id 唯一，role/supports/probe status 使用封闭词表；未知值拒绝。
- UIA `kind/value/score` 不得塞入 shell target；DOM/CSS/XPath/JavaScript 表达式不得作为
  selector bundle 的 target identity，继续由 browser tool 参数和 runtime contract 管理。
- `locale_dependent=true` 的可视文本不得成为唯一或主 selector。

## D4　Tool declaration
`tools/tools.json` 顶层包含 `schema_version/app_id/tools`。每个 `tools[]` 项是
`adapter_tool_envelope.v1`：

```text
{
  schema: ToolSchema,
  capabilities: [CapabilityId; >=1],
  postconditions: [Postcondition; >=1]
}
```

- `protocol/tool-schema/tool-schema-1.0.json` 继续拥有 `ToolSchema` 基础字段名和枚举；
  `adapter_tool_envelope.v1` 是该 schema 的外层 Adapter declaration，不是另一套 ToolSchema。
- `capabilities` 位于 envelope，不塞进 `ToolSchema`；每项必须命中 capability catalog，重复项拒绝。
- `postconditions` 位于 envelope，每项必须可由现有/已声明 assertion kind 执行；空数组拒绝。
- `schema.effect=read` 的最少一条 postcondition 必须证明无状态变化或等价证据；
  `schema.effect=write` 必须至少有一个可执行 postcondition。
- `reversibility` 加法式增加 `none_readonly`：
  - 仅允许与 `effect=read` 组合；
  - 任何 write 使用 `none_readonly` 都拒绝；
  - write 继续只允许 `l0_undo_stack`、`l1_snapshot`、`l2_compensation`、
    `l3_irreversible`。
- canonical 字段名是 `risk_level`，不是 `risk_class`。
  `effect`、`reversibility`、`risk_level`、`requires_approval`、`idempotent`
  只能由 Adapter 声明进入 registry，模型不得覆盖。
- envelope 和 ToolSchema 都按 unknown-field rejection 解析；外层字段被忽略不算通过。

兼容窗口：

- 当前 `protocol/tool-schema-1.0.json`、`parse_tool_schema` 与 runtime registry 尚未实现
  `none_readonly` 和 envelope 全字段消费；现有 binary 对 `read + none_readonly` 的归一化
  只是迁移兼容。
- 后续实现卡必须先升级 protocol meta-schema/生成类型并新增 adapter validator，再迁移现有包；
  在 schema 1.1 门禁落地前，不得宣称 envelope 已由运行时强制执行。

## D5　Rollback coverage
rollback 文件顶层包含 `schema_version/app_id/recipes`。每个 `effect=write` 工具必须恰好有
一行 coverage：

```text
{
  tool: ToolName,
  coverage: "recipe" | "no_recovery",
  reason: String,                 // 仅 no_recovery 必填，非空
  anchor_strategy: [Anchor; >=1]  // 仅 recipe 必填
}
```

- read tool 不得有 rollback coverage 行。
- L0/L1/L2 只允许 `coverage=recipe`；`anchor_strategy` 至少一项，必须声明 level、mechanism、
  restore/snapshot 语义与 post-verify。
- L3 只允许显式 `coverage=no_recovery`，附非空理由；工具必须同时为 high/critical、
  `requires_approval=true`，且不得进入 unattended。
- L1 必须提供真实快照存储与恢复路径；L2 必须提供可执行补偿动作；只写字符串描述不算覆盖。
- recipe 引用的 source/restore/post-verify 与 target tool 必须可解析到封闭动作或已声明 tool。
- 同一 tool 重复 coverage、多行歧义、悬空引用或 L3 缺 reason 均 fail-closed。

## D6　Interrupt grammar
interrupt 文件顶层包含 `schema_version/app_id/default_action/interrupts`。每个 interrupt 必须
有唯一 `id`、非空 `match` 和一个闭集动作。

允许的 match 键仅：

- `class_name`
- `automation_id`
- `automation_id_candidates`
- `title_candidates`
- `name_candidates`
- `class_name_contains`

允许的动作仅：

- `cancel`
- `dismiss_explicitly`
- `cancel_unless_declared_by_tool`
- `delegate_to_tool`
- `ask_human`

规则：

- 凡是可见文本匹配字段，其候选必须明确 `locale_dependent=true`；文本不得成为唯一匹配。
- `delegate_to_tool` 必须给出同一 bundle 中已声明 tool；历史 `delegate_to_task_*` 永久拒绝。
- `cancel_unless_declared_by_tool` 只有在声明 tool 明确接管时继续，否则按 cancel。
- `save/discard/replace/accept` 等危险动作不得成为 default_action 或 fallback。
- untrusted input、credential、captcha、覆盖确认、三态保存框必须 `requires_human=true`，
  并声明明确 resume condition。
- 歧义 match、多规则同时命中且无显式优先级、未知 match/action、目标 tool 悬空时 fail-closed。
- 固定 sleep、等待弹窗自行消失或按本地化标题猜测唯一性都不属于合法处理。

## 兼容与迁移顺序

1. **本 ADR/spec**：冻结 owner、字段、闭集与负向判据，不修改产品代码。
2. **protocol schema 1.1 + 通用只读 validator**：实现 `none_readonly`、envelope、bundle
   containment、App Map projection、rollback coverage 与 interrupt grammar 的机器校验。
3. **现有包收敛 A**：Notepad/Paint 补 bundle envelope、projection 验证、tool envelope、
   rollback 全 write 覆盖和 interrupt 动作迁移。
4. **现有包收敛 B**：Edge/Chrome 补 projection、rollback、selector model 与 tool validator。
5. **Excel Adapter pack**：只消费已冻结且通过 validator 的契约。
6. **COM lifecycle**：与 pack schema 解耦，继续覆盖成功、异常、取消与进程回收。

兼容窗口：

- `adapter.toml`、六件套 JSON 与 validator 的当前版本继续 fail-closed 运行，不因本 ADR
  自动放宽。
- protocol 1.0 到 1.1 必须保留一个版本窗口；旧字段如需弃用，至少保留一个 schema 版本的
  deprecated 标记，不得静默改义。
- 现有 pack 在新 validator 上线前保持原状；validator 首次接入 CI 时先以只读报告模式暴露
  缺口，再由独立收敛卡逐包修复，禁止自动改写 packs。

## 负向判据总表

| 决策 | 必须拒绝 |
|---|---|
| D1 | 缺 bundle 键、未知 bundle 键、绝对/`..`/symlink 越界、重复目标、未知版本、app_id 漂移 |
| D2 | 手工分叉 projection、悬空 source reference、重新生成字节不一致、静默丢弃知识 |
| D3 | 未知 selector_model、混用两套 target 形状、重复 id、可见文本唯一 selector、DOM selector 冒充 target |
| D4 | 未知 envelope/schema 字段、空 capabilities、悬空 capability、空 postconditions、write 使用 `none_readonly`、模型覆盖权威元数据 |
| D5 | write 缺覆盖、read 有覆盖、重复覆盖、L0~L2 空 recipe、L3 无 `no_recovery`/reason/人工批准、悬空恢复引用 |
| D6 | 未知 match/action、危险默认动作、human-only 可自动继续、`delegate_to_task_*`、悬空 tool、歧义规则无优先级 |

## 后果

正面：

- 六个 Adapter 声明文件从“约定存在”升级为同一套可验证 bundle。
- App Map、selector、tool、rollback、interrupt 各有唯一 owner，避免双事实源。
- 后续 validator、现有包收敛与 Excel pack 有明确实现顺序。

代价：

- protocol 1.0 与运行时 registry 存在迁移窗口，期间必须明确区分“目标契约”和“已执行门禁”。
- 现有四套包会有可观的一次性收敛工作；未补齐的包在 validator 接入后将被显式阻塞。
- `none_readonly` 是受控加法，仍需 schema/生成类型/loader 同步，不能只改文档就宣称支持。

## 验证方式

1. `cargo run -p xtask -- adr-index` 确认 ADR-0104、登记表与 decisions 三方一致。
2. `cargo run -p xtask -- verify-schemas`、`codegen --check` 证明本次没有偷改 canonical schema。
3. 文档负向扫描确认三份 spec 均出现 D1-D6 的 owner、兼容窗口与拒绝判据。
4. 产品路径零差异：`git diff --name-only` 不包含 `protocol/**`、`adapters/**`、
   `crates/**` 或 `apps/**`。
5. 后续实现卡的测试首次覆盖 untracked/越界 bundle、projection 字节漂移、selector model
   混用、tool envelope unknown field、rollback 全覆盖矩阵和 interrupt 闭集。

## 何时重新讨论

需要新增第三个 selector model、改变 App Map owner、允许 write 使用 `none_readonly`、
放宽 L3 rollback 要求、增加 interrupt match/action，或把 bundle 路径改成 capability
条件必填时，必须新 ADR，不得在本 ADR 上静默扩展。
