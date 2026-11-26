//! Contract tests for the private Excel T2.1 platform runtime adapter.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use assistant_platform_api::{PlatformError, PlatformResult};

use super::*;

const FINGERPRINT: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Debug, Clone, PartialEq, Eq)]
struct FakeWorkbook {
    id: u64,
    path: PathBuf,
}

#[derive(Default)]
struct FakeState {
    is_application_created: bool,
    create_application_count: usize,
    open_count: usize,
    close_count: usize,
    shutdown_count: usize,
    next_workbook_id: u64,
    workbooks: BTreeMap<u64, PathBuf>,
    fail_create: Option<ErrorCode>,
    fail_open: Option<ErrorCode>,
    fail_read_workbook: Option<ErrorCode>,
    fail_read_range: Option<ErrorCode>,
    fail_fingerprint: Option<ErrorCode>,
    fail_verify_ownership: Option<ErrorCode>,
    fail_close: Option<ErrorCode>,
    fail_shutdown: Option<ErrorCode>,
    shutdown_started: Option<Arc<AtomicBool>>,
    shutdown_release: Option<Arc<AtomicBool>>,
    malformed_workbook: bool,
    malformed_range: bool,
    range_address_override: Option<String>,
    last_range_selector: Option<RangeSelector>,
    last_range_mode: Option<ReadMode>,
}

struct FakeRunner {
    state: Arc<Mutex<FakeState>>,
}

impl FakeRunner {
    fn pair() -> (Self, Arc<Mutex<FakeState>>) {
        let state = Arc::new(Mutex::new(FakeState::default()));
        (
            Self {
                state: Arc::clone(&state),
            },
            state,
        )
    }

    fn set_failure(
        state: &Arc<Mutex<FakeState>>,
        setter: impl FnOnce(&mut FakeState) -> &mut Option<ErrorCode>,
        code: ErrorCode,
    ) -> TestResult {
        let mut state = lock_fake(state)?;
        *setter(&mut state) = Some(code);
        drop(state);
        Ok(())
    }
}

impl ExcelPlatformRunner for FakeRunner {
    type Workbook = FakeWorkbook;

    fn start() -> PlatformResult<Self> {
        Ok(Self {
            state: Arc::new(Mutex::new(FakeState::default())),
        })
    }

