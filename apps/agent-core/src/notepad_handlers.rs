//! Production Notepad handlers for the five 1a tools (ADR-0058 D4).
//!
//! Responsibilities:
//! - register the five Notepad tools declared by the adapter package;
//! - resolve the adapter target inside the target window scope;
//! - execute UIA read/write/key actions and return the pre/post fingerprint
//!   required by verification.
//!
//! Boundaries:
//! - does not decide policy or approval;
//! - does not expose platform handles across the `ToolBus`;
//! - does not operate a real Notepad in tests and does not invent missing
//!   observations.
//!
//! Invariants:
//! 1. every registered tool comes from `tools.json` and has one handler;
//! 2. a write returns only after the platform provider's own read-back
//!    postcondition succeeds;
//! 3. a missing fingerprint, unknown target, or unavailable capability is an
//!    explicit failure rather than a successful placeholder.
//!
//! Related documents: ADR-0022, ADR-0043, ADR-0058, `runtime-execution.md`.

use std::collections::BTreeMap;
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

use assistant_platform_api::{
    ErrorCode, Fingerprint, FingerprintScope, KeyChord, KeyModifier, KeyTarget, PlatformError,
    ResolvedElement, ResolvedWindow, UiAutomationProvider, WindowFilter, WindowProvider,
};
use assistant_protocol::serde_json;
use assistant_tool_bus::{CallContext, SourceDescriptor, ToolBusError, ToolHandler, ToolOutput};
use serde_json::{Map, Value, json};

use crate::notepad_files::snapshot_file;
use crate::notepad_registry::{
    TOOL_READ_TEXT, TOOL_REPLACE_TEXT, TOOL_SAVE, TOOL_SAVE_AS, TOOL_TAB_NEW,
};
use crate::notepad_targets::{
    EDITOR_TARGET, MAIN_WINDOW_TARGET, NotepadTargetCatalog, SAVE_AS_DIALOG_TARGET,
    SAVE_AS_FILENAME_TARGET, SAVE_AS_SAVE_BUTTON_TARGET,
};

const SAVE_AS_DIALOG_ATTEMPTS: usize = 30;
const SAVE_AS_DIALOG_INTERVAL_MS: u64 = 100;

pub(crate) fn build_handler_map<P>(
    context: &Arc<NotepadHandlerContext<P>>,
) -> BTreeMap<String, Arc<dyn ToolHandler>>
where
    P: WindowProvider + UiAutomationProvider + Send + Sync + 'static,
{
    let mut handlers: BTreeMap<String, Arc<dyn ToolHandler>> = BTreeMap::new();
    handlers.insert(
        TOOL_READ_TEXT.to_owned(),
        Arc::new(ReadTextHandler {
            context: Arc::clone(context),
        }),
    );
    handlers.insert(
        TOOL_REPLACE_TEXT.to_owned(),
        Arc::new(ReplaceTextHandler {
            context: Arc::clone(context),
        }),
    );
    handlers.insert(
        TOOL_SAVE.to_owned(),
        Arc::new(SaveHandler {
            context: Arc::clone(context),
        }),
    );
    handlers.insert(
        TOOL_TAB_NEW.to_owned(),
        Arc::new(NewTabHandler {
            context: Arc::clone(context),
        }),
    );
    handlers.insert(
        TOOL_SAVE_AS.to_owned(),
        Arc::new(SaveAsHandler {
            context: Arc::clone(context),
        }),
    );
    handlers
}

struct ReadTextHandler<P> {
    context: Arc<NotepadHandlerContext<P>>,
}

impl<P> ToolHandler for ReadTextHandler<P>
where
    P: WindowProvider + UiAutomationProvider + Send + Sync + 'static,
{
    fn call(
        &self,
        _call: &CallContext,
        _arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        self.context.read_text_output(TOOL_READ_TEXT)
    }
}

struct ReplaceTextHandler<P> {
    context: Arc<NotepadHandlerContext<P>>,
}

impl<P> ToolHandler for ReplaceTextHandler<P>
where
    P: WindowProvider + UiAutomationProvider + Send + Sync + 'static,
{
    fn call(
        &self,
        _call: &CallContext,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        self.context.replace_text_output(arguments)
    }
}

