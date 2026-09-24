//! 坐标空间与归一化点（架构 v2 §6.9：v1 完全缺失的一节）。
//!
//! 职责：给"走合成输入"的路径一个**唯一的规范空间**，并把换算集中在这里。
//! 边界：不做 DPI 探测、不枚举显示器 —— 那些是平台实现的事（TASK-017 / 018 / 040）。
//!
//! ## 不变量
//! 1. **内部统一 = 全局逻辑坐标、原点主显示器左上**（架构 v2 §6.9 规则 1）。
//!    平台侧拿到的物理像素必须先经 `to_physical` 换算，**不允许**各平台自己约定。
//! 2. 所有构造都拒绝 NaN / ±∞（"点偏"是静默失败，必须当场报错）。
//! 3. 换算是**纯函数**：同样的 `(point, space)` 必得同样的结果。
//!
//! 相关：架构 v2 §6.2（`resolved.coordinate_space`）/ §6.9、`docs/spec/naming.md` §5。

use serde::{Deserialize, Serialize};

use crate::ErrorCode;
use crate::error::{PlatformError, PlatformResult};

/// 坐标空间的单位口径。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CoordinateSpaceKind {
    /// 物理像素（Windows 在 Per-Monitor V2 感知模式下给出的坐标）。
    PhysicalPixels,
    /// 逻辑像素（DPI 缩放**之前**的坐标）。
    LogicalPixels,
}

/// 某个显示器组合下的坐标空间描述。
///
/// 对应架构 v2 §6.2 的 `resolved.coordinate_space`：`{ kind, scale, origin_monitor }`。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CoordinateSpace {
    /// 单位口径。
    kind: CoordinateSpaceKind,
    /// 该显示器组合的缩放系数（1.0 = 无缩放）。
    scale: f64,
    /// 原点所在显示器的标识（Windows 形如 `\\.\DISPLAY1`）。
    origin_display: String,
}

impl CoordinateSpace {
    /// 构造坐标空间描述。
    ///
    /// # Errors
    /// - `scale` 非有限或 ≤ 0 → `ToolInvalidArgs`（缩放为 0 会让所有点击落到同一个点）
    /// - `origin_display` 为空 → `ToolInvalidArgs`
    pub fn new(
        kind: CoordinateSpaceKind,
        scale: f64,
        origin_display: impl Into<String>,
    ) -> PlatformResult<Self> {
        if !scale.is_finite() || scale <= 0.0 {
            return Err(PlatformError::new(
                ErrorCode::ToolInvalidArgs,
                format!("coordinate scale must be finite and > 0, got {scale}"),
            ));
        }
        let origin_display = origin_display.into();
        if origin_display.trim().is_empty() {
            return Err(PlatformError::new(
                ErrorCode::ToolInvalidArgs,
                "origin_display must not be empty",
            ));
        }
        Ok(Self {
            kind,
            scale,
            origin_display,
        })
    }

    /// 单位口径。
    #[must_use]
    pub const fn kind(&self) -> CoordinateSpaceKind {
        self.kind
    }

    /// 缩放系数。
    #[must_use]
    pub const fn scale(&self) -> f64 {
        self.scale
    }

    /// 原点显示器标识。
    #[must_use]
    pub fn origin_display(&self) -> &str {
        &self.origin_display
    }

    /// 是否是恒等空间（逻辑像素 + 无缩放）—— 恒等时可以跳过乘法，但**不允许**跳过校准（§6.9 规则 3）。
    #[must_use]
    pub fn is_identity(&self) -> bool {
        self.kind == CoordinateSpaceKind::LogicalPixels && (self.scale - 1.0).abs() < f64::EPSILON
    }
}

/// 规范空间中的点（**全局逻辑坐标、原点主显示器左上**）。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct NormalizedPoint {
    /// 逻辑坐标 X。
    x_logical: f64,
    /// 逻辑坐标 Y。
    y_logical: f64,
}

impl NormalizedPoint {
    /// 构造一个规范点。
    ///
    /// # Errors
    /// 任一坐标为 NaN / ±∞ → `ToolInvalidArgs`。
    pub fn new(x_logical: f64, y_logical: f64) -> PlatformResult<Self> {
        if !x_logical.is_finite() || !y_logical.is_finite() {
            return Err(PlatformError::new(
                ErrorCode::ToolInvalidArgs,
                format!("normalized point must be finite, got ({x_logical}, {y_logical})"),
            ));
        }
        Ok(Self {
            x_logical,
            y_logical,
        })
    }

    /// 逻辑坐标 X。
    #[must_use]
    pub const fn x_logical(&self) -> f64 {
        self.x_logical
    }

    /// 逻辑坐标 Y。
    #[must_use]
    pub const fn y_logical(&self) -> f64 {
        self.y_logical
    }

