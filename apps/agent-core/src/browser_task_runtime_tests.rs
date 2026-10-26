//! Private tests for the browser task-package runner.
//!
//! The `ToolBus` uses scripted in-process handlers. No browser, profile, network,
//! or GUI is created.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::sync::{Arc, Mutex};

use assistant_agent_core::TaskPackageProvider;
use assistant_protocol::{RiskLevel, ToolEffect, ToolReversibility};
use assistant_tool_bus::{
    CallContext, Clock, MountSelection, SystemClock, ToolBus, ToolBusConfig, ToolBusError,
    ToolDefinition, ToolHandler, ToolOutput, ToolRegistry,
};
use serde_json::{Map, Value, json};

use super::{
    BrowserTaskOptions, browser_tool_schemas, execute_with_bus, extract_browser_output,
    write_output,
};
use crate::browser_runtime::BrowserSelection;

const EDGE_NAVIGATE: &str = "edge.cdp.navigate";
const EDGE_DOM_READ: &str = "edge.cdp.dom_read";
const EDGE_IDLE: &str = "edge.cdp.wait_network_idle";

struct FakeNavigate {
    current_page: Arc<Mutex<usize>>,
}

impl ToolHandler for FakeNavigate {
    fn call(
        &self,
        _call: &CallContext,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        let url = arguments
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| fake_error("navigate URL is missing"))?;
        let page = url
            .rsplit_once("page-")
            .and_then(|(_, suffix)| suffix.strip_suffix(".html"))
            .and_then(|page| page.parse::<usize>().ok())
            .ok_or_else(|| fake_error("navigate URL is not a scripted page"))?;
        let mut state = self
            .current_page
            .lock()
            .map_err(|_| fake_error("fake navigate state is poisoned"))?;
        let before = fingerprint(*state);
        *state = page;
        drop(state);
        let after = fingerprint(page);
        Ok(ToolOutput::json(json!({
            "ready": true,
            "fingerprint_scope": "cdp_target",
            "fingerprint": after,
            "previous_fingerprint": before,
            "elapsed_ms": 1,
        })))
    }
}

struct FakeDomRead {
    current_page: Arc<Mutex<usize>>,
}

impl ToolHandler for FakeDomRead {
    fn call(
        &self,
        _call: &CallContext,
        _arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        let page = *self
            .current_page
            .lock()
            .map_err(|_| fake_error("fake DOM state is poisoned"))?;
        let fingerprint = fingerprint(page);
        Ok(ToolOutput::untrusted(
            assistant_tool_bus::SourceDescriptor::app_content("com.microsoft.edge")
                .with_target("browser_page"),
            json!({
                "dom": format!("<main class=\"product-list\"><article data-page=\"{page}\"></article></main>"),
                "node_count": 2,
                "fingerprint_scope": "cdp_target",
                "fingerprint": fingerprint,
                "previous_fingerprint": fingerprint,
                "elapsed_ms": 0,
            }),
        ))
    }
}

struct FakeIdle;

impl ToolHandler for FakeIdle {
    fn call(
        &self,
        _call: &CallContext,
        _arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        let fingerprint = fingerprint(0);
        Ok(ToolOutput::json(json!({
            "idle": true,
            "fingerprint_scope": "cdp_target",
            "fingerprint": fingerprint,
            "previous_fingerprint": fingerprint,
            "elapsed_ms": 0,
        })))
    }
}