struct SaveHandler<P> {
    context: Arc<NotepadHandlerContext<P>>,
}

impl<P> ToolHandler for SaveHandler<P>
where
    P: WindowProvider + UiAutomationProvider + Send + Sync + 'static,
{
    fn call(
        &self,
        _call: &CallContext,
        _arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        self.context.save_output()
    }
}

struct NewTabHandler<P> {
    context: Arc<NotepadHandlerContext<P>>,
}

impl<P> ToolHandler for NewTabHandler<P>
where
    P: WindowProvider + UiAutomationProvider + Send + Sync + 'static,
{
    fn call(
        &self,
        _call: &CallContext,
        _arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        self.context.new_tab_output()
    }
}

struct SaveAsHandler<P> {
    context: Arc<NotepadHandlerContext<P>>,
}

impl<P> ToolHandler for SaveAsHandler<P>
where
    P: WindowProvider + UiAutomationProvider + Send + Sync + 'static,
{
    fn call(
        &self,
        _call: &CallContext,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        self.context.save_as_output(arguments)
    }
}

pub(crate) struct NotepadHandlerContext<P> {
    pub(crate) platform: Arc<P>,
    pub(crate) app_id: String,
    pub(crate) targets: Arc<NotepadTargetCatalog>,
}

impl<P> NotepadHandlerContext<P>
where
    P: WindowProvider + UiAutomationProvider + Send + Sync + 'static,
{
    fn read_text_output(&self, tool: &str) -> Result<ToolOutput, ToolBusError> {
        let (window, editor) = self.resolve_editor(tool)?;
        let before = self.fingerprint_event(&window, tool)?;
        let started = Instant::now();
        let text = self.read_element_text(&editor, tool)?;
        let canonical = normalize_line_endings(&text);
        let after = self.fingerprint_event(&window, tool)?;
        let data = json!({
            "text": canonical,
            "truncated": false,
            "fingerprint": after.as_str(),
            "previous_fingerprint": before.as_str(),
            "elapsed_ms": elapsed_ms(started),
        });
        Ok(ToolOutput::untrusted(
            SourceDescriptor::app_content(self.app_id.clone()).with_target("editor"),
            data,
        ))
    }

    fn replace_text_output(
        &self,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        let old_text = required_text(arguments, "old_text")?;
        let new_text = required_text(arguments, "new_text")?;
        let expected = required_u64(arguments, "expected_replacements")?;
        if old_text.is_empty() {
            return Err(invalid_arguments(
                TOOL_REPLACE_TEXT,
                "old_text must not be empty",
            ));
        }
        if expected == 0 {
            return Err(invalid_arguments(
                TOOL_REPLACE_TEXT,
                "expected_replacements must be >= 1",
            ));
        }
        if old_text == new_text {
            return Err(invalid_arguments(
                TOOL_REPLACE_TEXT,
                "new_text must differ from old_text",
            ));
        }
        let (window, editor) = self.resolve_editor(TOOL_REPLACE_TEXT)?;
        let before = self.fingerprint_event(&window, TOOL_REPLACE_TEXT)?;
        let current = normalize_line_endings(&self.read_element_text(&editor, TOOL_REPLACE_TEXT)?);
        let count = current.matches(old_text).count();
        if u64::try_from(count).unwrap_or(u64::MAX) != expected {
            return Err(invalid_arguments(
                TOOL_REPLACE_TEXT,
                format!("expected {expected} replacement(s), found {count}"),
            ));
        }
        let replaced = current.replace(old_text, new_text);
        let started = Instant::now();
        self.set_element_value(&editor, &replaced, TOOL_REPLACE_TEXT)?;
        let observed = normalize_line_endings(&self.read_element_text(&editor, TOOL_REPLACE_TEXT)?);
        if observed != replaced {
            return Err(ToolBusError::Mcp {
                code: -32_003,
                message: format!(
                    "{TOOL_REPLACE_TEXT}: read-back differs after UIA set_value (VerifyFailed)"
                ),
            });
        }
        let after = self.fingerprint_event(&window, TOOL_REPLACE_TEXT)?;
        let data = json!({
            "replacement_count": count,
            "canonical_text": observed,
            "fingerprint": after.as_str(),
            "previous_fingerprint": before.as_str(),
            "elapsed_ms": elapsed_ms(started),
        });
        Ok(ToolOutput::untrusted(
            SourceDescriptor::app_content(self.app_id.clone()).with_target("editor"),
            data,
        ))
    }

    fn save_output(&self) -> Result<ToolOutput, ToolBusError> {
        let window = self.resolve_window(MAIN_WINDOW_TARGET, TOOL_SAVE)?;
        let before = self.fingerprint_event(&window, TOOL_SAVE)?;
        let started = Instant::now();
        self.send_key(&window, "S", vec![KeyModifier::Control], TOOL_SAVE)?;
        let after = self.fingerprint_event(&window, TOOL_SAVE)?;
        let window_title = self.window_title(&window, TOOL_SAVE)?;
        Ok(ToolOutput::json(json!({
            "saved": true,
            "window_title": window_title,
            "fingerprint": after.as_str(),
            "previous_fingerprint": before.as_str(),
            "elapsed_ms": elapsed_ms(started),
        })))
    }

    fn save_as_output(&self, arguments: &Map<String, Value>) -> Result<ToolOutput, ToolBusError> {
        let target_path = PathBuf::from(required_text(arguments, "target_path")?.to_owned());
        if !target_path.is_absolute() {
            return Err(invalid_arguments(
                TOOL_SAVE_AS,
                "target_path must be absolute",
            ));
        }
        if arguments.get("overwrite_existing").and_then(Value::as_bool) != Some(false) {
            return Err(invalid_arguments(
                TOOL_SAVE_AS,
                "overwrite_existing must be false",
            ));
        }
        let before_file = snapshot_file(&target_path, TOOL_SAVE_AS)?;
        if before_file.exists {
            return Err(invalid_arguments(
                TOOL_SAVE_AS,
                format!("target file already exists: {}", target_path.display()),
            ));
        }
        let main_window = self.resolve_window(MAIN_WINDOW_TARGET, TOOL_SAVE_AS)?;
        let before_fingerprint = self.fingerprint_event(&main_window, TOOL_SAVE_AS)?;
        let started = Instant::now();
        self.send_key(
            &main_window,
            "S",
            vec![KeyModifier::Control, KeyModifier::Shift],
            TOOL_SAVE_AS,
        )?;
        let dialog = self.wait_for_window(SAVE_AS_DIALOG_TARGET, TOOL_SAVE_AS)?;
        let filename = self.resolve_element(SAVE_AS_FILENAME_TARGET, &dialog, TOOL_SAVE_AS)?;
        let save_button =
            self.resolve_element(SAVE_AS_SAVE_BUTTON_TARGET, &dialog, TOOL_SAVE_AS)?;
        self.set_element_value(
            &filename,
            target_path.to_string_lossy().as_ref(),
            TOOL_SAVE_AS,
        )?;
        self.invoke_element(&save_button, "invoke", TOOL_SAVE_AS)?;
        let after_file = snapshot_file(&target_path, TOOL_SAVE_AS)?;
        if !after_file.exists {
            return Err(ToolBusError::Mcp {
                code: -32_004,
                message: format!(
                    "{TOOL_SAVE_AS}: target file was not created after the save action \
                     (VerifyFailed)"
                ),
            });
        }
        let after_fingerprint = self.fingerprint_event(&main_window, TOOL_SAVE_AS)?;
        let files = serde_json::to_value(BTreeMap::from([(
            target_path.to_string_lossy().to_string(),
            assistant_verify::FileTransition::new(before_file, after_file),
        )]))
        .map_err(|error| ToolBusError::EnvelopeAssembly {
            tool: TOOL_SAVE_AS.to_owned(),
            reason: error.to_string(),
        })?;
        Ok(ToolOutput::json(json!({
            "target_path": target_path.to_string_lossy(),
            "file_created": true,
            "fingerprint": after_fingerprint.as_str(),
            "previous_fingerprint": before_fingerprint.as_str(),
            "elapsed_ms": elapsed_ms(started),
            "files": files,
        })))
    }

    fn resolve_editor(
        &self,
        tool: &str,
    ) -> Result<(ResolvedWindow, ResolvedElement), ToolBusError> {
        let window = self.resolve_window(MAIN_WINDOW_TARGET, tool)?;
        let editor = self.resolve_element(EDITOR_TARGET, &window, tool)?;
        Ok((window, editor))
    }

    fn resolve_window(&self, target: &str, tool: &str) -> Result<ResolvedWindow, ToolBusError> {
        let descriptor =
            self.targets
                .descriptor(target)
                .map_err(|error| ToolBusError::EnvelopeAssembly {
                    tool: tool.to_owned(),
                    reason: error.to_string(),
                })?;
        let result = poll_immediate(
            self.platform.resolve_window(descriptor),
            tool,
            "resolve_window",
        )?;
        result.map_err(|error| map_platform_error(tool, &error))
    }

    fn resolve_element(
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
        let result = poll_immediate(
            self.platform.resolve_element(scope, &chain),
            tool,
            "resolve_element",
        )?;
        result.map_err(|error| map_platform_error(tool, &error))
    }

    fn read_element_text(
        &self,
        element: &ResolvedElement,
        tool: &str,
    ) -> Result<String, ToolBusError> {
        let result = poll_immediate(self.platform.read_text(element), tool, "read_text")?;
        result.map_err(|error| map_platform_error(tool, &error))
    }

    fn set_element_value(
        &self,
        element: &ResolvedElement,
        value: &str,
        tool: &str,
    ) -> Result<(), ToolBusError> {
        let result = poll_immediate(self.platform.set_value(element, value), tool, "set_value")?;
        result.map_err(|error| map_platform_error(tool, &error))
    }

    fn invoke_element(
        &self,
        element: &ResolvedElement,
        action: &str,
        tool: &str,
    ) -> Result<(), ToolBusError> {
        let result = poll_immediate(
            self.platform.invoke_action(element, action),
            tool,
            "invoke_action",
        )?;
        result.map_err(|error| map_platform_error(tool, &error))
    }

    fn fingerprint_event(
        &self,
        window: &ResolvedWindow,
        tool: &str,
    ) -> Result<Fingerprint, ToolBusError> {
        let result = poll_immediate(
            self.platform
                .fingerprint(window, &FingerprintScope::WholeWindow),
            tool,
            "fingerprint",
        )?;
        result.map_err(|error| map_platform_error(tool, &error))
    }

    fn send_key(
        &self,
        window: &ResolvedWindow,
        key: &str,
        modifiers: Vec<KeyModifier>,
        tool: &str,
    ) -> Result<(), ToolBusError> {
        let chord = KeyChord::new(key.to_owned(), modifiers);
        let result = poll_immediate(
            self.platform
                .key_action(&chord, &KeyTarget::Window(window.clone())),
            tool,
            "key_action",
        )?;
        result.map_err(|error| map_platform_error(tool, &error))
    }

    fn window_title(&self, window: &ResolvedWindow, tool: &str) -> Result<String, ToolBusError> {
        let result = poll_immediate(
            self.platform.list_windows(&WindowFilter::any()),
            tool,
            "list_windows",
        )?;
        let windows = result.map_err(|error| map_platform_error(tool, &error))?;
        windows
            .into_iter()
            .find(|candidate| candidate.window().id() == window.id())
            .map(|candidate| candidate.title().to_owned())
            .ok_or_else(|| ToolBusError::UnknownTool {
                tool: format!("{tool}: target window disappeared"),
            })
    }

    fn wait_for_window(&self, target: &str, tool: &str) -> Result<ResolvedWindow, ToolBusError> {
        let mut last_error = None;
        for _attempt in 0..SAVE_AS_DIALOG_ATTEMPTS {
            match self.resolve_window(target, tool) {
                Ok(window) => return Ok(window),
                Err(error) => last_error = Some(error),
            }
            std::thread::sleep(Duration::from_millis(SAVE_AS_DIALOG_INTERVAL_MS));
        }
        Err(last_error.unwrap_or_else(|| invalid_arguments(tool, "save-as dialog did not appear")))
    }

    /// Creates a new tab and proves the tab count actually increased by one.
    ///
    /// The count is read from the adapter's **optional** `tab_count` target. An
    /// adapter that does not declare it fails closed in `resolve_element` with
    /// that target's name, rather than reporting a fabricated number; the action
    /// is therefore never performed unverifiably.
    fn new_tab_output(&self) -> Result<ToolOutput, ToolBusError> {
        let window = self.resolve_window(MAIN_WINDOW_TARGET, TOOL_TAB_NEW)?;
        let count_element = self.resolve_element(
            crate::notepad_targets::TAB_COUNT_TARGET,
            &window,
            TOOL_TAB_NEW,
        )?;
        let before = parse_tab_count(&self.read_element_text(&count_element, TOOL_TAB_NEW)?)?;
        let add_button = self.resolve_element(
            crate::notepad_targets::ADD_TAB_BUTTON_TARGET,
            &window,
            TOOL_TAB_NEW,
        )?;
        self.invoke_element(&add_button, "invoke", TOOL_TAB_NEW)?;
        let after = parse_tab_count(&self.read_element_text(&count_element, TOOL_TAB_NEW)?)?;
        if after != before.saturating_add(1) {
            return Err(ToolBusError::Mcp {
                code: -32_004,
                message: format!(
                    "{TOOL_TAB_NEW}: tab count did not increase by one \
                     (before={before}, after={after}) (VerifyFailed)"
                ),
            });
        }
        let fingerprint = self.fingerprint_event(&window, TOOL_TAB_NEW)?;
        Ok(ToolOutput::json(json!({
            "tab_count": after,
            "previous_tab_count": before,
            "fingerprint": fingerprint.as_str(),
        })))
    }
}

