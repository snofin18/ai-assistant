//! TASK-045 real-machine Paint T3.2 acceptance.

use super::paint_real_run::*;
use super::paint_real_run_t3_2_support::*;
use super::*;

/// TASK-045 real-machine acceptance. The test is opt-in and requires a human
/// to keep Paint foregrounded before the first synthetic input.
#[tokio::test]
#[ignore = "TASK-045: requires an interactive Paint 11.2605.81.0 window and human observer"]
async fn test_paint_t3_2_open_png_mark_region_save_as() -> Result<(), Box<dyn std::error::Error>> {
    assistant_platform_windows::enable_per_monitor_v2_dpi_awareness()?;
    let directory = TestDirectory::new("production-paint-real-t3-2-open-save")?;
    let source = directory.path.join("source.png");
    let target = directory.path.join("marked-output.png");
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
        t3_2_task_config_with_space(
            &directory.path,
            &source,
            &target,
            monitor_id.as_str(),
            monitor_scale,
        ),
        platform,
        Arc::new(SystemClock),
    )
    .await?;
    assert!(sink_handle.attach(&host.database_handle()));

    let now_ms = SystemClock.now_unix_ms();
    grant_t3_2_steps(&host, now_ms);
    let plan = host.plan_task()?;
    let foreground_probe = WindowsPlatform::new();
    require_real_paint_foreground(&foreground_probe, &paint_window).await?;
    let run = match host.execute_plan(plan, now_ms).await {
        Ok(run) => run,
        Err(error) => {
            if target.is_file() {
                let evidence =
                    workspace_root().join("target/paint-t3-2-ten-run/failed-save-target.bin");
                std::fs::copy(&target, &evidence)?;
            }
            return Err(error.into());
        }
    };
    assert_eq!(run.final_snapshot.status, TaskStatus::Completed);
    assert!(target.is_file());
    let saved_dimensions = read_png_dimensions(&target)?;
    assert_eq!(saved_dimensions, (2048, 1536));
    host.shutdown().await?;
    Ok(())
}