    fn create_application(&self) -> PlatformResult<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| platform_failure(ErrorCode::Fatal, "fake state poisoned"))?;
        if let Some(code) = state.fail_create {
            return Err(platform_failure(code, "scripted create failure"));
        }
        state.create_application_count = state.create_application_count.saturating_add(1);
        state.is_application_created = true;
        drop(state);
        Ok(())
    }

    fn open_read_only(&self, path: &Path) -> PlatformResult<Self::Workbook> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| platform_failure(ErrorCode::Fatal, "fake state poisoned"))?;
        if let Some(code) = state.fail_open {
            return Err(platform_failure(code, "scripted open failure"));
        }
        if !state.is_application_created || !state.workbooks.is_empty() {
            return Err(platform_failure(
                ErrorCode::ToolInvalidArgs,
                "fake workbook order is invalid",
            ));
        }
        state.next_workbook_id = state
            .next_workbook_id
            .checked_add(1)
            .ok_or_else(|| platform_failure(ErrorCode::Fatal, "fake workbook id overflowed"))?;
        let id = state.next_workbook_id;
        let workbook = FakeWorkbook {
            id,
            path: path.to_path_buf(),
        };
        state.workbooks.insert(id, path.to_path_buf());
        state.open_count = state.open_count.saturating_add(1);
        drop(state);
        Ok(workbook)
    }

    fn workbook_path<'a>(&self, workbook: &'a Self::Workbook) -> &'a Path {
        &workbook.path
    }

    fn owned_process(&self, _workbook: &Self::Workbook) -> PlatformResult<OwnedProcessIdentity> {
        Ok(OwnedProcessIdentity {
            process_id: 4242,
            start_ticks: 99,
            executable_path: "C:\\Program Files\\Microsoft Office\\root\\Office16\\EXCEL.EXE"
                .to_owned(),
        })
    }

    fn read_workbook(&self, workbook: &Self::Workbook) -> PlatformResult<WorkbookSnapshot> {
        let state = self
            .state
            .lock()
            .map_err(|_| platform_failure(ErrorCode::Fatal, "fake state poisoned"))?;
        if let Some(code) = state.fail_read_workbook {
            return Err(platform_failure(code, "scripted workbook read failure"));
        }
        let snapshot = WorkbookSnapshot {
            workbook: WorkbookIdentity {
                full_path: workbook.path.to_string_lossy().into_owned(),
                read_only: !state.malformed_workbook,
            },
            worksheets: vec![WorksheetInfo {
                name: "Sheet1".to_owned(),
                ordinal: 1,
                used_range_address: Some("A1:B2".to_owned()),
            }],
            named_ranges: vec![NamedRangeInfo {
                name: "SalesData".to_owned(),
                scope: NamedRangeScope::Workbook,
                refers_to: "=Sheet1!$A$1:$B$2".to_owned(),
            }],
            calculation: super::super::CalculationInfo {
                mode: "automatic".to_owned(),
                state: "done".to_owned(),
            },
        };
        drop(state);
        Ok(snapshot)
    }

    fn read_range(
        &self,
        _workbook: &Self::Workbook,
        selector: &RangeSelector,
        mode: ReadMode,
    ) -> PlatformResult<RangeSnapshot> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| platform_failure(ErrorCode::Fatal, "fake state poisoned"))?;
        state.last_range_selector = Some(selector.clone());
        state.last_range_mode = Some(mode);
        if let Some(code) = state.fail_read_range {
            return Err(platform_failure(code, "scripted range read failure"));
        }
        if state.malformed_range {
            let snapshot = RangeSnapshot {
                address: "A1:B1".to_owned(),
                rows: 2,
                columns: 1,
                cells: vec![RuntimeCell {
                    row: 1,
                    column: 1,
                    value_type: CellValueType::Number,
                    value_text: "1".to_owned(),
                }],
            };
            drop(state);
            return Ok(snapshot);
        }
        let (address, rows, columns, cells) = match selector {
            RangeSelector::NamedRange(_) => (
                "A1:B2".to_owned(),
                2,
                2,
                vec![
                    RuntimeCell {
                        row: 1,
                        column: 1,
                        value_type: CellValueType::Number,
                        value_text: "1".to_owned(),
                    },
                    RuntimeCell {
                        row: 1,
                        column: 2,
                        value_type: CellValueType::Number,
                        value_text: "2".to_owned(),
                    },
                    RuntimeCell {
                        row: 2,
                        column: 1,
                        value_type: CellValueType::Number,
                        value_text: "3".to_owned(),
                    },
                    RuntimeCell {
                        row: 2,
                        column: 2,
                        value_type: CellValueType::Number,
                        value_text: "4".to_owned(),
                    },
                ],
            ),
            RangeSelector::A1 { .. } => (
                "A1".to_owned(),
                1,
                1,
                vec![RuntimeCell {
                    row: 1,
                    column: 1,
                    value_type: CellValueType::Number,
                    value_text: "1".to_owned(),
                }],
            ),
        };
        let address = state.range_address_override.clone().unwrap_or(address);
        let snapshot = RangeSnapshot {
            address,
            rows,
            columns,
            cells,
        };
        drop(state);
        Ok(snapshot)
    }

    fn fingerprint(&self, _workbook: &Self::Workbook) -> PlatformResult<String> {
        let state = self
            .state
            .lock()
            .map_err(|_| platform_failure(ErrorCode::Fatal, "fake state poisoned"))?;
        if let Some(code) = state.fail_fingerprint {
            return Err(platform_failure(code, "scripted fingerprint failure"));
        }
        drop(state);
        Ok(FINGERPRINT.to_owned())
    }

    fn verify_ownership(&self, _workbook: &Self::Workbook) -> PlatformResult<()> {
        let state = self
            .state
            .lock()
            .map_err(|_| platform_failure(ErrorCode::Fatal, "fake state poisoned"))?;
        state.fail_verify_ownership.map_or(Ok(()), |code| {
            Err(platform_failure(code, "scripted ownership failure"))
        })
    }

    fn close_workbook(&self, workbook: &Self::Workbook) -> PlatformResult<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| platform_failure(ErrorCode::Fatal, "fake state poisoned"))?;
        if let Some(code) = state.fail_close {
            return Err(platform_failure(code, "scripted close failure"));
        }
        if state.workbooks.remove(&workbook.id).is_none() {
            return Err(platform_failure(
                ErrorCode::ToolInvalidArgs,
                "fake workbook is not open",
            ));
        }
        state.close_count = state.close_count.saturating_add(1);
        drop(state);
        Ok(())
    }

    fn shutdown(&mut self) -> PlatformResult<()> {
        let (failure, started, release) = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| platform_failure(ErrorCode::Fatal, "fake state poisoned"))?;
            state.shutdown_count = state.shutdown_count.saturating_add(1);
            (
                state.fail_shutdown,
                state.shutdown_started.clone(),
                state.shutdown_release.clone(),
            )
        };
        if let Some(code) = failure {
            return Err(platform_failure(code, "scripted shutdown failure"));
        }
        if let Some(started) = started {
            started.store(true, Ordering::Release);
        }
        if let Some(release) = release {
            let deadline = Instant::now() + Duration::from_secs(5);
            while !release.load(Ordering::Acquire) {
                if Instant::now() >= deadline {
                    return Err(platform_failure(
                        ErrorCode::Fatal,
                        "fake shutdown release deadline expired",
                    ));
                }
                thread::sleep(Duration::from_millis(5));
            }
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| platform_failure(ErrorCode::Fatal, "fake state poisoned"))?;
        state.is_application_created = false;
        state.workbooks.clear();
        drop(state);
        Ok(())
    }
}

