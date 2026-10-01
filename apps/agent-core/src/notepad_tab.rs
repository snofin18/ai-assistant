//! Tab-count observation for the Notepad handlers.
//!
//! Responsibilities:
//! - own the `notepad.tab.new` tool: read the adapter's optional `tab_count`
//!   readout, invoke the add-tab button, and prove the count increased by one;
//! - parse the `Tabs: <n>` readout that fixture-style adapters expose.
//!
//! Boundaries:
//! - does not decide policy, execute other tools, or resolve windows itself;
//! - does not fabricate a count when the adapter declares no readout.
//!
//! Invariants:
//! 1. the action is never performed when the count cannot be read (fail-closed);
//! 2. success requires `after == before + 1`, never "it probably worked";
//! 3. a readout that is not `Tabs: <n>` is `CapabilityMissing`, not zero.
//!
//! Split out of `notepad_handlers.rs` so that file stays under the 600-line
//! guideline (gov section 5.4). This is a child module of `notepad_handlers`,
//! so it may use the parent's private helpers.

/// Creates a new tab and proves the tab count actually increased by one.
///
/// The count is read from the adapter's **optional** `tab_count` target. An
/// adapter that does not declare it fails closed in `resolve_element` with that
/// target's name, so the action is never performed unverifiably.
impl<P> super::NotepadHandlerContext<P>
where
    P: assistant_platform_api::WindowProvider
        + assistant_platform_api::UiAutomationProvider
        + Send
        + Sync
        + 'static,
{
    pub(super) fn new_tab_output(
        &self,
    ) -> Result<assistant_tool_bus::ToolOutput, assistant_tool_bus::ToolBusError> {
        let window = self.resolve_window(super::MAIN_WINDOW_TARGET, super::TOOL_TAB_NEW)?;
        let count_element = self.resolve_element(
            crate::notepad_targets::TAB_COUNT_TARGET,
            &window,
            super::TOOL_TAB_NEW,
        )?;
        let before =
            parse_tab_count(&self.read_element_text(&count_element, super::TOOL_TAB_NEW)?)?;
        let add_button = self.resolve_element(
            crate::notepad_targets::ADD_TAB_BUTTON_TARGET,
            &window,
            super::TOOL_TAB_NEW,
        )?;
        self.invoke_element(&add_button, "invoke", super::TOOL_TAB_NEW)?;
        let after = parse_tab_count(&self.read_element_text(&count_element, super::TOOL_TAB_NEW)?)?;
        if after != before.saturating_add(1) {
            return Err(assistant_tool_bus::ToolBusError::Mcp {
                code: -32_004,
                message: format!(
                    "{}: tab count did not increase by one (before={before}, after={after}) \
                     (VerifyFailed)",
                    super::TOOL_TAB_NEW
                ),
            });
        }
        let fingerprint = self.fingerprint_event(&window, super::TOOL_TAB_NEW)?;
        Ok(assistant_tool_bus::ToolOutput::json(
            assistant_protocol::serde_json::json!({
                "tab_count": after,
                "previous_tab_count": before,
                "fingerprint": fingerprint.as_str(),
            }),
        ))
    }
}

/// Tool handler for `notepad.tab.new`.
pub(super) struct NewTabHandler<P> {
    pub(super) context: std::sync::Arc<super::NotepadHandlerContext<P>>,
}

impl<P> assistant_tool_bus::ToolHandler for NewTabHandler<P>
where
    P: assistant_platform_api::WindowProvider
        + assistant_platform_api::UiAutomationProvider
        + Send
        + Sync
        + 'static,
{
    fn call(
        &self,
        _call: &assistant_tool_bus::CallContext,
        _arguments: &assistant_protocol::serde_json::Map<
            String,
            assistant_protocol::serde_json::Value,
        >,
    ) -> Result<assistant_tool_bus::ToolOutput, assistant_tool_bus::ToolBusError> {
        self.context.new_tab_output()
    }
}

/// Parses the `Tabs: <n>` readout a fixture-style adapter exposes.
fn parse_tab_count(text: &str) -> Result<u64, assistant_tool_bus::ToolBusError> {
    let value = text.rsplit(':').next().unwrap_or(text).trim();
    value
        .parse::<u64>()
        .map_err(|_| assistant_tool_bus::ToolBusError::Mcp {
            code: -32_601,
            message: format!(
                "{}: tab count readout `{text}` is not `Tabs: <n>` (CapabilityMissing)",
                super::TOOL_TAB_NEW
            ),
        })
}

#[cfg(test)]
mod tests {
    use super::parse_tab_count;

    #[test]
    fn test_parse_tab_count_reads_the_fixture_readout() {
        assert_eq!(parse_tab_count("Tabs: 1").ok(), Some(1));
        assert_eq!(parse_tab_count("Tabs: 12").ok(), Some(12));
    }

    #[test]
    fn test_parse_tab_count_rejects_a_readout_that_is_not_a_count() {
        let error = parse_tab_count("no tabs here").err();
        assert!(matches!(
            error,
            Some(assistant_tool_bus::ToolBusError::Mcp { code: -32_601, .. })
        ));
    }
}
