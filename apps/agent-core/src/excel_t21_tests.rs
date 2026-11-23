//! Fake-runtime and negative tests for Excel T2.1 pure logic.

use std::sync::Arc;

use assistant_protocol::ErrorCode;
use serde_json::{Value, json};

use super::handlers::runtime_cell;
use super::*;

const TEST_WORKBOOK_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/book.xlsx");

fn test_text_cells_2x2() -> Vec<RuntimeCell> {
    vec![
        runtime_cell(1, 1, CellValueType::Text, "a"),
        runtime_cell(1, 2, CellValueType::Text, "b"),
        runtime_cell(2, 1, CellValueType::Text, "c"),
        runtime_cell(2, 2, CellValueType::Text, "d"),
    ]
}

fn open_fixture() -> Result<
    (
        Arc<FakeRuntime>,
        ExcelT21Handlers<FakeRuntime>,
        ExcelCallContext,
        String,
    ),
    ExcelT21Error,
> {
    let runtime = Arc::new(FakeRuntime::default());
    let (registry, _clock, _tokens) = test_registry("bus-a")?;
    let handlers = ExcelT21Handlers::new(Arc::clone(&runtime), registry);
    let call = test_context("bus-a", "task-a")?;
    let opened = handlers.open(
        &call,
        &arguments(json!({
            "workbook_path":TEST_WORKBOOK_PATH,
            "read_only":true
        }))?,
    )?;
    assert!(opened.untrusted);
    let token = token_from(&opened)?.to_owned();
    assert!((MIN_SESSION_TOKEN_BYTES..=MAX_SESSION_TOKEN_BYTES).contains(&token.len()));
    assert_eq!(
        opened.data.get("expires_at_utc").and_then(Value::as_str),
        Some("2023-11-14T22:28:20Z")
    );
    let worksheets = opened
        .data
        .get("worksheets")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_arguments("open output has no worksheets"))?;
    assert_eq!(worksheets.len(), 1);
    assert_eq!(
        worksheets
            .first()
            .and_then(|worksheet| worksheet.get("used_range_address")),
        None
    );
    Ok((runtime, handlers, call, token))
}

#[test]
fn test_canonical_decimal_normalizes_adds_and_compares_signed_values() -> Result<(), ExcelT21Error>
{
    assert_eq!(CanonicalDecimal::parse("001.2000")?.as_text()?, "1.2");
    assert_eq!(CanonicalDecimal::parse("1.2e3")?.as_text()?, "1200");
    assert_eq!(
        CanonicalDecimal::parse("-2.5")?
            .checked_add(&CanonicalDecimal::parse("4")?)?
            .as_text()?,
        "1.5"
    );
    assert_eq!(
        CanonicalDecimal::parse("-5")?
            .checked_add(&CanonicalDecimal::parse("2")?)?
            .as_text()?,
        "-3"
    );
    assert_eq!(
        CanonicalDecimal::parse("-5")?.cmp_value(&CanonicalDecimal::parse("2")?),
        std::cmp::Ordering::Less
    );
    Ok(())
}

#[test]
fn test_range_parser_accepts_closed_union_and_rejects_ambiguous_shapes() -> Result<(), ExcelT21Error>
{
    let named = parse_range_selector(&json!({"kind":"named_range","name":"SalesData"}))?;
    assert_eq!(named, RangeSelector::NamedRange("SalesData".to_owned()));
    let a1 = parse_range_selector(&json!({
        "kind":"a1",
        "worksheet":{"kind":"name","value":"Sheet1"},
        "range":"$A$1:C10"
    }))?;
    assert_eq!(a1.declared_cell_count(), Some(30));
    for invalid in [
        json!({"kind":"named_range","name":"SalesData","extra":true}),
        json!({"kind":"a1","worksheet":{"kind":"name","value":"Sheet1"},"range":"A0:B2"}),
        json!({"kind":"a1","worksheet":{"kind":"ordinal","value":0},"range":"A1:B2"}),
        json!({"kind":"a1","worksheet":{"kind":"name","value":"Sheet1"},"range":"B2:A1"}),
        json!({"kind":"a1","worksheet":{"kind":"name","value":"Sheet1"},"range":"XFE1"}),
    ] {
        assert_eq!(
            parse_range_selector(&invalid)
                .err()
                .ok_or_else(|| invalid_arguments("invalid range unexpectedly parsed"))?
                .code(),
            ErrorCode::ToolInvalidArgs
        );
    }
    Ok(())
}

