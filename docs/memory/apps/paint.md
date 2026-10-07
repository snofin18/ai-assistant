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

**未测**。本轮没有取得真实 Paint 窗口的进程、hwnd、窗口类或 UIA 根元素。

已知约束（来自 feasibility P3 与通用 Windows 规则）：

- 启动必须走 AUMID，不得把启动返回 PID 当成窗口属主 PID。
- 必须用 `EnumWindows` + `GetWindowThreadProcessId` 解析窗口属主。
- 画布坐标与屏幕坐标不是同一个空间，DPI / 多屏 / 最小化窗口都必须显式处理。

## 3. UIA 形状

**未测**。本轮 PowerShell / UIA 探针通道处于已知挂起状态，没有产出可引用的
Paint 树快照。

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

- 真实 Paint UIA 树与 AutomationId 普查。
- 画布 bounds、viewport origin、zoom 与 scroll offset 的精确读取方式。
- 单点 / drag 命中误差，含混合 DPI 多屏。
- 工具按钮、颜色按钮、图层行的真实 selector 成功率。
- `Ctrl+Z` 的 undo 粒度、合并行为与深度。
- 像素快照大小、截图耗时、树遍历耗时。
- 保存 / 另存为 / 未保存对话框与覆盖确认的真实按钮身份。
- TASK-044 的真实 Paint 校准结果。
