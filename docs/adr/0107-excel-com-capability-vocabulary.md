# ADR-0107：Excel COM/spreadsheet capability vocabulary

状态：**Accepted**（2026-11-23，人类要求继续 TASK-392 的下一步）
日期：2026-11-23
Clarifies：ADR-0042、ADR-0104、ADR-0105
Supersedes：无
Superseded by：无
关联：TASK-375、TASK-391、TASK-392、TASK-393
落点：`docs/spec/capability-matrix.md`、`protocol/capability-matrix/capability-1.0.json`

## 背景

TASK-392 已建立 `com.microsoft.excel` 的声明式 safety shell，但 capability catalog 没有
Excel COM/spreadsheet ID，因此 `tools.json` 与 rollback recipes 必须保持空数组。若继续用
accessibility、keyboard 或笼统的 app capability 冒充 COM，会同时破坏 policy 的风险判断和
Adapter 的机器契约。

本 ADR 只冻结静态 capability 词汇。它不实现 COM object、Excel runner、model-visible Tool、
ToolSchema 字段或真机验收；这些仍由 T2.1-T2.3 独立卡片完成。

## 决策

一句话：**Excel L1 COM 使用七个 experimental `com.*` capability；每个 ID 只表达一个受限
能力族，禁止通用 COM dispatch、别名、用户进程附着与 UIA 冒充。**

### D1：稳定 ID 与版本

新增以下 capability，全部 `stability=experimental`、`version=0.1.0`：

| Capability | 允许语义 |
|---|---|
| `com.workbook_open` | 在专用 owned Excel process 中打开调用方明确声明的工作簿，并记录进程/窗口/工作簿身份与回收责任 |
| `com.workbook_read` | 读取工作簿/worksheet 身份、命名区域、尺寸与计算状态等元数据，不修改单元格 |
| `com.range_read` | 对明确 scope 的 Range 按 `value2`、`formula` 或 `text` 之一读取有界数据 |
| `com.range_write` | 向命名区域或显式 Range 写入有界 scalar literal；仅允许 `value2`，公式形态字符串必须失败；必须提供 L1 值快照与文件影子副本 |
| `com.workbook_save` | 对已打开且身份未漂移的 owned workbook 原位保存；保存前必须完成冲突检测、抖动检测与影子副本 |
| `com.chart_create` | 从已确认的 Range 创建封闭图表类型的 chart object；不得执行宏或任意 COM method |
| `com.chart_export` | 将已确认 chart 导出为 create-new PNG；目标已存在、路径漂移或身份不明必须失败 |

`com.workbook_close` 不进入 catalog。关闭 workbook、Quit 与 COM release 是
`com.workbook_open` 的强制生命周期责任，不构成新的模型可见能力或 Tool。

### D2：风险、审批与可逆性下限

| Capability | Resource | Side effect | Risk floor | Approval floor | Recovery floor |
|---|---|---|---|---|---|
| `com.workbook_open` | invoke | process spawn / file open | L3 | required | 失败与成功路径都必须 bounded close / Quit / COM release / PID reap |
| `com.workbook_read` | read | none | L1 | auto | 不改变 workbook；失败返回明确错误 |
| `com.range_read` | read | none | L1 | auto | 不改变 workbook；读法与 range scope 必须显式 |
| `com.range_write` | write | workbook content | L2 | required | L1 value snapshot + file shadow copy + conflict fingerprint |
| `com.workbook_save` | write | disk | L2 | required | 保存前 shadow copy；冲突或身份漂移失败并要求人工 |
| `com.chart_create` | write | workbook content | L2 | required | L1 file shadow copy + workbook conflict fingerprint |
| `com.chart_export` | write | disk | L2 | required | create-new；失败不得留下半文件，已有目标绝不覆盖 |

这些是**风险下限**。具体 Tool 可以更严格，不得更低。Stage 2 的
`allow_unattended=false` 保持不变；本 ADR 不授权无人值守 Excel 写操作。

### D3：Tool 消费矩阵与风险合成

- `com.workbook_open` 只能出现在**专用的 `effect=write` Tool** 上，并必须同时满足
  `reversibility=l3_irreversible`、`risk_level=high|critical`、
  `requires_approval=true`、`idempotent=false`。在新增 closed-process rollback builtin 之前，
  open 的进程副作用不能伪装成 L1/L2，也不能藏在一个 read Tool 里。
- T2.1 的 read Tool 只可使用 `com.workbook_read`、`com.range_read`，并且只能作用于该 open
  Tool 已返回、身份可复验的 owned workbook；工作簿内容保持只读。
- T2.2（写入/重算/保存）可增加 `com.range_write` 与 `com.workbook_save`，且必须有 L1
  rollback coverage；不得声明 `l0_undo_stack`。
- T2.3（图表/PNG）可增加 `com.chart_create` 与 `com.chart_export`，导出必须 create-new，
  不覆盖既有文件。