fn platform_failure(code: ErrorCode, message: &str) -> PlatformError {
    PlatformError::new(code, message)
}

fn lock_fake(
    state: &Mutex<FakeState>,
) -> Result<std::sync::MutexGuard<'_, FakeState>, std::io::Error> {
    state
        .lock()
        .map_err(|_| std::io::Error::other("fake Excel state is poisoned"))
}

fn workbook_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("book.xlsx")
}

fn a1_selector() -> RangeSelector {
    let cell = super::super::A1Cell { row: 1, column: 1 };
    RangeSelector::A1 {
        worksheet: WorksheetSelector::Ordinal(1),
        start: cell,
        end: cell,
    }
}

#[test]
fn test_excel_t21_platform_runtime_repeated_cycles_converge() -> TestResult {
    let (runner, fake_state) = FakeRunner::pair();
    let runtime = PlatformExcelT21Runtime::with_runner(runner);
    let path = workbook_path();

    for expected_handle in 1..=32 {
        let opened = ExcelT21Runtime::open_read_only(&runtime, &path)?;
        assert_eq!(opened.handle.get(), expected_handle);
        assert_eq!(opened.owned_process.process_id, 4242);
        assert_eq!(opened.owned_process.start_ticks, 99);
        assert!(opened.owned_process.executable_path.ends_with("EXCEL.EXE"));
        assert_eq!(opened.worksheets.len(), 1);
        assert_eq!(runtime.active_workbook_count()?, 1);

        let metadata = ExcelT21Runtime::read_workbook(&runtime, opened.handle)?;
        assert_eq!(metadata.workbook.full_path, path.to_string_lossy());
        let range =
            ExcelT21Runtime::read_range(&runtime, opened.handle, &a1_selector(), ReadMode::Value2)?;
        assert_eq!(range.cells.len(), 1);
        assert_eq!(
            ExcelT21Runtime::fingerprint(&runtime, opened.handle)?,
            FINGERPRINT
        );
        ExcelT21Runtime::verify_ownership(&runtime, opened.handle, &opened.workbook)?;
        ExcelT21Runtime::close(&runtime, opened.handle)?;
        assert_eq!(runtime.active_workbook_count()?, 0);
    }

    {
        let state = lock_fake(&fake_state)?;
        assert_eq!(state.create_application_count, 1);
        assert_eq!(state.open_count, 32);
        assert_eq!(state.close_count, 32);
        assert!(state.workbooks.is_empty());
        drop(state);
    }
    runtime.shutdown()?;
    assert_eq!(lock_fake(&fake_state)?.shutdown_count, 1);
    Ok(())
}

