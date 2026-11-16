# Stage 2 Adapter 契约就绪审计

日期：2026-11-16

## 1. 范围与方法

本审计对照 Stage 2 计划的六件套：

1. `adapter.toml`
2. App Map
3. selector
4. tool
5. rollback
6. interrupts

审计对象为当前已有真实运行路径的 Notepad、Paint、Edge、Chrome 四套 Adapter。只读检查文件结构、
跨文件引用、Rust/Python 消费路径和现有验证器；不修改 `adapters/**`、`docs/spec/**`、
`docs/adr/**`、`protocol/**` 或产品代码。

基线：`origin/main` = `496789bfbeafa52c8aa39cd7eca7e0e42551db8f`。

## 2. 文件就绪矩阵

`OK` = 文件存在且 JSON/TOML 可解析；`MISS` = 文件不存在。

| Adapter | adapter.toml | 富 App Map | core App Map v1 | selector | tool | rollback | interrupts |
|---|---|---|---|---|---|---|---|
| `com.microsoft.notepad` | OK | OK | OK | OK | OK | OK | OK |
| `com.microsoft.paint` | OK | OK | OK | OK | OK | OK | OK |
| `browser.edge` | OK | OK | MISS | OK | OK | MISS | OK |
| `browser.chrome` | OK | OK | MISS | OK | OK | MISS | OK |

结论：Notepad/Paint 是完整七文件包；Edge/Chrome 仅五件套，缺 core App Map 投影和 rollback。
“文件存在”仍不等于“运行时生效”，下一节按消费路径区分。

## 3. 当前声明规模

| Adapter | selectors | tools | rollback recipes | interrupts | core App Map entries |
|---|---:|---:|---:|---:|---:|
| Notepad | 9 | 5 | 3 | 3 | 5 |
| Paint | 78 | 15 | 8 | 7 | 5 |
| Edge | 7 | 11 | 0 | 3 | MISS |
| Chrome | 7 | 11 | 0 | 3 | MISS |

结构版本不统一：

- 六类 JSON 多数使用 `schema_version: 1`；
- core App Map 使用 `version: 1`，且严格只接受 `version/app_id/entries`；
- 富 App Map 使用 `schema_version: 1`，没有 core loader 可接受的 `version`；
- 四种 TOML 都声明 `schema_version = 1`，但 section/字段集合不同。

## 4. 运行时消费矩阵

| 六件套 | 当前真实消费 | 证据与边界 |
|---|---|---|
| `adapter.toml` | 无 Rust 消费 | `rg "adapter.toml" crates apps xtask -g '*.rs'` 无匹配；仅 README 与 Python validator 读取 |
| 富 App Map | 无 Rust 消费 | `rg "app_map.json" crates apps xtask -g '*.rs'` 无匹配 |
| core App Map v1 | `assistant-core::AppMapLoader` | 严格解析 `version/app_id/entries`；每次由请求给相对路径；browser 两包没有投影文件 |
| selector | Notepad/Paint 被 target loader 消费 | `NotepadTargetCatalog::load` 读取候选链；Edge/Chrome 的扁平 shell target 形状不受该 loader 支持 |
| tool | Notepad/Paint/Edge/Chrome 被注册器消费 | 注册器只反序列化每条声明的 `schema` 字段；外层 `postconditions` 当前不进入运行时 |
| rollback | 无 pack JSON 消费 | `rg "rollback/recipes.json" crates apps xtask -g '*.rs'` 无匹配；`assistant-undo` 使用另一套 Rust typed recipe |
| interrupts | 无 Rust 消费 | `rg "interrupts/interrupts.json" crates apps xtask -g '*.rs'` 无匹配；只在 Python validator/文档中被读取 |

### 4.1 Tool 注册子集

