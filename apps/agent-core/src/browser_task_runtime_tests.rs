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

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use assistant_agent_core::TaskPackageProvider;
use assistant_tool_bus::{
    CallContext, Clock, MountSelection, SystemClock, ToolBus, ToolBusConfig, ToolBusError,
    ToolDefinition, ToolHandler, ToolOutput, ToolRegistry,
};
use serde_json::{Map, Value, json};

use super::{
    BrowserTaskOptions, PLAN_BUDGET_MS, browser_tool_schemas, execute_with_bus,
    extract_browser_output, write_output,
};
use crate::browser_runtime::BrowserSelection;

const EDGE_NAVIGATE: &str = "edge.cdp.navigate";
const EDGE_DOM_READ: &str = "edge.cdp.dom_read";
const EDGE_FORM_FILL: &str = "edge.cdp.form_fill";
const EDGE_FORM_READ: &str = "edge.cdp.form_read";
const EDGE_FORM_SUBMIT: &str = "edge.cdp.form_submit";
const EDGE_IDLE: &str = "edge.cdp.wait_network_idle";

#[test]
fn test_browser_plan_budget_covers_three_cold_navigation_windows() {
    assert_eq!(PLAN_BUDGET_MS, 180_000);
}

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

fn fake_form_fill_handler(_call: &CallContext, _arguments: &Map<String, Value>) -> ToolOutput {
    let fingerprint = fingerprint(0);
    ToolOutput::json(json!({
        "filled_fields": 1,
        "submitted": false,
        "fingerprint_scope": "cdp_target",
        "fingerprint": fingerprint,
        "previous_fingerprint": fingerprint,
        "elapsed_ms": 0,
    }))
}

fn fake_form_read_handler(_call: &CallContext, _arguments: &Map<String, Value>) -> ToolOutput {
    let fingerprint = fingerprint(0);
    ToolOutput::untrusted(
        assistant_tool_bus::SourceDescriptor::app_content("com.microsoft.edge")
            .with_target("browser_form"),
        json!({
            "origin": "http://127.0.0.1:8765",
            "action": "http://127.0.0.1:8765/submitted",
            "fields": [{"selector": "#name", "value": ""}],
            "fingerprint_scope": "cdp_target",
            "fingerprint": fingerprint,
            "previous_fingerprint": fingerprint,
            "elapsed_ms": 0,
        }),
    )
}

fn fake_form_submit_handler(_call: &CallContext, _arguments: &Map<String, Value>) -> ToolOutput {
    ToolOutput::untrusted(
        assistant_tool_bus::SourceDescriptor::app_content("com.microsoft.edge")
            .with_target("browser_form"),
        json!({
            "submitted": true,
            "post_url": "http://127.0.0.1:8765/submitted",
        }),
    )
}

