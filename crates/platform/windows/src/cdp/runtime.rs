//! CDP runtime provider backed by a bounded loopback transport.
//!
//! Responsibilities:
//! - discover one unique page target through `/json/list`;
//! - keep only opaque local target metadata for later RPC calls;
//! - implement navigation and DOM evaluation over RFC 6455;
//! - fail closed for operations not yet implemented.
//!
//! Boundaries:
//! - does not launch browsers or manage profiles;
//! - does not register production tool handlers;
//! - does not expose WebSocket objects or platform handles to callers;
//! - never connects outside loopback.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use assistant_platform_api::{
    CdpConnectOptions, CdpDomSnapshot, CdpLoadCondition, CdpPoint, CdpProvider, CdpScreenshot,
    CdpSessionSnapshot, CdpTargetIdentity, ErrorCode, PlatformError, PlatformResult,
    ResolvedCdpSession, ResolvedCdpTarget,
};

use super::http::{TargetPage, discover_pages};
use super::json::{self, JsonValue};
use super::websocket::CdpWebSocket;

const MIN_TIMEOUT: Duration = Duration::from_millis(100);
const MAX_TIMEOUT: Duration = Duration::from_secs(300);
const MAX_EXPRESSION_BYTES: usize = 64 * 1024;
const MAX_DOM_BYTES: usize = 1024 * 1024;
const MAX_REGISTERED_TARGETS: usize = 16;

/// CDP provider backed by one loopback browser endpoint.
#[derive(Debug)]
pub struct RuntimeCdpProvider {
    targets: Mutex<BTreeMap<u64, RuntimeTarget>>,
    next_session_id: AtomicU64,
    next_target_id: AtomicU64,
}

impl Default for RuntimeCdpProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeCdpProvider {
    /// 创建一个空 provider。
    #[must_use]
    pub const fn new() -> Self {
        Self {
            targets: Mutex::new(BTreeMap::new()),
            next_session_id: AtomicU64::new(1),
            next_target_id: AtomicU64::new(1),
        }
    }

    /// Wait for a network-idle quiet window on an already connected page target.
    ///
    /// This is the primitive used by the later `wait_network_idle` production
    /// handler. `quiet_ms` is capped at 10 seconds so one tool call cannot
    /// monopolize the provider indefinitely.
    ///
    /// # Errors
    ///
    /// Returns `ToolInvalidArgs` for a zero or excessive quiet window,
    /// `TargetNotFound` when the target is unknown, and `TargetUnresponsive`
    /// when no bounded network-idle window can be observed.
    pub fn wait_network_idle(
        &self,
        target: &ResolvedCdpTarget,
        quiet_ms: u64,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        let target = *target;
        async move {
            let record = self.target_record(&target)?;
            wait_network_idle_blocking(&record, quiet_ms)
        }
    }

    fn target_record(&self, target: &ResolvedCdpTarget) -> PlatformResult<RuntimeTarget> {
        let record = {
            let targets = self.targets.lock().map_err(|_| {
                PlatformError::new(ErrorCode::Fatal, "CDP target registry mutex is poisoned")
            })?;
            targets.get(&target.target_id()).cloned().ok_or_else(|| {
                PlatformError::new(
                    ErrorCode::TargetNotFound,
                    format!("CDP target {} is not registered", target.target_id()),
                )
            })?
        };
        if record.session != target.session() {
            return Err(PlatformError::new(
                ErrorCode::TargetNotFound,
                "CDP target belongs to a different session",
            ));
        }
        Ok(record)
    }

    fn register_target(&self, record: RuntimeTarget) -> PlatformResult<()> {
        {
            let mut targets = self.targets.lock().map_err(|_| {
                PlatformError::new(ErrorCode::Fatal, "CDP target registry mutex is poisoned")
            })?;
            if targets.len() >= MAX_REGISTERED_TARGETS {
                return Err(PlatformError::new(
                    ErrorCode::CapabilityMissing,
                    format!(
                        "CDP target registry reached its {MAX_REGISTERED_TARGETS}-target limit; \
                         close support is not implemented yet"
                    ),
                ));
            }
            targets.insert(record.target.target_id(), record);
        }
        Ok(())
    }

