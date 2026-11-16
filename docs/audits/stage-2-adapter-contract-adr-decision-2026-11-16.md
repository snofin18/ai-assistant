# Stage 2 Adapter 契约 ADR 裁决草案

状态：**Decision Draft，待人类裁决**　版本：1.0　日期：2026-11-16
关联：TASK-373、`stage-2-adapter-contract-readiness-2026-11-16.md`、[ADR:待建 0005]、ADR-0022、
ADR-0055、ADR-0062

> 本文是决策输入，不是 Accepted ADR 或 spec。任何“必须”仅表示建议中的候选规范；
> 在人类批准并回写 `docs/adr/**` / `docs/spec/**` 前，代码现状与既有契约不变。

## 1. 基线与不变量

TASK-373 已证明四套包都有六类声明文件，但只有部分运行时消费者生效：

| 对象 | 当前事实 |
|---|---|
| `adapter.toml`、富 App Map、rollback pack、interrupt pack | 无 Rust consumer |
| core App Map v1 | 仅 Notepad/Paint 有，由 `AppMapLoader` 严格消费 |
| selector | Notepad/Paint 被 target loader 消费；browser 是另一套扁平 shell target |
| tool | 四包都被注册器消费，但外层 `postconditions` 被忽略 |
| validator | Paint 与 Edge/Chrome 各一套 Python 局部校验，未接 xtask/CI；Notepad 无 pack validator |

新增契约必须保持四条不变量：

1. 每项事实只有一个 owner；其他文件只能引用或由它生成。
2. 未知版本、未知字段、悬空引用和无法执行的动作一律 fail-closed。
3. 文件存在不等于运行时生效；validator 必须区分“声明”“被消费”“被测试”。
4. DOM selector、UIA candidate、rollback recipe、interrupt matcher 不得用一个模糊的
   “通用 selector”概念混合语义。

## 2. 六项裁决

### D1 Bundle envelope

**建议**：`adapter.toml` 是六件套唯一入口，新增 `[bundle]` 固定路径表：

```toml
schema_version = 1

[adapter]
app_id = "com.microsoft.excel"

[bundle]
app_map = "app_map.json"
app_map_projection = "memory/app_map.v1.json"
app_map_projection_model = "core_app_map_v1"
selectors = "selectors/targets.json"
tools = "tools/tools.json"
rollback = "rollback/recipes.json"
interrupts = "interrupts/interrupts.json"
```

- 六个逻辑部件全部必填；rollback/interrupt 暂无条目时仍必须为空数组文件。
- 所有路径相对 bundle root，禁止绝对路径、`..`、越过 root 的 symlink 和重复目标。
- 除生成式 core projection 外，顶层文件使用 `schema_version`；遇到未知值拒绝加载，不从其他字段猜版本。
- `core_app_map_v1` 是唯一版本字段例外：保留当前 loader 的 `version/app_id/entries`，由
  `app_map_projection_model` 标识其形状，不向文件额外塞 `schema_version`。
- `app_id` 在 envelope 与各文件中必须逐字相等；版本范围只由 `adapter.toml` 拥有。
- `app_map_projection` 也必填：浏览器当前缺该文件，按迁移表补齐，不以“无 memory consumer”
  形成永久旁路。

**拒绝**：继续按文件是否存在临时推导 bundle；为不同 Adapter 保留多套 section 名；
允许未知版本继续加载。

**负向判据**：缺任一路径、悬空路径、跨 root、`app_id` 漂移、未知版本值或
`app_map_projection_model` 不匹配均拒绝。

### D2 App Map owner

**建议**：富 `app_map.json` 是人工维护的知识源；`memory/app_map.v1.json` 是由它确定性生成的
core 投影，不得手工独立修改。

- 富文件拥有 commands、ui_map、workflows、known_interrupts 等知识和 `source_reference`。
- 投影只拥有 core loader 接受的 `version/app_id/entries`；生成阶段校验每项 source reference，
  但运行时对外 provenance 仍由 loader 产生为 `<projection>#/entries/<index>`。
- 生成器负责同源；validator 重生成到临时内存并逐字节比较，不能只抽查字段。
- selector/tool 引用必须存在；反向不要求每个内部 target/tool 都进入 memory，但跳过项要有
  `reason` 或明确不在 projection scope。
- Edge/Chrome 迁移时补齐 projection；若某条知识无法表达为 v1 entry，必须在 generation report
  中列出，不能静默丢弃。
- 若未来要求运行时追溯到富 App Map，而不是只做生成期校验，那属于 `AppMapLoader`/公共 provenance
  行为变更，必须另立 ADR；本文不把它写成已存在能力。

