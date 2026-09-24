//! 坐标归一化（DPI / 多屏）—— 架构 v2 §6.9。
//!
//! 职责：把「规范空间」（`NormalizedPoint`：全局逻辑坐标、原点主显示器左上）换算成
//! `SendInput` 真正需要的**物理像素**，并把 DPI / 显示器枚举集中在这里。
//! 换算本身**不重写**：`NormalizedPoint::to_physical`（`crates/platform/api`）是唯一的纯函数实现，
//! 本模块只负责给出**正确的 `CoordinateSpace`**（哪台显示器、什么缩放、什么设备名）。
//! 边界：**不**做多次采样的坐标校准与误差矩阵（→ TASK-040）、**不**做窗口定位（→ `crate::window`）、
//! **不**发任何输入（→ `crate::input`）。
//!
//! ## 为什么纯逻辑与 FFI 分开放
//! 本文件在**所有平台**编译（`cargo clippy --target <非宿主>` 因此能覆盖它 —— ADR-0045 / PL-070 的教训：
//! `#[cfg(windows)]` 整块排掉的文件，本地门禁**结构上**看不到）。真实 DPI / 显示器枚举走下面的
//! `#[cfg(windows)]` 分支；纯函数（DPI → 缩放、显示器命中、构造 `CoordinateSpace`）三平台都编译并单测。
//!
//! ## 不变量
//! 1. **DPI 必须来自真实显示器**（§6.9 规则 3）：禁止把 96 / 1.0 当默认值冒充结果。
//! 2. **缩放必须有限且 > 0**：交给 `CoordinateSpace::new` 校验（0 会让所有点击落到同一个点）。
//! 3. **换算越界必须报错**（铁律 1）：`to_physical` 越界 → `TargetNotFound`，**不**饱和、**不**截断。
//!
//! ## 已知限制（本卡刻意不做）
//! 混合 DPI 的多屏下，本卡按**目标窗口所在显示器**的 DPI 换算（合成输入的目标总是一个窗口）。
//! 「同一动作跨两台不同 DPI 的显示器」的逐段换算归 TASK-040。
//!
//! 相关：架构 v2 §6.2（`resolved.coordinate_space`）/ §6.9、`docs/spec/naming.md` §5。

use assistant_platform_api::{
    CoordinateSpace, CoordinateSpaceKind, ErrorCode, NormalizedPoint, PhysicalPoint, PlatformError,
    PlatformResult,
};

// 本文件在**所有平台**编译（ADR-0045 的非宿主门禁要覆盖它），因此**不能**依赖 `crate::error`
// —— 那个模块只在 Windows 上编译（它要把 `windows` crate 的 HRESULT 翻成 `ErrorCode`）。
// 下面三个私有构造器与 `crate::error` 的同名函数语义一致，只是去掉了平台依赖。

/// 基准 DPI：Windows 把 96 DPI 定义为 100% 缩放（`scale == 1.0`）。
const BASELINE_DPI: u32 = 96;

/// 一台显示器（**物理像素**矩形 + 有效 DPI + 设备名）。
///
/// 为什么用「物理像素 + 有效 DPI」而不是逻辑矩形：`windows` crate 在 Per-Monitor V2
/// 感知进程里返回的就是物理像素（`GetMonitorInfoW` 的 `rcMonitor`），而 `CoordinateSpace`
/// 需要的是「这台显示器的缩放」—— 两者合起来才够 `to_physical` 用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorRecord {
    /// 设备名（Windows 形如 `\\.\DISPLAY1`）—— 进 `CoordinateSpace::origin_display`。
    device_name: String,
    /// 显示器矩形左边界（物理像素）。
    left: i32,
    /// 上边界。
    top: i32,
    /// 右边界（**开区间**）。
    right: i32,
    /// 下边界（**开区间**）。
    bottom: i32,
    /// 有效 DPI（`MDT_EFFECTIVE_DPI`；96 = 100%）。
    effective_dpi: u32,
    /// 是否主显示器。
    is_primary: bool,
}

