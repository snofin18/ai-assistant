//! Filesystem tests for the test-only Excel L1 shadow-copy port.
//!
//! Every test owns a unique temporary tree outside the repository. Synthetic
//! `.xlsx` bytes stand in for a workbook; no Excel or COM object is started.

use super::super::super::A1Cell;
use super::super::{
    CellValueType, ExcelL1CaptureRequest, ExcelL1WorkbookIdentity, OpenedWorkbook,
    OwnedProcessIdentity, RangeSelector, RangeSnapshot, ReadMode, RuntimeCell, WorkbookHandle,
    WorkbookIdentity, WorksheetInfo, WorksheetSelector,
};
use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

type TestResult<T = ()> = Result<T, String>;

static NEXT_TEST_ROOT: AtomicU64 = AtomicU64::new(1);

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
            "assistant-task414-{label}-{}-{sequence}",
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
        handle: WorkbookHandle::new(1),
        owned_process: OwnedProcessIdentity {
            process_id: 414,
            start_ticks: 414,
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
        "toolbus-session-414".to_owned(),
        "task-414".to_owned(),
        &opened,
        fingerprint(fingerprint_character),
    )
}

fn request(label: &str, workbook_path: &Path, bytes: &[u8]) -> ExcelL1CaptureRequest {
    ExcelL1CaptureRequest {
        anchor_id: format!("anchor-{label}"),
        step_id: format!("step-{label}"),
        workbook: workbook_identity(workbook_path, 'a'),
        selector: RangeSelector::A1 {
            worksheet: WorksheetSelector::Name("Sheet1".to_owned()),
            start: A1Cell { row: 1, column: 1 },
            end: A1Cell { row: 1, column: 1 },
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
        file_bytes: bytes.to_vec(),
    }
}

fn shadow_copy(relative_path: &str, bytes: &[u8]) -> TestResult<ShadowCopy> {
    let path = ShadowCopyPath::parse(relative_path)
        .map_err(|error| format!("parse shadow path: {error}"))?;
    let digest = content_digest(bytes).map_err(|error| error.to_string())?;
    Ok(ShadowCopy::new(path, digest))
}

fn write_bytes(path: &Path, bytes: &[u8]) -> TestResult {
    fs::write(path, bytes).map_err(|error| format!("write {}: {error}", path.display()))
}

fn assert_no_temporary_artifacts(root: &Path) -> TestResult {
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        if !directory.exists() {
            continue;
        }
        for entry in fs::read_dir(&directory).map_err(|error| error.to_string())? {
            let path = entry.map_err(|error| error.to_string())?.path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            let extension = path
                .extension()
                .and_then(|extension| extension.to_str())
                .unwrap_or_default();
            assert!(
                !extension.eq_ignore_ascii_case("tmp")
                    && !extension.eq_ignore_ascii_case("restore-tmp")
                    && !extension.eq_ignore_ascii_case("restore-backup"),
                "temporary artifact remained: {}",
                path.display()
            );
        }
    }
    Ok(())
}

#[test]
fn test_file_shadow_capture_restore_and_cleanup() -> TestResult {
    let root = TestRoot::create("capture-restore")?;
    let workbook_path = root.workbook_path();
    let original = b"original-xlsx-bytes";
    let written = b"written-xlsx-bytes";
    write_bytes(&workbook_path, original)?;
    let mut store = ExcelL1FileShadowStore::open(root.shadow_root(), 1024)
        .map_err(|error| error.to_string())?;
    let (canonical_source, source_bytes) =
        ExcelL1FileShadowStore::read_source(&workbook_path).map_err(|error| error.to_string())?;
    let metadata = shadow_copy("excel-l1/1/original.shadow", original)?;
    let capture = store
        .publish_bytes(canonical_source, &metadata, &source_bytes)
        .map_err(|error| error.to_string())?;
    let physical_path = store
        .physical_path(&metadata)
        .map_err(|error| error.to_string())?;
    assert!(physical_path.starts_with(root.shadow_root()));
    assert_eq!(capture.byte_count, original.len());
    assert_eq!(capture.shadow_copy, metadata);
    assert_eq!(store.captures.len(), 1);
    assert_eq!(store.active_bytes, original.len());

    write_bytes(&workbook_path, written)?;
    let written_digest = content_digest(written).map_err(|error| error.to_string())?;
    let receipt = store
        .restore(&metadata, &written_digest)
        .map_err(|error| error.to_string())?;
    assert_eq!(receipt.byte_count, original.len());
    assert_eq!(receipt.restored_digest, metadata.expected_digest);
    assert_eq!(
        fs::read(&workbook_path).map_err(|error| error.to_string())?,
        original
    );
    assert!(physical_path.exists());
    assert_no_temporary_artifacts(&root.path)?;

    store.cleanup().map_err(|error| error.to_string())?;
    assert!(!root.shadow_root().exists());
    assert_eq!(store.captures.len(), 0);
    assert_eq!(store.active_bytes, 0);
    Ok(())
}