#[test]
fn test_excel_t21_platform_runtime_rejects_second_open_and_preserves_close_failure() -> TestResult {
    let (runner, fake_state) = FakeRunner::pair();
    let runtime = PlatformExcelT21Runtime::with_runner(runner);
    let opened = ExcelT21Runtime::open_read_only(&runtime, &workbook_path())?;
    assert_eq!(
        ExcelT21Runtime::open_read_only(&runtime, &workbook_path())
            .err()
            .ok_or("second open unexpectedly passed")?
            .code(),
        ErrorCode::ToolInvalidArgs
    );

    FakeRunner::set_failure(
        &fake_state,
        |state| &mut state.fail_close,
        ErrorCode::CapabilityMissing,
    )?;
    assert_eq!(
        ExcelT21Runtime::close(&runtime, opened.handle)
            .err()
            .ok_or("scripted close failure unexpectedly passed")?
            .code(),
        ErrorCode::CapabilityMissing
    );
    assert_eq!(runtime.active_workbook_count()?, 1);

    lock_fake(&fake_state)?.fail_close = None;
    ExcelT21Runtime::close(&runtime, opened.handle)?;
    assert_eq!(runtime.active_workbook_count()?, 0);
    runtime.shutdown()?;
    Ok(())
}

#[test]
fn test_excel_t21_platform_runtime_shutdown_preserves_mapping_until_convergence() -> TestResult {
    let (runner, fake_state) = FakeRunner::pair();
    let runtime = PlatformExcelT21Runtime::with_runner(runner);
    let opened = ExcelT21Runtime::open_read_only(&runtime, &workbook_path())?;
    lock_fake(&fake_state)?.fail_shutdown = Some(ErrorCode::Fatal);

    let failure = ExcelT21Runtime::shutdown(&runtime)
        .err()
        .ok_or("scripted shutdown failure unexpectedly passed")?;
    assert_eq!(failure.code(), ErrorCode::Fatal);
    assert_eq!(runtime.active_workbook_count()?, 1);
    assert_eq!(lock_fake(&fake_state)?.shutdown_count, 1);

    let terminal = ExcelT21Runtime::read_workbook(&runtime, opened.handle)
        .err()
        .ok_or("terminal runtime unexpectedly read a workbook")?;
    assert_eq!(terminal.code(), ErrorCode::Fatal);
    assert!(terminal.to_string().contains("closing or terminal"));
    Ok(())
}

#[test]
fn test_excel_t21_platform_runtime_rejects_reads_while_shutdown_is_blocked() -> TestResult {
    let (runner, fake_state) = FakeRunner::pair();
    let runtime = Arc::new(PlatformExcelT21Runtime::with_runner(runner));
    let opened = ExcelT21Runtime::open_read_only(runtime.as_ref(), &workbook_path())?;
    let started = Arc::new(AtomicBool::new(false));
    let release = Arc::new(AtomicBool::new(false));
    {
        let mut state = lock_fake(&fake_state)?;
        state.shutdown_started = Some(Arc::clone(&started));
        state.shutdown_release = Some(Arc::clone(&release));
    }

    let shutdown_runtime = Arc::clone(&runtime);
    let shutdown_thread =
        thread::spawn(move || ExcelT21Runtime::shutdown(shutdown_runtime.as_ref()));
    let deadline = Instant::now() + Duration::from_secs(2);
    while !started.load(Ordering::Acquire) {
        if Instant::now() >= deadline {
            return Err("runtime shutdown did not enter the blocked runner".into());
        }
        thread::sleep(Duration::from_millis(5));
    }

    let read_started = Instant::now();
    let rejected = ExcelT21Runtime::read_workbook(runtime.as_ref(), opened.handle)
        .err()
        .ok_or("read during runtime shutdown unexpectedly passed")?;
    assert_eq!(rejected.code(), ErrorCode::Fatal);
    assert!(read_started.elapsed() < Duration::from_secs(1));

    release.store(true, Ordering::Release);
    shutdown_thread
        .join()
        .map_err(|_| std::io::Error::other("runtime shutdown test thread panicked"))??;
    assert_eq!(runtime.active_workbook_count()?, 0);
    Ok(())
}