#[test]
fn test_registry_rejects_cross_task_cross_bus_duplicate_and_global_limit()
-> Result<(), ExcelT21Error> {
    let (mut registry, _clock, _tokens) = test_registry("bus-a")?;
    let first = registry.register("task-a", opened(1))?;
    assert_eq!(
        registry
            .lookup("bus-a", "task-b", &first.token)
            .err()
            .ok_or_else(|| invalid_arguments("cross-task lookup unexpectedly passed"))?
            .code(),
        ErrorCode::ToolInvalidArgs
    );
    assert_eq!(
        registry
            .lookup("bus-b", "task-a", &first.token)
            .err()
            .ok_or_else(|| invalid_arguments("cross-bus lookup unexpectedly passed"))?
            .code(),
        ErrorCode::ToolInvalidArgs
    );
    assert_eq!(
        registry
            .register("task-a", opened(2))
            .err()
            .ok_or_else(|| invalid_arguments("duplicate task session unexpectedly passed"))?
            .code(),
        ErrorCode::ToolInvalidArgs
    );
    for task in 1..MAX_ACTIVE_SESSIONS {
        registry.register(&format!("task-{task}"), opened((task + 2) as u64))?;
    }
    assert_eq!(registry.active_session_count(), MAX_ACTIVE_SESSIONS);
    assert_eq!(
        registry
            .register("task-over-cap", opened(99))
            .err()
            .ok_or_else(|| invalid_arguments("global cap unexpectedly passed"))?
            .code(),
        ErrorCode::UserInteraction
    );
    assert_eq!(registry.active_session_count(), MAX_ACTIVE_SESSIONS);
    Ok(())
}

#[test]
fn test_registry_expires_and_cleanup_invalidates_immediately() -> Result<(), ExcelT21Error> {
    let (mut registry, clock, _tokens) = test_registry("bus-a")?;
    let first = registry.register("task-a", opened(1))?;
    clock.advance(SESSION_TTL_MS);
    let closing = registry.expire_sessions();
    assert_eq!(
        closing,
        vec![ClosingSession {
            handle: WorkbookHandle::new(1)
        }]
    );
    registry.finish_close(WorkbookHandle::new(1))?;
    assert_eq!(
        registry
            .lookup("bus-a", "task-a", &first.token)
            .err()
            .ok_or_else(|| invalid_arguments("expired token unexpectedly passed"))?
            .code(),
        ErrorCode::ToolInvalidArgs
    );
    let second = registry.register("task-b", opened(2))?;
    assert_eq!(
        registry.begin_close_task("task-b"),
        Some(ClosingSession {
            handle: WorkbookHandle::new(2)
        })
    );
    registry.finish_close(WorkbookHandle::new(2))?;
    assert_eq!(
        registry
            .lookup("bus-a", "task-b", &second.token)
            .err()
            .ok_or_else(|| invalid_arguments("cleaned token unexpectedly passed"))?
            .code(),
        ErrorCode::ToolInvalidArgs
    );
    Ok(())
}

#[test]
fn test_registry_rejects_invalid_opaque_token_length() -> Result<(), ExcelT21Error> {
    let clock = Arc::new(FakeClock::default());
    let mut registry =
        SessionRegistry::new("bus-a", clock, Arc::new(FixedTokenSource("short-token")))?;
    assert_eq!(
        registry
            .register("task-a", opened(1))
            .err()
            .ok_or_else(|| invalid_arguments("short token unexpectedly passed"))?
            .code(),
        ErrorCode::ToolInvalidArgs
    );
    Ok(())
}