**拒绝**：两份文件都手写且互不校验；把富文件直接扩成 core v1；浏览器永久豁免 projection。

**负向判据**：projection 与重生成结果有任何字节差异；source reference 悬空；app/version 漂移。

### D3 Selector model

**建议**：selector 文件按 pack 声明 `selector_model`：

- `uia_candidate_chain_v1`：Notepad/Paint 现有 policy + target + scored candidates 形状；
- `shell_target_v1`：browser 当前扁平 shell target 形状，只表达浏览器 shell 的 UIA target。

DOM selector、CDP expression 和 page target identity 不属于该文件；它们继续由 browser tool
参数/runtime contract 管理。两种 model 可共享 target id、probe status、role/supports 等外围字段，
但不得把 UIA `kind/value/score` 强塞进 shell target，也不得把 DOM 字符串当 UIA candidate。

- `selector_model` 未知即拒绝；
- 两种 model 都校验 target id 唯一、role 闭集、probe status 和 declared supports；
- candidate 的 `locale_dependent=true` 不得用可见文本作为唯一/主 selector；
- browser 若声明 `shell_target_v1`，必须同时声明它不提供 core-target resolution。

**拒绝**：为了“统一”强行把 browser target 交给当前不支持它的 loader；建立 expression 语言；
用 target 文件承载 page DOM selector。

**负向判据**：未知 model、混用两种 target 形状、重复 id、唯一 selector 依赖可见文本。

### D4 Tool declaration

**建议**：`protocol/tool-schema/tool-schema-1.0.json` 是现有 `ToolSchema` 字段名与枚举的 canonical
source；`docs/spec/tool-schema.md` 的类型草图必须在新 ADR/spec 中改成与它一致，不能再把
`postconditions` / `capabilities` 描述为协议内字段，也不能继续使用 `risk_class` 旧名。
Adapter bundle 增加一层严格 envelope：

```text
{
  schema: ToolSchema,
  capabilities: [CapabilityId],
  postconditions: [Postcondition; >=1]
}
```

- envelope 和 `ToolSchema` 都 `additionalProperties=false`；注册器必须反序列化完整结构，
  不能再把外层 `postconditions` 忽略。
- `risk_level` 是 canonical 字段名；`capabilities` 由 Adapter envelope 拥有，进入 registry 前必须
  全部命中 `capability-matrix`。这两项 owner 若被否决，新 ADR 必须给出替代落点。
- 每个 write 必须有至少一个可执行 postcondition；read 至少声明
  `read_only_no_state_change` 或等价的真实证据后置条件。
- app-owned `effect`、`reversibility`、`risk_level`、`requires_approval` 由声明进入 registry，
  model/planner 不得覆盖。
- 推荐给 `ToolReversibility` **加法式**新增 `none_readonly`，仅允许 `effect=read`；write 仍只允许
  L0~L3。获批前保留现有私有归一化并标注迁移项，禁止再复制到第三个 registry。
- validator 对 `none_readonly` / read-write 组合建立正反例；任何未知值继续 fail-closed。

理由：当前 `parse_tool_schema` 和 runtime-tools 常量都为 `none_readonly` 做私有归一化，
证明 spec 与实际 Adapter 已不一致；同时 spec 的 `risk_class`/内层 `postconditions` 与协议
schema 的 `risk_level`/无内层后置条件也不一致。继续依赖 binary 特判会把这些事实复制到 Excel。

**拒绝**：把 `none_readonly` 继续解析为 `l0_undo_stack` 而不改 spec；让每个 Adapter 自带
归一化；允许 adapter-level `postconditions` 仅作文档。

**负向判据**：未知外/内字段被吞；write 无 postcondition；capability 未登记；`none_readonly`
用于 write；spec 与 canonical schema 字段名不一致；声明字段与 policy 注入字段不一致。

### D5 Rollback contract

**建议**：rollback pack 对每个 `effect=write` 工具必须有恰好一条覆盖记录：

- L0/L1/L2：至少一个可执行 `anchor_strategy`，指向已声明快照/恢复路径；
- L3：允许 `strategy = "no_recovery"`，但必须给 `reason` 且工具同时为 high/critical、
  `requires_approval=true`、非 unattended；
- read 工具不得有 recipe；
- recipe 引用的 source/restore/post_verify 必须落入闭集或引用已声明 tool；
- L1 不能声称“有快照”却没有存储或恢复路径；L2 不能声称“补偿”却没有可执行动作；
- browser navigate/form_fill/download 等现有 write 工具在包收敛时补行，不能用“有统一 runtime
  undo stack”替代声明。

**拒绝**：只看已存在的 recipe 引用；把 L3 缺 recipe 当自动合法；用字符串描述冒充可执行恢复。

