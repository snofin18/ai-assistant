//! Shared T3.2 fake and real acceptance helpers.

use super::*;
use assistant_protocol::serde_json::json;

pub fn t3_2_png_header(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = vec![0_u8; 24];
    bytes
        .get_mut(..8)
        .expect("PNG signature slice")
        .copy_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);
    bytes
        .get_mut(8..12)
        .expect("PNG length slice")
        .copy_from_slice(&13_u32.to_be_bytes());
    bytes
        .get_mut(12..16)
        .expect("PNG type slice")
        .copy_from_slice(b"IHDR");
    bytes
        .get_mut(16..20)
        .expect("PNG width slice")
        .copy_from_slice(&width.to_be_bytes());
    bytes
        .get_mut(20..24)
        .expect("PNG height slice")
        .copy_from_slice(&height.to_be_bytes());
    bytes
}

pub fn t3_2_task_config(
    data_root: &Path,
    opened_path: &Path,
    target_path: &Path,
) -> ProductionConfig {
    t3_2_task_config_with_space(data_root, opened_path, target_path, "\\\\.\\DISPLAY1", 1.0)
}

pub fn t3_2_task_config_with_space(
    data_root: &Path,
    opened_path: &Path,
    target_path: &Path,
    monitor_id: &str,
    monitor_scale: f64,
) -> ProductionConfig {
    let adapter_root = workspace_root().join("adapters/com.microsoft.paint");
    let task_package_path = adapter_root
        .join("tasks")
        .join("t3.2.open-png-mark-region-save-as.json");
    let ui_config = UiServerConfig::new(
        "assistant-agent-core-paint-t3-2-test",
        "ASSISTANT_AGENT_CORE_PAINT_T3_2_TEST_TOKEN",
        Duration::from_secs(1),
    )
    .with_allowed_peer("C:\\fixture\\paint-t3-2-peer.exe");
    let inputs = json!({
        "input.opened_path": opened_path.to_string_lossy(),
        "input.target_path": target_path.to_string_lossy(),
        "input.rectangle_start": {"x": 64, "y": 48},
        "input.rectangle_start.x": 64,
        "input.rectangle_start.y": 48,
        "input.rectangle_end": {"x": 1984, "y": 1488},
        "input.rectangle_end.x": 1984,
        "input.rectangle_end.y": 1488,
        "input.foreground_red": 237,
        "input.foreground_green": 28,
        "input.foreground_blue": 36,
        "input.layer_id": "layer-1",
        "input.expected_layer_name": "Layer 1",
        "input.viewport_offset_x_px": 0,
        "input.viewport_offset_y_px": 0,
        "input.coordinate_space": {
            "kind": "physical_pixels",
            "monitor_id": monitor_id,
            "scale": monitor_scale
        },
        "input.tolerance_px": 2
    })
    .as_object()
    .cloned()
    .expect("T3.2 inputs");
    ProductionConfig::new(data_root, adapter_root, task_package_path, ui_config)
        .with_adapter_kind(AdapterKind::Paint)
        .with_task_inputs(inputs)
}

pub fn grant_t3_2_steps<P>(host: &assistant_agent_core::ProductionHost<P>, now_ms: i64)
where
    P: WindowProvider + UiAutomationProvider + Send + Sync + 'static,
{
    for step_id in [
        "open_png",
        "select_rectangle_tool",
        "select_shape_outline",
        "select_shape_fill",
        "select_shape_size",
        "select_foreground_color",
        "select_layer",
        "draw_rectangle",
        "save_as_new_file",
    ] {
        host.approvals()
            .grant(&GrantRequest {
                task_id: host.task_id().as_str(),
                step_id,
                scope: ApprovalScope::Once,
                now_ms,
                ttl_ms: 60_000,
                uses: 1,
            })
            .expect("grant T3.2 step");
    }
}

pub fn read_png_dimensions(path: &Path) -> Result<(u32, u32), std::io::Error> {
    let bytes = std::fs::read(path)?;
    if bytes.len() < 24 || bytes.get(..8) != Some(b"\x89PNG\r\n\x1a\n".as_slice()) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "saved file is not a PNG",
        ));
    }
    let width = bytes
        .get(16..20)
        .and_then(|raw| raw.try_into().ok())
        .map(u32::from_be_bytes)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "missing width"))?;
    let height = bytes
        .get(20..24)
        .and_then(|raw| raw.try_into().ok())
        .map(u32::from_be_bytes)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "missing height"))?;
    Ok((width, height))
}
