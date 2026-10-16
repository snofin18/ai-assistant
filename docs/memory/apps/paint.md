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
- 所有 selector 目标均标记 `probe_status=required`。

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
8. 本轮 PowerShell / UIA 探针挂起，selector id 仍是 provisional，不得当作已验证事实。

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

- 完整 AutomationId 普查；形状和调色板仍无稳定 AutomationId。
- 画布 bounds、viewport origin、zoom 与 scroll offset 的精确读取方式。
- 单点 / drag 命中误差，含混合 DPI 多屏。
- 工具按钮、颜色按钮、图层行的真实 selector 成功率。
- `Ctrl+Z` 的 undo 粒度、合并行为与深度。
- 像素快照大小、截图耗时、树遍历耗时。
- 保存 / 另存为 / 未保存对话框与覆盖确认的真实按钮身份。
- TASK-044 的真实 Paint 校准结果。

## 9. TASK-044 真实 UIA 探针（2026-10-07）

探针方法：不入库的临时 Rust 程序（`target/paint_probe`）通过 AUMID 启动
Paint，使用 `IUIAutomation` + `ControlViewWalker` 只读导出窗口树。

| 目标 | role / class | AutomationId | 观察 |
|---|---|---|---|
| 主窗口 | Window / `MSPaintApp` | 空 | 进程 `mspaint.exe`；标题本地化 |
| 工具栏区域 | Pane / `LandmarkTarget` | 空 | 只能作父作用域兜底 |
| 铅笔工具 | Button / `ToggleButton` | `PencilTool` | 稳定 ID |
| 橡皮擦工具 | Button / `ToggleButton` | `EraserTool` | 稳定 ID |
| 矩形形状 | ListItem / `GridViewItem` | 空 | 可见名 `矩形`；只能 fallback-only |
| 前景色 1 | RadioButton / `RadioButton` | 空 | 可见名本地化；只能 fallback-only |
| 调色板红色 | ListItem / `GridViewItem` | 空 | 可见名本地化；只能 fallback-only |
| 图层开关 | Button / `ToggleButton` | 空 | 需父作用域消歧 |
| 画布 host | Pane / `ScrollViewer` | `scrollViewer` | 稳定 ID |
| 画布 | Group / 空 | `image` | 实测 bounds `(696,525)-(905,562)`；无子动作 |
| 画布尺寸 | Text / `TextBlock` | `CanvasSizeTextBlock` | 实测 `418 x 74` 像素 |
| 缩放组合框 | ComboBox / `ComboBox` | `ZoomValuesComboBox` | 稳定 ID |
| 缩放滑块 | Slider / `Slider` | `ZoomSliderControl` | 稳定 ID |

画布换算观察：画布像素尺寸 `418 x 74`，渲染 bounds `209 x 37`，当前
zoom ratio = `0.5`；画布屏幕原点观察到 `(696,525)`。这只是一次单显示器
物理像素观察，混合 DPI / 多屏仍未验收。

未测项不变：真实拖拽误差、连续 10 次成功率、像素容差最终校准。

### 9.1 TASK-044 selector 解析实测（2026-10-08）

用仓库自身的 `assistant-platform-windows::WindowsPlatform`（临时 harness，不入库）打真实
Paint 11.2605.81.0 窗口，实测**声明式 selector 包当前跑不通**：

| 目标 | 声明候选 | 实测结果 |
|---|---|---|
| `main_window` | `class_and_role class=WinUIDesktopWin32WindowClass role=Window` | `TargetNotFound`（0 命中）；真实 class 是 **`MSPaintApp`** |
| `main_window` | `class_and_role class=MSPaintApp role=Window` | 解析成功 |
| `canvas` | `class_and_role class=Image role=Image` | `TargetAmbiguous`（窗口内 38 命中） |
| `canvas` | `automation_id=image` | 解析成功，`role=Group`、`bounds=(696,525,905,562)`、`size=209x37` |
| `rectangle_tool_button` | `class_and_role class=GridViewItem role=ListItem` | `TargetAmbiguous`（窗口内 43 命中） |

结论：`adapters/com.microsoft.paint/selectors/targets.json` 的 class/role 候选与真实树不符，
`main_window` 在第 1 步就会让 TASK-106 的 7 个 handler 全部失败。**校准 selectors 不在
TASK-044 write scope 内**（`DRIFT-044-2`），需另立拥有 `selectors/**` 写权限的校准卡；
校准前真机十次验收不可跑。

