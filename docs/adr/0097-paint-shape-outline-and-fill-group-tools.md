# ADR-0097　Paint 形状轮廓 / 形状填充按组拆工具（ADR-0096 的增量落地）

状态：**Accepted**（2026-10-19，预授权自动化裁决；TASK-318）
日期：2026-10-19
Supersedes：—
Superseded by：—
关联：**TASK-309**、**TASK-317**、ADR-0096、ADR-0095、ADR-0091、
`docs/memory/apps/paint.md` §13.9 / §13.11 / §13.12 / §13.13 / §13.15 / §13.18

---

## 背景

ADR-0096 把 Paint 工具栏寻址冻结为「一个总分类一个工具 + 组内有界 `path`」，TASK-317
落地了三个组：`paint.tool.select`（工具）/ `paint.shape.select`（形状）/
`paint.brush.select`（画笔）。TASK-309 的剩余面里还有两个同一模型的组没进 schema：

- **形状轮廓**（flyout `形状轮廓`）：`paint.md` §13.11 记 7 项。
- **形状填充**（flyout `形状填充`）：`paint.md` §13.12 记 7 项。

`paint.md` §13.13 曾记「三个按钮在未选中形状时 disabled」，并据此把这两个组判为
「必须先选中形状才能用」。**2026-10-19 的只读 ControlView 复测推翻了这条推断的一部分**：

```text
PROBE: expanded aid="CollapsedButton" state=Expanded
PROBE: expanded name="形状轮廓" state=Expanded
[5] name="无边框"   aid="" type=MenuItem focusable=True patterns=[Invoke,Toggle,ScrollItem,VirtualizedItem]
[5] name="实心边框" aid="" type=MenuItem focusable=True patterns=[Invoke,Toggle,ScrollItem,VirtualizedItem]
[5] name="蜡笔" / "记号笔" / "油画笔" / "普通铅笔" / "水彩画笔"  同上

PROBE: expanded name="形状填充" state=Expanded
[5] name="无填充"   aid="" type=MenuItem focusable=True patterns=[Invoke,Toggle,ScrollItem,VirtualizedItem]
[5] name="纯色填充" aid="" type=MenuItem focusable=True patterns=[Invoke,Toggle,ScrollItem,VirtualizedItem]
[5] name="蜡笔" / "记号笔" / "油画笔" / "普通铅笔" / "水彩画笔"  同上
```

即：**这两个 flyout 在未选中形状时也能展开并枚举出全部 7 项**（`focusable=True`、
`Invoke + Toggle`）。§13.13 的 disabled 结论只对「点击/选择」这一步成立，不适用于
「展开并枚举」；而 `大小` flyout 仍实测为展开后为空（§13.15），`大小` 按钮本身也没有
非本地化锚点 —— 所以**本 ADR 不覆盖 `大小`**，它继续按「未取证不放行」fail-closed。

## 决策（一句话）

把 `形状轮廓` 与 `形状填充` 各拆成一个语义工具，沿用 ADR-0096 的**组内词汇表 + 有界
`path` + 通道按组声明**模型；`大小` 组不在此列，直到有非本地化锚点或真机端到端证据。

### D1　对外语义面

| 工具 | 总分类 | 状态 |
|---|---|---|
| `paint.outline.select` | 形状轮廓 | 新（本 ADR） |
| `paint.fill.select` | 形状填充 | 新（本 ADR） |
| `paint.tool.select` / `paint.shape.select` / `paint.brush.select` | 工具 / 形状 / 画笔 | 已有（ADR-0096 / TASK-317） |
| `大小`（形状笔宽） | 形状 → 大小 | **不进 schema**（无稳定锚点，见背景） |
| 图像 / 选择 / Copilot | — | 不进 schema（与 ADR-0096 一致） |

命名理由：`paint.tool.select` 已占用「填充」这个**工具组**子项（`fill_tool_button`），
所以形状填充只能叫 `paint.fill.select`，两者语义不同、不得合并。

### D2　组内寻址与错误语义

与 ADR-0096 D2 完全一致：`path: string[]`、长度 `1..=1`、逐元素命中本组词汇表；
未知元素 / 空数组 / 超深 → **既有** `ToolInvalidArgs` fail-closed，不新增 ErrorCode。

### D3　词汇表（来自 ControlView 实测）

- `paint.outline.select`：`无边框` / `实心边框` / `蜡笔` / `记号笔` / `油画笔` /
  `普通铅笔` / `水彩画笔`。
- `paint.fill.select`：`无填充` / `纯色填充` / `蜡笔` / `记号笔` / `油画笔` /
  `普通铅笔` / `水彩画笔`。

两条词汇表**必须各自独立**：后 5 项两组同名（§13.6 记录的同名冲突家族）。

### D4　通道

两级 `ExpandCollapse`：先展开形状库（`CollapsedButton`），再按 name 展开
`形状轮廓` / `形状填充` 按钮，然后对 `MenuItem` 发 `Invoke`。该项不在展开前存在于树里，
所以解析必须是「先解一次 → 折叠时展开 → 有界重解」的两段式；实测无法在第一次解析到。

### D5　读回

沿用 ADR-0096 D5：不得用会粘住的 `ToggleState` 冒充当前值，输出显式
`ui_state_readback=false`；只回本地记录的 `selected_leaf` / `previous_leaf`。

### D6　失败与边界

- 展开后 1 s 内仍无目标项 → 既有 `TargetNotFound`（经 `UnknownTool` 映射），不降级、
  不猜 name。
- 本次**不做**真机端到端（选中形状后落笔）；那属 TASK-309 真机段，需要人类在场。

## 影响

- 改**公共 Tool schema**（新增两个 mounted tool + 两条词汇表）→ 属铁律 10，先有本 ADR。
- 适配包 `tools.json` / `selectors/targets.json` / 校验器 / handler / registry 同步。
- `大小` 与真机端到端继续留在 TASK-309 的未闭项；本 ADR 不把 WIP 写成 Done。

## 备选方案（未采纳）

1. **把轮廓 / 填充做进 `paint.shape.select` 的多级 `path`**：会让「选形状」与「设形状边框」
   两种语义混在一个工具的词汇表里，且 §13.6 的同名家族会跨语义串味；否决。
2. **沿用扁平 enum，只加 14 个取值**：重复 ADR-0096 已否决的老路，且每加一项动公共 schema。
3. **把 `大小` 一起做**：`大小` 按钮无非本地化锚点、flyout 未选中形状时为空 —— 无证据可依，
   放行即违反「未取证不放行」；否决。

## 验证方式

- 契约：非法 `path`（未知元素 / 空数组 / 超长）必须 `ToolInvalidArgs`。
- 同名消歧：`蜡笔` 同时存在于轮廓 / 填充 / 画笔三组时，三个工具各自解析到本组元素。
- 通道：折叠态下先展开形状库再展开目标 flyout，能在有界窗口内解析到 `MenuItem`。
- 读回：输出 `ui_state_readback=false`，不得出现「UI 未变但输出成功」。
