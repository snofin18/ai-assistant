//! Load and register the five declared Notepad tools.
//!
//! Responsibilities:
//! - parse `tools.json` into the protocol's `ToolSchema`;
//! - verify that every declared tool has exactly one production handler;
//! - return the registry plus the Planner tool catalog.
//!
//! Boundaries:
//! - does not execute tools or resolve targets;
//! - does not add tools that are absent from the adapter declaration;
//! - does not weaken unknown schema values into defaults.
//!
//! Invariants:
//! 1. the registered set is exactly the five 1a Notepad tools;
//! 2. unknown schema values fail deserialization;
//! 3. a handler count mismatch fails before `ToolBus` startup.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use assistant_platform_api::{UiAutomationProvider, WindowProvider};
use assistant_protocol::{ErrorCode, ToolSchema};
use assistant_tool_bus::{ToolBusError, ToolDefinition, ToolHandler, ToolRegistry};
use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;

use crate::notepad_handlers::{NotepadHandlerContext, build_handler_map};
use crate::notepad_rollback::{CapturedRollbackAnchor, NotepadRollback};
use crate::notepad_targets::{MAIN_WINDOW_TARGET, NotepadTargetCatalog};
use crate::runtime_host_ops::{ReservedHostOperationError, ReservedHostOperations};
use crate::runtime_tools::TOOL_HOST_CAPTURE_INITIAL_FINGERPRINT;
use crate::target_lease::TargetLeaseGate;

/// Tool name for reading the active document.
pub(crate) const TOOL_READ_TEXT: &str = "notepad.file.read_text";
/// Tool name for replacing text.
pub(crate) const TOOL_REPLACE_TEXT: &str = "notepad.file.replace_text";
/// Tool name for saving the active document.
pub(crate) const TOOL_SAVE: &str = "notepad.file.save";
/// Tool name for creating a new tab.
pub(crate) const TOOL_TAB_NEW: &str = "notepad.tab.new";
/// Tool name for saving to a new path.
pub(crate) const TOOL_SAVE_AS: &str = "notepad.file.save_as";

pub(crate) const EXPECTED_TOOL_NAMES: &[&str] = &[
    TOOL_READ_TEXT,
    TOOL_REPLACE_TEXT,
    TOOL_SAVE,
    TOOL_TAB_NEW,
    TOOL_SAVE_AS,
];

/// Hard cap on the number of tasks whose rollback anchors are kept.
///
/// Each anchor holds the editor text plus the target file bytes. A long-running Host must not
/// retain every task forever, so the oldest task is evicted once the cap is exceeded.
pub(crate) const MAX_TASK_ANCHORS: usize = 64;

/// Hard cap for the UTF-8 prefix reader.
///
/// A task package controls `max_text_bytes`, so the Host must not let an untrusted
/// declaration turn one read into an unbounded allocation.
pub(crate) const MAX_READ_UTF8_PREFIX_BYTES: u64 = 16 * 1024 * 1024;

/// Largest number of bytes needed to trim an incomplete UTF-8 sequence at the buffer end.
const UTF8_INCOMPLETE_SEQUENCE_BYTES: usize = 3;

/// Failure while loading or registering the adapter tool declarations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub(crate) enum NotepadRegistryError {
    /// The declaration file could not be read.
    #[error("tool declaration file `{path}` could not be read: {reason}")]
    Read {
        /// Attempted path.
        path: String,
        /// I/O failure.
        reason: String,
    },

    /// The declaration is malformed or inconsistent.
    #[error("tool declaration is invalid: {reason}")]
    Malformed {
        /// Failure detail.
        reason: String,
    },

    /// A declared tool has no handler.
    #[error("declared tool `{tool}` has no production handler")]
    MissingHandler {
        /// Missing tool name.
        tool: String,
    },

    /// Tool registration failed.
    #[error("tool registration failed: {0}")]
    ToolBus(#[from] ToolBusError),
}

/// Registry plus the catalog shape supplied to the Planner.
pub(crate) struct NotepadRegistryBuild {
    pub(crate) registry: ToolRegistry,
    pub(crate) tool_schemas: Vec<ToolSchema>,
    pub(crate) host_operations: Arc<dyn ReservedHostOperations>,
}

