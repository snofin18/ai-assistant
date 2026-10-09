//! Fake-platform production acceptance for the Paint T3.1 runtime slice.

#![cfg(windows)]
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::unwrap_used
)]

use std::future::{Future, ready};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use assistant_agent_core::{
    AdapterKind, GrantRequest, ProductionConfig, StorageBlobSink, UiServerConfig,
    assemble_production_host,
};
use assistant_hitl::ApprovalScope;
use assistant_platform_api::{
    CaptureOptions, CoordinateSpace, ElementBounds, ElementQuery, ElementState, ErrorCode,
    Fingerprint, FingerprintScope, FocusPolicy, ImageBlobSink, ImageRef, KeyChord, KeyTarget,
    NormalizedPoint, PlatformError, PlatformResult, PointerAction, ResolvedElement, ResolvedWindow,
    ScrollTarget, Selection, SelectorChain, TextEditOp, Timeout, TreeOptions, TreeSnapshot,
    UiAutomationProvider, WindowFilter, WindowInfo, WindowProvider, WindowState,
};
use assistant_storage::BlobId;
use assistant_task_engine::TaskStatus;
use serde_json::Value;

const FIXED_NOW_MS: i64 = 1_700_000_000_000;

struct FixedClock;

impl assistant_storage::Clock for FixedClock {
    fn now_unix_ms(&self) -> i64 {
        FIXED_NOW_MS
    }
}

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new(label: &str) -> Result<Self, std::io::Error> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(std::io::Error::other)?
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "assistant-agent-core-{label}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path)?;
        Ok(Self { path })
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

const WINDOW_ID: u64 = 100;
const TOOLBAR_ID: u64 = 101;
const RECTANGLE_TOOL_ID: u64 = 102;
const FOREGROUND_COLOR_ID: u64 = 103;
const LAYERS_PANEL_ID: u64 = 104;
const LAYER_ITEM_ID: u64 = 105;
const STATUS_BAR_ID: u64 = 106;
const CANVAS_ID: u64 = 107;
const LAYERS_TOGGLE_ID: u64 = 108;

#[derive(Clone)]
struct PaintFakePlatform {
    state: Arc<Mutex<PaintFakeState>>,
    blob_sink: Option<Arc<dyn ImageBlobSink>>,
}

struct PaintFakeState {
    revision: u64,
    tool: String,
    foreground_rgb: String,
    layer_id: String,
    layer_name: String,
    layers_expanded: bool,
    drawn: bool,
    pointer_actions: usize,
    capture_calls: usize,
}

impl PaintFakePlatform {
    fn new(blob_sink: Option<Arc<dyn ImageBlobSink>>) -> Self {
        Self {
            state: Arc::new(Mutex::new(PaintFakeState {
                revision: 1,
                tool: "pencil".to_owned(),
                foreground_rgb: "0,0,0".to_owned(),
                layer_id: "layer-0".to_owned(),
                layer_name: "Layer 0".to_owned(),
                layers_expanded: false,
                drawn: false,
                pointer_actions: 0,
                capture_calls: 0,
            })),
            blob_sink,
        }
    }

    fn fingerprint(&self) -> Fingerprint {
        let revision = self.state.lock().expect("paint fake state").revision;
        Fingerprint::parse(format!("sha256:{revision:064x}")).expect("fake fingerprint")
    }
}

impl WindowProvider for PaintFakePlatform {
    fn list_windows(
        &self,
        _filter: &WindowFilter,
    ) -> impl Future<Output = PlatformResult<Vec<WindowInfo>>> + Send {
        ready(Ok(vec![WindowInfo::new(
            window(),
            "com.microsoft.paint".to_owned(),
            "Paint".to_owned(),
        )]))
    }

    fn resolve_window(
        &self,
        _descriptor: &assistant_platform_api::TargetDescriptor,
    ) -> impl Future<Output = PlatformResult<ResolvedWindow>> + Send {
        ready(Ok(window()))
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
        ready(capture_fixture(self.blob_sink.as_ref(), &self.state))
    }
}

