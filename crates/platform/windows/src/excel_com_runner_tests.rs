//! Test backends for the Excel COM runner prototype.
//!
//! Responsibilities:
//! - record stage order, worker thread identity, failures, and panics;
//! - provide deterministic queue blocking without real OS or Excel dependencies.
//!
//! Boundaries:
//! - these backends exist only in crate tests;
//! - they never create COM objects, launch Excel, or touch workbook files.
//!
//! Invariants:
//! 1. every stage is observed on the worker thread;
//! 2. injected failures preserve the requested `ErrorCode`;
//! 3. queue tests release their gate before shutdown.

#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::print_stderr,
    clippy::unwrap_used
)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, ThreadId};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use assistant_platform_api::ErrorCode;

#[cfg(windows)]
use super::api::real_com_support::{RealComAcceptanceGuard, verify_real_com_read_surface};
#[cfg(windows)]
use super::windows_backend::parse_named_range_scope;
#[cfg(windows)]
use super::*;
#[cfg(windows)]
use crate::excel_process::{excel_process_ids, process_identity};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    Create,
    ProcessId,
    Open,
    Read,
    ReadWorkbook,
    ReadRange,
    VerifyOwnership,
    Close,
    Quit,
}

#[derive(Default)]
struct Tracker {
    events: Vec<Stage>,
    threads: Vec<ThreadId>,
    failure: Option<(Stage, ErrorCode)>,
    panic_at: Option<Stage>,
}

struct RecordingBackend {
    tracker: Arc<Mutex<Tracker>>,
}

impl RecordingBackend {
    fn new(
        failure: Option<(Stage, ErrorCode)>,
        panic_at: Option<Stage>,
    ) -> (Self, Arc<Mutex<Tracker>>) {
        let tracker = Arc::new(Mutex::new(Tracker {
            failure,
            panic_at,
            ..Tracker::default()
        }));
        (
            Self {
                tracker: Arc::clone(&tracker),
            },
            tracker,
        )
    }

    fn observe(&self, stage: Stage) -> PlatformResult<()> {
        let (should_panic, failure) = {
            let mut tracker = self.tracker.lock().expect("tracker mutex");
            tracker.events.push(stage);
            tracker.threads.push(thread::current().id());
            (tracker.panic_at == Some(stage), tracker.failure)
        };
        assert!(!should_panic, "injected backend panic");
        if let Some((failure_stage, code)) = failure
            && failure_stage == stage
        {
            return Err(PlatformError::new(code, "injected backend failure"));
        }
        Ok(())
    }
}

impl ExcelComBackend for RecordingBackend {
    fn create_application(&mut self) -> PlatformResult<()> {
        self.observe(Stage::Create)
    }

    fn application_identity(&mut self) -> PlatformResult<ExcelOwnedProcessIdentity> {
        self.observe(Stage::ProcessId)?;
        test_identity(4242)
    }

    fn open_read_only(&mut self, _path: &Path) -> PlatformResult<()> {
        self.observe(Stage::Open)
    }

    fn read_active_sheet_text(&self, address: &str) -> PlatformResult<String> {
        self.observe(Stage::Read)?;
        Ok(format!("value@{address}"))
    }

    fn read_workbook(&mut self, path: &Path) -> PlatformResult<ExcelWorkbookSnapshot> {
        self.observe(Stage::ReadWorkbook)?;
        let worksheet =
            ExcelWorksheetInfo::new("Sheet1".to_owned(), 1, Some("$A$1:$B$2".to_owned()))?;
        let named_range = ExcelNamedRangeInfo::new(
            "SalesData".to_owned(),
            ExcelNamedRangeScope::Workbook,
            "=Sheet1!$A$1:$B$2".to_owned(),
        )?;
        ExcelWorkbookSnapshot::new(
            path.to_string_lossy().into_owned(),
            true,
            vec![worksheet],
            vec![named_range],
            ExcelCalculationInfo::new("manual".to_owned(), "done".to_owned()),
        )
    }