/// Builds the five declared Notepad tools and their Host handlers.
pub(crate) fn build_notepad_registry<P>(
    platform: Arc<P>,
    targets: Arc<NotepadTargetCatalog>,
    tools_path: &Path,
    task_inputs: &serde_json::Map<String, Value>,
    input_leases: TargetLeaseGate,
) -> Result<NotepadRegistryBuild, NotepadRegistryError>
where
    P: WindowProvider + UiAutomationProvider + Send + Sync + 'static,
{
    let declared = load_and_validate_declarations(tools_path, targets.app_id(), EXPECTED_TOOL_NAMES)?;

    let context = Arc::new(NotepadHandlerContext {
        platform,
        app_id: declared.app_id.clone(),
        targets,
        input_leases,
    });
    let host_operations = Arc::new(NotepadReservedHostOperations {
        context: Arc::clone(&context),
        rollback: NotepadRollback::new(Arc::clone(&context)),
        anchors: Mutex::new(TaskAnchorRegistry::default()),
        task_target_path: document_path_from_inputs(task_inputs),
        task_requires_l1: required_anchor_levels_include_l1(task_inputs),
    });
    let handlers = build_handler_map(&context);
    let (registry, tool_schemas) =
        register_declared_tools(declared, EXPECTED_TOOL_NAMES, &handlers)?;
    Ok(NotepadRegistryBuild {
        registry,
        tool_schemas,
        host_operations,
    })
}

/// Registers exactly the declared tools the adapter is expected to expose.
///
/// ADR-0084 D1: the expected name set and handler map are arguments, so a second
/// adapter (Paint) reuses this loop instead of a second registration path. The
/// declared set must match `expected_tool_names` exactly: an unknown name, a
/// duplicate, a missing handler, or a missing tool all fail closed.
fn register_declared_tools(
    declared: DeclaredToolsFile,
    expected_tool_names: &[&str],
    handlers: &BTreeMap<String, Arc<dyn ToolHandler>>,
) -> Result<(ToolRegistry, Vec<ToolSchema>), NotepadRegistryError> {
    let mut registry = ToolRegistry::new();
    let mut tool_schemas = Vec::with_capacity(declared.tools.len());
    let mut names = BTreeSet::new();
    for declared_tool in declared.tools {
        let schema = parse_tool_schema(declared_tool.schema)?;
        if schema.version != "1.0" {
            return Err(NotepadRegistryError::Malformed {
                reason: format!("tool `{}` must use schema version 1.0", schema.name),
            });
        }
        if !expected_tool_names.contains(&schema.name.as_str()) {
            return Err(NotepadRegistryError::Malformed {
                reason: format!("unexpected production tool `{}`", schema.name),
            });
        }
        if !names.insert(schema.name.clone()) {
            return Err(NotepadRegistryError::Malformed {
                reason: format!("duplicate production tool `{}`", schema.name),
            });
        }
        let handler = handlers.get(&schema.name).cloned().ok_or_else(|| {
            NotepadRegistryError::MissingHandler {
                tool: schema.name.clone(),
            }
        })?;
        registry.register(tool_definition(&schema)?, handler)?;
        tool_schemas.push(schema);
    }
    if let Some(missing) = missing_production_tools(&names, expected_tool_names) {
        return Err(NotepadRegistryError::Malformed {
            reason: format!("missing production tools: {missing}"),
        });
    }
    // ADR-0060: the reserved runtime tools belong to the **Planner catalog** but
    // never to the model-visible `ToolBus` mount, so they are appended to the
    // catalog here instead of being registered as handlers.
    tool_schemas.extend(
        crate::runtime_tools::planner_schemas()
            .map_err(|reason| NotepadRegistryError::Malformed { reason })?,
    );
    tool_schemas.sort_by(|left, right| left.name.cmp(&right.name));
    Ok((registry, tool_schemas))
}