impl UiAutomationProvider for PaintFakePlatform {
    fn snapshot_tree(
        &self,
        root: &ResolvedWindow,
        _options: &TreeOptions,
    ) -> impl Future<Output = PlatformResult<TreeSnapshot>> + Send {
        ready(Ok(TreeSnapshot::new(root.clone(), self.fingerprint(), 8)))
    }

    fn resolve_element(
        &self,
        _scope: &ResolvedWindow,
        chain: &SelectorChain,
    ) -> impl Future<Output = PlatformResult<ResolvedElement>> + Send {
        let Some(candidate) = chain.candidates().iter().find(|candidate| {
            matches!(
                candidate.id(),
                "toolbar-command-bar"
                    | "shape-gallery-role-parent"
                    | "color-gallery-role-parent"
                    | "layers-container-name-fallback"
                    | "layers-toggle-role-parent"
                    | "layers-list-automation-id"
                    | "status-bar-canvas-size-automation-id"
                    | "canvas-image-automation-id"
            )
        }) else {
            return ready(Err(platform_error(
                ErrorCode::ToolInvalidArgs,
                "selector chain has no fixture-supported target candidate",
            )));
        };
        let id = match candidate.id() {
            "toolbar-command-bar" => TOOLBAR_ID,
            "shape-gallery-role-parent" => RECTANGLE_TOOL_ID,
            "color-gallery-role-parent" => FOREGROUND_COLOR_ID,
            "layers-container-name-fallback" => LAYERS_PANEL_ID,
            "layers-toggle-role-parent" => LAYERS_TOGGLE_ID,
            "layers-list-automation-id" => {
                let mut state = self.state.lock().expect("paint fake state");
                // The real panel is opened by Alt+L; the fixture accepts either
                // handler sequence and exposes the measured list once requested.
                state.layers_expanded = true;
                LAYER_ITEM_ID
            }
            "status-bar-canvas-size-automation-id" => STATUS_BAR_ID,
            "canvas-image-automation-id" => CANVAS_ID,
            _ => {
                return ready(Err(platform_error(
                    ErrorCode::TargetNotFound,
                    format!("fixture has no element for candidate `{}`", candidate.id()),
                )));
            }
        };
        ready(Ok(element(id)))
    }