#[test]
fn test_file_shadow_capture_refuses_existing_destination() -> TestResult {
    let root = TestRoot::create("existing-destination")?;
    let workbook_path = root.workbook_path();
    let original = b"original-xlsx-bytes";
    write_bytes(&workbook_path, original)?;
    let mut store = ExcelL1FileShadowStore::open(root.shadow_root(), 1024)
        .map_err(|error| error.to_string())?;
    let (canonical_source, source_bytes) =
        ExcelL1FileShadowStore::read_source(&workbook_path).map_err(|error| error.to_string())?;
    let metadata = shadow_copy("excel-l1/1/existing.shadow", original)?;
    let destination = root.shadow_root().join("excel-l1/1/existing.shadow");
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    write_bytes(&destination, b"already-here")?;

    let error = require_error!(
        store.publish_bytes(canonical_source, &metadata, &source_bytes),
        "existing shadow"
    );
    assert_eq!(error.code(), ErrorCode::UserInteraction);
    assert_eq!(
        fs::read(&destination).map_err(|error| error.to_string())?,
        b"already-here"
    );
    assert_no_temporary_artifacts(&root.path)?;
    assert_eq!(store.captures.len(), 0);
    assert_eq!(store.active_bytes, 0);
    Ok(())
}

#[test]
fn test_file_shadow_restore_rejects_target_drift_without_writing() -> TestResult {
    let root = TestRoot::create("target-drift")?;
    let workbook_path = root.workbook_path();
    let original = b"original-xlsx-bytes";
    let written = b"agent-written-xlsx";
    let user_change = b"user-changed-xlsx";
    write_bytes(&workbook_path, original)?;
    let mut store = ExcelL1FileShadowStore::open(root.shadow_root(), 1024)
        .map_err(|error| error.to_string())?;
    let (canonical_source, source_bytes) =
        ExcelL1FileShadowStore::read_source(&workbook_path).map_err(|error| error.to_string())?;
    let metadata = shadow_copy("excel-l1/1/drift.shadow", original)?;
    store
        .publish_bytes(canonical_source, &metadata, &source_bytes)
        .map_err(|error| error.to_string())?;
    write_bytes(&workbook_path, user_change)?;
    let expected_post_digest = content_digest(written).map_err(|error| error.to_string())?;

    let error = require_error!(
        store.restore(&metadata, &expected_post_digest),
        "user drift"
    );
    assert_eq!(error.code(), ErrorCode::VerifyFailed);
    assert_eq!(
        fs::read(&workbook_path).map_err(|error| error.to_string())?,
        user_change
    );
    assert_eq!(
        fs::read(
            store
                .physical_path(&metadata)
                .map_err(|error| error.to_string())?
        )
        .map_err(|error| error.to_string())?,
        original
    );
    assert_eq!(store.captures.len(), 1);
    Ok(())
}

#[test]
fn test_file_shadow_restore_rejects_tampered_shadow_and_preserves_target() -> TestResult {
    let root = TestRoot::create("shadow-drift")?;
    let workbook_path = root.workbook_path();
    let original = b"original-xlsx-bytes";
    let written = b"agent-written-xlsx";
    write_bytes(&workbook_path, original)?;
    let mut store = ExcelL1FileShadowStore::open(root.shadow_root(), 1024)
        .map_err(|error| error.to_string())?;
    let (canonical_source, source_bytes) =
        ExcelL1FileShadowStore::read_source(&workbook_path).map_err(|error| error.to_string())?;
    let metadata = shadow_copy("excel-l1/1/tampered.shadow", original)?;
    store
        .publish_bytes(canonical_source, &metadata, &source_bytes)
        .map_err(|error| error.to_string())?;
    let physical_path = store
        .physical_path(&metadata)
        .map_err(|error| error.to_string())?;
    write_bytes(&physical_path, b"tampered-shadow")?;
    write_bytes(&workbook_path, written)?;
    let written_digest = content_digest(written).map_err(|error| error.to_string())?;

    let error = require_error!(store.restore(&metadata, &written_digest), "tampered shadow");
    assert_eq!(error.code(), ErrorCode::VerifyFailed);
    assert_eq!(
        fs::read(&workbook_path).map_err(|error| error.to_string())?,
        written
    );
    assert_eq!(
        fs::read(&physical_path).map_err(|error| error.to_string())?,
        b"tampered-shadow"
    );
    Ok(())
}

