# apps/paint.md - Microsoft Paint 应用档案（L2）

> ADR-0021 规定的 8 个固定小节。未测项必须显式写「未测」，不得把推断当事实。
> TASK-043 的 selector、坐标容差与像素容差在真机校准前都是 provisional。

## 1. 身份与版本

| 项 | 实测值 | 取法 |
|---|---|---|
| 包名 | `Microsoft.Paint` | 本机 AppX 注册表 / 包族名 |
| 版本 | **11.2605.81.0** | `reg query HKLM\Software\Microsoft\Windows\CurrentVersion\Appx\AppxAllUserStore\Applications /f Microsoft.Paint /k` |
| AUMID | `Microsoft.Paint_8wekyb3d8bbwe!App` | 包族名 + `!App` |
| UI 框架 | WinUI 3（feasibility P3） | `target-apps-feasibility.md` §3 P3 |
| 是否打包应用 | 是（MSIX） | 包族名与 AppX 注册表 |

## 2. 进程与窗口模型

**2026-10-07 初次实测**：进程为 `mspaint.exe`，窗口类为 `MSPaintApp`，AUMID 为 `Microsoft.Paint_8wekyb3d8bbwe!App`；真实只读 UIA 探针已导出窗口树，详见 §9。启动 PID 仍不得当窗口属主 PID，窗口属主必须走 Win32 查询。

已知约束（来自 feasibility P3 与通用 Windows 规则）：

- 启动必须走 AUMID，不得把启动返回 PID 当成窗口属主 PID。
- 必须用 `EnumWindows` + `GetWindowThreadProcessId` 解析窗口属主。
- 画布坐标与屏幕坐标不是同一个空间，DPI / 多屏 / 最小化窗口都必须显式处理。

## 3. UIA 形状

**2026-10-07 初次实测**：真实 Paint 树已由一次性 Rust UIA 探针导出；工具栏、形状、颜色、缩放与画布形状见 §9。PowerShell UIA 仍不作为本卡路径。

已声明的 provisional 形状：

- 工具栏、工具按钮、颜色按钮、图层面板、状态栏预计可由 UIA 覆盖。
- 画布是单一自绘表面，预计只能读取 bounds，不能读取像素内容或子控件动作。
- selector 候选链见 `adapters/com.microsoft.paint/selectors/targets.json`。
- selector 目标 15 / 21 已升 `observed`（2026-10-18 TASK-309：窗口 / 工具按钮 / 形状库 / 尺寸四档 / 颜色 / 图层 / 状态栏 / 画布），其余仍 `required`。

## 4. 画布坐标与像素约定

Paint Adapter 明确区分：

- **canvas space**：文档左上角原点，单位是画布像素。
- **screen space**：物理屏幕像素，必须带 monitor id 与 DPI scale。

坐标换算输入必须包含：

```text
canvas_bounds_px
zoom_ratio
viewport_offset_x_px
viewport_offset_y_px
monitor_id
dpi_scale
```

任何 zoom、scroll、resize、DPI 或显示器变化后，旧 point 立即失效。

像素验证不得使用逐像素相等。必须使用 `pixels` 容差或 pHash / dHash 的
`hamming_within`，并且给出 `confidence_min`。

## 5. 性能实测

**未测**。Paint 全窗口树遍历、工具按钮定位、画布 bounds 读取、一次 drag
耗时、截图耗时与像素快照大小都没有本轮真机数据。

## 6. 已知坑

1. 画布坐标不等于屏幕坐标；缩放和滚动偏移会让旧坐标系统性点偏。
2. 画布是自绘表面，不能把 UIA 控件树当成画布内容读回路径。
3. 当前工具、颜色、图层都是隐式状态；画之前必须回读。
4. 一次拖拽可能被应用合并成一个 undo，也可能拆成多个；像素快照必须作证。
5. 图层选择决定绘制落在哪一层；虚拟化列表只实例化可见项。
6. AI 功能可能联网且耗时不可预测；Adapter 将其列为不支持并转 `NeedsHuman`。
7. Save As 是跨进程 Shell 对话框，禁止静默覆盖已有文件。
8. 2026-10-07 的 PowerShell / UIA 探针曾挂起；2026-10-16 的 ControlView 探针已成功，但 `observed` 只表示 UIA 形状实测，selector 成功率仍未验证。

