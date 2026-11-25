//! Private contract tests for the Excel T2.2 scalar range write transaction.
//!
//! The fake port only mutates synthetic snapshots. No workbook, COM object, GUI,
//! filesystem shadow copy, save, or production registration is touched.
use super::*;
use crate::excel_t21::l1::ExcelL1CaptureRequest;
use crate::excel_t21::{
    A1Cell, OpenedWorkbook, OwnedProcessIdentity, WorkbookHandle, WorkbookIdentity, WorksheetInfo,
};

type TestResult<T = ()> = Result<T, String>;

macro_rules! require_ok {
    ($expression:expr, $context:expr) => {
        match $expression {
            Ok(value) => value,
            Err(error) => return Err(format!("{}: {error}", $context)),
        }
    };
}

macro_rules! require_error {
    ($expression:expr, $context:expr) => {
        match $expression {
            Ok(_) => return Err(format!("{}: unexpectedly succeeded", $context)),
            Err(error) => error,
        }
    };
}

fn workbook_path() -> String {
    std::env::temp_dir()
        .join("task-406-book.xlsx")
        .to_string_lossy()
        .into_owned()
}

fn workbook_identity(fingerprint_value: char) -> ExcelL1WorkbookIdentity {
    let executable_path = std::env::current_exe().map_or_else(
        |_| {
            if cfg!(windows) {
                "C:\\Windows\\System32\\cmd.exe".to_owned()
            } else {
                "/usr/bin/excel".to_owned()
            }
        },
        |path| path.to_string_lossy().into_owned(),
    );
    let opened = OpenedWorkbook {
        handle: WorkbookHandle::new(406),
        owned_process: OwnedProcessIdentity {
            process_id: 406,
            start_ticks: 406,
            executable_path,
        },
        workbook: WorkbookIdentity {
            full_path: workbook_path(),
            read_only: false,
        },
        worksheets: vec![WorksheetInfo {
            name: "Sheet1".to_owned(),
            ordinal: 1,
            used_range_address: Some("Sheet1!A1:B2".to_owned()),
        }],
    };
    ExcelL1WorkbookIdentity::from_opened(
        "toolbus-session-406".to_owned(),
        "task-406".to_owned(),
        &opened,
        fingerprint(fingerprint_value),
    )
}

fn single_cell_selector() -> RangeSelector {
    let cell = A1Cell { row: 1, column: 1 };
    RangeSelector::A1 {
        worksheet: WorksheetSelector::Name("Sheet1".to_owned()),
        start: cell,
        end: cell,
    }
}

fn four_cell_selector() -> RangeSelector {
    RangeSelector::A1 {
        worksheet: WorksheetSelector::Name("Sheet1".to_owned()),
        start: A1Cell { row: 1, column: 1 },
        end: A1Cell { row: 2, column: 2 },
    }
}

fn named_selector() -> RangeSelector {
    RangeSelector::NamedRange("SalesData".to_owned())
}

fn capture_request(selector: RangeSelector) -> TestResult<ExcelL1CaptureRequest> {
    let (address, rows, columns) = selector_dimensions(&selector, "Sheet1!A1")?;
    let cells = (1..=rows)
        .flat_map(|row| {
            (1..=columns).map(move |column| RuntimeCell {
                row,
                column,
                value_type: CellValueType::Number,
                value_text: "1".to_owned(),
            })
        })
        .collect();
    Ok(ExcelL1CaptureRequest {
        anchor_id: "anchor-406".to_owned(),
        step_id: "step-406".to_owned(),
        workbook: workbook_identity('a'),
        selector,
        snapshot: RangeSnapshot {
            address,
            rows,
            columns,
            cells,
        },
        mode: ReadMode::Value2,
        file_bytes: b"task-406-workbook-before-write".to_vec(),
    })
}