#[test]
fn test_excel_t21_platform_runtime_preserves_typed_errors_and_stale_handles() -> TestResult {
    let (runner, fake_state) = FakeRunner::pair();
    let runtime = PlatformExcelT21Runtime::with_runner(runner);
    let opened = ExcelT21Runtime::open_read_only(&runtime, &workbook_path())?;

    FakeRunner::set_failure(
        &fake_state,
        |state| &mut state.fail_read_workbook,
        ErrorCode::TargetNotFound,
    )?;
    assert_eq!(
        ExcelT21Runtime::read_workbook(&runtime, opened.handle)
            .err()
            .ok_or("scripted workbook failure unexpectedly passed")?
            .code(),
        ErrorCode::TargetNotFound
    );
    lock_fake(&fake_state)?.fail_read_workbook = None;

    ExcelT21Runtime::close(&runtime, opened.handle)?;
    assert_eq!(
        ExcelT21Runtime::close(&runtime, opened.handle)
            .err()
            .ok_or("duplicate close unexpectedly passed")?
            .code(),
        ErrorCode::TargetNotFound
    );
    assert_eq!(
        ExcelT21Runtime::verify_ownership(&runtime, opened.handle, &opened.workbook)
            .err()
            .ok_or("stale ownership check unexpectedly passed")?
            .code(),
        ErrorCode::TargetNotFound
    );
    runtime.shutdown()?;
    Ok(())
}

#[test]
fn test_excel_t21_platform_runtime_cleans_up_failed_open_and_malformed_snapshot() -> TestResult {
    let (runner, fake_state) = FakeRunner::pair();
    let runtime = PlatformExcelT21Runtime::with_runner(runner);
    FakeRunner::set_failure(
        &fake_state,
        |state| &mut state.fail_read_workbook,
        ErrorCode::ToolInvalidArgs,
    )?;
    assert_eq!(
        ExcelT21Runtime::open_read_only(&runtime, &workbook_path())
            .err()
            .ok_or("scripted open metadata failure unexpectedly passed")?
            .code(),
        ErrorCode::ToolInvalidArgs
    );
    {
        let state = lock_fake(&fake_state)?;
        assert_eq!(state.open_count, 1);
        assert_eq!(state.close_count, 1);
        assert!(state.workbooks.is_empty());
        drop(state);
    }

    lock_fake(&fake_state)?.fail_read_workbook = None;
    lock_fake(&fake_state)?.malformed_workbook = true;
    assert_eq!(
        ExcelT21Runtime::open_read_only(&runtime, &workbook_path())
            .err()
            .ok_or("malformed workbook snapshot unexpectedly passed")?
            .code(),
        ErrorCode::ToolInvalidArgs
    );
    assert_eq!(lock_fake(&fake_state)?.close_count, 2);
    runtime.shutdown()?;
    Ok(())
}

#[test]
fn test_excel_t21_platform_runtime_terminates_when_open_cleanup_fails() -> TestResult {
    let (runner, fake_state) = FakeRunner::pair();
    let runtime = PlatformExcelT21Runtime::with_runner(runner);
    {
        let mut state = lock_fake(&fake_state)?;
        state.fail_read_workbook = Some(ErrorCode::ToolInvalidArgs);
        state.fail_close = Some(ErrorCode::CapabilityMissing);
    }

    let failure = ExcelT21Runtime::open_read_only(&runtime, &workbook_path())
        .err()
        .ok_or("open cleanup failure unexpectedly passed")?;
    assert_eq!(failure.code(), ErrorCode::Fatal);
    assert!(failure.to_string().contains("runner shutdown completed"));
    assert_eq!(runtime.active_workbook_count()?, 0);
    {
        let state = lock_fake(&fake_state)?;
        assert_eq!(state.close_count, 0);
        assert_eq!(state.shutdown_count, 1);
        assert!(state.workbooks.is_empty());
        drop(state);
    }
    let terminal = ExcelT21Runtime::open_read_only(&runtime, &workbook_path())
        .err()
        .ok_or("terminal runtime unexpectedly reopened")?;
    assert_eq!(terminal.code(), ErrorCode::Fatal);
    assert!(terminal.to_string().contains("terminal"));
    Ok(())
}

