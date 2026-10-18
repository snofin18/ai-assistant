//! Shape-group second-level flyout handlers for Paint (ADR-0097 / ADR-0098).
//!
//! Responsibilities:
//! - implement `paint.outline.select`, `paint.fill.select`, and `paint.size.select`;
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
//! 3. `paint.size.select` resolves its items by `AutomationId` (ADR-0098 D3);
//!    it never selects a shape on the caller's behalf, so a shape group with no
//!    active shape fails closed with the existing `TargetNotFound`.
//!
//! Related documents: ADR-0096, ADR-0097, ADR-0098, and `docs/memory/apps/paint.md`
//! sections 13.11 / 13.12 / 13.14 / 13.16 / 13.19 / 13.21.

use std::time::Instant;

use assistant_tool_bus::{ToolBusError, ToolOutput};
use serde_json::{Map, Value, json};

use super::paint_handler_support::{
    FILL_GROUP_VOCABULARY, OUTLINE_GROUP_VOCABULARY, SIZE_GROUP_VOCABULARY, elapsed_ms,
    fill_target, outline_target, required_selection_path, size_target, unavailable,
};
use super::{
    PaintHandlerContext, PaintSelectionState, TARGET_MAIN_WINDOW, TOOL_FILL_SELECT,
    TOOL_OUTLINE_SELECT, TOOL_SIZE_SELECT,
};
use crate::handler_support as support;

/// State lives in `paint_handlers`; this enum selects which record to read/write.
#[derive(Debug, Clone, Copy)]
pub(super) enum ShapeGroupFlyout {
    Outline,
    Fill,
    Size,
}

impl PaintSelectionState {
    /// Read one shape-group flyout record (outline / fill / size).
    pub(super) fn shape_group_flyout_leaf(&self, flyout: ShapeGroupFlyout) -> Option<String> {
        match flyout {
            ShapeGroupFlyout::Outline => self.outline_leaf.clone(),
            ShapeGroupFlyout::Fill => self.fill_leaf.clone(),
            ShapeGroupFlyout::Size => self.size_leaf.clone(),
        }
    }

    /// Write one shape-group flyout record.
    pub(super) fn set_shape_group_flyout_leaf(&mut self, flyout: ShapeGroupFlyout, leaf: String) {
        match flyout {
            ShapeGroupFlyout::Outline => self.outline_leaf = Some(leaf),
            ShapeGroupFlyout::Fill => self.fill_leaf = Some(leaf),
            ShapeGroupFlyout::Size => self.size_leaf = Some(leaf),
        }
    }
}

impl ShapeGroupFlyout {
    /// Model-visible tool name for this flyout.
    const fn tool(self) -> &'static str {
        match self {
            Self::Outline => TOOL_OUTLINE_SELECT,
            Self::Fill => TOOL_FILL_SELECT,
            Self::Size => TOOL_SIZE_SELECT,
        }
    }

    /// Group-local vocabulary; each group stays independent even when names repeat.
    const fn vocabulary(self) -> &'static [&'static str] {
        match self {
            Self::Outline => OUTLINE_GROUP_VOCABULARY,
            Self::Fill => FILL_GROUP_VOCABULARY,
            Self::Size => SIZE_GROUP_VOCABULARY,
        }
    }

    /// Maps a validated leaf to its adapter-pack target id.
    fn resolve_target(self, leaf: &str) -> Option<&'static str> {
        match self {
            Self::Outline => outline_target(leaf),
            Self::Fill => fill_target(leaf),
            Self::Size => size_target(leaf),
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
        self.select_shape_group_flyout_output(arguments, ShapeGroupFlyout::Outline)
    }

    /// `paint.fill.select`: pick one shape fill style.
    pub(super) fn select_fill_output(
        &self,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        self.select_shape_group_flyout_output(arguments, ShapeGroupFlyout::Fill)
    }

    /// `paint.size.select`: pick one shape stroke width (1 / 3 / 5 / 8 px).
    ///
    /// Unlike the outline / fill groups this one resolves each item by the
    /// measured `AutomationId`; see ADR-0098 D3.
    pub(super) fn select_size_output(
        &self,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        self.select_shape_group_flyout_output(arguments, ShapeGroupFlyout::Size)
    }

    /// Shared body of the shape-group flyout tools.
    ///
    /// Validates the group-local path, expands the shape gallery plus the target
    /// flyout, invokes the matching `MenuItem`, and returns the locally recorded
    /// previous/selected leaf. It intentionally never reports a `ToggleState`
    /// read-back, because Paint keeps that signal stuck after a tool switch.
    fn select_shape_group_flyout_output(
        &self,
        arguments: &Map<String, Value>,
        flyout: ShapeGroupFlyout,
    ) -> Result<ToolOutput, ToolBusError> {
        let tool = flyout.tool();
        let path = required_selection_path(arguments, tool, flyout.vocabulary(), 1)?;
        let leaf = path
            .first()
            .ok_or_else(|| support::invalid_arguments(tool, "path must not be empty"))?
            .as_str();
        let target = flyout.resolve_target(leaf).ok_or_else(|| {
            support::invalid_arguments(tool, format!("path `{leaf}` has no declared Paint target"))
        })?;
        let window = self.resolve_window(TARGET_MAIN_WINDOW, tool)?;
        let item = match flyout {
            ShapeGroupFlyout::Outline => {
                self.wait_for_outline_flyout_item(&window, target, tool)?
            }
            ShapeGroupFlyout::Fill => self.wait_for_fill_flyout_item(&window, target, tool)?,
            ShapeGroupFlyout::Size => self.wait_for_size_flyout_item(&window, target, tool)?,
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
