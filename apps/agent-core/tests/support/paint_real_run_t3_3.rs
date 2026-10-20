//! Paint T3.3 draw / undo / pixel-snapshot acceptance.

use super::paint_real_run::*;
use super::paint_real_run_fake::T3_2FakePlatform;
use super::paint_real_run_t3_2_support::t3_2_png_header;
use super::*;
use assistant_protocol::serde_json::json;

fn t3_3_task_config(
    data_root: &Path,
    opened_path: &Path,
    monitor_id: &str,
    monitor_scale: f64,
) -> ProductionConfig {
    let adapter_root = workspace_root().join("adapters/com.microsoft.paint");
    let task_package_path = adapter_root
        .join("tasks")
        .join("t3.3.open-png-draw-rollback.json");
    let ui_config = UiServerConfig::new(
        "assistant-agent-core-paint-t3-3-test",
        "ASSISTANT_AGENT_CORE_PAINT_T3_3_TEST_TOKEN",
        Duration::from_secs(1),
    )
    .with_allowed_peer("C:\\fixture\\paint-t3-3-peer.exe");
    let inputs = json!({
        "input.opened_path": opened_path.to_string_lossy(),
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
        "input.expected_undo_steps": 1
    })
    .as_object()
    .cloned()
    .expect("T3.3 inputs");
    ProductionConfig::new(data_root, adapter_root, task_package_path, ui_config)
        .with_adapter_kind(AdapterKind::Paint)
        .with_task_inputs(inputs)
}

fn grant_t3_3_steps<P>(host: &assistant_agent_core::ProductionHost<P>, now_ms: i64)
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
        "rollback_last_write",
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
            .expect("grant T3.3 step");
    }
}

#[tokio::test]
async fn test_paint_t3_3_draw_undo_fake_positive() -> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("production-paint-t3-3-positive")?;
    let source = directory.path.join("source.png");
    let target = directory.path.join("unused.png");
    let source_bytes = t3_2_png_header(2048, 1536);
    std::fs::write(&source, &source_bytes)?;
    let sink = StorageBlobSink::new();
    let platform = T3_2FakePlatform::new(
        source.clone(),
        target,
        source_bytes,
        2048,
        1536,
        Some(Arc::new(sink.clone())),
    );
    let host = assemble_production_host(
        t3_3_task_config(&directory.path, &source, "\\\\.\\DISPLAY1", 1.0),
        platform.clone(),
        Arc::new(FixedClock),
    )
    .await?;
    assert!(sink.attach(&host.database_handle()));
    grant_t3_3_steps(&host, FIXED_NOW_MS);
    let plan = host.plan_task()?;
    let run = host.execute_plan(plan, FIXED_NOW_MS).await?;
    assert_eq!(run.final_snapshot.status, TaskStatus::Completed);
    assert!(
        !platform.is_drawn(),
        "rollback must restore the pre-write pixels"
    );
    host.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn test_paint_t3_3_noop_undo_fails_closed() -> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("production-paint-t3-3-noop")?;
    let source = directory.path.join("source.png");
    let target = directory.path.join("unused.png");
    let source_bytes = t3_2_png_header(2048, 1536);
    std::fs::write(&source, &source_bytes)?;
    let sink = StorageBlobSink::new();
    let platform = T3_2FakePlatform::new(
        source.clone(),
        target,
        source_bytes,
        2048,
        1536,
        Some(Arc::new(sink.clone())),
    )
    .with_ignored_undo();
    let host = assemble_production_host(
        t3_3_task_config(&directory.path, &source, "\\\\.\\DISPLAY1", 1.0),
        platform.clone(),
        Arc::new(FixedClock),
    )
    .await?;
    assert!(sink.attach(&host.database_handle()));
    grant_t3_3_steps(&host, FIXED_NOW_MS);
    let plan = host.plan_task()?;
    let Err(error) = host.execute_plan(plan, FIXED_NOW_MS).await else {
        return Err("no-op undo must fail closed".into());
    };
    assert!(
        error
            .to_string()
            .contains("Ctrl+Z produced no pixel change"),
        "unexpected no-op undo error: {error}"
    );
    assert!(platform.is_drawn());
    host.shutdown().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "TASK-046: requires an interactive Paint 11.2605.81.0 window and human observer"]
async fn test_paint_t3_3_draw_undo_pixel_snapshot_verify() -> Result<(), Box<dyn std::error::Error>>
{
    assistant_platform_windows::enable_per_monitor_v2_dpi_awareness()?;
    let directory = TestDirectory::new("production-paint-real-t3-3")?;
    let source = directory.path.join("source.png");
    let prepare = workspace_root().join("eval/tasks/paint/t3.2/prepare_fixture.py");
    let fixture = std::process::Command::new("python")
        .arg(&prepare)
        .arg("--output")
        .arg(&source)
        .current_dir(workspace_root())
        .output()?;
    if !fixture.status.success() {
        return Err(format!(
            "fixture generator failed: {}",
            String::from_utf8_lossy(&fixture.stderr)
        )
        .into());
    }
    let blob_sink = StorageBlobSink::new();
    let sink_handle = blob_sink.clone();
    let platform = WindowsPlatform::new().with_blob_sink(blob_sink.leak());
    let paint_window = wait_for_real_paint_foreground(&platform).await?;
    settle_then_require_foreground(&platform, &paint_window, REAL_RUN_FOREGROUND_SETTLE).await?;
    assert_capture_size_matches_window(&platform, &paint_window).await?;
    let window_geometry = platform.window_geometry(&paint_window).await?;
    let monitor_id = window_geometry.space().origin_display().to_owned();
    let monitor_scale = window_geometry.space().scale();
    let host = assemble_production_host(
        t3_3_task_config(&directory.path, &source, monitor_id.as_str(), monitor_scale),
        platform,
        Arc::new(SystemClock),
    )
    .await?;
    assert!(sink_handle.attach(&host.database_handle()));
    let now_ms = SystemClock.now_unix_ms();
    grant_t3_3_steps(&host, now_ms);
    let plan = host.plan_task()?;
    let foreground_probe = WindowsPlatform::new();
    require_real_paint_foreground(&foreground_probe, &paint_window).await?;
    let run = host.execute_plan(plan, now_ms).await?;
    assert_eq!(run.final_snapshot.status, TaskStatus::Completed);
    host.shutdown().await?;
    Ok(())
}
