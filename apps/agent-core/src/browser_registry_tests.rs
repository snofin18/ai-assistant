//! Unit and `ToolBus` contract tests for browser production handlers.
//!
//! The runtime is fully scripted: no browser, profile, network, or GUI is created.

use std::sync::Mutex;

use assistant_platform_api::{
    CdpDomSnapshot, CdpLoadCondition, ErrorCode, PlatformError, PlatformResult, ResolvedCdpSession,
};
use assistant_tool_bus::{
    CallContext, MountSelection, SystemClock, ToolBus, ToolBusConfig, ToolBusError,
};
use serde_json::{Map, Value, json};

use super::*;
use crate::browser_handlers::{BrowserHandlerContext, build_handler_map};

const EDGE_TOOLS: &str = "adapters/browser.edge/tools/tools.json";

#[derive(Default)]
struct FakeRuntime {
    state: Mutex<FakeState>,
}

#[derive(Default)]
struct FakeState {
    current_page: usize,
    navigate_calls: Vec<(String, CdpLoadCondition)>,
    dom_calls: Vec<String>,
    idle_calls: Vec<u64>,
    dom_response: Option<String>,
    dom_error: Option<ErrorCode>,
}

impl BrowserCdpRuntime for FakeRuntime {
    fn navigate(
        &self,
        _target: &ResolvedCdpTarget,
        url: &str,
        condition: &CdpLoadCondition,
    ) -> PlatformResult<()> {
        let mut state = self.state.lock().map_err(|_| {
            PlatformError::new(ErrorCode::Fatal, "fake browser runtime mutex poisoned")
        })?;
        state
            .navigate_calls
            .push((url.to_owned(), condition.clone()));
        state.current_page = url
            .rsplit_once("page-")
            .and_then(|(_, suffix)| suffix.strip_suffix(".html"))
            .and_then(|page| page.parse::<usize>().ok())
            .unwrap_or(0);
        drop(state);
        Ok(())
    }

    fn evaluate_dom(
        &self,
        _target: &ResolvedCdpTarget,
        expression: &str,
    ) -> PlatformResult<CdpDomSnapshot> {
        let (dom, error, current_page) = {
            let mut state = self.state.lock().map_err(|_| {
                PlatformError::new(ErrorCode::Fatal, "fake browser runtime mutex poisoned")
            })?;
            state.dom_calls.push(expression.to_owned());
            (
                state.dom_response.clone(),
                state.dom_error,
                state.current_page,
            )
        };
        if let Some(code) = error {
            return Err(PlatformError::new(code, "scripted DOM failure"));
        }
        let dom = dom.unwrap_or_else(|| {
            if expression.contains("querySelector") {
                json!({
                    "dom": format!("<main class=\"product-list\"><article data-page=\"{current_page}\"></article></main>"),
                    "node_count": 1,
                })
                .to_string()
            } else {
                String::new()
            }
        });
        let url = if current_page == 0 {
            "about:blank".to_owned()
        } else {
            format!("http://127.0.0.1:8765/page-{current_page}.html")
        };
        Ok(CdpDomSnapshot::new(
            &url,
            if current_page == 0 {
                "about:blank"
            } else {
                "fixture"
            },
            None,
            dom,
        ))
    }

    fn wait_network_idle(&self, _target: &ResolvedCdpTarget, quiet_ms: u64) -> PlatformResult<()> {
        self.state
            .lock()
            .map_err(|_| {
                PlatformError::new(ErrorCode::Fatal, "fake browser runtime mutex poisoned")
            })?
            .idle_calls
            .push(quiet_ms);
        Ok(())
    }
}