### 9.2 handler read-back 契约与真实控件不匹配（2026-10-08）

真实 ControlView 树还暴露出比 selector 更深的一层：`paint_handlers.rs` 的 read-back 契约
只在 fake 平台成立，真实 Paint 提供不了。逐条实测：

| 工具 | handler 要求 | 真实 Paint |
|---|---|---|
| `paint.tool.select` | 按钮文本 == 英文工具 id（`rectangle`） | 矩形项是 `GridViewItem`、无 aid、Name 本地化为 `矩形` |
| `paint.color.select_foreground` | `set_value(button,"R,G,B")` 后读回 `"R,G,B"` | 前景色是 `RadioButton`、Name 本地化为 `颜色 1: 黑色`、无 set_value |
| `paint.layer.select` | `ListViewItem` + 读回 `Layer 1` | 层面板默认折叠，整棵树没有 `ListView` / `ListViewItem` |
| `paint.document.new` | 解析 `status_bar` 文本里的两个整数 | 没有 `StatusBar` 角色元素；尺寸在 `TextBlock aid=CanvasSizeTextBlock`（`418 × 74像素`） |

**结论**：TASK-106 的 Paint handler 只被 fake 验证过（`production_paint.rs` 的 fake 按 handler
的期望回放）。只校准 selector 不足以让 T3.1 跑起来；工具/颜色/图层的 read-back 语义与
形状/调色板的锚点都需要设计裁决。已开 **TASK-256**（`DRIFT-044-3`），并在卡里列出四条待裁决。

另外两条事实：形状调色板与颜色调色板是**两个** `GridView`，本身也只能靠本地化 Name
（`形状` / `颜色`）区分；任意 RGB（如 `0,180,0`）不在固定调色板里，需走「编辑颜色」对话框
或收窄评测用例。

## 10. 运行时装配（TASK-106）

- `apps/agent-core` 已为 T3.1 注册 7 个 Paint handler：
  `paint.document.new` / `paint.tool.select` / `paint.color.select_foreground` /
  `paint.layer.select` / `paint.canvas.resolve_point` / `paint.canvas.draw_rectangle` /
  `paint.canvas.capture_pixels`。
- canvas 物理矩形通过 `UiAutomationProvider::element_bounds` 读取（ADR-0085），不再从
  `TreeSnapshot` 猜几何；`TreeSnapshot` 仍只有窗口句柄、指纹与节点数。
- 当前 selector 仍是 provisional，T3.1 fake 平台已跑通 `Plan -> Completed`；
  真机十次运行与拖拽误差仍由 `DRIFT-044-1` / `PL-113` 跟踪。

## 11. 真机 KeyTip / 调色板 / 形状实测（TASK-256，2026-10-09）

一次性探针 `target/paint_keytip`（不入库）：读 UIA `AccessKey` 属性、用 `keybd_event`
发 `Alt` + KeyTip 字母、并对固定调色板做外部交叉验证。版本 **11.2605.81.0**，界面 **zh-CN**。

### 11.1 Ribbon KeyTips 可用（L2 通道）

按 `Alt` 会弹出 KeyTip 徽标；`Alt` 后按字母可触发。UIA `AccessKey` 属性给出权威清单：

| 控件 | KeyTip | AutomationId |
|---|---|---|
| 铅笔 / 填充 / 文本 | `Alt+T,P` / `Alt+T,F` / `Alt+T,X` | `PencilTool` / 空 / 空 |
| 橡皮擦 / 颜色选取器 / 放大镜 | `Alt+T,E` / `Alt+T,C` / `Alt+T,M` | `EraserTool` / 空 / 空 |
| 画笔（split-button） | `Alt+B` | `BrushesSplitButton` |
| 形状轮廓 / 形状填充 / 大小 | `Alt+S,O` / `Alt+S,F` / `Alt+S,Z` | 空（未选形状时 disabled） |
| 颜色 1 / 颜色 2 / 编辑颜色 | `Alt+1` / `Alt+2` / `Alt+E,C` | 空 |
| 图层开关 | **`Alt+L`** | 空（父节点名 `层`） |
| 裁剪 / 删除背景 / 旋转 / 翻转 / 重设大小和倾斜 | `Alt+I,C` / `Alt+I,B` / `Alt+I,O` / `Alt+I,F` / `Alt+I,E` | `CropButton` / 空 / `RotateDropdown` / `Flip` / 空 |
| 保存 / 共享 / 撤消 / 重做 / 设置 / Copilot | `Alt+S,A` / `Alt+D` / `Alt+U` / `Alt+R` / `Alt+Y` / `Alt+C` | 空 / 空 / 空 / 空 / `SettingsButton` / `CopilotDropDownButton` |