impl MonitorRecord {
    /// 构造一台显示器的记录（**不做**校验 —— 校验在 `coordinate_space_for_monitor`）。
    ///
    /// `is_primary` 默认 `false`，由 [`MonitorRecord::with_primary`] 设定：主显示器是
    /// **Win32 的 `MONITORINFOF_PRIMARY`**（权威），不是「枚举到的第一台」（顺序不保证）。
    #[must_use]
    pub const fn new(
        device_name: String,
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
        effective_dpi: u32,
    ) -> Self {
        Self {
            device_name,
            left,
            top,
            right,
            bottom,
            effective_dpi,
            is_primary: false,
        }
    }

    /// 设定「是否主显示器」（builder 形式，避免构造函数超过 6 个参数）。
    #[must_use]
    pub const fn with_primary(mut self, is_primary: bool) -> Self {
        self.is_primary = is_primary;
        self
    }

    /// 设备名。
    #[must_use]
    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// 有效 DPI。
    #[must_use]
    pub const fn effective_dpi(&self) -> u32 {
        self.effective_dpi
    }

    /// 是否主显示器。
    #[must_use]
    pub const fn is_primary(&self) -> bool {
        self.is_primary
    }

    /// 左边界（物理像素）。
    #[must_use]
    pub const fn left(&self) -> i32 {
        self.left
    }

    /// 上边界（物理像素）。
    #[must_use]
    pub const fn top(&self) -> i32 {
        self.top
    }

    /// 右边界（物理像素，开区间）。
    #[must_use]
    pub const fn right(&self) -> i32 {
        self.right
    }

    /// 下边界（物理像素，开区间）。
    #[must_use]
    pub const fn bottom(&self) -> i32 {
        self.bottom
    }

    /// 物理点是否落在这台显示器上（**半开区间** `[left, right) × [top, bottom)`）。
    ///
    /// 为什么用半开区间：相邻显示器共享边界像素，闭区间会让边界点同时命中两台 →
    /// 命中不唯一。Windows 保证显示器矩形不重叠，半开区间与它一致。
    #[must_use]
    pub const fn contains_physical(&self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }
}

/// DPI → 缩放系数（纯函数；96 DPI → 1.0）。
///
/// # Errors
/// `dpi == 0` → `ToolInvalidArgs`（0 会让所有点击落到同一个点，是典型的静默失败源）。
pub fn scale_from_dpi(dpi: u32) -> PlatformResult<f64> {
    if dpi == 0 {
        return Err(invalid_args(
            "coordinate scale: DPI must be > 0 (0 would map every point to the same place)",
        ));
    }
    Ok(f64::from(dpi) / f64::from(BASELINE_DPI))
}

/// 由显示器记录构造 `CoordinateSpace`（纯函数）。
///
/// 单位口径固定为 `PhysicalPixels` —— 因为 `SendInput` 收的就是物理像素
/// （架构 v2 §6.9 规则 1：规范空间是逻辑坐标，**合成输入前必须换算**）。
///
/// # Errors
/// DPI 为 0 → `ToolInvalidArgs`；设备名为空 → `ToolInvalidArgs`（`CoordinateSpace::new` 判）。
pub fn coordinate_space_for_monitor(monitor: &MonitorRecord) -> PlatformResult<CoordinateSpace> {
    CoordinateSpace::new(
        CoordinateSpaceKind::PhysicalPixels,
        scale_from_dpi(monitor.effective_dpi())?,
        monitor.device_name().to_string(),
    )
}

/// 按物理点命中显示器下标（纯函数）。
///
/// # Errors
/// 没有任何显示器包含该点 → `TargetNotFound`（**不**取最近的：取最近会在多屏边缘
/// 把点击落到另一台显示器上，属于"点偏了却看起来成功"）。
pub fn monitor_index_containing_physical(
    monitors: &[MonitorRecord],
    x: i32,
    y: i32,
) -> PlatformResult<usize> {
    monitors
        .iter()
        .position(|monitor| monitor.contains_physical(x, y))
        .ok_or_else(|| {
            target_not_found(format!(
                "no display contains physical point ({x}, {y}); known displays: {}",
                monitors.len()
            ))
        })
}

