//! Paint T3.2 fake UIA and window provider for production acceptance.

use super::paint_real_run_t3_2_support::*;
use super::*;
use assistant_platform_api::KeyModifier;

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
const T3_2_ZOOM_ID: u64 = 2_112;
const T3_2_ZOOM_SLIDER_ID: u64 = 2_113;
const T3_2_ZOOM_OPTION_ID: u64 = 2_114;

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

fn activate_t3_2_dialog(state: &mut T3_2FakeState) -> PlatformResult<()> {
    let Some(dialog) = state.dialog else {
        return Err(platform_error(
            ErrorCode::TargetNotFound,
            "T3.2 fake dialog is not open",
        ));
    };
    match dialog {
        FileDialogKind::Open => {
            let bytes = state.source_bytes.clone();
            std::fs::write(&state.source_path, bytes).map_err(|error| {
                platform_error(
                    ErrorCode::Fatal,
                    format!("T3.2 fake open could not materialize the source: {error}"),
                )
            })?;
            state.dialog = None;
            state.open_invocations = state.open_invocations.saturating_add(1);
        }
        FileDialogKind::SaveAs => {
            if state.dialog_filename.is_empty() {
                return Err(platform_error(
                    ErrorCode::ToolInvalidArgs,
                    "T3.2 fake save has no filename",
                ));
            }
            let target = PathBuf::from(&state.dialog_filename);
            let bytes = state.source_bytes.clone();
            std::fs::write(&target, bytes).map_err(|error| {
                platform_error(
                    ErrorCode::Fatal,
                    format!("T3.2 fake save could not write target: {error}"),
                )
            })?;
            state.dialog = None;
            state.save_invocations = state.save_invocations.saturating_add(1);
        }
    }
    state.dialog_action_invocations = state.dialog_action_invocations.saturating_add(1);
    Ok(())
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
                || id.contains("zoom-values-combo")
                || id.contains("zoom-slider-control")
                || id.contains("zoom-50-percent")
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
        if id.contains("zoom-values-combo") {
            return ready(Ok(t3_2_element(T3_2_ZOOM_ID, T3_2_MAIN_WINDOW_ID)));
        }
        if id.contains("zoom-slider-control") {
            return ready(Ok(t3_2_element(T3_2_ZOOM_SLIDER_ID, T3_2_MAIN_WINDOW_ID)));
        }
        if id.contains("zoom-50-percent") {
            return ready(Ok(t3_2_element(T3_2_ZOOM_OPTION_ID, T3_2_MAIN_WINDOW_ID)));
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
            T3_2_CANVAS_ID => ElementBounds::new(100, 100, 1124, 868),
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
            T3_2_CANVAS_ID => ElementBounds::new(100, 100, 1124, 868),
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
            T3_2_ZOOM_ID => "50%".to_owned(),
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
            T3_2_ZOOM_ID | T3_2_ZOOM_SLIDER_ID => {}
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
        let toggle_zoom_combo =
            (action == "expand" || action == "collapse") && element.id().value() == T3_2_ZOOM_ID;
        let focus_dialog_filename =
            action == "focus" && element.id().value() == T3_2_DIALOG_FILENAME_ID;
        if action != "invoke" && !toggle_zoom_combo && !focus_dialog_filename {
            return ready(Err(platform_error(
                ErrorCode::CapabilityMissing,
                "T3.2 fake does not support this action",
            )));
        }
        let mut state = self.state.lock().expect("t3.2 fake state");
        match element.id().value() {
            T3_2_DIALOG_ACTION_ID => {
                if let Err(error) = activate_t3_2_dialog(&mut state) {
                    return ready(Err(error));
                }
            }
            T3_2_DIALOG_FILENAME_ID if action == "focus" => {}
            T3_2_ZOOM_ID if action == "expand" || action == "collapse" => {}
            T3_2_ZOOM_OPTION_ID if action == "invoke" => {}
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
            (T3_2_ZOOM_ID, Selection::ByIndex(12)) => {}
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
            ("Enter", []) => {
                if let Err(error) = activate_t3_2_dialog(&mut state) {
                    return ready(Err(error));
                }
                state.revision = state.revision.saturating_add(1);
            }
            ("Tab" | "P", []) => {
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
