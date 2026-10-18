# ADR-0098　Paint 形状笔宽（大小）按组拆工具（ADR-0096 / ADR-0097 的增量落地）

状态：**Accepted**（2026-10-19，预授权自动化裁决；TASK-319）
日期：2026-10-19
Supersedes：—
Superseded by：—
关联：**TASK-309**、**TASK-318**、ADR-0096、ADR-0097、ADR-0095、ADR-0091、
`docs/memory/apps/paint.md` §13.13 / §13.14 / §13.16 / §13.18 / §13.20 / §13.21

---

## 背景

ADR-0096 把 Paint 工具栏寻址冻结为「一个总分类一个工具 + 组内有界 `path`」；
ADR-0097 又落了「形状轮廓 / 形状填充」两组，并把 **`大小`（形状笔宽）明确排除在 schema 之外**，
理由是：`大小` 按钮没有非本地化锚点、且 ADR-0097 当时观察到 flyout 展开后为空 ——
原文写的是「**直到有非本地化锚点或真机端到端证据**」。

**2026-10-19 的只读 ControlView 复测（本 run 第一手）把这条前提拆掉了一半**：在形状组已有
活动形状（`向下箭头`）时，两级展开后 `大小` flyout **枚举出全部 4 项，且四项都带稳定
`AutomationId`**：

```text
PROBE: window name="无标题 - 画图" class="MSPaintApp" pid=20128 proc=mspaint
PROBE: expanded aid="CollapsedButton" state=Expanded
PROBE: expanded name="大小" state=Expanded
[5] name="大小"   aid="Size1" type=MenuItem focusable=True patterns=[Invoke,Toggle,ScrollItem,VirtualizedItem]
[5] name="3 像素" aid="Size3" type=MenuItem focusable=True patterns=[Invoke,Toggle,ScrollItem,VirtualizedItem]
[5] name="5 像素" aid="Size5" type=MenuItem focusable=True patterns=[Invoke,Toggle,ScrollItem,VirtualizedItem]
[5] name="8 像素" aid="Size8" type=MenuItem focusable=True patterns=[Invoke,Toggle,ScrollItem,VirtualizedItem]
PROBE: seen=180 emitted=18 truncated=False elapsed_ms=559
```

同一 run 的另一次只读展开给出 `大小` 按钮本身（无 aid、只有本地化 name）：

```text
[6] name="大小" aid="" type=Button focusable=True patterns=[Invoke,ExpandCollapse,ScrollItem]
```

结论：

1. **条目锚点是非本地化的**（`Size1`–`Size8`），比轮廓 / 填充两组还硬 —— ADR-0097 排除它的
   第一条理由不再成立；
2. **`大小` 按钮与 `Size1` 条目、画笔 `大小` Slider 三者同名**（§13.16 记录的三重歧义），
   所以按钮必须按 `name + role=Button` 解析，条目必须按 `AutomationId` 解析，**都不许按 name**；
3. ADR-0097 / §13.20 记的「flyout 为空」是**形状未激活**那一态的合法表现，不是通道缺陷；
   条目存在性取决于形状组是否已有活动形状。本 ADR **不**改变这条前置条件，只把它写成
   调用方义务（见 D4）。

## 决策（一句话）

把 `大小`（形状笔宽）拆成一个语义工具，沿用 ADR-0096 / ADR-0097 的**组内词汇表 + 有界
`path` + 通道按组声明**模型；与轮廓 / 填充的唯一差别是：**条目按 `AutomationId` 解析**，
词汇表用像素宽度而不是本地化显示名。

### D1　对外语义面

| 工具 | 总分类 | 状态 |
|---|---|---|
| `paint.size.select` | 形状 → 大小（形状笔宽） | 新（本 ADR） |
| `paint.outline.select` / `paint.fill.select` | 形状轮廓 / 形状填充 | 已有（ADR-0097 / TASK-318） |
| `paint.tool.select` / `paint.shape.select` / `paint.brush.select` | 工具 / 形状 / 画笔 | 已有（ADR-0096 / TASK-317） |
| 画笔组的 `尺寸` / `不透明度` Slider | 画笔 | **不进 schema**（本次探测的那一态里两个 Slider 不在树上，无证据） |
| 选择 / 图像 / Copilot / 层 | — | 不进 schema（与 ADR-0096 / ADR-0097 一致） |

命名理由：`docs/spec/naming.md` 的 Tool 名口径是 `<app>.<domain>.<action>`（三段），
所以不能写 `paint.shape.size.select`；domain 取 `size`，与 `outline` / `fill` 同形。
语义上它**只**指形状笔宽，**不**指画笔尺寸（后者本次不立项）。

### D2　组内寻址与错误语义