/// 逻辑点 → 物理点，按给定显示器的缩放换算（纯函数）。
///
/// 换算本身走 `NormalizedPoint::to_physical`（`crates/platform/api` 的唯一实现），
/// 本函数只负责把显示器记录变成 `CoordinateSpace` —— 避免两处各写一遍缩放公式。
///
/// # Errors
/// 显示器 DPI / 设备名非法 → `ToolInvalidArgs`；换算结果超出 `i32` → `TargetNotFound`。
pub fn to_physical_on_monitor(
    point: &NormalizedPoint,
    monitor: &MonitorRecord,
) -> PlatformResult<PhysicalPoint> {
    point.to_physical(&coordinate_space_for_monitor(monitor)?)
}

/// 虚拟屏幕（`SendInput` 的 `MOUSEEVENTF_ABSOLUTE` 用的归一化基准）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualScreen {
    /// 左边界（物理像素，可为负 —— 主显示器左边还有屏时）。
    left: i32,
    /// 上边界。
    top: i32,
    /// 宽度（> 0）。
    width: i32,
    /// 高度（> 0）。
    height: i32,
}

impl VirtualScreen {
    /// 构造虚拟屏幕。
    #[must_use]
    pub const fn new(left: i32, top: i32, width: i32, height: i32) -> Self {
        Self {
            left,
            top,
            width,
            height,
        }
    }

    /// 左边界。
    #[must_use]
    pub const fn left(&self) -> i32 {
        self.left
    }

    /// 上边界。
    #[must_use]
    pub const fn top(&self) -> i32 {
        self.top
    }

    /// 宽度。
    #[must_use]
    pub const fn width(&self) -> i32 {
        self.width
    }

    /// 高度。
    #[must_use]
    pub const fn height(&self) -> i32 {
        self.height
    }

    /// 把物理点映射到 `MOUSEEVENTF_ABSOLUTE` 要求的 `0..=65535` 归一化坐标（纯函数）。
    ///
    /// 公式来自 Microsoft Learn（`MOUSEINPUT.dx/dy` + `MOUSEEVENTF_ABSOLUTE`）：
    /// `dx = (x - vs.left) * 65535 / (vs.width - 1)`。用 `i64` 中间量避免乘法溢出。
    ///
    /// # Errors
    /// 虚拟屏幕宽 / 高 ≤ 1 → `ToolInvalidArgs`（分母为 0 或负）；
    /// 点落在虚拟屏幕外 → `TargetNotFound`（**不**夹到边界 —— 夹住会把点击落到别的显示器上）。
    pub fn normalize(&self, point: PhysicalPoint) -> PlatformResult<(i32, i32)> {
        if self.width <= 1 || self.height <= 1 {
            return Err(invalid_args(format!(
                "virtual screen must be at least 2x2 physical pixels, got {}x{}",
                self.width, self.height
            )));
        }
        let x = point.x_px();
        let y = point.y_px();
        if !self.contains(x, y) {
            return Err(target_not_found(format!(
                "physical point ({x}, {y}) is outside the virtual screen \
                 ({}, {} .. {}, {})",
                self.left,
                self.top,
                self.right(),
                self.bottom()
            )));
        }
        let dx = scale_axis(i64::from(x) - i64::from(self.left), self.width)?;
        let dy = scale_axis(i64::from(y) - i64::from(self.top), self.height)?;
        Ok((dx, dy))
    }

    /// 右边界（开区间）。
    #[must_use]
    pub const fn right(&self) -> i32 {
        self.left.saturating_add(self.width)
    }

    /// 下边界（开区间）。
    #[must_use]
    pub const fn bottom(&self) -> i32 {
        self.top.saturating_add(self.height)
    }

    /// 物理点是否落在虚拟屏幕内（半开区间）。
    #[must_use]
    pub const fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right() && y >= self.top && y < self.bottom()
    }
}