- `com.workbook_open` Tool 的 `capabilities` 必须恰好只含该一个 ID；与其他 capability
  混用一律失败。
- read Tool 不得在 envelope 中声明 `com.workbook_open`，也不得在 handler 内隐藏 open；
  它只接收 open Tool 已返回且身份可复验的 workbook target。
- 其他 Tool envelope 的 `capabilities` 必须是所需 ID 的非空、去重、byte-order 升序子集；
  不得把七个全部塞进每个 Tool。模型可见输出不得暴露 COM handle。

### D4：永久拒绝的替代路径

以下名称和语义永久不进入 capability catalog：

- `com.execute`
- `com.eval`
- `com.invoke`
- `com.macro`
- `com.raw_dispatch`
- `com.script`
- `office.*`
- `spreadsheet.*` 作为 `com.*` 的同义 alias

同时禁止：

- 附着到用户既有 `EXCEL.EXE`、按裸 PID 猜测所有权或终止非本轮进程；
- 用 UIA grid traversal、clipboard、keyboard 或 accessibility `element.*` capability
  冒充 COM workbook/range 读写；
- `DisplayAlerts=false`、自动 Save/Discard/Replace/Update/Accept、自动 Enable Editing；
- 宏执行、外部链接更新、公式写入、结构修改（插入/删除 row、column、sheet）或任意 COM
  method/property 字符串；
- 覆盖既有 chart PNG、把 `com.chart_export` 用作任意文件写入能力。

### D5：身份与数据语义

- Workbook identity 首选完整路径；未保存工作簿必须使用本轮创建的 identity，不得按标题猜测。
- Worksheet identity 使用名称或稳定 ordinal；写入前必须复验身份没有因插入/重命名而漂移。
- 命名区域优先于 A1；A1 只能作为调用方显式提供的 fallback。
- `com.range_read` 必须明确使用 `value2`、`formula` 或 `text` 之一；三种读法不得被一个含糊的
  “cell value”替代。
- `com.range_write` 固定使用 `value2`，只接受 JSON scalar literal；文字字符串若首个非空白
  字符是 `=`、`+`、`-` 或 `@`，必须在写入前失败。公式写入需要新 ADR。
- T2.2 写入后必须同时读取目标 `.Value2` 与 `.Formula`，证明值一致且没有意外生成公式。

### D6：catalog 与 validator

- `protocol/capability-matrix/capability-1.0.json` 是七个 ID 的唯一事实源。
- `protocol/adapter-catalog/adapter-catalog-1.0.json` 必须由既有 `xtask codegen` 生成并逐字节校验。
- Excel pack validator 只能断言 catalog 中存在 `com.*`、legacy prefix 不存在和 broad escape
  denylist，不得手写第二份 capability catalog。
- Excel 专属 contract test 可以按本 ADR 锁定精确七项集合、`experimental` 与 `0.1.0`；
  该测试不是运行时 validator 枚举，生产 validator 仍只消费协议 generated catalog。
- catalog 出现 ID 只表示“能力词汇已批准”，不表示 Adapter Tool、COM runner 或真机路径已实现。

## 拒绝条件

- 任一新 ID 不是上述七个、不是 `experimental/0.1.0`，或新增 alias；
- `com.workbook_open` 未单独使用 write/high|critical/approval/non-idempotent/L3，
  或藏在 read Tool、L1/L2 Tool、低风险 Tool 中；
- Tool 使用 capability 但 envelope 为空、重复、未排序或出现 catalog 之外的 ID；
- 读取 Tool 用 UIA/keyboard/accessibility ID 冒充 COM；
- 写入 Tool 声明 Excel undo、L0 或没有 L1 rollback coverage；
- 附着用户 Excel process，或在身份/所有权无法证明时继续；
- chart export 覆盖既有文件，或把 export 用作通用写文件；
- 把本 ADR 合并解释为 T2.1-T2.3 已实现或真机验收通过。

## 后果

- Excel pack 可以开始实现 T2.1-T2.3 的 Tool envelope，但仍不得在 runner 未实现时暴露 Tool。
- Policy/Tool 卡获得明确的 capability 粒度和最低风险，不再需要发明 app-specific alias。
- 代价是 open/read/write/save/chart 拆成多个 ID，Tool 必须声明精确子集；这是为了避免一个
  宽泛 `excel` 能力同时授权读取、写入和导出。

## 验证方式

1. `adr-index`、`verify-schemas` 与 `codegen --check` 全绿。
2. Excel pack validator 读取真实 catalog，证明七个 `com.*` 已存在且无 legacy alias /
   broad escape。
3. App Map 与 projection 明确写明“capability ADR 已解除，runner/Tool 仍待实现”。
4. 所有专属负向测试、workspace 与文档门禁通过；无真实 Excel/COM/GUI 副作用。

## 何时重新讨论

新增/删除 capability、提升到 stable、改变 risk/approval 下限、允许公式写入、结构修改、
宏、Save As、外部链接、用户进程附着或通用 COM dispatch 时，必须新 ADR。