#[test]
fn test_excel_t21_platform_runtime_rejects_malformed_range_shape() -> TestResult {
    let (runner, fake_state) = FakeRunner::pair();
    let runtime = PlatformExcelT21Runtime::with_runner(runner);
    let opened = ExcelT21Runtime::open_read_only(&runtime, &workbook_path())?;
    lock_fake(&fake_state)?.malformed_range = true;
    assert_eq!(
        ExcelT21Runtime::read_range(&runtime, opened.handle, &a1_selector(), ReadMode::Value2)
            .err()
            .ok_or("malformed range unexpectedly passed")?
            .code(),
        ErrorCode::ToolInvalidArgs
    );
    lock_fake(&fake_state)?.malformed_range = false;
    ExcelT21Runtime::close(&runtime, opened.handle)?;
    runtime.shutdown()?;
    Ok(())
}

#[test]
fn test_excel_t21_platform_runtime_preserves_range_fingerprint_and_ownership_errors() -> TestResult
{
    let (runner, fake_state) = FakeRunner::pair();
    let runtime = PlatformExcelT21Runtime::with_runner(runner);
    let opened = ExcelT21Runtime::open_read_only(&runtime, &workbook_path())?;

    FakeRunner::set_failure(
        &fake_state,
        |state| &mut state.fail_read_range,
        ErrorCode::TargetAmbiguous,
    )?;
    assert_eq!(
        ExcelT21Runtime::read_range(&runtime, opened.handle, &a1_selector(), ReadMode::Value2)
            .err()
            .ok_or("scripted range failure unexpectedly passed")?
            .code(),
        ErrorCode::TargetAmbiguous
    );
    lock_fake(&fake_state)?.fail_read_range = None;

    FakeRunner::set_failure(
        &fake_state,
        |state| &mut state.fail_fingerprint,
        ErrorCode::VerifyFailed,
    )?;
    assert_eq!(
        ExcelT21Runtime::fingerprint(&runtime, opened.handle)
            .err()
            .ok_or("scripted fingerprint failure unexpectedly passed")?
            .code(),
        ErrorCode::VerifyFailed
    );
    lock_fake(&fake_state)?.fail_fingerprint = None;

    FakeRunner::set_failure(
        &fake_state,
        |state| &mut state.fail_verify_ownership,
        ErrorCode::TargetUnresponsive,
    )?;
    assert_eq!(
        ExcelT21Runtime::verify_ownership(&runtime, opened.handle, &opened.workbook)
            .err()
            .ok_or("scripted ownership failure unexpectedly passed")?
            .code(),
        ErrorCode::TargetUnresponsive
    );
    lock_fake(&fake_state)?.fail_verify_ownership = None;

    ExcelT21Runtime::close(&runtime, opened.handle)?;
    runtime.shutdown()?;
    Ok(())
}

#[test]
fn test_excel_t21_platform_runtime_passes_selector_and_mode_to_port() -> TestResult {
    let (runner, fake_state) = FakeRunner::pair();
    let runtime = PlatformExcelT21Runtime::with_runner(runner);
    let opened = ExcelT21Runtime::open_read_only(&runtime, &workbook_path())?;

    let named_selector = RangeSelector::NamedRange("SalesData".to_owned());
    ExcelT21Runtime::read_range(&runtime, opened.handle, &named_selector, ReadMode::Formula)?;
    {
        let state = lock_fake(&fake_state)?;
        assert_eq!(state.last_range_selector.as_ref(), Some(&named_selector));
        assert_eq!(state.last_range_mode, Some(ReadMode::Formula));
    }

    let a1_selector = RangeSelector::A1 {
        worksheet: WorksheetSelector::Name("Sheet1".to_owned()),
        start: super::super::A1Cell { row: 1, column: 1 },
        end: super::super::A1Cell { row: 1, column: 1 },
    };
    ExcelT21Runtime::read_range(&runtime, opened.handle, &a1_selector, ReadMode::Text)?;
    {
        let state = lock_fake(&fake_state)?;
        assert_eq!(state.last_range_selector.as_ref(), Some(&a1_selector));
        assert_eq!(state.last_range_mode, Some(ReadMode::Text));
    }

    ExcelT21Runtime::close(&runtime, opened.handle)?;
    runtime.shutdown()?;
    Ok(())
}