与 ADR-0096 D2 / ADR-0097 D2 完全一致：`path: string[]`、长度 `1..=1`、逐元素命中本组词汇表；
未知元素 / 空数组 / 超深 → **既有** `ToolInvalidArgs` fail-closed，不新增 ErrorCode。

### D3　词汇表与锚点（来自 ControlView 实测）

词汇表 = **像素宽度**，不是显示名：

```text
"1 像素" -> Size1     "3 像素" -> Size3     "5 像素" -> Size5     "8 像素" -> Size8
```

**为什么不复用显示名当词汇表**（与轮廓 / 填充两组不同）：`Size1` 那一项的 UIA name 是
`大小`（沿用了分组标题），不是 `1 像素`；而 `大小` 又被 `大小` 按钮与画笔 `大小` Slider 共用。
把显示名当 identity 会让「选 1 像素」在词汇表层就产生歧义 —— §13.16 已记录过这个坑。
像素宽度是这一组**唯一稳定且无歧义**的 identity，锚点则是稳定 `AutomationId`。

### D4　通道与前置条件

通道：两级 `ExpandCollapse`，与轮廓 / 填充同形 ——

1. 先展开形状库（`CollapsedButton`，`shapes_gallery_toggle`）；
2. 再按 `exact_name_and_role`（`大小` + `Button`）展开 `大小` 按钮（`shape_size_button`）；
3. 在有界窗口内按 `AutomationId` 重解目标条目，然后对 `MenuItem` 发 `Invoke`。

**前置条件（调用方义务）**：形状组必须已有活动形状，否则 `大小` flyout 展开后为空，
本工具以**既有 `TargetNotFound`** fail-closed。本工具**不得**自己先去点一个形状 ——
那会在调用方没有要求的时候改动画布工具状态（静默副作用），属铁律 1 / 9 的禁区。
需要「先选形状」的调用方显式先调 `paint.shape.select`。

### D5　读回

沿用 ADR-0096 D5 / ADR-0097 D5：不得用会粘住的 `ToggleState` 冒充当前值，输出显式
`ui_state_readback=false`；只回本地记录的 `selected_leaf` / `previous_leaf`。

### D6　失败与边界

- 展开后 1 s 内仍无目标项 → 既有 `TargetNotFound`（经 `UnknownTool` 映射），不降级、不猜 name。
- 形状未激活导致 flyout 为空 → 同一路径 fail-closed，**不**退化成合成点击。
- 本次**不做**真机端到端（选中形状后落笔并验证像素）；那属 TASK-309 真机段，需要人类在场。

## 影响

- 改**公共 Tool schema**（新增一个 mounted tool + 一条词汇表）→ 属铁律 10，先有本 ADR。
- 适配包 `tools.json` / `selectors/targets.json` / 校验器 / handler / registry / mounted 集合同步。
- `docs/memory/apps/paint.md` §13.20 的「条目只在选中形状后出现，而选中形状需要真实 GUI 交互」
  是**过度推广**：选中形状是 `paint.shape.select` 已有的程序化路径；本卡按实测改写该节并新增 §13.21。
- ADR-0097 的「`大小` 不进 schema」只在其自身写明的条件下成立；本 ADR 引用该条件落地，
  **不**改写 ADR-0097 正文（ADR 只增不改），二者关系记录在本节与 LEDGER。

## 备选方案（未采纳）

1. **把 `大小` 并进 `paint.shape.select` 的多级 `path`**：会让「选形状」与「设形状笔宽」两种
   语义共用一个词汇表，且 `大小` 是独立 flyout（互斥展开）—— 与 ADR-0097 否决过的方向同病。
2. **词汇表沿用显示名（`大小` / `3 像素` / `5 像素` / `8 像素`）**：`大小` 三重歧义直接进
   schema，模型无法从 schema 读出合法输入 —— 否决。
3. **handler 自动先选一个形状再展开**：静默改画布工具状态，违反「无静默失败 / 不扩范围」；否决。
4. **顺手把画笔 `尺寸` / `不透明度` Slider 一起做**：本次探测的那一态里两个 Slider 不在树上，
   无 Min/Max/CurrentValue 证据；放行即违反「未取证不放行」；否决。

## 验证方式

- 契约：非法 `path`（未知元素 / 空数组 / 超长 / 形状组或轮廓组的值）必须 `ToolInvalidArgs`。
- 锚点：`1 像素` → `Size1`、`3 像素` → `Size3`、`5 像素` → `Size5`、`8 像素` → `Size8`；
  校验器同时钉住四个 target 的 `AutomationId` 与工具枚举取值。
- 通道：折叠态下先展开形状库、再展开 `大小`，能在有界窗口内按 aid 解析到 `MenuItem`。
- 读回：输出 `ui_state_readback=false`，不得出现「UI 未变但输出成功」。