## 7. 通道结论

| 层 | 可用性 | 说明 |
|---|---|---|
| L1 API | 否 | Paint 没有 COM / CLI / 插件自动化接口。 |
| L2 命令 | 部分 | `Ctrl+N/O/S/Z/Y` 等；工具快捷键不稳定，只能辅助。 |
| L3 UIA | 部分 | 工具栏、颜色、图层、状态栏可行；画布内容不可读。 |
| L4 合成输入 | 必需 | 画布绘制只能走 pointer / drag。 |
| L5 视觉验证 | 必需 | 画布内容只能通过截图、像素容差或感知哈希验证。 |

撤销声明：Paint 支持 `Ctrl+Z`（L0），但 Adapter 同时要求 L1 像素快照。
工具、颜色、图层状态不属于文档内容，用 L2 补偿动作回滚。

## 8. 未测项

- 完整 AutomationId 普查；形状项与调色板项仍无稳定 AutomationId（尺寸四档 `Size1`–`Size8` 除外）。
- 画布 bounds、viewport origin、zoom 与 scroll offset 的精确读取方式。
- 单点 / drag 命中误差，含混合 DPI 多屏。
- 工具按钮、颜色按钮、图层行的真实 selector 成功率。
- `Ctrl+Z` 的 undo 粒度、合并行为与深度。
- 像素快照大小、截图耗时、树遍历耗时。
- 保存 / 另存为 / 未保存对话框与覆盖确认的真实按钮身份。
- TASK-044 的真实 Paint 校准结果。

## 9-12. 早期探针与运行时装配（已归档）

→ 见 `docs/memory/archive/paint-early-probes.md`。现行 §13.8 起的真机工具集探针实测保留在本文档。

## 13. 画笔 / 形状 / 颜色 工具集清单（人类截图，2026-10-16）

> 来源：人类提供的本机 Paint 界面截图，用于「完整画笔 / 形状 / 颜色工具集」的对照。
> 名称按截图中文字**逐条抄录**：不改名、不翻译、不补全。截图未覆盖的部分一律标「未覆盖」。
> 本节只记**有哪些项、叫什么**；每项的 UIA 形状（name / aid / control type / pattern）
> 仍须真机探针取证，未取证前不得当事实用。

### 13.1–13.7 工具集静态清单（已归档）

→ 见 `docs/memory/archive/paint-early-probes.md`。以下从 §13.8 起保留 TASK-309 的真机 UIA 探针实测结果。

### 13.8 真机 UIA 探针实测（2026-10-16，TASK-309 ControlView）

探针：`eval/tasks/paint/probe-toolset.ps1`（**只读**，ControlView BFS，封顶 900 元素 / 25 s）。
一次完整 dump：`seen=91 emitted=91 truncated=False elapsed_ms=177`；
窗口 `name="无标题 - 画图" class=MSPaintApp proc=mspaint`。

**工具栏实测（name / AutomationId / UIA ControlType）**：

| name | AutomationId | ControlType |
|---|---|---|
| 选择 | — | RadioButton |
| 裁剪 | `CropButton` | Button |
| 删除背景 | — | Button |
| 旋转 | `RotateDropdown` | Button |
| 翻转 | `Flip` | Button |
| 重设大小和倾斜 | — | Button |
| 铅笔 | `PencilTool` | Button |
| 填充 | — | Button |
| 文本 | — | Button |
| 橡皮擦 | `EraserTool` | Button |
| 颜色选取器 | — | Button |
| 放大镜 | — | Button |
| 画笔 | `BrushesSplitButton` | **RadioButton** |
| 形状 | `CollapsedButton` | Button |
| `颜色 1: 黑色` | — | RadioButton |
| `颜色 2: 白色` | — | RadioButton |
| 编辑颜色 | — | Button |
| Copilot | `CopilotDropDownButton` | Button |
| 大小 | — | Slider |
| 不透明度 | — | Slider |
| 工具栏选项 | `OptionsButton` | Button |

**修正 §11.1**：那张表里写的 `ToggleButton`（铅笔 / 填充 / 文本）与 `SplitButton`（画笔）是
**XAML 类名**；UIA `ControlType` 实测分别是 `Button` 与 `RadioButton`。按 ControlType 找控件会漏命中，
必须用 `AutomationId`。

