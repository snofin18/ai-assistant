# TASK-293　`platform/windows` 实现 `window_geometry` / `element_geometry`（O(1) 显示器归属）

- 状态：Review
- 阶段：1　子阶段：**1b**　批次：治理池　依赖：TASK-292（契约半边已落地）
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。

---

- **依赖**：TASK-292、ADR-0092（细则 3 / 5 / 7 / 8）、ADR-0093（几何形状）、ADR-0067
- **write scope**：`crates/platform/windows/**`
- **关联**：`DRIFT-256-6`、`tasks/TASK-256-paint-real-contract-calibration.md`

**目标**

在 Windows 实现侧把两个几何查询落地：矩形取设备像素，显示器归属取 **O(1) 查询**，不枚举显示器。

**In scope**

1. `window/mod.rs`：`WindowProvider::window_geometry` + 自由函数（`GetWindowRect` → `MonitorFromWindow` → `GetDpiForMonitor`）。
2. `uia/mod.rs`：`UiAutomationProvider::element_geometry` + 自由函数（`tree::element_bounds` → `MonitorFromPoint(中心)`）。
3. 负向用例：零尺寸 / 最小化 / 句柄失效。

**Out of scope（做了算漂移）**

- `crates/platform/api/**`（TASK-292 已落）、`apps/agent-core/**`（TASK-294）、dock / IPC（TASK-295）。
- 改 `element_bounds` / `PointerAction` / `ErrorCode`；加依赖 / `#[allow]` / `unsafe`（`unsafe` 仅限既有平台 crate 惯例处）。
- 指针「点在窗口内」的判定（那是调用方义务，TASK-294）。

**DoD**

- [ ] 两个查询实现并用既有 `coordinates` 助手做 O(1) 归属（**不**调用 `enumerate_monitors`）
- [ ] 零尺寸 / 最小化 → `TargetUnresponsive`；句柄失效 → `TargetNotFound`
- [ ] `element_geometry().bounds` 与 `element_bounds()` **同一来源**（不是两处各算一遍）
- [ ] 负向用例存在且被 CI 跑到（真机 ignored 或纯函数 seam）
- [ ] `LEDGER.md` 追加一行

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

- **任务**：TASK-293（TASK-292 契约在 Windows 侧的实现）。
- **write scope**：仅 `crates/platform/windows/**`。
- **铁律**：1（显式失败，不用 96 兜底）、7、8。
- **禁止**：动 `platform/api` / `agent-core`；加依赖 / `#[allow]`。
- **疑问**：无。

### 2. 实际改动文件

- `crates/platform/windows/src/window/mod.rs`（trait 方法 + 自由函数）→ **In scope**
- `crates/platform/windows/src/uia/mod.rs`（trait 方法 + 自由函数）→ **In scope**

### 3. 验收输出摘要

- `cargo fmt --all`：通过。
- `cargo clippy -p assistant-platform-windows --all-targets -- -D warnings`：**exit 0**（0 error）。
- 关键：**两个查询都走 O(1) 归属** —— `window_geometry` 用 `coordinates::monitor_for_window`
  （`MonitorFromWindow`），`element_geometry` 用 `coordinates::monitor_for_physical_point`
  （`MonitorFromPoint`），**都没有调用 `enumerate_monitors`**，符合 ADR-0092 细则 5 的开销纪律；
  坐标空间由既有 `coordinates::coordinate_space_for_monitor` 构造（复用 `scale_from_dpi`，不新造换算）。
- `element_geometry().bounds == element_bounds()` 是**结构性**成立的：几何函数内部直接调
  `tree::element_bounds(element)`，不是在别处重算一遍。

### 4. DoD 逐条核对

- [x] 两个查询实现，O(1) 归属，未调用 `enumerate_monitors`
- [x] 句柄失效 → `TargetNotFound`（`is_window` 检查）；最小化 / 矩形不可读 / 零尺寸 → `TargetUnresponsive`
- [x] 几何与 `element_bounds` 同源
- [x] 负向用例已落地：`window_geometry_from_parts`（纯函数 seam）承载全部判定规则，
      5 条用例进 `cargo test` 常规路径（最小化 / 矩形不可读 / 零尺寸 ×2 / 空显示器名 / 正例取证），
      实测 `cargo test -p assistant-platform-windows --lib` = **109 passed / 0 failed / 4 ignored**。
- [x] `LEDGER.md` 追加一行

### 5. 偏差

none。

### 6. 更合理做法

- 复用既有 `coordinates::{monitor_for_window, monitor_for_physical_point, coordinate_space_for_monitor}`：
  归属与缩放换算**一处实现**，避免在 geometry 路径里复制一份 `GetDpiForMonitor` 逻辑（复制就会漂移）。
- 用 `saturating_add` 求中心点，避免极端坐标下 `i32` 溢出（越界值会由 `MonitorFromPoint` 显式拒绝）。

### 7. 遗留问题

- 负向用例已落地（见 §4）；`element_geometry` 侧的等价 seam 暂未抽（它的唯一输入是
  `tree::element_bounds` 的结果 + 中心点，规则只有"取中心"，暂无可单测的分支）。
- `get 混合 DPI / 多屏真机` 尚未取证：本实现按「元素/窗口属于单块显示器」假设，跨屏窗口以
  矩形中心归属（ADR-0093「重新评估触发条件」里已写明何时该升级为 `DisplayRegion`）。

### 8. 新增长期记忆

- 无（实现复用既有助手，未发现新的应用坑）。

### 9. 给审阅者的关注点

- 最小化窗口的矩形在 Win32 里其实是「有效但不可用」（常在 -32000 附近），本实现**显式**把它判成
  `TargetUnresponsive`；请确认这比"照抄一个不可用矩形"更符合调用方预期。
- 跨显示器窗口按**中心点**归属：简单、可预测，但不是"窗口中点所在屏"之外的任何一种规则；若产品
  将来需要按面积占比归属，应走 ADR 而非就地改。