已验证：`Alt+T,P` 真的把工具切到铅笔；`Alt+L` 真的让 `layersList` 出现。

### 11.2 形状：**没有** KeyTip，且形状项不可键盘聚焦

形状库容器是 `Group`（本地化名 `形状`），画廊是 `List` / `GridView`，**都无 AutomationId**；
22–23 个形状项（`直线` / `曲线` / `椭圆` / `矩形` / …）全部 `IsKeyboardFocusable=false`。
形状库本身也没有 KeyTip 徽标（只有作用于「已选形状」的 `形状轮廓` / `形状填充` / `大小`）。

→ **结论：Paint 11.2605.81.0 没有任何原生键盘方式选择形状工具。** 只能回到 L3 的 UIA 定位。

### 11.3 固定调色板：逐色块无 KeyTip，但就是一组已知 RGB

调色板容器是 `Group`（本地化名 `颜色`），画廊是 `List` / `GridView`，无 AutomationId；
20 个色块（`黑色` / `灰色` / …）无 KeyTip、无 aid、名字本地化，但 `IsKeyboardFocusable=true`。

调色板 = 公开的 **MS Paint 20** 色表（来源 `lospec.com/palette-list/ms-paint-20`），
与本机实测 RGB **逐色相等**（按实测索引顺序）：

```text
0  #000000  1  #7f7f7f  2  #880015  3  #ed1c24  4  #ff7f27
5  #fff200  6  #22b14c  7  #00a2e8  8  #3f48cc  9  #a349a4
10 #ffffff  11 #c3c3c3  12 #b97a57  13 #ffaec9  14 #ffc90e
15 #efe4b0  16 #b5e61d  17 #99d9ea  18 #7092be  19 #c8bfe7
```

→ 固定色与任意 RGB 都可以**绕开色块选择**：走「编辑颜色」（`Alt+E,C`）写 `HexTextBox`
或 `RedTextBox` / `GreenTextBox` / `BlueTextBox`（四个都是稳定 aid 的 Edit）。
注意别拿 Lospec 的另一个 **「MS Paint Basic」44 色**（XP/Vista 时代）当成本表。

### 11.4 图层面板：`Alt+L` 直接开关

发 `Alt+L` 后 `layersList`（`List`，aid=`layersList`）出现，含 `图层 1`（ListItem，可聚焦）
与 `背景`（Button，aid=`BackgroundButton`）。**不需要**先解析本地化的 `层` 按钮。

### 11.5 新坑

1. **「编辑颜色」弹层打开时整树 UIA 遍历会挂住** —— 探针卡死需强杀。解析器必须给遍历加
   上限 / 超时（ADR-0063 同族）。
2. **图层面板在 UIA 树里 `offscreen=false` 且有真实 rect，但 `CopyFromScreen` 截不到**
   （合成层）。视觉校验要走 `PrintWindow(PW_RENDERFULLCONTENT)`（ADR-0076 已是这条路）。
3. UIA 返回的 bounds 与 PowerShell `CopyFromScreen` 截图的像素尺度不一致（DPI 感知差异），
   对坐标不要跨这两个来源混算。

### 11.6 影响

- 工具 / 图层 / 任意颜色有 L2 或稳定 aid 路径；**形状与固定色块没有**。
- 形状库与调色板容器的消歧需要新候选：**ADR-0086**（`SelectorKind::ExactName`，UIA 精确相等，
  零依赖、本地化兜底），落地卡 **TASK-257**。

## 12. 形状项：没有任何可用 UIA pattern，只能走 L4 合成点击（TASK-256 / TASK-294）

**2026-10-15 真机一手证据（`DRIFT-256-5`，Paint 11.2605.81.0 / zh-CN）**：形状库展开后，
`Button|矩形` 在全树**唯一**（新锚点 `exact_name_and_role {name: 矩形, role: Button}` 正确），
但它**没有任何可用的 UIA pattern** 能改状态：