    fn read_range(
        &mut self,
        _selector: &ExcelRangeSelector,
        mode: ExcelReadMode,
    ) -> PlatformResult<ExcelRangeSnapshot> {
        self.observe(Stage::ReadRange)?;
        let cells = vec![
            ExcelRuntimeCell::new(1, 1, ExcelCellValueType::Number, "2".to_owned())?,
            ExcelRuntimeCell::new(1, 2, ExcelCellValueType::Boolean, "true".to_owned())?,
            ExcelRuntimeCell::new(2, 1, ExcelCellValueType::Text, "hello".to_owned())?,
            ExcelRuntimeCell::new(2, 2, ExcelCellValueType::Blank, String::new())?,
        ];
        ExcelRangeSnapshot::new("$A$1:$B$2".to_owned(), 2, 2, mode, cells)
    }

    fn verify_ownership(&self, _identity: &ExcelOwnedProcessIdentity) -> PlatformResult<()> {
        self.observe(Stage::VerifyOwnership)
    }

    fn close_workbook(&mut self) -> PlatformResult<()> {
        self.observe(Stage::Close)
    }

    fn quit_application(&mut self) -> PlatformResult<()> {
        self.observe(Stage::Quit)
    }
}

struct CreateGate {
    started: Mutex<bool>,
    started_ready: Condvar,
    released: Mutex<bool>,
    release_ready: Condvar,
}

impl CreateGate {
    fn new() -> Self {
        Self {
            started: Mutex::new(false),
            started_ready: Condvar::new(),
            released: Mutex::new(false),
            release_ready: Condvar::new(),
        }
    }

    fn wait_until_started(&self) {
        let mut started = self.started.lock().expect("started mutex");
        while !*started {
            started = self.started_ready.wait(started).expect("started wait");
        }
        drop(started);
    }

    fn release(&self) {
        *self.released.lock().expect("released mutex") = true;
        self.release_ready.notify_all();
    }

    fn block_create(&self) {
        *self.started.lock().expect("started mutex") = true;
        self.started_ready.notify_all();
        let mut released = self.released.lock().expect("released mutex");
        while !*released {
            released = self.release_ready.wait(released).expect("release wait");
        }
        drop(released);
    }
}

struct BlockingBackend {
    gate: Arc<CreateGate>,
    quit_called: Arc<AtomicBool>,
}

impl ExcelComBackend for BlockingBackend {
    fn create_application(&mut self) -> PlatformResult<()> {
        self.gate.block_create();
        Ok(())
    }

    fn application_identity(&mut self) -> PlatformResult<ExcelOwnedProcessIdentity> {
        test_identity(77)
    }

    fn open_read_only(&mut self, _path: &Path) -> PlatformResult<()> {
        Ok(())
    }

    fn read_active_sheet_text(&self, _address: &str) -> PlatformResult<String> {
        Ok(String::new())
    }

    fn read_workbook(&mut self, _path: &Path) -> PlatformResult<ExcelWorkbookSnapshot> {
        Err(runner_protocol_error(
            "blocking backend metadata unavailable",
        ))
    }

    fn read_range(
        &mut self,
        _selector: &ExcelRangeSelector,
        _mode: ExcelReadMode,
    ) -> PlatformResult<ExcelRangeSnapshot> {
        Err(runner_protocol_error("blocking backend range unavailable"))
    }

    fn verify_ownership(&self, _identity: &ExcelOwnedProcessIdentity) -> PlatformResult<()> {
        Ok(())
    }

    fn close_workbook(&mut self) -> PlatformResult<()> {
        Ok(())
    }

    fn quit_application(&mut self) -> PlatformResult<()> {
        self.quit_called.store(true, Ordering::SeqCst);
        Ok(())
    }
}

fn recording_runner(
    failure: Option<(Stage, ErrorCode)>,
    panic_at: Option<Stage>,
) -> (ExcelComRunner, Arc<Mutex<Tracker>>) {
    let (backend, tracker) = RecordingBackend::new(failure, panic_at);
    let runner = ExcelComRunner::spawn(move || backend).expect("runner");
    (runner, tracker)
}

fn test_identity(process_id: u32) -> PlatformResult<ExcelOwnedProcessIdentity> {
    ExcelOwnedProcessIdentity::new(
        process_id,
        u64::from(process_id),
        "C:\\Program Files\\Microsoft Office\\root\\Office16\\EXCEL.EXE".to_owned(),
    )
}

