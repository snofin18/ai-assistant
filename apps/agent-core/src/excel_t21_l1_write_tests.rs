//! Private contract tests for the Excel T2.2 scalar write input gate.
//!
//! The tests use synthetic workbook identity and anchor bytes only. No workbook,
//! filesystem shadow copy, COM object, GUI, or production registration is touched.
use super::*;
use crate::excel_t21::l1::ExcelL1CaptureRequest;
use crate::excel_t21::{
    OpenedWorkbook, OwnedProcessIdentity, RangeSelector, RangeSnapshot, ReadMode, RuntimeCell,
    WorkbookHandle, WorkbookIdentity, WorksheetInfo, WorksheetSelector,
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

fn fingerprint(value: char) -> String {
    format!("sha256:{}", value.to_string().repeat(64))
}

fn workbook_path() -> String {
    std::env::temp_dir()
        .join("task-405-book.xlsx")
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
        handle: WorkbookHandle::new(405),
        owned_process: OwnedProcessIdentity {
            process_id: 405,
            start_ticks: 405,
            executable_path,
        },
        workbook: WorkbookIdentity {
            full_path: workbook_path(),
            read_only: false,
        },
        worksheets: vec![WorksheetInfo {
            name: "Sheet1".to_owned(),
            ordinal: 1,
            used_range_address: Some("Sheet1!A1".to_owned()),
        }],
    };
    ExcelL1WorkbookIdentity::from_opened(
        "toolbus-session-405".to_owned(),
        "task-405".to_owned(),
        &opened,
        fingerprint(fingerprint_value),
    )
}

fn capture_request() -> ExcelL1CaptureRequest {
    ExcelL1CaptureRequest {
        anchor_id: "anchor-405".to_owned(),
        step_id: "step-405".to_owned(),
        workbook: workbook_identity('a'),
        selector: RangeSelector::A1 {
            worksheet: WorksheetSelector::Name("Sheet1".to_owned()),
            start: crate::excel_t21::A1Cell { row: 1, column: 1 },
            end: crate::excel_t21::A1Cell { row: 1, column: 1 },
        },
        snapshot: RangeSnapshot {
            address: "Sheet1!A1".to_owned(),
            rows: 1,
            columns: 1,
            cells: vec![RuntimeCell {
                row: 1,
                column: 1,
                value_type: CellValueType::Number,
                value_text: "1".to_owned(),
            }],
        },
        mode: ReadMode::Value2,
        file_bytes: b"task-405-workbook-before-write".to_vec(),
    }
}

fn captured() -> TestResult<(
    ExcelL1AnchorStore,
    ExcelL1AnchorHandle,
    ExcelL1WorkbookIdentity,
)> {
    let mut store = ExcelL1AnchorStore::new();
    let request = capture_request();
    let observed = request.workbook.clone();
    let handle = require_ok!(store.capture(request), "capture");
    Ok((store, handle, observed))
}

fn assert_not_prepared(
    store: &mut ExcelL1AnchorStore,
    handle: ExcelL1AnchorHandle,
    observed: &ExcelL1WorkbookIdentity,
) -> TestResult {
    require_ok!(
        prepare_scalar_write(store, handle, observed, "5"),
        "valid scalar after failed validation"
    );
    Ok(())
}