#[test]
fn test_handlers_open_read_summarize_and_cleanup_fake_workbook() -> Result<(), ExcelT21Error> {
    let (runtime, handlers, call, token) = open_fixture()?;
    let metadata = handlers.workbook_read(&call, &arguments(json!({"session_id":token}))?)?;
    assert!(metadata.untrusted);
    assert_eq!(
        metadata
            .data
            .get("fingerprint_scope")
            .and_then(Value::as_str),
        Some("workbook")
    );
    let range = handlers.range_read(
        &call,
        &arguments(json!({
            "session_id":token,
            "range":{"kind":"a1","worksheet":{"kind":"ordinal","value":1},"range":"C3:D4"},
            "mode":"value2"
        }))?,
    )?;
    assert!(range.untrusted);
    assert_eq!(
        range
            .data
            .get("cells")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(4)
    );
    assert_eq!(
        range
            .data
            .get("cells")
            .and_then(Value::as_array)
            .and_then(|cells| cells.first())
            .and_then(|cell| cell.get("row"))
            .and_then(Value::as_u64),
        Some(3)
    );
    let summary = handlers.summarize(
        &call,
        &arguments(json!({
            "session_id":token,
            "range":{"kind":"named_range","name":"SalesData"},
            "mode":"value2"
        }))?,
    )?;
    assert_eq!(summary.data.get("sum").and_then(Value::as_str), Some("6"));
    handlers.cleanup_task("task-a")?;
    let (_, closed, active) = runtime.snapshot()?;
    assert_eq!(closed, vec![1]);
    assert_eq!(active, 0);
    assert_eq!(
        handlers
            .workbook_read(&call, &arguments(json!({"session_id":token}))?)
            .err()
            .ok_or_else(|| invalid_arguments("cleaned token unexpectedly passed"))?
            .code(),
        ErrorCode::ToolInvalidArgs
    );
    Ok(())
}

#[test]
fn test_global_session_capacity_is_shared_across_registries() -> Result<(), ExcelT21Error> {
    let capacity = SessionCapacity::default();
    let clock_a = Arc::new(FakeClock::default());
    let clock_b = Arc::new(FakeClock::default());
    let mut first = SessionRegistry::with_capacity(
        "bus-a",
        clock_a,
        Arc::new(SequenceTokenSource::default()),
        capacity.clone(),
    )?;
    let mut second = SessionRegistry::with_capacity(
        "bus-b",
        clock_b,
        Arc::new(SequenceTokenSource::default()),
        capacity.clone(),
    )?;
    for index in 0..MAX_ACTIVE_SESSIONS {
        let target = if index % 2 == 0 {
            &mut first
        } else {
            &mut second
        };
        let handle = u64::try_from(index + 1)
            .map_err(|_| ExcelT21Error::new(ErrorCode::Fatal, "test handle does not fit u64"))?;
        target.register(&format!("task-{index}"), opened(handle))?;
    }
    assert_eq!(capacity.active_count(), MAX_ACTIVE_SESSIONS);
    assert_eq!(
        first
            .register("task-over-cap", opened(99))
            .err()
            .ok_or_else(|| invalid_arguments("shared cap unexpectedly passed"))?
            .code(),
        ErrorCode::UserInteraction
    );
    Ok(())
}