/// Loads the tool declaration file and validates its app id and tool count.
///
/// # Errors
///
/// Returns [`NotepadRegistryError::Read`] for an unreadable file and
/// [`NotepadRegistryError::Malformed`] for invalid JSON, a mismatched app id, or a wrong count.
fn load_and_validate_declarations(
    tools_path: &Path,
    expected_app_id: &str,
    expected_tool_names: &[&str],
) -> Result<DeclaredToolsFile, NotepadRegistryError> {
    let body = std::fs::read_to_string(tools_path).map_err(|error| NotepadRegistryError::Read {
        path: tools_path.display().to_string(),
        reason: error.to_string(),
    })?;
    let declared: DeclaredToolsFile =
        serde_json::from_str(&body).map_err(|error| NotepadRegistryError::Malformed {
            reason: error.to_string(),
        })?;
    if declared.app_id != expected_app_id {
        return Err(NotepadRegistryError::Malformed {
            reason: format!(
                "tools app_id `{}` does not match target app_id `{}`",
                declared.app_id, expected_app_id
            ),
        });
    }
    if declared.tools.len() != expected_tool_names.len() {
        return Err(NotepadRegistryError::Malformed {
            reason: format!(
                "expected {} declared tools, found {}",
                expected_tool_names.len(),
                declared.tools.len()
            ),
        });
    }
    Ok(declared)
}

fn missing_production_tools(
    names: &BTreeSet<String>,
    expected_tool_names: &[&str],
) -> Option<String> {
    let missing = expected_tool_names
        .iter()
        .filter(|name| !names.contains(**name))
        .copied()
        .collect::<Vec<_>>()
        .join(", ");
    (!missing.is_empty()).then_some(missing)
}

/// Resolves the document path from the task inputs so the rollback anchor can snapshot the file.
fn document_path_from_inputs(task_inputs: &serde_json::Map<String, Value>) -> Option<String> {
    for key in ["input.file_path", "input.target_path"] {
        if let Some(path) = task_inputs.get(key).and_then(Value::as_str) {
            return Some(path.to_owned());
        }
    }
    None
}

/// Whether the task package declares an L1 anchor requirement.
fn required_anchor_levels_include_l1(task_inputs: &serde_json::Map<String, Value>) -> bool {
    task_inputs
        .get("rollback.required_anchor_levels")
        .and_then(Value::as_array)
        .is_some_and(|levels| levels.iter().any(|level| level.as_str() == Some("L1")))
}

struct NotepadReservedHostOperations<P> {
    context: Arc<NotepadHandlerContext<P>>,
    rollback: NotepadRollback<P>,
    anchors: Mutex<TaskAnchorRegistry<CapturedRollbackAnchor>>,
    task_target_path: Option<String>,
    task_requires_l1: bool,
}

/// Per-task rollback anchors plus the insertion order used to evict the oldest task.
struct TaskAnchorRegistry<T> {
    anchors: BTreeMap<String, T>,
    order: VecDeque<String>,
}

impl<T> Default for TaskAnchorRegistry<T> {
    fn default() -> Self {
        Self {
            anchors: BTreeMap::new(),
            order: VecDeque::new(),
        }
    }
}

impl<T> TaskAnchorRegistry<T> {
    /// Inserts or replaces a task anchor and enforces the hard cap.
    ///
    /// The returned anchor is the evicted oldest one, which the caller drops **outside** the lock.
    fn insert(&mut self, task_id: String, anchor: T) -> Option<T> {
        if self.anchors.insert(task_id.clone(), anchor).is_none() {
            self.order.push_back(task_id);
        }
        let mut evicted = None;
        while self.anchors.len() > MAX_TASK_ANCHORS {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            evicted = self.anchors.remove(&oldest).or(evicted);
        }
        evicted
    }

    fn get(&self, task_id: &str) -> Option<&T> {
        self.anchors.get(task_id)
    }

    fn remove(&mut self, task_id: &str) {
        self.anchors.remove(task_id);
        self.order.retain(|entry| entry != task_id);
    }
}