**`颜色 1` / `颜色 2` 可直接读回**：两个 RadioButton 的 name 形如 `颜色 1: 黑色` / `颜色 2: 白色`，
当前颜色就编码在 name 里 —— 这是现成的读回路径，不必截图取色。

**固定调色板实测 UIA name**（`List` 下 20 个 `ListItem`，aid 全空、全部 `focusable=true`）：

| 行 | UIA name |
|---|---|
| 1 | 黑色 / 灰色 / 深红色 / 红色 / 橙色 / 黄色 / 绿色 / 青绿 / 靛蓝色 / 紫色 |
| 2 | 白色 / 浅灰 / 褐色 / 玫瑰红 / 金色 / 浅黄色 / 酸橙色 / 淡青绿色 / 蓝灰色 / 淡紫色 |

**修正 §13.7**：该节两行色名里第 1 行的 `青蓝` / `蓝` 实测为 `青绿` / `靛蓝色`；第 2 行我目测的
`棕 / 粉 / 橙黄 / 米 / 黄绿 / 浅青 / 蓝灰 / 淡紫` 实测为
`褐色 / 玫瑰红 / 金色 / 浅黄色 / 酸橙色 / 淡青绿色 / 蓝灰色 / 淡紫色` —— **以本节为准**。

**仍需探针**：形状画廊 23 项、画笔 flyout 9 项、轮廓 / 填充 flyout 各 7 项。这些元素只在对应
flyout 展开时才进树，本轮只读探测（不点击）拿不到。

### 13.9 形状库 + 三个 flyout 按钮实测（2026-10-16，TASK-309，UIA pattern 展开）

**展开方式（重要）**：`形状`（aid `CollapsedButton`）支持 **`ExpandCollapsePattern`** —— 直接
`Expand()` 就能打开形状库，**不需要键鼠、不需要合成点击**；`Collapse()` 收回。
`形状轮廓` / `形状填充` / `大小` 三个按钮同样支持 `Invoke + ExpandCollapse`。

展开后新增的三个按钮：

| name | aid | ControlType | patterns |
|---|---|---|---|
| 形状轮廓 | — | Button | Invoke + ExpandCollapse |
| 形状填充 | — | Button | Invoke + ExpandCollapse |
| 大小 | — | Button | Invoke + ExpandCollapse |

**形状 23 项**（每项是**两层**：外层 `ListItem` + 内层 `Button`）：

| 行 | 顺序（左 → 右） |
|---|---|
| 1 | 直线 / 曲线 / 椭圆 / 矩形 / 圆角矩形 / 多边形 / 三角形 / 直角三角形 |
| 2 | 菱形 / 五边形 / 六边形 / 向右键 / 向左键 / 向上箭头 / 向下箭头 / 四角星 |
| 3 | 五角星 / 六角星 / 圆角矩形标注 / 椭圆形标注 / 云形标注 / 心形 / 闪电 |

与人类给的标准中文名称**逐项一致**（含 `向右键` / `向左键` 这种不对称命名）。

**修正 §11.2（结论性）**：那里写「形状项全部 `IsKeyboardFocusable=false`」—— **只对 ListItem 外层成立**。
内层 Button 实测 `focusable=true` 且 `patterns=[Toggle,ScrollItem]`。因此 §12 的
「形状项没有任何可用 UIA pattern，只能走 L4 合成点击」**不成立**：23 项都能用 `TogglePattern`
程序化选中，合成点击不再是必需通道。

**选择器**：23 项 aid **全空**，只有本地化 name —— 必须用 `ExactNameAndRole`（ADR-0091）+
容器（形状库 List）消歧；`矩形` 同时也是工具栏外的唯一同名点，注意别与其它同名项混。

### 13.10 画笔 flyout（`BrushesSplitButton` 展开，UIA pattern）

`ExpandCollapsePattern.Expand()` 展开后新增一个 **`Menu`**（rect 1407,558,486,840），
其下 **9 个 `MenuItem`**：aid 全空、`focusable=true`、`patterns=[Invoke,Toggle]`，name 就是笔型名。