#[test]
fn test_capacity_is_released_after_close_and_not_leaked_on_expiry_overflow()
-> Result<(), ExcelT21Error> {
    let capacity = SessionCapacity::default();
    let clock = Arc::new(FakeClock::default());
    clock.set(i64::MAX);
    let mut overflow = SessionRegistry::with_capacity(
        "bus-a",
        clock,
        Arc::new(SequenceTokenSource::default()),
        capacity.clone(),
    )?;
    assert_eq!(
        overflow
            .register("task-overflow", opened(1))
            .err()
            .ok_or_else(|| invalid_arguments("expiry overflow unexpectedly passed"))?
            .code(),
        ErrorCode::Fatal
    );
    assert_eq!(capacity.active_count(), 0);

    let clock = Arc::new(FakeClock::default());
    clock.set(1_700_000_000_000);
    let mut registry = SessionRegistry::with_capacity(
        "bus-b",
        clock.clone(),
        Arc::new(SequenceTokenSource::default()),
        capacity.clone(),
    )?;
    registry.register("task-a", opened(2))?;
    assert_eq!(capacity.active_count(), 1);
    clock.advance(SESSION_TTL_MS);
    let closing = registry.expire_sessions();
    assert_eq!(capacity.active_count(), 1);
    for session in closing {
        registry.finish_close(session.handle)?;
    }
    assert_eq!(capacity.active_count(), 0);
    Ok(())
}

#[test]
fn test_handler_ttl_expiry_closes_runtime_handle() -> Result<(), ExcelT21Error> {
    let runtime = Arc::new(FakeRuntime::default());
    let (registry, clock, _tokens) = test_registry("bus-a")?;
    let handlers = ExcelT21Handlers::new(Arc::clone(&runtime), registry);
    let call = test_context("bus-a", "task-a")?;
    let opened = handlers.open(
        &call,
        &arguments(json!({
            "workbook_path":TEST_WORKBOOK_PATH,
            "read_only":true
        }))?,
    )?;
    let token = token_from(&opened)?;
    clock.advance(SESSION_TTL_MS);
    assert_eq!(
        handlers
            .workbook_read(&call, &arguments(json!({"session_id":token}))?)
            .err()
            .ok_or_else(|| invalid_arguments("expired handler token unexpectedly passed"))?
            .code(),
        ErrorCode::ToolInvalidArgs
    );
    let (_, closed, active) = runtime.snapshot()?;
    assert_eq!(closed, vec![1]);
    assert_eq!(active, 0);
    Ok(())
}

#[test]
fn test_failed_expiry_close_retains_handle_and_capacity_for_retry() -> Result<(), ExcelT21Error> {
    let runtime = Arc::new(FakeRuntime::default());
    runtime.set_close_error_handle(Some(1))?;
    let (registry, clock, _tokens) = test_registry("bus-a")?;
    let handlers = ExcelT21Handlers::new(Arc::clone(&runtime), registry);
    let call = test_context("bus-a", "task-a")?;
    let opened = handlers.open(
        &call,
        &arguments(json!({
            "workbook_path":TEST_WORKBOOK_PATH,
            "read_only":true
        }))?,
    )?;
    let token = token_from(&opened)?;
    clock.advance(SESSION_TTL_MS);
    assert_eq!(
        handlers
            .workbook_read(&call, &arguments(json!({"session_id":token}))?)
            .err()
            .ok_or_else(|| invalid_arguments("failed close unexpectedly passed"))?
            .code(),
        ErrorCode::Fatal
    );
    assert_eq!(runtime.snapshot()?.2, 1);
    assert_eq!(
        handlers
            .workbook_read(&call, &arguments(json!({"session_id":token}))?)
            .err()
            .ok_or_else(|| invalid_arguments("retried expired token unexpectedly passed"))?
            .code(),
        ErrorCode::ToolInvalidArgs
    );
    let (_, closed, active) = runtime.snapshot()?;
    assert_eq!(closed, vec![1]);
    assert_eq!(active, 0);
    Ok(())
}

