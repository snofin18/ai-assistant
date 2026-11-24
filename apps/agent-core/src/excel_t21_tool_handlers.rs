//! Test-only `ToolBus` adapters for the Excel T2.1 pure handlers.
//!
//! Responsibilities:
//! - wrap the four TASK-395 handlers behind the existing `ToolHandler` trait;
//! - map structured Excel failures to the typed `HandlerFailure` bridge;
//! - attach `app_content` provenance and the workbook/range payload budgets.
//!
//! Boundaries:
//! - this module is compiled only inside the test-only `excel_t21` module;
//! - it is not referenced by production assembly or the Excel Adapter declaration;
//! - it does not start Excel, touch COM, read `.xlsx`, or decide policy.
//!
//! Invariants:
//! 1. every Excel `ErrorCode` is preserved verbatim in `HandlerFailure.code`;
//! 2. application-derived payloads are always returned as untrusted;
//! 3. payload limits are checked before the `ToolBus` envelope can truncate data.
//!
//! Related documents: TASK-398, ADR-0108, ADR-0109, and
//! `docs/spec/excel-t21-readonly-workflow.md`.

use std::sync::Arc;

use assistant_protocol::ErrorCode;
use assistant_tool_bus::{
    CallContext, SourceDescriptor, ToolBusConfig, ToolBusError, ToolHandler, ToolOutput,
};
use serde_json::{Map, Value};

use super::{
    ExcelCallContext, ExcelT21Error, ExcelT21Handlers, ExcelT21Runtime, ExcelToolResult,
    MAX_RANGE_PAYLOAD_BYTES, MAX_WORKBOOK_PAYLOAD_BYTES, TOOL_RANGE_READ, TOOL_RANGE_SUMMARIZE,
    TOOL_WORKBOOK_OPEN, TOOL_WORKBOOK_READ,
};

const EXCEL_APP_ID: &str = "com.microsoft.excel";
const EXCEL_SOURCE_TARGET: &str = "excel_workbook";

/// Test-only wrapper set bound to one `ToolBus` session id.
pub(crate) struct ExcelT21ToolHandlerSet<R> {
    handlers: Arc<ExcelT21Handlers<R>>,
    tool_bus_session_id: String,
}