| 元素 | 实测 pattern | 结果 |
|---|---|---|
| `Button\|矩形` | `Toggle,ScrollItem` | `TogglePattern.Toggle()` **两向空操作**（`Off→Off`、`On→On`） |
| `Button\|矩形` | 无 `InvokePattern` | `invoke` 返回 `CapabilityMissing` |
| `ListItem\|矩形` | `ScrollItem,VirtualizedItem` | **无** `SelectionItemPattern` |

→ 形状项只能靠**真实鼠标点击**选中。`paint.tool.select` 因此从
`invoke_action(toggle)`（空操作）切到 **L4 合成点击**（TASK-294 的
`click_element_center`）：取 `element_geometry` + `window_geometry` 各一次 → 断言元素中心落在
窗口矩形内（半开区间，越界 `ToolInvalidArgs`、**禁 clamp**，ADR-0092 细则 8）→ 在窗口独占租约内
`pointer_action(Click)`。这条通道是产品主线（点击 / 连续点击 / 拖拽），不是权宜。

**教训：fake 通过 ≠ 真机通过。** 旧 handler 走 `invoke_action(toggle)` 在 fake 上成立、在真机上
是空操作；现在 fake **不再**为 `(rectangle_tool_id, "toggle")` 背书，改用「点落在矩形工具矩形内」
的 `Click` 才把工具置为 `rectangle`，并用调用计数断言（1×`element_geometry` + 1×`window_geometry`
+ 1×`pointer_action`）把 ADR-0092 细则 5 变成机器判据。

### 12.1 人被合成鼠标干扰（真机验收的坑，2026-10-15）

有人值守的真机用例有个天然竞争：**人类用鼠标把 Paint 切到前台的那一刻，指针（连同那次点击）仍在动**，
而用例随后就要发**合成鼠标**输入（`pointer_action` 会真的移动光标）。两者抢同一个光标会让点击落偏。

处置（`production_paint.rs` 的真机用例）：① 前台确认后先等一段**固定静默窗口**
（`REAL_RUN_FOREGROUND_SETTLE`，当前 2 s）才允许发输入；② 静默结束与**首次输入前**各做一次前台复核，
丢了前台就 fail-closed 报错、**不向任何窗口发输入**（相当于把「焦点错位会打到别的应用」这条平台约束
在验收入口再兜一层）。复跑确认：前台门 + 静默 + 复核全过、工具选择通过、无鼠标干扰失败。

### 12.2 形状 flyout 是独立弹出窗口 + 坐标被 DPI 虚拟化（2026-10-15 实测）

**「光标停在画笔 label 上」不是点错了。** 一手实测：形状库展开后是一个**独立顶层弹出窗口**
（`class = Microsoft.UI.Content.PopupWindowSiteBridge`，可见时 rect `(648,301)-(964,423)`），
不属于主窗口，所以 `PrintWindow(主窗口)` 里看不到它。同一时刻量到（均为进程可见的坐标）：

| 元素 | rect |
|---|---|
| 主窗口（`MSPaintApp`） | `(344,162)-(1413,710)` |
| 形状库弹出窗 | `(648,301)-(964,423)` |
| `矩形` 项（`exact_name_and_role{矩形, Button}`） | `(731,312)-(751,332)`，20×20 |
| `CollapsedButton`（形状库展开钮） | `(789,253)-(823,299)` |
| `BrushesSplitButton`（画笔 split-button） | `(724,249)-(764,303)` |

`矩形` 项正好落在**画笔按钮正下方约 9 px**。选中形状后弹出窗关闭，光标就停在下方那行 label 上 ——
看起来像「点在画笔标签上」，实际落点就是 `矩形` 项中心。**受控验证**：在 `(741,322)` 点一次后，
主窗口的 `形状` 变成选中工具（粗框高亮），`画笔` 高亮消失 → **合成点击这条通道本身是有效的**。

**副产品（需 ADR 级裁决）：进程未声明 DPI 感知。** 本机显示为 **3200×2000 @ 200%（DPI 192）**，
但仓库里没有任何 `dpiAware` / `dpiAwareness` manifest 或 `SetProcessDpiAwarenessContext` 调用；
实测进程看到的是 **1600×1000 / 96 DPI**，`window_geometry().space().scale()` 报 **1.0** ——
即被 Windows **DPI 虚拟化**。这与 ADR-0092 细则 3「Windows = UIA 物理像素（进程 Per-Monitor-V2 感知）」
的字面口径不符（当前只是「读写都在同一个虚拟化空间里」所以单屏自洽；混合 DPI 未验证）。
另记：UIA **只在该窗口处于活动状态时**才暴露 `CollapsedButton` / `BrushesSplitButton` / 画布 Name
（Paint 失活时这些元素解析为 `TargetNotFound`），所以真机用例的前台门是必需的。