#[test]
fn test_open_registry_rejection_closes_new_fake_workbook() -> Result<(), ExcelT21Error> {
    let runtime = Arc::new(FakeRuntime::default());
    let (registry, _clock, _tokens) = test_registry("bus-a")?;
    let handlers = ExcelT21Handlers::new(Arc::clone(&runtime), registry);
    for index in 0..MAX_ACTIVE_SESSIONS {
        let call = test_context("bus-a", &format!("task-{index}"))?;
        handlers.open(
            &call,
            &arguments(json!({
                "workbook_path":TEST_WORKBOOK_PATH,
                "read_only":true
            }))?,
        )?;
    }
    let overflow_call = test_context("bus-a", "task-over-cap")?;
    assert_eq!(
        handlers
            .open(
                &overflow_call,
                &arguments(json!({
                    "workbook_path":TEST_WORKBOOK_PATH,
                    "read_only":true
                }))?,
            )
            .err()
            .ok_or_else(|| invalid_arguments("global cap unexpectedly passed"))?
            .code(),
        ErrorCode::UserInteraction
    );
    let (_, closed, active) = runtime.snapshot()?;
    let overflow_handle = u64::try_from(MAX_ACTIVE_SESSIONS + 1)
        .map_err(|_| ExcelT21Error::new(ErrorCode::Fatal, "overflow handle does not fit u64"))?;
    assert_eq!(closed, vec![overflow_handle]);
    assert_eq!(active, MAX_ACTIVE_SESSIONS);
    Ok(())
}

#[test]
fn test_open_identity_and_ownership_failures_close_new_handle() -> Result<(), ExcelT21Error> {
    let runtime = Arc::new(FakeRuntime::default());
    runtime.set_opened_identity_override(Some(format!("{TEST_WORKBOOK_PATH}.xlsx")))?;
    let (registry, _clock, _tokens) = test_registry("bus-a")?;
    let handlers = ExcelT21Handlers::new(Arc::clone(&runtime), registry);
    let call = test_context("bus-a", "task-a")?;
    assert_eq!(
        handlers
            .open(
                &call,
                &arguments(json!({
                    "workbook_path":TEST_WORKBOOK_PATH,
                    "read_only":true
                }))?,
            )
            .err()
            .ok_or_else(|| invalid_arguments("identity mismatch unexpectedly passed"))?
            .code(),
        ErrorCode::Fatal
    );
    assert_eq!(runtime.snapshot()?.1, vec![1]);

    let runtime = Arc::new(FakeRuntime::default());
    runtime.set_ownership_error(true)?;
    let (registry, _clock, _tokens) = test_registry("bus-a")?;
    let handlers = ExcelT21Handlers::new(Arc::clone(&runtime), registry);
    assert_eq!(
        handlers
            .open(
                &call,
                &arguments(json!({
                    "workbook_path":TEST_WORKBOOK_PATH,
                    "read_only":true
                }))?,
            )
            .err()
            .ok_or_else(|| invalid_arguments("ownership failure unexpectedly passed"))?
            .code(),
        ErrorCode::TargetNotFound
    );
    assert_eq!(runtime.snapshot()?.1, vec![1]);
    assert_eq!(runtime.snapshot()?.2, 0);
    Ok(())
}

#[test]
fn test_open_rejects_false_read_only_and_runtime_existing_process() -> Result<(), ExcelT21Error> {
    let runtime = Arc::new(FakeRuntime::default());
    let (registry, _clock, _tokens) = test_registry("bus-a")?;
    let handlers = ExcelT21Handlers::new(Arc::clone(&runtime), registry);
    let call = test_context("bus-a", "task-a")?;
    assert_eq!(
        handlers
            .open(
                &call,
                &arguments(json!({
                    "workbook_path":TEST_WORKBOOK_PATH,
                    "read_only":false
                }))?,
            )
            .err()
            .ok_or_else(|| invalid_arguments("false read_only unexpectedly passed"))?
            .code(),
        ErrorCode::ToolInvalidArgs
    );
    runtime.set_open_error(ExcelT21Error::new(
        ErrorCode::UserInteraction,
        "existing EXCEL.EXE",
    ))?;
    assert_eq!(
        handlers
            .open(
                &call,
                &arguments(json!({
                    "workbook_path":TEST_WORKBOOK_PATH,
                    "read_only":true
                }))?,
            )
            .err()
            .ok_or_else(|| invalid_arguments("existing process unexpectedly passed"))?
            .code(),
        ErrorCode::UserInteraction
    );
    assert_eq!(runtime.snapshot()?.0, Vec::<String>::new());
    Ok(())
}

