//! Shape-group second-level flyout handlers for Paint (ADR-0097).
//!
//! Responsibilities:
//! - implement `paint.outline.select` and `paint.fill.select`;
//! - validate the group-local `path`, expand the shape gallery and the target
//!   flyout, invoke the matching `MenuItem`, and record the local selection.
//!
//! Boundaries:
//! - does not decide policy or approval, and does not write pixels;
//! - does not guess UI state: the outputs always carry `ui_state_readback=false`.
//!
//! Invariants:
//! 1. a path element must be in this group's own vocabulary, otherwise the call
//!    fails closed with the existing `ToolInvalidArgs`;
//! 2. the previous leaf is only ever the locally recorded value, never a
//!    `ToggleState` read-back (that signal is known to stick).
//!
//! Related documents: ADR-0096, ADR-0097, and `docs/memory/apps/paint.md`
//! sections 13.11 / 13.12 / 13.19.

use std::time::Instant;

use assistant_tool_bus::{ToolBusError, ToolOutput};
use serde_json::{Map, Value, json};

use super::paint_handler_support::{
    FILL_GROUP_VOCABULARY, OUTLINE_GROUP_VOCABULARY, elapsed_ms, fill_target, outline_target,
    required_selection_path, unavailable,
};
use super::{
    PaintHandlerContext, PaintSelectionState, TARGET_MAIN_WINDOW, TOOL_FILL_SELECT,
    TOOL_OUTLINE_SELECT,
};
use crate::handler_support as support;

/// State lives in `paint_handlers`; this enum selects which record to read/write.
#[derive(Debug, Clone, Copy)]
pub(super) enum ShapeGroupFlyout {
    Outline,
    Fill,
}

impl PaintSelectionState {
    /// Read one shape-group flyout record (outline / fill).
    pub(super) fn shape_group_flyout_leaf(&self, flyout: ShapeGroupFlyout) -> Option<String> {
        match flyout {
            ShapeGroupFlyout::Outline => self.outline_leaf.clone(),
            ShapeGroupFlyout::Fill => self.fill_leaf.clone(),
        }
    }

    /// Write one shape-group flyout record.
    pub(super) fn set_shape_group_flyout_leaf(&mut self, flyout: ShapeGroupFlyout, leaf: String) {
        match flyout {
            ShapeGroupFlyout::Outline => self.outline_leaf = Some(leaf),
            ShapeGroupFlyout::Fill => self.fill_leaf = Some(leaf),
        }
    }
}

impl<P> PaintHandlerContext<P>
where
    P: assistant_platform_api::WindowProvider
        + assistant_platform_api::UiAutomationProvider
        + Send
        + Sync
        + 'static,
{
    /// `paint.outline.select`: pick one shape outline style.
    pub(super) fn select_outline_output(
        &self,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        self.select_shape_group_flyout_output(
            arguments,
            TOOL_OUTLINE_SELECT,
            OUTLINE_GROUP_VOCABULARY,
            outline_target,
            ShapeGroupFlyout::Outline,
        )
    }

    /// `paint.fill.select`: pick one shape fill style.
    pub(super) fn select_fill_output(
        &self,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        self.select_shape_group_flyout_output(
            arguments,
            TOOL_FILL_SELECT,
            FILL_GROUP_VOCABULARY,
            fill_target,
            ShapeGroupFlyout::Fill,
        )
    }

    /// Shared body of the two shape-group flyout tools.
    ///
    /// Validates the group-local path, expands the shape gallery plus the target
    /// flyout, invokes the matching `MenuItem`, and returns the locally recorded
    /// previous/selected leaf. It intentionally never reports a `ToggleState`
    /// read-back, because Paint keeps that signal stuck after a tool switch.
    fn select_shape_group_flyout_output(
        &self,
        arguments: &Map<String, Value>,
        tool: &str,
        vocabulary: &[&str],
        resolve_target: fn(&str) -> Option<&'static str>,
        flyout: ShapeGroupFlyout,
    ) -> Result<ToolOutput, ToolBusError> {
        let path = required_selection_path(arguments, tool, vocabulary, 1)?;
        let leaf = path
            .first()
            .ok_or_else(|| support::invalid_arguments(tool, "path must not be empty"))?
            .as_str();
        let target = resolve_target(leaf).ok_or_else(|| {
            support::invalid_arguments(tool, format!("path `{leaf}` has no declared Paint target"))
        })?;
        let window = self.resolve_window(TARGET_MAIN_WINDOW, tool)?;
        let item = match flyout {
            ShapeGroupFlyout::Outline => {
                self.wait_for_outline_flyout_item(&window, target, tool)?
            }
            ShapeGroupFlyout::Fill => self.wait_for_fill_flyout_item(&window, target, tool)?,
        };
        let previous_leaf = {
            let state = self
                .selection
                .lock()
                .map_err(|_| unavailable("paint selection state"))?;
            state
                .shape_group_flyout_leaf(flyout)
                .unwrap_or_else(|| "unknown".to_owned())
        };
        let before = self.fingerprint_event(&window, tool)?;
        let started = Instant::now();
        self.invoke_element(&item, "invoke", tool)?;
        {
            let mut state = self
                .selection
                .lock()
                .map_err(|_| unavailable("paint selection state"))?;
            state.set_shape_group_flyout_leaf(flyout, leaf.to_owned());
        }
        let after = self.fingerprint_event(&window, tool)?;
        Ok(ToolOutput::json(json!({
            "selected_leaf": leaf,
            "previous_leaf": previous_leaf,
            "selected_path": path,
            "path_depth": 1,
            "ui_state_readback": false,
            "fingerprint": after.as_str(),
            "previous_fingerprint": before.as_str(),
            "elapsed_ms": elapsed_ms(started),
        })))
    }
}
