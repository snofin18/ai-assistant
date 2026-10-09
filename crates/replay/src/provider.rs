//! Offline platform providers backed by a recording.

use std::future::{Future, ready};
use std::sync::Arc;

use assistant_platform_api::{
    CaptureOptions, CoordinateSpace, ElementBounds, ErrorCode, FocusPolicy, ImageRef, KeyChord,
    KeyTarget, PlatformError, PlatformResult, PointerAction, ResolvedElement, ResolvedWindow,
    ScrollTarget, Selection, SelectorChain, SelectorKind, SelectorValue, TextEditOp, Timeout,
    TreeOptions, TreeSnapshot, UiAutomationProvider, WindowFilter, WindowInfo, WindowProvider,
    WindowState,
};

use crate::model::{RecordedNode, Recording};

const MAX_PARENT_DEPTH: u32 = 32;

/// Shared recorded replay state.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ReplaySession {
    recording: Arc<Recording>,
}

impl ReplaySession {
    /// Build a replay session from a recording.
    #[must_use]
    pub fn new(recording: Recording) -> Self {
        Self {
            recording: Arc::new(recording),
        }
    }

    /// Load a replay session from JSON.
    ///
    /// # Errors
    /// Returns [`crate::ReplayError`] if JSON or recording invariants are invalid.
    pub fn from_json(raw: &str) -> Result<Self, crate::ReplayError> {
        Ok(Self::new(Recording::from_json(raw)?))
    }

    /// Recording data.
    #[must_use]
    pub fn recording(&self) -> &Recording {
        &self.recording
    }

    /// Recorded window handle.
    #[must_use]
    pub fn window(&self) -> ResolvedWindow {
        resolved_window(
            self.recording.window().local_handle_id(),
            self.recording.window().display_label(),
        )
    }

    /// Offline UI Automation provider.
    #[must_use]
    pub fn ui_automation_provider(&self) -> ReplayUiAutomationProvider {
        ReplayUiAutomationProvider {
            recording: Arc::clone(&self.recording),
        }
    }

    /// Offline window provider.
    #[must_use]
    pub fn window_provider(&self) -> ReplayWindowProvider {
        ReplayWindowProvider {
            recording: Arc::clone(&self.recording),
        }
    }
}

/// Offline UI Automation provider backed by recorded nodes and text outcomes.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ReplayUiAutomationProvider {
    recording: Arc<Recording>,
}

impl UiAutomationProvider for ReplayUiAutomationProvider {
    fn snapshot_tree(
        &self,
        root: &ResolvedWindow,
        options: &TreeOptions,
    ) -> impl Future<Output = PlatformResult<TreeSnapshot>> + Send {
        let result = if options.max_depth().is_some() {
            Err(platform_error(
                ErrorCode::CapabilityMissing,
                "depth-limited tree snapshots are not replayed in v0",
            ))
        } else if root.id().value() == self.recording.window().local_handle_id() {
            assistant_platform_api::Fingerprint::parse(self.recording.window().fingerprint())
                .and_then(|fingerprint| {
                    node_count(&self.recording, options.include_offscreen())
                        .map(|node_count| TreeSnapshot::new(root.clone(), fingerprint, node_count))
                })
        } else {
            Err(platform_error(
                ErrorCode::TargetNotFound,
                "recorded window handle does not match",
            ))
        };
        ready(result)
    }

    fn resolve_element(
        &self,
        scope: &ResolvedWindow,
        chain: &SelectorChain,
    ) -> impl Future<Output = PlatformResult<ResolvedElement>> + Send {
        ready(resolve_element(&self.recording, scope, chain))
    }

    fn element_bounds(
        &self,
        element: &ResolvedElement,
    ) -> impl Future<Output = PlatformResult<ElementBounds>> + Send {
        ready(element_bounds(&self.recording, element))
    }

    fn wait_for(
        &self,
        scope: &ResolvedWindow,
        query: &assistant_platform_api::ElementQuery,
        state: &assistant_platform_api::ElementState,
        timeout: Timeout,
    ) -> impl Future<Output = PlatformResult<ResolvedElement>> + Send {
        let _ = (scope, query, state, timeout);
        ready(Err(platform_error(
            ErrorCode::CapabilityMissing,
            "wait_for is not recorded in replay v0",
        )))
    }