/// 按**逻辑点**推导它应该用哪台显示器的缩放（纯函数，三平台可单测）。
///
/// 为什么需要这个：`UiAutomationProvider::pointer_action(pt, act)` 的签名**没有目标窗口**
/// （TASK-016 定死的 trait 形状），而 `pt` 是**全局逻辑坐标** —— 不同显示器可以有不同缩放。
/// 因此这里只能反推，且**反推不唯一时明确报错**（绝不猜 —— 铁律 1）。
///
/// 策略：
/// 1. 全部显示器 DPI 相同（含单显示器）→ 直接用它（无歧义）。
/// 2. 混合 DPI：先用**主显示器**的缩放试算 → 拿物理点命中显示器 → 用命中的缩放**重算一次**；
///    两次命中同一台 → 采用；否则 → 报错。
/// 3. 点不在任何显示器上 → `TargetNotFound`。
///
/// # Errors
/// 显示器列表为空 / 混合 DPI 无法收敛 / 点不在显示器上 → 分别是
/// `CapabilityMissing` / `CapabilityMissing` / `TargetNotFound`（每一种都带诊断信息）。
pub fn coordinate_space_for_logical_point(
    monitors: &[MonitorRecord],
    point: &NormalizedPoint,
) -> PlatformResult<CoordinateSpace> {
    let Some(first) = monitors.first() else {
        return Err(capability_missing(
            "coordinate space: no display was enumerated, cannot convert a logical point",
        ));
    };
    let uniform = monitors
        .iter()
        .all(|monitor| monitor.effective_dpi() == first.effective_dpi());
    if uniform {
        return coordinate_space_for_monitor(first);
    }
    let primary = monitors
        .iter()
        .find(|monitor| monitor.is_primary())
        .unwrap_or(first);
    let primary_space = coordinate_space_for_monitor(primary)?;
    let guess = point.to_physical(&primary_space)?;
    let Some(hit) = monitor_at_physical(monitors, guess.x_px(), guess.y_px()) else {
        return Err(target_not_found(format!(
            "coordinate space: logical point ({}, {}) maps outside every display",
            point.x_logical(),
            point.y_logical()
        )));
    };
    let refined_space = coordinate_space_for_monitor(hit)?;
    let refined = point.to_physical(&refined_space)?;
    let Some(settled) = monitor_at_physical(monitors, refined.x_px(), refined.y_px()) else {
        return Err(target_not_found(format!(
            "coordinate space: logical point ({}, {}) does not settle on any display",
            point.x_logical(),
            point.y_logical()
        )));
    };
    if settled.device_name() != hit.device_name() {
        return Err(capability_missing(format!(
            "coordinate space: displays have different DPI and logical point ({}, {}) cannot be \
             attributed to one of them unambiguously (needs a target-carrying signature; \
             see docs/PARKING_LOT.md PL-074)",
            point.x_logical(),
            point.y_logical()
        )));
    }
    Ok(refined_space)
}

/// 按物理点取命中的显示器（纯函数；命中多台取第一台）。
#[must_use]
pub fn monitor_at_physical(monitors: &[MonitorRecord], x: i32, y: i32) -> Option<&MonitorRecord> {
    monitors
        .iter()
        .find(|monitor| monitor.contains_physical(x, y))
}

/// 单轴归一化：`offset * 65535 / (extent - 1)`，越界 / 非整数结果 → `ToolInvalidArgs`。
fn scale_axis(offset: i64, extent: i32) -> PlatformResult<i32> {
    let denominator = i64::from(extent) - 1;
    if denominator <= 0 {
        return Err(invalid_args(
            "virtual screen extent must be > 1 to normalize an absolute pointer position",
        ));
    }
    let numerator = offset
        .checked_mul(65_535)
        .ok_or_else(|| invalid_args("pointer normalization overflowed"))?;
    let normalized = numerator / denominator;
    i32::try_from(normalized).map_err(|_| {
        invalid_args(format!(
            "normalized pointer coordinate {normalized} does not fit in i32"
        ))
    })
}

/// 构造 `ToolInvalidArgs`（理由见文件头的「为什么本地重复这三个构造器」）。
fn invalid_args(message: impl Into<String>) -> PlatformError {
    PlatformError::new(ErrorCode::ToolInvalidArgs, message)
}