#[test]
fn test_file_shadow_already_at_anchor_consumes_anchor_without_write() -> TestResult {
    let root = TestRoot::create("already-at-anchor")?;
    let workbook_path = root.workbook_path();
    let original = b"original-xlsx-bytes";
    write_bytes(&workbook_path, original)?;
    let mut anchors = ExcelL1AnchorStore::new();
    let mut file_store = ExcelL1FileShadowStore::open(root.shadow_root(), 1024)
        .map_err(|error| error.to_string())?;
    let handle = anchors
        .capture_with_file_shadow(
            request("already-at-anchor", &workbook_path, b"ignored"),
            &mut file_store,
        )
        .map_err(|error| error.to_string())?;
    let pre_write = workbook_identity(&workbook_path, 'a');
    anchors
        .prepare_write(handle, &pre_write)
        .map_err(|error| error.to_string())?;
    anchors
        .record_post_write(handle, &workbook_identity(&workbook_path, 'b'))
        .map_err(|error| error.to_string())?;
    let current_digest = content_digest(original).map_err(|error| error.to_string())?;

    let outcome = anchors
        .restore_with_file_shadow(handle, &pre_write, &current_digest, &file_store)
        .map_err(|error| error.to_string())?;
    assert!(matches!(
        outcome,
        ExcelL1FileRestoreOutcome::AlreadyAtAnchor
    ));
    assert_eq!(
        fs::read(&workbook_path).map_err(|error| error.to_string())?,
        original
    );
    assert_eq!(anchors.active_anchor_count(), 0);
    assert_eq!(file_store.captures.len(), 1);
    Ok(())
}

#[test]
fn test_file_shadow_total_bound_and_absolute_path_are_rejected() -> TestResult {
    let root = TestRoot::create("bounds-path")?;
    let workbook_path = root.workbook_path();
    let source = b"four";
    write_bytes(&workbook_path, source)?;
    let mut store =
        ExcelL1FileShadowStore::open(root.shadow_root(), 3).map_err(|error| error.to_string())?;
    let (canonical_source, source_bytes) =
        ExcelL1FileShadowStore::read_source(&workbook_path).map_err(|error| error.to_string())?;
    let bound_metadata = shadow_copy("excel-l1/1/bound.shadow", source)?;
    let bound_error = require_error!(
        store.publish_bytes(canonical_source.clone(), &bound_metadata, &source_bytes),
        "total bound"
    );
    assert_eq!(bound_error.code(), ErrorCode::UserInteraction);
    store.cleanup().map_err(|error| error.to_string())?;

    let mut larger_store = ExcelL1FileShadowStore::open(root.shadow_root(), 1024)
        .map_err(|error| error.to_string())?;
    let absolute_path = std::env::temp_dir().join("outside-task414.shadow");
    let absolute_metadata = shadow_copy(&absolute_path.to_string_lossy(), source)?;
    let path_error = require_error!(
        larger_store.publish_bytes(canonical_source, &absolute_metadata, &source_bytes),
        "absolute shadow path"
    );
    assert_eq!(path_error.code(), ErrorCode::ToolInvalidArgs);
    assert_eq!(larger_store.captures.len(), 0);
    Ok(())
}

#[test]
fn test_file_shadow_source_requires_local_xlsx_regular_file() -> TestResult {
    let root = TestRoot::create("source-shape")?;
    let non_xlsx = root.path.join("book.xls");
    write_bytes(&non_xlsx, b"not-xlsx")?;
    let missing = root.path.join("missing.xlsx");

    let extension_error = require_error!(
        ExcelL1FileShadowStore::read_source(&non_xlsx),
        "non-xlsx source"
    );
    assert_eq!(extension_error.code(), ErrorCode::ToolInvalidArgs);
    let missing_error = require_error!(
        ExcelL1FileShadowStore::read_source(&missing),
        "missing source"
    );
    assert_eq!(missing_error.code(), ErrorCode::TargetNotFound);
    Ok(())
}

