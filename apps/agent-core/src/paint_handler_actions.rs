//! UIA, input, and capture primitives for the Paint T3.1 handlers.
//!
//! Responsibilities:
//! - resolve adapter targets inside an already-resolved window scope;
//! - run the platform primitives the seven T3.1 tools need (read / invoke /
//!   select / key / drag / capture);
//! - gate every synthetic input behind the shared target lease.
//!
//! Boundaries:
//! - does not decide policy or approval;
//! - does not parse tool arguments or assemble tool envelopes.
//!
//! Invariants:
//! 1. every platform call is polled through `support::poll_immediate`;
//! 2. platform failures are mapped to `ErrorCode`-bearing `ToolBusError`s;
//! 3. input primitives take an exclusive window lease keyed by app + window.
//!
//! Related documents: ADR-0067 (explicit `CoordinateSpace`), ADR-0085
//! (`element_bounds`), and `apps/agent-core/src/paint_handlers.rs`.
use crate::handler_support as support;
use crate::target_lease::window_lease_key;
use assistant_platform_api::{
    CaptureOptions, CoordinateSpace, ElementBounds, Fingerprint, FingerprintScope, ImageRef,
    KeyChord, KeyModifier, KeyTarget, PointerAction, ResolvedElement, ResolvedWindow, Selection,
    UiAutomationProvider, WindowProvider,
};
use assistant_storage::BlobId;
use assistant_tool_bus::ToolBusError;
use std::time::Duration;

use super::paint_handler_support::normalized_point;
use super::{
    DragRequest, PaintHandlerContext, TARGET_LAYER_ITEM, TARGET_LAYERS_TOGGLE,
    TARGET_RECTANGLE_TOOL, TARGET_SHAPES_TOGGLE,
};

/// 形状库展开后的有界重解次数与间隔（实测形状项**异步物化**，见 `docs/memory/apps/paint.md` §11）。
///
/// 上限 = 20 × 50 ms = 1 s：既给 Paint 留物化时间，也是**硬上限**（ADR-0063 同族：不许无限轮询）。
const SHAPES_GALLERY_ATTEMPTS: usize = 20;
const SHAPES_GALLERY_INTERVAL_MS: u64 = 50;