fn temporary_workbook() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "assistant-task401-{}-{nonce}.xlsx",
        std::process::id()
    ));
    std::fs::write(&path, b"task401 fake workbook").expect("write temporary workbook");
    path
}

#[test]
fn test_runner_preserves_fifo_order_on_dedicated_thread() {
    let main_thread = thread::current().id();
    let (mut runner, tracker) = recording_runner(None, None);

    runner.create_application().expect("create");
    assert_eq!(runner.application_process_id().expect("process id"), 4242);
    runner.quit_application().expect("quit");
    runner.shutdown().expect("shutdown");

    let tracker = tracker.lock().expect("tracker mutex");
    assert_eq!(
        tracker.events,
        vec![Stage::Create, Stage::ProcessId, Stage::Quit]
    );
    assert_eq!(tracker.threads.len(), 3);
    assert!(
        tracker
            .threads
            .iter()
            .all(|thread_id| *thread_id != main_thread)
    );
    assert!(tracker.threads.windows(2).all(|pair| pair[0] == pair[1]));
    drop(tracker);
}

#[test]
fn test_runner_open_read_close_preserves_boundary_and_handle_identity() {
    let workbook_path = temporary_workbook();
    let (mut runner, tracker) = recording_runner(None, None);

    runner.create_application().expect("create");
    let workbook = runner
        .open_read_only(&workbook_path)
        .expect("open read-only workbook");
    assert_eq!(workbook.session_id(), 1);
    assert!(workbook.canonical_path().is_absolute());
    assert_eq!(
        workbook.canonical_path().file_name(),
        workbook_path.file_name()
    );
    assert_eq!(
        runner
            .read_active_sheet_text(&workbook, "a1")
            .expect("read active sheet text")
            .value(),
        "value@A1"
    );
    assert_eq!(
        runner
            .open_read_only(&workbook_path)
            .expect_err("second open must fail")
            .code(),
        ErrorCode::Fatal
    );
    runner.close_workbook(&workbook).expect("close workbook");
    let stale = runner
        .read_active_sheet_text(
            &ExcelWorkbookHandle {
                runner_id: 1,
                session_id: 1,
                canonical_path: workbook_path.clone(),
                owned_process: test_identity(1).expect("test identity"),
            },
            "A1",
        )
        .expect_err("stale handle must fail");
    assert_eq!(stale.code(), ErrorCode::ToolInvalidArgs);
    runner.shutdown().expect("shutdown");

    let tracker = tracker.lock().expect("tracker mutex");
    assert_eq!(
        tracker.events,
        vec![
            Stage::Create,
            Stage::ProcessId,
            Stage::Open,
            Stage::Read,
            Stage::Close,
            Stage::Quit
        ]
    );
    drop(tracker);
    std::fs::remove_file(workbook_path).expect("remove temporary workbook");
}

fn assert_failure_code_preserved(stage: Stage, code: ErrorCode) {
    let (mut runner, tracker) = recording_runner(Some((stage, code)), None);

    match stage {
        Stage::Create => {
            let error = runner.create_application().expect_err("create failure");
            assert_eq!(error.code(), code);
        }
        Stage::ProcessId => {
            let error = runner
                .create_application()
                .expect_err("process identity failure");
            assert_eq!(error.code(), code);
        }
        Stage::Quit => {
            runner.create_application().expect("create");
            let error = runner.quit_application().expect_err("quit failure");
            assert_eq!(error.code(), code);
        }
        Stage::Open
        | Stage::Read
        | Stage::ReadWorkbook
        | Stage::ReadRange
        | Stage::VerifyOwnership
        | Stage::Close => assert_workbook_failure_code_preserved(&runner, stage, code),
    }

    let shutdown = runner.shutdown();
    if matches!(stage, Stage::Quit | Stage::Close) {
        assert!(
            shutdown.is_err(),
            "shutdown must not hide a failed explicit quit"
        );
    } else {
        shutdown.expect("shutdown");
    }
    let failure = {
        let tracker = tracker.lock().expect("tracker mutex");
        tracker.failure
    };
    assert_eq!(failure, Some((stage, code)));
}

