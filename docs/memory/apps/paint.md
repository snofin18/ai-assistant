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