impl<R> ExcelT21ToolHandlerSet<R>
where
    R: ExcelT21Runtime + 'static,
{
    /// Creates wrappers that use the same registry and `ToolBus` session identity.
    pub(crate) fn new(handlers: Arc<ExcelT21Handlers<R>>) -> Result<Self, ExcelT21Error> {
        let tool_bus_session_id = handlers
            .sessions
            .lock()
            .map_err(|_| {
                ExcelT21Error::new(
                    ErrorCode::Fatal,
                    "Excel session registry is poisoned while deriving its ToolBus identity",
                )
            })?
            .tool_bus_session_id
            .clone();
        Ok(Self {
            handlers,
            tool_bus_session_id,
        })
    }

    /// Returns the only `ToolBus` config that may be paired with this registry.
    pub(crate) fn tool_bus_config(&self) -> ToolBusConfig {
        ToolBusConfig::new(self.tool_bus_session_id.clone())
    }

    /// Returns exactly the four private handlers and their canonical tool names.
    pub(crate) fn tool_handlers(self: &Arc<Self>) -> [(&'static str, Arc<dyn ToolHandler>); 4] {
        [
            (
                TOOL_WORKBOOK_OPEN,
                private_handler(self, ExcelToolAction::Open),
            ),
            (
                TOOL_WORKBOOK_READ,
                private_handler(self, ExcelToolAction::WorkbookRead),
            ),
            (
                TOOL_RANGE_READ,
                private_handler(self, ExcelToolAction::RangeRead),
            ),
            (
                TOOL_RANGE_SUMMARIZE,
                private_handler(self, ExcelToolAction::RangeSummarize),
            ),
        ]
    }
}

fn private_handler<R>(
    handlers: &Arc<ExcelT21ToolHandlerSet<R>>,
    action: ExcelToolAction,
) -> Arc<dyn ToolHandler>
where
    R: ExcelT21Runtime + 'static,
{
    Arc::new(ExcelT21ToolHandler {
        handlers: Arc::clone(handlers),
        action,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExcelToolAction {
    Open,
    WorkbookRead,
    RangeRead,
    RangeSummarize,
}

impl ExcelToolAction {
    const fn maximum_payload_bytes(self) -> usize {
        match self {
            Self::Open | Self::WorkbookRead => MAX_WORKBOOK_PAYLOAD_BYTES,
            Self::RangeRead | Self::RangeSummarize => MAX_RANGE_PAYLOAD_BYTES,
        }
    }

    const fn payload_error_code(self) -> ErrorCode {
        match self {
            Self::Open | Self::WorkbookRead => ErrorCode::UserInteraction,
            Self::RangeRead | Self::RangeSummarize => ErrorCode::ToolInvalidArgs,
        }
    }
}

struct ExcelT21ToolHandler<R> {
    handlers: Arc<ExcelT21ToolHandlerSet<R>>,
    action: ExcelToolAction,
}

impl<R> ToolHandler for ExcelT21ToolHandler<R>
where
    R: ExcelT21Runtime + 'static,
{
    fn call(
        &self,
        call: &CallContext,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        let context =
            ExcelCallContext::new(self.handlers.tool_bus_session_id.clone(), call.task_id())
                .map_err(|error| map_excel_failure(&error))?;
        let result = match self.action {
            ExcelToolAction::Open => self.handlers.handlers.open(&context, arguments),
            ExcelToolAction::WorkbookRead => {
                self.handlers.handlers.workbook_read(&context, arguments)
            }
            ExcelToolAction::RangeRead => self.handlers.handlers.range_read(&context, arguments),
            ExcelToolAction::RangeSummarize => {
                self.handlers.handlers.summarize(&context, arguments)
            }
        }
        .map_err(|error| map_excel_failure(&error))?;
        checked_tool_output(self.action, result)
    }
}

fn map_excel_failure(error: &ExcelT21Error) -> ToolBusError {
    ToolBusError::HandlerFailure {
        code: error.code(),
        message: error.to_string(),
    }
}

fn checked_tool_output(
    action: ExcelToolAction,
    result: ExcelToolResult,
) -> Result<ToolOutput, ToolBusError> {
    let maximum = action.maximum_payload_bytes();
    let actual_bytes = serde_json::to_vec(&result.data).map_err(|error| {
        map_excel_failure(&ExcelT21Error::new(
            ErrorCode::Fatal,
            format!("Excel payload serialization failed: {error}"),
        ))
    })?;
    if actual_bytes.len() > maximum {
        return Err(map_excel_failure(&ExcelT21Error::new(
            action.payload_error_code(),
            format!(
                "Excel payload is {} bytes; maximum is {maximum}",
                actual_bytes.len()
            ),
        )));
    }
    let output = if result.untrusted {
        ToolOutput::untrusted(
            SourceDescriptor::app_content(EXCEL_APP_ID).with_target(EXCEL_SOURCE_TARGET),
            result.data,
        )
    } else {
        ToolOutput::json(result.data)
    };
    Ok(output.with_max_bytes(maximum))
}

#[cfg(test)]
mod tests {
    use super::*;
    use assistant_protocol::{ErrorCode, ToolSchema};
    use assistant_tool_bus::{MountSelection, SystemClock, ToolBus, ToolDefinition, ToolRegistry};
    use serde_json::json;

    const TEST_WORKBOOK_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/book.xlsx");
    type TestError = Box<dyn std::error::Error + Send + Sync>;

    fn definition(value: Value) -> Result<ToolDefinition, TestError> {
        let schema: ToolSchema = serde_json::from_value(value)?;
        Ok(ToolDefinition::from_schema(schema)?)
    }

    fn open_definition() -> Result<ToolDefinition, TestError> {
        definition(json!({
            "version": "1.1",
            "name": TOOL_WORKBOOK_OPEN,
            "description": "Test-only Excel workbook open wrapper.",
            "input": {
                "type": "object",
                "required": ["workbook_path", "read_only"],
                "additionalProperties": false,
                "properties": {
                    "workbook_path": {"type": "string"},
                    "read_only": {"const": true}
                }
            },
            "output": {},
            "risk_level": "high",
            "effect": "write",
            "reversibility": "l3_irreversible",
            "requires_approval": true,
            "idempotent": false
        }))
    }

    fn workbook_read_definition() -> Result<ToolDefinition, TestError> {
        definition(json!({
            "version": "1.1",
            "name": TOOL_WORKBOOK_READ,
            "description": "Test-only Excel workbook metadata read wrapper.",
            "input": {
                "type": "object",
                "required": ["session_id"],
                "additionalProperties": false,
                "properties": {
                    "session_id": {"type": "string", "minLength": 32, "maxLength": 128}
                }
            },
            "output": {},
            "risk_level": "low",
            "effect": "read",
            "reversibility": "none_readonly",
            "requires_approval": false,
            "idempotent": true
        }))
    }

    fn range_definition(name: &str) -> Result<ToolDefinition, TestError> {
        definition(json!({
            "version": "1.1",
            "name": name,
            "description": "Test-only Excel range read wrapper.",
            "input": {
                "type": "object",
                "required": ["session_id", "range", "mode"],
                "additionalProperties": false,
                "properties": {
                    "session_id": {"type": "string", "minLength": 32, "maxLength": 128},
                    "range": {"type": "object"},
                    "mode": {"type": "string"}
                }
            },
            "output": {},
            "risk_level": "low",
            "effect": "read",
            "reversibility": "none_readonly",
            "requires_approval": false,
            "idempotent": true
        }))
    }

    fn error(message: &str) -> TestError {
        Box::new(std::io::Error::other(message))
    }

    #[test]
    fn test_excel_error_codes_map_to_handler_failure_without_loss() -> Result<(), TestError> {
        for expected in ErrorCode::ALL {
            let mapped = map_excel_failure(&ExcelT21Error::new(expected, "typed Excel failure"));
            match mapped {
                ToolBusError::HandlerFailure { code, message } => {
                    assert_eq!(code, expected);
                    assert_eq!(message, "typed Excel failure");
                }
                other => return Err(error(&format!("expected HandlerFailure, got {other:?}"))),
            }
        }
        Ok(())
    }

    #[test]
    fn test_checked_tool_output_rejects_payload_above_range_bound() -> Result<(), TestError> {
        let oversized = "x".repeat(MAX_RANGE_PAYLOAD_BYTES + 1);
        let failure = checked_tool_output(
            ExcelToolAction::RangeRead,
            ExcelToolResult {
                data: json!({"value": oversized}),
                untrusted: true,
            },
        )
        .err()
        .ok_or_else(|| error("oversized Excel payload unexpectedly passed"))?;
        match failure {
            ToolBusError::HandlerFailure { code, message } => {
                assert_eq!(code, ErrorCode::ToolInvalidArgs);
                assert!(message.contains("maximum"));
            }
            other => return Err(error(&format!("expected HandlerFailure, got {other:?}"))),
        }
        Ok(())
    }

    #[test]
    fn test_pure_range_handler_maps_oversized_payload_to_tool_invalid_args() -> Result<(), TestError>
    {
        let runtime = Arc::new(super::super::FakeRuntime::default());
        let (sessions, _clock, _tokens) = super::super::test_registry("bus-a")?;
        let handlers = ExcelT21Handlers::new(Arc::clone(&runtime), sessions);
        let call = super::super::test_context("bus-a", "task-a")?;
        let opened = handlers.open(
            &call,
            &super::super::arguments(json!({
                "workbook_path": TEST_WORKBOOK_PATH,
                "read_only": true
            }))?,
        )?;
        let token = opened
            .data
            .get("session_id")
            .and_then(Value::as_str)
            .ok_or_else(|| error("open result has no session_id"))?;
        let oversized = "x".repeat(MAX_RANGE_PAYLOAD_BYTES + 1);
        runtime.set_range_response(super::super::RangeSnapshot {
            address: "A1".to_owned(),
            rows: 1,
            columns: 1,
            cells: vec![super::super::handlers::runtime_cell(
                1,
                1,
                super::super::CellValueType::Text,
                &oversized,
            )],
        })?;
        let failure = handlers
            .range_read(
                &call,
                &super::super::arguments(json!({
                    "session_id": token,
                    "range": {"kind":"named_range","name":"SalesData"},
                    "mode": "value2"
                }))?,
            )
            .err()
            .ok_or_else(|| error("oversized pure handler payload unexpectedly passed"))?;
        assert_eq!(failure.code(), ErrorCode::ToolInvalidArgs);
        Ok(())
    }

    fn wrapped_registry(
        handler_list: &[(&'static str, Arc<dyn ToolHandler>)],
    ) -> Result<ToolRegistry, TestError> {
        let [
            open_handler,
            read_handler,
            range_read_handler,
            summarize_handler,
        ] = handler_list
        else {
            return Err(error("Excel wrappers must contain exactly four handlers"));
        };
        let mut registry = ToolRegistry::new();
        registry.register(open_definition()?, Arc::clone(&open_handler.1))?;
        registry.register(workbook_read_definition()?, Arc::clone(&read_handler.1))?;
        registry.register(
            range_definition(TOOL_RANGE_READ)?,
            Arc::clone(&range_read_handler.1),
        )?;
        registry.register(
            range_definition(TOOL_RANGE_SUMMARIZE)?,
            Arc::clone(&summarize_handler.1),
        )?;
        Ok(registry)
    }

    async fn start_wrapped_bus(
        wrappers: &Arc<ExcelT21ToolHandlerSet<super::super::FakeRuntime>>,
        handler_list: &[(&'static str, Arc<dyn ToolHandler>)],
    ) -> Result<ToolBus, TestError> {
        let names = handler_list
            .iter()
            .map(|(name, _)| (*name).to_owned())
            .collect::<Vec<_>>();
        let (bus, _report) = ToolBus::start(
            wrapped_registry(handler_list)?,
            MountSelection::names(names),
            wrappers.tool_bus_config(),
            Arc::new(SystemClock),
        )
        .await?;
        Ok(bus)
    }

    async fn start_bus_and_open()
    -> Result<(ToolBus, Arc<super::super::FakeRuntime>, String), TestError> {
        let runtime = Arc::new(super::super::FakeRuntime::default());
        let (sessions, _clock, _tokens) = super::super::test_registry("bus-a")?;
        let handlers = Arc::new(ExcelT21Handlers::new(Arc::clone(&runtime), sessions));
        let wrappers = Arc::new(ExcelT21ToolHandlerSet::new(handlers)?);
        assert_eq!(wrappers.tool_bus_config().session_id(), "bus-a");
        let handler_list = wrappers.tool_handlers();
        let bus = start_wrapped_bus(&wrappers, &handler_list).await?;
        let call = CallContext::new("task-a", "step-a");
        let opened = bus
            .call_tool(
                TOOL_WORKBOOK_OPEN,
                super::super::arguments(json!({
                    "workbook_path": TEST_WORKBOOK_PATH,
                    "read_only": true
                }))?,
                &call,
            )
            .await?;
        assert!(opened.ok);
        assert!(opened.untrusted);
        let source = opened
            .source
            .as_ref()
            .ok_or_else(|| error("open envelope has no source"))?;
        assert_eq!(source.kind, assistant_protocol::SourceKind::AppContent);
        assert_eq!(source.app_id.as_deref(), Some(EXCEL_APP_ID));
        assert_eq!(source.target.as_deref(), Some(EXCEL_SOURCE_TARGET));
        let opened_data = opened
            .data
            .as_ref()
            .map(|value| &value.0)
            .ok_or_else(|| error("open envelope has no data"))?;
        let token = opened_data
            .get("session_id")
            .and_then(Value::as_str)
            .ok_or_else(|| error("open envelope has no session_id"))?
            .to_owned();
        Ok((bus, runtime, token))
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_all_wrapped_handlers_round_trip_success() -> Result<(), TestError> {
        let (bus, _runtime, token) = start_bus_and_open().await?;
        let call = CallContext::new("task-a", "step-a");
        let metadata = bus
            .call_tool(
                TOOL_WORKBOOK_READ,
                super::super::arguments(json!({"session_id": token.clone()}))?,
                &call,
            )
            .await?;
        assert!(metadata.ok);
        assert!(metadata.untrusted);

        let range = bus
            .call_tool(
                TOOL_RANGE_READ,
                super::super::arguments(json!({
                    "session_id": token.clone(),
                    "range": {"kind":"a1","worksheet":{"kind":"ordinal","value":1},"range":"C3:D4"},
                    "mode": "value2"
                }))?,
                &call,
            )
            .await?;
        assert!(range.ok);
        assert!(range.untrusted);

        let summary = bus
            .call_tool(
                TOOL_RANGE_SUMMARIZE,
                super::super::arguments(json!({
                    "session_id": token.clone(),
                    "range": {"kind":"named_range","name":"SalesData"},
                    "mode": "value2"
                }))?,
                &call,
            )
            .await?;
        assert!(summary.ok);
        assert!(summary.untrusted);
        bus.shutdown().await?;
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_wrapped_handler_round_trip_preserves_verify_failed() -> Result<(), TestError> {
        let (bus, runtime, token) = start_bus_and_open().await?;
        let call = CallContext::new("task-a", "step-a");
        let cross_task = bus
            .call_tool(
                TOOL_WORKBOOK_READ,
                super::super::arguments(json!({"session_id": token.clone()}))?,
                &CallContext::new("task-b", "step-b"),
            )
            .await?;
        assert!(!cross_task.ok);
        assert_eq!(
            cross_task
                .error
                .as_ref()
                .ok_or_else(|| error("cross-task failure has no error"))?
                .code,
            ErrorCode::ToolInvalidArgs
        );

        runtime.set_fingerprint_drift(true)?;
        let failed = bus
            .call_tool(
                TOOL_WORKBOOK_READ,
                super::super::arguments(json!({"session_id": token}))?,
                &call,
            )
            .await?;
        assert!(!failed.ok);
        let failure = failed
            .error
            .as_ref()
            .ok_or_else(|| error("failed envelope has no error"))?;
        assert_eq!(failure.code, ErrorCode::VerifyFailed);
        assert_eq!(
            failure.message,
            "excel.workbook.read: workbook fingerprint changed during a read"
        );

        bus.shutdown().await?;
        Ok(())
    }
}