#[test]
fn test_excel_t21_platform_runtime_rejects_a1_address_drift() -> TestResult {
    let (runner, fake_state) = FakeRunner::pair();
    let runtime = PlatformExcelT21Runtime::with_runner(runner);
    let opened = ExcelT21Runtime::open_read_only(&runtime, &workbook_path())?;
    lock_fake(&fake_state)?.range_address_override = Some("B1".to_owned());
    assert_eq!(
        ExcelT21Runtime::read_range(&runtime, opened.handle, &a1_selector(), ReadMode::Value2)
            .err()
            .ok_or("A1 address drift unexpectedly passed")?
            .code(),
        ErrorCode::ToolInvalidArgs
    );
    lock_fake(&fake_state)?.range_address_override = None;
    ExcelT21Runtime::close(&runtime, opened.handle)?;
    runtime.shutdown()?;
    Ok(())
}

#[test]
fn test_excel_t21_platform_runtime_mapping_is_total() {
    assert_eq!(map_read_mode(ReadMode::Value2), ExcelReadMode::Value2);
    assert_eq!(map_read_mode(ReadMode::Formula), ExcelReadMode::Formula);
    assert_eq!(map_read_mode(ReadMode::Text), ExcelReadMode::Text);

    assert_eq!(
        map_cell_value_type(ExcelCellValueType::Blank),
        CellValueType::Blank
    );
    assert_eq!(
        map_cell_value_type(ExcelCellValueType::Number),
        CellValueType::Number
    );
    assert_eq!(
        map_cell_value_type(ExcelCellValueType::Text),
        CellValueType::Text
    );
    assert_eq!(
        map_cell_value_type(ExcelCellValueType::Boolean),
        CellValueType::Boolean
    );
    assert_eq!(
        map_cell_value_type(ExcelCellValueType::Error),
        CellValueType::Error
    );

    assert_eq!(
        map_range_selector(&RangeSelector::NamedRange("SalesData".to_owned())),
        ExcelRangeSelector::NamedRange("SalesData".to_owned())
    );
    assert!(matches!(
        map_range_selector(&a1_selector()),
        ExcelRangeSelector::A1 {
            worksheet: ExcelWorksheetSelector::Ordinal(1),
            start_row: 1,
            start_column: 1,
            end_row: 1,
            end_column: 1,
        }
    ));
}

#[test]
#[ignore = "TASK-403 manual real-COM smoke; requires a human-selected .xlsx and no existing Excel"]
fn test_excel_t21_platform_runtime_windows_real_com_cycles() -> TestResult {
    let path = std::env::var("EXCEL_T21_ACCEPTANCE_WORKBOOK")?;
    let path = PathBuf::from(path);
    let runtime = PlatformExcelT21Runtime::<ExcelComRunner>::start()
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    for _ in 0..3 {
        let opened = ExcelT21Runtime::open_read_only(&runtime, &path)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        let metadata = ExcelT21Runtime::read_workbook(&runtime, opened.handle)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        assert!(!metadata.worksheets.is_empty());
        let _range =
            ExcelT21Runtime::read_range(&runtime, opened.handle, &a1_selector(), ReadMode::Value2)
                .map_err(|error| std::io::Error::other(error.to_string()))?;
        let _fingerprint = ExcelT21Runtime::fingerprint(&runtime, opened.handle)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        ExcelT21Runtime::verify_ownership(&runtime, opened.handle, &opened.workbook)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        ExcelT21Runtime::close(&runtime, opened.handle)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
    }
    runtime
        .shutdown()
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    Ok(())
}
