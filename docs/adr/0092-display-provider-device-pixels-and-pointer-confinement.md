# ADR-0092　显示器归属：独立 `DisplayProvider`、设备像素口径与指针窗口约束

状态：Accepted　日期：2026-10-15　Supersedes：—　Superseded by：—
关联：ADR-0016（平台 trait 形状 / RPITIT）、ADR-0022 D4（可见文本不作主 selector）、ADR-0043（解析必须在窗口 scope 内）、
ADR-0045（非宿主平台 clippy）、ADR-0063（长期状态有界 / 子进程回收）、ADR-0067（显式 `CoordinateSpace`）、
ADR-0085（`element_bounds`）、`tasks/TASK-256-paint-real-contract-calibration.md`（`DRIFT-256-6`）、
`cross-platform-ai-assistant-architecture-v2.md` §13.1.2（能力矩阵）/ §13.4（Linux Wayland-first）

## 背景

产品主线是**主窗口 dock 磁吸在目标应用旁，用户在自家 app 内下指令，助手只操作被吸附应用的窗口范围**。
这要求三件事：拿到目标窗口在桌面上的矩形、拿到元素矩形并知道它**属于哪块显示器**、以及把合成输入
**限制在该窗口内**。

现状（`DRIFT-256-6` 的第一手证据）：

- `WindowProvider::resolve_window` 只回 `WindowInfo { window, app_id, title }` —— **没有几何**；
  `WindowState` 只有 `minimized / foreground / occluded`。
- `element_bounds`（ADR-0085）已有，回物理像素矩形，但**不带显示器归属**。
- `PointerAction` + 显式 `CoordinateSpace { kind, scale, origin_display }` 机制齐全（ADR-0067），
  但 Windows 侧 `coordinates::monitor_for_coordinate_space` 会**按 `origin_display` 查真实显示器并校验 scale**：
  猜一个名字 → 多屏 / 缩放机上**静默点偏**（且点偏的是鼠标）。
- 平台内部其实**早就有** `coordinates::enumerate_monitors`（`MonitorRecord{device_name, bounds, scale, primary}`）
  与 `win32::window_rect`（`GetWindowRect`）—— 只是没暴露到 `platform/api`。

`crates/platform` 目前只有 `api` 与 `windows`；macOS/Linux 在阶段 5~7。所以本 ADR 的首要任务不是"给
Windows 加三行代码"，而是**把契约定成平台中立的**，让后两个平台只是实现同一组 trait，而不是各造一套。

## 决策（一句话）

**加法式**新增独立 trait **`DisplayProvider`**（显示器目录）与三处几何查询（窗口几何 / 元素几何），
**统一以「设备像素 + 显式 `DisplayId` + `scale`」表达坐标**，并把「合成指针必须落在已解析目标窗口矩形内」
写成契约级不变量；热路径**禁止枚举显示器**，换算只在平台实现内部做一次。

细则：

1. **独立 trait（人类 2026-10-15 裁决）**：
   `pub trait DisplayProvider: Send + Sync { fn displays(&self) -> impl Future<Output = PlatformResult<Vec<DisplayInfo>>> + Send; }`
   —— 不复用 `PlatformService`，以便「某平台只实现显示器目录、不实现窗口几何」这类能力分级。
2. **纯类型进 `crates/platform/api`**（`#[non_exhaustive]`，只回可序列化数据 —— 铁律 8）：
   - `DisplayId(String)`：不透明、非空。Windows 形如 `\\.\DISPLAY1`、macOS 为 `CGDirectDisplayID`、Wayland/X11 为输出名。
   - `DisplayInfo { id, bounds: ElementBounds, scale: f64, primary: bool }`
   - `WindowGeometry { bounds: ElementBounds, display: DisplayId, scale: f64 }`
   - `ElementGeometry { bounds: ElementBounds, display: DisplayId, scale: f64 }`
3. **单位口径 = 设备像素（人类 2026-10-15 裁决）**，换算在**平台实现内部**完成，core / handler **零换算**：
   Windows = UIA 物理像素（进程 Per-Monitor-V2 感知）；macOS = Quartz 点 × `backingScaleFactor`；
   X11 = 像素（scale 1.0）；Wayland = 输出局部逻辑 × 输出 scale。每个几何值**自带 `scale`**，
   调用方据此构造 `CoordinateSpace`，不必再去问目录。
4. **不承诺"一个全球桌面网格"，只承诺自洽 + 可归属**：同一次会话内所有几何查询与合成输入必须使用同一套
   口径；该口径的**逐平台定义写在各实现里**（Wayland 没有全局桌面坐标，见细则 7）。
5. **热路径纪律（开销，人类 2026-10-15 点名的关注点）**：
   - **`displays()` 不得出现在 pointer 热路径**；热点只用 `window_geometry` / `element_geometry` 自带的 `display` + `scale`。
   - `displays()` **必须可缓存**：有界 TTL + 显式失效（ADR-0063：长期状态有界；时钟一律 trait 注入），
     显示器变化事件（Windows `WM_DISPLAYCHANGE`、macOS `NSApplicationDidChangeScreenParametersNotification`、
     X11 RandR）触发失效。缓存命中时换算开销 = 0。
   - 若某平台换算本身贵（例如要跨进程问 compositor 要输出 scale）→ **不得在热路径现算**，必须走缓存。
   - **可机器验证的上限**：`click_element_center` 一次点击的平台调用 = **1 次元素几何 + 1 次 pointer_action**
     （元素解析算在调用方那一步）；用既有 fake 的调用计数器断言，超限即红灯。