| Adapter | 声明数 | 当前挂载数 | 说明 |
|---|---:|---:|---|
| Notepad | 5 | 5 | `Exact` 选择 |
| Paint | 15 | 15 | `Subset` 选择，目前所有声明工具都在期望集合 |
| Edge | 11 | 6 | 只挂载 `navigate`、`dom_read`、`form_fill`、`form_read`、`form_submit`、`wait_network_idle` |
| Chrome | 11 | 6 | 同上；`download`、`screenshot`、三项 shell tool 仍未挂载 |

注册器 `DeclaredTool { schema: Value }` 不声明外层字段，因此 `postconditions` 会被 serde 忽略。
同时，`read + none_readonly` 在 binary 层被精确归一化成 `l0_undo_stack`，其余未知可逆性仍
fail-closed。这两项是当前兼容边界，不是六件套 schema 已正式化的证据。

## 5. 跨文件契约缺口

### 5.1 App Map owner 未冻结

Notepad/Paint 同时维护富知识与 core v1 投影，且投影中的 `source_reference` 指回富 App Map、
selector、tool 或 rollback。两份文件没有机器校验保证同源，也没有统一的富 schema。

Edge/Chrome 有富 App Map，但没有 core v1 投影。因此“App Map”在不同 Adapter 中代表两种不可互换
的数据形状。

### 5.2 selector 包存在两套形状

Notepad/Paint 使用同一候选链外层：

```text
schema_version + app_id + app_version_range + policy + targets
```

Paint 额外有 pack-level `probe_status` 和 target-level `probe_status`。

Edge/Chrome 使用另一套扁平 shell target：

```text
schema_version + app_id + targets[{id, role, class_name, automation_id, supports, probe_status}]
```

浏览器包没有 `policy`、候选链、`app_version_range` 或 `allow_fallback_entry`。把两类文件交给
同一个 loader 会失败，当前也没有针对扁平形状的 Rust loader。

### 5.3 `adapter.toml` 只有约定，没有统一 schema

四包的共同 section 只有 `adapter/platforms/health/safety`。差异包括：

- Notepad/Paint 有 `[adapter].aumid`，浏览器没有；
- 浏览器有 `[profile]`，Notepad/Paint 没有；
- Paint 有画布专用 safety keys，Notepad 有文本专用 keys；
- Paint/浏览器有 `health.probe_status`，Notepad 没有。

这些差异可能合法，但目前没有通用的 required/optional 字段定义，也没有 Rust loader 在启动时拒绝
缺字段或未知版本。

### 5.4 rollback recipe 未覆盖所有声明的可逆写工具

按“tool 的 `effect != read`”统计：

- Notepad：4 个写工具，3 个有 recipe；`notepad.file.save_as` 为
  `l3_irreversible`，未声明 recipe；
- Paint：13 个写工具，8 个有 recipe；未覆盖
  `paint.document.new`、`paint.outline.select`、`paint.fill.select`、`paint.size.select`、
  `paint.canvas.rollback_last_write`；
- Edge/Chrome：各 4 个写工具，0 个 recipe；`l1_snapshot` / `l2_compensation` 声明没有对应
  pack-level recovery definition。

L3 不承诺恢复可以合法，但 L1/L2 的“声明存在、recipe 缺失”必须由统一规则裁决。Paint 的两个 Python
validator 当前只检查 recipe 引用的 tool 是否存在，以及 draw recipe 的 L0/L1，不检查所有 L1/L2
写工具的反向覆盖。

### 5.5 interrupt 文件没有运行时匹配语法消费者

四个包共有 16 条 interrupt 声明，运行时没有 parser 或 matcher。现有 validator 只覆盖局部：

- Edge/Chrome validator 检查三条 human-only interrupt 的 `cancel`；
- Paint validator 只检查 unsaved canvas 的 `cancel` 和 forbidden defaults；
- Notepad 没有 pack validator。

Notepad 的 `save-as-dialog` 使用 `delegate_to_task_038`，Paint 使用
`delegate_to_task_045`；其他条目使用 `cancel` 或 `dismiss_explicitly`。在这些 enum/操作语义未统一前，
不能把文件存在解释为所有 Adapter 有同一 interrupt 行为。