| # | name | ControlType | patterns |
|---|---|---|---|
| 1 | 画笔 | MenuItem | Invoke + Toggle |
| 2 | 毛笔 | MenuItem | Invoke + Toggle |
| 3 | 书写笔 | MenuItem | Invoke + Toggle |
| 4 | 喷枪 | MenuItem | Invoke + Toggle |
| 5 | 油画笔 | MenuItem | Invoke + Toggle |
| 6 | 蜡笔 | MenuItem | Invoke + Toggle |
| 7 | 记号笔 | MenuItem | Invoke + Toggle |
| 8 | 普通铅笔 | MenuItem | Invoke + Toggle |
| 9 | 水彩画笔 | MenuItem | Invoke + Toggle |

与 §13.1 的截图清单**逐项一致**（顺序也一致）。每项另有一个 `Text`（aid `TextBlock`）承载显示文字，
**选择要打 MenuItem，不要打 Text**。

### 13.11 形状轮廓 flyout（展开形状库后再展开「形状轮廓」按钮）

两级 pattern 展开：先 `CollapsedButton` 展开形状库，再按 name `形状轮廓` 展开其 flyout。
展开后新增 **7 个 `MenuItem`**：aid 全空、`focusable=true`、`patterns=[Invoke,Toggle]`。

`无边框` / `实心边框` / `蜡笔` / `记号笔` / `油画笔` / `普通铅笔` / `水彩画笔`

与 §13.3 的截图清单**逐项一致**，且**证实人类说法**：最上面一项确实是 `无边框`。

### 13.12 形状填充 flyout（同两级展开）

**7 个 `MenuItem`**：aid 全空、`focusable=true`、`patterns=[Invoke,Toggle]`。

`无填充` / `纯色填充` / `蜡笔` / `记号笔` / `油画笔` / `普通铅笔` / `水彩画笔`

与 §13.4 逐项一致，并**证实人类说法**：最上面一项是 `无填充`。

### 13.13 形状工具的三个按钮：**必须先选中形状**（2026-10-16 人类说明 + 实测证实）

「形状轮廓」/「形状填充」/「大小」三个按钮**在没选中形状时是 disabled**。实测差异很干净：

| 状态 | 三个按钮的 `focusable` | 展开 `大小` 后 |
|---|---|---|
| 未选中形状 | **`false`** | 展开成功但**条目为空**（见 13.15） |
| 选中形状后 | **`true`** | 正常列出尺寸（见 13.14） |

→ **自动化必须先选中一个形状再碰这三个按钮**；`focusable=false` 可直接当「不可用」的判据，
不必盲点。这也解释了 13.15 那次为什么探到空的：那次是「没选形状」的合法状态，不是通道缺陷。

### 13.14 `大小` flyout（选中形状后）：条目带稳定 aid

选中形状后再两级展开（`CollapsedButton` → name `大小`），拿到：

| name | AutomationId | ControlType | patterns |
|---|---|---|---|
| `1 像素` | **`Size1`** | MenuItem | Invoke + Toggle |
| `3 像素` | **`Size3`** | MenuItem | Invoke + Toggle |
| `5 像素` | **`Size5`** | MenuItem | Invoke + Toggle |
| `8 像素` | **`Size8`** | MenuItem | Invoke + Toggle |

`Size3` / `Size5` / `Size8` 是**本项目在 Paint 里看到的第一批稳定 aid 枚举项**（形状项 aid 全空）。

**四档的 aid 是齐的**：`Size1` / `Size3` / `Size5` / `Size8`，全部 `focusable=true`、
`patterns=[Invoke,Toggle]`。**但它们不能靠 name 选** —— 见 §13.16。

### 13.16 ⚠️ `Size1` 的 name 是「大小」，不是「1 像素」（2026-10-16 实测）

`大小` flyout 的 4 个 MenuItem 里，**第一项的 name 是 `大小`**（沿用了分组标题），
不是 `1 像素`；每项下面另有一个 `Text`（aid `TextBlock`）才承载显示文字。

| name | aid | ControlType |
|---|---|---|
| **`大小`** ← 陷阱 | `Size1` | MenuItem |
| `3 像素` | `Size3` | MenuItem |
| `5 像素` | `Size5` | MenuItem |
| `8 像素` | `Size8` | MenuItem |

**两个后果**：

1. **按 name 找 `1 像素` 会失败**（`name="1 像素"` 只命中它的 `Text` 子节点，而 Text 不可操作）；
   必须用 **`aid=Size1`**。