    fn element_bounds(
        &self,
        element: &ResolvedElement,
    ) -> impl Future<Output = PlatformResult<ElementBounds>> + Send {
        let bounds = match element.id().value() {
            CANVAS_ID => ElementBounds::new(40, 50, 140, 150),
            TOOLBAR_ID => ElementBounds::new(10, 10, 300, 50),
            RECTANGLE_TOOL_ID | FOREGROUND_COLOR_ID => ElementBounds::new(20, 20, 80, 60),
            LAYERS_PANEL_ID | LAYER_ITEM_ID => ElementBounds::new(160, 20, 280, 200),
            LAYERS_TOGGLE_ID => ElementBounds::new(200, 20, 240, 60),
            STATUS_BAR_ID => ElementBounds::new(0, 180, 320, 200),
            _ => ElementBounds::new(0, 0, 1, 1),
        };
        ready(bounds)
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
            "wait_for is not used by the Paint fixture",
        )))
    }

    fn read_text(
        &self,
        element: &ResolvedElement,
    ) -> impl Future<Output = PlatformResult<String>> + Send {
        let state = self.state.lock().expect("paint fake state");
        let text = match element.id().value() {
            RECTANGLE_TOOL_ID => state.tool.clone(),
            FOREGROUND_COLOR_ID => state.foreground_rgb.clone(),
            LAYERS_PANEL_ID | LAYER_ITEM_ID => state.layer_name.clone(),
            STATUS_BAR_ID => "100 x 100 px".to_owned(),
            _ => {
                return ready(Err(platform_error(
                    ErrorCode::CapabilityMissing,
                    "fixture exposes read-back only for calibrated Paint controls",
                )));
            }
        };
        drop(state);
        ready(Ok(text))
    }

    fn set_value(
        &self,
        element: &ResolvedElement,
        value: &str,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        if element.id().value() != FOREGROUND_COLOR_ID {
            return ready(Err(platform_error(
                ErrorCode::CapabilityMissing,
                "fixture only supports setting the foreground color",
            )));
        }
        let mut state = self.state.lock().expect("paint fake state");
        value.clone_into(&mut state.foreground_rgb);
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
            "edit_text is not used by the Paint fixture",
        )))
    }

    fn invoke_action(
        &self,
        element: &ResolvedElement,
        _action: &str,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        let mut state = self.state.lock().expect("paint fake state");
        match element.id().value() {
            RECTANGLE_TOOL_ID => "rectangle".clone_into(&mut state.tool),
            LAYERS_TOGGLE_ID => {
                state.layers_expanded = !state.layers_expanded;
            }
            _ => {
                return ready(Err(platform_error(
                    ErrorCode::CapabilityMissing,
                    "fixture only supports the rectangle tool and layer toggle",
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
        let mut state = self.state.lock().expect("paint fake state");
        match (element.id().value(), selection) {
            (RECTANGLE_TOOL_ID, Selection::ByIndex(index)) if *index == 3 => {
                "rectangle".clone_into(&mut state.tool);
            }
            (FOREGROUND_COLOR_ID, Selection::ByIndex(index)) => match *index {
                3 => "237,28,36".clone_into(&mut state.foreground_rgb),
                _ => {
                    return ready(Err(platform_error(
                        ErrorCode::TargetNotFound,
                        format!("fixture has no measured palette index {index}"),
                    )));
                }
            },
            (LAYER_ITEM_ID, Selection::ByIndex(index)) if *index == 0 => {
                "layer-1".clone_into(&mut state.layer_id);
                "Layer 1".clone_into(&mut state.layer_name);
            }
            (LAYER_ITEM_ID, Selection::ByStableValue(value)) if value == "layer-1" => {
                "layer-1".clone_into(&mut state.layer_id);
                "Layer 1".clone_into(&mut state.layer_name);
            }
            _ => {
                return ready(Err(platform_error(
                    ErrorCode::TargetNotFound,
                    "fixture has no selectable child matching the requested contract",
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
            "scroll is not used by the Paint fixture",
        )))
    }

    fn pointer_action(
        &self,
        _coordinate_space: &CoordinateSpace,
        _point: NormalizedPoint,
        _action: &PointerAction,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        let mut state = self.state.lock().expect("paint fake state");
        state.pointer_actions = state.pointer_actions.saturating_add(1);
        state.drawn = true;
        state.revision = state.revision.saturating_add(1);
        drop(state);
        ready(Ok(()))
    }

    fn key_action(
        &self,
        chord: &KeyChord,
        _target: &KeyTarget,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        if chord.key() != "N" {
            return ready(Err(platform_error(
                ErrorCode::ToolInvalidArgs,
                "fixture only supports Ctrl+N",
            )));
        }
        let mut state = self.state.lock().expect("paint fake state");
        "pencil".clone_into(&mut state.tool);
        "0,0,0".clone_into(&mut state.foreground_rgb);
        "layer-0".clone_into(&mut state.layer_id);
        "Layer 0".clone_into(&mut state.layer_name);
        state.layers_expanded = false;
        state.drawn = false;
        state.revision = state.revision.saturating_add(1);
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

fn capture_fixture(
    blob_sink: Option<&Arc<dyn ImageBlobSink>>,
    state: &Arc<Mutex<PaintFakeState>>,
) -> PlatformResult<ImageRef> {
    let Some(blob_sink) = blob_sink else {
        return Err(platform_error(
            ErrorCode::Fatal,
            "fixture capture has no injected ImageBlobSink",
        ));
    };
    let mut state = state.lock().expect("paint fake state");
    state.capture_calls = state.capture_calls.saturating_add(1);
    let drawn = state.drawn;
    drop(state);
    let width = 4_u32;
    let height = 4_u32;
    let mut pixels = vec![0_u8; usize::try_from(width * height * 4).expect("small fixture")];
    if drawn {
        let Some(pixel) = pixels.get_mut(0..4) else {
            return Err(platform_error(
                ErrorCode::Fatal,
                "fixture pixel buffer is unexpectedly small",
            ));
        };
        pixel.copy_from_slice(&[36, 28, 237, 255]);
    }
    let content_address = BlobId::of_content(&pixels);
    blob_sink.store_bgra(width, height, content_address.as_str(), &pixels)?;
    Ok(ImageRef::new(
        content_address.as_str().to_owned(),
        width,
        height,
    ))
}

fn window() -> ResolvedWindow {
    ResolvedWindow::new(
        assistant_platform_api::LocalHandleId::new(WINDOW_ID),
        "Paint".to_owned(),
    )
}

fn element(id: u64) -> ResolvedElement {
    ResolvedElement::new(
        assistant_platform_api::LocalHandleId::new(id),
        assistant_platform_api::LocalHandleId::new(WINDOW_ID),
        "Fixture".to_owned(),
    )
}

fn platform_error(code: ErrorCode, message: impl Into<String>) -> PlatformError {
    PlatformError::new(code, message)
}

fn paint_config(data_root: &Path) -> ProductionConfig {
    let adapter_root = workspace_root().join("adapters/com.microsoft.paint");
    let task_package_path = adapter_root
        .join("tasks")
        .join("t3.1.new-canvas-rectangle-color-screenshot.json");
    let ui_config = UiServerConfig::new(
        "assistant-agent-core-paint-test",
        "ASSISTANT_AGENT_CORE_PAINT_TEST_TOKEN",
        Duration::from_secs(1),
    )
    .with_allowed_peer("C:\\fixture\\paint-peer.exe");
    let inputs = serde_json::json!({
        "input.canvas_width": 100,
        "input.canvas_height": 100,
        "input.rectangle_start": {"x": 10, "y": 10},
        "input.rectangle_start.x": 10,
        "input.rectangle_start.y": 10,
        "input.rectangle_end": {"x": 50, "y": 50},
        "input.rectangle_end.x": 50,
        "input.rectangle_end.y": 50,
        "input.foreground_red": 237,
        "input.foreground_green": 28,
        "input.foreground_blue": 36,
        "input.layer_id": "layer-1",
        "input.expected_layer_name": "Layer 1",
        "input.zoom_ratio": 1.0,
        "input.viewport_offset_x_px": 0,
        "input.viewport_offset_y_px": 0,
        "input.coordinate_space": {
            "kind": "physical_pixels",
            "monitor_id": "\\\\.\\DISPLAY1",
            "scale": 1.0
        },
        "input.tolerance_px": 2
    })
    .as_object()
    .cloned()
    .expect("paint task inputs");
    ProductionConfig::new(data_root, adapter_root, task_package_path, ui_config)
        .with_adapter_kind(AdapterKind::Paint)
        .with_task_inputs(inputs)
}

#[tokio::test]
async fn test_paint_t3_1_runs_to_completed_on_the_fake_platform()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("production-paint-t3-1")?;
    let sink = StorageBlobSink::new();
    let platform = PaintFakePlatform::new(Some(Arc::new(sink.clone())));
    let host = assemble_production_host(
        paint_config(&directory.path),
        platform.clone(),
        Arc::new(FixedClock),
    )
    .await?;
    assert!(sink.attach(&host.database_handle()));

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
            now_ms: FIXED_NOW_MS,
            ttl_ms: 60_000,
            uses: 1,
        })?;
    }

    let plan = host.plan_task()?;
    assert_eq!(plan.steps.len(), 10);
    let run = host.execute_plan(plan, FIXED_NOW_MS).await?;
    assert_eq!(run.final_snapshot.status, TaskStatus::Completed);
    assert!(
        run.final_snapshot
            .steps
            .iter()
            .all(|step| { step.status == assistant_task_engine::StepStatus::Committed })
    );
    {
        let state = platform.state.lock().expect("paint fake state");
        assert!(state.drawn);
        assert_eq!(state.pointer_actions, 1);
        assert!(state.capture_calls >= 3);
        drop(state);
    }
    host.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn test_paint_missing_tool_fails_closed_at_assembly() -> Result<(), Box<dyn std::error::Error>>
{
    let directory = TestDirectory::new("production-paint-missing-tool")?;
    let adapter_root = directory.path.join("adapter");
    copy_paint_adapter(&adapter_root)?;
    let tools_path = adapter_root.join("tools").join("tools.json");
    let body = std::fs::read_to_string(&tools_path)?;
    let mut tools: Value = serde_json::from_str(&body)?;
    tools
        .get_mut("tools")
        .and_then(Value::as_array_mut)
        .expect("tools array")
        .retain(|tool| {
            tool.pointer("/schema/name").and_then(Value::as_str)
                != Some("paint.canvas.draw_rectangle")
        });
    std::fs::write(&tools_path, serde_json::to_vec(&tools)?)?;

    let mut config = paint_config(&directory.path);
    config.adapter_root = adapter_root;
    let platform = PaintFakePlatform::new(None);
    let error = match assemble_production_host(config, platform, Arc::new(FixedClock)).await {
        Ok(_host) => panic!("missing Paint tool must fail assembly"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("missing production tools"));
    Ok(())
}

#[tokio::test]
async fn test_paint_missing_target_fails_closed_at_assembly()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("production-paint-missing-target")?;
    let adapter_root = directory.path.join("adapter");
    copy_paint_adapter(&adapter_root)?;
    let targets_path = adapter_root.join("selectors").join("targets.json");
    let body = std::fs::read_to_string(&targets_path)?;
    let mut targets: Value = serde_json::from_str(&body)?;
    targets
        .get_mut("targets")
        .and_then(Value::as_array_mut)
        .expect("targets array")
        .retain(|target| target.get("id").and_then(Value::as_str) != Some("canvas"));
    std::fs::write(&targets_path, serde_json::to_vec(&targets)?)?;

    let mut config = paint_config(&directory.path);
    config.adapter_root = adapter_root;
    let platform = PaintFakePlatform::new(None);
    let error = match assemble_production_host(config, platform, Arc::new(FixedClock)).await {
        Ok(_host) => panic!("missing Paint target must fail assembly"),
        Err(error) => error,
    };
    assert!(
        error
            .to_string()
            .contains("missing required target `canvas`")
    );
    Ok(())
}

#[tokio::test]
async fn test_paint_missing_blob_sink_fails_closed_at_capture()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("production-paint-missing-sink")?;
    let platform = PaintFakePlatform::new(None);
    let host = assemble_production_host(
        paint_config(&directory.path),
        platform,
        Arc::new(FixedClock),
    )
    .await?;
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
            now_ms: FIXED_NOW_MS,
            ttl_ms: 60_000,
            uses: 1,
        })?;
    }
    let plan = host.plan_task()?;
    let error = match host.execute_plan(plan, FIXED_NOW_MS).await {
        Ok(_run) => panic!("capture without a blob sink must fail"),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains("ImageBlobSink"),
        "unexpected capture error: {error}"
    );
    host.shutdown().await?;
    Ok(())
}

fn copy_paint_adapter(destination: &Path) -> Result<(), std::io::Error> {
    let source = workspace_root().join("adapters/com.microsoft.paint");
    std::fs::create_dir_all(destination.join("selectors"))?;
    std::fs::create_dir_all(destination.join("tools"))?;
    std::fs::copy(
        source.join("selectors").join("targets.json"),
        destination.join("selectors").join("targets.json"),
    )?;
    std::fs::copy(
        source.join("tools").join("tools.json"),
        destination.join("tools").join("tools.json"),
    )?;
    Ok(())
}
