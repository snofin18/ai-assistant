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

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use assistant_platform_api::{UiAutomationProvider, WindowProvider};
use assistant_protocol::ToolSchema;
use assistant_tool_bus::{ToolBusError, ToolDefinition, ToolRegistry};
use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;

use crate::notepad_handlers::{NotepadHandlerContext, build_handler_map};
use crate::notepad_targets::NotepadTargetCatalog;

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
}

/// Builds the five declared Notepad tools and their Host handlers.
pub(crate) fn build_notepad_registry<P>(
    platform: Arc<P>,
    targets: Arc<NotepadTargetCatalog>,
    tools_path: &Path,
) -> Result<NotepadRegistryBuild, NotepadRegistryError>
where
    P: WindowProvider + UiAutomationProvider + Send + Sync + 'static,
{
    let body = std::fs::read_to_string(tools_path).map_err(|error| NotepadRegistryError::Read {
        path: tools_path.display().to_string(),
        reason: error.to_string(),
    })?;
    let declared: DeclaredToolsFile =
        serde_json::from_str(&body).map_err(|error| NotepadRegistryError::Malformed {
            reason: error.to_string(),
        })?;
    if declared.app_id != targets.app_id() {
        return Err(NotepadRegistryError::Malformed {
            reason: format!(
                "tools app_id `{}` does not match target app_id `{}`",
                declared.app_id,
                targets.app_id()
            ),
        });
    }
    if declared.tools.len() != EXPECTED_TOOL_NAMES.len() {
        return Err(NotepadRegistryError::Malformed {
            reason: format!(
                "expected {} declared tools, found {}",
                EXPECTED_TOOL_NAMES.len(),
                declared.tools.len()
            ),
        });
    }

    let context = Arc::new(NotepadHandlerContext {
        platform,
        app_id: declared.app_id.clone(),
        targets,
    });
    let handlers = build_handler_map(&context);
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
        if !EXPECTED_TOOL_NAMES.contains(&schema.name.as_str()) {
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
    if names.len() != EXPECTED_TOOL_NAMES.len() {
        let missing = EXPECTED_TOOL_NAMES
            .iter()
            .filter(|name| !names.contains(**name))
            .copied()
            .collect::<Vec<_>>()
            .join(", ");
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
    Ok(NotepadRegistryBuild {
        registry,
        tool_schemas,
    })
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
