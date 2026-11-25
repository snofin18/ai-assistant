//! Private contract tests for the Excel T2.2 L1 anchor store.
//!
//! Tests use caller-provided bytes and synthetic value snapshots only; no workbook,
//! filesystem shadow copy, COM object, GUI, or production registration is touched.
use super::*;

type TestResult<T = ()> = Result<T, String>;

macro_rules! require_ok {
    ($expression:expr, $context:expr) => {
        match $expression {
            Ok(value) => value,
            Err(error) => return Err(format!("{}: {error}", $context)),
        }
    };
}

macro_rules! require_some {
    ($expression:expr, $context:expr) => {
        match $expression {
            Some(value) => value,
            None => return Err(format!("{}: value was absent", $context)),
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
        .join("task-404-book.xlsx")
        .to_string_lossy()
        .into_owned()
}
fn workbook_identity(path: String, fingerprint_value: char) -> ExcelL1WorkbookIdentity {
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
        handle: WorkbookHandle::new(1),
        owned_process: OwnedProcessIdentity {
            process_id: 404,
            start_ticks: 404,
            executable_path,
        },
        workbook: WorkbookIdentity {
            full_path: path,
            read_only: false,
        },
        worksheets: vec![WorksheetInfo {
            name: "Sheet1".to_owned(),
            ordinal: 1,
            used_range_address: Some("Sheet1!A1:B1".to_owned()),
        }],
    };
    ExcelL1WorkbookIdentity::from_opened(
        "toolbus-session-404".to_owned(),
        "task-404".to_owned(),
        &opened,
        fingerprint(fingerprint_value),
    )
}

fn cell(row: u32, column: u32, value_type: CellValueType, value_text: &str) -> RuntimeCell {
    RuntimeCell {
        row,
        column,
        value_type,
        value_text: value_text.to_owned(),
    }
}

fn request(anchor_id: &str, pre: char, file_bytes: &[u8]) -> ExcelL1CaptureRequest {
    ExcelL1CaptureRequest {
        anchor_id: anchor_id.to_owned(),
        step_id: format!("step-{anchor_id}"),
        workbook: workbook_identity(workbook_path(), pre),
        selector: RangeSelector::A1 {
            worksheet: WorksheetSelector::Name("Sheet1".to_owned()),
            start: super::super::A1Cell { row: 1, column: 1 },
            end: super::super::A1Cell { row: 1, column: 2 },
        },
        snapshot: RangeSnapshot {
            address: "Sheet1!A1:B1".to_owned(),
            rows: 1,
            columns: 2,
            cells: vec![
                cell(1, 1, CellValueType::Number, "5.0"),
                cell(1, 2, CellValueType::Text, "hello"),
            ],
        },
        mode: ReadMode::Value2,
        file_bytes: file_bytes.to_vec(),
    }
}

fn single_cell_request(anchor_id: &str, file_bytes: &[u8]) -> ExcelL1CaptureRequest {
    ExcelL1CaptureRequest {
        selector: RangeSelector::NamedRange("SalesData".to_owned()),
        snapshot: RangeSnapshot {
            address: "Sheet1!A1".to_owned(),
            rows: 1,
            columns: 1,
            cells: vec![cell(1, 1, CellValueType::Number, "1")],
        },
        ..request(anchor_id, 'a', file_bytes)
    }
}

#[test]
fn test_capture_binds_value_snapshot_shadow_digest_and_undo_anchor() -> TestResult {
    let mut store = ExcelL1AnchorStore::new();
    let file_bytes = b"workbook-before-write".to_vec();
    let handle = require_ok!(
        store.capture(request("anchor-1", 'a', &file_bytes)),
        "capture"
    );

    let snapshot = require_ok!(store.value_snapshot(handle), "value snapshot");
    assert_eq!(snapshot.cell_count(), 2);
    assert!(snapshot.payload_bytes() <= MAX_RANGE_PAYLOAD_BYTES);
    assert_eq!(
        snapshot.selector(),
        &json!({
            "kind": "a1",
            "worksheet": {"kind": "name", "value": "Sheet1"},
            "range": "A1:B1"
        })
    );
    assert_eq!(
        require_ok!(store.shadow_bytes(handle), "shadow bytes"),
        file_bytes
    );
    let anchor = require_ok!(store.anchor(handle), "anchor");
    assert_eq!(
        require_some!(anchor.shadow_copy(), "shadow anchor")
            .expected_digest
            .as_str(),
        BlobId::of_content(&file_bytes).as_str()
    );
    assert_eq!(
        require_ok!(store.workbook(handle), "workbook binding")
            .workbook
            .full_path,
        workbook_path()
    );
    assert_eq!(store.active_anchor_count(), 1);
    assert_eq!(store.active_shadow_bytes(), file_bytes.len());
    Ok(())
}

#[test]
fn test_prepare_write_rejects_fingerprint_drift_without_marking_prepared() -> TestResult {
    let mut store = ExcelL1AnchorStore::new();
    let handle = require_ok!(store.capture(request("anchor-1", 'a', b"xlsx")), "capture");

    let error = require_error!(
        store.prepare_write(handle, &workbook_identity(workbook_path(), 'b')),
        "fingerprint drift"
    );
    assert_eq!(error.code(), ErrorCode::VerifyFailed);
    let anchor = require_ok!(store.anchor(handle), "anchor");
    assert!(anchor.post_fingerprint().is_none());
    require_ok!(
        store.prepare_write(handle, &workbook_identity(workbook_path(), 'a')),
        "unchanged pre-fingerprint"
    );
    Ok(())
}
#[test]
fn test_prepare_write_rejects_each_identity_field_drift() -> TestResult {
    let mut session_drift = workbook_identity(workbook_path(), 'a');
    session_drift.tool_bus_session_id.push_str("-other");
    let mut task_drift = workbook_identity(workbook_path(), 'a');
    task_drift.task_id.push_str("-other");
    let mut path_drift = workbook_identity(workbook_path(), 'a');
    path_drift.workbook.full_path = std::env::temp_dir()
        .join("task-404-other.xlsx")
        .to_string_lossy()
        .into_owned();
    let mut process_drift = workbook_identity(workbook_path(), 'a');
    process_drift.owned_process.process_id += 1;
    for (index, observed) in [session_drift, task_drift, path_drift, process_drift]
        .into_iter()
        .enumerate()
    {
        let mut store = ExcelL1AnchorStore::new();
        let handle = require_ok!(
            store.capture(request(&format!("anchor-{index}"), 'a', b"xlsx")),
            "capture"
        );
        let error = require_error!(store.prepare_write(handle, &observed), "identity drift");
        assert_eq!(error.code(), ErrorCode::VerifyFailed);
    }
    Ok(())
}
#[test]
fn test_post_and_rollback_reject_identity_drift() -> TestResult {
    let mut store = ExcelL1AnchorStore::new();
    let handle = require_ok!(store.capture(request("anchor-1", 'a', b"xlsx")), "capture");
    require_ok!(
        store.prepare_write(handle, &workbook_identity(workbook_path(), 'a')),
        "prepare"
    );
    let mut post_drift = workbook_identity(workbook_path(), 'b');
    post_drift.owned_process.start_ticks += 1;
    let error = require_error!(store.record_post_write(handle, &post_drift), "post drift");
    assert_eq!(error.code(), ErrorCode::VerifyFailed);

    require_ok!(
        store.record_post_write(handle, &workbook_identity(workbook_path(), 'b')),
        "post"
    );
    let mut rollback_drift = workbook_identity(workbook_path(), 'b');
    rollback_drift.tool_bus_session_id.push_str("-other");
    let error = require_error!(
        store.rollback_readiness(handle, &rollback_drift),
        "rollback drift"
    );
    assert_eq!(error.code(), ErrorCode::VerifyFailed);
    Ok(())
}

#[test]
fn test_write_preparation_is_single_use() -> TestResult {
    let mut store = ExcelL1AnchorStore::new();
    let handle = require_ok!(store.capture(request("anchor-1", 'a', b"xlsx")), "capture");
    require_ok!(
        store.prepare_write(handle, &workbook_identity(workbook_path(), 'a')),
        "first preparation"
    );
    let error = require_error!(
        store.prepare_write(handle, &workbook_identity(workbook_path(), 'a')),
        "second preparation"
    );
    assert_eq!(error.code(), ErrorCode::VerifyFailed);
    Ok(())
}

#[test]
fn test_post_fingerprint_requires_prepared_write_and_is_single_use() -> TestResult {
    let mut store = ExcelL1AnchorStore::new();
    let handle = require_ok!(store.capture(request("anchor-1", 'a', b"xlsx")), "capture");
    let early = require_error!(
        store.record_post_write(handle, &workbook_identity(workbook_path(), 'b')),
        "post before prepare"
    );
    assert_eq!(early.code(), ErrorCode::VerifyFailed);

    require_ok!(
        store.prepare_write(handle, &workbook_identity(workbook_path(), 'a')),
        "write preparation"
    );
    require_ok!(
        store.record_post_write(handle, &workbook_identity(workbook_path(), 'b')),
        "post fingerprint"
    );
    let duplicate = require_error!(
        store.record_post_write(handle, &workbook_identity(workbook_path(), 'c')),
        "duplicate post"
    );
    assert_eq!(duplicate.code(), ErrorCode::VerifyFailed);
    Ok(())
}

#[test]
fn test_rollback_readiness_distinguishes_ready_anchor_and_user_change() -> TestResult {
    let mut store = ExcelL1AnchorStore::new();
    let handle = require_ok!(store.capture(request("anchor-1", 'a', b"xlsx")), "capture");
    require_ok!(
        store.prepare_write(handle, &workbook_identity(workbook_path(), 'a')),
        "write preparation"
    );
    require_ok!(
        store.record_post_write(handle, &workbook_identity(workbook_path(), 'b')),
        "post fingerprint"
    );

    assert_eq!(
        require_ok!(
            store.rollback_readiness(handle, &workbook_identity(workbook_path(), 'b')),
            "rollback readiness"
        ),
        ExcelL1RollbackReadiness::ReadyToRestore
    );
    assert_eq!(
        require_ok!(
            store.rollback_readiness(handle, &workbook_identity(workbook_path(), 'a')),
            "already at anchor"
        ),
        ExcelL1RollbackReadiness::AlreadyAtAnchor
    );
    let changed = require_error!(
        store.rollback_readiness(handle, &workbook_identity(workbook_path(), 'c')),
        "user change"
    );
    assert_eq!(changed.code(), ErrorCode::VerifyFailed);
    Ok(())
}

#[test]
fn test_rollback_without_post_fingerprint_reports_missing_evidence() -> TestResult {
    let mut store = ExcelL1AnchorStore::new();
    let handle = require_ok!(store.capture(request("anchor-1", 'a', b"xlsx")), "capture");
    let error = require_error!(
        store.rollback_readiness(handle, &workbook_identity(workbook_path(), 'a')),
        "missing post"
    );
    assert_eq!(error.code(), ErrorCode::VerifyFailed);
    Ok(())
}

#[test]
fn test_capture_rejects_formula_and_text_modes() -> TestResult {
    for mode in [ReadMode::Formula, ReadMode::Text] {
        let mut store = ExcelL1AnchorStore::new();
        let mut invalid = request("anchor-1", 'a', b"xlsx");
        invalid.mode = mode;
        let error = require_error!(store.capture(invalid), "non-value2 mode");
        assert_eq!(error.code(), ErrorCode::ToolInvalidArgs);
        assert_eq!(store.active_anchor_count(), 0);
    }
    Ok(())
}

#[test]
fn test_capture_rejects_partial_duplicate_or_out_of_bounds_cells() -> TestResult {
    let cases = [
        vec![cell(1, 1, CellValueType::Number, "5")],
        vec![
            cell(1, 1, CellValueType::Number, "5"),
            cell(1, 1, CellValueType::Text, "duplicate"),
        ],
        vec![
            cell(1, 1, CellValueType::Number, "5"),
            cell(2, 1, CellValueType::Text, "outside"),
        ],
    ];
    for cells in cases {
        let mut store = ExcelL1AnchorStore::new();
        let mut invalid = request("anchor-1", 'a', b"xlsx");
        invalid.snapshot.cells = cells;
        let error = require_error!(store.capture(invalid), "invalid cell set");
        assert_eq!(error.code(), ErrorCode::UserInteraction);
        assert_eq!(store.active_anchor_count(), 0);
    }
    Ok(())
}

#[test]
fn test_capture_rejects_noncanonical_boolean_and_unknown_error() -> TestResult {
    for (value_type, value_text) in [
        (CellValueType::Boolean, "TRUE"),
        (CellValueType::Error, "#NOT_REAL!"),
    ] {
        let mut store = ExcelL1AnchorStore::new();
        let mut invalid = request("anchor-1", 'a', b"xlsx");
        *invalid
            .snapshot
            .cells
            .get_mut(0)
            .ok_or_else(|| "test request lost its first cell".to_owned())? =
            cell(1, 1, value_type, value_text);
        let error = require_error!(store.capture(invalid), "invalid cell value");
        assert_eq!(error.code(), ErrorCode::UserInteraction);
    }
    Ok(())
}

#[test]
fn test_capture_rejects_a1_drift_and_non_positive_worksheet_ordinal() -> TestResult {
    let mut store = ExcelL1AnchorStore::new();
    let mut drift = request("anchor-1", 'a', b"xlsx");
    drift.snapshot.address = "Sheet1!A1:C1".to_owned();
    drift.snapshot.columns = 3;
    drift
        .snapshot
        .cells
        .push(cell(1, 3, CellValueType::Text, "extra"));
    let error = require_error!(store.capture(drift), "A1 drift");
    assert_eq!(error.code(), ErrorCode::UserInteraction);

    let mut ordinal = request("anchor-2", 'a', b"xlsx");
    ordinal.selector = RangeSelector::A1 {
        worksheet: WorksheetSelector::Ordinal(0),
        start: super::super::A1Cell { row: 1, column: 1 },
        end: super::super::A1Cell { row: 1, column: 2 },
    };
    let error = require_error!(store.capture(ordinal), "zero ordinal");
    assert_eq!(error.code(), ErrorCode::ToolInvalidArgs);
    Ok(())
}
#[test]
fn test_capture_maps_relative_cells_to_absolute_range_origin() -> TestResult {
    let mut store = ExcelL1AnchorStore::new();
    let mut request = request("anchor-1", 'a', b"xlsx");
    request.selector = RangeSelector::A1 {
        worksheet: WorksheetSelector::Name("Sheet1".to_owned()),
        start: super::super::A1Cell { row: 5, column: 3 },
        end: super::super::A1Cell { row: 6, column: 4 },
    };
    request.snapshot = RangeSnapshot {
        address: "C5:D6".to_owned(),
        rows: 2,
        columns: 2,
        cells: vec![
            cell(1, 1, CellValueType::Text, "top-left"),
            cell(1, 2, CellValueType::Blank, ""),
            cell(2, 1, CellValueType::Boolean, "true"),
            cell(2, 2, CellValueType::Number, "42"),
        ],
    };
    let handle = require_ok!(store.capture(request), "capture");
    let snapshot = require_ok!(store.value_snapshot(handle), "snapshot");
    assert_eq!((snapshot.cells().len()), 4);
    let first = snapshot
        .cells()
        .first()
        .ok_or_else(|| "first cell missing".to_owned())?;
    let second = snapshot
        .cells()
        .get(3)
        .ok_or_else(|| "bottom-right cell missing".to_owned())?;
    assert_eq!((first.row, first.column), (5, 3));
    assert_eq!((second.row, second.column), (6, 4));
    Ok(())
}
#[test]
fn test_capture_rejects_worksheet_and_dimension_address_drift() -> TestResult {
    let mut store = ExcelL1AnchorStore::new();
    let mut worksheet = request("anchor-1", 'a', b"xlsx");
    worksheet.snapshot.address = "Sheet2!A1:B1".to_owned();
    let error = require_error!(store.capture(worksheet), "worksheet drift");
    assert_eq!(error.code(), ErrorCode::TargetNotFound);

    let mut dimensions = request("anchor-2", 'a', b"xlsx");
    dimensions.snapshot.address = "Sheet1!A1:C1".to_owned();
    let error = require_error!(store.capture(dimensions), "dimension drift");
    assert_eq!(error.code(), ErrorCode::UserInteraction);
    Ok(())
}
#[test]
fn test_capture_rejects_inverted_a1_selector_without_panicking() -> TestResult {
    let mut store = ExcelL1AnchorStore::new();
    let mut request = request("anchor-1", 'a', b"xlsx");
    request.selector = RangeSelector::A1 {
        worksheet: WorksheetSelector::Name("Sheet1".to_owned()),
        start: super::super::A1Cell { row: 1, column: 2 },
        end: super::super::A1Cell { row: 1, column: 1 },
    };
    let error = require_error!(store.capture(request), "inverted selector");
    assert_eq!(error.code(), ErrorCode::ToolInvalidArgs);
    Ok(())
}
#[test]
fn test_capture_rejects_excess_cell_vector_before_normalization() -> TestResult {
    let mut store = ExcelL1AnchorStore::new();
    let mut request = request("anchor-1", 'a', b"xlsx");
    request.snapshot.cells = vec![cell(1, 1, CellValueType::Text, "x"); 4097];
    let error = require_error!(store.capture(request), "excess cells");
    assert_eq!(error.code(), ErrorCode::UserInteraction);
    Ok(())
}

#[test]
fn test_capture_rejects_invalid_workbook_path_and_fingerprint() -> TestResult {
    let mut store = ExcelL1AnchorStore::new();
    let mut invalid_path = request("anchor-1", 'a', b"xlsx");
    invalid_path.workbook.workbook.full_path = "\\\\server\\share\\book.xlsx".to_owned();
    let error = require_error!(store.capture(invalid_path), "UNC path");
    assert_eq!(error.code(), ErrorCode::ToolInvalidArgs);

    let mut invalid_fingerprint = request("anchor-2", 'a', b"xlsx");
    invalid_fingerprint.workbook.fingerprint = "not-a-fingerprint".to_owned();
    let error = require_error!(store.capture(invalid_fingerprint), "fingerprint");
    assert_eq!(error.code(), ErrorCode::ToolInvalidArgs);
    Ok(())
}
#[test]
fn test_capture_rejects_invalid_opened_worksheet_list() -> TestResult {
    let mut store = ExcelL1AnchorStore::new();
    let mut invalid = request("anchor-1", 'a', b"xlsx");
    invalid.workbook.worksheets = vec![
        WorksheetInfo {
            name: "Sheet1".to_owned(),
            ordinal: 1,
            used_range_address: None,
        },
        WorksheetInfo {
            name: "Sheet1".to_owned(),
            ordinal: 1,
            used_range_address: None,
        },
    ];
    let error = require_error!(store.capture(invalid), "invalid worksheet list");
    assert_eq!(error.code(), ErrorCode::ToolInvalidArgs);
    Ok(())
}

#[test]
fn test_capture_rejects_empty_shadow_bytes_and_payload_over_256_kib() -> TestResult {
    let mut store = ExcelL1AnchorStore::new();
    let error = require_error!(
        store.capture(request("anchor-1", 'a', b"")),
        "empty shadow bytes"
    );
    assert_eq!(error.code(), ErrorCode::ToolInvalidArgs);

    let mut oversized = request("anchor-2", 'a', b"xlsx");
    let second = oversized
        .snapshot
        .cells
        .get_mut(1)
        .ok_or_else(|| "test request lost its second cell".to_owned())?;
    second.value_text = "x".repeat(MAX_RANGE_PAYLOAD_BYTES);
    let error = require_error!(store.capture(oversized), "payload over bound");
    assert_eq!(error.code(), ErrorCode::ToolInvalidArgs);
    assert_eq!(store.active_anchor_count(), 0);
    Ok(())
}

#[test]
fn test_capture_rejects_more_than_4096_cells() -> TestResult {
    let mut store = ExcelL1AnchorStore::new();
    let mut invalid = request("anchor-1", 'a', b"xlsx");
    invalid.selector = RangeSelector::NamedRange("LargeRange".to_owned());
    invalid.snapshot.address = "Sheet1!A1:AAAA1".to_owned();
    invalid.snapshot.rows = 1;
    invalid.snapshot.columns = 4097;
    invalid.snapshot.cells.clear();
    let error = require_error!(store.capture(invalid), "cell bound");
    assert_eq!(error.code(), ErrorCode::ToolInvalidArgs);
    Ok(())
}

#[test]
fn test_shadow_digest_changes_with_file_bytes() -> TestResult {
    let mut store = ExcelL1AnchorStore::new();
    let first = require_ok!(
        store.capture(request("anchor-1", 'a', b"first")),
        "first capture"
    );
    let second = require_ok!(
        store.capture(request("anchor-2", 'a', b"second")),
        "second capture"
    );
    let first_anchor = require_ok!(store.anchor(first), "first anchor");
    let second_anchor = require_ok!(store.anchor(second), "second anchor");
    let first_digest = require_some!(first_anchor.shadow_copy(), "first shadow")
        .expected_digest
        .clone();
    let second_digest = require_some!(second_anchor.shadow_copy(), "second shadow")
        .expected_digest
        .clone();
    assert_ne!(first_digest, second_digest);
    Ok(())
}

#[test]
fn test_release_removes_anchor_and_shadow_bytes_and_rejects_duplicate_release() -> TestResult {
    let mut store = ExcelL1AnchorStore::new();
    let handle = require_ok!(store.capture(request("anchor-1", 'a', b"12345")), "capture");
    require_ok!(store.release(handle), "release");
    assert_eq!(store.active_anchor_count(), 0);
    assert_eq!(store.active_shadow_bytes(), 0);
    let error = require_error!(store.release(handle), "duplicate release");
    assert_eq!(error.code(), ErrorCode::TargetNotFound);
    Ok(())
}

#[test]
fn test_shutdown_releases_every_anchor_and_resets_bytes() -> TestResult {
    let mut store = ExcelL1AnchorStore::new();
    for index in 0..3 {
        require_ok!(
            store.capture(request(&format!("anchor-{index}"), 'a', b"xlsx")),
            "capture"
        );
    }
    assert_eq!(store.active_anchor_count(), 3);
    store.shutdown();
    assert_eq!(store.active_anchor_count(), 0);
    assert_eq!(store.active_shadow_bytes(), 0);
    Ok(())
}

#[test]
fn test_anchor_count_is_bounded_without_silent_eviction() -> TestResult {
    let mut store = ExcelL1AnchorStore::new();
    for index in 0..MAX_EXCEL_L1_ANCHORS {
        require_ok!(
            store.capture(single_cell_request(&format!("anchor-{index}"), b"xlsx")),
            "capture within bound"
        );
    }
    let error = require_error!(
        store.capture(single_cell_request("anchor-overflow", b"xlsx")),
        "capacity"
    );
    assert_eq!(error.code(), ErrorCode::UserInteraction);
    assert_eq!(store.active_anchor_count(), MAX_EXCEL_L1_ANCHORS);
    Ok(())
}

#[test]
fn test_shadow_byte_total_is_bounded() -> TestResult {
    let mut store = ExcelL1AnchorStore::new();
    let oversized = vec![0_u8; MAX_EXCEL_L1_SHADOW_BYTES + 1];
    let error = require_error!(
        store.capture(single_cell_request("anchor-overflow", &oversized)),
        "shadow total"
    );
    assert_eq!(error.code(), ErrorCode::UserInteraction);
    assert_eq!(store.active_anchor_count(), 0);
    assert_eq!(store.active_shadow_bytes(), 0);
    Ok(())
}

#[test]
fn test_repeated_capture_and_release_keeps_mapping_and_byte_count_bounded() -> TestResult {
    let mut store = ExcelL1AnchorStore::new();
    for index in 0..128 {
        let handle = require_ok!(
            store.capture(single_cell_request(&format!("anchor-{index}"), b"xlsx")),
            "capture"
        );
        assert_eq!(store.active_anchor_count(), 1);
        assert_eq!(store.active_shadow_bytes(), 4);
        require_ok!(store.release(handle), "release");
        assert_eq!(store.active_anchor_count(), 0);
        assert_eq!(store.active_shadow_bytes(), 0);
    }
    Ok(())
}