2. `name="大小"` 同时命中**尺寸按钮本身**、**画笔的大小 Slider** 和这个 MenuItem —— 三重歧义。
   尺寸选择**一律走 aid**，不要走 name。

> 本条推翻了 §13.14 早前「`1 像素` 没有 MenuItem」的记法：当时是按「像素」筛 name 才漏掉的，
> 与「当前生效值另行渲染」无关。人类把值改到非第一位后复测，结论不变 —— 与选中状态无关。

### 13.15 `大小` flyout：未选中形状时条目为空（原因见 13.13；结论已被 13.14 / 13.16 修正）

同两级展开（`CollapsedButton` → name `大小`）。`ExpandCollapsePattern.Expand()` 成功
（`state=Expanded`），但**展开后 ControlView 里没有任何「N 像素」条目**：

- `type=MenuItem` 全树只有 3 项（`文件` / `编辑` / `查看`），不含尺寸；
- 完整无过滤遍历 `seen=174 emitted=174 truncated=False`，唯一匹配 `像素` 的是画布尺寸文本
  `418 × 74像素`（aid `CanvasSizeTextBlock`）；
- 坐标带 `x∈1780~1999` 只捞到 `大小` 按钮本身。

→ **§13.5 的 4 档（1 / 3 / 5 / 8 像素）目前没有 L3 通道**。[superseded:2026-10-18 TASK-309 —— §13.14 / §13.16 已证明选中形状后四档带稳定 aid `Size1`–`Size8`]
该行推断（「条目只存在于合成层」）已作废：条目缺失是「未选中形状」的合法状态，见 §13.13 / §13.14。
**未取证的尺寸值不得猜测放行**；已取证的 `Size1`–`Size8` 一律走 aid，不走 name（§13.16）。


### 13.17 工具组 / 画笔组是**两条正交轴**（2026-10-18 人类说明 + 工具栏截图）

**纠正一条我实际用错过的模型**：把「活动工具切到橡皮擦」理解成「画笔组被禁用 / 画笔被取消选择」是**错的**。Paint 的模型是：

- **工具组**（截图标签「工具」）：铅笔 / 填充 / 文本 / 橡皮擦 / 颜色选取器 / 放大镜 —— 选的是**当前工具**。
- **画笔组**（截图标签「画笔」）：`BrushesSplitButton` + 尺寸 / 不透明度 —— 选的是**笔型这一参数**。
- 两轴**正交**：`铅笔` 用画笔组里的某个笔做**加法**，`橡皮擦` 用同一支笔做**减法**。切到橡皮擦**不会**让画笔组失效，只是把「谁在用这支笔」从铅笔换成橡皮擦。

**对 selector / handler 的直接影响**：

1. `paint.tool.select` 的 schema enum 把 `brush` 与 `pencil` / `eraser` 并列。按上面这条，`brush` **不在同一根轴上**，它更像「笔型参数」而不是「工具」。这处语义**需要重新对齐**（可能要拆成 tool 与 brush-style 两个字段），属于改 schema，**不能靠猜**。
2. 2026-10-18 探针看到 `BrushesSplitButton.IsSelected` 随工具切换在 True/False 间变 —— 那是**工具轴**移动的反映，**不能**据此推断「画笔组被禁用」。
3. `PencilTool` / `EraserTool` 的 `Toggle()` 实测生效，且二者是**加减配对**：handler 切到橡皮擦时**不应**去动画笔组。

**工具栏截图可见分组**：选择 / 图像（裁剪 / 翻转 / 旋转 / 重设大小和倾斜）/ 工具（铅笔 / 填充 / 文本 / 橡皮擦【截图当时选中】/ 颜色选取器 / 放大镜）/ 画笔（`BrushesSplitButton`）/ 形状。

### 13.18 工具栏是**树**，不是一张扁平清单（2026-10-18 人类说明；纠正 §13.17 那节的扁平读法）

**总分类（8 个）**：选择 / 图像 / 工具 / 画笔 / 形状 / 颜色 / Copilot / 层。
**子分类**挂在各自总分类下** —— 例如「工具」下：铅笔 / 填充 / 文本 / 橡皮擦 / 颜色选取器 / 放大镜。

这与 §13.8 的实测**完全吻合**：ControlView 里这 8 个总分类就是 `Group`，其下按钮是它的子节点。

