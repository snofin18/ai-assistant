//! Production handlers for the implemented browser CDP tools.
//!
//! Responsibilities:
//! - translate validated tool arguments into calls on the existing CDP runtime;
//! - keep DOM reads bounded and mark returned page content as untrusted;
//! - publish target-scoped before/after fingerprints for runtime verification;
//! - map platform failures through the shared handler error boundary.
//!
//! Boundaries:
//! - does not launch or close a browser;
//! - does not decide policy, approval, or site eligibility;
//! - only exposes the dedicated form submit handler; does not implement download,
//!   screenshot, or generic click.
//!
//! Invariants:
//! 1. a provider future that is unexpectedly pending fails closed;
//! 2. DOM content is always returned through `app_content` trust metadata;
//! 3. browser fingerprints always use the `cdp_target` scope;
//! 4. unimplemented browser tools are never registered by the companion registry.

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use std::time::Instant;

use assistant_platform_api::{
    CdpDomSnapshot, CdpLoadCondition, CdpProvider, ErrorCode, PlatformError, PlatformResult,
    ResolvedCdpTarget,
};
use assistant_platform_windows::cdp::runtime::RuntimeCdpProvider;
use assistant_protocol::serde_json;
use assistant_storage::BlobId;
use assistant_tool_bus::{CallContext, SourceDescriptor, ToolBusError, ToolHandler, ToolOutput};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::handler_support as support;

/// Edge tool name for navigation.
const EDGE_TOOL_NAVIGATE: &str = "edge.cdp.navigate";
/// Edge tool name for DOM reads.
const EDGE_TOOL_DOM_READ: &str = "edge.cdp.dom_read";
/// Edge tool name for form filling.
const EDGE_TOOL_FORM_FILL: &str = "edge.cdp.form_fill";
const EDGE_TOOL_FORM_READ: &str = "edge.cdp.form_read";
const EDGE_TOOL_FORM_SUBMIT: &str = "edge.cdp.form_submit";
/// Edge tool name for network-idle waits.
const EDGE_TOOL_WAIT_NETWORK_IDLE: &str = "edge.cdp.wait_network_idle";

/// Chrome tool name for navigation.
const CHROME_TOOL_NAVIGATE: &str = "chrome.cdp.navigate";
/// Chrome tool name for DOM reads.
const CHROME_TOOL_DOM_READ: &str = "chrome.cdp.dom_read";
/// Chrome tool name for form filling.
const CHROME_TOOL_FORM_FILL: &str = "chrome.cdp.form_fill";
const CHROME_TOOL_FORM_READ: &str = "chrome.cdp.form_read";
const CHROME_TOOL_FORM_SUBMIT: &str = "chrome.cdp.form_submit";
/// Chrome tool name for network-idle waits.
const CHROME_TOOL_WAIT_NETWORK_IDLE: &str = "chrome.cdp.wait_network_idle";

const DEFAULT_NAVIGATION_QUIET_MS: u64 = 500;
const MAX_SELECTOR_BYTES: usize = 4 * 1024;
const MAX_URL_BYTES: usize = 8 * 1024;
const MAX_DOM_NODES: u64 = 1_000;
const MAX_QUIET_MS: u64 = 10_000;
const CDP_TARGET_FINGERPRINT_SCOPE: &str = "cdp_target";
const CDP_IDENTITY_EXPRESSION: &str =
    "JSON.stringify({ url: location.href, title: document.title })";

#[path = "browser_form_fill.rs"]
mod form_fill;
#[path = "browser_form_read.rs"]
mod form_read;
#[path = "browser_form_submit.rs"]
mod form_submit;

use form_fill::FormFillHandler;
use form_read::FormReadHandler;
use form_submit::FormSubmitHandler;

/// Synchronous boundary used by handlers and tests around the asynchronous CDP trait.
pub trait BrowserCdpRuntime: Send + Sync {
    fn navigate(
        &self,
        target: &ResolvedCdpTarget,
        url: &str,
        condition: &CdpLoadCondition,
    ) -> PlatformResult<()>;

    fn evaluate_dom(
        &self,
        target: &ResolvedCdpTarget,
        expression: &str,
    ) -> PlatformResult<CdpDomSnapshot>;

    fn wait_network_idle(&self, target: &ResolvedCdpTarget, quiet_ms: u64) -> PlatformResult<()>;
}