fn captured(
    selector: RangeSelector,
) -> TestResult<(
    ExcelL1AnchorStore,
    ExcelL1AnchorHandle,
    ExcelL1WorkbookIdentity,
)> {
    let mut store = ExcelL1AnchorStore::new();
    let request = capture_request(selector)?;
    let observed = request.workbook.clone();
    let handle = require_ok!(store.capture(request), "capture");
    Ok((store, handle, observed))
}

fn selector_dimensions(
    selector: &RangeSelector,
    fallback_address: &str,
) -> TestResult<(String, u32, u32)> {
    match selector {
        RangeSelector::NamedRange(_) => Ok((fallback_address.to_owned(), 1, 1)),
        RangeSelector::A1 { start, end, .. } => {
            let rows = end.row.saturating_sub(start.row).saturating_add(1);
            let columns = end.column.saturating_sub(start.column).saturating_add(1);
            let start_address =
                require_ok!(cell_address(start.row, start.column), "start cell address");
            let end_address = require_ok!(cell_address(end.row, end.column), "end cell address");
            let address = if start_address == end_address {
                format!("Sheet1!{start_address}")
            } else {
                format!("Sheet1!{start_address}:{end_address}")
            };
            Ok((address, rows, columns))
        }
    }
}

fn fingerprint(value: char) -> String {
    fake_fingerprint(value)
}

