//! Lease-guarded synthetic keyboard input for the Notepad handlers.
//!
//! Responsibilities:
//! - map window and element key targets to one window-scoped lease key;
//! - acquire the exclusive target lease before delegating to the platform;
//! - keep the provider error mapping used by the rest of the Notepad handlers.
//!
//! Boundaries:
//! - no target resolution, policy decision, or general tool dispatch;
//! - no direct platform call outside the lease gate.
//!
//! Invariants:
//! 1. a conflicting task never reaches `key_action`;
//! 2. success and failure both release the lease through `TargetLeaseGate::run`.

use assistant_platform_api::{
    KeyChord, KeyModifier, KeyTarget, ResolvedElement, ResolvedWindow, UiAutomationProvider,
    WindowProvider,
};
use assistant_tool_bus::ToolBusError;

use super::{NotepadHandlerContext, poll_immediate, support};
use crate::target_lease::window_lease_key;

impl<P> NotepadHandlerContext<P>
where
    P: WindowProvider + UiAutomationProvider + Send + Sync + 'static,
{
    /// Sends a key chord to a resolved window while holding its exclusive lease.
    ///
    /// # Errors
    ///
    /// Returns a target-lease error before calling the platform when another task owns the
    /// window, then returns the platform's usual error when focus cannot be confirmed or the
    /// input cannot be sent.
    pub(crate) fn send_key(
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
            let result = poll_immediate(
                self.platform
                    .key_action(&chord, &KeyTarget::Window(window.clone())),
                tool,
                "key_action",
            )?;
            result.map_err(|error| support::map_platform_error(tool, &error))
        })
    }

    /// Sends a key chord to a resolved element, which makes the platform confirm focus first.
    ///
    /// Use this instead of [`Self::send_key`] whenever a shortcut must land in a specific
    /// element (for example `Ctrl+Z` on the editor) rather than anywhere in the window.
    ///
    /// # Errors
    ///
    /// Returns a target-lease error before calling the platform when another task owns the
    /// element's window, then returns the platform's error when focus cannot be confirmed or the
    /// input cannot be sent.
    pub(crate) fn send_key_to_element(
        &self,
        task_id: &str,
        element: &ResolvedElement,
        key: &str,
        modifiers: Vec<KeyModifier>,
        tool: &str,
    ) -> Result<(), ToolBusError> {
        let lease_key = window_lease_key(&self.app_id, element.parent(), tool)?;
        self.input_leases.run(task_id, lease_key, tool, || {
            let chord = KeyChord::new(key.to_owned(), modifiers);
            let result = poll_immediate(
                self.platform
                    .key_action(&chord, &KeyTarget::Element(element.clone())),
                tool,
                "key_action",
            )?;
            result.map_err(|error| support::map_platform_error(tool, &error))
        })
    }
}
