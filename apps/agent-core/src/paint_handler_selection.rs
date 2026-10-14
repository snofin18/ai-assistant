//! Selection state and canvas point resolution for the Paint T3.1 handlers.
//!
//! Responsibilities:
//! - hold and read back the logical tool / colour / layer selection;
//! - hand out monotonically increasing document generations;
//! - map a resolved canvas point back to its physical screen point.
//!
//! Boundaries:
//! - does not decide policy or approval;
//! - does not touch the platform: every value here comes from a prior step.
//!
//! Invariants:
//! 1. a draw step only runs on the tool / colour / layer captured beforehand;
//! 2. a canvas point that was never resolved, or that disagrees with the draw
//!    arguments, fails closed instead of being approximated;
//! 3. the document generation never wraps (`u64::MAX` fails closed).
//!
//! Related documents: `apps/agent-core/src/paint_handlers.rs` and the measured
//! Paint 11.2605.81.0 palette contract.
use assistant_tool_bus::ToolBusError;

use super::paint_handler_support::{unavailable, verify_failed};
use super::{
    CanvasPoint, DrawRequest, PAINT_PALETTE, PaintHandlerContext, PaintLayerSelection,
    PaintSelectionState, PhysicalPointPx, TOOL_CANVAS_DRAW_RECTANGLE, TOOL_DOCUMENT_NEW,
};

impl<P> PaintHandlerContext<P> {
    pub(super) fn reset_selection(&self) -> Result<(), ToolBusError> {
        *self
            .selection
            .lock()
            .map_err(|_| unavailable("paint selection state"))? = PaintSelectionState::default();
        Ok(())
    }

    pub(super) fn selection_tool(&self) -> Result<Option<String>, ToolBusError> {
        Ok(self
            .selection
            .lock()
            .map_err(|_| unavailable("paint selection state"))?
            .tool
            .clone())
    }

    pub(super) fn selection_color(&self) -> Result<Option<String>, ToolBusError> {
        Ok(self
            .selection
            .lock()
            .map_err(|_| unavailable("paint selection state"))?
            .color
            .clone())
    }

    pub(super) fn selection_layer_id(&self) -> Result<Option<String>, ToolBusError> {
        Ok(self
            .selection
            .lock()
            .map_err(|_| unavailable("paint selection state"))?
            .layer
            .as_ref()
            .map(|layer| layer.id.clone()))
    }

    pub(super) fn next_document_generation(&self) -> Result<u64, ToolBusError> {
        let mut generation = self
            .document_generation
            .lock()
            .map_err(|_| unavailable("paint document generation state"))?;
        let next = generation
            .checked_add(1)
            .ok_or_else(|| ToolBusError::EnvelopeAssembly {
                tool: TOOL_DOCUMENT_NEW.to_owned(),
                reason: "document generation reached u64::MAX".to_owned(),
            })?;
        *generation = next;
        drop(generation);
        Ok(next)
    }

    pub(super) fn resolved_point(
        &self,
        canvas: CanvasPoint,
        slot: ResolvedPointSlot,
        tool: &str,
    ) -> Result<PhysicalPointPx, ToolBusError> {
        let state = self
            .selection
            .lock()
            .map_err(|_| unavailable("paint selection state"))?;
        let point = match slot {
            ResolvedPointSlot::Start => state.start_point.as_ref(),
            ResolvedPointSlot::End => state.end_point.as_ref(),
        }
        .ok_or_else(|| {
            verify_failed(format!(
                "{tool}: canvas point ({}, {}) was not resolved by a prior step",
                canvas.horizontal, canvas.vertical
            ))
        })?;
        if point.canvas.horizontal != canvas.horizontal || point.canvas.vertical != canvas.vertical
        {
            return Err(verify_failed(format!(
                "{tool}: resolved {} point ({}, {}) does not match draw arguments ({}, {})",
                slot.label(),
                point.canvas.horizontal,
                point.canvas.vertical,
                canvas.horizontal,
                canvas.vertical
            )));
        }
        let resolved = point.screen;
        drop(state);
        Ok(resolved)
    }

    pub(super) fn verified_active_layer(
        &self,
        request: &DrawRequest,
    ) -> Result<PaintLayerSelection, ToolBusError> {
        let state = self
            .selection
            .lock()
            .map_err(|_| unavailable("paint selection state"))?;
        if state.tool.as_deref() != Some(request.expected_tool.as_str()) {
            return Err(verify_failed(format!(
                "{TOOL_CANVAS_DRAW_RECTANGLE}: active tool is {:?}, expected `{}`",
                state.tool, request.expected_tool
            )));
        }
        if state.color.as_deref() != Some(request.expected_color.as_str()) {
            return Err(verify_failed(format!(
                "{TOOL_CANVAS_DRAW_RECTANGLE}: active color is {:?}, expected `{}`",
                state.color, request.expected_color
            )));
        }
        let layer = state.layer.clone().ok_or_else(|| {
            verify_failed(format!(
                "{TOOL_CANVAS_DRAW_RECTANGLE}: active layer is unavailable"
            ))
        })?;
        if layer.id != request.expected_layer_id {
            return Err(verify_failed(format!(
                "{TOOL_CANVAS_DRAW_RECTANGLE}: active layer is not `{}`",
                request.expected_layer_id
            )));
        }
        drop(state);
        Ok(layer)
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum ResolvedPointSlot {
    Start,
    End,
}

impl ResolvedPointSlot {
    const fn label(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::End => "end",
        }
    }
}

pub(super) fn palette_index(red: u8, green: u8, blue: u8) -> Option<u32> {
    PAINT_PALETTE
        .iter()
        .find_map(|&(rgb, index)| (rgb == (red, green, blue)).then_some(index))
}

pub(super) fn paint_layer_contract(layer_id: &str) -> Option<(u32, &'static str)> {
    match layer_id {
        "layer-1" => Some((0, "Layer 1")),
        _ => None,
    }
}