    fn select_target<'a>(
        options: &CdpConnectOptions,
        pages: &'a [TargetPage],
    ) -> PlatformResult<&'a TargetPage> {
        let matches = pages
            .iter()
            .filter(|page| {
                page.url == options.page_url()
                    && options.page_title().is_none_or(|title| page.title == title)
            })
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => Err(PlatformError::new(
                ErrorCode::TargetNotFound,
                format!("no CDP page target matched URL `{}`", options.page_url()),
            )),
            [page] => Ok(*page),
            _ => Err(PlatformError::new(
                ErrorCode::TargetAmbiguous,
                format!(
                    "multiple CDP page targets matched URL `{}`",
                    options.page_url()
                ),
            )),
        }
    }
}

impl CdpProvider for RuntimeCdpProvider {
    fn connect(
        &self,
        options: &CdpConnectOptions,
    ) -> impl Future<Output = PlatformResult<CdpSessionSnapshot>> + Send {
        let result = (|| {
            let timeout = bounded_timeout(options.session_timeout_ms())?;
            let pages = discover_pages(options.host(), options.port(), timeout)?;
            let page = Self::select_target(options, &pages)?;
            let session =
                ResolvedCdpSession::new(self.next_session_id.fetch_add(1, Ordering::Relaxed));
            let target = ResolvedCdpTarget::new(
                session,
                self.next_target_id.fetch_add(1, Ordering::Relaxed),
            );
            let identity = CdpTargetIdentity::new(&page.url, &page.title);
            let identity = if let Some(marker) = options.page_marker() {
                identity.with_marker(marker)
            } else {
                identity
            };
            self.register_target(RuntimeTarget {
                session,
                target,
                page_marker: options.page_marker().map(ToString::to_string),
                websocket_url: page.websocket_url.clone(),
                port: options.port(),
                timeout,
            })?;
            Ok(CdpSessionSnapshot::new(session, target, identity))
        })();
        std::future::ready(result)
    }

    fn navigate(
        &self,
        target: &ResolvedCdpTarget,
        url: &str,
        condition: &CdpLoadCondition,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        let target = *target;
        let url = url.to_string();
        let condition = condition.clone();
        async move {
            let record = self.target_record(&target)?;
            navigate_blocking(&record, &url, &condition)
        }
    }

    fn evaluate_dom(
        &self,
        target: &ResolvedCdpTarget,
        expression: &str,
    ) -> impl Future<Output = PlatformResult<CdpDomSnapshot>> + Send {
        let target = *target;
        let expression = expression.to_string();
        async move {
            let record = self.target_record(&target)?;
            evaluate_dom_blocking(&record, &expression)
        }
    }

    fn click(
        &self,
        _target: &ResolvedCdpTarget,
        _point: &CdpPoint,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        std::future::ready(Err(capability_missing("click")))
    }

    fn capture_screenshot(
        &self,
        _target: &ResolvedCdpTarget,
    ) -> impl Future<Output = PlatformResult<CdpScreenshot>> + Send {
        std::future::ready(Err(capability_missing("capture_screenshot")))
    }

    fn set_download_directory(
        &self,
        _target: &ResolvedCdpTarget,
        _directory: &str,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        std::future::ready(Err(capability_missing("set_download_directory")))
    }

    fn close(
        &self,
        _target: &ResolvedCdpTarget,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        std::future::ready(Err(capability_missing("close")))
    }
}

#[derive(Debug, Clone)]
struct RuntimeTarget {
    session: ResolvedCdpSession,
    target: ResolvedCdpTarget,
    page_marker: Option<String>,
    websocket_url: String,
    port: u16,
    timeout: Duration,
}