**仍缺的一条**：`paint.tool.select` **没有后置条件** —— 它只回写本地 selection，不校验工具真的变了，
所以「点没点中」在测试里无法证伪（这正是 §12 里「fake 通过 ≠ 真机通过」的另一面）。

### 12.3 调色板色块只存在于 control view —— 与 ADR-0087 冲突（2026-10-15 实测）

T3.1 真机链路卡在 `paint.color.select_foreground`：`TargetNotFound: select: child index 3 is out of
range for this element`。只读探针（临时，未入库）用与 handler **完全相同**的链解析
`foreground_color_button`（`exact_name_and_role{颜色, Group}` → `role_and_parent{role: List}`），
拿到 `List`（240×72 @ `(888,240)`），然后：

| `child_at` 用的 walker | `select(ByIndex(0..5))` |
|---|---|
| `ContentViewWalker`（现行实现，ADR-0087） | **全部** `TargetNotFound: child index N is out of range` |
| `ControlViewWalker`（临时改一行验证） | **全部成功** |

→ **调色板的色块是 control 元素、但不是 content 元素**：该 `List` 在 content view 里是 0 个子项。
ADR-0087 的「形状库 / 调色板两类列表都走 `ContentViewWalker`」这条对调色板**不成立**（形状库路径
现在也不再经过 `Selection::ByIndex`，而是 selector + 合成点击）。

处置：这是改 ADR 已决事项（漂移触发器 ④ / ⑧），**已停工作**并记录，等裁决。候选修法（需 ADR 勘误）：
`child_at` 用「**content view → control view** 有序回退」，取不到再显式 `TargetNotFound`（仍 fail-closed、
不改公共 API、`ensure_realized` 与 `Selection::ByIndex` 语义保留）。

### 12.4 图层面板：`Alt+L` 是开关，且它的本地化父候选有 4 个命中（2026-10-15 实测）

§11.4 的结论「`Alt+L` 开合图层面板、**不需要**解析本地化的『层』按钮」在真机上再次被证实，
并补一条量化事实：**`name_regex "层"` 命中 4 个元素**（`TargetAmbiguous`），所以任何以它为父候选的
`RoleAndParent` 链都必然失败 —— 这正是 T3.1 卡在 `paint.layer.select` 的原因。

落定：图层面板走「先解析 `layersList`（aid）→ **缺失才**发 `Alt+L` → 20 × 50 ms 有界轮询重解 →
仍缺失则显式失败」；**不再**解析那个歧义 toggle，也**不再**在选完层之后盲发一次开关键把面板收回去
（面板保持展开是无害终态）。形状库（异步物化）与图层面板共用同一段有界轮询。

### 12.5 缩放显示器上的窗口截取只拿到左上角 1/4（2026-10-16 实测，**纠正本文件早前的错误结论**）

**被纠正的结论**：同一会话早前写过「`PrintWindow` 抓不到 Paint 的画布合成层」——**这是错的**。
真实原因是**截图缓冲区尺寸**，不是通道能力。

- 本机为 **200%**（物理 3200×2000、DPI 192），而 agent / 测试进程**不声明 DPI 感知** →
  `GetWindowRect` 返回**虚拟化**尺寸 `1069×548`；
- `PrintWindow(PW_RENDERFULLCONTENT)` 却按**物理**尺寸（`2138×1096`）渲染窗口内容；
- 于是按 `GetWindowRect` 尺寸分配的位图**只装下窗口左上角四分之一** —— 画布与笔迹都在被裁掉的那部分里。

**一手复现（同一 HWND `264626`，Paint 11.2605.81.0）**：

| 位图尺寸 | 抓到的内容 |
|---|---|
| `1069×548`（= 虚拟化窗口尺寸，也正是产品 `capture` 的取法） | 只有工具栏 + 应用背景灰；**无画布、无红色**（整图 0 行纯白） |
| `2138×1096`（= 物理尺寸） | **完整窗口**：画布、层层面板、**红色矩形**全在；`#ed1c24` 像素 **2276** 个 |
| `3207×1644`（3×） | 与 2× **内容逐像素相同**（多出的是空白 padding）→ PrintWindow 固定按物理尺寸渲染 |

