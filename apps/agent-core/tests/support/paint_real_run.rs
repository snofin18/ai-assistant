//! Real-machine Paint acceptance (opt-in) plus the foreground-settling guards around it.
//!
//! Included by `production_paint.rs` via `#[path]`; it reuses that test binary's private helpers
//! (`paint_config`, `TestDirectory`, `workspace_root`, and the shared imports) through `super::*`.
//! Kept in its own file so `production_paint.rs` stays under the 900-line hygiene hard limit.

use super::*;

use assistant_platform_api::{SelectorCandidate, SelectorKind, SelectorValue};

/// Read the live document's canvas parameters.
///
/// The T3.1 task package declares `canvas_width` / `canvas_height` / `zoom_ratio` as **caller
/// inputs** and states the precondition "zoom ratio and viewport offsets match the canvas state at
/// resolve time". The fixture constants (100x100 at zoom 1.0) make the mapped end point fall
/// outside the real render (`paint.canvas.resolve_point` then fails closed with `InvalidArguments`),
/// so the real run must inject what it actually measured:
///
/// - rendered width from the `canvas` element (`automation_id = image`);
/// - document pixel size parsed from `CanvasSizeTextBlock` (e.g. `418 x 74像素`);
/// - `zoom_ratio = rendered_width / document_width`.
///
/// Viewport offsets stay 0: a freshly created document is not scrolled. The declared rectangle
/// points (10,10)-(50,50) are kept, so a canvas smaller than 51 px on either axis fails loudly here
/// instead of silently drawing at a clamped point.
async fn read_real_canvas_inputs(
    platform: &WindowsPlatform,
    window: &ResolvedWindow,
) -> Result<PaintCanvasInputs, Box<dyn std::error::Error>> {
    let canvas_chain = assisted_selector_chain("canvas-aid", "image");
    let canvas = platform.resolve_element(window, &canvas_chain).await?;
    let geometry = platform.element_geometry(&canvas).await?;
    let render_width = geometry.bounds().width();
    if render_width <= 0 {
        return Err("Paint canvas has a non-positive rendered width".into());
    }

    let size_chain = assisted_selector_chain("canvas-size-aid", "CanvasSizeTextBlock");
    let size_element = platform.resolve_element(window, &size_chain).await?;
    let text = platform.read_text(&size_element).await?;
    let (width_px, height_px) = parse_canvas_pixel_size(&text).ok_or_else(|| {
        format!("CanvasSizeTextBlock did not expose two positive integers: {text:?}")
    })?;

    if width_px <= 50 || height_px <= 50 {
        return Err(format!(
            "Paint canvas {width_px}x{height_px} is too small for the declared rectangle points \
             (10,10)-(50,50)"
        )
        .into());
    }

    let zoom_ratio = f64::from(render_width) / f64::from(width_px);
    // The synthetic-input coordinate space must be measured too: on this 200% display the real
    // scale is 2, and a hard-coded 1.0 is rejected by `pointer_action`'s explicit-space check
    // (ADR-0067) — which is exactly the fail-closed behaviour we want.
    let monitor_id: &'static str = Box::leak(
        geometry
            .space()
            .origin_display()
            .to_owned()
            .into_boxed_str(),
    );
    Ok(PaintCanvasInputs {
        width_px,
        height_px,
        zoom_ratio,
        monitor_id,
        monitor_scale: geometry.space().scale(),
    })
}

/// Single-candidate automation-id chain (same shape the adapter declares for these targets).
fn assisted_selector_chain(id: &str, automation_id: &str) -> SelectorChain {
    SelectorChain::new(vec![
        SelectorCandidate::new(
            id,
            SelectorKind::AutomationId,
            SelectorValue::Text(automation_id.to_owned()),
            1.0,
            false,
        )
        .expect("automation-id candidate"),
    ])
}

/// `"418 x 74像素"` -> `(418, 74)` (same rule the handler uses: first two positive integers).
fn parse_canvas_pixel_size(text: &str) -> Option<(u32, u32)> {
    let mut numbers = text
        .split(|character: char| !character.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<u32>().ok());
    let width = numbers.next()?;
    let height = numbers.next()?;
    (width > 0 && height > 0).then_some((width, height))
}

/// Recording-only blob sink: verifies the *size* of `capture`'s image without touching storage.
///
/// Deliberately不落盘 —— 这里要证的是尺寸：DPI 感知缺失时 `capture` 按**虚拟化**尺寸分配缓冲，
/// 只拿到窗口左上角一块（`docs/memory/apps/paint.md` §12.5）。
#[derive(Debug, Default)]
struct RecordingBlobSink {
    last: Mutex<Option<(u32, u32)>>,
}

