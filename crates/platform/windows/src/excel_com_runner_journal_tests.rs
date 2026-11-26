//! Runner-level tests for the platform-private Excel ownership journal.
//!
//! These tests verify publication order and cleanup behavior through the real
//! private runner state machine. They use fake COM backends only.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;
use crate::cdp::JsonValue;

static NEXT_TEMPORARY_DIRECTORY_ID: AtomicU64 = AtomicU64::new(1);

struct TemporaryDirectory {
    path: PathBuf,
}

impl TemporaryDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "assistant-task411-runner-{}-{}",
            NEXT_TEMPORARY_DIRECTORY_ID.fetch_add(1, Ordering::Relaxed),
            std::process::id()
        ));
        std::fs::create_dir(&path).expect("create temporary directory");
        Self { path }
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            eprintln!(
                "TASK-411 temporary directory cleanup failed for {}: {error}",
                self.path.display()
            );
        }
    }
}

#[test]
fn test_temporary_workbooks_are_unique_within_one_process() {
    let first = temporary_workbook();
    let second = temporary_workbook();
    assert_ne!(first, second);
    std::fs::remove_file(first).expect("remove first temporary workbook");
    std::fs::remove_file(second).expect("remove second temporary workbook");
}

#[test]
fn test_runner_journal_lifecycle_is_bounded_per_session() {
    let directory = TemporaryDirectory::new();
    let workbook_path = temporary_workbook();
    let store =
        ExcelOwnershipJournalStore::for_tests_with_root(directory.path.clone()).expect("store");
    let (backend, _tracker) = RecordingBackend::new(None, None);
    let mut runner =
        ExcelComRunner::spawn_with_journal_store(move || backend, store).expect("runner");

    runner.create_application().expect("create");
    assert!(journal_phase_exists(&directory.path, "ownership-captured"));

    let first = runner
        .open_read_only(&workbook_path)
        .expect("first read-only workbook");
    assert!(journal_phase_exists(&directory.path, "opening"));
    runner.close_workbook(&first).expect("close first workbook");
    assert!(!journal_phase_exists(&directory.path, "opening"));
    assert!(!journal_phase_exists(&directory.path, "closing"));

    let second = runner
        .open_read_only(&workbook_path)
        .expect("second read-only workbook");
    assert_ne!(first.session_id(), second.session_id());
    assert_eq!(journal_contents(&directory.path).len(), 2);
    assert!(journal_phase_exists(&directory.path, "opening"));
    runner
        .close_workbook(&second)
        .expect("close second workbook");

    runner.shutdown().expect("shutdown");
    assert!(journal_contents(&directory.path).is_empty());
    std::fs::remove_file(workbook_path).expect("remove temporary workbook");
}

#[test]
fn test_runner_journal_publish_failure_prevents_risky_cleanup() {
    let directory = TemporaryDirectory::new();
    let root_file = directory.path.join("journal-root-file");
    std::fs::write(&root_file, b"not a journal directory").expect("write root file");
    let store = ExcelOwnershipJournalStore::for_tests_unchecked(root_file);
    let (backend, tracker) = RecordingBackend::new(None, None);
    let mut runner =
        ExcelComRunner::spawn_with_journal_store(move || backend, store).expect("runner");

    let create = runner
        .create_application()
        .expect_err("journal publish failure");
    assert_eq!(create.code(), ErrorCode::Fatal);
    let shutdown = runner.shutdown().expect_err("cleanup journal failure");
    assert_eq!(shutdown.code(), ErrorCode::Fatal);
    let tracker = tracker.lock().expect("tracker");
    assert_eq!(tracker.events, vec![Stage::Create, Stage::ProcessId]);
}

#[test]
fn test_runner_journal_is_published_before_open_close_and_quit() {
    let directory = TemporaryDirectory::new();
    let workbook_path = temporary_workbook();
    let store =
        ExcelOwnershipJournalStore::for_tests_with_root(directory.path.clone()).expect("store");
    let backend = OrderingBackend {
        journal_root: directory.path.clone(),
    };
    let mut runner =
        ExcelComRunner::spawn_with_journal_store(move || backend, store).expect("runner");

    runner.create_application().expect("create");
    runner
        .open_read_only(&workbook_path)
        .expect("open read-only workbook");
    runner.shutdown().expect("shutdown");
    assert!(journal_contents(&directory.path).is_empty());
    std::fs::remove_file(workbook_path).expect("remove temporary workbook");
}