    fn read_text(
        &self,
        element: &ResolvedElement,
    ) -> impl Future<Output = PlatformResult<String>> + Send {
        ready(read_text(&self.recording, element))
    }

    fn set_value(
        &self,
        element: &ResolvedElement,
        value: &str,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        let _ = (element, value);
        ready(Err(platform_error(
            ErrorCode::CapabilityMissing,
            "set_value is not replayed in v0",
        )))
    }

    fn edit_text(
        &self,
        element: &ResolvedElement,
        operation: &TextEditOp,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        let _ = (element, operation);
        ready(Err(platform_error(
            ErrorCode::CapabilityMissing,
            "edit_text is not replayed in v0",
        )))
    }

    fn invoke_action(
        &self,
        element: &ResolvedElement,
        action: &str,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        let _ = (element, action);
        ready(Err(platform_error(
            ErrorCode::CapabilityMissing,
            "invoke_action is not replayed in v0",
        )))
    }

    fn select(
        &self,
        element: &ResolvedElement,
        selection: &Selection,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        let _ = (element, selection);
        ready(Err(platform_error(
            ErrorCode::CapabilityMissing,
            "select is not replayed in v0",
        )))
    }

    fn scroll(
        &self,
        element: &ResolvedElement,
        target: &ScrollTarget,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        let _ = (element, target);
        ready(Err(platform_error(
            ErrorCode::CapabilityMissing,
            "scroll is not replayed in v0",
        )))
    }

    fn pointer_action(
        &self,
        coordinate_space: &CoordinateSpace,
        point: assistant_platform_api::NormalizedPoint,
        action: &PointerAction,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        let _ = (coordinate_space, point, action);
        ready(Err(platform_error(
            ErrorCode::CapabilityMissing,
            "pointer_action is not replayed in v0",
        )))
    }

    fn key_action(
        &self,
        chord: &KeyChord,
        target: &KeyTarget,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        let _ = (chord, target);
        ready(Err(platform_error(
            ErrorCode::CapabilityMissing,
            "key_action is not replayed in v0",
        )))
    }

    fn fingerprint(
        &self,
        window: &ResolvedWindow,
        scope: &assistant_platform_api::FingerprintScope,
    ) -> impl Future<Output = PlatformResult<assistant_platform_api::Fingerprint>> + Send {
        let result = if window.id().value() == self.recording.window().local_handle_id() {
            match scope {
                assistant_platform_api::FingerprintScope::WholeWindow => {
                    assistant_platform_api::Fingerprint::parse(
                        self.recording.window().fingerprint(),
                    )
                }
                _ => Err(platform_error(
                    ErrorCode::CapabilityMissing,
                    "only the recorded whole-window fingerprint is available",
                )),
            }
        } else {
            Err(platform_error(
                ErrorCode::TargetNotFound,
                "recorded window handle does not match",
            ))
        };
        ready(result)
    }
}

/// Offline window provider backed by recorded window metadata.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ReplayWindowProvider {
    recording: Arc<Recording>,
}

impl WindowProvider for ReplayWindowProvider {
    fn list_windows(
        &self,
        filter: &WindowFilter,
    ) -> impl Future<Output = PlatformResult<Vec<WindowInfo>>> + Send {
        let windows = if filter.title_regex().is_some() {
            return ready(Err(platform_error(
                ErrorCode::CapabilityMissing,
                "title-regex window filtering is not replayed in v0",
            )));
        } else if filter
            .app_id()
            .is_none_or(|app_id| app_id == self.recording.application())
        {
            vec![self.window_info()]
        } else {
            Vec::new()
        };
        ready(Ok(windows))
    }

    fn resolve_window(
        &self,
        descriptor: &assistant_platform_api::TargetDescriptor,
    ) -> impl Future<Output = PlatformResult<ResolvedWindow>> + Send {
        let result = match descriptor.validate() {
            Err(error) => Err(error),
            Ok(()) if descriptor.app_id() == self.recording.application() => Ok(self.window()),
            Ok(()) => Err(platform_error(
                ErrorCode::TargetNotFound,
                "recorded application id does not match descriptor",
            )),
        };
        ready(result)
    }