impl<P> ReservedHostOperations for NotepadReservedHostOperations<P>
where
    P: WindowProvider + UiAutomationProvider + Send + Sync + 'static,
{
    fn inspect_target_path(&self, target_path: &str) -> Result<Value, String> {
        self.context
            .inspect_target_path_data(target_path)
            .map_err(|error| error.to_string())
    }

    fn set_editor_value(&self, text: &str) -> Result<Value, String> {
        self.context
            .set_editor_value_data(text)
            .map_err(|error| error.to_string())
    }

    fn read_utf8_prefix(
        &self,
        target_path: Option<&str>,
        max_text_bytes: u64,
    ) -> Result<Value, ReservedHostOperationError> {
        let target_path = target_path.or(self.task_target_path.as_deref());
        read_utf8_prefix_data(target_path, max_text_bytes)
    }

    fn capture_initial_fingerprint(&self) -> Result<Value, String> {
        // ADR-0065 D2/D3: the reading comes from the **injected** platform, so the test fake and
        // `WindowsPlatform` travel this same code path. Nothing here may branch on build
        // configuration, and D4 forbids substituting a constant or a content digest.
        let tool = TOOL_HOST_CAPTURE_INITIAL_FINGERPRINT;
        let started = Instant::now();
        let window = self
            .context
            .resolve_window(MAIN_WINDOW_TARGET, tool)
            .map_err(|error| error.to_string())?;
        let fingerprint = self
            .context
            .fingerprint_event(&window, tool)
            .map_err(|error| error.to_string())?;
        let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        Ok(serde_json::json!({
            "initial_fingerprint": fingerprint.as_str(),
            // ADR-0065 D5: observing changes nothing, so the pre- and post-step readings are the
            // same value and the step's `state_unchanged` postcondition asserts a fact.
            "fingerprint": fingerprint.as_str(),
            "previous_fingerprint": fingerprint.as_str(),
            "elapsed_ms": elapsed_ms,
        }))
    }

    fn capture_rollback_anchor(
        &self,
        task_id: &str,
        step_id: &str,
        sequence: u32,
        target_path: Option<&str>,
    ) -> Result<Value, String> {
        let target_path = target_path.or(self.task_target_path.as_deref());
        if target_path.is_none() && self.task_requires_l1 {
            return Err(
                "rollback anchor: task requires L1 but no target file path was provided; \
                 pass `input.file_path` or `input.target_path` so the disk snapshot is captured"
                    .to_owned(),
            );
        }
        let captured =
            self.rollback
                .capture(task_id, step_id, sequence, target_path.map(Path::new))?;
        let digest = assistant_storage::BlobId::of_content(captured.canonical_text().as_bytes());
        let target_path = captured
            .target_path()
            .map(|path| path.to_string_lossy().to_string());
        let mut registry = self
            .anchors
            .lock()
            .map_err(|_| "rollback anchor registry is poisoned".to_owned())?;
        let evicted = registry.insert(task_id.to_owned(), captured);
        drop(registry);
        // Dropping the evicted anchor outside the lock releases its file bytes without holding it.
        drop(evicted);
        Ok(serde_json::json!({
            "anchor_captured": true,
            "editor_digest": format!("sha256:{}", digest.as_str()),
            "target_path": target_path,
        }))
    }

    fn execute_rollback(
        &self,
        task_id: &str,
        step_id: &str,
        restore_file: bool,
    ) -> Result<Value, String> {
        let captured = {
            let guard = self
                .anchors
                .lock()
                .map_err(|_| "rollback anchor registry is poisoned".to_owned())?;
            guard
                .get(task_id)
                .cloned()
                .ok_or_else(|| format!("rollback: no captured anchor for task `{task_id}`"))?
        };
        self.rollback
            .execute(task_id, step_id, &captured, restore_file)
    }

    fn observe_rollback_state(&self, task_id: &str) -> Result<Value, String> {
        let captured = {
            let guard = self
                .anchors
                .lock()
                .map_err(|_| "rollback anchor registry is poisoned".to_owned())?;
            guard
                .get(task_id)
                .cloned()
                .ok_or_else(|| format!("rollback: no captured anchor for task `{task_id}`"))?
        };
        self.rollback.observe_state(&captured)
    }

    fn release_task(&self, task_id: &str) -> Result<(), String> {
        let mut registry = self
            .anchors
            .lock()
            .map_err(|_| "rollback anchor registry is poisoned".to_owned())?;
        registry.remove(task_id);
        drop(registry);
        Ok(())
    }
}