impl ImageBlobSink for RecordingBlobSink {
    fn store_bgra(
        &self,
        width: u32,
        height: u32,
        _content_address: &str,
        _bgra: &[u8],
    ) -> PlatformResult<()> {
        *self.last.lock().expect("recording sink") = Some((width, height));
        Ok(())
    }
}

/// Assert that `capture`'s image size equals `window_geometry`'s rectangle (ADR-0092 rule 3,
/// capture side). A mismatch means DPI awareness is not in effect and the capture is clipped.
async fn assert_capture_size_matches_window(
    platform: &WindowsPlatform,
    window: &ResolvedWindow,
) -> Result<(), Box<dyn std::error::Error>> {
    let geometry = platform.window_geometry(window).await?;
    let bounds = geometry.bounds();
    let expected_width = u32::try_from(bounds.width())?;
    let expected_height = u32::try_from(bounds.height())?;

    let sink: &'static RecordingBlobSink = Box::leak(Box::new(RecordingBlobSink::default()));
    let capturing = WindowsPlatform::new().with_blob_sink(sink);
    let image = capturing
        .capture(window, &CaptureOptions::new(false))
        .await?;

    if (image.width(), image.height()) != (expected_width, expected_height) {
        return Err(format!(
            "capture returned {}x{} but the window geometry is {}x{} — per-monitor-v2 DPI \
             awareness is not in effect (see docs/memory/apps/paint.md §12.5)",
            image.width(),
            image.height(),
            expected_width,
            expected_height
        )
        .into());
    }
    Ok(())
}

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
    // ADR-0092 rule 3: process-wide DPI awareness must be declared before *any* window / DPI
    // query. An integration-test binary has no application manifest, so it must be called here,
    // at the very top of the test body.
    assistant_platform_windows::enable_per_monitor_v2_dpi_awareness()?;
    let directory = TestDirectory::new("production-paint-real-t3-1")?;
    let blob_sink = StorageBlobSink::new();
    let sink_handle = blob_sink.clone();
    let platform = WindowsPlatform::new().with_blob_sink(blob_sink.leak());
    let paint_window = wait_for_real_paint_foreground(&platform).await?;
    // Let the human's mouse settle after they raised Paint, then re-check the foreground before the
    // plan is allowed to send any synthetic pointer input.
    settle_then_require_foreground(&platform, &paint_window, REAL_RUN_FOREGROUND_SETTLE).await?;
    // Capture-side self-check for ADR-0092 rule 3: the image must span the whole window.
    assert_capture_size_matches_window(&platform, &paint_window).await?;
    // The task package requires the injected zoom / offsets to match the canvas state at resolve
    // time, so measure the live document instead of reusing the fake platform's fixture constants.
    let canvas = read_real_canvas_inputs(&platform, &paint_window).await?;
    let host = assemble_production_host(
        paint_config_with_canvas(&directory.path, canvas),
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

/// TASK-307: continue on the already-open Paint document without calling `paint.document.new`.
#[tokio::test]
#[ignore = "TASK-307: requires an interactive Paint window with an existing unsaved canvas and human observer"]
async fn test_paint_t3_2_continues_existing_canvas() -> Result<(), Box<dyn std::error::Error>> {
    assistant_platform_windows::enable_per_monitor_v2_dpi_awareness()?;
    let directory = TestDirectory::new("production-paint-real-t3-2")?;
    let blob_sink = StorageBlobSink::new();
    let sink_handle = blob_sink.clone();
    let platform = WindowsPlatform::new().with_blob_sink(blob_sink.leak());
    let paint_window = wait_for_real_paint_foreground(&platform).await?;
    settle_then_require_foreground(&platform, &paint_window, REAL_RUN_FOREGROUND_SETTLE).await?;
    assert_capture_size_matches_window(&platform, &paint_window).await?;
    let canvas = read_real_canvas_inputs(&platform, &paint_window).await?;
    let host = assemble_production_host(
        paint_continue_config(&directory.path, canvas),
        platform,
        Arc::new(SystemClock),
    )
    .await?;
    assert!(sink_handle.attach(&host.database_handle()));

    let now_ms = SystemClock.now_unix_ms();
    let approvals = host.approvals();
    for step_id in [
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
    assert_eq!(plan.steps.len(), 9);
    assert!(
        !plan
            .steps
            .iter()
            .any(|step| step.id.as_str() == "new_document"),
        "T3.2 must never create a new document"
    );
    require_real_paint_foreground(&platform, &paint_window).await?;
    let run = host.execute_plan(plan, now_ms).await?;
    assert_eq!(run.final_snapshot.status, TaskStatus::Completed);
    host.shutdown().await?;
    Ok(())
}