impl BrowserCdpRuntime for RuntimeCdpProvider {
    fn navigate(
        &self,
        target: &ResolvedCdpTarget,
        url: &str,
        condition: &CdpLoadCondition,
    ) -> PlatformResult<()> {
        poll_ready(
            CdpProvider::navigate(self, target, url, condition),
            "browser CDP navigate",
        )?
    }

    fn evaluate_dom(
        &self,
        target: &ResolvedCdpTarget,
        expression: &str,
    ) -> PlatformResult<CdpDomSnapshot> {
        poll_ready(
            CdpProvider::evaluate_dom(self, target, expression),
            "browser CDP evaluate_dom",
        )?
    }

    fn wait_network_idle(&self, target: &ResolvedCdpTarget, quiet_ms: u64) -> PlatformResult<()> {
        poll_ready(
            Self::wait_network_idle(self, target, quiet_ms),
            "browser CDP wait_network_idle",
        )?
    }
}

pub struct BrowserHandlerContext<R> {
    pub(crate) runtime: Arc<R>,
    pub(crate) target: ResolvedCdpTarget,
    pub(crate) app_id: String,
}

pub fn build_handler_map<R>(
    context: &Arc<BrowserHandlerContext<R>>,
    tool_names: &BrowserToolNames,
) -> BTreeMap<String, Arc<dyn ToolHandler>>
where
    R: BrowserCdpRuntime + 'static,
{
    let mut handlers: BTreeMap<String, Arc<dyn ToolHandler>> = BTreeMap::new();
    handlers.insert(
        tool_names.navigate.clone(),
        Arc::new(NavigateHandler {
            context: Arc::clone(context),
            tool_name: tool_names.navigate.clone(),
        }),
    );
    handlers.insert(
        tool_names.dom_read.clone(),
        Arc::new(DomReadHandler {
            context: Arc::clone(context),
            tool_name: tool_names.dom_read.clone(),
        }),
    );
    handlers.insert(
        tool_names.form_fill.clone(),
        Arc::new(FormFillHandler {
            context: Arc::clone(context),
            tool_name: tool_names.form_fill.clone(),
        }),
    );
    handlers.insert(
        tool_names.form_read.clone(),
        Arc::new(FormReadHandler {
            context: Arc::clone(context),
            tool_name: tool_names.form_read.clone(),
        }),
    );
    handlers.insert(
        tool_names.form_submit.clone(),
        Arc::new(FormSubmitHandler {
            context: Arc::clone(context),
            tool_name: tool_names.form_submit.clone(),
        }),
    );
    handlers.insert(
        tool_names.wait_network_idle.clone(),
        Arc::new(WaitNetworkIdleHandler {
            context: Arc::clone(context),
            tool_name: tool_names.wait_network_idle.clone(),
        }),
    );
    handlers
}

/// Edge or Chrome browser tool family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserToolFamily {
    /// Microsoft Edge adapter.
    Edge,
    /// Google Chrome adapter.
    Chrome,
}

