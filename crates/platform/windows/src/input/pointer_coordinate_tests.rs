//! TASK-231 / ADR-0067：跨显示器拖拽的两端点坐标空间测试。

use super::*;
use crate::coordinates::{MonitorRecord, coordinate_space_for_monitor};

/// 测试夹具：规范点。
fn normalized(x: f64, y: f64) -> NormalizedPoint {
    match NormalizedPoint::new(x, y) {
        Ok(point) => point,
        Err(failure) => unreachable!("测试夹具必须合法: {failure}"),
    }
}

#[test]
fn test_drag_endpoints_use_each_declared_coordinate_space() {
    // 副屏 200% 在左、主屏 100% 在右。起点逻辑点按副屏 scale 换算，释放点按主屏 scale 换算；
    // 若误用同一个起点空间，终点会从 1500 变成 3000（跨屏拖拽的真实缺陷）。
    let displays = vec![
        MonitorRecord::new(r"\\.\DISPLAY2".to_string(), 0, 0, 1280, 720, 192),
        MonitorRecord::new(r"\\.\DISPLAY1".to_string(), 1280, 0, 3200, 1080, 96).with_primary(true),
    ];
    let Some(secondary) = displays.first() else {
        unreachable!("测试夹具必须有副屏");
    };
    let Some(primary) = displays.get(1) else {
        unreachable!("测试夹具必须有主屏");
    };
    let start_space = match coordinate_space_for_monitor(secondary) {
        Ok(space) => space,
        Err(failure) => unreachable!("测试副屏空间必须合法: {failure}"),
    };
    let drop_space = match coordinate_space_for_monitor(primary) {
        Ok(space) => space,
        Err(failure) => unreachable!("测试主屏空间必须合法: {failure}"),
    };
    let start_logical = normalized(500.0, 100.0);
    let drop_logical = normalized(1500.0, 300.0);
    let action = PointerAction::DragTo {
        drop_at: drop_logical,
        drop_coordinate_space: drop_space,
    };

    let (start, drop) = match physical_points_for_pointer_action(
        &displays,
        &start_space,
        &start_logical,
        &action,
    ) {
        Ok(points) => points,
        Err(failure) => unreachable!("跨屏拖拽必须能换算: {failure}"),
    };
    assert_eq!(start.x_px(), 1000);
    assert_eq!(start.y_px(), 200);
    let Some(drop) = drop else {
        unreachable!("DragTo 必须产出释放点");
    };
    assert_eq!(drop.x_px(), 1500);
    assert_eq!(drop.y_px(), 300);
}