**修正后的结论**：

1. **`PrintWindow` 能抓到 Paint 画布与笔迹** —— ADR-0076 的主通道本身可用；ADR-0076 里
   「WGC 留给确有硬件合成窗口证据的后续卡」这条**目前没有证据支持**，不要据此换通道。
2. 真问题在 **DPI 感知缺失**：`crates/platform/windows/src/window/capture.rs` 的缓冲尺寸来自
   `win32::window_rect`（`GetWindowRect`）；不感知 DPI 的进程拿到虚拟化尺寸 → **缩放显示器上 `capture`
   只返回窗口左上角 1/4 图**（`MAX_CAPTURE_PIXELS` 的像素上限校验也因此按错误尺寸算）。
3. 所以 **ADR-0092 细则 3 要求的 Per-Monitor-V2 感知**不只是坐标口径问题，**同时是截图完整性问题**。
4. 旁证：状态栏显示 **`40 × 40像素`**（即 (10,10)→(50,50) 的 40×40）与 **`100%`** 缩放 →
   T3.1 那一次拖拽在**画布像素层面是正确的**，缺的只是能把它拍下来的截图。

**对 harness 的影响**：TASK-303 的 `zoom_ratio = 渲染宽 / 画布宽` 得到 0.5 —— 在**当前（不感知 DPI 的）
进程坐标系**里这是**自洽且正确**的换算（物理 418 px 在该空间里就是 209 px），所以真机那次画出了正确的
40×40。一旦进程改为 PMv2 感知，同一公式会自动变成 1.0，**公式无需改**。

### 12.6 未保存三态框是 UIA 不可见的 WinUI 弹层（2026-10-16 实测，TASK-306）

人类保留未保存的红色矩形并复跑 T3.1 时，Paint 弹出：

```text
是否要保存?
"无标题"中有未保存的更改。
保存 / 不保存 / 取消
```

**通道事实**：

- `PrintWindow(PW_RENDERFULLCONTENT)` 能抓到完整窗口与这个框；
- UIA **ControlView 与 RawView 都看不到它**（只看到主窗口工具栏 / 画布 / 图层面板）；
- `EnumThreadWindows` 能看到同一 Paint 线程上的顶层窗口
  `class=Microsoft.UI.Content.PopupWindowSiteBridge`、`title=主机弹出窗口`；
- `WindowState.occluded` 现在把目标线程上的该类弹层（以及经典 `#32770`）保守视为遮挡；
  真实只读探针在框打开时得到 `occluded=true`。

**为什么这直接影响 T3.1 的结论**：`paint.document.new` 盲发 `Ctrl+N` 后，保存框留在前台；
旧红色矩形仍在画布上，而 `visual_assert` 的 `changed_ratio_within` 只要求
`changed_ratio <= 0.08`，**两张完全相同的图也会 Satisfied**。因此那一轮“视觉断言通过 + 指纹不变”
不是矛盾，而是：**本轮没有有效写入新内容，旧矩形让视觉断言误过，`state_changed` 正确报警**。
`crates/verify/tests/visual_postcondition_contract.rs` 已用契约测试固定“相同图可 Satisfied”。

**处置**：TASK-306 让平台层把该弹层纳入 `WindowState.occluded`，并让 `paint.document.new`
在发 `Ctrl+N` 前、以及 `Ctrl+N` 后有界窗口内发现弹层时立即以 `ErrorCode::UserInteraction`
fail-closed；不再继续 `Alt+L` / 工具 / 拖拽。TASK-305 的 A 方案（删除 `state_changed`）已撤回。

### 12.7 已有画布续画 T3.2（2026-10-16 实测，TASK-307）
T3.2 任务包共 9 步、不含 `paint.document.new`；`draw_rectangle` 用 before/after capture 的 content address 做下界，相同即 `VerifyFailed`。生产入口默认 `--document-mode existing` → T3.2，只有显式 `new` 才选 T3.1；真机已有未保存画布复跑 `1 passed / 8.72 s`，无保存框。

## 13. 画笔 / 形状 / 颜色 工具集清单（人类截图，2026-10-16）