#[test]
fn test_handlers_reject_cross_task_and_fingerprint_drift() -> Result<(), ExcelT21Error> {
    let (runtime, handlers, call, token) = open_fixture()?;
    assert_eq!(
        handlers
            .workbook_read(
                &test_context("bus-a", "task-b")?,
                &arguments(json!({"session_id":token}))?,
            )
            .err()
            .ok_or_else(|| invalid_arguments("cross-task handler read unexpectedly passed"))?
            .code(),
        ErrorCode::ToolInvalidArgs
    );
    runtime.set_fingerprint_drift(true)?;
    assert_eq!(
        handlers
            .workbook_read(&call, &arguments(json!({"session_id":token}))?)
            .err()
            .ok_or_else(|| invalid_arguments("fingerprint drift unexpectedly passed"))?
            .code(),
        ErrorCode::VerifyFailed
    );
    Ok(())
}

#[test]
fn test_range_handler_rejects_a1_bounds_before_runtime() -> Result<(), ExcelT21Error> {
    let (_runtime, handlers, call, token) = open_fixture()?;
    assert_eq!(
        handlers
            .range_read(
                &call,
                &arguments(json!({
                    "session_id":token,
                    "range":{"kind":"a1","worksheet":{"kind":"name","value":"Sheet1"},"range":"A1:BM65"},
                    "mode":"value2"
                }))?,
            )
            .err()
            .ok_or_else(|| invalid_arguments("over-bound range unexpectedly passed"))?
            .code(),
        ErrorCode::ToolInvalidArgs
    );
    handlers.cleanup_task("task-a")?;
    Ok(())
}

#[test]
fn test_range_read_preserves_explicit_formula_and_text_modes() -> Result<(), ExcelT21Error> {
    let (_runtime, handlers, call, token) = open_fixture()?;
    for mode in ["formula", "text"] {
        let result = handlers.range_read(
            &call,
            &arguments(json!({
                "session_id":token,
                "range":{"kind":"a1","worksheet":{"kind":"name","value":"Sheet1"},"range":"C3"},
                "mode":mode
            }))?,
        )?;
        assert_eq!(
            result
                .data
                .get("cells")
                .and_then(Value::as_array)
                .and_then(|cells| cells.first())
                .and_then(|cell| cell.get("mode"))
                .and_then(Value::as_str),
            Some(mode)
        );
    }
    Ok(())
}

#[test]
fn test_range_read_rejects_wrong_worksheet_and_unknown_excel_error() -> Result<(), ExcelT21Error> {
    let (runtime, handlers, call, token) = open_fixture()?;
    runtime.set_range_response(RangeSnapshot {
        address: "OtherSheet!C3:D4".to_owned(),
        rows: 2,
        columns: 2,
        cells: test_text_cells_2x2(),
    })?;
    assert_eq!(
        handlers
            .range_read(
                &call,
                &arguments(json!({
                    "session_id":token,
                    "range":{"kind":"a1","worksheet":{"kind":"ordinal","value":1},"range":"C3:D4"},
                    "mode":"value2"
                }))?,
            )
            .err()
            .ok_or_else(|| invalid_arguments("address mismatch unexpectedly passed"))?
            .code(),
        ErrorCode::TargetNotFound
    );
    runtime.set_range_response(RangeSnapshot {
        address: "A1:A1".to_owned(),
        rows: 1,
        columns: 1,
        cells: vec![runtime_cell(
            1,
            1,
            CellValueType::Error,
            "#UNKNOWN_INTERNAL",
        )],
    })?;
    assert_eq!(
        handlers
            .range_read(
                &call,
                &arguments(json!({
                    "session_id":token,
                    "range":{"kind":"named_range","name":"SalesData"},
                    "mode":"value2"
                }))?,
            )
            .err()
            .ok_or_else(|| invalid_arguments("unknown Excel error unexpectedly passed"))?
            .code(),
        ErrorCode::UserInteraction
    );
    Ok(())
}