fn assert_workbook_failure_code_preserved(runner: &ExcelComRunner, stage: Stage, code: ErrorCode) {
    runner.create_application().expect("create");
    let workbook_path = temporary_workbook();
    let open_result = runner.open_read_only(&workbook_path);
    match stage {
        Stage::Open => {
            let error = open_result.expect_err("open failure");
            assert_eq!(error.code(), code);
        }
        Stage::Read => {
            let workbook = open_result.expect("open before read failure");
            let error = runner
                .read_active_sheet_text(&workbook, "A1")
                .expect_err("read failure");
            assert_eq!(error.code(), code);
        }
        Stage::ReadWorkbook => {
            let workbook = open_result.expect("open before workbook failure");
            let error = runner
                .read_workbook(&workbook)
                .expect_err("workbook metadata failure");
            assert_eq!(error.code(), code);
        }
        Stage::ReadRange => {
            let workbook = open_result.expect("open before range failure");
            let error = runner
                .read_range(
                    &workbook,
                    ExcelRangeSelector::A1 {
                        worksheet: ExcelWorksheetSelector::Ordinal(1),
                        start_row: 1,
                        start_column: 1,
                        end_row: 1,
                        end_column: 1,
                    },
                    ExcelReadMode::Value2,
                )
                .expect_err("range failure");
            assert_eq!(error.code(), code);
        }
        Stage::VerifyOwnership => {
            let workbook = open_result.expect("open before ownership failure");
            let error = runner
                .verify_ownership(&workbook)
                .expect_err("ownership failure");
            assert_eq!(error.code(), code);
        }
        Stage::Close => {
            let workbook = open_result.expect("open before close failure");
            let error = runner.close_workbook(&workbook).expect_err("close failure");
            assert_eq!(error.code(), code);
        }
        Stage::Create | Stage::ProcessId | Stage::Quit => {
            unreachable!("outer match restricts workbook stages")
        }
    }
    std::fs::remove_file(workbook_path).expect("remove temporary workbook");
}

#[test]
fn test_runner_preserves_backend_error_codes_without_downgrade() {
    assert_failure_code_preserved(Stage::Create, ErrorCode::TargetNotFound);
    assert_failure_code_preserved(Stage::ProcessId, ErrorCode::PlatformPermission);
    assert_failure_code_preserved(Stage::Open, ErrorCode::ToolInvalidArgs);
    assert_failure_code_preserved(Stage::Read, ErrorCode::TargetNotFound);
    assert_failure_code_preserved(Stage::Close, ErrorCode::UserInteraction);
    assert_failure_code_preserved(Stage::Quit, ErrorCode::UserInteraction);
}

#[test]
fn test_runner_rejects_invalid_paths_and_cell_addresses_before_dispatch() {
    let (mut runner, _tracker) = recording_runner(None, None);

    let relative = runner
        .open_read_only("book.xlsx")
        .expect_err("relative path must fail");
    assert_eq!(relative.code(), ErrorCode::ToolInvalidArgs);
    let wrong_extension = runner
        .open_read_only(std::env::temp_dir().join("book.xls"))
        .expect_err("non-xlsx extension must fail");
    assert_eq!(wrong_extension.code(), ErrorCode::ToolInvalidArgs);
    let traversing = runner
        .open_read_only(
            std::env::temp_dir()
                .join("sub")
                .join("..")
                .join("book.xlsx"),
        )
        .expect_err("dot-dot path must fail");
    assert_eq!(traversing.code(), ErrorCode::ToolInvalidArgs);

    let stale = ExcelWorkbookHandle {
        runner_id: 1,
        session_id: 1,
        canonical_path: std::env::temp_dir().join("book.xlsx"),
        owned_process: test_identity(1).expect("test identity"),
    };
    for invalid in [
        "1", "A0", "A01", "A1:B2", "$$", "AAAA1", "XFE1", "ZZZ1", "A1048577",
    ] {
        let error = runner
            .read_active_sheet_text(&stale, invalid)
            .expect_err("invalid A1 address must fail");
        assert_eq!(error.code(), ErrorCode::ToolInvalidArgs);
    }
    runner.create_application().expect("create");
    runner.shutdown().expect("shutdown");
}

