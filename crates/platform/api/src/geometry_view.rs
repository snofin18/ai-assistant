//! 几何视图：把「矩形 + 它所属的坐标空间」作为**一个值**返回（ADR-0092 细则 3 / ADR-0093 细则 3）。
//!
//! ## 职责
//! 给调用方两样东西，缺一不可：
//! - `bounds`：目标窗口 / 元素的矩形，**设备像素**（与 `ElementBounds` / `element_bounds` 同口径）；
//! - `space`：该矩形所属的 `CoordinateSpace`（`origin_display` = 显示器、`scale` = 该显示器缩放）。
//!
//! ## 为什么是「矩形 + 坐标空间」而不是「矩形 + 显示器 id + scale」
//! `CoordinateSpace` 已经是 `pointer_action` 的入参，且**已经**载有显示器标识与缩放。把三者拆开返回，
//! 调用方还得自己 `CoordinateSpace::new(...)` 拼回去 —— 多一处可写错的地方，也容易与既有
//! `DisplayInfo` 的**有理数**缩放口径打架（ADR-0093 的由来）。
//!
//! ## 边界（不做什么）
//! - 不做单位换算：换算由平台实现在**返回前**做完一次（ADR-0092 细则 3），本层只承载结果。
//! - 不做范围判定：`pointer_action` 前的「点必须落在目标窗口矩形内」是调用方的义务
//!   （ADR-0092 细则 8，越界 → `ToolInvalidArgs`，**禁止 clamp**）。
//! - 不序列化句柄：本层只有值类型（铁律 8）。
//!
//! ## 不变量
//! 1. `space.kind()` 恒为 `CoordinateSpaceKind::PhysicalPixels` —— 与 `bounds` 的单位一致。
//! 2. `space.origin_display()` 必须对应一枚**真实枚举到的显示器**（平台实现负责，猜值即缺陷）。
//! 3. `ElementGeometry::bounds` 必须等于同一元素上的 `element_bounds()`（两处上测试）。
//!
//! 相关：ADR-0092、ADR-0093、ADR-0067（显式坐标空间）、ADR-0085（`element_bounds`）。

use crate::geometry::CoordinateSpace;
use crate::traits::ElementBounds;

/// 窗口的几何视图：设备像素矩形 + 它所属的坐标空间。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct WindowGeometry {
    bounds: ElementBounds,
    space: CoordinateSpace,
}

impl WindowGeometry {
    /// 构造窗口几何视图。
    ///
    /// 调用方（平台实现）负责保证 `space` 的显示器真实存在、且 `scale` 与该显示器一致；
    /// 本构造函数只做值承载，不重复校验（校验发生在平台侧取数时，见 ADR-0092 细则 5）。
    #[must_use]
    pub const fn new(bounds: ElementBounds, space: CoordinateSpace) -> Self {
        Self { bounds, space }
    }

    /// 窗口矩形（设备像素，虚拟桌面坐标系）。
    #[must_use]
    pub const fn bounds(&self) -> &ElementBounds {
        &self.bounds
    }

    /// 该矩形所属的坐标空间（含显示器标识与缩放）。
    #[must_use]
    pub const fn space(&self) -> &CoordinateSpace {
        &self.space
    }
}

/// 元素的几何视图：设备像素矩形 + 它所属的坐标空间。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ElementGeometry {
    bounds: ElementBounds,
    space: CoordinateSpace,
}

impl ElementGeometry {
    /// 构造元素几何视图（与 [`WindowGeometry::new`] 同约定）。
    #[must_use]
    pub const fn new(bounds: ElementBounds, space: CoordinateSpace) -> Self {
        Self { bounds, space }
    }

    /// 元素矩形（设备像素，虚拟桌面坐标系）。
    #[must_use]
    pub const fn bounds(&self) -> &ElementBounds {
        &self.bounds
    }

    /// 该矩形所属的坐标空间（含显示器标识与缩放）。
    #[must_use]
    pub const fn space(&self) -> &CoordinateSpace {
        &self.space
    }
}