/// Parses the `Tabs: <n>` readout the fixture exposes as `TabCountText`.
fn parse_tab_count(text: &str) -> Result<u64, ToolBusError> {
    let value = text.rsplit(':').next().unwrap_or(text).trim();
    value.parse::<u64>().map_err(|_| ToolBusError::Mcp {
        code: -32_601,
        message: format!(
            "{TOOL_TAB_NEW}: tab count readout `{text}` is not `Tabs: <n>` (CapabilityMissing)"
        ),
    })
}

fn required_text<'a>(
    arguments: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, ToolBusError> {
    arguments
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_arguments("notepad.handler", format!("{field} must be a string")))
}

fn required_u64(arguments: &Map<String, Value>, field: &str) -> Result<u64, ToolBusError> {
    arguments.get(field).and_then(Value::as_u64).ok_or_else(|| {
        invalid_arguments(
            "notepad.handler",
            format!("{field} must be a non-negative integer"),
        )
    })
}

fn invalid_arguments(tool: &str, reason: impl Into<String>) -> ToolBusError {
    ToolBusError::InvalidArguments {
        tool: tool.to_owned(),
        reason: reason.into(),
    }
}

/// Executes only immediately-ready provider futures.
///
/// The Windows provider is synchronous internally and exposes that work as an
/// immediately-ready future. A pending future means this assumption no longer
/// holds, so the handler fails closed instead of blocking or spawning.
fn poll_immediate<F: Future>(
    future: F,
    tool: &str,
    operation: &str,
) -> Result<F::Output, ToolBusError> {
    let mut future = std::pin::pin!(future);
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    match future.as_mut().poll(&mut context) {
        Poll::Ready(output) => Ok(output),
        Poll::Pending => Err(ToolBusError::Transport {
            reason: format!("{tool}: {operation} future unexpectedly returned Pending"),
        }),
    }
}

fn map_platform_error(tool: &str, error: &PlatformError) -> ToolBusError {
    let reason = format!("{:?}: {}", error.code(), error.message());
    match error.code() {
        ErrorCode::ToolInvalidArgs => ToolBusError::InvalidArguments {
            tool: tool.to_owned(),
            reason,
        },
        ErrorCode::TargetNotFound => ToolBusError::UnknownTool {
            tool: format!("{tool}: {reason}"),
        },
        ErrorCode::CapabilityMissing => ToolBusError::Mcp {
            code: -32_601,
            message: format!("{tool}: {reason}"),
        },
        _ => ToolBusError::Mcp {
            code: -32_000,
            message: format!("{tool}: {reason}"),
        },
    }
}

fn normalize_line_endings(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::{normalize_line_endings, parse_tab_count};

    #[test]
    fn test_normalize_line_endings_handles_crlf_and_cr() {
        assert_eq!(normalize_line_endings("a\r\nb\rc\n"), "a\nb\nc\n");
    }

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