    /// 换算到物理像素（架构 v2 §6.9 规则 3：合成输入**前**必须换算）。
    ///
    /// 为什么四舍五入：物理像素是整数，直接截断会让 1.5 → 1（系统性偏左上）。
    /// 为什么用 `i32::try_from` 而不是 `as`：`as` 转换在越界时**饱和**（`1e9 as i32` = `i32::MAX`）
    /// 且不报错 —— 那正是"点偏了却看起来成功"的静默失败形态（铁律 1）。
    ///
    /// # Errors
    /// 换算结果超出 `i32` 可表示范围（或为 NaN）→ `TargetNotFound`
    /// （该点在当前显示器组合下不可达）。
    pub fn to_physical(&self, space: &CoordinateSpace) -> PlatformResult<PhysicalPoint> {
        let x_rounded = (self.x_logical * space.scale).round();
        let y_rounded = (self.y_logical * space.scale).round();
        let Some(x_px) = exact_i32_from_f64(x_rounded) else {
            return Err(Self::out_of_range_error(x_rounded, y_rounded));
        };
        let Some(y_px) = exact_i32_from_f64(y_rounded) else {
            return Err(Self::out_of_range_error(x_rounded, y_rounded));
        };
        Ok(PhysicalPoint { x_px, y_px })
    }

    /// 越界错误（两个坐标一起报，便于定位是哪个显示器组合出的问题）。
    ///
    /// 为什么不用 `From`/`TryFrom` 的默认错误：默认错误不含坐标，而排查"点到哪去了"
    /// 全靠这两个数；错误信息必须自带证据（铁律 1）。
    fn out_of_range_error(x_rounded: f64, y_rounded: f64) -> PlatformError {
        PlatformError::new(
            ErrorCode::TargetNotFound,
            format!("physical point out of range: ({x_rounded}, {y_rounded})"),
        )
    }

    /// 到另一点的欧氏距离（逻辑坐标单位）。
    #[must_use]
    pub fn distance_to(&self, other: &Self) -> f64 {
        // 用 `f64::hypot` 而不是 `(dx*dx + dy*dy).sqrt()`：后者在 dx/dy 量级很大时会**溢出**
        // 成 `inf`（平方先溢出），hypot 不会 —— 距离是给"点选是否命中"用的，溢出会变成假命中。
        (self.x_logical - other.x_logical).hypot(self.y_logical - other.y_logical)
    }
}

/// 把**整数性** `f64` 精确转成 `i32`（越界 / 非整数 / NaN / ±∞ → `None`）。
///
/// 为什么不用 `as`（这是本 crate 里唯一的"数值转换"决策，故写全理由）：
/// 1. `as` 在越界时**饱和**（`1e9 as i32 == i32::MAX`）且不报错 —— 那正是"点偏了却看起来
///    成功"的静默失败形态（铁律 1）；本函数对越界返回 `None`，由调用方转成 `TargetNotFound`。
/// 2. 本 workspace 把 `clippy::cast_possible_truncation`（pedantic）当 deny，而 TASK-016 的
///    Out of scope 禁止 `#[allow]` 放宽；标准库也**没有** `TryFrom<f64> for i32`
///    （只有 `as` 与 `unsafe` 的 `to_int_unchecked`，而 `unsafe` 同样被本卡禁止）。
///    因此这里改用**整数域的逐位合成**：只用 `f64::from(u32)`（精确、无舍入）+ 比较 + 减法。
///
/// 正确性依据：`|值| ≤ 2^31` 时每个中间量都是 ≤ 2^32 的整数，而 `f64` 对 ≤ 2^53 的整数是
/// **精确**的（52 位尾数），所以比较与减法都不引入误差。32 轮之后 `remaining` 必为 `0`
/// （输入是整数且在范围内）或非 `0`（越界 / 非整数）。
fn exact_i32_from_f64(value: f64) -> Option<i32> {
    let magnitude = value.abs();
    if !magnitude.is_finite() {
        return None;
    }
    let mut remaining = magnitude;
    let mut assembled: u32 = 0;
    let mut weight: u32 = 1 << 31;
    while weight != 0 {
        let step = f64::from(weight);
        if remaining >= step {
            remaining -= step;
            assembled |= weight;
        }
        weight >>= 1;
    }
    if remaining != 0.0 {
        // 还有剩余 = 输入超出 2^32-1 或不是整数（后者只可能来自调用方的 bug）。
        return None;
    }
    if value < 0.0 {
        // 负数里只有 `i32::MIN`（-2^31）能取到 2^31；其余负数都在 `i32` 范围内。
        if assembled == 1 << 31 {
            return Some(i32::MIN);
        }
        return i32::try_from(assembled).ok().map(|magnitude| -magnitude);
    }
    i32::try_from(assembled).ok()
}

/// 物理像素点（平台实现真正传给 `SendInput` / `CGEvent` 的形态）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PhysicalPoint {
    /// 物理像素 X。
    x_px: i32,
    /// 物理像素 Y。
    y_px: i32,
}

impl PhysicalPoint {
    /// 物理像素 X。
    #[must_use]
    pub const fn x_px(&self) -> i32 {
        self.x_px
    }

    /// 物理像素 Y。
    #[must_use]
    pub const fn y_px(&self) -> i32 {
        self.y_px
    }
}