#[test]
fn test_browser_task_options_split_runtime_and_task_flags() {
    let output = std::env::temp_dir().join(format!(
        "assistant-browser-task-options-{}.json",
        std::process::id()
    ));
    let csv_output = std::env::temp_dir().join(format!(
        "assistant-browser-task-options-{}.csv",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&output);
    let _ = std::fs::remove_file(&csv_output);
    let expected_output = output.display().to_string();
    let expected_csv = csv_output.display().to_string();
    let arguments = vec![
        "--task-package".to_owned(),
        "package.json".to_owned(),
        "--output".to_owned(),
        expected_output.clone(),
        "--csv-output".to_owned(),
        expected_csv.clone(),
        "--csv-base-url".to_owned(),
        "http://127.0.0.1:8765".to_owned(),
        "--approve-step".to_owned(),
        "navigate_page_1".to_owned(),
        "--approve-step".to_owned(),
        "navigate_page_2".to_owned(),
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
    let csv = options.csv.expect("CSV options");
    assert_eq!(csv.output_path.display().to_string(), expected_csv);
    assert_eq!(
        options.approved_step_ids,
        vec!["navigate_page_1", "navigate_page_2"]
    );
}

#[test]
fn test_browser_csv_options_must_be_paired_and_use_http_origin() {
    let output = std::env::temp_dir().join(format!(
        "assistant-browser-task-csv-pair-{}.json",
        std::process::id()
    ));
    let csv = std::env::temp_dir().join(format!(
        "assistant-browser-task-csv-pair-{}.csv",
        std::process::id()
    ));
    let base = vec![
        "--task-package".to_owned(),
        "package.json".to_owned(),
        "--output".to_owned(),
        output.display().to_string(),
        "--csv-output".to_owned(),
        csv.display().to_string(),
        "--browser".to_owned(),
        "edge".to_owned(),
        "--browser-executable".to_owned(),
        r"C:\Edge\msedge.exe".to_owned(),
        "--browser-profile-root".to_owned(),
        r"C:\temp\profiles".to_owned(),
    ];
    let error = BrowserTaskOptions::parse(&base).expect_err("CSV base URL is required");
    assert!(error.to_string().contains("--csv-base-url"));

    let mut invalid = base;
    invalid.extend([
        "--csv-base-url".to_owned(),
        "file:///tmp/example".to_owned(),
    ]);
    let error = BrowserTaskOptions::parse(&invalid).expect_err("non-HTTP origin must fail");
    assert!(error.to_string().contains("--csv-base-url is invalid"));
}

#[test]
fn test_browser_task_options_reject_duplicate_step_approvals() {
    let output = std::env::temp_dir().join(format!(
        "assistant-browser-task-duplicate-approval-{}.json",
        std::process::id()
    ));
    let arguments = vec![
        "--task-package".to_owned(),
        "package.json".to_owned(),
        "--output".to_owned(),
        output.display().to_string(),
        "--approve-step".to_owned(),
        "navigate_page_1".to_owned(),
        "--approve-step".to_owned(),
        "navigate_page_1".to_owned(),
        "--browser".to_owned(),
        "edge".to_owned(),
        "--browser-executable".to_owned(),
        r"C:\Edge\msedge.exe".to_owned(),
        "--browser-profile-root".to_owned(),
        r"C:\temp\profiles".to_owned(),
    ];
    let error =
        BrowserTaskOptions::parse(&arguments).expect_err("duplicate step approval must fail");
    assert!(error.to_string().contains("provided more than once"));
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
        EDGE_FORM_FILL,
        EDGE_FORM_READ,
        EDGE_FORM_SUBMIT,
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
    assert_eq!(filtered.len(), 6);
    assert!(
        filtered
            .iter()
            .all(|schema| schema.name.starts_with("edge.cdp."))
    );
}

#[tokio::test]
async fn test_unapproved_authoritative_write_waits_for_explicit_step_approval() {
    let error = scripted_run(&[])
        .await
        .expect_err("missing navigate approval must stop the runner");
    assert!(
        error
            .to_string()
            .contains("step `navigate_page_1` ended as AwaitingApproval"),
        "{error}"
    );
}

#[tokio::test]
async fn test_invalid_step_approvals_fail_closed() {
    for (approvals, expected) in [
        (vec!["unknown_step".to_owned()], "not present"),
        (vec!["read_page_1".to_owned()], "read-only"),
        (
            (1..=7)
                .map(|index| format!("navigate_page_{index}"))
                .collect(),
            "exceed the plan step limit",
        ),
    ] {
        let error = scripted_run(&approvals)
            .await
            .expect_err("invalid step approval must fail");
        assert!(error.to_string().contains(expected), "{error}");
    }
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
    let approvals = vec![
        "navigate_page_1".to_owned(),
        "navigate_page_2".to_owned(),
        "navigate_page_3".to_owned(),
    ];
    let result = execute_with_bus(
        &bus,
        provider,
        schemas,
        "com.microsoft.edge",
        SystemClock.now_unix_ms(),
        &approvals,
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
    write_output(&output, &result, None, None)?;
    let observed = std::fs::read_to_string(&output)?;
    assert!(observed.contains("\"status\": \"completed\""));
    assert!(write_output(&output, &result, None, None).is_err());
    std::fs::remove_file(output)?;
    Ok(())
}

#[test]
fn test_output_writer_bundles_json_and_csv() -> Result<(), Box<dyn std::error::Error>> {
    let json_path = std::env::temp_dir().join(format!(
        "assistant-browser-task-bundle-{}.json",
        std::process::id()
    ));
    let csv_path = std::env::temp_dir().join(format!(
        "assistant-browser-task-bundle-{}.csv",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&json_path);
    let _ = std::fs::remove_file(&csv_path);
    let result = super::BrowserTaskResult {
        task_id: "task".to_owned(),
        outputs: json!({"page_1_ready": true})
            .as_object()
            .expect("object")
            .clone(),
        steps: vec![json!({"id": "navigate_page_1"})],
    };
    let csv = super::browser_task_csv::CsvArtifact {
        bytes: b"page_number\r\n1\r\n".to_vec(),
        rows: 1,
        first_product_url: "http://127.0.0.1:8765/one".to_owned(),
        last_product_url: "http://127.0.0.1:8765/one".to_owned(),
        expected_origin: "http://127.0.0.1:8765".to_owned(),
    };
    write_output(&json_path, &result, Some(&csv_path), Some(&csv))?;
    let json_bytes = std::fs::read(&json_path)?;
    assert!(std::str::from_utf8(&json_bytes)?.contains("\"csv_output\""));
    assert_eq!(std::fs::read(&csv_path)?, csv.bytes);
    std::fs::remove_file(json_path)?;
    std::fs::remove_file(csv_path)?;
    Ok(())
}

#[test]
fn test_output_writer_cleans_json_without_touching_raced_csv()
-> Result<(), Box<dyn std::error::Error>> {
    let json_path = std::env::temp_dir().join(format!(
        "assistant-browser-task-cleanup-{}.json",
        std::process::id()
    ));
    let csv_path = std::env::temp_dir().join(format!(
        "assistant-browser-task-cleanup-{}.csv",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&json_path);
    std::fs::write(&csv_path, b"preexisting")?;
    let result = super::BrowserTaskResult {
        task_id: "task".to_owned(),
        outputs: Map::new(),
        steps: Vec::new(),
    };
    let csv = super::browser_task_csv::CsvArtifact {
        bytes: b"new".to_vec(),
        rows: 1,
        first_product_url: "http://127.0.0.1:8765/one".to_owned(),
        last_product_url: "http://127.0.0.1:8765/one".to_owned(),
        expected_origin: "http://127.0.0.1:8765".to_owned(),
    };
    assert!(write_output(&json_path, &result, Some(&csv_path), Some(&csv)).is_err());
    assert!(!json_path.exists());
    assert_eq!(std::fs::read(&csv_path)?, b"preexisting");
    std::fs::remove_file(csv_path)?;
    Ok(())
}

async fn scripted_run(
    approvals: &[String],
) -> Result<super::BrowserTaskResult, super::browser_task_support::BrowserTaskError> {
    let current_page = Arc::new(Mutex::new(0usize));
    let (registry, schemas) = scripted_registry(&current_page).map_err(|error| {
        super::browser_task_support::argument_error(format!("scripted registry: {error}"))
    })?;
    let names = schemas
        .iter()
        .map(|schema| schema.name.clone())
        .collect::<Vec<_>>();
    let (bus, _report) = ToolBus::start(
        registry,
        MountSelection::names(names),
        ToolBusConfig::new("browser-task-approval-test"),
        Arc::new(SystemClock),
    )
    .await
    .map_err(super::BrowserTaskError::from)?;
    let provider = TaskPackageProvider::from_package_file(&package_path()).map_err(|error| {
        super::browser_task_support::argument_error(format!("task provider: {error}"))
    })?;
    let result = execute_with_bus(
        &bus,
        provider,
        schemas,
        "com.microsoft.edge",
        SystemClock.now_unix_ms(),
        approvals,
    )
    .await;
    bus.shutdown()
        .await
        .map_err(super::BrowserTaskError::from)?;
    result
}

fn scripted_registry(
    current_page: &Arc<Mutex<usize>>,
) -> Result<(ToolRegistry, Vec<assistant_protocol::ToolSchema>), Box<dyn std::error::Error>> {
    let mut registry = ToolRegistry::new();
    let schemas = authoritative_browser_schemas()?;
    for schema in &schemas {
        let definition = ToolDefinition::new(
            schema.name.clone(),
            schema.description.clone(),
            schema.risk_level,
            schema.effect,
            schema.reversibility,
            schema.input.clone(),
        )?
        .with_output_schema(schema.output.clone())?
        .with_idempotent(schema.idempotent)?
        .with_requires_approval(schema.requires_approval)?
        .with_tags(schema.tags.clone());
        let handler: Arc<dyn ToolHandler> = if schema.name == EDGE_NAVIGATE {
            Arc::new(FakeNavigate {
                current_page: Arc::clone(current_page),
            })
        } else if schema.name == EDGE_DOM_READ {
            Arc::new(FakeDomRead {
                current_page: Arc::clone(current_page),
            })
        } else if schema.name == EDGE_FORM_FILL {
            Arc::new(|call: &CallContext, arguments: &Map<String, Value>| {
                Ok(fake_form_fill_handler(call, arguments))
            })
        } else if schema.name == EDGE_FORM_READ {
            Arc::new(|call: &CallContext, arguments: &Map<String, Value>| {
                Ok(fake_form_read_handler(call, arguments))
            })
        } else if schema.name == EDGE_FORM_SUBMIT {
            Arc::new(|call: &CallContext, arguments: &Map<String, Value>| {
                Ok(fake_form_submit_handler(call, arguments))
            })
        } else {
            Arc::new(FakeIdle)
        };
        registry.register(definition, handler)?;
    }
    Ok((registry, schemas))
}

fn authoritative_browser_schemas()
-> Result<Vec<assistant_protocol::ToolSchema>, Box<dyn std::error::Error>> {
    let root: Value = serde_json::from_str(&std::fs::read_to_string(tools_path())?)?;
    let tools = root
        .get("tools")
        .and_then(Value::as_array)
        .ok_or("browser tools.json must contain a tools array")?;
    let mut schemas = BTreeMap::new();
    for tool in tools {
        let mut schema = tool
            .get("schema")
            .cloned()
            .ok_or("browser tool entry must contain a schema")?;
        let object = schema
            .as_object_mut()
            .ok_or("browser tool schema must be an object")?;
        if object.get("effect").and_then(Value::as_str) == Some("read")
            && object.get("reversibility").and_then(Value::as_str) == Some("none_readonly")
        {
            object.insert(
                "reversibility".to_owned(),
                Value::String("l0_undo_stack".to_owned()),
            );
        }
        let schema: assistant_protocol::ToolSchema = serde_json::from_value(schema)?;
        schemas.insert(schema.name.clone(), schema);
    }
    [
        EDGE_NAVIGATE,
        EDGE_DOM_READ,
        EDGE_FORM_FILL,
        EDGE_FORM_READ,
        EDGE_FORM_SUBMIT,
        EDGE_IDLE,
    ]
    .into_iter()
    .map(|name| {
        schemas
            .remove(name)
            .ok_or_else(|| format!("authoritative browser schema `{name}` is missing").into())
    })
    .collect()
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

fn tools_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../adapters/browser.edge/tools/tools.json")
}
