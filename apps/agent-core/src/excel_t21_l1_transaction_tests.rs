//! Contract tests for the test-only Excel T2.2 scalar file-transaction coordinator.
//!
//! These tests use synthetic `.xlsx` bytes in task-owned temporary directories. They do not
//! start Excel, touch COM, operate a GUI, save a user workbook, or reach production assembly.

use super::*;
use crate::excel_t21::{
    A1Cell, OpenedWorkbook, OwnedProcessIdentity, WorkbookHandle, WorkbookIdentity, WorksheetInfo,
};
use assistant_storage::BlobId;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

type TestResult<T = ()> = Result<T, String>;
type PreparedTransaction = (
    TestRoot,
    ExcelL1AnchorStore,
    ExcelL1FileShadowStore,
    FakePort,
    ExcelL1ScalarFileWriteReceipt,
    Vec<u8>,
    PathBuf,
);

static NEXT_TEST_ROOT: AtomicU64 = AtomicU64::new(1);

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

struct TestRoot {
    path: PathBuf,
}

impl TestRoot {
    fn create(label: &str) -> TestResult<Self> {
        let sequence = NEXT_TEST_ROOT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "assistant-task415-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&path)
            .map_err(|error| format!("create test root {}: {error}", path.display()))?;
        Ok(Self { path })
    }

    fn shadow_root(&self) -> PathBuf {
        self.path.join("shadows")
    }

    fn workbook_path(&self) -> PathBuf {
        self.path.join("book.xlsx")
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            assert!(
                fs::remove_dir_all(&self.path).is_ok(),
                "failed to remove test root {}",
                self.path.display()
            );
        }
    }
}

fn fingerprint(character: char) -> String {
    format!("sha256:{}", character.to_string().repeat(64))
}

fn content_digest(bytes: &[u8]) -> TestResult<ContentDigest> {
    ContentDigest::parse(BlobId::of_content(bytes).as_str())
        .map_err(|error| format!("parse content digest: {error}"))
}

fn workbook_identity(path: &Path, fingerprint_character: char) -> ExcelL1WorkbookIdentity {
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
        handle: WorkbookHandle::new(415),
        owned_process: OwnedProcessIdentity {
            process_id: 415,
            start_ticks: 415,
            executable_path,
        },
        workbook: WorkbookIdentity {
            full_path: path.to_string_lossy().into_owned(),
            read_only: false,
        },
        worksheets: vec![WorksheetInfo {
            name: "Sheet1".to_owned(),
            ordinal: 1,
            used_range_address: Some("Sheet1!A1".to_owned()),
        }],
    };
    ExcelL1WorkbookIdentity::from_opened(
        "toolbus-session-415".to_owned(),
        "task-415".to_owned(),
        &opened,
        fingerprint(fingerprint_character),
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

fn capture_request(label: &str, workbook_path: &Path) -> ExcelL1CaptureRequest {
    ExcelL1CaptureRequest {
        anchor_id: format!("anchor-{label}"),
        step_id: format!("step-{label}"),
        workbook: workbook_identity(workbook_path, 'a'),
        selector: single_cell_selector(),
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
        file_bytes: Vec::new(),
    }
}

fn write_bytes(path: &Path, bytes: &[u8]) -> TestResult {
    fs::write(path, bytes).map_err(|error| format!("write {}: {error}", path.display()))
}

fn read_bytes(path: &Path) -> TestResult<Vec<u8>> {
    fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))
}

fn physical_shadow_path(
    store: &ExcelL1AnchorStore,
    handle: ExcelL1AnchorHandle,
    file_store: &ExcelL1FileShadowStore,
) -> TestResult<PathBuf> {
    let shadow_copy = require_ok!(store.anchor(handle), "anchor")
        .shadow_copy()
        .ok_or_else(|| "anchor has no shadow copy".to_owned())?;
    file_store
        .physical_path(shadow_copy)
        .map_err(|error| error.to_string())
}

#[derive(Debug, Default)]
struct FakePort {
    calls: Vec<&'static str>,
    fail_write: Option<ErrorCode>,
    post_fingerprint: Option<String>,
}