> 来源：人类提供的本机 Paint 界面截图，用于「完整画笔 / 形状 / 颜色工具集」的对照。
> 名称按截图中文字**逐条抄录**：不改名、不翻译、不补全。截图未覆盖的部分一律标「未覆盖」。
> 本节只记**有哪些项、叫什么**；每项的 UIA 形状（name / aid / control type / pattern）
> 仍须真机探针取证，未取证前不得当事实用。

### 13.1 画笔下拉（`BrushesSplitButton` 展开后的笔型列表）

截图自上而下共 **9** 项，每项右侧为该笔型的笔画预览：

| # | 名称 | 截图当时状态 |
|---|---|---|
| 1 | 画笔 | 选中（行左侧蓝色竖条） |
| 2 | 毛笔 | 悬停高亮（灰底） |
| 3 | 书写笔 | — |
| 4 | 喷枪 | — |
| 5 | 油画笔 | — |
| 6 | 蜡笔 | — |
| 7 | 记号笔 | — |
| 8 | 普通铅笔 | — |
| 9 | 水彩画笔 | — |

**边界**：截图下沿紧贴最后一项，列表是否可滚动、其下是否还有未截到的笔型 —— **未覆盖 / 未确认**。

**命名提醒（两组同名，必须消歧）**：

1. 清单第 1 项「画笔」与工具栏 `BrushesSplitButton` 同名（工具栏那个 name 也是 `画笔`）。
   只按 name 选会同时命中工具栏按钮与本 flyout 项 —— 必须叠加层级 / aid / 是否在 flyout 内区分。
2. 清单第 8 项「普通铅笔」与工具栏「铅笔」不是同一个控件（工具栏 `ToggleButton`
   aid = `PencilTool`、KeyTip `Alt+T,P`，见 §11.1）。

**待探针确认**（尚未真机取证）：每项的 name / aid / ControlType、是否 `IsKeyboardFocusable`、
有无可用 UIA pattern、以及 flyout 项与 `BrushesSplitButton` 的层级关系。

### 13.2 形状库（形状画廊，23 项，8 / 8 / 7 三行）

图库**只有图标、没有文字标签**；下表名称由人类提供，是 **Paint 的标准中文名称**
（2026-10-16 确认，替代此前按图标形状的推断）。按截图从上到下、从左到右：

| 行 | 顺序（左 → 右） |
|---|---|
| 1 | 直线 / 曲线 / 椭圆 / 矩形 / 圆角矩形 / 多边形 / 三角形 / 直角三角形 |
| 2 | 菱形 / 五边形 / 六边形 / 向右键 / 向左键 / 向上箭头 / 向下箭头 / 四角星 |
| 3 | 五角星 / 六角星 / 圆角矩形标注 / 椭圆形标注 / 云形标注 / 心形 / 闪电 |

截图当时 `椭圆` 带蓝色选中框。图库是否可滚动、是否还有第 4 行 —— **未覆盖**。

**仍未取证**：以上是**标准中文名称**，**不等于 UIA name**。目前唯一有真机 name 证据的仍是 `矩形`
（TASK-256 记到 `Button` / name `矩形`，见 §12）；其余 22 项的真实 name / aid / pattern 仍需真机探针，
取名时**不得**直接用本表反推 selector。

### 13.3 形状轮廓（轮廓按钮展开）

自上而下共 **7** 项（2026-10-16 清晰截图确认；此前基于模糊截图的暂读作废）：

| # | 名称 | 截图当时状态 |
|---|---|---|
| 1 | 无边框 | — |
| 2 | 实心边框 | — |
| 3 | 蜡笔 | — |
| 4 | 记号笔 | 选中（行左侧蓝色竖条） |
| 5 | 油画笔 | — |
| 6 | 普通铅笔 | — |
| 7 | 水彩画笔 | — |

列表是否可滚动、`水彩画笔` 之下是否还有项 —— 仍**未覆盖**（两版截图都止于 `水彩画笔`）。

### 13.4 形状填充（填充按钮展开）

自上而下共 **7** 项（2026-10-16 清晰截图确认；此前基于模糊截图的暂读作废）：

| # | 名称 | 截图当时状态 |
|---|---|---|
| 1 | 无填充 | — |
| 2 | 纯色填充 | — |
| 3 | 蜡笔 | — |
| 4 | 记号笔 | — |
| 5 | 油画笔 | 选中（行左侧蓝色竖条） |
| 6 | 普通铅笔 | — |
| 7 | 水彩画笔 | — |