```text
[6] name="选择"    type=Group
[6] name="图像"    type=Group
[6] name="工具"    type=Group   <- 子：铅笔(PencilTool) / 填充 / 文本 / 橡皮擦(EraserTool) / 颜色选取器 / 放大镜
[6] name="画笔"    type=Group   <- 子：BrushesSplitButton（笔型）+ 尺寸 / 不透明度
[6] name="形状"    type=Group   <- 子：CollapsedButton（画廊）+ 形状轮廓 / 形状填充 / 大小
[6] name="颜色"    type=Group   <- 子：颜色 1 / 颜色 2 / 20 色 List / 编辑颜色
[6] name="Copilot" type=Group
[6] name="层"      type=Group
```

**对 schema 的影响（关键）**：`paint.tool.select` 现在的扁平 enum

```text
["pencil","brush","eraser","fill","rectangle","ellipse","line","color_picker"]
```

把**不同总分类下的子项**混成了一份全局清单：`pencil` / `eraser` / `fill` / `color_picker` 属「工具」，`rectangle` / `ellipse` / `line` 属「形状」，而 `brush` 属「画笔」（且是笔型参数，见 §13.17）。清单还缺了「文本」「放大镜」等真实子项。

**正确形态应是「路径」而不是「名字」**：按总分类限定子项，例如 `工具/铅笔`、`形状/矩形`、`画笔/毛笔`。好处：① 同名不再有歧义（§13.6 记过 `蜡笔` / `记号笔` / `油画笔` / `普通铅笔` / `水彩画笔` 在三个 flyout 里都出现）；② 新增子项不必改全局 enum；③ 每个总分类可以有自己的切换与读回通道（工具 = Toggle、画笔 = flyout MenuItem、形状 = L4 点击……）。

**待裁决**（属改公共 schema，按铁律 10 先 ADR）：把 `paint.tool.select` 从扁平 enum 改成**总分类 + 子项**的寻址模型。

### 13.19 「形状轮廓 / 形状填充」未选中形状时也能展开并枚举（2026-10-19，TASK-318 只读复测）

§13.13 记「三个按钮在未选中形状时 disabled」，并据此推广成「必须先选中形状才能用这两个
flyout」。**2026-10-19 的只读 ControlView 复测修正了这条推广**：`形状轮廓` / `形状填充`
两个按钮本身 `focusable=True`，展开后各自列出**全部 7 项**（`MenuItem`、
`focusable=True`、patterns=`[Invoke,Toggle,ScrollItem,VirtualizedItem]`）：

```text
PROBE: expanded aid="CollapsedButton" state=Expanded
PROBE: expanded name="形状轮廓" state=Expanded
[5] name="无边框"   aid="" type=MenuItem focusable=True
[5] name="实心边框" aid="" type=MenuItem focusable=True
[5] name="蜡笔" / "记号笔" / "油画笔" / "普通铅笔" / "水彩画笔"

PROBE: expanded name="形状填充" state=Expanded
[5] name="无填充"   aid="" type=MenuItem focusable=True
[5] name="纯色填充" aid="" type=MenuItem focusable=True
[5] name="蜡笔" / "记号笔" / "油画笔" / "普通铅笔" / "水彩画笔"
```

**同刻只有一个 flyout 的条目在树里**：先展开轮廓再展开填充后，MenuItem 全表只剩填充的
7 项（加「文件 / 编辑 / 查看」）—— 打开填充会自动收起轮廓。因此按 `exact_name_and_role`
解析这两组条目**不存在跨 flyout 歧义**，尽管后 5 项名与画笔组同名（§13.10）。

### 13.20 `大小` flyout：未选中形状时展开后条目为空（2026-10-19 复测）

同一次复测里，`大小` 按钮 `focusable=True` 且展开成功，但**展开后 ControlView 里没有任何
`MenuItem` 条目**（与 §13.15 一致）。所以 `大小` 与轮廓 / 填充**不是**同一情况：

- 轮廓 / 填充：只靠 `ExpandCollapsePattern.Expand()` 就能枚举，已有可落地契约 → ADR-0097。
- 大小：条目只在**选中形状后**出现，而选中形状需要真实 GUI 交互 → 继续留在 schema 之外，
  由 TASK-309 的真机段处理。