6. **一致性不变量**：同一元素上 `element_geometry().bounds == element_bounds()`；`WindowGeometry.scale` 与
   `ElementGeometry.scale` 必须等于 `displays()` 里同 `DisplayId` 的 `scale`（两处都上测试）。
7. **能力分级 + Wayland 降级**：Wayland 下协议**不允许**枚举 / 定位其他应用窗口 → `window_geometry` 返回
   `CapabilityMissing`（**绝不返回空矩形假装可用**）；`DisplayProvider::displays()` 在 Wayland 可用但只覆盖
   compositor 暴露的输出，且 scale 可为分数。`CapabilityMatrix.channels` 增加
   `display-catalog` / `window-geometry` / `element-geometry` / `pointer-confinement` 四项，
   Wayland 的 `window-geometry` 与 `pointer-confinement` 进 `degradations`。
8. **指针窗口约束（C4）**：合成指针的点**必须**落在已解析目标窗口的 `bounds` 内；越界 → `ToolInvalidArgs`。
   **禁止 clamp / 悄悄挪点**（静默扩大范围 = 铁律 9）。键盘侧 `KeyTarget::Window(window)` 已天然满足该约束。
9. **不改**：`PointerAction` 形状、`ErrorCode`（本 ADR 不新增错误码，用既有 `ToolInvalidArgs` / `CapabilityMissing` /
   `TargetNotFound` / `TargetUnresponsive`）、`element_bounds`、`protocol/tool-schema`。

## 考虑过的选项（含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | 只加 `element_display`，热路径查两次（bounds + display） | ❌ 否决 | 每次点击多一次平台往返；人类明确要求先把开销算清 |
| 2 | 显示器目录塞进 `PlatformService` | ❌ 否决 | 人类 2026-10-15 裁决：单独开 `DisplayProvider`，便于按能力分级 |
| 3 | core 侧做单位换算 | ❌ 否决 | core 不该知道平台单位；换算在实现侧做一次即可 |
| 4 | 统一用逻辑像素 | ❌ 否决 | Windows 原生给物理像素，反算引入舍入误差；人类裁决用设备像素 |
| 5 | handler 里猜一个显示器名 / scale | ❌ 否决 | 混合 DPI 下会**静默点偏**，ADR-0067 已禁止收敛式猜测 |
| 6 | 越界时 clamp 到窗口内 | ❌ 否决 | 静默挪点 = 静默扩大范围（铁律 9）；应 fail-closed |
| 7 | 不做归属查询，只把窗口矩形当"范围" | ❌ 否决 | 无归属就没法构造合法 `CoordinateSpace`，点击落不到目标窗口 |

## 影响

- `crates/platform/api`：新增 `DisplayProvider` + 4 个纯类型 + `WindowProvider::window_geometry` +
  `UiAutomationProvider::element_geometry`。**平台中立**，不得出现 Windows 专有词汇。
- `crates/platform/windows`：实现三处查询（复用既有 `coordinates::enumerate_monitors` / `win32::window_rect`）
  + 目录缓存与失效钩子；`unsupported.rs` 的非 Windows 桩一律显式 `CapabilityMissing`。
- `apps/agent-core`：新增 `click_element_center` 原语（几何 + 窗口内断言 + 租约内 `pointer_action(Click)`）。
- `apps/desktop-ui` / IPC：dock 定位需要 `WindowGeometry` 经 IPC 到 UI（**新 IPC 方法 = 公共接口，另卡 + spec 同步**）。
- `docs/spec/capability-matrix.md` 与 `protocol/capability-matrix`：补四项能力分级（改 schema 才需再走 ADR）。
- 落地顺序（一卡一会话，ADR-0031）：**卡2** `platform/api` + `platform/windows` 实现与单测 →
  **卡3** `click_element_center` + TASK-256 切到合成点击 → **卡4** dock 定位 + IPC。

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 换了行不通的平台 API 形状，三个平台各改一遍 | 契约只放纯类型 + 四查询；Windows 先实现，macOS/Linux 在阶段 5~7 只填实现 |
| 换算在热路径现算导致每次点击变慢 | 细则 5：换算在实现侧一次；`displays()` 禁入热路径；调用计数上限 + 缓存 TTL |
| 缓存陈旧 → 点了错的显示器 | 显式失效钩子 + 有界 TTL；缓存命中也要过"点在窗口内"的断言 |
| `GetWindowRect` 含 DWM 阴影边框，边缘点击越界 | 窗口矩形只作**范围约束**，真实点击点用 UIA 元素几何；边缘留显式 inset |
| `element_bounds` 随布局失效 | 点击前重解一次，不跨步骤缓存坐标 |
| Wayland 上承诺了做不到的事 | 细则 7：能力矩阵显式 `degradations`，`window_geometry` 直接 `CapabilityMissing` |

## 验证方式

1. `cargo test -p assistant-platform-api -p assistant-platform-windows`：四个负向用例（未知显示器 /
   点在窗口外 / 零尺寸窗口 / 最小化窗口）必须**显式失败**，不得返回近似值。
2. 一致性不变量（细则 6）各有单测。
3. 调用计数断言（细则 5）：fake 记录平台调用数，`click_element_center` 超上限即红灯。
4. ADR-0045 的两条非宿主 `--target` clippy：`platform/api` 是纯 Rust，必须三平台都过。
5. 真机：TASK-256 的 ignored 用例（选矩形工具走合成点击）复跑。

**重新评估的触发条件**：出现「窗口横跨两块显示器」或需要比单 `DisplayId` 更细的归属时，再评估是否引入
`DisplayRegion`；在此之前不预先抽象。
