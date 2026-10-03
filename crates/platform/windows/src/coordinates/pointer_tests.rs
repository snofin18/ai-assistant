//! TASK-231 / ADR-0067：显式坐标空间的混合 DPI 与负向测试。

use super::*;

/// 测试夹具：一台显示器。
fn display(device: &str, bounds: (i32, i32, i32, i32), dpi: u32) -> MonitorRecord {
    let (left, top, right, bottom) = bounds;
    MonitorRecord::new(device.to_string(), left, top, right, bottom, dpi)
}

/// 测试夹具：规范点。
fn point(x: f64, y: f64) -> NormalizedPoint {
    match NormalizedPoint::new(x, y) {
        Ok(point) => point,
        Err(failure) => unreachable!("测试夹具必须合法: {failure}"),
    }
}

/// 测试夹具：显示器对应的显式坐标空间。
fn space_for(display: &MonitorRecord) -> CoordinateSpace {
    match coordinate_space_for_monitor(display) {
        Ok(space) => space,
        Err(failure) => unreachable!("测试显示器必须能构造坐标空间: {failure}"),
    }
}

/// 混合 DPI 夹具：副屏（192 DPI）在左边、主屏（96 DPI）在右边。
fn mixed_dpi_displays() -> Vec<MonitorRecord> {
    vec![
        display(r"\\.\DISPLAY2", (0, 0, 1280, 720), 192),
        display(r"\\.\DISPLAY1", (1280, 0, 3200, 1080), 96).with_primary(true),
    ]
}

#[test]
fn test_explicit_space_resolves_previously_ambiguous_point() {
    // 旧启发式对 (700, 100) 主屏试算与副屏重算会互相矛盾并报 CapabilityMissing。
    // ADR-0067 后，调用方选哪台显示器的空间，换算就是哪个结果；两个结果都落在真实显示器上。
    let displays = mixed_dpi_displays();
    let Some(secondary) = displays.first() else {
        unreachable!("混合 DPI 夹具必须至少有一台显示器");
    };
    let Some(primary) = displays.get(1) else {
        unreachable!("混合 DPI 夹具必须有第二台显示器");
    };
    let secondary_space = space_for(secondary);
    let primary_space = space_for(primary);

    assert_eq!(
        physical_point_for_coordinate_space(&displays, &secondary_space, &point(700.0, 100.0))
            .map(|physical| (physical.x_px(), physical.y_px())),
        Ok((1400, 200))
    );
    assert_eq!(
        physical_point_for_coordinate_space(&displays, &primary_space, &point(700.0, 100.0))
            .map(|physical| (physical.x_px(), physical.y_px())),
        Ok((700, 100))
    );
}

#[test]
fn test_unknown_explicit_display_is_target_not_found() {
    // 负向用例（ADR-0019 N1）：显式空间可以消除歧义，但不能凭空发明一台显示器。
    let displays = mixed_dpi_displays();
    let missing =
        match CoordinateSpace::new(CoordinateSpaceKind::PhysicalPixels, 2.0, r"\\.\DISPLAY9") {
            Ok(space) => space,
            Err(failure) => unreachable!("测试坐标空间必须合法: {failure}"),
        };
    let failure =
        match physical_point_for_coordinate_space(&displays, &missing, &point(100.0, 100.0)) {
            Ok(physical) => unreachable!(
                "未知显示器必须报错，实际得到 ({}, {})",
                physical.x_px(),
                physical.y_px()
            ),
            Err(failure) => failure,
        };
    assert_eq!(failure.code(), ErrorCode::TargetNotFound);
}

#[test]
fn test_explicit_space_with_mismatched_scale_is_invalid_args() {
    // 负向用例：设备名对了但 scale 与真实 DPI 不一致 → 不采用这个“看起来能算”的空间。
    let displays = mixed_dpi_displays();
    let Some(secondary) = displays.first() else {
        unreachable!("混合 DPI 夹具必须至少有一台显示器");
    };
    let mismatched = match CoordinateSpace::new(
        CoordinateSpaceKind::PhysicalPixels,
        1.0,
        secondary.device_name().to_string(),
    ) {
        Ok(space) => space,
        Err(failure) => unreachable!("测试坐标空间必须合法: {failure}"),
    };
    let failure =
        match physical_point_for_coordinate_space(&displays, &mismatched, &point(100.0, 100.0)) {
            Ok(physical) => unreachable!(
                "scale 不一致必须报错，实际得到 ({}, {})",
                physical.x_px(),
                physical.y_px()
            ),
            Err(failure) => failure,
        };
    assert_eq!(failure.code(), ErrorCode::ToolInvalidArgs);
}

#[test]
fn test_explicit_space_requires_physical_pixels() {
    // 负向用例：LogicalPixels 语义下再乘 scale 会把逻辑坐标当物理坐标用错。
    let displays = mixed_dpi_displays();
    let logical =
        match CoordinateSpace::new(CoordinateSpaceKind::LogicalPixels, 1.0, r"\\.\DISPLAY1") {
            Ok(space) => space,
            Err(failure) => unreachable!("测试坐标空间必须合法: {failure}"),
        };
    let failure =
        match physical_point_for_coordinate_space(&displays, &logical, &point(100.0, 100.0)) {
            Ok(physical) => unreachable!(
                "LogicalPixels 必须被拒绝，实际得到 ({}, {})",
                physical.x_px(),
                physical.y_px()
            ),
            Err(failure) => failure,
        };
    assert_eq!(failure.code(), ErrorCode::ToolInvalidArgs);
}

#[test]
fn test_explicit_space_outside_all_displays_is_target_not_found() {
    // 负向用例：显式空间决定 scale，但不会把越界点静默夹到屏幕内。
    let displays = mixed_dpi_displays();
    let Some(secondary) = displays.first() else {
        unreachable!("混合 DPI 夹具必须至少有一台显示器");
    };
    let failure = match physical_point_for_coordinate_space(
        &displays,
        &space_for(secondary),
        &point(10_000.0, 10_000.0),
    ) {
        Ok(physical) => unreachable!(
            "远点必须报错，实际得到 ({}, {})",
            physical.x_px(),
            physical.y_px()
        ),
        Err(failure) => failure,
    };
    assert_eq!(failure.code(), ErrorCode::TargetNotFound);
}

#[test]
fn test_no_display_reports_capability_missing() {
    let identity = match CoordinateSpace::new(CoordinateSpaceKind::PhysicalPixels, 1.0, "test") {
        Ok(space) => space,
        Err(failure) => unreachable!("测试坐标空间必须合法: {failure}"),
    };
    let failure = match physical_point_for_coordinate_space(&[], &identity, &point(0.0, 0.0)) {
        Ok(physical) => unreachable!(
            "空显示器列表必须报错，实际得到 ({}, {})",
            physical.x_px(),
            physical.y_px()
        ),
        Err(failure) => failure,
    };
    assert_eq!(failure.code(), ErrorCode::CapabilityMissing);
}