#[test]
fn test_dom_read_handler_returns_untrusted_bounded_payload()
-> Result<(), Box<dyn std::error::Error>> {
    let runtime = Arc::new(FakeRuntime::default());
    let context = Arc::new(BrowserHandlerContext {
        runtime: Arc::clone(&runtime),
        target: browser_target(),
        app_id: BrowserFamily::Edge.app_id().to_owned(),
    });
    let handlers = build_handler_map(&context, &BrowserFamily::Edge.tool_names());
    let handler = handlers
        .get("edge.cdp.dom_read")
        .ok_or("missing DOM handler")?;
    let output = handler.call(
        &CallContext::new("task", "step"),
        &arguments(json!({"selector": "main.product-list", "max_nodes": 10}))?,
    )?;

    assert!(output.is_untrusted());
    let dom_calls = {
        let recorded = runtime.state.lock().map_err(|_| "fake mutex poisoned")?;
        recorded.dom_calls.clone()
    };
    let expression = dom_calls.first().ok_or("DOM call was not recorded")?;
    assert_eq!(dom_calls.len(), 1);
    assert!(expression.contains("\"main.product-list\""));
    assert!(expression.contains("slice(10)"));
    Ok(())
}

#[tokio::test]
async fn test_edge_registry_runs_three_tools_through_real_toolbus()
-> Result<(), Box<dyn std::error::Error>> {
    let runtime = Arc::new(FakeRuntime::default());
    let build = build_browser_registry_with_runtime(
        Arc::clone(&runtime),
        browser_target(),
        BrowserFamily::Edge,
        &tools_path(BrowserFamily::Edge),
    )?;
    assert_eq!(build.registry().len(), 3);
    assert!(build.registry().contains("edge.cdp.navigate"));
    assert!(build.registry().contains("edge.cdp.dom_read"));
    assert!(build.registry().contains("edge.cdp.wait_network_idle"));
    let (registry, schemas) = build.into_parts();
    assert!(
        schemas
            .iter()
            .any(|schema| schema.name == "edge.cdp.navigate")
    );
    assert!(
        schemas
            .iter()
            .any(|schema| schema.name == "edge.cdp.dom_read")
    );
    assert!(
        schemas
            .iter()
            .any(|schema| schema.name == "edge.cdp.wait_network_idle")
    );

    let (bus, _report) = ToolBus::start(
        registry,
        MountSelection::all(),
        ToolBusConfig::new("browser-handler-test"),
        Arc::new(SystemClock),
    )
    .await?;
    call_edge_tools(&bus).await?;
    bus.shutdown().await?;

    let (navigate_calls, idle_calls) = {
        let recorded = runtime.state.lock().map_err(|_| "fake mutex poisoned")?;
        (recorded.navigate_calls.clone(), recorded.idle_calls.clone())
    };
    let (_, condition) = navigate_calls
        .first()
        .ok_or("navigate call was not recorded")?;
    assert_eq!(navigate_calls.len(), 1);
    assert_eq!(idle_calls, vec![250]);
    assert!(matches!(
        condition,
        CdpLoadCondition::NetworkIdle { quiet_ms: 500 }
    ));
    Ok(())
}

async fn call_edge_tools(bus: &ToolBus) -> Result<(), ToolBusError> {
    let call = CallContext::new("task", "step");
    let navigate = bus
        .call_tool(
            "edge.cdp.navigate",
            arguments(
                json!({"url": "http://127.0.0.1:8765/page-1.html", "load_condition": "network_idle"}),
            )?,
            &call,
        )
        .await?;
    assert!(navigate.ok);
    let navigate_data = navigate.data.as_ref().map(|data| &data.0);
    assert_eq!(
        navigate_data
            .and_then(|data| data.get("fingerprint_scope"))
            .and_then(Value::as_str),
        Some("cdp_target")
    );
    assert_ne!(
        navigate_data
            .and_then(|data| data.get("fingerprint"))
            .and_then(Value::as_str),
        navigate_data
            .and_then(|data| data.get("previous_fingerprint"))
            .and_then(Value::as_str)
    );
    let dom = bus
        .call_tool(
            "edge.cdp.dom_read",
            arguments(json!({"selector": "main", "max_nodes": 1000}))?,
            &call,
        )
        .await?;
    assert!(dom.ok);
    let dom_data = dom.data.as_ref().map(|data| &data.0);
    assert_eq!(
        dom_data
            .and_then(|data| data.get("fingerprint"))
            .and_then(Value::as_str),
        dom_data
            .and_then(|data| data.get("previous_fingerprint"))
            .and_then(Value::as_str)
    );
    let idle = bus
        .call_tool(
            "edge.cdp.wait_network_idle",
            arguments(json!({"quiet_ms": 250}))?,
            &call,
        )
        .await?;
    assert!(idle.ok);
    Ok(())
}