    fn window_state(
        &self,
        window: &ResolvedWindow,
    ) -> impl Future<Output = PlatformResult<WindowState>> + Send {
        let result = if window.id().value() == self.recording.window().local_handle_id() {
            Ok(WindowState::new(
                self.recording.window().is_minimized(),
                self.recording.window().is_foreground(),
                self.recording.window().is_occluded(),
            ))
        } else {
            Err(platform_error(
                ErrorCode::TargetNotFound,
                "recorded window handle does not match",
            ))
        };
        ready(result)
    }

    fn bring_to_front(
        &self,
        window: &ResolvedWindow,
        policy: FocusPolicy,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        let _ = (window, policy);
        ready(Err(platform_error(
            ErrorCode::CapabilityMissing,
            "bring_to_front is not replayed in v0",
        )))
    }

    fn capture(
        &self,
        window: &ResolvedWindow,
        options: &CaptureOptions,
    ) -> impl Future<Output = PlatformResult<ImageRef>> + Send {
        let _ = (window, options);
        ready(Err(platform_error(
            ErrorCode::CapabilityMissing,
            "capture is not replayed in v0",
        )))
    }
}

impl ReplayWindowProvider {
    fn window(&self) -> ResolvedWindow {
        resolved_window(
            self.recording.window().local_handle_id(),
            self.recording.window().display_label(),
        )
    }

    fn window_info(&self) -> WindowInfo {
        WindowInfo::new(
            self.window(),
            self.recording.application().to_string(),
            self.recording.window().title().to_string(),
        )
    }
}

fn resolve_element(
    recording: &Recording,
    scope: &ResolvedWindow,
    chain: &SelectorChain,
) -> PlatformResult<ResolvedElement> {
    if scope.id().value() != recording.window().local_handle_id() {
        return Err(platform_error(
            ErrorCode::TargetNotFound,
            "recorded window handle does not match",
        ));
    }
    if chain.candidates().is_empty() {
        return Err(platform_error(
            ErrorCode::ToolInvalidArgs,
            "selector chain is empty and can never resolve",
        ));
    }
    for candidate in chain.candidates() {
        let matches = resolve_candidate(recording, chain, candidate, 0)?;
        match matches.as_slice() {
            [] => {}
            [node] => {
                return Ok(resolved_element(node));
            }
            _ => {
                return Err(platform_error(
                    ErrorCode::TargetAmbiguous,
                    format!(
                        "selector candidate `{}` matched multiple nodes",
                        candidate.id()
                    ),
                ));
            }
        }
    }
    Err(platform_error(
        ErrorCode::TargetNotFound,
        "no selector candidate matched a recorded node",
    ))
}

fn resolve_candidate<'a>(
    recording: &'a Recording,
    chain: &SelectorChain,
    candidate: &assistant_platform_api::SelectorCandidate,
    depth: u32,
) -> PlatformResult<Vec<&'a RecordedNode>> {
    if depth > MAX_PARENT_DEPTH {
        return Err(platform_error(
            ErrorCode::ToolInvalidArgs,
            "RoleAndParent nesting exceeded the replay limit",
        ));
    }
    let matches = match (candidate.kind(), candidate.value()) {
        // ADR-0086：回放必须与 Windows 的 UIA 精确 Name 条件保持同语义；
        // 不使用 contains / regex，避免把 `形状轮廓` 当成 `形状`。
        (SelectorKind::ExactName, SelectorValue::Text(value)) if value.is_empty() => {
            return Err(platform_error(
                ErrorCode::ToolInvalidArgs,
                "ExactName candidate value must not be empty; refusing to match every node",
            ));
        }
        (SelectorKind::ExactName, SelectorValue::Text(value)) => recording
            .nodes()
            .iter()
            .filter(|node| node.name() == value)
            .collect(),
        (SelectorKind::ExactName, _) => {
            return Err(platform_error(
                ErrorCode::CapabilityMissing,
                "ExactName candidate requires SelectorValue::Text",
            ));
        }
        (SelectorKind::AutomationId, SelectorValue::Text(value)) => recording
            .nodes()
            .iter()
            .filter(|node| node.automation_id() == Some(value.as_str()))
            .collect(),
        (SelectorKind::ClassAndRole, SelectorValue::ClassAndRole { class, role }) => recording
            .nodes()
            .iter()
            .filter(|node| node.class_name() == Some(class.as_str()) && node.role() == role)
            .collect(),
        (SelectorKind::RoleAndParent, SelectorValue::RoleAndParent { role, parent_id }) => {
            let Some(parent_candidate) = chain
                .candidates()
                .iter()
                .find(|candidate| candidate.id() == parent_id)
            else {
                return Err(platform_error(
                    ErrorCode::ToolInvalidArgs,
                    format!("RoleAndParent references unknown parent candidate `{parent_id}`"),
                ));
            };
            let parent_matches =
                resolve_candidate(recording, chain, parent_candidate, depth.saturating_add(1))?;
            let parent = match parent_matches.as_slice() {
                [] => return Ok(Vec::new()),
                [parent] => parent.local_handle_id(),
                _ => {
                    return Err(platform_error(
                        ErrorCode::TargetAmbiguous,
                        format!("parent candidate `{parent_id}` matched multiple nodes"),
                    ));
                }
            };
            recording
                .nodes()
                .iter()
                .filter(|node| node.role() == role && node.parent_handle_id() == Some(parent))
                .collect()
        }
        _ => Vec::new(),
    };
    Ok(matches)
}