#[test]
fn test_range_handlers_reject_shape_unknown_errors_and_no_numbers() -> Result<(), ExcelT21Error> {
    let runtime = Arc::new(FakeRuntime::default());
    let (registry, _clock, _tokens) = test_registry("bus-a")?;
    let handlers = ExcelT21Handlers::new(Arc::clone(&runtime), registry);
    let call = test_context("bus-a", "task-a")?;
    let opened = handlers.open(
        &call,
        &arguments(json!({
            "workbook_path":TEST_WORKBOOK_PATH,
            "read_only":true
        }))?,
    )?;
    let token = token_from(&opened)?;
    runtime.set_range_response(RangeSnapshot {
        address: "A1:A2".to_owned(),
        rows: 2,
        columns: 1,
        cells: vec![runtime_cell(
            1,
            1,
            CellValueType::Error,
            "#UNKNOWN_INTERNAL",
        )],
    })?;
    assert_eq!(
        handlers
            .range_read(
                &call,
                &arguments(json!({
                    "session_id":token,
                    "range":{"kind":"named_range","name":"SalesData"},
                    "mode":"value2"
                }))?,
            )
            .err()
            .ok_or_else(|| invalid_arguments("partial malformed range unexpectedly passed"))?
            .code(),
        ErrorCode::UserInteraction
    );
    runtime.set_range_response(RangeSnapshot {
        address: "A1:A2".to_owned(),
        rows: 2,
        columns: 1,
        cells: vec![
            runtime_cell(1, 1, CellValueType::Text, "alpha"),
            runtime_cell(2, 1, CellValueType::Blank, ""),
        ],
    })?;
    assert_eq!(
        handlers
            .summarize(
                &call,
                &arguments(json!({
                    "session_id":token,
                    "range":{"kind":"named_range","name":"SalesData"},
                    "mode":"value2"
                }))?,
            )
            .err()
            .ok_or_else(|| invalid_arguments("all-text summary unexpectedly passed"))?
            .code(),
        ErrorCode::ToolInvalidArgs
    );
    Ok(())
}