#[test]
fn test_scalar_gate_accepts_closed_set_and_preserves_number_lexical_precision() -> TestResult {
    let cases = [
        ("null", CellValueType::Blank, ""),
        ("true", CellValueType::Boolean, "true"),
        ("false", CellValueType::Boolean, "false"),
        ("-12.50", CellValueType::Number, "-12.5"),
        (
            "9007199254740993.0",
            CellValueType::Number,
            "9007199254740993",
        ),
        (
            "0.10000000000000001",
            CellValueType::Number,
            "0.10000000000000001",
        ),
        (
            "1.234567890123456789",
            CellValueType::Number,
            "1.234567890123456789",
        ),
        (r#"" hello ""#, CellValueType::Text, " hello "),
        (r#""""#, CellValueType::Text, ""),
    ];
    for (input, expected_type, expected_text) in cases {
        let (mut store, handle, observed) = captured()?;
        let literal = require_ok!(
            prepare_scalar_write(&mut store, handle, &observed, input),
            "valid scalar"
        );
        assert_eq!(literal.value_type(), expected_type);
        assert_eq!(literal.value_text(), expected_text);
    }
    Ok(())
}

#[test]
fn test_scalar_gate_rejects_arrays_objects_and_malformed_numbers() -> TestResult {
    for input in [
        "[1]",
        r#"{"value":1}"#,
        "01",
        "+1",
        ".5",
        "1.",
        "1e",
        "NaN",
        "null trailing",
    ] {
        let (mut store, handle, observed) = captured()?;
        let error = require_error!(
            prepare_scalar_write(&mut store, handle, &observed, input),
            "non-scalar or malformed scalar"
        );
        assert_eq!(error.code(), ErrorCode::ToolInvalidArgs);
        assert_not_prepared(&mut store, handle, &observed)?;
    }
    Ok(())
}

#[test]
fn test_scalar_gate_rejects_formula_prefixes_after_hidden_prefix_characters() -> TestResult {
    for input in [
        r#""=SUM(A1:A2)""#,
        r#""+1""#,
        r#""-1""#,
        r#""@cmd""#,
        r#"" \t\n=1""#,
        r#""\u3000+1""#,
        r#""\u0000=1""#,
        r#""\u0001=1""#,
        r#""\uFEFF=1""#,
        r#""\u200B=1""#,
        r#""\u200E=1""#,
        r#""\u200F=1""#,
        r#""\u202E=1""#,
        r#""\u2060@cmd""#,
        r#""\u2066=1""#,
        r#""\u00AD=1""#,
        r#""\u061C=1""#,
        r#""\u034F=1""#,
        r#""\uFE0F=1""#,
    ] {
        let (mut store, handle, observed) = captured()?;
        let error = require_error!(
            prepare_scalar_write(&mut store, handle, &observed, input),
            "formula-like text"
        );
        assert_eq!(error.code(), ErrorCode::ToolInvalidArgs);
        assert_not_prepared(&mut store, handle, &observed)?;
    }

    for hidden in ['\u{E0000}', '\u{E0001}', '\u{E0002}', '\u{E0FFF}'] {
        let astral_hidden = format!("\"{hidden}=1\"");
        let (mut store, handle, observed) = captured()?;
        let error = require_error!(
            prepare_scalar_write(&mut store, handle, &observed, &astral_hidden),
            "astral default-ignorable formula prefix"
        );
        assert_eq!(error.code(), ErrorCode::ToolInvalidArgs);
        assert_not_prepared(&mut store, handle, &observed)?;
    }

    for input in [r#""\u200Bhello""#, r#""\uFEFFprice""#] {
        let (mut store, handle, observed) = captured()?;
        let literal = require_ok!(
            prepare_scalar_write(&mut store, handle, &observed, input),
            "hidden prefix before ordinary text"
        );
        assert_eq!(literal.value_type(), CellValueType::Text);
    }
    Ok(())
}

#[test]
fn test_scalar_gate_accepts_exact_payload_bound_and_rejects_one_byte_over() -> TestResult {
    let accepted_text = "x".repeat(MAX_RANGE_PAYLOAD_BYTES - 2);
    let accepted_input = format!("\"{accepted_text}\"");
    assert_eq!(accepted_input.len(), MAX_RANGE_PAYLOAD_BYTES);
    let (mut store, handle, observed) = captured()?;
    let literal = require_ok!(
        prepare_scalar_write(&mut store, handle, &observed, &accepted_input),
        "exact payload bound"
    );
    assert_eq!(literal.value_text().len(), MAX_RANGE_PAYLOAD_BYTES - 2);

    let rejected_input = format!("\"{}\"", "x".repeat(MAX_RANGE_PAYLOAD_BYTES - 1));
    assert_eq!(rejected_input.len(), MAX_RANGE_PAYLOAD_BYTES + 1);
    let (mut store, handle, observed) = captured()?;
    let error = require_error!(
        prepare_scalar_write(&mut store, handle, &observed, &rejected_input),
        "payload over bound"
    );
    assert_eq!(error.code(), ErrorCode::ToolInvalidArgs);
    assert_not_prepared(&mut store, handle, &observed)?;
    Ok(())
}

#[test]
fn test_scalar_validation_failure_does_not_consume_write_preparation() -> TestResult {
    let (mut store, handle, observed) = captured()?;
    let error = require_error!(
        prepare_scalar_write(&mut store, handle, &observed, r#"{"bad":true}"#),
        "invalid scalar"
    );
    assert_eq!(error.code(), ErrorCode::ToolInvalidArgs);
    assert_not_prepared(&mut store, handle, &observed)
}

#[test]
fn test_scalar_gate_rejects_fingerprint_and_identity_drift() -> TestResult {
    let (mut store, handle, observed) = captured()?;
    let fingerprint_drift = workbook_identity('b');
    let error = require_error!(
        prepare_scalar_write(&mut store, handle, &fingerprint_drift, "5"),
        "fingerprint drift"
    );
    assert_eq!(error.code(), ErrorCode::VerifyFailed);

    let mut identity_drift = workbook_identity('a');
    identity_drift.workbook_handle = WorkbookHandle::new(999);
    let error = require_error!(
        prepare_scalar_write(&mut store, handle, &identity_drift, "5"),
        "identity drift"
    );
    assert_eq!(error.code(), ErrorCode::VerifyFailed);

    require_ok!(
        prepare_scalar_write(&mut store, handle, &observed, "5"),
        "prepared write after drift"
    );
    let error = require_error!(
        prepare_scalar_write(&mut store, handle, &observed, "6"),
        "duplicate preparation"
    );
    assert_eq!(error.code(), ErrorCode::VerifyFailed);
    Ok(())
}

#[test]
fn test_scalar_gate_rejects_released_and_unknown_handles() -> TestResult {
    let (mut store, handle, observed) = captured()?;
    require_ok!(store.release(handle), "release");
    let error = require_error!(
        prepare_scalar_write(&mut store, handle, &observed, "5"),
        "released handle"
    );
    assert_eq!(error.code(), ErrorCode::TargetNotFound);

    let unknown = ExcelL1AnchorHandle(u64::MAX);
    let error = require_error!(
        prepare_scalar_write(&mut store, unknown, &observed, "5"),
        "unknown handle"
    );
    assert_eq!(error.code(), ErrorCode::TargetNotFound);
    Ok(())
}