#[test]
fn test_runner_rejects_a_workbook_handle_from_another_runner() {
    let first_path = temporary_workbook();
    let second_path = temporary_workbook();
    let (mut first, _first_tracker) = recording_runner(None, None);
    let (mut second, _second_tracker) = recording_runner(None, None);
    first.create_application().expect("first create");
    second.create_application().expect("second create");
    let first_workbook = first
        .open_read_only(&first_path)
        .expect("first open workbook");
    second
        .open_read_only(&second_path)
        .expect("second open workbook");

    let mismatch = second
        .read_active_sheet_text(&first_workbook, "A1")
        .expect_err("foreign runner handle must fail");
    assert_eq!(mismatch.code(), ErrorCode::ToolInvalidArgs);

    first.close_workbook(&first_workbook).expect("first close");
    first.shutdown().expect("first shutdown");
    second.shutdown().expect("second shutdown");
    std::fs::remove_file(first_path).expect("remove first temporary workbook");
    std::fs::remove_file(second_path).expect("remove second temporary workbook");
}

#[test]
fn test_runner_full_read_surface_reports_metadata_ranges_and_fingerprint() {
    let workbook_path = temporary_workbook();
    let (mut runner, _tracker) = recording_runner(None, None);
    runner.create_application().expect("create");
    let workbook = runner
        .open_read_only(&workbook_path)
        .expect("open workbook");

    let metadata = runner
        .read_workbook(&workbook)
        .expect("read workbook metadata");
    assert_eq!(metadata.worksheets().len(), 1);
    assert_eq!(metadata.worksheets()[0].name(), "Sheet1");
    assert_eq!(metadata.named_ranges().len(), 1);
    assert_eq!(metadata.calculation().mode(), "manual");

    for mode in [
        ExcelReadMode::Value2,
        ExcelReadMode::Formula,
        ExcelReadMode::Text,
    ] {
        let snapshot = runner
            .read_range(
                &workbook,
                ExcelRangeSelector::A1 {
                    worksheet: ExcelWorksheetSelector::Name("Sheet1".to_owned()),
                    start_row: 1,
                    start_column: 1,
                    end_row: 2,
                    end_column: 2,
                },
                mode,
            )
            .expect("read range");
        assert_eq!(snapshot.rows(), 2);
        assert_eq!(snapshot.columns(), 2);
        assert_eq!(snapshot.cells().len(), 4);
        assert_eq!(snapshot.mode(), mode);
    }

    let first = runner
        .fingerprint_workbook(&workbook)
        .expect("first fingerprint");
    let second = runner
        .fingerprint_workbook(&workbook)
        .expect("second fingerprint");
    assert_eq!(first, second);
    runner
        .verify_ownership(&workbook)
        .expect("verify ownership");
    runner.close_workbook(&workbook).expect("close");
    runner.shutdown().expect("shutdown");
    std::fs::remove_file(workbook_path).expect("remove workbook");
}

#[test]
fn test_runner_rejects_invalid_read_selectors_and_snapshot_bounds() {
    let (mut runner, _tracker) = recording_runner(None, None);
    runner.create_application().expect("create");
    let workbook_path = temporary_workbook();
    let workbook = runner
        .open_read_only(&workbook_path)
        .expect("open workbook");

    for selector in [
        ExcelRangeSelector::NamedRange(String::new()),
        ExcelRangeSelector::A1 {
            worksheet: ExcelWorksheetSelector::Ordinal(0),
            start_row: 1,
            start_column: 1,
            end_row: 1,
            end_column: 1,
        },
        ExcelRangeSelector::A1 {
            worksheet: ExcelWorksheetSelector::Ordinal(1),
            start_row: 2,
            start_column: 1,
            end_row: 1,
            end_column: 1,
        },
        ExcelRangeSelector::A1 {
            worksheet: ExcelWorksheetSelector::Ordinal(1),
            start_row: 1,
            start_column: 1,
            end_row: 65,
            end_column: 64,
        },
    ] {
        let error = runner
            .read_range(&workbook, selector, ExcelReadMode::Value2)
            .expect_err("invalid range selector");
        assert_eq!(error.code(), ErrorCode::ToolInvalidArgs);
    }

    let snapshot_error = ExcelRangeSnapshot::new(
        "$A$1".to_owned(),
        2,
        2,
        ExcelReadMode::Value2,
        vec![ExcelRuntimeCell::new(1, 1, ExcelCellValueType::Blank, String::new()).expect("cell")],
    )
    .expect_err("incomplete range must fail");
    assert_eq!(snapshot_error.code(), ErrorCode::Fatal);

    runner.close_workbook(&workbook).expect("close");
    runner.shutdown().expect("shutdown");
    std::fs::remove_file(workbook_path).expect("remove workbook");
}