/// 构造 `CapabilityMissing`（本显示器组合做不到，**不是** Fatal）。
fn capability_missing(message: impl Into<String>) -> PlatformError {
    PlatformError::new(ErrorCode::CapabilityMissing, message)
}

/// 构造 `TargetNotFound`（点在当前显示器组合下不可达）。
fn target_not_found(message: impl Into<String>) -> PlatformError {
    PlatformError::new(ErrorCode::TargetNotFound, message)
}

#[cfg(windows)]
mod win32;

/// Windows 侧的真实枚举 / 查询（纯逻辑与 FFI 分开，见文件头）。
#[cfg(windows)]
pub use win32::{
    coordinate_space_for_window, dpi_for_window, enumerate_monitors, monitor_for_physical_point,
    monitor_for_window, virtual_screen,
};

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试夹具：一台显示器（`bounds` = 物理像素矩形 `(left, top, right, bottom)`）。
    fn display(device: &str, bounds: (i32, i32, i32, i32), dpi: u32) -> MonitorRecord {
        let (left, top, right, bottom) = bounds;
        MonitorRecord::new(device.to_string(), left, top, right, bottom, dpi)
    }

    /// 测试夹具：规范点（坐标非法就直接判夹具坏掉，而不是让断言静默通过）。
    fn point(x: f64, y: f64) -> NormalizedPoint {
        match NormalizedPoint::new(x, y) {
            Ok(point) => point,
            Err(failure) => unreachable!("测试夹具必须合法: {failure}"),
        }
    }

    /// 测试夹具：恒等坐标空间（scale = 1.0）—— `PhysicalPoint` 没有公开构造器，
    /// 只能经 `to_physical` 得到，这也顺便证明了「换算是唯一的换算入口」。
    fn identity_space() -> CoordinateSpace {
        match CoordinateSpace::new(CoordinateSpaceKind::PhysicalPixels, 1.0, "test-display") {
            Ok(space) => space,
            Err(failure) => unreachable!("测试夹具必须合法: {failure}"),
        }
    }

    /// 测试夹具：物理点（经恒等空间换算）。
    fn physical(x: f64, y: f64) -> PhysicalPoint {
        match point(x, y).to_physical(&identity_space()) {
            Ok(physical) => physical,
            Err(failure) => unreachable!("测试夹具必须合法: {failure}"),
        }
    }

    #[test]
    fn test_scale_from_dpi_baseline_is_identity() {
        // 96 DPI = Windows 定义的 100%。
        assert_eq!(scale_from_dpi(96).map(f64::to_bits), Ok(1.0_f64.to_bits()));
        // 144 DPI = 150%。
        assert_eq!(scale_from_dpi(144).map(f64::to_bits), Ok(1.5_f64.to_bits()));
    }

    #[test]
    fn test_scale_from_dpi_zero_is_invalid_args_not_one() {
        // 负向用例（ADR-0019 N1）：0 会让所有点击落到同一个点，**不得**退回 1.0 冒充结果。
        let failure = match scale_from_dpi(0) {
            Ok(scale) => unreachable!("0 DPI 必须报错，实际得到 {scale}"),
            Err(failure) => failure,
        };
        assert_eq!(failure.code(), ErrorCode::ToolInvalidArgs);
    }

    #[test]
    fn test_contains_physical_uses_half_open_intervals() {
        let record = display(r"\\.\DISPLAY1", (0, 0, 1920, 1080), 96).with_primary(true);
        assert!(record.contains_physical(0, 0));
        assert!(record.contains_physical(1919, 1079));
        // 右/下边界是**开区间**：相邻显示器共享边界像素时命中必须唯一。
        assert!(!record.contains_physical(1920, 0));
        assert!(!record.contains_physical(0, 1080));
    }

    #[test]
    fn test_virtual_screen_normalize_maps_extremes_to_full_range() {
        let screen = VirtualScreen::new(0, 0, 1920, 1080);
        // 左上角 → (0, 0)；右下角（width-1, height-1）→ (65535, 65535)。
        assert_eq!(screen.normalize(physical(0.0, 0.0)), Ok((0, 0)));
        assert_eq!(
            screen.normalize(physical(1919.0, 1079.0)),
            Ok((65_535, 65_535))
        );
    }

    #[test]
    fn test_virtual_screen_normalize_rejects_points_outside() {
        // 负向用例（ADR-0019 N1）：越界**不夹边界**（夹边界 = 点到错误位置却看起来成功）。
        let screen = VirtualScreen::new(0, 0, 1920, 1080);
        let failure = match screen.normalize(physical(1920.0, 0.0)) {
            Ok(normalized) => unreachable!("越界点必须报错，实际得到 {normalized:?}"),
            Err(failure) => failure,
        };
        assert_eq!(failure.code(), ErrorCode::TargetNotFound);
    }

    #[test]
    fn test_uniform_dpi_uses_the_single_scale() {
        // 单显示器（或全部同 DPI）：无歧义，直接用它的缩放。
        let displays = vec![display(r"\\.\DISPLAY1", (0, 0, 2560, 1440), 144).with_primary(true)];
        let space = match coordinate_space_for_logical_point(&displays, &point(100.0, 50.0)) {
            Ok(space) => space,
            Err(failure) => unreachable!("单显示器必须能换算: {failure}"),
        };
        assert_eq!(space.origin_display(), r"\\.\DISPLAY1");
        assert_eq!(space.scale().to_bits(), 1.5_f64.to_bits());
        assert_eq!(
            point(100.0, 50.0)
                .to_physical(&space)
                .map(|physical| (physical.x_px(), physical.y_px())),
            Ok((150, 75))
        );
    }

    /// 混合 DPI 夹具：副屏（192 DPI）在**左边**、主屏（96 DPI）在右边。
    /// 这个几何是刻意的 —— 它让「先按主屏缩放试算」的迭代**可能**落到另一台显示器上。
    fn mixed_dpi_displays() -> Vec<MonitorRecord> {
        vec![
            display(r"\\.\DISPLAY2", (0, 0, 1280, 720), 192),
            display(r"\\.\DISPLAY1", (1280, 0, 3200, 1080), 96).with_primary(true),
        ]
    }

    #[test]
    fn test_mixed_dpi_point_settles_on_its_own_display() {
        // 逻辑点 (500, 100)：主屏缩放试算命中副屏 → 用副屏缩放（×2）重算仍落在副屏 → 收敛。
        let displays = mixed_dpi_displays();
        let space = match coordinate_space_for_logical_point(&displays, &point(500.0, 100.0)) {
            Ok(space) => space,
            Err(failure) => unreachable!("副屏上的点必须收敛: {failure}"),
        };
        assert_eq!(space.origin_display(), r"\\.\DISPLAY2");
        assert_eq!(space.scale().to_bits(), 2.0_f64.to_bits());
    }

    #[test]
    fn test_mixed_dpi_ambiguous_point_reports_capability_missing() {
        // 负向用例（ADR-0019 N1）：逻辑点 (700, 100) 按主屏缩放命中副屏，但按副屏缩放（×2）
        // 重算落到**主屏** → 无法唯一归属 → **不猜**，明确报错（trait 的 `pointer_action`
        // 不带目标窗口，见 docs/PARKING_LOT.md PL-074）。
        let displays = mixed_dpi_displays();
        let failure = match coordinate_space_for_logical_point(&displays, &point(700.0, 100.0)) {
            Ok(space) => unreachable!("歧义点必须报错，实际得到 {}", space.origin_display()),
            Err(failure) => failure,
        };
        assert_eq!(failure.code(), ErrorCode::CapabilityMissing);
    }

    #[test]
    fn test_no_display_reports_capability_missing() {
        let failure = match coordinate_space_for_logical_point(&[], &point(0.0, 0.0)) {
            Ok(space) => unreachable!("空显示器列表必须报错，实际得到 {}", space.origin_display()),
            Err(failure) => failure,
        };
        assert_eq!(failure.code(), ErrorCode::CapabilityMissing);
    }
}
