//! Real-machine Paint acceptance (opt-in) plus the foreground-settling guards around it.
//!
//! Included by `production_paint.rs` via `#[path]`; it reuses that test binary's private helpers
//! (`paint_config`, `TestDirectory`, `workspace_root`, and the shared imports) through `super::*`.
//! Kept in its own file so `production_paint.rs` stays under the 900-line hygiene hard limit.

use super::*;

use assistant_platform_api::{KeyModifier, SelectorCandidate, SelectorKind, SelectorValue};
use assistant_protocol::serde_json::json;

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
pub async fn read_real_canvas_inputs(
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
pub async fn assert_capture_size_matches_window(
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
pub const REAL_RUN_FOREGROUND_SETTLE: Duration = Duration::from_secs(2);

/// Fails unless `window` is currently the foreground window.
///
/// Fail-closed on purpose: sending synthetic input to whatever happens to be foreground is exactly
/// the "focus 错位会打到别的应用上" hazard the platform contract forbids, so a lost foreground
/// aborts the run instead of driving the wrong window.
pub async fn require_real_paint_foreground(
    platform: &WindowsPlatform,
    window: &ResolvedWindow,
) -> Result<(), Box<dyn std::error::Error>> {
    if !platform.window_state(window).await?.foreground() {
        return Err("Paint is not the foreground window; refusing to send synthetic input".into());
    }
    Ok(())
}

/// Waits `settle`, then requires `window` to still be the foreground window.
pub async fn settle_then_require_foreground(
    platform: &WindowsPlatform,
    window: &ResolvedWindow,
    settle: Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    tokio::time::sleep(settle).await;
    require_real_paint_foreground(platform, window).await
}

pub async fn wait_for_real_paint_foreground(
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

const T3_2_MAIN_WINDOW_ID: u64 = 2_100;
const T3_2_DIALOG_WINDOW_ID: u64 = 2_101;
const T3_2_DIALOG_FILENAME_ID: u64 = 2_102;
const T3_2_DIALOG_ACTION_ID: u64 = 2_103;
const T3_2_CANVAS_ID: u64 = 2_104;
const T3_2_STATUS_ID: u64 = 2_105;
const T3_2_RECTANGLE_ID: u64 = 2_106;
const T3_2_FOREGROUND_ID: u64 = 2_107;
const T3_2_LAYER_ID: u64 = 2_108;
const T3_2_OUTLINE_ID: u64 = 2_109;
const T3_2_FILL_ID: u64 = 2_110;
const T3_2_SIZE_ID: u64 = 2_111;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileDialogKind {
    Open,
    SaveAs,
}

struct T3_2FakeState {
    revision: u64,
    canvas_revision: u32,
    tool: String,
    foreground_rgb: String,
    layer_id: String,
    layer_name: String,
    shapes_expanded: bool,
    layers_expanded: bool,
    dialog: Option<FileDialogKind>,
    dialog_filename: String,
    source_path: PathBuf,
    target_path: PathBuf,
    canvas_width_px: u32,
    canvas_height_px: u32,
    source_bytes: Vec<u8>,
    drawn: bool,
    dialog_action_invocations: usize,
    open_invocations: usize,
    save_invocations: usize,
    pointer_actions: usize,
    race_create_on_set_value: bool,
    outline_visible: bool,
    fill_visible: bool,
    size_visible: bool,
}

#[derive(Clone)]
struct T3_2FakePlatform {
    state: Arc<Mutex<T3_2FakeState>>,
    blob_sink: Option<Arc<dyn ImageBlobSink>>,
}

impl T3_2FakePlatform {
    fn new(
        source_path: PathBuf,
        target_path: PathBuf,
        source_bytes: Vec<u8>,
        canvas_width_px: u32,
        canvas_height_px: u32,
        blob_sink: Option<Arc<dyn ImageBlobSink>>,
    ) -> Self {
        Self {
            state: Arc::new(Mutex::new(T3_2FakeState {
                revision: 1,
                canvas_revision: 0,
                tool: "pencil".to_owned(),
                foreground_rgb: "0,0,0".to_owned(),
                layer_id: "layer-0".to_owned(),
                layer_name: "Layer 0".to_owned(),
                shapes_expanded: false,
                layers_expanded: false,
                dialog: None,
                dialog_filename: String::new(),
                source_path,
                target_path,
                canvas_width_px,
                canvas_height_px,
                source_bytes,
                drawn: false,
                dialog_action_invocations: 0,
                open_invocations: 0,
                save_invocations: 0,
                pointer_actions: 0,
                race_create_on_set_value: false,
                outline_visible: true,
                fill_visible: true,
                size_visible: true,
            })),
            blob_sink,
        }
    }

    fn with_race_create_on_set_value(self) -> Self {
        self.state
            .lock()
            .expect("t3.2 fake state")
            .race_create_on_set_value = true;
        self
    }

    fn fingerprint(&self) -> Fingerprint {
        let revision = self.state.lock().expect("t3.2 fake state").revision;
        Fingerprint::parse(format!("sha256:{revision:064x}")).expect("t3.2 fake fingerprint")
    }
}

impl WindowProvider for T3_2FakePlatform {
    fn list_windows(
        &self,
        _filter: &WindowFilter,
    ) -> impl Future<Output = PlatformResult<Vec<WindowInfo>>> + Send {
        let dialog_open = self.state.lock().expect("t3.2 fake state").dialog.is_some();
        let mut windows = vec![WindowInfo::new(
            t3_2_main_window(),
            "mspaint.exe".to_owned(),
            "Paint".to_owned(),
        )];
        if dialog_open {
            windows.push(WindowInfo::new(
                t3_2_dialog_window(),
                "mspaint.exe".to_owned(),
                "File dialog".to_owned(),
            ));
        }
        ready(Ok(windows))
    }

    fn resolve_window(
        &self,
        descriptor: &assistant_platform_api::TargetDescriptor,
    ) -> impl Future<Output = PlatformResult<ResolvedWindow>> + Send {
        let is_dialog = descriptor.window_candidates().iter().any(|candidate| {
            candidate.id().contains("file-open") || candidate.id().contains("save-as")
        });
        if is_dialog {
            let open = self.state.lock().expect("t3.2 fake state").dialog.is_some();
            if open {
                return ready(Ok(t3_2_dialog_window()));
            }
            return ready(Err(platform_error(
                ErrorCode::TargetNotFound,
                "T3.2 fake file dialog is not open",
            )));
        }
        ready(Ok(t3_2_main_window()))
    }

    fn window_state(
        &self,
        _window: &ResolvedWindow,
    ) -> impl Future<Output = PlatformResult<WindowState>> + Send {
        ready(Ok(WindowState::new(false, true, false)))
    }

    fn bring_to_front(
        &self,
        _window: &ResolvedWindow,
        _policy: FocusPolicy,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        ready(Ok(()))
    }

    fn capture(
        &self,
        _window: &ResolvedWindow,
        _options: &CaptureOptions,
    ) -> impl Future<Output = PlatformResult<ImageRef>> + Send {
        let Some(blob_sink) = self.blob_sink.as_ref() else {
            return ready(Err(platform_error(
                ErrorCode::Fatal,
                "T3.2 fake capture has no ImageBlobSink",
            )));
        };
        let state = self.state.lock().expect("t3.2 fake state");
        let width = 100_u32;
        let height = 100_u32;
        let mut pixels =
            vec![255_u8; usize::try_from(width * height * 4).expect("T3.2 fake image size")];
        if state.drawn {
            for pixel in pixels.as_chunks_mut::<4>().0.iter_mut().take(3) {
                pixel.copy_from_slice(&[36, 28, 237, 255]);
            }
        }
        let content_address = BlobId::of_content(&pixels);
        let stored = blob_sink.store_bgra(width, height, content_address.as_str(), &pixels);
        drop(state);
        match stored {
            Ok(()) => ready(Ok(ImageRef::new(
                content_address.as_str().to_owned(),
                width,
                height,
            ))),
            Err(error) => ready(Err(error)),
        }
    }

    fn window_geometry(
        &self,
        _window: &ResolvedWindow,
    ) -> impl Future<Output = PlatformResult<WindowGeometry>> + Send {
        ready(
            ElementBounds::new(0, 0, 2400, 1800)
                .map(|bounds| WindowGeometry::new(bounds, fixture_space())),
        )
    }
}

impl UiAutomationProvider for T3_2FakePlatform {
    fn snapshot_tree(
        &self,
        root: &ResolvedWindow,
        _options: &TreeOptions,
    ) -> impl Future<Output = PlatformResult<TreeSnapshot>> + Send {
        ready(Ok(TreeSnapshot::new(root.clone(), self.fingerprint(), 8)))
    }

    fn resolve_element(
        &self,
        scope: &ResolvedWindow,
        chain: &SelectorChain,
    ) -> impl Future<Output = PlatformResult<ResolvedElement>> + Send {
        let candidate = chain.candidates().iter().find(|candidate| {
            let id = candidate.id();
            id.contains("file-open-filename")
                || id.contains("file-open-button")
                || id.contains("save-as-filename")
                || id.contains("save-as-button")
                || id.contains("rectangle-item")
                || id.contains("color-gallery")
                || id.contains("layers-list")
                || id.contains("status-bar-canvas")
                || id.contains("canvas-image")
                || id.contains("shape-outline")
                || id.contains("shape-fill")
                || id.contains("shape-size")
                || id.contains("outline-style-solid")
                || id.contains("fill-style-none")
                || id.contains("size-3px")
                || id.contains("shapes-gallery-toggle")
        });
        let Some(candidate) = candidate else {
            return ready(Err(platform_error(
                ErrorCode::ToolInvalidArgs,
                "T3.2 fake selector chain has no supported candidate",
            )));
        };
        let id = candidate.id();
        if id.contains("file-open-filename") || id.contains("save-as-filename") {
            return ready(Ok(t3_2_element(
                T3_2_DIALOG_FILENAME_ID,
                T3_2_DIALOG_WINDOW_ID,
            )));
        }
        if id.contains("file-open-button") || id.contains("save-as-button") {
            return ready(Ok(t3_2_element(
                T3_2_DIALOG_ACTION_ID,
                T3_2_DIALOG_WINDOW_ID,
            )));
        }
        if id.contains("rectangle-item") {
            let expanded = self.state.lock().expect("t3.2 fake state").shapes_expanded;
            return if expanded {
                ready(Ok(t3_2_element(T3_2_RECTANGLE_ID, T3_2_MAIN_WINDOW_ID)))
            } else {
                ready(Err(platform_error(
                    ErrorCode::TargetNotFound,
                    "T3.2 fake shape gallery is collapsed",
                )))
            };
        }
        if id.contains("color-gallery") {
            return ready(Ok(t3_2_element(T3_2_FOREGROUND_ID, T3_2_MAIN_WINDOW_ID)));
        }
        if id.contains("layers-list") {
            return ready(Ok(t3_2_element(T3_2_LAYER_ID, T3_2_MAIN_WINDOW_ID)));
        }
        if id.contains("status-bar-canvas") {
            return ready(Ok(t3_2_element(T3_2_STATUS_ID, T3_2_MAIN_WINDOW_ID)));
        }
        if id.contains("canvas-image") {
            return ready(Ok(t3_2_element(T3_2_CANVAS_ID, T3_2_MAIN_WINDOW_ID)));
        }
        if id.contains("outline-style-solid") {
            if !self.state.lock().expect("t3.2 fake state").outline_visible {
                return ready(Err(platform_error(
                    ErrorCode::TargetNotFound,
                    "T3.2 fake outline item is no longer materialized",
                )));
            }
            return ready(Ok(t3_2_element(T3_2_OUTLINE_ID, T3_2_MAIN_WINDOW_ID)));
        }
        if id.contains("fill-style-none") {
            if !self.state.lock().expect("t3.2 fake state").fill_visible {
                return ready(Err(platform_error(
                    ErrorCode::TargetNotFound,
                    "T3.2 fake fill item is no longer materialized",
                )));
            }
            return ready(Ok(t3_2_element(T3_2_FILL_ID, T3_2_MAIN_WINDOW_ID)));
        }
        if id.contains("size-3px") {
            if !self.state.lock().expect("t3.2 fake state").size_visible {
                return ready(Err(platform_error(
                    ErrorCode::TargetNotFound,
                    "T3.2 fake size item is no longer materialized",
                )));
            }
            return ready(Ok(t3_2_element(T3_2_SIZE_ID, T3_2_MAIN_WINDOW_ID)));
        }
        if id.contains("shapes-gallery-toggle") {
            return ready(Ok(t3_2_element(T3_2_RECTANGLE_ID, T3_2_MAIN_WINDOW_ID)));
        }
        let _ = scope;
        ready(Err(platform_error(
            ErrorCode::TargetNotFound,
            "T3.2 fake element not materialized",
        )))
    }

    fn element_bounds(
        &self,
        element: &ResolvedElement,
    ) -> impl Future<Output = PlatformResult<ElementBounds>> + Send {
        let bounds = match element.id().value() {
            T3_2_CANVAS_ID => ElementBounds::new(100, 100, 500, 500),
            T3_2_DIALOG_FILENAME_ID => ElementBounds::new(20, 20, 180, 40),
            T3_2_DIALOG_ACTION_ID => ElementBounds::new(200, 20, 240, 40),
            _ => ElementBounds::new(0, 0, 1, 1),
        };
        ready(bounds)
    }

    fn element_geometry(
        &self,
        element: &ResolvedElement,
    ) -> impl Future<Output = PlatformResult<ElementGeometry>> + Send {
        let bounds = match element.id().value() {
            T3_2_CANVAS_ID => ElementBounds::new(100, 100, 500, 500),
            T3_2_RECTANGLE_ID => ElementBounds::new(20, 20, 80, 60),
            T3_2_DIALOG_ACTION_ID => ElementBounds::new(200, 20, 240, 40),
            _ => ElementBounds::new(0, 0, 1, 1),
        };
        ready(Ok(ElementGeometry::new(
            bounds.expect("t3.2 element bounds"),
            fixture_space(),
        )))
    }

    fn wait_for(
        &self,
        _scope: &ResolvedWindow,
        _query: &ElementQuery,
        _state: &ElementState,
        _timeout: Timeout,
    ) -> impl Future<Output = PlatformResult<ResolvedElement>> + Send {
        ready(Err(platform_error(
            ErrorCode::CapabilityMissing,
            "wait_for is not used by the T3.2 fake platform",
        )))
    }

    fn read_text(
        &self,
        element: &ResolvedElement,
    ) -> impl Future<Output = PlatformResult<String>> + Send {
        let state = self.state.lock().expect("t3.2 fake state");
        let text = match element.id().value() {
            T3_2_RECTANGLE_ID => state.tool.clone(),
            T3_2_FOREGROUND_ID => state.foreground_rgb.clone(),
            T3_2_LAYER_ID => state.layer_name.clone(),
            T3_2_STATUS_ID => format!("{} x {} px", state.canvas_width_px, state.canvas_height_px),
            _ => String::new(),
        };
        drop(state);
        ready(Ok(text))
    }

    fn set_value(
        &self,
        element: &ResolvedElement,
        value: &str,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        let mut state = self.state.lock().expect("t3.2 fake state");
        match element.id().value() {
            T3_2_DIALOG_FILENAME_ID => {
                value.clone_into(&mut state.dialog_filename);
                if state.race_create_on_set_value
                    && state.dialog == Some(FileDialogKind::SaveAs)
                    && let Err(error) = std::fs::write(&state.target_path, b"protected")
                {
                    return ready(Err(platform_error(
                        ErrorCode::Fatal,
                        format!("T3.2 race fixture could not create the target: {error}"),
                    )));
                }
            }
            T3_2_FOREGROUND_ID => {
                value.clone_into(&mut state.foreground_rgb);
            }
            _ => {
                return ready(Err(platform_error(
                    ErrorCode::CapabilityMissing,
                    "T3.2 fake set_value target is unsupported",
                )));
            }
        }
        state.revision = state.revision.saturating_add(1);
        drop(state);
        ready(Ok(()))
    }

    fn edit_text(
        &self,
        _element: &ResolvedElement,
        _operation: &TextEditOp,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        ready(Err(platform_error(
            ErrorCode::CapabilityMissing,
            "edit_text is not used by the T3.2 fake platform",
        )))
    }

    fn invoke_action(
        &self,
        element: &ResolvedElement,
        action: &str,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        if action != "invoke" {
            return ready(Err(platform_error(
                ErrorCode::CapabilityMissing,
                "T3.2 fake only supports invoke",
            )));
        }
        let mut state = self.state.lock().expect("t3.2 fake state");
        match element.id().value() {
            T3_2_DIALOG_ACTION_ID => {
                let Some(dialog) = state.dialog else {
                    return ready(Err(platform_error(
                        ErrorCode::TargetNotFound,
                        "T3.2 fake dialog is not open",
                    )));
                };
                match dialog {
                    FileDialogKind::Open => {
                        let bytes = state.source_bytes.clone();
                        if std::fs::write(&state.source_path, bytes).is_err() {
                            return ready(Err(platform_error(
                                ErrorCode::Fatal,
                                "T3.2 fake open could not materialize the source",
                            )));
                        }
                        state.dialog = None;
                        state.open_invocations = state.open_invocations.saturating_add(1);
                    }
                    FileDialogKind::SaveAs => {
                        if state.dialog_filename.is_empty() {
                            return ready(Err(platform_error(
                                ErrorCode::ToolInvalidArgs,
                                "T3.2 fake save has no filename",
                            )));
                        }
                        let target = PathBuf::from(&state.dialog_filename);
                        let bytes = state.source_bytes.clone();
                        if std::fs::write(&target, bytes).is_err() {
                            return ready(Err(platform_error(
                                ErrorCode::Fatal,
                                "T3.2 fake save could not write target",
                            )));
                        }
                        state.dialog = None;
                        state.save_invocations = state.save_invocations.saturating_add(1);
                    }
                }
                state.dialog_action_invocations = state.dialog_action_invocations.saturating_add(1);
            }
            T3_2_OUTLINE_ID => state.outline_visible = false,
            T3_2_FILL_ID => state.fill_visible = false,
            T3_2_SIZE_ID => state.size_visible = false,
            T3_2_RECTANGLE_ID => {
                state.shapes_expanded = true;
            }
            _ => {
                return ready(Err(platform_error(
                    ErrorCode::CapabilityMissing,
                    "T3.2 fake invoke target is unsupported",
                )));
            }
        }
        state.revision = state.revision.saturating_add(1);
        drop(state);
        ready(Ok(()))
    }

    fn select(
        &self,
        element: &ResolvedElement,
        selection: &Selection,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        let mut state = self.state.lock().expect("t3.2 fake state");
        match (element.id().value(), selection) {
            (T3_2_FOREGROUND_ID, Selection::ByIndex(3)) => {
                "237,28,36".clone_into(&mut state.foreground_rgb);
            }
            (T3_2_LAYER_ID, Selection::ByIndex(0)) => {
                "layer-1".clone_into(&mut state.layer_id);
                "Layer 1".clone_into(&mut state.layer_name);
            }
            _ => {
                return ready(Err(platform_error(
                    ErrorCode::TargetNotFound,
                    "T3.2 fake select target is unsupported",
                )));
            }
        }
        state.revision = state.revision.saturating_add(1);
        drop(state);
        ready(Ok(()))
    }

    fn scroll(
        &self,
        _element: &ResolvedElement,
        _target: &ScrollTarget,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        ready(Err(platform_error(
            ErrorCode::CapabilityMissing,
            "scroll is not used by the T3.2 fake platform",
        )))
    }

    fn pointer_action(
        &self,
        coordinate_space: &CoordinateSpace,
        point: NormalizedPoint,
        action: &PointerAction,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        let mut state = self.state.lock().expect("t3.2 fake state");
        state.pointer_actions = state.pointer_actions.saturating_add(1);
        state.revision = state.revision.saturating_add(1);
        match action {
            PointerAction::Click => {
                if click_lands_in_element(point, coordinate_space, T3_2_RECTANGLE_ID) {
                    "rectangle".clone_into(&mut state.tool);
                }
            }
            PointerAction::DragTo { .. } => {
                state.drawn = true;
                state.canvas_revision = state.canvas_revision.saturating_add(1);
            }
            _ => {}
        }
        drop(state);
        ready(Ok(()))
    }

    fn key_action(
        &self,
        chord: &KeyChord,
        _target: &KeyTarget,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        let mut state = self.state.lock().expect("t3.2 fake state");
        match (chord.key(), chord.modifiers()) {
            ("O", modifiers) if modifiers.contains(&KeyModifier::Control) => {
                state.dialog = Some(FileDialogKind::Open);
                state.dialog_filename.clear();
                state.revision = state.revision.saturating_add(1);
            }
            ("S", modifiers)
                if modifiers.contains(&KeyModifier::Control)
                    && modifiers.contains(&KeyModifier::Shift) =>
            {
                state.dialog = Some(FileDialogKind::SaveAs);
                state.dialog_filename.clear();
                state.revision = state.revision.saturating_add(1);
            }
            ("Escape", []) => {
                state.dialog = None;
                state.revision = state.revision.saturating_add(1);
            }
            ("L", modifiers) if modifiers.contains(&KeyModifier::Alt) => {
                state.layers_expanded = !state.layers_expanded;
                state.revision = state.revision.saturating_add(1);
            }
            ("N", modifiers) if modifiers.contains(&KeyModifier::Control) => {
                "pencil".clone_into(&mut state.tool);
                "0,0,0".clone_into(&mut state.foreground_rgb);
                "layer-0".clone_into(&mut state.layer_id);
                "Layer 0".clone_into(&mut state.layer_name);
                state.shapes_expanded = false;
                state.drawn = false;
                state.revision = state.revision.saturating_add(1);
            }
            _ => {
                return ready(Err(platform_error(
                    ErrorCode::ToolInvalidArgs,
                    "T3.2 fake key chord is unsupported",
                )));
            }
        }
        drop(state);
        ready(Ok(()))
    }

    fn fingerprint(
        &self,
        _window: &ResolvedWindow,
        _scope: &FingerprintScope,
    ) -> impl Future<Output = PlatformResult<Fingerprint>> + Send {
        ready(Ok(self.fingerprint()))
    }
}

fn t3_2_main_window() -> ResolvedWindow {
    ResolvedWindow::new(
        assistant_platform_api::LocalHandleId::new(T3_2_MAIN_WINDOW_ID),
        "Paint".to_owned(),
    )
}

fn t3_2_dialog_window() -> ResolvedWindow {
    ResolvedWindow::new(
        assistant_platform_api::LocalHandleId::new(T3_2_DIALOG_WINDOW_ID),
        "mspaint.exe [#32770] File dialog".to_owned(),
    )
}

fn t3_2_element(id: u64, parent: u64) -> ResolvedElement {
    ResolvedElement::new(
        assistant_platform_api::LocalHandleId::new(id),
        assistant_platform_api::LocalHandleId::new(parent),
        "T3.2 fixture".to_owned(),
    )
}

fn click_lands_in_element(
    point: NormalizedPoint,
    coordinate_space: &CoordinateSpace,
    element_id: u64,
) -> bool {
    let bounds = match element_id {
        T3_2_RECTANGLE_ID => ElementBounds::new(20, 20, 80, 60),
        _ => return false,
    }
    .expect("t3.2 click bounds");
    let scale = coordinate_space.scale();
    let horizontal = point.x_logical() * scale;
    let vertical = point.y_logical() * scale;
    horizontal >= f64::from(bounds.left())
        && horizontal < f64::from(bounds.right())
        && vertical >= f64::from(bounds.top())
        && vertical < f64::from(bounds.bottom())
}

fn t3_2_png_header(width: u32, height: u32) -> Vec<u8> {
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

fn t3_2_task_config(data_root: &Path, opened_path: &Path, target_path: &Path) -> ProductionConfig {
    t3_2_task_config_with_space(data_root, opened_path, target_path, "\\\\.\\DISPLAY1", 1.0)
}

fn t3_2_task_config_with_space(
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

fn grant_t3_2_steps<P>(host: &assistant_agent_core::ProductionHost<P>, now_ms: i64)
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

#[tokio::test]
async fn test_paint_t3_2_open_mark_save_as_fake_positive() -> Result<(), Box<dyn std::error::Error>>
{
    let directory = TestDirectory::new("production-paint-t3-2-positive")?;
    let source = directory.path.join("source.png");
    let target = directory.path.join("marked-output.png");
    let source_bytes = t3_2_png_header(2048, 1536);
    std::fs::write(&source, &source_bytes)?;
    let sink = StorageBlobSink::new();
    let platform = T3_2FakePlatform::new(
        source.clone(),
        target.clone(),
        source_bytes,
        2048,
        1536,
        Some(Arc::new(sink.clone())),
    );
    let host = assemble_production_host(
        t3_2_task_config(&directory.path, &source, &target),
        platform.clone(),
        Arc::new(FixedClock),
    )
    .await?;
    assert!(sink.attach(&host.database_handle()));
    grant_t3_2_steps(&host, FIXED_NOW_MS);
    let plan = host.plan_task()?;
    let run = host.execute_plan(plan, FIXED_NOW_MS).await?;
    assert_eq!(run.final_snapshot.status, TaskStatus::Completed);
    {
        let state = platform.state.lock().expect("t3.2 fake state");
        assert!(state.drawn);
        assert!(state.dialog.is_none());
        assert_eq!(state.dialog_action_invocations, 2);
        assert_eq!(state.open_invocations, 1);
        assert_eq!(state.save_invocations, 1);
        drop(state);
    }
    assert!(std::fs::metadata(&target)?.len() > 0);
    host.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn test_paint_t3_2_existing_target_is_rejected_before_handler()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("production-paint-t3-2-existing")?;
    let source = directory.path.join("source.png");
    let target = directory.path.join("protected-output.png");
    let source_bytes = t3_2_png_header(2048, 1536);
    std::fs::write(&source, &source_bytes)?;
    let protected = b"protected".to_vec();
    std::fs::write(&target, &protected)?;
    let sink = StorageBlobSink::new();
    let platform = T3_2FakePlatform::new(
        source.clone(),
        target.clone(),
        source_bytes,
        2048,
        1536,
        Some(Arc::new(sink.clone())),
    );
    let host = assemble_production_host(
        t3_2_task_config(&directory.path, &source, &target),
        platform.clone(),
        Arc::new(FixedClock),
    )
    .await?;
    assert!(sink.attach(&host.database_handle()));
    grant_t3_2_steps(&host, FIXED_NOW_MS);
    let plan = host.plan_task()?;
    let run = host.execute_plan(plan, FIXED_NOW_MS).await?;
    assert!(run.is_awaiting_approval());
    assert_eq!(
        run.pending_approval()
            .map(|approval| approval.step_id.as_str()),
        Some("reject_existing_target")
    );
    assert_eq!(std::fs::read(&target)?, protected);
    {
        let state = platform.state.lock().expect("t3.2 fake state");
        assert_eq!(state.save_invocations, 0);
        drop(state);
    }
    host.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn test_paint_t3_2_race_target_is_rejected_without_save_click()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("production-paint-t3-2-race")?;
    let source = directory.path.join("source.png");
    let target = directory.path.join("race-output.png");
    let source_bytes = t3_2_png_header(2048, 1536);
    std::fs::write(&source, &source_bytes)?;
    let sink = StorageBlobSink::new();
    let platform = T3_2FakePlatform::new(
        source.clone(),
        target.clone(),
        source_bytes,
        2048,
        1536,
        Some(Arc::new(sink.clone())),
    )
    .with_race_create_on_set_value();
    let host = assemble_production_host(
        t3_2_task_config(&directory.path, &source, &target),
        platform.clone(),
        Arc::new(FixedClock),
    )
    .await?;
    assert!(sink.attach(&host.database_handle()));
    grant_t3_2_steps(&host, FIXED_NOW_MS);
    let plan = host.plan_task()?;
    let Err(error) = host.execute_plan(plan, FIXED_NOW_MS).await else {
        panic!("race target must fail closed");
    };
    assert!(
        error.to_string().contains("UserInteraction"),
        "unexpected race error: {error}"
    );
    assert_eq!(std::fs::read(&target)?, b"protected");
    {
        let state = platform.state.lock().expect("t3.2 fake state");
        assert_eq!(state.save_invocations, 0);
        assert!(state.dialog.is_none());
        drop(state);
    }
    host.shutdown().await?;
    Ok(())
}

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
    let run = host.execute_plan(plan, now_ms).await?;
    assert_eq!(run.final_snapshot.status, TaskStatus::Completed);
    assert!(target.is_file());
    let saved_dimensions = read_png_dimensions(&target)?;
    assert_eq!(saved_dimensions, (2048, 1536));
    host.shutdown().await?;
    Ok(())
}

fn read_png_dimensions(path: &Path) -> Result<(u32, u32), std::io::Error> {
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
