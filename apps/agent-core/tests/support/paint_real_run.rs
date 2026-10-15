//! Real-machine Paint acceptance (opt-in) plus the foreground-settling guards around it.
//!
//! Included by `production_paint.rs` via `#[path]`; it reuses that test binary's private helpers
//! (`paint_config`, `TestDirectory`, `workspace_root`, and the shared imports) through `super::*`.
//! Kept in its own file so `production_paint.rs` stays under the 900-line hygiene hard limit.

use super::*;

/// Quiet window between "Paint became the foreground window" and the first synthetic input.
///
/// The human brings Paint forward with the mouse; their pointer (and the click that raised the
/// window) can still be settling when the test would otherwise start driving the app. Synthetic
/// pointer input and a human's mouse then fight over the same cursor, so give the desktop a fixed
/// quiet window and re-assert the foreground before sending anything. Kept as a constant so the
/// wait is visible and tunable.
const REAL_RUN_FOREGROUND_SETTLE: Duration = Duration::from_secs(2);

/// Fails unless `window` is currently the foreground window.
///
/// Fail-closed on purpose: sending synthetic input to whatever happens to be foreground is exactly
/// the "focus 错位会打到别的应用上" hazard the platform contract forbids, so a lost foreground
/// aborts the run instead of driving the wrong window.
async fn require_real_paint_foreground(
    platform: &WindowsPlatform,
    window: &ResolvedWindow,
) -> Result<(), Box<dyn std::error::Error>> {
    if !platform.window_state(window).await?.foreground() {
        return Err("Paint is not the foreground window; refusing to send synthetic input".into());
    }
    Ok(())
}

/// Waits `settle`, then requires `window` to still be the foreground window.
async fn settle_then_require_foreground(
    platform: &WindowsPlatform,
    window: &ResolvedWindow,
    settle: Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    tokio::time::sleep(settle).await;
    require_real_paint_foreground(platform, window).await
}

async fn wait_for_real_paint_foreground(
    platform: &WindowsPlatform,
) -> Result<ResolvedWindow, Box<dyn std::error::Error>> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let windows = platform.list_windows(&WindowFilter::any()).await?;
        if let Some(info) = windows
            .into_iter()
            .find(|info| info.app_id().to_ascii_lowercase().contains("mspaint"))
            && platform.window_state(info.window()).await?.foreground()
        {
            return Ok(info.window().clone());
        }
        if Instant::now() >= deadline {
            return Err("timed out waiting for a human to bring Paint to the foreground".into());
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

/// Real-machine acceptance is intentionally opt-in: it needs an interactive Paint window and
/// a human observer. The test uses the same adapter, task package, approvals, and production
/// assembly as the fake acceptance; no result is fabricated when Paint is absent.
#[tokio::test]
#[ignore = "TASK-256: requires an interactive Paint 11.2605.81.0 window and human observer"]
async fn test_paint_t3_1_runs_on_real_paint() -> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("production-paint-real-t3-1")?;
    let blob_sink = StorageBlobSink::new();
    let sink_handle = blob_sink.clone();
    let platform = WindowsPlatform::new().with_blob_sink(blob_sink.leak());
    let paint_window = wait_for_real_paint_foreground(&platform).await?;
    // Let the human's mouse settle after they raised Paint, then re-check the foreground before the
    // plan is allowed to send any synthetic pointer input.
    settle_then_require_foreground(&platform, &paint_window, REAL_RUN_FOREGROUND_SETTLE).await?;
    let host = assemble_production_host(
        paint_config(&directory.path),
        platform,
        Arc::new(SystemClock),
    )
    .await?;
    assert!(sink_handle.attach(&host.database_handle()));

    let now_ms = SystemClock.now_unix_ms();
    let approvals = host.approvals();
    for step_id in [
        "new_document",
        "select_rectangle_tool",
        "select_foreground_color",
        "select_layer",
        "draw_rectangle",
    ] {
        approvals.grant(&GrantRequest {
            task_id: host.task_id().as_str(),
            step_id,
            scope: ApprovalScope::Once,
            now_ms,
            ttl_ms: 60_000,
            uses: 1,
        })?;
    }

    let plan = host.plan_task()?;
    assert_eq!(plan.steps.len(), 10);
    // Assembling the host and granting approvals take time, and the mouse-driven foreground check
    // is cheap: re-assert it right before the first input so a distracted human can never have the
    // run drive the wrong window.
    require_real_paint_foreground(&platform, &paint_window).await?;
    let run = host.execute_plan(plan, now_ms).await?;
    assert_eq!(run.final_snapshot.status, TaskStatus::Completed);
    host.shutdown().await?;
    Ok(())
}