impl ExcelL1WritePort for FakePort {
    fn write_scalar(
        &mut self,
        _workbook: &ExcelL1WorkbookIdentity,
        _selector: &RangeSelector,
        _literal: &ExcelL1ScalarLiteral,
    ) -> Result<(), ExcelT21Error> {
        self.calls.push("write");
        self.fail_write.map_or(Ok(()), |code| {
            Err(ExcelT21Error::new(
                code,
                "scripted transaction write failure",
            ))
        })
    }

    fn recalculate(
        &mut self,
        _workbook: &ExcelL1WorkbookIdentity,
        _selector: &RangeSelector,
    ) -> Result<(), ExcelT21Error> {
        self.calls.push("recalculate");
        Ok(())
    }

    fn read_range(
        &mut self,
        _workbook: &ExcelL1WorkbookIdentity,
        selector: &RangeSelector,
        mode: ReadMode,
    ) -> Result<ExcelL1RangeReadback, ExcelT21Error> {
        self.calls.push(match mode {
            ReadMode::Value2 => "read:value2",
            ReadMode::Formula => "read:formula",
            ReadMode::Text => "read:text",
        });
        let (value_type, value_text) = match mode {
            ReadMode::Value2 => (CellValueType::Number, "2".to_owned()),
            ReadMode::Formula | ReadMode::Text => (CellValueType::Text, "2".to_owned()),
        };
        Ok(ExcelL1RangeReadback {
            mode,
            snapshot: fake_snapshot(selector, value_type, value_text),
        })
    }

    fn fingerprint(
        &mut self,
        _workbook: &ExcelL1WorkbookIdentity,
    ) -> Result<String, ExcelT21Error> {
        self.calls.push("fingerprint");
        Ok(self
            .post_fingerprint
            .clone()
            .unwrap_or_else(|| fingerprint('b')))
    }
}

fn fake_snapshot(
    selector: &RangeSelector,
    value_type: CellValueType,
    value_text: String,
) -> RangeSnapshot {
    let address = match selector {
        RangeSelector::NamedRange(_) => "Sheet1!A1".to_owned(),
        RangeSelector::A1 { start, end, .. } => {
            let start_address =
                cell_address(start.row, start.column).unwrap_or_else(|_| "A1".to_owned());
            let end_address = cell_address(end.row, end.column).unwrap_or_else(|_| "A1".to_owned());
            if start_address == end_address {
                format!("Sheet1!{start_address}")
            } else {
                format!("Sheet1!{start_address}:{end_address}")
            }
        }
    };
    RangeSnapshot {
        address,
        rows: 1,
        columns: 1,
        cells: vec![RuntimeCell {
            row: 1,
            column: 1,
            value_type,
            value_text,
        }],
    }
}

fn prepared(label: &str) -> TestResult<PreparedTransaction> {
    let root = TestRoot::create(label)?;
    let workbook_path = root.workbook_path();
    let original = b"task-415-original-xlsx-bytes".to_vec();
    write_bytes(&workbook_path, &original)?;
    let mut anchors = ExcelL1AnchorStore::new();
    let mut file_store = require_ok!(
        ExcelL1FileShadowStore::open(root.shadow_root(), 1024),
        "open file shadow store"
    );
    let mut port = FakePort::default();
    let receipt = require_ok!(
        execute_scalar_write_with_file_shadow(
            &mut anchors,
            &mut file_store,
            capture_request(label, &workbook_path),
            "2",
            &mut port,
        ),
        "execute transaction"
    );
    Ok((
        root,
        anchors,
        file_store,
        port,
        receipt,
        original,
        workbook_path,
    ))
}