#[test]
fn test_summarize_rejects_formula_mode_and_handles_negative_min_max() -> Result<(), ExcelT21Error> {
    let (runtime, handlers, call, token) = open_fixture()?;
    assert_eq!(
        handlers
            .summarize(
                &call,
                &arguments(json!({
                    "session_id":token,
                    "range":{"kind":"named_range","name":"SalesData"},
                    "mode":"formula"
                }))?,
            )
            .err()
            .ok_or_else(|| invalid_arguments("formula summary unexpectedly passed"))?
            .code(),
        ErrorCode::ToolInvalidArgs
    );
    runtime.set_range_response(RangeSnapshot {
        address: "A1:A3".to_owned(),
        rows: 3,
        columns: 1,
        cells: vec![
            runtime_cell(1, 1, CellValueType::Number, "-5"),
            runtime_cell(2, 1, CellValueType::Number, "2.5"),
            runtime_cell(3, 1, CellValueType::Boolean, "true"),
        ],
    })?;
    let summary = handlers.summarize(
        &call,
        &arguments(json!({
            "session_id":token,
            "range":{"kind":"named_range","name":"SalesData"},
            "mode":"value2"
        }))?,
    )?;
    assert_eq!(summary.data.get("min").and_then(Value::as_str), Some("-5"));
    assert_eq!(summary.data.get("max").and_then(Value::as_str), Some("2.5"));
    assert_eq!(
        summary.data.get("sum").and_then(Value::as_str),
        Some("-2.5")
    );
    assert_eq!(
        summary
            .data
            .get("non_numeric_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    Ok(())
}

#[test]
fn test_workbook_read_rejects_worksheet_and_named_range_bounds() -> Result<(), ExcelT21Error> {
    let runtime = Arc::new(FakeRuntime::default());
    let (registry, _clock, _tokens) = test_registry("bus-a")?;
    let handlers = ExcelT21Handlers::new(Arc::clone(&runtime), registry);
    let call = test_context("bus-a", "task-a")?;
    let opened = handlers.open(
        &call,
        &arguments(json!({
            "workbook_path":TEST_WORKBOOK_PATH,
            "read_only":true
        }))?,
    )?;
    let token = token_from(&opened)?;
    let workbook = WorkbookIdentity {
        full_path: TEST_WORKBOOK_PATH.to_owned(),
        read_only: true,
    };
    let worksheets = (1..=MAX_WORKSHEETS + 1)
        .map(|ordinal| {
            Ok(WorksheetInfo {
                name: format!("Sheet{ordinal}"),
                ordinal: u32::try_from(ordinal).map_err(|_| {
                    ExcelT21Error::new(ErrorCode::Fatal, "worksheet ordinal does not fit u32")
                })?,
                used_range_address: None,
            })
        })
        .collect::<Result<Vec<_>, ExcelT21Error>>()?;
    runtime.set_workbook_response(WorkbookSnapshot {
        workbook: workbook.clone(),
        worksheets,
        named_ranges: Vec::new(),
        calculation: CalculationInfo {
            mode: "automatic".to_owned(),
            state: "done".to_owned(),
        },
    })?;
    assert_eq!(
        handlers
            .workbook_read(&call, &arguments(json!({"session_id":token}))?)
            .err()
            .ok_or_else(|| invalid_arguments("worksheet bound unexpectedly passed"))?
            .code(),
        ErrorCode::UserInteraction
    );
    let named_ranges = (0..=MAX_NAMED_RANGES)
        .map(|index| NamedRangeInfo {
            name: format!("Name{index}"),
            scope: NamedRangeScope::Workbook,
            refers_to: "=Sheet1!$A$1".to_owned(),
        })
        .collect();
    runtime.set_workbook_response(WorkbookSnapshot {
        workbook,
        worksheets: vec![WorksheetInfo {
            name: "Sheet1".to_owned(),
            ordinal: 1,
            used_range_address: Some("A1".to_owned()),
        }],
        named_ranges,
        calculation: CalculationInfo {
            mode: "automatic".to_owned(),
            state: "done".to_owned(),
        },
    })?;
    assert_eq!(
        handlers
            .workbook_read(&call, &arguments(json!({"session_id":token}))?)
            .err()
            .ok_or_else(|| invalid_arguments("named-range bound unexpectedly passed"))?
            .code(),
        ErrorCode::UserInteraction
    );
    Ok(())
}

#[test]
fn test_excel_tools_declaration_remains_empty() -> Result<(), ExcelT21Error> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../adapters/com.microsoft.excel/tools/tools.json");
    let body = std::fs::read_to_string(path).map_err(|error| {
        ExcelT21Error::new(
            ErrorCode::Fatal,
            format!("Excel tools declaration cannot be read: {error}"),
        )
    })?;
    let value: Value = serde_json::from_str(&body).map_err(|error| {
        ExcelT21Error::new(
            ErrorCode::Fatal,
            format!("Excel tools declaration is malformed: {error}"),
        )
    })?;
    assert_eq!(
        value.get("tools").and_then(Value::as_array).map(Vec::len),
        Some(0)
    );
    Ok(())
}