#[test]
fn test_file_shadow_root_rejects_repository_and_nonempty_directories() -> TestResult {
    let repository_child = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| "agent-core manifest has no repository parent".to_owned())?
        .join("task414-forbidden-shadow-root");
    let repository_error = require_error!(
        ExcelL1FileShadowStore::open(&repository_child, 1024),
        "repository root"
    );
    assert_eq!(repository_error.code(), ErrorCode::ToolInvalidArgs);
    assert!(!repository_child.exists());

    let root = TestRoot::create("nonempty-root")?;
    let nonempty_root = root.path.join("nonempty-shadows");
    fs::create_dir_all(&nonempty_root).map_err(|error| error.to_string())?;
    let unrelated = nonempty_root.join("unrelated.txt");
    write_bytes(&unrelated, b"must-remain")?;
    let nonempty_error = require_error!(
        ExcelL1FileShadowStore::open(&nonempty_root, 1024),
        "nonempty root"
    );
    assert_eq!(nonempty_error.code(), ErrorCode::ToolInvalidArgs);
    assert_eq!(
        fs::read(&unrelated).map_err(|error| error.to_string())?,
        b"must-remain"
    );
    Ok(())
}

#[test]
fn test_file_shadow_backup_restore_never_replaces_reappeared_target() -> TestResult {
    let root = TestRoot::create("rollback-no-replace")?;
    let target = root.path.join("target.xlsx");
    let backup = root.path.join("target.restore-backup");
    write_bytes(&target, b"reappeared-target")?;
    write_bytes(&backup, b"original-backup")?;

    let error = require_error!(
        restore_backup_without_replacing(&target, &backup),
        "rollback target reappeared"
    );
    assert_eq!(error.code(), ErrorCode::UserInteraction);
    assert_eq!(
        fs::read(&target).map_err(|error| error.to_string())?,
        b"reappeared-target"
    );
    assert_eq!(
        fs::read(&backup).map_err(|error| error.to_string())?,
        b"original-backup"
    );
    Ok(())
}

#[test]
fn test_file_shadow_staged_failure_preserves_user_interaction_category() -> TestResult {
    let root = TestRoot::create("staged-reappeared")?;
    let target = root.path.join("target.xlsx");
    let backup = root.path.join("target.restore-backup");
    let temporary = root.path.join("target.restore-tmp");
    write_bytes(&target, b"reappeared-target")?;
    write_bytes(&backup, b"staged-agent-target")?;
    write_bytes(&temporary, b"restore-bytes")?;
    let expected =
        content_digest(b"different-recorded-target").map_err(|error| error.to_string())?;

    let error = require_error!(
        verify_staged_target(&target, &backup, &temporary, &expected),
        "staged target reappeared"
    );
    assert_eq!(error.code(), ErrorCode::UserInteraction);
    assert_eq!(
        fs::read(&target).map_err(|error| error.to_string())?,
        b"reappeared-target"
    );
    assert_eq!(
        fs::read(&backup).map_err(|error| error.to_string())?,
        b"staged-agent-target"
    );
    assert!(!temporary.exists());
    Ok(())
}

#[test]
fn test_file_shadow_anchor_capture_releases_memory_when_publish_fails() -> TestResult {
    let root = TestRoot::create("anchor-capture-rollback")?;
    let workbook_path = root.workbook_path();
    let original = b"original-xlsx-bytes";
    write_bytes(&workbook_path, original)?;
    let mut anchors = ExcelL1AnchorStore::new();
    let mut file_store = ExcelL1FileShadowStore::open(root.shadow_root(), 1024)
        .map_err(|error| error.to_string())?;
    let digest = content_digest(original).map_err(|error| error.to_string())?;
    let collision = root
        .shadow_root()
        .join("excel-l1/1")
        .join(format!("{}.shadow", digest.as_str()));
    if let Some(parent) = collision.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    write_bytes(&collision, b"collision")?;
    let error = require_error!(
        anchors.capture_with_file_shadow(
            request("capture-rollback", &workbook_path, b"ignored"),
            &mut file_store,
        ),
        "physical collision"
    );
    assert_eq!(error.code(), ErrorCode::UserInteraction);
    assert_eq!(anchors.active_anchor_count(), 0);
    assert_eq!(anchors.active_shadow_bytes(), 0);
    assert_eq!(file_store.captures.len(), 0);
    assert_eq!(
        fs::read(&collision).map_err(|error| error.to_string())?,
        b"collision"
    );
    Ok(())
}