fn navigate_blocking(
    record: &RuntimeTarget,
    url: &str,
    condition: &CdpLoadCondition,
) -> PlatformResult<()> {
    validate_navigation_url(url)?;
    let quiet_ms = match condition {
        CdpLoadCondition::Load => None,
        CdpLoadCondition::NetworkIdle { quiet_ms } => {
            if *quiet_ms == 0 || *quiet_ms > 10_000 {
                return Err(PlatformError::new(
                    ErrorCode::ToolInvalidArgs,
                    "network-idle quiet_ms must be in 1..=10000",
                ));
            }
            Some(*quiet_ms)
        }
        _ => {
            return Err(PlatformError::new(
                ErrorCode::CapabilityMissing,
                "unsupported CDP load condition",
            ));
        }
    };
    let deadline = deadline(record.timeout)?;
    let mut socket = CdpWebSocket::connect(&record.websocket_url, record.port, record.timeout)?;
    send_rpc(
        &mut socket,
        1,
        "Page.enable",
        &json::empty_object(),
        deadline,
    )?;
    if quiet_ms.is_some() {
        send_rpc(
            &mut socket,
            2,
            "Network.enable",
            &json::empty_object(),
            deadline,
        )?;
    }
    socket.send_json(&json::object([
        ("id", json::unsigned(3)),
        ("method", json::string("Page.navigate")),
        ("params", json::object([("url", json::string(url))])),
    ]))?;

    let mut progress = NavigationProgress::new();
    loop {
        let wait_deadline = quiet_ms.map_or(deadline, |quiet| {
            deadline.min(progress.quiet_deadline(quiet))
        });
        let Some(message) = socket.receive_json_until(wait_deadline)? else {
            if progress.is_complete(quiet_ms) {
                socket.close()?;
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(PlatformError::new(
                    ErrorCode::TargetUnresponsive,
                    format!(
                        "CDP navigation did not reach `{}` before timeout",
                        condition.expected_state()
                    ),
                ));
            }
            continue;
        };
        progress.handle(&message)?;
        if progress.is_complete(quiet_ms) {
            socket.close()?;
            return Ok(());
        }
    }
}

#[derive(Debug)]
struct NavigationProgress {
    acknowledged: bool,
    loaded: bool,
    in_flight: BTreeSet<String>,
    last_activity: Instant,
}

impl NavigationProgress {
    fn new() -> Self {
        Self {
            acknowledged: false,
            loaded: false,
            in_flight: BTreeSet::new(),
            last_activity: Instant::now(),
        }
    }