#[test]
fn test_write_port_executes_closed_scalar_set_and_records_post_fingerprint() -> TestResult {
    let cases = [
        ("null", CellValueType::Blank, "", ""),
        ("true", CellValueType::Boolean, "true", "TRUE"),
        ("false", CellValueType::Boolean, "false", "FALSE"),
        ("-12.50", CellValueType::Number, "-12.5", "-12.5"),
        (
            "9007199254740993",
            CellValueType::Number,
            "9007199254740993",
            "9007199254740993",
        ),
        (r#""hello""#, CellValueType::Text, "hello", "hello"),
    ];
    for (input, expected_type, expected_value, expected_formula) in cases {
        let (mut store, handle, observed) = captured(single_cell_selector())?;
        let mut port = FakePort::new();
        let selector = single_cell_selector();
        let receipt = require_ok!(
            execute_scalar_write(&mut store, handle, &observed, &selector, input, &mut port,),
            "scalar write"
        );
        assert_eq!(receipt.value_type(), expected_type);
        assert_eq!(receipt.value_text(), expected_value);
        assert_eq!(receipt.formula_text(), expected_formula);
        assert_eq!(receipt.cell_count(), 1);
        assert_eq!(receipt.post_fingerprint(), fingerprint('b'));
        assert_eq!(
            port.call_names()?,
            vec![
                "write",
                "recalculate",
                "read:value2",
                "read:formula",
                "fingerprint"
            ]
        );
        {
            let state = require_ok!(port.lock(), "lock fake port");
            assert_eq!(state.last_selector.as_ref(), Some(&selector));
            assert_eq!(state.written_value_type, Some(expected_type));
            assert_eq!(state.written_text.as_deref(), Some(expected_value));
        }
        assert_eq!(
            require_ok!(store.anchor(handle), "anchor")
                .post_fingerprint()
                .map(ToString::to_string)
                .as_deref(),
            Some(fingerprint('b').as_str())
        );
    }
    Ok(())
}

#[test]
fn test_write_port_fills_multi_cell_range_with_the_same_scalar() -> TestResult {
    let (mut store, handle, observed) = captured(four_cell_selector())?;
    let mut port = FakePort::new();
    let receipt = require_ok!(
        execute_scalar_write(
            &mut store,
            handle,
            &observed,
            &four_cell_selector(),
            "42",
            &mut port,
        ),
        "multi-cell write"
    );
    assert_eq!(receipt.cell_count(), 4);
    assert_eq!(receipt.value_text(), "42");
    assert_eq!(receipt.formula_text(), "42");
    Ok(())
}

#[test]
fn test_write_port_binds_named_range_readback_to_the_captured_address() -> TestResult {
    let (mut store, handle, observed) = captured(named_selector())?;
    let mut port = FakePort::new();
    let receipt = require_ok!(
        execute_scalar_write(
            &mut store,
            handle,
            &observed,
            &named_selector(),
            "5",
            &mut port,
        ),
        "named range write"
    );
    assert_eq!(receipt.cell_count(), 1);
    assert_eq!(
        require_ok!(port.lock(), "lock fake port")
            .last_selector
            .as_ref(),
        Some(&named_selector())
    );

    let (mut store, handle, observed) = captured(named_selector())?;
    let mut port = FakePort::new();
    require_ok!(port.lock(), "lock fake port").value_snapshot_override = Some(RangeSnapshot {
        address: "Sheet2!A1".to_owned(),
        rows: 1,
        columns: 1,
        cells: vec![RuntimeCell {
            row: 1,
            column: 1,
            value_type: CellValueType::Number,
            value_text: "5".to_owned(),
        }],
    });
    let error = require_error!(
        execute_scalar_write(
            &mut store,
            handle,
            &observed,
            &named_selector(),
            "5",
            &mut port,
        ),
        "named range address drift"
    );
    assert_eq!(error.code(), ErrorCode::UserInteraction);
    assert!(
        require_ok!(store.anchor(handle), "anchor")
            .post_fingerprint()
            .is_none()
    );
    Ok(())
}

#[test]
fn test_write_port_rejects_selector_drift_before_consuming_anchor() -> TestResult {
    let (mut store, handle, observed) = captured(single_cell_selector())?;
    let mut port = FakePort::new();
    let other_cell = RangeSelector::A1 {
        worksheet: WorksheetSelector::Ordinal(1),
        start: A1Cell { row: 1, column: 2 },
        end: A1Cell { row: 1, column: 2 },
    };
    let error = require_error!(
        execute_scalar_write(&mut store, handle, &observed, &other_cell, "5", &mut port,),
        "selector drift"
    );
    assert_eq!(error.code(), ErrorCode::TargetNotFound);
    assert!(port.call_names()?.is_empty());

    let receipt = require_ok!(
        execute_scalar_write(
            &mut store,
            handle,
            &observed,
            &single_cell_selector(),
            "5",
            &mut port,
        ),
        "unchanged selector after drift"
    );
    assert_eq!(receipt.cell_count(), 1);
    Ok(())
}

#[test]
fn test_write_port_scalar_validation_failure_does_not_consume_anchor() -> TestResult {
    let (mut store, handle, observed) = captured(single_cell_selector())?;
    let mut port = FakePort::new();
    let error = require_error!(
        execute_scalar_write(
            &mut store,
            handle,
            &observed,
            &single_cell_selector(),
            r#"{"bad":true}"#,
            &mut port,
        ),
        "non-scalar input"
    );
    assert_eq!(error.code(), ErrorCode::ToolInvalidArgs);
    assert!(port.call_names()?.is_empty());
    require_ok!(
        execute_scalar_write(
            &mut store,
            handle,
            &observed,
            &single_cell_selector(),
            "5",
            &mut port,
        ),
        "valid scalar after parser rejection"
    );
    Ok(())
}

#[test]
fn test_write_port_rejects_prepare_identity_drift_without_port_calls() -> TestResult {
    let (mut store, handle, observed) = captured(single_cell_selector())?;
    let mut fingerprint_drift = observed.clone();
    fingerprint_drift.fingerprint = fingerprint('b');
    let mut port = FakePort::new();
    let error = require_error!(
        execute_scalar_write(
            &mut store,
            handle,
            &fingerprint_drift,
            &single_cell_selector(),
            "5",
            &mut port,
        ),
        "fingerprint drift"
    );
    assert_eq!(error.code(), ErrorCode::VerifyFailed);
    assert!(port.call_names()?.is_empty());

    let mut identity_drift = observed.clone();
    identity_drift.workbook_handle = WorkbookHandle::new(999);
    let error = require_error!(
        execute_scalar_write(
            &mut store,
            handle,
            &identity_drift,
            &single_cell_selector(),
            "5",
            &mut port,
        ),
        "identity drift"
    );
    assert_eq!(error.code(), ErrorCode::VerifyFailed);
    assert!(port.call_names()?.is_empty());

    require_ok!(
        execute_scalar_write(
            &mut store,
            handle,
            &observed,
            &single_cell_selector(),
            "5",
            &mut port,
        ),
        "valid identity after drift"
    );
    Ok(())
}

#[test]
fn test_write_port_failure_consumes_single_use_anchor_without_post_fingerprint() -> TestResult {
    let (mut store, handle, observed) = captured(single_cell_selector())?;
    let mut port = FakePort::new();
    require_ok!(port.lock(), "lock fake port").fail_write = Some(ErrorCode::TargetUnresponsive);
    let error = require_error!(
        execute_scalar_write(
            &mut store,
            handle,
            &observed,
            &single_cell_selector(),
            "5",
            &mut port,
        ),
        "scripted write failure"
    );
    assert_eq!(error.code(), ErrorCode::TargetUnresponsive);
    assert_eq!(port.call_names()?, vec!["write"]);
    assert!(
        require_ok!(store.anchor(handle), "anchor")
            .post_fingerprint()
            .is_none()
    );
    require_ok!(port.lock(), "unlock fake write failure").fail_write = None;

    let retry = require_error!(
        execute_scalar_write(
            &mut store,
            handle,
            &observed,
            &single_cell_selector(),
            "6",
            &mut port,
        ),
        "retry after consumed write"
    );
    assert_eq!(retry.code(), ErrorCode::VerifyFailed);
    assert_eq!(port.call_names()?, vec!["write"]);
    Ok(())
}

#[test]
fn test_write_port_failures_after_side_effect_never_record_post() -> TestResult {
    let cases = ["recalculate", "read:value2", "read:formula", "fingerprint"];
    for failure in cases {
        let (mut store, handle, observed) = captured(single_cell_selector())?;
        let mut port = FakePort::new();
        {
            let mut state = require_ok!(port.lock(), "lock fake port");
            match failure {
                "recalculate" => state.fail_recalculate = Some(ErrorCode::TargetUnresponsive),
                "fingerprint" => state.fail_fingerprint = Some(ErrorCode::TargetUnresponsive),
                "read:value2" => state.fail_read_value2 = Some(ErrorCode::TargetUnresponsive),
                "read:formula" => state.fail_read_formula = Some(ErrorCode::TargetUnresponsive),
                _ => return Err(format!("unknown failure case {failure}")),
            }
            drop(state);
        }
        let error = require_error!(
            execute_scalar_write(
                &mut store,
                handle,
                &observed,
                &single_cell_selector(),
                "5",
                &mut port,
            ),
            failure
        );
        assert_eq!(error.code(), ErrorCode::TargetUnresponsive);
        assert!(
            require_ok!(store.anchor(handle), "anchor")
                .post_fingerprint()
                .is_none()
        );
        let expected_calls = match failure {
            "recalculate" => vec!["write", "recalculate"],
            "read:value2" => vec!["write", "recalculate", "read:value2"],
            "read:formula" => {
                vec!["write", "recalculate", "read:value2", "read:formula"]
            }
            "fingerprint" => vec![
                "write",
                "recalculate",
                "read:value2",
                "read:formula",
                "fingerprint",
            ],
            _ => return Err(format!("unknown failure case {failure}")),
        };
        assert_eq!(port.call_names()?, expected_calls);

        {
            let mut state = require_ok!(port.lock(), "clear fake failure");
            state.fail_recalculate = None;
            state.fail_read_value2 = None;
            state.fail_read_formula = None;
            state.fail_fingerprint = None;
        }
        let before_retry = port.call_names()?;
        let retry = require_error!(
            execute_scalar_write(
                &mut store,
                handle,
                &observed,
                &single_cell_selector(),
                "6",
                &mut port,
            ),
            "retry after side-effect failure"
        );
        assert_eq!(retry.code(), ErrorCode::VerifyFailed);
        assert_eq!(port.call_names()?, before_retry);
    }
    Ok(())
}

#[test]
fn test_write_port_rejects_mode_or_formula_drift_and_consumes_anchor() -> TestResult {
    for drift in ["mode", "formula"] {
        let (mut store, handle, observed) = captured(single_cell_selector())?;
        let mut port = FakePort::new();
        {
            let mut state = require_ok!(port.lock(), "lock fake port");
            match drift {
                "mode" => state.value_mode_override = Some(ReadMode::Text),
                "formula" => {
                    state.formula_snapshot_override = Some(RangeSnapshot {
                        address: "Sheet1!A1".to_owned(),
                        rows: 1,
                        columns: 1,
                        cells: vec![RuntimeCell {
                            row: 1,
                            column: 1,
                            value_type: CellValueType::Text,
                            value_text: "=1+1".to_owned(),
                        }],
                    });
                }
                _ => return Err(format!("unknown drift case {drift}")),
            }
            drop(state);
        }
        let error = require_error!(
            execute_scalar_write(
                &mut store,
                handle,
                &observed,
                &single_cell_selector(),
                "5",
                &mut port,
            ),
            drift
        );
        assert!(matches!(
            error.code(),
            ErrorCode::UserInteraction | ErrorCode::VerifyFailed
        ));
        assert!(
            require_ok!(store.anchor(handle), "anchor")
                .post_fingerprint()
                .is_none()
        );
        let calls_before_retry = port.call_names()?;
        {
            let mut state = require_ok!(port.lock(), "clear drift");
            state.value_mode_override = None;
            state.formula_snapshot_override = None;
        }
        let retry = require_error!(
            execute_scalar_write(
                &mut store,
                handle,
                &observed,
                &single_cell_selector(),
                "6",
                &mut port,
            ),
            "retry after drift"
        );
        assert_eq!(retry.code(), ErrorCode::VerifyFailed);
        assert_eq!(port.call_names()?, calls_before_retry);
    }
    Ok(())
}

fn fake_number_cell(row: u32, column: u32) -> RuntimeCell {
    RuntimeCell {
        row,
        column,
        value_type: CellValueType::Number,
        value_text: "5".to_owned(),
    }
}

fn cell_set_readback_cases() -> [(&'static str, RangeSnapshot); 2] {
    [
        (
            "partial",
            RangeSnapshot {
                address: "Sheet1!A1:B2".to_owned(),
                rows: 2,
                columns: 2,
                cells: vec![fake_number_cell(1, 1)],
            },
        ),
        (
            "out-of-bounds",
            RangeSnapshot {
                address: "Sheet1!A1:B2".to_owned(),
                rows: 2,
                columns: 2,
                cells: vec![
                    fake_number_cell(1, 1),
                    fake_number_cell(1, 2),
                    fake_number_cell(2, 1),
                    fake_number_cell(3, 1),
                ],
            },
        ),
    ]
}

fn address_readback_cases() -> [(&'static str, RangeSnapshot); 4] {
    [
        (
            "address",
            RangeSnapshot {
                address: "Sheet1!C1:D2".to_owned(),
                rows: 2,
                columns: 2,
                cells: vec![
                    fake_number_cell(1, 1),
                    fake_number_cell(1, 2),
                    fake_number_cell(2, 1),
                    fake_number_cell(2, 2),
                ],
            },
        ),
        (
            "dimensions",
            RangeSnapshot {
                address: "Sheet1!A1".to_owned(),
                rows: 2,
                columns: 2,
                cells: vec![
                    fake_number_cell(1, 1),
                    fake_number_cell(1, 2),
                    fake_number_cell(2, 1),
                    fake_number_cell(2, 2),
                ],
            },
        ),
        (
            "worksheet",
            RangeSnapshot {
                address: "Sheet2!A1:B2".to_owned(),
                rows: 2,
                columns: 2,
                cells: vec![
                    fake_number_cell(1, 1),
                    fake_number_cell(1, 2),
                    fake_number_cell(2, 1),
                    fake_number_cell(2, 2),
                ],
            },
        ),
        (
            "type",
            RangeSnapshot {
                address: "Sheet1!A1:B2".to_owned(),
                rows: 2,
                columns: 2,
                cells: vec![
                    RuntimeCell {
                        row: 1,
                        column: 1,
                        value_type: CellValueType::Text,
                        value_text: "5".to_owned(),
                    },
                    fake_number_cell(1, 2),
                    fake_number_cell(2, 1),
                    fake_number_cell(2, 2),
                ],
            },
        ),
    ]
}

#[test]
fn test_write_port_rejects_malformed_or_drifted_readbacks() -> TestResult {
    let cases = cell_set_readback_cases()
        .into_iter()
        .chain(address_readback_cases());
    for (case, snapshot) in cases {
        let (mut store, handle, observed) = captured(four_cell_selector())?;
        let mut port = FakePort::new();
        require_ok!(port.lock(), "lock fake port").value_snapshot_override = Some(snapshot);
        let error = require_error!(
            execute_scalar_write(
                &mut store,
                handle,
                &observed,
                &four_cell_selector(),
                "5",
                &mut port,
            ),
            case
        );
        assert!(matches!(
            error.code(),
            ErrorCode::UserInteraction | ErrorCode::VerifyFailed
        ));
        assert!(
            require_ok!(store.anchor(handle), "anchor")
                .post_fingerprint()
                .is_none()
        );
        let calls_before_retry = port.call_names()?;
        let retry = require_error!(
            execute_scalar_write(
                &mut store,
                handle,
                &observed,
                &four_cell_selector(),
                "6",
                &mut port,
            ),
            "retry after malformed readback"
        );
        assert_eq!(retry.code(), ErrorCode::VerifyFailed);
        assert_eq!(port.call_names()?, calls_before_retry);
    }
    Ok(())
}

#[test]
fn test_write_port_rejects_unchanged_or_duplicate_post_fingerprint() -> TestResult {
    let (mut store, handle, observed) = captured(single_cell_selector())?;
    let mut port = FakePort::new();
    require_ok!(port.lock(), "lock fake port").post_fingerprint = Some(fingerprint('a'));
    let error = require_error!(
        execute_scalar_write(
            &mut store,
            handle,
            &observed,
            &single_cell_selector(),
            "5",
            &mut port,
        ),
        "unchanged fingerprint"
    );
    assert_eq!(error.code(), ErrorCode::VerifyFailed);
    assert!(
        require_ok!(store.anchor(handle), "anchor")
            .post_fingerprint()
            .is_none()
    );

    let (mut store, handle, observed) = captured(single_cell_selector())?;
    let mut port = FakePort::new();
    require_ok!(
        execute_scalar_write(
            &mut store,
            handle,
            &observed,
            &single_cell_selector(),
            "5",
            &mut port,
        ),
        "successful write"
    );
    let mut duplicate = observed.clone();
    duplicate.fingerprint = fingerprint('c');
    let duplicate_error = require_error!(
        store.record_post_write(handle, &duplicate),
        "duplicate post fingerprint"
    );
    assert_eq!(duplicate_error.code(), ErrorCode::VerifyFailed);
    Ok(())
}

#[test]
fn test_write_port_rejects_released_handle_without_port_calls() -> TestResult {
    let (mut store, handle, observed) = captured(single_cell_selector())?;
    require_ok!(store.release(handle), "release");
    let mut port = FakePort::new();
    let error = require_error!(
        execute_scalar_write(
            &mut store,
            handle,
            &observed,
            &single_cell_selector(),
            "5",
            &mut port,
        ),
        "released handle"
    );
    assert_eq!(error.code(), ErrorCode::TargetNotFound);
    assert!(port.call_names()?.is_empty());
    Ok(())
}