#[test]
fn test_file_shadow_anchor_restore_requires_post_fingerprint_and_consumes_anchor() -> TestResult {
    let root = TestRoot::create("anchor-restore")?;
    let workbook_path = root.workbook_path();
    let original = b"original-xlsx-bytes";
    let written = b"agent-written-xlsx";
    write_bytes(&workbook_path, original)?;
    let mut anchors = ExcelL1AnchorStore::new();
    let mut file_store = ExcelL1FileShadowStore::open(root.shadow_root(), 1024)
        .map_err(|error| error.to_string())?;
    let handle = anchors
        .capture_with_file_shadow(
            request("anchor-restore", &workbook_path, b"ignored"),
            &mut file_store,
        )
        .map_err(|error| error.to_string())?;
    let pre_write = workbook_identity(&workbook_path, 'a');
    anchors
        .prepare_write(handle, &pre_write)
        .map_err(|error| error.to_string())?;
    write_bytes(&workbook_path, written)?;
    let post_write = workbook_identity(&workbook_path, 'b');
    anchors
        .record_post_write(handle, &post_write)
        .map_err(|error| error.to_string())?;
    let expected_current_digest = content_digest(written).map_err(|error| error.to_string())?;

    let outcome = anchors
        .restore_with_file_shadow(handle, &post_write, &expected_current_digest, &file_store)
        .map_err(|error| error.to_string())?;
    match outcome {
        ExcelL1FileRestoreOutcome::Restored(receipt) => {
            assert_eq!(receipt.byte_count, original.len());
        }
        ExcelL1FileRestoreOutcome::AlreadyAtAnchor => {
            return Err("expected a restored receipt".to_owned());
        }
    }
    assert_eq!(
        fs::read(&workbook_path).map_err(|error| error.to_string())?,
        original
    );
    assert_eq!(anchors.active_anchor_count(), 0);
    assert_eq!(file_store.captures.len(), 1);
    Ok(())
}

#[test]
fn test_file_shadow_anchor_restore_conflict_preserves_anchor_and_target() -> TestResult {
    let root = TestRoot::create("anchor-conflict")?;
    let workbook_path = root.workbook_path();
    let original = b"original-xlsx-bytes";
    let written = b"agent-written-xlsx";
    let user_change = b"user-changed-xlsx";
    write_bytes(&workbook_path, original)?;
    let mut anchors = ExcelL1AnchorStore::new();
    let mut file_store = ExcelL1FileShadowStore::open(root.shadow_root(), 1024)
        .map_err(|error| error.to_string())?;
    let handle = anchors
        .capture_with_file_shadow(
            request("anchor-conflict", &workbook_path, b"ignored"),
            &mut file_store,
        )
        .map_err(|error| error.to_string())?;
    let without_post = workbook_identity(&workbook_path, 'a');
    let missing = require_error!(
        anchors.restore_with_file_shadow(
            handle,
            &without_post,
            &content_digest(original).map_err(|error| error.to_string())?,
            &file_store,
        ),
        "post fingerprint evidence"
    );
    assert_eq!(missing.code(), ErrorCode::VerifyFailed);
    assert_eq!(anchors.active_anchor_count(), 1);

    let pre_write = workbook_identity(&workbook_path, 'a');
    anchors
        .prepare_write(handle, &pre_write)
        .map_err(|error| error.to_string())?;
    write_bytes(&workbook_path, written)?;
    let post_write = workbook_identity(&workbook_path, 'b');
    anchors
        .record_post_write(handle, &post_write)
        .map_err(|error| error.to_string())?;
    write_bytes(&workbook_path, user_change)?;
    let expected_current_digest = content_digest(written).map_err(|error| error.to_string())?;
    let drift = require_error!(
        anchors.restore_with_file_shadow(
            handle,
            &post_write,
            &expected_current_digest,
            &file_store,
        ),
        "user drift"
    );
    assert_eq!(drift.code(), ErrorCode::VerifyFailed);
    assert_eq!(
        fs::read(&workbook_path).map_err(|error| error.to_string())?,
        user_change
    );
    assert_eq!(anchors.active_anchor_count(), 1);
    assert_eq!(file_store.captures.len(), 1);
    Ok(())
}