#[test]
fn test_scalar_file_transaction_invalid_scalar_precedes_capture() -> TestResult {
    let root = TestRoot::create("invalid-before-capture")?;
    let workbook_path = root.workbook_path();
    write_bytes(&workbook_path, b"original")?;
    let mut anchors = ExcelL1AnchorStore::new();
    let mut file_store = require_ok!(
        ExcelL1FileShadowStore::open(root.shadow_root(), 1024),
        "open file shadow store"
    );
    let mut port = FakePort::default();

    let error = require_error!(
        execute_scalar_write_with_file_shadow(
            &mut anchors,
            &mut file_store,
            capture_request("invalid", &workbook_path),
            "[1]",
            &mut port,
        ),
        "invalid scalar"
    );
    assert_eq!(error.code(), ErrorCode::ToolInvalidArgs);
    assert_eq!(anchors.active_anchor_count(), 0);
    assert!(port.calls.is_empty());
    assert!(!root.shadow_root().join("excel-l1").exists());
    file_store
        .cleanup()
        .map_err(|error| format!("cleanup file shadow: {error}"))?;
    Ok(())
}

#[test]
fn test_scalar_file_transaction_capture_publish_failure_converges() -> TestResult {
    let root = TestRoot::create("capture-publish-failure")?;
    let workbook_path = root.workbook_path();
    write_bytes(&workbook_path, b"task-415-source-exceeds-budget")?;
    let mut anchors = ExcelL1AnchorStore::new();
    let mut file_store = require_ok!(
        ExcelL1FileShadowStore::open(root.shadow_root(), 1),
        "open bounded file shadow store"
    );
    let mut port = FakePort::default();
    let error = require_error!(
        execute_scalar_write_with_file_shadow(
            &mut anchors,
            &mut file_store,
            capture_request("publish-failure", &workbook_path),
            "2",
            &mut port,
        ),
        "capture publish failure"
    );
    assert_eq!(error.code(), ErrorCode::UserInteraction);
    assert_eq!(anchors.active_anchor_count(), 0);
    assert_eq!(anchors.active_shadow_bytes(), 0);
    assert!(port.calls.is_empty());
    file_store
        .cleanup()
        .map_err(|error| format!("cleanup file shadow: {error}"))?;
    assert!(!root.shadow_root().exists());
    Ok(())
}

#[test]
fn test_scalar_file_transaction_success_then_verified_rollback() -> TestResult {
    let (root, mut anchors, mut file_store, port, receipt, original, workbook_path) =
        prepared("success-rollback")?;
    let handle = receipt.anchor_handle();
    let shadow_path = physical_shadow_path(&anchors, handle, &file_store)?;
    assert_eq!(anchors.active_anchor_count(), 1);
    assert_eq!(
        port.calls,
        vec![
            "write",
            "recalculate",
            "read:value2",
            "read:formula",
            "fingerprint"
        ]
    );
    assert_eq!(receipt.original_file_digest(), &content_digest(&original)?);
    assert_eq!(receipt.write_receipt().post_fingerprint(), fingerprint('b'));

    let saved = b"task-415-agent-saved-xlsx-bytes";
    write_bytes(&workbook_path, saved)?;
    let saved_digest = content_digest(saved)?;
    let outcome = require_ok!(
        rollback_scalar_write_with_file_shadow(&mut anchors, &file_store, &receipt, &saved_digest,),
        "rollback"
    );
    assert!(matches!(outcome, ExcelL1FileRestoreOutcome::Restored(_)));
    assert_eq!(read_bytes(&workbook_path)?, original);
    assert_eq!(anchors.active_anchor_count(), 0);
    assert!(shadow_path.exists());

    file_store
        .cleanup()
        .map_err(|error| format!("cleanup file shadow: {error}"))?;
    assert!(!root.shadow_root().exists());
    Ok(())
}

#[test]
fn test_scalar_file_transaction_duplicate_rollback_fails_after_anchor_release() -> TestResult {
    let (_root, mut anchors, mut file_store, _port, receipt, original, workbook_path) =
        prepared("duplicate-rollback")?;
    let saved = b"task-415-saved-once";
    write_bytes(&workbook_path, saved)?;
    let saved_digest = content_digest(saved)?;
    require_ok!(
        rollback_scalar_write_with_file_shadow(&mut anchors, &file_store, &receipt, &saved_digest,),
        "first rollback"
    );
    let error = require_error!(
        rollback_scalar_write_with_file_shadow(&mut anchors, &file_store, &receipt, &saved_digest,),
        "duplicate rollback"
    );
    assert_eq!(error.code(), ErrorCode::TargetNotFound);
    assert_eq!(read_bytes(&workbook_path)?, original);
    assert_eq!(anchors.active_anchor_count(), 0);
    file_store
        .cleanup()
        .map_err(|error| format!("cleanup file shadow: {error}"))?;
    Ok(())
}