impl BrowserToolFamily {
    pub(crate) const fn app_id(self) -> &'static str {
        match self {
            Self::Edge => "com.microsoft.edge",
            Self::Chrome => "com.google.chrome",
        }
    }

    pub(crate) fn tool_names(self) -> BrowserToolNames {
        match self {
            Self::Edge => BrowserToolNames {
                navigate: EDGE_TOOL_NAVIGATE.to_owned(),
                dom_read: EDGE_TOOL_DOM_READ.to_owned(),
                form_fill: EDGE_TOOL_FORM_FILL.to_owned(),
                form_read: EDGE_TOOL_FORM_READ.to_owned(),
                form_submit: EDGE_TOOL_FORM_SUBMIT.to_owned(),
                wait_network_idle: EDGE_TOOL_WAIT_NETWORK_IDLE.to_owned(),
            },
            Self::Chrome => BrowserToolNames {
                navigate: CHROME_TOOL_NAVIGATE.to_owned(),
                dom_read: CHROME_TOOL_DOM_READ.to_owned(),
                form_fill: CHROME_TOOL_FORM_FILL.to_owned(),
                form_read: CHROME_TOOL_FORM_READ.to_owned(),
                form_submit: CHROME_TOOL_FORM_SUBMIT.to_owned(),
                wait_network_idle: CHROME_TOOL_WAIT_NETWORK_IDLE.to_owned(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserToolNames {
    pub(crate) navigate: String,
    pub(crate) dom_read: String,
    pub(crate) form_fill: String,
    pub(crate) form_read: String,
    pub(crate) form_submit: String,
    pub(crate) wait_network_idle: String,
}

impl BrowserToolNames {
    pub(crate) fn as_vec(&self) -> Vec<String> {
        // form_submit stays handler-only until adapter schema and registry wiring land.
        vec![
            self.navigate.clone(),
            self.dom_read.clone(),
            self.form_fill.clone(),
            self.form_read.clone(),
            self.wait_network_idle.clone(),
        ]
    }
}

struct NavigateHandler<R> {
    context: Arc<BrowserHandlerContext<R>>,
    tool_name: String,
}

impl<R> ToolHandler for NavigateHandler<R>
where
    R: BrowserCdpRuntime + 'static,
{
    fn call(
        &self,
        _call: &CallContext,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        let url = required_string(arguments, "url", &self.tool_name)?;
        validate_http_url(url, &self.tool_name)?;
        let requested_condition = required_string(arguments, "load_condition", &self.tool_name)?;
        let condition = match requested_condition {
            "load" => CdpLoadCondition::Load,
            "network_idle" => CdpLoadCondition::NetworkIdle {
                quiet_ms: DEFAULT_NAVIGATION_QUIET_MS,
            },
            _ => {
                return Err(support::invalid_arguments(
                    &self.tool_name,
                    "load_condition must be `load` or `network_idle`",
                ));
            }
        };
        let started = Instant::now();
        let before = self
            .context
            .runtime
            .evaluate_dom(&self.context.target, CDP_IDENTITY_EXPRESSION)
            .map_err(|error| support::map_platform_error(&self.tool_name, &error))?;
        self.context
            .runtime
            .navigate(&self.context.target, url, &condition)
            .map_err(|error| support::map_platform_error(&self.tool_name, &error))?;
        let after = self
            .context
            .runtime
            .evaluate_dom(&self.context.target, CDP_IDENTITY_EXPRESSION)
            .map_err(|error| support::map_platform_error(&self.tool_name, &error))?;
        let before_fingerprint = identity_fingerprint(&before)?;
        let after_fingerprint = identity_fingerprint(&after)?;
        Ok(ToolOutput::json(json!({
            "ready": true,
            "load_condition": requested_condition,
            "fingerprint_scope": CDP_TARGET_FINGERPRINT_SCOPE,
            "fingerprint": after_fingerprint,
            "previous_fingerprint": before_fingerprint,
            "elapsed_ms": elapsed_millis(started.elapsed()),
        })))
    }
}

struct DomReadHandler<R> {
    context: Arc<BrowserHandlerContext<R>>,
    tool_name: String,
}

impl<R> ToolHandler for DomReadHandler<R>
where
    R: BrowserCdpRuntime + 'static,
{
    fn call(
        &self,
        _call: &CallContext,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        let selector = required_string(arguments, "selector", &self.tool_name)?;
        if selector.len() > MAX_SELECTOR_BYTES {
            return Err(support::invalid_arguments(
                &self.tool_name,
                "selector exceeds 4 KiB",
            ));
        }
        let max_nodes = required_u64(arguments, "max_nodes", &self.tool_name)?;
        if !(1..=MAX_DOM_NODES).contains(&max_nodes) {
            return Err(support::invalid_arguments(
                &self.tool_name,
                "max_nodes must be between 1 and 1000",
            ));
        }

        let expression = dom_read_expression(selector, max_nodes)?;
        let snapshot = self
            .context
            .runtime
            .evaluate_dom(&self.context.target, &expression)
            .map_err(|error| support::map_platform_error(&self.tool_name, &error))?;
        let payload: DomReadPayload =
            serde_json::from_str(snapshot.dom()).map_err(|error| ToolBusError::Mcp {
                code: -32_000,
                message: format!(
                    "{}: CDP DOM payload is not the required JSON object: {error}",
                    self.tool_name
                ),
            })?;
        if payload.node_count > max_nodes {
            return Err(ToolBusError::Mcp {
                code: -32_000,
                message: format!(
                    "{}: CDP DOM payload returned node_count={} above max_nodes={max_nodes}",
                    self.tool_name, payload.node_count
                ),
            });
        }
        let fingerprint = dom_fingerprint(&snapshot)?;
        Ok(ToolOutput::untrusted(
            SourceDescriptor::app_content(self.context.app_id.clone()).with_target("browser_page"),
            json!({
                "dom": payload.dom,
                "node_count": payload.node_count,
                "fingerprint_scope": CDP_TARGET_FINGERPRINT_SCOPE,
                "fingerprint": fingerprint,
                "previous_fingerprint": fingerprint,
                "elapsed_ms": 0,
            }),
        ))
    }
}

struct WaitNetworkIdleHandler<R> {
    context: Arc<BrowserHandlerContext<R>>,
    tool_name: String,
}

impl<R> ToolHandler for WaitNetworkIdleHandler<R>
where
    R: BrowserCdpRuntime + 'static,
{
    fn call(
        &self,
        _call: &CallContext,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        let quiet_ms = required_u64(arguments, "quiet_ms", &self.tool_name)?;
        if !(1..=MAX_QUIET_MS).contains(&quiet_ms) {
            return Err(support::invalid_arguments(
                &self.tool_name,
                "quiet_ms must be between 1 and 10000",
            ));
        }
        self.context
            .runtime
            .wait_network_idle(&self.context.target, quiet_ms)
            .map_err(|error| support::map_platform_error(&self.tool_name, &error))?;
        Ok(ToolOutput::json(json!({
            "idle": true,
            "quiet_ms": quiet_ms,
        })))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DomReadPayload {
    dom: String,
    node_count: u64,
}

fn dom_read_expression(selector: &str, max_nodes: u64) -> Result<String, ToolBusError> {
    let selector_literal = serde_json::to_string(selector).map_err(|error| ToolBusError::Mcp {
        code: -32_000,
        message: format!("browser CDP selector serialization failed: {error}"),
    })?;
    Ok(format!(
        "(() => {{ const root = document.querySelector({selector_literal}); \
         if (!root) return JSON.stringify({{ dom: '', node_count: 0 }}); \
         const clone = root.cloneNode(true); \
         const nodes = [clone, ...clone.querySelectorAll('*')]; \
         for (const node of nodes.slice({max_nodes})) node.remove(); \
         return JSON.stringify({{ dom: String(clone.outerHTML), node_count: Math.min(nodes.length, {max_nodes}) }}); }})()"
    ))
}

fn validate_http_url(url: &str, tool: &str) -> Result<(), ToolBusError> {
    if url.len() > MAX_URL_BYTES {
        return Err(support::invalid_arguments(tool, "url exceeds 8 KiB"));
    }
    let is_http = url.starts_with("http://") || url.starts_with("https://");
    if !is_http
        || url
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(support::invalid_arguments(
            tool,
            "url must be an absolute HTTP(S) URL without whitespace or control characters",
        ));
    }
    Ok(())
}

fn identity_fingerprint(snapshot: &CdpDomSnapshot) -> Result<String, ToolBusError> {
    fingerprint_value(&json!({
        "url": snapshot.target_url(),
        "title": snapshot.target_title(),
        "marker": snapshot.target_marker(),
    }))
}

fn dom_fingerprint(snapshot: &CdpDomSnapshot) -> Result<String, ToolBusError> {
    fingerprint_value(&json!({
        "url": snapshot.target_url(),
        "title": snapshot.target_title(),
        "marker": snapshot.target_marker(),
        "dom": snapshot.dom(),
    }))
}

fn fingerprint_value(value: &Value) -> Result<String, ToolBusError> {
    let encoded = serde_json::to_vec(value).map_err(|error| ToolBusError::Mcp {
        code: -32_000,
        message: format!("browser fingerprint serialization failed: {error}"),
    })?;
    Ok(format!("sha256:{}", BlobId::of_content(&encoded).as_str()))
}

fn elapsed_millis(elapsed: std::time::Duration) -> u64 {
    u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
}

fn required_string<'a>(
    arguments: &'a Map<String, Value>,
    field: &str,
    tool: &str,
) -> Result<&'a str, ToolBusError> {
    arguments
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            support::invalid_arguments(tool, format!("{field} must be a non-empty string"))
        })
}

fn required_u64(
    arguments: &Map<String, Value>,
    field: &str,
    tool: &str,
) -> Result<u64, ToolBusError> {
    arguments.get(field).and_then(Value::as_u64).ok_or_else(|| {
        support::invalid_arguments(tool, format!("{field} must be a non-negative integer"))
    })
}

fn poll_ready<F>(future: F, operation: &str) -> PlatformResult<F::Output>
where
    F: Future,
{
    let mut future = std::pin::pin!(future);
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    match future.as_mut().poll(&mut context) {
        Poll::Ready(output) => Ok(output),
        Poll::Pending => Err(PlatformError::new(
            ErrorCode::TargetUnresponsive,
            format!("{operation} future unexpectedly returned Pending"),
        )),
    }
}