/// Reads a bounded prefix from one UTF-8 text file.
///
/// The helper deliberately keeps the buffer bounded by `max_text_bytes` and then trims at
/// most three trailing bytes so a multi-byte code point is never split. The returned
/// `bytes_read` is the number actually included after that trim; `bytes_total` is the file
/// size observed before the read.
fn read_utf8_prefix_data(
    target_path: Option<&str>,
    max_text_bytes: u64,
) -> Result<Value, ReservedHostOperationError> {
    validate_read_budget(max_text_bytes)?;
    let path = validate_absolute_path(target_path.ok_or_else(|| {
        invalid_host_arguments(
            "target_path is required; pass an absolute file path in the task inputs",
        )
    })?)?;
    let (mut file, bytes_total) = open_target_file(&path)?;
    let read_limit = max_text_bytes.min(bytes_total);
    let mut buffer = read_bounded_prefix(&mut file, read_limit, &path)?;
    let (text, bytes_read) = decode_utf8_prefix(&mut buffer, bytes_total, read_limit)?;
    let bytes_read = u64::try_from(bytes_read).map_err(|_| {
        ReservedHostOperationError::new(
            ErrorCode::Fatal,
            "read prefix length does not fit u64".to_owned(),
        )
    })?;
    Ok(serde_json::json!({
        "text": text,
        "truncated": bytes_total > bytes_read,
        "bytes_read": bytes_read,
        "bytes_total": bytes_total,
    }))
}

/// Validates the caller's prefix budget before any file is opened.
fn validate_read_budget(max_text_bytes: u64) -> Result<(), ReservedHostOperationError> {
    if max_text_bytes == 0 {
        return Err(invalid_host_arguments(
            "max_text_bytes must be a positive integer",
        ));
    }
    if max_text_bytes > MAX_READ_UTF8_PREFIX_BYTES {
        return Err(invalid_host_arguments(&format!(
            "max_text_bytes exceeds the hard cap of {MAX_READ_UTF8_PREFIX_BYTES} bytes"
        )));
    }
    Ok(())
}

/// Opens one target file and returns its observed size.
fn open_target_file(path: &Path) -> Result<(File, u64), ReservedHostOperationError> {
    let file = File::open(path).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => ReservedHostOperationError::new(
            ErrorCode::TargetNotFound,
            format!("target file `{}` does not exist", path.display()),
        ),
        std::io::ErrorKind::PermissionDenied => ReservedHostOperationError::new(
            ErrorCode::PlatformPermission,
            format!("target file `{}` cannot be read", path.display()),
        ),
        _ => ReservedHostOperationError::new(
            ErrorCode::Fatal,
            format!("target file `{}` cannot be opened: {error}", path.display()),
        ),
    })?;
    let bytes_total = file
        .metadata()
        .map_err(|error| {
            ReservedHostOperationError::new(
                ErrorCode::Fatal,
                format!(
                    "target file `{}` metadata is unavailable: {error}",
                    path.display()
                ),
            )
        })?
        .len();
    Ok((file, bytes_total))
}

/// Reads at most `read_limit` bytes into a fresh buffer.
fn read_bounded_prefix(
    file: &mut File,
    read_limit: u64,
    path: &Path,
) -> Result<Vec<u8>, ReservedHostOperationError> {
    let mut buffer = Vec::new();
    file.take(read_limit)
        .read_to_end(&mut buffer)
        .map_err(|error| {
            ReservedHostOperationError::new(
                ErrorCode::Fatal,
                format!("target file `{}` cannot be read: {error}", path.display()),
            )
        })?;
    Ok(buffer)
}