#[test]
fn test_chrome_registry_mounts_only_implemented_tools() -> Result<(), Box<dyn std::error::Error>> {
    let build = build_browser_registry_with_runtime(
        Arc::new(FakeRuntime::default()),
        browser_target(),
        BrowserFamily::Chrome,
        &tools_path(BrowserFamily::Chrome),
    )?;
    assert_eq!(build.registry().len(), 3);
    assert!(build.registry().contains("chrome.cdp.navigate"));
    assert!(!build.registry().contains("chrome.cdp.form_fill"));
    assert!(!build.registry().contains("chrome.cdp.download"));
    assert!(!build.registry().contains("chrome.cdp.screenshot"));
    Ok(())
}

#[test]
fn test_invalid_arguments_fail_before_runtime() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = Arc::new(FakeRuntime::default());
    let context = Arc::new(BrowserHandlerContext {
        runtime: Arc::clone(&runtime),
        target: browser_target(),
        app_id: BrowserFamily::Edge.app_id().to_owned(),
    });
    let handlers = build_handler_map(&context, &BrowserFamily::Edge.tool_names());
    let navigate = handlers
        .get("edge.cdp.navigate")
        .ok_or("missing navigate handler")?;
    let error = navigate
        .call(
            &CallContext::new("task", "step"),
            &arguments(json!({"url": "file:///tmp/page", "load_condition": "load"}))?,
        )
        .err()
        .ok_or("file URL must be rejected")?;
    assert!(matches!(error, ToolBusError::InvalidArguments { .. }));

    let dom = handlers
        .get("edge.cdp.dom_read")
        .ok_or("missing DOM handler")?;
    let error = dom
        .call(
            &CallContext::new("task", "step"),
            &arguments(json!({"selector": "main", "max_nodes": 1001}))?,
        )
        .err()
        .ok_or("max_nodes above 1000 must be rejected")?;
    assert!(matches!(error, ToolBusError::InvalidArguments { .. }));
    assert!(
        runtime
            .state
            .lock()
            .map_err(|_| "fake mutex poisoned")?
            .navigate_calls
            .is_empty()
    );
    Ok(())
}

#[test]
fn test_malformed_dom_payload_fails_closed() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = Arc::new(FakeRuntime::default());
    runtime
        .state
        .lock()
        .map_err(|_| "fake mutex poisoned")?
        .dom_response = Some("not-json".to_owned());
    let context = Arc::new(BrowserHandlerContext {
        runtime,
        target: browser_target(),
        app_id: BrowserFamily::Edge.app_id().to_owned(),
    });
    let handlers = build_handler_map(&context, &BrowserFamily::Edge.tool_names());
    let error = handlers
        .get("edge.cdp.dom_read")
        .ok_or("missing DOM handler")?
        .call(
            &CallContext::new("task", "step"),
            &arguments(json!({"selector": "main", "max_nodes": 10}))?,
        )
        .err()
        .ok_or("malformed DOM payload must fail")?;
    assert!(matches!(error, ToolBusError::Mcp { .. }));
    Ok(())
}

fn browser_target() -> ResolvedCdpTarget {
    ResolvedCdpTarget::new(ResolvedCdpSession::new(1), 1)
}

fn arguments(value: Value) -> Result<Map<String, Value>, ToolBusError> {
    match value {
        Value::Object(map) => Ok(map),
        _ => Err(ToolBusError::InvalidArguments {
            tool: "browser-test".to_owned(),
            reason: "test arguments must be an object".to_owned(),
        }),
    }
}

fn tools_path(family: BrowserFamily) -> std::path::PathBuf {
    let adapter = match family {
        BrowserFamily::Edge => EDGE_TOOLS,
        BrowserFamily::Chrome => "adapters/browser.chrome/tools/tools.json",
    };
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(adapter)
}