#[test]
fn test_browser_task_options_split_runtime_and_task_flags() {
    let output = std::env::temp_dir().join(format!(
        "assistant-browser-task-options-{}.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&output);
    let expected_output = output.display().to_string();
    let arguments = vec![
        "--task-package".to_owned(),
        "package.json".to_owned(),
        "--output".to_owned(),
        expected_output.clone(),
        "--browser".to_owned(),
        "edge".to_owned(),
        "--browser-executable".to_owned(),
        r"C:\Edge\msedge.exe".to_owned(),
        "--browser-profile-root".to_owned(),
        r"C:\temp\profiles".to_owned(),
    ];
    let options = BrowserTaskOptions::parse(&arguments).expect("options");
    assert_eq!(options.task_package_path.to_string_lossy(), "package.json");
    assert_eq!(options.output_path.display().to_string(), expected_output);
}

#[test]
fn test_browser_output_aliases_are_explicit() {
    let data = json!({"ready": true, "dom": "<main></main>"})
        .as_object()
        .expect("object")
        .clone();
    assert_eq!(
        extract_browser_output("edge.cdp.navigate", "page_1_ready", &data),
        Some(Value::Bool(true))
    );
    assert_eq!(
        extract_browser_output("edge.cdp.dom_read", "page_1_dom", &data),
        Some(Value::String("<main></main>".to_owned()))
    );
    assert_eq!(
        extract_browser_output("edge.cdp.dom_read", "unknown", &data),
        None
    );
}

#[test]
fn test_browser_tool_schemas_exclude_reserved_planner_catalog() {
    let mut schemas = Vec::new();
    for name in [
        EDGE_NAVIGATE,
        EDGE_DOM_READ,
        EDGE_IDLE,
        "assistant.runtime.verify_postconditions",
    ] {
        schemas.push(
            serde_json::from_value(json!({
                "name": name,
                "version": "1.0",
                "description": "schema",
                "input": {"type": "object"},
                "output": {"type": "object"},
                "risk_level": "low",
                "effect": "read",
                "reversibility": "l0_undo_stack",
                "requires_approval": false,
                "tags": [],
            }))
            .expect("schema"),
        );
    }
    let filtered = browser_tool_schemas(schemas, BrowserSelection::Edge).expect("filtered");
    assert_eq!(filtered.len(), 3);
    assert!(
        filtered
            .iter()
            .all(|schema| schema.name.starts_with("edge.cdp."))
    );
}

#[tokio::test]
async fn test_six_step_browser_package_commits_and_exports_declared_outputs()
-> Result<(), Box<dyn std::error::Error>> {
    let current_page = Arc::new(Mutex::new(0usize));
    let (registry, schemas) = scripted_registry(&current_page)?;
    let names = schemas
        .iter()
        .map(|schema| schema.name.clone())
        .collect::<Vec<_>>();
    let (bus, _report) = ToolBus::start(
        registry,
        MountSelection::names(names),
        ToolBusConfig::new("browser-task-runner-test"),
        Arc::new(SystemClock),
    )
    .await?;
    let provider = TaskPackageProvider::from_package_file(&package_path())?;
    let result = execute_with_bus(
        &bus,
        provider,
        schemas,
        "com.microsoft.edge",
        SystemClock.now_unix_ms(),
    )
    .await?;
    bus.shutdown().await?;

    assert_eq!(result.task_id, "edge.t5_1.books_list_to_csv");
    assert_eq!(result.steps.len(), 6);
    assert_eq!(result.outputs.get("page_1_ready"), Some(&Value::Bool(true)));
    assert!(
        result
            .outputs
            .get("page_1_dom")
            .and_then(Value::as_str)
            .is_some_and(|dom| dom.contains("data-page=\"1\""))
    );
    assert!(
        result
            .outputs
            .get("page_3_dom")
            .and_then(Value::as_str)
            .is_some_and(|dom| dom.contains("data-page=\"3\""))
    );
    Ok(())
}

#[test]
fn test_output_writer_creates_new_file_and_refuses_overwrite()
-> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::temp_dir().join(format!(
        "assistant-browser-task-output-{}.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&output);
    let result = super::BrowserTaskResult {
        task_id: "task".to_owned(),
        outputs: json!({"page_1_ready": true})
            .as_object()
            .expect("object")
            .clone(),
        steps: vec![json!({"id": "navigate_page_1"})],
    };
    write_output(&output, &result)?;
    let observed = std::fs::read_to_string(&output)?;
    assert!(observed.contains("\"status\": \"completed\""));
    assert!(write_output(&output, &result).is_err());
    std::fs::remove_file(output)?;
    Ok(())
}

fn scripted_registry(
    current_page: &Arc<Mutex<usize>>,
) -> Result<(ToolRegistry, Vec<assistant_protocol::ToolSchema>), Box<dyn std::error::Error>> {
    let mut registry = ToolRegistry::new();
    let mut schemas = Vec::new();
    for (name, description, input) in [
        (
            EDGE_NAVIGATE,
            "scripted navigation",
            json!({
                "type": "object",
                "properties": {
                    "url": {"type": "string"},
                    "load_condition": {"type": "string"},
                },
                "required": ["url", "load_condition"],
            }),
        ),
        (
            EDGE_DOM_READ,
            "scripted DOM read",
            json!({
                "type": "object",
                "properties": {
                    "selector": {"type": "string"},
                    "max_nodes": {"type": "integer"},
                },
                "required": ["selector", "max_nodes"],
            }),
        ),
        (
            EDGE_IDLE,
            "scripted idle wait",
            json!({
                "type": "object",
                "properties": {"quiet_ms": {"type": "integer"}},
                "required": ["quiet_ms"],
            }),
        ),
    ] {
        let definition = ToolDefinition::new(
            name,
            description,
            RiskLevel::Low,
            ToolEffect::Read,
            ToolReversibility::L0UndoStack,
            input.clone(),
        )?;
        schemas.push(serde_json::from_value(json!({
            "name": name,
            "version": "1.0",
            "description": description,
            "input": input,
            "output": {"type": "object"},
            "risk_level": "low",
            "effect": "read",
            "reversibility": "l0_undo_stack",
            "requires_approval": false,
            "tags": [],
        }))?);
        let handler: Arc<dyn ToolHandler> = if name == EDGE_NAVIGATE {
            Arc::new(FakeNavigate {
                current_page: Arc::clone(current_page),
            })
        } else if name == EDGE_DOM_READ {
            Arc::new(FakeDomRead {
                current_page: Arc::clone(current_page),
            })
        } else {
            Arc::new(FakeIdle)
        };
        registry.register(definition, handler)?;
    }
    Ok((registry, schemas))
}

fn fingerprint(value: usize) -> String {
    format!("sha256:{value:064x}")
}

fn fake_error(reason: impl Into<String>) -> ToolBusError {
    ToolBusError::Mcp {
        code: -32_000,
        message: reason.into(),
    }
}

fn package_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../adapters/browser.edge/tasks/t5.1.books-list-to-csv.json")
}
