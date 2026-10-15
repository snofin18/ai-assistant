# ADR-0093　勘误 ADR-0092 的接口形状：几何回 `CoordinateSpace`、撤销 `DisplayProvider`

状态：Accepted　日期：2026-10-15　Supersedes：**ADR-0092 细则 1 与细则 2**（其余条款不变）　Superseded by：—
关联：ADR-0092（本勘误的对象）、ADR-0016（RPITIT）、ADR-0030（派生值指向 SSOT）、ADR-0031（一卡一文件 / 契约先行）、
`tasks/TASK-256-paint-real-contract-calibration.md`（`DRIFT-256-6`）

## 背景

卡2（ADR-0092 的平台实现）**开工第一步是读现状**，当场发现 ADR-0092 的接口形状与既有契约冲突：

- `crates/platform/api/src/traits/session.rs:27` **已经存在**公共类型
  `DisplayInfo { id, scale_numerator, scale_denominator, primary }`，且经
  `SessionState::displays()`（`session.rs:129`）与 `PlatformService::session_state()`（`session.rs:147`）对外可用
  —— **显示器目录在契约里早就有了**。
- ADR-0092 却写「新增 `DisplayInfo { id, bounds, scale: f64, primary }`」，于是：① 与既有公共类型**同名不同形**，
  重声明 = 删改既有公共 API；② `scale` 表示与既有的**有理数**口径冲突（后者是为避免浮点比较刻意选的）；
  ③ `DisplayProvider::displays()` 与既有目录**重复**。

`DRIFT-256-6` 的第一手证据（`monitor_for_coordinate_space` 按 `origin_display` 查真实显示器并校验 scale）
仍然成立 —— 缺的从来不是"目录"，而是**几何值本身的显示器归属**。

## 决策（一句话）

**撤销 ADR-0092 的 `DisplayProvider` 与"新增 `DisplayInfo`"两处**；几何查询直接返回
`{ bounds: ElementBounds, space: CoordinateSpace }` —— `space.origin_display()` 即显示器标识、
`space.scale()` 即缩放，**不再新造 scale 表示**，也不动既有公共类型。

细则：

1. **撤销独立 trait**：显示器目录复用既有 `SessionState.displays()`，不新增 `DisplayProvider`。
   ADR-0092 的**热路径纪律原样保留**：`session_state()` 同样**不得**出现在 pointer 热路径。
2. **不重声明 `DisplayInfo`**：既有类型保持只读。它没有 `bounds` 字段不是问题 —— 点击路径要的是
   **窗口 / 元素的矩形 + 该矩形所属的坐标空间**，不是"每块显示器的矩形"。
3. **几何查询的形状**（trait 归属沿用 ADR-0092）：

   ```rust
   pub struct WindowGeometry  { bounds: ElementBounds, space: CoordinateSpace }
   pub struct ElementGeometry { bounds: ElementBounds, space: CoordinateSpace }

   trait WindowProvider { fn window_geometry(&self, window: &ResolvedWindow) -> /* … WindowGeometry */; }
   trait UiAutomationProvider { fn element_geometry(&self, element: &ResolvedElement) -> /* … ElementGeometry */; }
   ```

   `CoordinateSpace` 已含 `kind / scale / origin_display` 三要素，正是 `pointer_action` 的入参 ——
   调用方拿到几何就能直接构造点击，**一次查询**，不需要再问目录。
4. **一致性不变量（替换 ADR-0092 细则 6）**：`element_geometry().bounds == element_bounds()`；
   `space.kind() == CoordinateSpaceKind::PhysicalPixels`；`space.origin_display()` 必须对应一枚**真实枚举到的显示器**。
5. **其余条款全部不变**：设备像素口径（0092 细则 3）、同会话自洽 + 可归属（4）、热路径纪律与
   「一次 `click_element_center` = 1 次几何 + 1 次 `pointer_action`」的调用上限（5）、
   Wayland 进 `degradations` 且 `window_geometry` 返回 `CapabilityMissing`（7）、
   指针必须落在目标窗口矩形内且**禁止 clamp**（8）、不改 `PointerAction` / `ErrorCode` / `element_bounds`（9）。

## 考虑过的选项（含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | 直接改 ADR-0092 正文 | ❌ 否决 | 仓库规则：改决策 = **新 ADR** + `decisions.md` 追加行，不删旧行 |
| 2 | 给既有 `DisplayInfo` 加法式补 `bounds` | ❌ 否决 | `DisplayInfo::new` 是 4 参 `const fn`，补字段会动既有构造面；而点击路径**根本不需要**目录里的矩形 |
| 3 | 保留 `DisplayProvider` 但只做目录查询 | ❌ 否决 | 与 `SessionState.displays()` 重复；多一个逐平台必实现的 trait，却不带来任何新信息 |
| 4 | 新造 `DisplayId` newtype | ❌ 否决 | `CoordinateSpace.origin_display()` 已是 `String` 标识；再包一层不增加任何约束 |
| 5 | 几何回 `(ElementBounds, DisplayId, f64)` 三元组 | ❌ 否决 | 等于把 `CoordinateSpace` 拆开又拼回去；调用方还得自己调 `CoordinateSpace::new`，多一处可写错的地方 |

## 影响

- `crates/platform/api`：只新增 **2 个结构 + 2 个方法**，**不含新 trait**；`DisplayInfo` / `SessionState` 一字不改。
- 卡2 范围相应缩小（不必实现"显示器目录查询"）；`docs/spec/capability-matrix.md` 的能力项由 4 项减为
  **3 项**（`window-geometry` / `element-geometry` / `pointer-confinement`）。
- `crates/platform/windows`：`window_geometry` 复用 `win32::window_rect`；`element_geometry` 复用既有
  `element_bounds` 路径 + `coordinates::enumerate_monitors` 求归属；`unsupported.rs` 一律显式 `CapabilityMissing`。

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| `space` 里的 `origin_display` 与真实显示器不匹配（真机点偏） | 细则 4 的不变量 + 平台侧 `monitor_for_coordinate_space` 既有的校验（找不到即 `TargetNotFound`） |
| 调用方拿 `space` 去点**窗口外**的点 | ADR-0092 细则 8 原样保留：越界 → `ToolInvalidArgs`，禁止 clamp |
| 目录（`SessionState`）被误用成热路径 | ADR-0092 细则 5 原样保留：热路径禁止目录查询，调用计数上限机器断言 |

## 验证方式

1. 卡2 单测：`element_geometry().bounds == element_bounds()`；`space.kind()` 恒为 `PhysicalPixels`；
   四个负向用例（未知显示器 / 点在窗口外 / 零尺寸窗口 / 最小化窗口）必须**显式失败**。
2. 调用计数：`click_element_center` = 1 几何 + 1 `pointer_action`（fake 计数器断言）。
3. 非宿主 `--target` clippy（ADR-0045）。
4. 真机：TASK-256 的 ignored 用例复跑（选矩形工具走合成点击）。

**重新评估的触发条件**：若将来 dock 需要"显示器的物理矩形"来做跨显示器避让，再评估补一个只读的
`DisplayBounds`；届时同样走新 ADR，不在本勘误里预先加字段。