### 5.6 验证覆盖不统一

- Paint：有 `validate_adapter_pack.py`，本次 `PASSED`，覆盖 selector/tool/recipe/interrupt/App Map
  的局部交叉约束；
- Edge/Chrome：有共享 `validate_adapter_packs.py`，本次两包均 `PACK OK`；
- Notepad：只有 JSON/TOML 语法检查和任务卡历史测试，没有独立 pack validator；
- `xtask` 当前没有 adapter-pack 通用校验子命令；
- 两个 Python validator 未被 `xtask` 或 CI workflow 直接调用。

因此当前验证是“每个包各有一套局部检查”，不是六件套统一契约门禁。

## 6. 下一份 ADR 的决策输入

本审计不替人类决定公共契约。ADR 至少需要裁决以下六项：

1. **Bundle envelope**：六件套是否全部必填，是否允许按 capability 条件必填；统一
   `schema_version` 位置与兼容策略。
2. **App Map owner**：富知识与 core v1 投影谁是权威，是否由生成器保证同源，浏览器是否必须提供
   projection。
3. **Selector model**：UIA 候选链与 browser shell target 是否共用 AST；纯 CDP Adapter 的 shell
   target 是必填还是 capability 条件必填。
4. **Tool declaration**：`postconditions`、`none_readonly`、approval/risk 等 adapter-level 字段的
   owner 与运行时消费方式，禁止继续依赖 binary 私有归一化而不落契约。
5. **Rollback contract**：L1/L2/L3 各自是否必须声明 recipe；不可逆工具如何显式声明“无恢复路径”；
   browser 的 navigation/form fill/download 如何表达补偿。
6. **Interrupt grammar**：匹配字段、允许动作、`requires_human`、`delegate_to_*`、
   `dismiss_explicitly` 的闭集语义，以及所有 Adapter 的统一默认。

## 7. 推荐拆卡顺序

1. **ADR/spec 设计卡**：只冻结六件套 envelope、owner 与闭集语义，不改产品代码。
2. **通用只读 validator 卡**：按新 spec 校验四套现有包，先暴露缺口，不改包内容。
3. **现有包收敛卡**：分成 Notepad/Paint 与 Edge/Chrome 两张，避免单个卡跨多个 Adapter owner。
4. **Excel Adapter pack 卡**：依赖前三步结果，避免把当前不统一形状复制到第四个真实应用。
5. **COM lifecycle 卡**：与 Adapter schema 解耦推进，但仍按 Stage 2 计划完成成功、异常、取消回收。

第 1 步修改公共契约，按 AGENTS 漂移规则需要单独裁决；本 automation 不越过该闸门。

## 8. 本次验证

```text
python adapters/com.microsoft.paint/tests/validate_adapter_pack.py
PASSED: app_id=com.microsoft.paint targets=78 tools=15 recipes=8 probe_status=required

python adapters/browser.edge/tests/validate_adapter_packs.py
PACK OK browser.edge
PACK OK browser.chrome

rg "adapter.toml" crates apps xtask -g '*.rs'
NO_RUST_ADAPTER_MANIFEST_CONSUMER

rg "app_map.json" crates apps xtask -g '*.rs'
NO_RUST_RICH_APP_MAP_CONSUMER

rg "rollback/recipes.json" crates apps xtask -g '*.rs'
NO_RUST_ROLLBACK_PACK_CONSUMER

rg "interrupts/interrupts.json" crates apps xtask -g '*.rs'
NO_RUST_INTERRUPT_PACK_CONSUMER
```

## 9. 判断

Stage 2 的下一真实阻塞不是 Excel 文件格式，而是 Adapter bundle 目前没有单一事实源：
四套包能看到六类文件、局部 validator 和各自的 runtime 切片，但不能证明它们受同一 schema 与同一
消费路径约束。

因此建议先完成 ADR/spec 设计，再实现通用 validator。当前包继续按既有 fail-closed 行为运行，
不得因为本审计而放宽任何现有断言。
