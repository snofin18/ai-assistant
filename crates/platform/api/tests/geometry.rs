//! `CoordinateSpace` / `NormalizedPoint` 的校验与换算：正向 + 负向（ADR-0019 N1）。
//!
//! 为什么放在 `tests/`：校验与换算只依赖公开 API（`src/geometry.rs` 不暴露内部状态），
//! 放这里也让 `src/geometry.rs` 保持小。断言风格见 `tests/common/mod.rs`。

mod common;

use assistant_platform_api::{
    CoordinateSpace, CoordinateSpaceKind, ErrorCode, NormalizedPoint, PlatformError,
};
use common::ok_or_fail;

/// 构造一个合法坐标空间（参数固定合法；被拒绝即测试失败）。
fn space(kind: CoordinateSpaceKind, scale: f64) -> CoordinateSpace {
    ok_or_fail(
        CoordinateSpace::new(kind, scale, "DISPLAY1"),
        "测试用的坐标空间必须合法",
    )
}

#[test]
fn test_scale_must_be_positive_and_finite() {
    // 负向：缩放为 0 会让所有点击落到同一个点；NaN / inf 会让换算结果无意义。
    for scale in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let built = CoordinateSpace::new(CoordinateSpaceKind::PhysicalPixels, scale, "d");
        assert_eq!(
            built.err().map(|error| error.code()),
            Some(ErrorCode::ToolInvalidArgs),
            "scale={scale} 必须被拒绝"
        );
    }
}

#[test]
fn test_origin_display_must_not_be_blank() {
    let built = CoordinateSpace::new(CoordinateSpaceKind::LogicalPixels, 1.0, "  ");
    assert_eq!(
        built.err().map(|error| error.code()),
        Some(ErrorCode::ToolInvalidArgs)
    );
}

#[test]
fn test_point_rejects_non_finite() {
    assert!(NormalizedPoint::new(f64::NAN, 0.0).is_err());
    assert!(NormalizedPoint::new(0.0, f64::NEG_INFINITY).is_err());
}

#[test]
fn test_to_physical_applies_scale_and_rounds() {
    let point = ok_or_fail(NormalizedPoint::new(10.0, 20.0), "有限坐标必须可构造");
    let physical = point.to_physical(&space(CoordinateSpaceKind::PhysicalPixels, 1.5));
    assert_eq!(
        physical.map(|pixel| (pixel.x_px(), pixel.y_px())),
        Ok((15, 30))
    );
}

#[test]
fn test_to_physical_rejects_out_of_range_instead_of_saturating() {
    let point = ok_or_fail(NormalizedPoint::new(1.0e9, 0.0), "有限坐标必须可构造");
    let physical = point.to_physical(&space(CoordinateSpaceKind::PhysicalPixels, 1000.0));
    let error = physical.err();
    assert_eq!(
        error.as_ref().map(PlatformError::code),
        Some(ErrorCode::TargetNotFound)
    );
    // 必须是"报错"而不是"饱和成 i32::MAX 还报成功"（铁律 1）。
    assert!(
        error.is_some_and(|error| error.message().contains("out of range")),
        "越界错误必须自带坐标证据"
    );
}

#[test]
fn test_to_physical_accepts_i32_boundaries_exactly() {
    // `exact_i32_from_f64` 的边界：`i32::MIN` / `i32::MAX` 必须**逐位精确**通过
    // （不是"大约通过"），否则窗口边角的点击会静默偏移。
    let identity = space(CoordinateSpaceKind::PhysicalPixels, 1.0);
    let max = ok_or_fail(
        NormalizedPoint::new(f64::from(i32::MAX), 0.0),
        "i32::MAX 必须可构造",
    );
    assert_eq!(
        max.to_physical(&identity).map(|pixel| pixel.x_px()),
        Ok(i32::MAX)
    );
    let min = ok_or_fail(
        NormalizedPoint::new(f64::from(i32::MIN), 0.0),
        "i32::MIN 必须可构造",
    );
    assert_eq!(
        min.to_physical(&identity).map(|pixel| pixel.x_px()),
        Ok(i32::MIN)
    );
}

#[test]
fn test_to_physical_rejects_one_past_i32_boundaries() {
    // 负向：刚好越过一格就必须报错，而不是回绕或饱和。
    let identity = space(CoordinateSpaceKind::PhysicalPixels, 1.0);
    for beyond in [2_147_483_648.0, -2_147_483_649.0, 1.0e300, -1.0e300] {
        let point = ok_or_fail(NormalizedPoint::new(beyond, 0.0), "有限坐标必须可构造");
        assert_eq!(
            point.to_physical(&identity).err().map(|error| error.code()),
            Some(ErrorCode::TargetNotFound),
            "{beyond} 必须被拒绝"
        );
    }
}

#[test]
fn test_to_physical_rounds_half_away_from_zero() {
    let scaled = space(CoordinateSpaceKind::PhysicalPixels, 1.5);
    let point = ok_or_fail(NormalizedPoint::new(1.0, -1.0), "有限坐标必须可构造");
    assert_eq!(
        point
            .to_physical(&scaled)
            .map(|pixel| (pixel.x_px(), pixel.y_px())),
        Ok((2, -2)),
        "1.5 → 2、-1.5 → -2（`f64::round` 的半数远离零）"
    );
}

#[test]
fn test_distance_is_euclidean() {
    let a = ok_or_fail(NormalizedPoint::new(0.0, 0.0), "a 必须可构造");
    let b = ok_or_fail(NormalizedPoint::new(3.0, 4.0), "b 必须可构造");
    assert!((a.distance_to(&b) - 5.0).abs() < 1e-9);
    assert!((b.distance_to(&a) - 5.0).abs() < 1e-9, "距离必须对称");
}

#[test]
fn test_distance_does_not_overflow_for_large_coordinates() {
    // 负向回归：`(dx*dx + dy*dy).sqrt()` 在 1e155 量级先平方溢出成 `inf`，`hypot` 不会。
    // 溢出会让"点选是否命中"变成假命中（铁律 1）。
    let a = ok_or_fail(NormalizedPoint::new(0.0, 0.0), "a 必须可构造");
    let b = ok_or_fail(NormalizedPoint::new(1.0e155, 1.0e155), "b 必须可构造");
    assert!(a.distance_to(&b).is_finite(), "大坐标下的距离必须有限");
}

#[test]
fn test_is_identity_only_for_unscaled_logical() {
    assert!(space(CoordinateSpaceKind::LogicalPixels, 1.0).is_identity());
    assert!(!space(CoordinateSpaceKind::LogicalPixels, 1.25).is_identity());
    assert!(!space(CoordinateSpaceKind::PhysicalPixels, 1.0).is_identity());
}
