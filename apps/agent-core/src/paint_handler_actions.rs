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

use super::paint_handler_support::normalized_point;
use super::{DragRequest, PaintHandlerContext, TARGET_LAYER_ITEM, TARGET_LAYERS_TOGGLE};

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