**负向判据**：任一 write 缺覆盖行；read 有 recipe；L0~L2 为空策略；L3 无显式 reason；
恢复引用悬空。

### D6 Interrupt grammar

**建议**：interrupt pack 使用小型、闭合、可机器匹配的 grammar：

- match 仅允许 `class_name`、`automation_id`、`automation_id_candidates`、`title_candidates`、
  `name_candidates`、`class_name_contains`；文本字段必须标 `locale_dependent=true`。
- action 闭集为 `cancel`、`dismiss_explicitly`、`cancel_unless_declared_by_tool`、
  `delegate_to_tool`、`ask_human`；`delegate_to_tool` 的目标必须是已声明 tool，不再引用历史 task id。
- 默认 action 允许 `cancel`、`dismiss_explicitly`、`ask_human`；`cancel_unless_declared_by_tool`
  只允许在声明 tool 明确接管时继续；`save`、`discard`、`replace`、`accept` 永久不能成为默认。
- untrusted input、credential、captcha、overwrite、三态保存框必须 `requires_human=true`；
  resume condition 必须显式。
- 匹配歧义、多个 regex-like substring 同时命中、目标 tool 悬空或动作未知时 fail-closed。

**拒绝**：自由字符串 action；用固定 sleep 等弹窗自行消失；用本地化标题作唯一匹配；
默认替用户选择保存/丢弃/覆盖。

**负向判据**：未知 match/action；默认危险动作；human-only interrupt 可无人继续；
`delegate_to_task_*` 悬空或无法解析到 tool。

## 3. 四包迁移影响

| Adapter | D1 | D2 | D3 | D4 | D5 | D6 |
|---|---|---|---|---|---|---|
| Notepad | 补 `[bundle]` | 生成现有 projection | `uia_candidate_chain_v1` | 独立 validator；移除私有归一化前需新 spec | 4 个 write 中 Save As 需 L3 显式 no-recovery 行 | 接受 `automation_id_candidates`；`delegate_to_task_038` 改 tool/delegate |
| Paint | 补 `[bundle]` | 生成现有 projection，校验 source refs | `uia_candidate_chain_v1` | 5 个未覆盖 recipe 的 write 逐项裁决 | 13 个 write 全覆盖记录；当前 8 有 recipe、5 待裁决 | 保留 `cancel_unless_declared_by_tool` / teaching-tip `dismiss_explicitly`；`delegate_to_task_045` 改 tool/delegate |
| Edge | 补 `[bundle]` + projection | 首建 projection | `shell_target_v1` | postconditions 入消费；read 统一语义 | 4 个 write 全补记录 | 三条 human-only 迁到闭集 |
| Chrome | 同 Edge | 首建 projection | 同 Edge | 同 Edge | 同 Edge | 同 Edge |

迁移不得一次性“大改四包”；先由通用 validator 输出缺口报告，再按 Notepad/Paint 与
Edge/Chrome 两批收敛，保持 owner 隔离。

## 4. 后续五步

1. **ADR/spec 设计卡（人类闸门）**：冻结 D1~D6，写 schema 兼容和迁移条件。
2. **通用只读 validator 卡**：检查四包并输出逐项报告，不自动改包。
3. **现有包收敛卡 A/B**：A = Notepad/Paint；B = Edge/Chrome；按 owner 分开。
4. **Excel Adapter pack 卡**：只消费已冻结并已通过 validator 的契约。
5. **COM lifecycle 卡**：与 pack schema 解耦，覆盖成功、异常、取消和进程回收。

第 1 步必须产出新 ADR 编号（当前登记表下一可用号为 0104）与对应 spec 变更；第 2 步之后才可改
Adapters 或 Rust loader。Excel 与 COM 不得抢跑。

## 5. 人类裁决清单

1. 接受六槽 `[bundle]` 全必填，还是允许按 capability 条件必填？
2. 接受富 App Map ↔ core projection 的“源 + 确定性生成”关系吗？
3. 接受 `uia_candidate_chain_v1` 与 `shell_target_v1` 两个显式 model，而非强行统一 AST 吗？
4. 是否为 `ToolReversibility` 加法式加入 `none_readonly`？
5. 是否要求所有 write 都有 rollback 覆盖行，L3 用显式 no-recovery？
6. 是否接受 D6 的五动作闭集并淘汰历史 task-id delegate？

任一答案改变，决策包必须按裁决更新；不得只改实现不改契约。

## 6. 合并证据

- PR：#738。
- checks：**11/11 success**，pending 0 / failed 0。
- base：`main`；状态：`MERGEABLE` / `CLEAN`。
- merge commit：`7e5805c2e6280cde07fb5f780eff09ecf278c771`。
- post-merge main CI：`43863545233` success。