    fn handle(&mut self, message: &JsonValue) -> PlatformResult<()> {
        if message_id(message) == Some(3) {
            ensure_rpc_success(message)?;
            self.acknowledged = true;
            if let Some(error_text) = message
                .get("result")
                .and_then(|result| result.get("errorText"))
                .and_then(JsonValue::as_str)
                .filter(|text| !text.is_empty())
            {
                return Err(PlatformError::new(
                    ErrorCode::Transient,
                    format!("CDP navigation failed: {error_text}"),
                ));
            }
        }
        if let Some(method) = message_method(message) {
            match method {
                "Page.loadEventFired" => {
                    self.loaded = true;
                    self.last_activity = Instant::now();
                }
                "Network.requestWillBeSent" => {
                    if let Some(request_id) = event_request_id(message) {
                        self.in_flight.insert(request_id);
                    }
                    self.last_activity = Instant::now();
                }
                "Network.loadingFinished" | "Network.loadingFailed" => {
                    if let Some(request_id) = event_request_id(message) {
                        self.in_flight.remove(&request_id);
                    }
                    self.last_activity = Instant::now();
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn quiet_deadline(&self, quiet_ms: u64) -> Instant {
        self.last_activity + Duration::from_millis(quiet_ms)
    }

    fn is_complete(&self, quiet_ms: Option<u64>) -> bool {
        if !self.acknowledged || !self.loaded {
            return false;
        }
        quiet_ms.is_none_or(|quiet| {
            self.in_flight.is_empty()
                && self.last_activity.elapsed() >= Duration::from_millis(quiet)
        })
    }
}

fn wait_network_idle_blocking(record: &RuntimeTarget, quiet_ms: u64) -> PlatformResult<()> {
    if quiet_ms == 0 || quiet_ms > 10_000 {
        return Err(PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            "network-idle quiet_ms must be in 1..=10000",
        ));
    }
    let deadline = deadline(record.timeout)?;
    let mut socket = CdpWebSocket::connect(&record.websocket_url, record.port, record.timeout)?;
    send_rpc(
        &mut socket,
        1,
        "Network.enable",
        &json::empty_object(),
        deadline,
    )?;
    let mut in_flight = BTreeSet::new();
    let mut last_activity = Instant::now();
    loop {
        let quiet_deadline = last_activity + Duration::from_millis(quiet_ms);
        let Some(message) = socket.receive_json_until(deadline.min(quiet_deadline))? else {
            if in_flight.is_empty() && last_activity.elapsed() >= Duration::from_millis(quiet_ms) {
                socket.close()?;
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(PlatformError::new(
                    ErrorCode::TargetUnresponsive,
                    "CDP network-idle wait exceeded its timeout",
                ));
            }
            continue;
        };
        if let Some(method) = message_method(&message) {
            match method {
                "Network.requestWillBeSent" => {
                    if let Some(request_id) = event_request_id(&message) {
                        in_flight.insert(request_id);
                    }
                    last_activity = Instant::now();
                }
                "Network.loadingFinished" | "Network.loadingFailed" => {
                    if let Some(request_id) = event_request_id(&message) {
                        in_flight.remove(&request_id);
                    }
                    last_activity = Instant::now();
                }
                _ => {}
            }
        }
    }
}

fn evaluate_dom_blocking(
    record: &RuntimeTarget,
    expression: &str,
) -> PlatformResult<CdpDomSnapshot> {
    if expression.trim().is_empty() || expression.len() > MAX_EXPRESSION_BYTES {
        return Err(PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            "CDP DOM expression is empty or exceeds 64 KiB",
        ));
    }
    // The expression is trusted internal JavaScript assembled after handler-side
    // selector and length validation. Quoting it here would evaluate the source
    // text as a string instead of executing the DOM read.
    let wrapped = format!(
        "(() => {{ const value = ({expression}); return JSON.stringify({{ \
         url: location.href, title: document.title, dom: String(value) }}); }})()"
    );
    let deadline = deadline(record.timeout)?;
    let mut socket = CdpWebSocket::connect(&record.websocket_url, record.port, record.timeout)?;
    let response = send_rpc(
        &mut socket,
        1,
        "Runtime.evaluate",
        &json::object([
            ("expression", json::string(wrapped)),
            ("returnByValue", json::boolean(true)),
            ("awaitPromise", json::boolean(false)),
        ]),
        deadline,
    )?;
    if response.get("exceptionDetails").is_some() {
        return Err(PlatformError::new(
            ErrorCode::VerifyFailed,
            format!(
                "CDP Runtime.evaluate raised an exception: {}",
                response
                    .get("exceptionDetails")
                    .and_then(|details| details.get("text"))
                    .and_then(JsonValue::as_str)
                    .unwrap_or("unknown exception")
            ),
        ));
    }
    let encoded_snapshot = response
        .get("result")
        .and_then(|result| result.get("value"))
        .and_then(JsonValue::as_str)
        .ok_or_else(|| {
            PlatformError::new(
                ErrorCode::VerifyFailed,
                "CDP Runtime.evaluate did not return a string expression result",
            )
        })?;
    let snapshot: JsonValue = json::parse(encoded_snapshot.as_bytes()).map_err(|error| {
        PlatformError::new(
            ErrorCode::VerifyFailed,
            format!("CDP DOM snapshot JSON is invalid: {error}"),
        )
    })?;
    let url = snapshot_string(&snapshot, "url")?;
    let title = snapshot_string(&snapshot, "title")?;
    let dom = snapshot_string(&snapshot, "dom")?;
    if dom.len() > MAX_DOM_BYTES {
        return Err(PlatformError::new(
            ErrorCode::VerifyFailed,
            "CDP DOM snapshot exceeds 1 MiB",
        ));
    }
    socket.close()?;
    Ok(CdpDomSnapshot::new(
        &url,
        &title,
        record.page_marker.as_deref(),
        dom,
    ))
}

fn send_rpc(
    socket: &mut CdpWebSocket,
    id: u64,
    method: &str,
    params: &JsonValue,
    deadline: Instant,
) -> PlatformResult<JsonValue> {
    socket.send_json(&json::object([
        ("id", json::unsigned(id)),
        ("method", json::string(method)),
        ("params", params.clone()),
    ]))?;
    loop {
        let message = socket.receive_json_until(deadline)?.ok_or_else(|| {
            PlatformError::new(
                ErrorCode::TargetUnresponsive,
                format!("CDP `{method}` timed out before a response"),
            )
        })?;
        if message_id(&message) == Some(id) {
            ensure_rpc_success(&message)?;
            return message.get("result").cloned().ok_or_else(|| {
                PlatformError::new(
                    ErrorCode::Transient,
                    format!("CDP `{method}` response is missing `result`"),
                )
            });
        }
    }
}

fn ensure_rpc_success(message: &JsonValue) -> PlatformResult<()> {
    let Some(error) = message.get("error") else {
        return Ok(());
    };
    let code = error.get("code").and_then(JsonValue::as_i64).unwrap_or(0);
    let text = error
        .get("message")
        .and_then(JsonValue::as_str)
        .unwrap_or("unknown CDP error");
    Err(PlatformError::new(
        ErrorCode::Transient,
        format!("CDP JSON-RPC error {code}: {text}"),
    ))
}

fn message_id(message: &JsonValue) -> Option<u64> {
    message.get("id").and_then(JsonValue::as_u64)
}

fn message_method(message: &JsonValue) -> Option<&str> {
    message.get("method").and_then(JsonValue::as_str)
}

fn event_request_id(message: &JsonValue) -> Option<String> {
    message
        .get("params")
        .and_then(|params| params.get("requestId"))
        .and_then(JsonValue::as_str)
        .map(ToString::to_string)
}

fn snapshot_string(value: &JsonValue, field: &str) -> PlatformResult<String> {
    value
        .get(field)
        .and_then(JsonValue::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| {
            PlatformError::new(
                ErrorCode::VerifyFailed,
                format!("CDP DOM snapshot is missing string field `{field}`"),
            )
        })
}

fn validate_navigation_url(url: &str) -> PlatformResult<()> {
    if url.starts_with("http://") || url.starts_with("https://") {
        return Ok(());
    }
    Err(PlatformError::new(
        ErrorCode::ToolInvalidArgs,
        "CDP navigation accepts only http:// or https:// URLs",
    ))
}

fn bounded_timeout(milliseconds: u64) -> PlatformResult<Duration> {
    if milliseconds == 0 {
        return Err(PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            "CDP session_timeout_ms must be greater than zero",
        ));
    }
    let requested = Duration::from_millis(milliseconds);
    Ok(requested.clamp(MIN_TIMEOUT, MAX_TIMEOUT))
}

fn deadline(timeout: Duration) -> PlatformResult<Instant> {
    Instant::now().checked_add(timeout).ok_or_else(|| {
        PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            "CDP timeout exceeds the process monotonic clock range",
        )
    })
}