#[test]
fn test_named_range_scope_parser_is_explicit() {
    assert_eq!(
        parse_named_range_scope("Sheet1!SalesData").expect("worksheet scope"),
        (
            ExcelNamedRangeScope::Worksheet("Sheet1".to_owned()),
            "SalesData".to_owned()
        )
    );
    assert_eq!(
        parse_named_range_scope("SalesData").expect("workbook scope"),
        (ExcelNamedRangeScope::Workbook, "SalesData".to_owned())
    );
    let invalid = parse_named_range_scope("!").expect_err("invalid qualified name");
    assert_eq!(invalid.code(), ErrorCode::Fatal);
}

#[test]
fn test_runner_request_queue_capacity_is_one() {
    let gate = Arc::new(CreateGate::new());
    let quit_called = Arc::new(AtomicBool::new(false));
    let mut runner = ExcelComRunner::spawn({
        let gate = Arc::clone(&gate);
        let quit_called = Arc::clone(&quit_called);
        move || BlockingBackend { gate, quit_called }
    })
    .expect("runner");

    thread::scope(|scope| {
        let create = scope.spawn(|| runner.create_application());
        gate.wait_until_started();

        let (queued_sender, queued_receiver) = mpsc::sync_channel(1);
        runner
            .sender
            .try_send(RunnerCommand::ApplicationProcessId(queued_sender))
            .expect("second request should occupy the queue");

        let full = runner
            .dispatch(RunnerCommand::QuitApplication)
            .expect_err("third dispatch must fail full");
        assert_eq!(full.code(), ErrorCode::TargetUnresponsive);

        gate.release();
        create.join().expect("create thread").expect("create");
        assert_eq!(
            queued_receiver
                .recv_timeout(Duration::from_secs(1))
                .expect("queued response")
                .expect("queued process id"),
            RunnerValue::ProcessId(77)
        );
    });
    runner.shutdown().expect("shutdown");
    assert!(quit_called.load(Ordering::SeqCst));
}

#[test]
fn test_worker_abandons_created_application_when_response_receiver_disappears() {
    let gate = Arc::new(CreateGate::new());
    let quit_called = Arc::new(AtomicBool::new(false));
    let runner = ExcelComRunner::spawn({
        let gate = Arc::clone(&gate);
        let quit_called = Arc::clone(&quit_called);
        move || BlockingBackend { gate, quit_called }
    })
    .expect("runner");
    let (response_sender, response_receiver) = mpsc::sync_channel(1);
    runner
        .sender
        .try_send(RunnerCommand::CreateApplication(response_sender))
        .expect("create command");
    gate.wait_until_started();
    drop(response_receiver);
    gate.release();

    let mut runner = runner;
    let shutdown = runner
        .shutdown()
        .expect_err("worker should have disconnected after response loss");
    assert_eq!(shutdown.code(), ErrorCode::Fatal);
    assert!(quit_called.load(Ordering::SeqCst));
}

#[test]
fn test_runner_disconnect_and_repeated_shutdown_fail_explicitly() {
    let (mut runner, _tracker) = recording_runner(None, None);
    runner.shutdown().expect("first shutdown");

    let disconnected = runner
        .application_process_id()
        .expect_err("disconnected runner must fail");
    assert_eq!(disconnected.code(), ErrorCode::Fatal);
    let repeated = runner.shutdown().expect_err("second shutdown must fail");
    assert_eq!(repeated.code(), ErrorCode::Fatal);
}