/// Decodes the buffer, trimming an incomplete trailing UTF-8 sequence exactly once.
fn decode_utf8_prefix(
    buffer: &mut Vec<u8>,
    bytes_total: u64,
    read_limit: u64,
) -> Result<(String, usize), ReservedHostOperationError> {
    match std::str::from_utf8(buffer) {
        Ok(text) => Ok((text.to_owned(), buffer.len())),
        Err(error) if error.error_len().is_none() && bytes_total > read_limit => {
            let valid_up_to = error.valid_up_to();
            let trimmed = buffer.len().saturating_sub(valid_up_to);
            debug_assert!(trimmed <= UTF8_INCOMPLETE_SEQUENCE_BYTES);
            buffer.truncate(valid_up_to);
            let bytes_read = buffer.len();
            std::str::from_utf8(buffer)
                .map(str::to_owned)
                .map(|text| (text, bytes_read))
                .map_err(|_| {
                    ReservedHostOperationError::new(
                        ErrorCode::Fatal,
                        "UTF-8 boundary trim produced invalid bytes".to_owned(),
                    )
                })
        }
        Err(error) => Err(ReservedHostOperationError::new(
            ErrorCode::ToolInvalidArgs,
            format!(
                "target file prefix is not valid UTF-8 at byte {}",
                error.valid_up_to()
            ),
        )),
    }
}

/// Validates the absolute path and rejects parent-directory traversal components.
fn validate_absolute_path(target_path: &str) -> Result<PathBuf, ReservedHostOperationError> {
    let path = PathBuf::from(target_path);
    if !path.is_absolute() {
        return Err(invalid_host_arguments("target_path must be absolute"));
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(invalid_host_arguments(
            "target_path must not contain parent-directory traversal",
        ));
    }
    Ok(path)
}

/// Creates a `ToolInvalidArgs` host-operation error.
fn invalid_host_arguments(message: &str) -> ReservedHostOperationError {
    ReservedHostOperationError::new(ErrorCode::ToolInvalidArgs, message.to_owned())
}

fn tool_definition(schema: &ToolSchema) -> Result<ToolDefinition, ToolBusError> {
    ToolDefinition::new(
        schema.name.clone(),
        schema.description.clone(),
        schema.risk_level,
        schema.effect,
        schema.reversibility,
        schema.input.clone(),
    )?
    .with_output_schema(schema.output.clone())?
    .with_idempotent(schema.idempotent)?
    .with_requires_approval(schema.requires_approval)
    .map(|definition| definition.with_tags(schema.tags.clone()))
}

#[derive(Debug, Deserialize)]
struct DeclaredToolsFile {
    app_id: String,
    tools: Vec<DeclaredTool>,
}

#[derive(Debug, Deserialize)]
struct DeclaredTool {
    schema: Value,
}