fn capability_missing(operation: &str) -> PlatformError {
    PlatformError::new(
        ErrorCode::CapabilityMissing,
        format!(
            "real CDP {operation} is not implemented by TASK-109; the loopback transport \
             currently covers target discovery, navigation and DOM evaluation"
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target_record(index: u64) -> RuntimeTarget {
        let session = ResolvedCdpSession::new(index);
        RuntimeTarget {
            session,
            target: ResolvedCdpTarget::new(session, index),
            page_marker: None,
            websocket_url: format!("ws://127.0.0.1:1/devtools/page/{index}"),
            port: 1,
            timeout: Duration::from_millis(100),
        }
    }

    #[test]
    fn test_target_registry_rejects_growth_past_the_hard_limit() -> PlatformResult<()> {
        let provider = RuntimeCdpProvider::new();
        for index in 1..=MAX_REGISTERED_TARGETS {
            provider.register_target(target_record(index as u64))?;
        }
        let error = provider
            .register_target(target_record((MAX_REGISTERED_TARGETS + 1) as u64))
            .err()
            .ok_or_else(|| {
                PlatformError::new(
                    ErrorCode::Fatal,
                    "target registry accepted growth past its hard limit",
                )
            })?;
        assert_eq!(error.code(), ErrorCode::CapabilityMissing);
        Ok(())
    }
}
