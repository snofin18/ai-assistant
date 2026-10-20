//! File-open and zoom settling methods for Paint handlers.

use super::paint_handler_actions::{
    FILE_DIALOG_INTERVAL_MS, OPEN_CANVAS_ATTEMPTS, OPEN_CANVAS_STABLE_CAPTURES, OPEN_ZOOM_ATTEMPTS,
    OPEN_ZOOM_PERCENT, OPEN_ZOOM_RATIO, OPEN_ZOOM_RATIO_TOLERANCE,
};
use super::paint_handler_support::{parse_canvas_size, verify_failed};
use super::{
    PaintHandlerContext, TARGET_CANVAS, TARGET_MAIN_WINDOW, TARGET_STATUS_BAR,
    TARGET_ZOOM_VALUES_COMBO,
};
use assistant_platform_api::{
    ImageRef, ResolvedWindow, Selection, UiAutomationProvider, WindowProvider,
};
use assistant_tool_bus::ToolBusError;
use std::time::Duration;

impl<P> PaintHandlerContext<P>
where
    P: WindowProvider + UiAutomationProvider + Send + Sync + 'static,
{
    pub(super) fn read_open_canvas(
        &self,
        expected: (u32, u32),
        tool: &str,
    ) -> Result<(u32, u32, f64), ToolBusError> {
        let window = self.resolve_window(TARGET_MAIN_WINDOW, tool)?;
        let mut last_observed = "document size did not match the fixture".to_owned();
        for _attempt in 0..OPEN_CANVAS_ATTEMPTS {
            let status = self.resolve_element(TARGET_STATUS_BAR, &window, tool)?;
            let text = self.read_element_text(&status, tool)?;
            if let Some((width, height)) = parse_canvas_size(&text)
                && (width, height) == expected
            {
                let canvas = self.resolve_element(TARGET_CANVAS, &window, tool)?;
                let bounds = self.element_bounds(&canvas, tool)?;
                let zoom_ratio = f64::from(bounds.width()) / f64::from(width);
                let height_ratio = f64::from(bounds.height()) / f64::from(height);
                let is_expected_zoom = (zoom_ratio - OPEN_ZOOM_RATIO).abs()
                    <= OPEN_ZOOM_RATIO_TOLERANCE
                    && (height_ratio - OPEN_ZOOM_RATIO).abs() <= OPEN_ZOOM_RATIO_TOLERANCE;
                if zoom_ratio.is_finite()
                    && zoom_ratio > 0.0
                    && height_ratio.is_finite()
                    && is_expected_zoom
                {
                    self.wait_for_stable_capture(&window, tool)?;
                    return Ok((width, height, zoom_ratio));
                }
                last_observed =
                    format!("zoom ratio {zoom_ratio:.4}, height ratio {height_ratio:.4}");
            }
            std::thread::sleep(Duration::from_millis(FILE_DIALOG_INTERVAL_MS));
        }
        Err(verify_failed(format!(
            "{tool}: opened canvas did not settle at {}% for {}x{}; last observed: {last_observed}",
            OPEN_ZOOM_PERCENT, expected.0, expected.1,
        )))
    }

    pub(super) fn set_open_zoom(
        &self,
        window: &ResolvedWindow,
        tool: &str,
    ) -> Result<(), ToolBusError> {
        let zoom_combo = self.resolve_element(TARGET_ZOOM_VALUES_COMBO, window, tool)?;
        let mut last_value = String::new();
        for _attempt in 0..OPEN_ZOOM_ATTEMPTS {
            self.invoke_element(&zoom_combo, "expand", tool)?;
            // Direct content-view children start with EditableText at index 0;
            // the measured 50% ListItem is therefore index 12, not the raw menu offset.
            self.select_element(&zoom_combo, &Selection::ByIndex(12), tool)?;
            self.invoke_element(&zoom_combo, "collapse", tool)?;
            last_value = self.read_element_text(&zoom_combo, tool)?;
            if last_value.trim() == format!("{OPEN_ZOOM_PERCENT}%") {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(FILE_DIALOG_INTERVAL_MS));
        }
        Err(verify_failed(format!(
            "{tool}: zoom did not read back as {OPEN_ZOOM_PERCENT}%; last value was `{last_value}`"
        )))
    }

    pub(super) fn wait_for_stable_capture(
        &self,
        window: &ResolvedWindow,
        tool: &str,
    ) -> Result<ImageRef, ToolBusError> {
        let mut previous_blob_id = None;
        let mut stable_capture_count = 0_u32;
        let mut last_blob_id = String::new();
        for _attempt in 0..OPEN_CANVAS_ATTEMPTS {
            let capture = self.capture_window(window, tool)?;
            capture.blob_id().clone_into(&mut last_blob_id);
            if previous_blob_id.as_deref() == Some(last_blob_id.as_str()) {
                stable_capture_count = stable_capture_count.saturating_add(1);
            } else {
                previous_blob_id = Some(last_blob_id.clone());
                stable_capture_count = 1;
            }
            if stable_capture_count >= OPEN_CANVAS_STABLE_CAPTURES {
                return Ok(capture);
            }
            std::thread::sleep(Duration::from_millis(FILE_DIALOG_INTERVAL_MS));
        }
        Err(verify_failed(format!(
            "{tool}: window pixels did not stabilize after {OPEN_CANVAS_ATTEMPTS} attempts; \
             last blob was `{last_blob_id}`"
        )))
    }
}