列表下沿同样**未覆盖**。

### 13.5 大小（大小按钮展开）

自上而下：`1 像素` / `3 像素` / `5 像素`（截图当时选中）/ `8 像素`。
是否还有更大的尺寸 —— **未覆盖**。

### 13.6 同名冲突（写 selector 必须先消歧）

`蜡笔` / `记号笔` / `油画笔` / `普通铅笔` / `水彩画笔` 这 5 个名字在**三个 flyout 里都会出现**：
画笔下拉（§13.1，另含 `画笔` / `毛笔` / `书写笔` / `喷枪`）、形状轮廓（§13.3）、形状填充（§13.4）。
只按 name 选会跨 flyout 误命中。正确顺序是**先锚定 flyout**（§13.1 的 `BrushesSplitButton`、
§13.3 的轮廓按钮、§13.4 的填充按钮），再在 flyout 内按 name 选。

### 13.7 颜色面板（调色板展开）

布局与语义（2026-10-16 人类逐项说明确认；此前「底部空心圆含义未确认」的记法作废）：

- **左侧上下两个大圆 = 两个「当前要用的颜色」槽位**，点其中一个即把它设为当前激活槽。
  选色是**两步**流程：先点左边大圆之一 → 再点右侧某个颜色，该大圆就变成那个颜色。
  （截图里上方大圆 = 黑、带蓝色选中环，下方大圆 = 白。按 Paint 默认值**推断**上方 = `颜色 1`、
  下方 = `颜色 2`（KeyTip `Alt+1` / `Alt+2`，见 §11.1），**未逐项确认**。）
- **右侧带 `+` 的彩虹大圆 = 「编辑颜色」入口**：点进去可配出任意颜色，并与下面 10 个小圆联动。
- **下方 10 个小圆 = 自定义颜色槽位**：自定义颜色被选中后会**依次存入**这 10 个备用；
  之后再点其中某个，左侧「当前激活的大圆」就切换成该自定义颜色。
  **没填入过颜色的槽位是不可选（disabled）状态**，只有存过自定义色的槽位才能点。

固定色块（**2 行 × 10**）的**顺序与 §11.3 的真机实测 RGB 逐色一致**（本截图 = 第二次独立确认）：

| 行 | 顺序（左 → 右） |
|---|---|
| 1 | 黑色 / 灰色 / 深红色 / 红色 / 橙色 / 黄色 / 绿色 / 青绿 / 靛蓝色 / 紫色 |
| 2 | 白色 / 浅灰 / 褐色 / 玫瑰红 / 金色 / 浅黄色 / 酸橙色 / 淡青绿色 / 蓝灰色 / 淡紫色 |

**对自动化有两条影响**：

1. 选色不必逐点点色块：固定色走 §11.3 的 20 色表，任意 RGB 走「编辑颜色」（`Alt+E,C` →
   `HexTextBox` / `RedTextBox` / `GreenTextBox` / `BlueTextBox`）。
2. **下面 10 个自定义槽位装的是用户内容，颜色不固定**，不得当稳定 selector 目标，
   也不得假定它们初始为空、为白或可预测 —— 读回与断言都必须容忍任意颜色。

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

### 13.15 `大小` flyout：未选中形状时条目为空（原因见 13.13）

同两级展开（`CollapsedButton` → name `大小`）。`ExpandCollapsePattern.Expand()` 成功
（`state=Expanded`），但**展开后 ControlView 里没有任何「N 像素」条目**：

- `type=MenuItem` 全树只有 3 项（`文件` / `编辑` / `查看`），不含尺寸；
- 完整无过滤遍历 `seen=174 emitted=174 truncated=False`，唯一匹配 `像素` 的是画布尺寸文本
  `418 × 74像素`（aid `CanvasSizeTextBlock`）；
- 坐标带 `x∈1780~1999` 只捞到 `大小` 按钮本身。

→ **§13.5 的 4 档（1 / 3 / 5 / 8 像素）目前没有 L3 通道**。与 §11.2 / §12.3 同族：
该 flyout 的条目只存在于合成层。要选尺寸得走键盘或 L4 合成点击，或另找 RawView / 非 ControlView 路径。
**未取证的尺寸值不得猜测放行**。