#[test]
fn test_scalar_file_transaction_user_drift_preserves_target_and_anchor() -> TestResult {
    let (_root, mut anchors, mut file_store, _port, receipt, _original, workbook_path) =
        prepared("user-drift")?;
    let shadow_path = physical_shadow_path(&anchors, receipt.anchor_handle(), &file_store)?;
    let saved = b"task-415-saved-before-user-edit";
    let user_change = b"task-415-user-changed-after-save";
    write_bytes(&workbook_path, saved)?;
    let saved_digest = content_digest(saved)?;
    write_bytes(&workbook_path, user_change)?;

    let error = require_error!(
        rollback_scalar_write_with_file_shadow(&mut anchors, &file_store, &receipt, &saved_digest,),
        "user drift"
    );
    assert_eq!(error.code(), ErrorCode::VerifyFailed);
    assert_eq!(read_bytes(&workbook_path)?, user_change);
    assert_eq!(anchors.active_anchor_count(), 1);
    assert!(shadow_path.exists());
    anchors.shutdown();
    file_store
        .cleanup()
        .map_err(|error| format!("cleanup file shadow: {error}"))?;
    Ok(())
}

#[test]
fn test_scalar_file_transaction_write_failure_retains_bounded_evidence() -> TestResult {
    let root = TestRoot::create("write-failure")?;
    let workbook_path = root.workbook_path();
    write_bytes(&workbook_path, b"task-415-original")?;
    let mut anchors = ExcelL1AnchorStore::new();
    let mut file_store = require_ok!(
        ExcelL1FileShadowStore::open(root.shadow_root(), 1024),
        "open file shadow store"
    );
    let mut port = FakePort {
        fail_write: Some(ErrorCode::Fatal),
        ..FakePort::default()
    };
    let error = require_error!(
        execute_scalar_write_with_file_shadow(
            &mut anchors,
            &mut file_store,
            capture_request("write-failure", &workbook_path),
            "2",
            &mut port,
        ),
        "write failure"
    );
    assert_eq!(error.code(), ErrorCode::Fatal);
    assert_eq!(anchors.active_anchor_count(), 1);
    assert_eq!(port.calls, vec!["write"]);
    let handle = ExcelL1AnchorHandle(1);
    let shadow_path = physical_shadow_path(&anchors, handle, &file_store)?;
    assert!(shadow_path.exists());

    anchors.shutdown();
    file_store
        .cleanup()
        .map_err(|error| format!("cleanup file shadow: {error}"))?;
    assert_eq!(anchors.active_anchor_count(), 0);
    assert!(!root.shadow_root().exists());
    Ok(())
}

#[test]
fn test_scalar_file_transaction_missing_post_fingerprint_is_explicit_failure() -> TestResult {
    let root = TestRoot::create("missing-post-fingerprint")?;
    let workbook_path = root.workbook_path();
    write_bytes(&workbook_path, b"task-415-original")?;
    let mut anchors = ExcelL1AnchorStore::new();
    let mut file_store = require_ok!(
        ExcelL1FileShadowStore::open(root.shadow_root(), 1024),
        "open file shadow store"
    );
    let mut port = FakePort {
        post_fingerprint: Some(fingerprint('a')),
        ..FakePort::default()
    };
    let error = require_error!(
        execute_scalar_write_with_file_shadow(
            &mut anchors,
            &mut file_store,
            capture_request("same-fingerprint", &workbook_path),
            "2",
            &mut port,
        ),
        "unchanged fingerprint"
    );
    assert_eq!(error.code(), ErrorCode::VerifyFailed);
    assert_eq!(anchors.active_anchor_count(), 1);
    let handle = ExcelL1AnchorHandle(1);
    assert_eq!(
        require_ok!(anchors.anchor(handle), "anchor").post_fingerprint(),
        None
    );
    anchors.shutdown();
    file_store
        .cleanup()
        .map_err(|error| format!("cleanup file shadow: {error}"))?;
    Ok(())
}