#[test]
fn test_runner_journal_retains_cleanup_evidence_after_close_failure() {
    let directory = TemporaryDirectory::new();
    let workbook_path = temporary_workbook();
    let store =
        ExcelOwnershipJournalStore::for_tests_with_root(directory.path.clone()).expect("store");
    let (backend, _tracker) =
        RecordingBackend::new(Some((Stage::Close, ErrorCode::UserInteraction)), None);
    let mut runner =
        ExcelComRunner::spawn_with_journal_store(move || backend, store).expect("runner");

    runner.create_application().expect("create");
    runner
        .open_read_only(&workbook_path)
        .expect("open read-only workbook");
    let error = runner.shutdown().expect_err("close failure");
    assert_eq!(error.code(), ErrorCode::UserInteraction);
    assert!(journal_phase_exists(&directory.path, "cleanup"));
    assert!(journal_phase_exists(&directory.path, "closing"));
    assert_eq!(journal_contents(&directory.path).len(), 4);
    std::fs::remove_file(workbook_path).expect("remove temporary workbook");
}

#[test]
fn test_runner_journal_retains_cleanup_evidence_after_quit_failure() {
    let directory = TemporaryDirectory::new();
    let workbook_path = temporary_workbook();
    let store =
        ExcelOwnershipJournalStore::for_tests_with_root(directory.path.clone()).expect("store");
    let (backend, _tracker) =
        RecordingBackend::new(Some((Stage::Quit, ErrorCode::UserInteraction)), None);
    let mut runner =
        ExcelComRunner::spawn_with_journal_store(move || backend, store).expect("runner");

    runner.create_application().expect("create");
    runner
        .open_read_only(&workbook_path)
        .expect("open read-only workbook");
    let error = runner.shutdown().expect_err("quit failure");
    assert_eq!(error.code(), ErrorCode::UserInteraction);
    assert!(journal_phase_exists(&directory.path, "cleanup"));
    assert!(!journal_phase_exists(&directory.path, "closing"));
    assert_eq!(journal_contents(&directory.path).len(), 2);
    std::fs::remove_file(workbook_path).expect("remove temporary workbook");
}

struct OrderingBackend {
    journal_root: PathBuf,
}

impl ExcelComBackend for OrderingBackend {
    fn create_application(&mut self) -> PlatformResult<()> {
        Ok(())
    }

    fn application_identity(&mut self) -> PlatformResult<ExcelOwnedProcessIdentity> {
        test_identity(5151)
    }

    fn open_read_only(&mut self, _path: &Path) -> PlatformResult<()> {
        require_journal_phase(&self.journal_root, "opening")
    }

    fn close_workbook(&mut self) -> PlatformResult<()> {
        require_journal_phase(&self.journal_root, "cleanup")?;
        require_journal_phase(&self.journal_root, "closing")
    }

    fn quit_application(&mut self) -> PlatformResult<()> {
        require_journal_phase(&self.journal_root, "cleanup")
    }
}

fn require_journal_phase(directory: &Path, phase: &str) -> PlatformResult<()> {
    if journal_phase_exists(directory, phase) {
        Ok(())
    } else {
        Err(PlatformError::new(
            ErrorCode::Fatal,
            format!("journal phase {phase} was not published before the backend operation"),
        ))
    }
}

fn journal_phase_exists(directory: &Path, phase: &str) -> bool {
    journal_contents(directory)
        .iter()
        .any(|content| content.get("phase").and_then(JsonValue::as_str) == Some(phase))
}

fn journal_contents(directory: &Path) -> Vec<JsonValue> {
    let mut paths = std::fs::read_dir(directory)
        .expect("read journal directory")
        .map(|entry| entry.expect("journal entry").path())
        .collect::<Vec<_>>();
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let content = std::fs::read(path).expect("read journal");
            crate::cdp::parse(&content).expect("parse journal")
        })
        .collect()
}