impl<P> PaintHandlerContext<P>
where
    P: WindowProvider + UiAutomationProvider + Send + Sync + 'static,
{
    pub(super) fn resolve_window(
        &self,
        target: &str,
        tool: &str,
    ) -> Result<ResolvedWindow, ToolBusError> {
        let descriptor =
            self.targets
                .descriptor(target)
                .map_err(|error| ToolBusError::EnvelopeAssembly {
                    tool: tool.to_owned(),
                    reason: error.to_string(),
                })?;
        let result = support::poll_immediate(
            self.platform.resolve_window(descriptor),
            tool,
            "resolve_window",
        )?;
        result.map_err(|error| support::map_platform_error(tool, &error))
    }

    pub(super) fn resolve_element(
        &self,
        target: &str,
        scope: &ResolvedWindow,
        tool: &str,
    ) -> Result<ResolvedElement, ToolBusError> {
        let descriptor =
            self.targets
                .descriptor(target)
                .map_err(|error| ToolBusError::EnvelopeAssembly {
                    tool: tool.to_owned(),
                    reason: error.to_string(),
                })?;
        let chain =
            assistant_platform_api::SelectorChain::new(descriptor.element_candidates().to_vec());
        let result = support::poll_immediate(
            self.platform.resolve_element(scope, &chain),
            tool,
            "resolve_element",
        )?;
        result.map_err(|error| support::map_platform_error(tool, &error))
    }

    pub(super) fn resolve_layer_list(
        &self,
        window: &ResolvedWindow,
        tool: &str,
    ) -> Result<(ResolvedElement, bool), ToolBusError> {
        match self.resolve_element(TARGET_LAYER_ITEM, window, tool) {
            Ok(layer_list) => Ok((layer_list, false)),
            Err(ToolBusError::UnknownTool { .. }) => {
                let toggle = self.resolve_element(TARGET_LAYERS_TOGGLE, window, tool)?;
                self.invoke_element(&toggle, "toggle", tool)?;
                let layer_list = self.resolve_element(TARGET_LAYER_ITEM, window, tool)?;
                Ok((layer_list, true))
            }
            Err(error) => Err(error),
        }
    }

    /// 解析形状库里的目标形状项；折叠时先展开再**有界轮询**重解。
    ///
    /// 实测（2026-10-14 真机探针）：形状库折叠时形状项**还没物化**，直接解析只会
    /// `TargetNotFound`；容器内那个 `AutomationId=CollapsedButton` 的按钮用
    /// `InvokePattern` 能展开（同一元素上的 `ExpandCollapsePattern::Expand()` 在真机上失败），
    /// 展开后 `GridView` 是**异步**填充的，所以要按固定次数 × 间隔重解，而不是解一次就算。
    ///
    /// 错误语义：折叠判定只接受 `ToolBusError::UnknownTool`（= 平台 `TargetNotFound`
    /// 经 `handler_support::map_platform_error` 映射后的形态）；其它错误直接上抛，
    /// 免得把真正的管线故障当成"还没展开"。1 s 内仍没物化 → fail-closed。
    pub(super) fn wait_for_shape_item(
        &self,
        window: &ResolvedWindow,
        tool: &str,
    ) -> Result<ResolvedElement, ToolBusError> {
        match self.resolve_element(TARGET_RECTANGLE_TOOL, window, tool) {
            Ok(item) => return Ok(item),
            Err(ToolBusError::UnknownTool { .. }) => {}
            Err(error) => return Err(error),
        }
        let toggle = self.resolve_element(TARGET_SHAPES_TOGGLE, window, tool)?;
        self.invoke_element(&toggle, "invoke", tool)?;
        let mut last_error = None;
        for _attempt in 0..SHAPES_GALLERY_ATTEMPTS {
            match self.resolve_element(TARGET_RECTANGLE_TOOL, window, tool) {
                Ok(item) => return Ok(item),
                Err(error) => last_error = Some(error),
            }
            std::thread::sleep(Duration::from_millis(SHAPES_GALLERY_INTERVAL_MS));
        }
        Err(last_error.unwrap_or_else(|| {
            support::invalid_arguments(tool, "shape gallery did not materialize after expanding")
        }))
    }

    pub(super) fn element_bounds(
        &self,
        element: &ResolvedElement,
        tool: &str,
    ) -> Result<ElementBounds, ToolBusError> {
        let result = support::poll_immediate(
            self.platform.element_bounds(element),
            tool,
            "element_bounds",
        )?;
        result.map_err(|error| support::map_platform_error(tool, &error))
    }

    pub(super) fn read_element_text(
        &self,
        element: &ResolvedElement,
        tool: &str,
    ) -> Result<String, ToolBusError> {
        let result = support::poll_immediate(self.platform.read_text(element), tool, "read_text")?;
        result.map_err(|error| support::map_platform_error(tool, &error))
    }

    pub(super) fn invoke_element(
        &self,
        element: &ResolvedElement,
        action: &str,
        tool: &str,
    ) -> Result<(), ToolBusError> {
        let result = support::poll_immediate(
            self.platform.invoke_action(element, action),
            tool,
            "invoke_action",
        )?;
        result.map_err(|error| support::map_platform_error(tool, &error))
    }

    pub(super) fn select_element(
        &self,
        element: &ResolvedElement,
        selection: &Selection,
        tool: &str,
    ) -> Result<(), ToolBusError> {
        let result =
            support::poll_immediate(self.platform.select(element, selection), tool, "select")?;
        result.map_err(|error| support::map_platform_error(tool, &error))
    }

    pub(super) fn fingerprint_event(
        &self,
        window: &ResolvedWindow,
        tool: &str,
    ) -> Result<Fingerprint, ToolBusError> {
        let result = support::poll_immediate(
            self.platform
                .fingerprint(window, &FingerprintScope::WholeWindow),
            tool,
            "fingerprint",
        )?;
        result.map_err(|error| support::map_platform_error(tool, &error))
    }

    pub(super) fn send_key(
        &self,
        task_id: &str,
        window: &ResolvedWindow,
        key: &str,
        modifiers: Vec<KeyModifier>,
        tool: &str,
    ) -> Result<(), ToolBusError> {
        let lease_key = window_lease_key(&self.app_id, window.id(), tool)?;
        self.input_leases.run(task_id, lease_key, tool, || {
            let chord = KeyChord::new(key.to_owned(), modifiers);
            let result = support::poll_immediate(
                self.platform
                    .key_action(&chord, &KeyTarget::Window(window.clone())),
                tool,
                "key_action",
            )?;
            result.map_err(|error| support::map_platform_error(tool, &error))
        })
    }

    pub(super) fn drag(
        &self,
        task_id: &str,
        window: &ResolvedWindow,
        coordinate_space: &CoordinateSpace,
        points: DragRequest,
        tool: &str,
    ) -> Result<(), ToolBusError> {
        let start = normalized_point(points.start, coordinate_space, tool)?;
        let drop_at = normalized_point(points.end, coordinate_space, tool)?;
        let action = PointerAction::DragTo {
            drop_at,
            drop_coordinate_space: coordinate_space.clone(),
        };
        let lease_key = window_lease_key(&self.app_id, window.id(), tool)?;
        self.input_leases.run(task_id, lease_key, tool, || {
            let result = support::poll_immediate(
                self.platform
                    .pointer_action(coordinate_space, start, &action),
                tool,
                "pointer_action",
            )?;
            result.map_err(|error| support::map_platform_error(tool, &error))
        })
    }

    pub(super) fn capture_window(
        &self,
        window: &ResolvedWindow,
        tool: &str,
    ) -> Result<ImageRef, ToolBusError> {
        let result = support::poll_immediate(
            self.platform.capture(window, &CaptureOptions::new(true)),
            tool,
            "capture",
        )?;
        let image = result.map_err(|error| support::map_platform_error(tool, &error))?;
        BlobId::parse(image.blob_id()).map_err(|error| ToolBusError::EnvelopeAssembly {
            tool: tool.to_owned(),
            reason: format!("capture returned an invalid content address: {error}"),
        })?;
        Ok(image)
    }
}