#[test]
fn test_runner_thread_panic_is_explicit_and_not_reported_as_success() {
    let (mut runner, _tracker) = recording_runner(None, Some(Stage::Create));
    let create = runner
        .create_application()
        .expect_err("panicking worker must fail");
    assert_eq!(create.code(), ErrorCode::Fatal);
    let shutdown = runner
        .shutdown()
        .expect_err("panicked worker shutdown must fail");
    assert_eq!(shutdown.code(), ErrorCode::Fatal);
}

#[test]
fn test_runner_state_rejects_out_of_order_operations() {
    let (mut runner, _tracker) = recording_runner(None, None);
    let before_create = runner
        .application_process_id()
        .expect_err("process id before create must fail");
    assert_eq!(before_create.code(), ErrorCode::Fatal);
    runner.create_application().expect("create");
    let duplicate = runner
        .create_application()
        .expect_err("duplicate create must fail");
    assert_eq!(duplicate.code(), ErrorCode::Fatal);
    runner.quit_application().expect("quit");
    let after_quit = runner
        .quit_application()
        .expect_err("duplicate quit must fail");
    assert_eq!(after_quit.code(), ErrorCode::Fatal);
    runner.shutdown().expect("shutdown");
}

#[cfg(windows)]
#[test]
fn test_windows_idispatch_backend_starts_without_creating_excel() {
    let mut runner = ExcelComRunner::start().expect("Windows COM runner");
    runner.shutdown().expect("idle Windows runner shutdown");
}

#[cfg(windows)]
#[test]
#[ignore = "TASK-401 human-present real Excel COM acceptance"]
fn test_windows_excel_real_com_single_workbook_acceptance() {
    assert!(
        excel_process_ids()
            .expect("Excel process inventory")
            .is_empty(),
        "close every EXCEL.EXE before running the human-present acceptance"
    );
    let workbook_path = std::env::var_os("EXCEL_T21_ACCEPTANCE_WORKBOOK")
        .map(PathBuf::from)
        .expect("EXCEL_T21_ACCEPTANCE_WORKBOOK must name one local .xlsx file");
    let bytes_before = std::fs::read(&workbook_path).expect("read workbook before acceptance");
    let digest_before = crate::digest::sha256_hex_bytes(&bytes_before);

    let mut guard = RealComAcceptanceGuard::new(
        ExcelComRunner::start().expect("start Windows Excel COM runner"),
    );
    guard
        .runner()
        .create_application()
        .expect("create owned Excel application");
    let process_id = guard
        .runner()
        .application_process_id()
        .expect("resolve owned Excel process id");
    let process_identity = process_identity(process_id)
        .expect("query owned Excel identity")
        .expect("owned Excel identity");
    guard.set_owned_process(process_identity.clone());
    eprintln!(
        "TASK-401 acceptance workbook={} pid={} start={} path={} digest_before={}",
        workbook_path.display(),
        process_id,
        process_identity.start_time_100ns,
        process_identity.executable_path,
        digest_before
    );
    let workbook = guard
        .runner()
        .open_read_only(&workbook_path)
        .expect("open workbook read-only");
    verify_real_com_read_surface(guard.runner(), &workbook);
    guard
        .runner()
        .close_workbook(&workbook)
        .expect("close workbook without saving");
    guard.shutdown().expect("quit and join Excel worker");

    // Excel can take several seconds to release its process after Quit on the
    // first workbook with error cells. Keep the wait bounded while allowing
    // normal COM teardown to finish.
    let mut remaining = true;
    for _attempt in 0..200 {
        if !excel_process_ids()
            .expect("Excel process inventory")
            .contains(&process_id)
        {
            remaining = false;
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    assert!(
        !remaining,
        "owned Excel process {process_id} remained alive"
    );
    assert!(
        excel_process_ids()
            .expect("Excel process inventory")
            .is_empty(),
        "acceptance must leave no EXCEL.EXE process"
    );
    let bytes_after = std::fs::read(&workbook_path).expect("read workbook after acceptance");
    let digest_after = crate::digest::sha256_hex_bytes(&bytes_after);
    eprintln!("TASK-401 acceptance digest_after={digest_after}");
    assert_eq!(
        digest_before, digest_after,
        "read-only acceptance must not modify the workbook"
    );
}