fn parse_tool_schema(mut value: Value) -> Result<ToolSchema, NotepadRegistryError> {
    let object = value
        .as_object_mut()
        .ok_or_else(|| NotepadRegistryError::Malformed {
            reason: "tool schema must be a JSON object".to_owned(),
        })?;
    // PITFALL(app=notepad): the 1a adapter declares `none_readonly` for read-only
    // tools, while the protocol's closed ToolReversibility enum has no such
    // variant. Normalize only that exact read-only combination; any other
    // unknown value still fails deserialization below.
    if object.get("effect").and_then(Value::as_str) == Some("read")
        && object.get("reversibility").and_then(Value::as_str) == Some("none_readonly")
    {
        object.insert(
            "reversibility".to_owned(),
            Value::String("l0_undo_stack".to_owned()),
        );
    }
    serde_json::from_value(value).map_err(|error| NotepadRegistryError::Malformed {
        reason: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::print_stderr, clippy::unwrap_used)]

    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use assistant_protocol::ErrorCode;
    use serde_json::Value;

    use super::{MAX_TASK_ANCHORS, TaskAnchorRegistry, read_utf8_prefix_data};

    fn field<'value>(value: &'value Value, name: &str) -> &'value Value {
        value.get(name).expect("tested field is present")
    }

    struct TempFile {
        path: PathBuf,
    }

    impl TempFile {
        fn new(label: &str, bytes: &[u8]) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "assistant-notepad-registry-{label}-{}-{nanos}.txt",
                std::process::id()
            ));
            std::fs::write(&path, bytes).expect("write temp file");
            Self { path }
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            if let Err(error) = std::fs::remove_file(&self.path) {
                eprintln!("failed to remove {}: {error}", self.path.display());
            }
        }
    }

    #[test]
    fn test_read_utf8_prefix_returns_bounded_text() {
        let file = TempFile::new("bounded", b"abcdef");
        let value = read_utf8_prefix_data(Some(&file.path.to_string_lossy()), 3)
            .expect("bounded read must succeed");
        assert_eq!(field(&value, "text"), "abc");
        assert_eq!(field(&value, "truncated"), true);
        assert_eq!(field(&value, "bytes_read"), 3);
        assert_eq!(field(&value, "bytes_total"), 6);
    }

    #[test]
    fn test_read_utf8_prefix_trims_incomplete_multibyte_suffix() {
        let file = TempFile::new("utf8-boundary", "ab€".as_bytes());
        let value = read_utf8_prefix_data(Some(&file.path.to_string_lossy()), 4)
            .expect("boundary read must succeed");
        assert_eq!(field(&value, "text"), "ab");
        assert_eq!(field(&value, "truncated"), true);
        assert_eq!(field(&value, "bytes_read"), 2);
        assert_eq!(field(&value, "bytes_total"), 5);
    }

    #[test]
    fn test_read_utf8_prefix_handles_one_megabyte_file() {
        const BUDGET: u64 = 1_048_576;
        const FILE_BYTES: usize = 1_048_577;
        let file = TempFile::new("one-megabyte", &vec![b'a'; FILE_BYTES]);
        let value = read_utf8_prefix_data(Some(&file.path.to_string_lossy()), BUDGET)
            .expect("one-megabyte prefix read must succeed");
        assert_eq!(field(&value, "bytes_read"), BUDGET);
        assert_eq!(
            field(&value, "bytes_total"),
            u64::try_from(FILE_BYTES).expect("fixture size fits u64")
        );
        assert_eq!(field(&value, "truncated"), true);
    }

    #[test]
    fn test_read_utf8_prefix_missing_file_returns_target_not_found() {
        let missing = std::env::temp_dir().join("assistant-missing-prefix-file.txt");
        let error = read_utf8_prefix_data(Some(&missing.to_string_lossy()), 8)
            .expect_err("missing file must fail");
        assert_eq!(error.code(), ErrorCode::TargetNotFound);
    }

    #[test]
    fn test_read_utf8_prefix_rejects_relative_and_parent_paths() {
        let relative =
            read_utf8_prefix_data(Some("relative.txt"), 8).expect_err("relative path must fail");
        assert_eq!(relative.code(), ErrorCode::ToolInvalidArgs);

        let parent = std::env::temp_dir()
            .join("child")
            .join("..")
            .join("file.txt");
        let traversal = read_utf8_prefix_data(Some(&parent.to_string_lossy()), 8)
            .expect_err("parent traversal must fail");
        assert_eq!(traversal.code(), ErrorCode::ToolInvalidArgs);
    }

    #[test]
    fn test_read_utf8_prefix_rejects_non_positive_budget() {
        let file = TempFile::new("zero-budget", b"abc");
        let error = read_utf8_prefix_data(Some(&file.path.to_string_lossy()), 0)
            .expect_err("zero budget must fail");
        assert_eq!(error.code(), ErrorCode::ToolInvalidArgs);
    }

    #[test]
    fn test_task_anchor_registry_evicts_oldest_at_capacity() {
        let mut registry: TaskAnchorRegistry<u64> = TaskAnchorRegistry::default();
        let first = registry.insert("task-0000".to_owned(), 1);
        assert_eq!(first, None);
        for index in 1..MAX_TASK_ANCHORS {
            let evicted = registry.insert(format!("task-{index:04}"), index as u64 + 1);
            assert_eq!(
                evicted, None,
                "the cap is not exceeded until the next insert"
            );
        }
        let evicted = registry.insert("task-over-cap".to_owned(), 999);
        assert_eq!(evicted, Some(1), "the oldest anchor must be evicted");
        assert!(registry.get("task-0000").is_none());
        assert_eq!(registry.get("task-over-cap"), Some(&999));
    }
}