fn read_text(recording: &Recording, element: &ResolvedElement) -> PlatformResult<String> {
    let handle = element.id().value();
    if !recording
        .nodes()
        .iter()
        .any(|node| node.local_handle_id() == handle)
    {
        return Err(platform_error(
            ErrorCode::TargetNotFound,
            "element handle is not present in recording",
        ));
    }
    recording
        .read_text()
        .iter()
        .find(|outcome| outcome.element_handle_id() == handle)
        .map(|outcome| outcome.text().to_string())
        .ok_or_else(|| {
            platform_error(
                ErrorCode::CapabilityMissing,
                "element has no recorded text outcome",
            )
        })
}

fn element_bounds(
    recording: &Recording,
    element: &ResolvedElement,
) -> PlatformResult<ElementBounds> {
    let handle = element.id().value();
    let node = recording
        .nodes()
        .iter()
        .find(|node| node.local_handle_id() == handle)
        .ok_or_else(|| {
            platform_error(
                ErrorCode::TargetNotFound,
                "element handle is not present in recording",
            )
        })?;
    let [left, top, right, bottom] = node.bounds();
    let left = i32::try_from(left).map_err(|_| {
        platform_error(
            ErrorCode::Fatal,
            "recorded bounds left edge does not fit in i32",
        )
    })?;
    let top = i32::try_from(top).map_err(|_| {
        platform_error(
            ErrorCode::Fatal,
            "recorded bounds top edge does not fit in i32",
        )
    })?;
    let right = i32::try_from(right).map_err(|_| {
        platform_error(
            ErrorCode::Fatal,
            "recorded bounds right edge does not fit in i32",
        )
    })?;
    let bottom = i32::try_from(bottom).map_err(|_| {
        platform_error(
            ErrorCode::Fatal,
            "recorded bounds bottom edge does not fit in i32",
        )
    })?;
    if right <= left || bottom <= top {
        return Err(platform_error(
            ErrorCode::TargetUnresponsive,
            "recorded element bounds are empty",
        ));
    }
    ElementBounds::new(left, top, right, bottom)
}

fn resolved_window(handle: u64, display_label: &str) -> ResolvedWindow {
    ResolvedWindow::new(
        assistant_platform_api::LocalHandleId::new(handle),
        display_label.to_string(),
    )
}

fn resolved_element(node: &RecordedNode) -> ResolvedElement {
    ResolvedElement::new(
        assistant_platform_api::LocalHandleId::new(node.local_handle_id()),
        assistant_platform_api::LocalHandleId::new(node.parent_handle_id().unwrap_or(0)),
        node.role().to_string(),
    )
}

fn node_count(recording: &Recording, include_offscreen: bool) -> PlatformResult<u32> {
    let count = recording
        .nodes()
        .iter()
        .filter(|node| include_offscreen || !node.is_offscreen())
        .count();
    u32::try_from(count).map_err(|_| {
        platform_error(
            ErrorCode::ToolInvalidArgs,
            "recorded node count does not fit in u32",
        )
    })
}

fn platform_error(code: ErrorCode, message: impl Into<String>) -> PlatformError {
    PlatformError::new(code, message)
}
