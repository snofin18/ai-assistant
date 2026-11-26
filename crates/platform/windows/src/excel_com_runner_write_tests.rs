//! Focused tests for the test-only Excel write calibration surface.

use std::sync::Mutex;
#[cfg(windows)]
use std::time::{SystemTime, UNIX_EPOCH};

use super::*;
#[cfg(windows)]
use crate::excel_process::ExcelProcessIdentity;
#[cfg(windows)]
use windows::Win32::Foundation::CloseHandle;

#[derive(Debug, Clone, PartialEq)]
enum WriteEvent {
    OpenWritable,
    WriteScalar(ExcelScalarValue),
    Recalculate,
    Save,
}

struct WriteAcceptanceBackend {
    events: Arc<Mutex<Vec<WriteEvent>>>,
    fail_save: bool,
}

impl ExcelComBackend for WriteAcceptanceBackend {
    fn create_application(&mut self) -> PlatformResult<()> {
        Ok(())
    }

    fn application_identity(&mut self) -> PlatformResult<ExcelOwnedProcessIdentity> {
        test_identity(407)
    }

    fn execute_write_acceptance(
        &mut self,
        request: &ExcelWriteAcceptanceRequest,
    ) -> PlatformResult<()> {
        let event = match request {
            ExcelWriteAcceptanceRequest::OpenWritable { .. } => WriteEvent::OpenWritable,
            ExcelWriteAcceptanceRequest::WriteScalar { scalar, .. } => {
                WriteEvent::WriteScalar(scalar.clone())
            }
            ExcelWriteAcceptanceRequest::Recalculate { .. } => WriteEvent::Recalculate,
            ExcelWriteAcceptanceRequest::Save { .. } if self.fail_save => {
                return Err(PlatformError::new(
                    ErrorCode::Fatal,
                    "injected save failure",
                ));
            }
            ExcelWriteAcceptanceRequest::Save { .. } => WriteEvent::Save,
        };
        self.events.lock().expect("write events").push(event);
        Ok(())
    }

    fn close_workbook(&mut self) -> PlatformResult<()> {
        Ok(())
    }

    fn quit_application(&mut self) -> PlatformResult<()> {
        Ok(())
    }
}

fn write_runner(fail_save: bool) -> (ExcelComRunner, Arc<Mutex<Vec<WriteEvent>>>) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let backend = WriteAcceptanceBackend {
        events: Arc::clone(&events),
        fail_save,
    };
    let runner = ExcelComRunner::spawn(move || backend).expect("write runner");
    (runner, events)
}

fn a1_cell(row: u32, column: u32) -> ExcelRangeSelector {
    ExcelRangeSelector::A1 {
        worksheet: ExcelWorksheetSelector::Name("Sheet1".to_owned()),
        start_row: row,
        start_column: column,
        end_row: row,
        end_column: column,
    }
}

#[test]
fn test_write_acceptance_routes_closed_sequence_on_worker() {
    let workbook_path = temporary_workbook();
    let (mut runner, events) = write_runner(false);
    runner.create_application().expect("create application");
    let workbook = runner
        .open_writable_for_acceptance(&workbook_path)
        .expect("open writable");
    runner
        .write_scalar_for_acceptance(&workbook, a1_cell(1, 1), ExcelScalarValue::Number(41.0))
        .expect("write scalar");
    runner
        .recalculate_for_acceptance(&workbook)
        .expect("recalculate");
    runner
        .save_workbook_for_acceptance(&workbook)
        .expect("save workbook");
    runner.close_workbook(&workbook).expect("close workbook");
    runner.shutdown().expect("shutdown");

    assert_eq!(
        events.lock().expect("write events").as_slice(),
        [
            WriteEvent::OpenWritable,
            WriteEvent::WriteScalar(ExcelScalarValue::Number(41.0)),
            WriteEvent::Recalculate,
            WriteEvent::Save,
        ]
    );
    std::fs::remove_file(workbook_path).expect("remove temporary workbook");
}

#[test]
fn test_write_acceptance_rejects_stale_handle_before_backend() {
    let workbook_path = temporary_workbook();
    let (mut runner, events) = write_runner(false);
    runner.create_application().expect("create application");
    let workbook = runner
        .open_writable_for_acceptance(&workbook_path)
        .expect("open writable");
    runner.close_workbook(&workbook).expect("close workbook");

    let error = runner
        .recalculate_for_acceptance(&workbook)
        .expect_err("stale handle");
    assert_eq!(error.code(), ErrorCode::ToolInvalidArgs);
    assert_eq!(
        events.lock().expect("write events").as_slice(),
        [WriteEvent::OpenWritable]
    );

    runner.shutdown().expect("shutdown");
    std::fs::remove_file(workbook_path).expect("remove temporary workbook");
}

#[test]
fn test_write_acceptance_rejects_foreign_runner_handle_before_backend() {
    let first_path = temporary_workbook();
    let second_path = temporary_workbook();
    let (mut first, first_events) = write_runner(false);
    let (mut second, _second_events) = write_runner(false);
    first
        .create_application()
        .expect("create first application");
    second
        .create_application()
        .expect("create second application");
    let _first_workbook = first
        .open_writable_for_acceptance(&first_path)
        .expect("open first writable");
    let second_workbook = second
        .open_writable_for_acceptance(&second_path)
        .expect("open second writable");

    let error = first
        .write_scalar_for_acceptance(
            &second_workbook,
            a1_cell(1, 1),
            ExcelScalarValue::Number(1.0),
        )
        .expect_err("foreign runner handle");
    assert_eq!(error.code(), ErrorCode::ToolInvalidArgs);
    assert_eq!(
        first_events.lock().expect("write events").as_slice(),
        [WriteEvent::OpenWritable]
    );

    first.shutdown().expect("shutdown first");
    second.shutdown().expect("shutdown second");
    std::fs::remove_file(first_path).expect("remove first workbook");
    std::fs::remove_file(second_path).expect("remove second workbook");
}

#[test]
fn test_write_acceptance_save_failure_preserves_explicit_error() {
    let workbook_path = temporary_workbook();
    let (mut runner, events) = write_runner(true);
    runner.create_application().expect("create application");
    let workbook = runner
        .open_writable_for_acceptance(&workbook_path)
        .expect("open writable");
    let error = runner
        .save_workbook_for_acceptance(&workbook)
        .expect_err("save failure");
    assert_eq!(error.code(), ErrorCode::Fatal);
    assert_eq!(error.message(), "injected save failure");
    runner.shutdown().expect("shutdown");

    assert_eq!(
        events.lock().expect("write events").as_slice(),
        [WriteEvent::OpenWritable]
    );
    std::fs::remove_file(workbook_path).expect("remove temporary workbook");
}

#[cfg(windows)]
pub(super) fn terminate_verified_process(identity: &ExcelProcessIdentity) -> PlatformResult<()> {
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE, TerminateProcess,
    };

    // SAFETY: OpenProcess is called with query/terminate access for one
    // explicitly identified process and returns an owned handle.
    let process = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE,
            false,
            identity.process_id,
        )
    }
    .map_err(|error| {
        PlatformError::new(
            ErrorCode::Fatal,
            format!(
                "OpenProcess failed for owned Excel {}: {}",
                identity.process_id,
                error.message()
            ),
        )
    })?;
    let process = RawProcessHandle(process);
    let (creation_time, executable_path) =
        read_process_identity_from_handle(process.0, identity.process_id)?;
    if creation_time != identity.start_time_100ns
        || !executable_path.eq_ignore_ascii_case(&identity.executable_path)
    {
        return Err(PlatformError::new(
            ErrorCode::Fatal,
            format!(
                "owned Excel identity changed before verified termination: pid={} expected_start={} actual_start={creation_time} expected_path={} actual_path={executable_path}",
                identity.process_id, identity.start_time_100ns, identity.executable_path
            ),
        ));
    }
    // SAFETY: the same open handle was verified above and remains live for
    // this synchronous termination call.
    unsafe { TerminateProcess(process.0, 1) }.map_err(|error| {
        PlatformError::new(
            ErrorCode::Fatal,
            format!(
                "TerminateProcess failed for owned Excel {}: {}",
                identity.process_id,
                error.message()
            ),
        )
    })
}

#[cfg(windows)]
fn read_process_identity_from_handle(
    process: windows::Win32::Foundation::HANDLE,
    process_id: u32,
) -> PlatformResult<(u64, String)> {
    use windows::Win32::System::Threading::{
        GetProcessTimes, PROCESS_NAME_FORMAT, QueryFullProcessImageNameW,
    };
    use windows::core::PWSTR;

    let mut creation = windows::Win32::Foundation::FILETIME::default();
    let mut exit = windows::Win32::Foundation::FILETIME::default();
    let mut kernel = windows::Win32::Foundation::FILETIME::default();
    let mut user = windows::Win32::Foundation::FILETIME::default();
    // SAFETY: the process handle grants query access and all FILETIME outputs
    // point to distinct live stack values.
    unsafe {
        GetProcessTimes(
            process,
            &raw mut creation,
            &raw mut exit,
            &raw mut kernel,
            &raw mut user,
        )
    }
    .map_err(|error| {
        PlatformError::new(
            ErrorCode::Fatal,
            format!(
                "GetProcessTimes failed for owned Excel {process_id}: {}",
                error.message()
            ),
        )
    })?;
    let mut path_buffer = vec![0_u16; 32_768];
    let mut path_length = u32::try_from(path_buffer.len()).map_err(|_| {
        PlatformError::new(
            ErrorCode::Fatal,
            "process path buffer length does not fit u32",
        )
    })?;
    // SAFETY: the process handle is live, path_buffer is writable, and its
    // capacity is passed through a live u32 out-parameter.
    unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_FORMAT(0),
            PWSTR(path_buffer.as_mut_ptr()),
            &raw mut path_length,
        )
    }
    .map_err(|error| {
        PlatformError::new(
            ErrorCode::Fatal,
            format!(
                "QueryFullProcessImageNameW failed for owned Excel {process_id}: {}",
                error.message()
            ),
        )
    })?;
    let path_length = usize::try_from(path_length).map_err(|_| {
        PlatformError::new(ErrorCode::Fatal, "process path length does not fit usize")
    })?;
    let executable_path =
        String::from_utf16_lossy(path_buffer.get(..path_length).ok_or_else(|| {
            PlatformError::new(ErrorCode::Fatal, "process path length exceeded its buffer")
        })?);
    let creation_time =
        (u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime);
    Ok((creation_time, executable_path))
}

#[cfg(windows)]
struct RawProcessHandle(windows::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl Drop for RawProcessHandle {
    fn drop(&mut self) {
        // SAFETY: this handle is owned by the wrapper and is closed exactly once.
        if let Err(error) = unsafe { CloseHandle(self.0) } {
            eprintln!("TASK-409 process handle close failed: {}", error.message());
        }
    }
}

#[cfg(windows)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FailureStage {
    Open,
    Write,
    Recalculate,
    Save,
}

#[cfg(windows)]
struct FailureBackend {
    fail_at: FailureStage,
}

#[cfg(windows)]
impl ExcelComBackend for FailureBackend {
    fn create_application(&mut self) -> PlatformResult<()> {
        Ok(())
    }

    fn application_identity(&mut self) -> PlatformResult<ExcelOwnedProcessIdentity> {
        super::test_identity(409)
    }

    fn execute_write_acceptance(
        &mut self,
        request: &ExcelWriteAcceptanceRequest,
    ) -> PlatformResult<()> {
        let stage = match request {
            ExcelWriteAcceptanceRequest::OpenWritable { .. } => FailureStage::Open,
            ExcelWriteAcceptanceRequest::WriteScalar { .. } => FailureStage::Write,
            ExcelWriteAcceptanceRequest::Recalculate { .. } => FailureStage::Recalculate,
            ExcelWriteAcceptanceRequest::Save { .. } => FailureStage::Save,
        };
        if stage == self.fail_at {
            return Err(PlatformError::new(
                ErrorCode::Fatal,
                format!("TASK-409 injected fake {stage:?} failure"),
            ));
        }
        Ok(())
    }

    fn close_workbook(&mut self) -> PlatformResult<()> {
        Ok(())
    }

    fn quit_application(&mut self) -> PlatformResult<()> {
        Ok(())
    }
}

#[cfg(windows)]
pub(super) fn failure_runner(fail_at: FailureStage) -> PlatformResult<ExcelComRunner> {
    let backend = FailureBackend { fail_at };
    ExcelComRunner::spawn(move || backend)
}

#[cfg(windows)]
struct TemporaryCalibrationFiles {
    workbook: PathBuf,
    shadow: PathBuf,
}

#[cfg(windows)]
impl TemporaryCalibrationFiles {
    fn from_source(source: &Path) -> PlatformResult<Self> {
        let source = crate::excel_process::validate_workbook_path(source)?;
        let directory =
            crate::excel_process::validate_local_workbook_directory(&std::env::temp_dir())?;
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let workbook = directory.join(format!(
            "assistant-task407-{}-{nonce}.xlsx",
            std::process::id()
        ));
        let shadow = directory.join(format!(
            "assistant-task407-{}-{nonce}.shadow.xlsx",
            std::process::id()
        ));
        std::fs::copy(&source, &workbook)
            .map_err(|error| fixture_io_error("create working copy", &workbook, &error))?;
        if let Err(copy_error) = std::fs::copy(&source, &shadow) {
            return match std::fs::remove_file(&workbook) {
                Ok(()) => Err(fixture_io_error("create shadow copy", &shadow, &copy_error)),
                Err(cleanup_error) => Err(PlatformError::new(
                    ErrorCode::Fatal,
                    format!(
                        "create shadow copy failed for {}: {copy_error}; cleanup of {} also failed: {cleanup_error}",
                        shadow.display(),
                        workbook.display()
                    ),
                )),
            };
        }
        let workbook = crate::excel_process::validate_workbook_path(&workbook)?;
        let shadow = crate::excel_process::validate_workbook_path(&shadow)?;
        Ok(Self { workbook, shadow })
    }

    fn cleanup(&self) -> PlatformResult<()> {
        for path in [&self.workbook, &self.shadow] {
            if path.exists() {
                std::fs::remove_file(path).map_err(|error| {
                    fixture_io_error("remove calibration fixture", path, &error)
                })?;
            }
        }
        Ok(())
    }
}

#[cfg(windows)]
impl Drop for TemporaryCalibrationFiles {
    fn drop(&mut self) {
        if let Err(error) = self.cleanup() {
            eprintln!("TASK-407 temporary cleanup failed: {error}");
        }
    }
}

#[cfg(windows)]
fn fixture_io_error(context: &str, path: &Path, error: &std::io::Error) -> PlatformError {
    PlatformError::new(
        ErrorCode::Fatal,
        format!("{context} failed for {}: {error}", path.display()),
    )
}

#[cfg(windows)]
fn read_one_cell(
    runner: &ExcelComRunner,
    workbook: &ExcelWorkbookHandle,
    row: u32,
    column: u32,
    mode: ExcelReadMode,
) -> (ExcelCellValueType, String) {
    let snapshot = runner
        .read_range(workbook, a1_cell(row, column), mode)
        .expect("read cell");
    assert_eq!(snapshot.cells().len(), 1);
    let cell = &snapshot.cells()[0];
    eprintln!(
        "TASK-407 raw mode={mode:?} row={row} column={column} type={:?} value={}",
        cell.value_type(),
        cell.value_text()
    );
    (cell.value_type(), cell.value_text().to_owned())
}

#[cfg(windows)]
fn assert_value2(
    runner: &ExcelComRunner,
    workbook: &ExcelWorkbookHandle,
    row: u32,
    column: u32,
    expected_type: ExcelCellValueType,
    expected_text: &str,
) {
    let (value_type, value_text) =
        read_one_cell(runner, workbook, row, column, ExcelReadMode::Value2);
    assert_eq!(value_type, expected_type);
    assert_eq!(value_text, expected_text);
}

#[cfg(windows)]
fn assert_formula(
    runner: &ExcelComRunner,
    workbook: &ExcelWorkbookHandle,
    row: u32,
    column: u32,
    expected_type: ExcelCellValueType,
    expected_text: &str,
) {
    let (value_type, value_text) =
        read_one_cell(runner, workbook, row, column, ExcelReadMode::Formula);
    assert_eq!(value_type, expected_type);
    assert_eq!(value_text, expected_text);
}

#[cfg(windows)]
fn wait_for_owned_process_exit(process_id: u32) {
    let mut remaining = true;
    for attempt in 0..450 {
        if !excel_process_ids()
            .expect("Excel process inventory")
            .contains(&process_id)
        {
            remaining = false;
            break;
        }
        if attempt > 0 && attempt % 50 == 0 {
            eprintln!(
                "TASK-407 waiting for owned Excel {process_id} teardown: {}s",
                attempt / 10
            );
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
        "TASK-407 acceptance must leave no EXCEL.EXE process"
    );
}

#[cfg(windows)]
type WriteCase = (
    u32,
    u32,
    ExcelScalarValue,
    ExcelCellValueType,
    &'static str,
    ExcelCellValueType,
    &'static str,
);

#[cfg(windows)]
fn single_cell_write_cases() -> [WriteCase; 5] {
    [
        (
            1,
            1,
            ExcelScalarValue::Number(41.0),
            ExcelCellValueType::Number,
            "41",
            ExcelCellValueType::Text,
            "41",
        ),
        (
            1,
            2,
            ExcelScalarValue::Number(1.0),
            ExcelCellValueType::Number,
            "1",
            ExcelCellValueType::Text,
            "1",
        ),
        (
            1,
            3,
            ExcelScalarValue::Boolean(false),
            ExcelCellValueType::Boolean,
            "false",
            ExcelCellValueType::Text,
            "FALSE",
        ),
        (
            1,
            4,
            ExcelScalarValue::Text("TASK-407".to_owned()),
            ExcelCellValueType::Text,
            "TASK-407",
            ExcelCellValueType::Text,
            "TASK-407",
        ),
        (
            1,
            5,
            ExcelScalarValue::Blank,
            ExcelCellValueType::Blank,
            "",
            ExcelCellValueType::Blank,
            "",
        ),
    ]
}

#[cfg(windows)]
fn verify_initial_fixture(runner: &ExcelComRunner, workbook: &ExcelWorkbookHandle) {
    for (row, column, expected_type, expected_text) in [
        (1, 1, ExcelCellValueType::Number, "2"),
        (1, 2, ExcelCellValueType::Number, "3"),
        (1, 3, ExcelCellValueType::Boolean, "true"),
        (1, 4, ExcelCellValueType::Text, "hello"),
        (1, 5, ExcelCellValueType::Blank, ""),
        (1, 6, ExcelCellValueType::Number, "5"),
    ] {
        assert_value2(runner, workbook, row, column, expected_type, expected_text);
    }
    assert_formula(runner, workbook, 1, 6, ExcelCellValueType::Text, "=A1+B1");
}

#[cfg(windows)]
fn calibrate_single_cell_writes(runner: &ExcelComRunner, workbook: &ExcelWorkbookHandle) {
    for (
        row,
        column,
        scalar,
        expected_type,
        expected_text,
        expected_formula_type,
        expected_formula_text,
    ) in single_cell_write_cases()
    {
        runner
            .write_scalar_for_acceptance(workbook, a1_cell(row, column), scalar)
            .expect("write scalar");
        runner
            .recalculate_for_acceptance(workbook)
            .expect("recalculate");
        assert_value2(runner, workbook, row, column, expected_type, expected_text);
        assert_formula(
            runner,
            workbook,
            row,
            column,
            expected_formula_type,
            expected_formula_text,
        );
    }
    assert_value2(runner, workbook, 1, 6, ExcelCellValueType::Number, "42");
    assert_formula(runner, workbook, 1, 6, ExcelCellValueType::Text, "=A1+B1");
}

#[cfg(windows)]
fn calibrate_two_cell_range(runner: &ExcelComRunner, workbook: &ExcelWorkbookHandle) {
    let two_cell_range = ExcelRangeSelector::A1 {
        worksheet: ExcelWorksheetSelector::Name("Sheet1".to_owned()),
        start_row: 2,
        start_column: 1,
        end_row: 2,
        end_column: 2,
    };
    runner
        .write_scalar_for_acceptance(
            workbook,
            two_cell_range.clone(),
            ExcelScalarValue::Number(9.0),
        )
        .expect("write two-cell range");
    runner
        .recalculate_for_acceptance(workbook)
        .expect("recalculate two-cell range");
    let range_values = runner
        .read_range(workbook, two_cell_range.clone(), ExcelReadMode::Value2)
        .expect("read two-cell Value2 range");
    let range_formulas = runner
        .read_range(workbook, two_cell_range, ExcelReadMode::Formula)
        .expect("read two-cell Formula range");
    eprintln!("TASK-407 raw two-cell Value2={range_values:?} Formula={range_formulas:?}");
    for cells in [range_values.cells(), range_formulas.cells()] {
        assert_eq!(cells.len(), 2);
        assert_eq!(cells[0].value_text(), "9");
        assert_eq!(cells[1].value_text(), "9");
    }
    assert_eq!(
        range_values.cells()[0].value_type(),
        ExcelCellValueType::Number
    );
    assert_eq!(
        range_values.cells()[1].value_type(),
        ExcelCellValueType::Number
    );
    assert_eq!(
        range_formulas.cells()[0].value_type(),
        ExcelCellValueType::Text
    );
    assert_eq!(
        range_formulas.cells()[1].value_type(),
        ExcelCellValueType::Text
    );
}

#[cfg(windows)]
fn save_close_and_restore(
    guard: &mut RealComAcceptanceGuard,
    workbook: &ExcelWorkbookHandle,
    files: &TemporaryCalibrationFiles,
    process_id: u32,
    original_digest: &str,
) -> String {
    guard
        .runner()
        .save_workbook_for_acceptance(workbook)
        .expect("save working copy");
    guard
        .runner()
        .close_workbook(workbook)
        .expect("close working copy");
    let stale = guard
        .runner()
        .read_range(workbook, a1_cell(1, 1), ExcelReadMode::Value2)
        .expect_err("closed handle must fail");
    assert_eq!(stale.code(), ErrorCode::ToolInvalidArgs);
    guard.shutdown().expect("shutdown first Excel runner");
    wait_for_owned_process_exit(process_id);

    let written_bytes = std::fs::read(&files.workbook).expect("read written working copy");
    let written_digest = crate::digest::sha256_hex_bytes(&written_bytes);
    eprintln!("TASK-407 written_digest={written_digest}");
    assert_ne!(
        original_digest, written_digest,
        "saved scalar writes must change the workbook digest"
    );
    std::fs::copy(&files.shadow, &files.workbook).expect("restore working copy from shadow");
    let restored_bytes = std::fs::read(&files.workbook).expect("read restored working copy");
    let restored_digest = crate::digest::sha256_hex_bytes(&restored_bytes);
    eprintln!("TASK-407 restored_digest={restored_digest}");
    assert_eq!(
        original_digest, restored_digest,
        "shadow restore must reproduce the original bytes"
    );
    written_digest
}

#[cfg(windows)]
fn verify_restored_readback(files: &TemporaryCalibrationFiles) {
    assert!(
        excel_process_ids()
            .expect("Excel process inventory")
            .is_empty(),
        "restored readback needs a clean Excel process baseline"
    );
    let mut guard =
        RealComAcceptanceGuard::new(ExcelComRunner::start().expect("start readback runner"));
    guard
        .runner()
        .create_application()
        .expect("create readback Excel application");
    let process_id = guard
        .runner()
        .application_process_id()
        .expect("readback process id");
    let identity = process_identity(process_id)
        .expect("query readback identity")
        .expect("readback identity");
    guard.set_owned_process(identity);
    let workbook = guard
        .runner()
        .open_read_only(&files.workbook)
        .expect("open restored working copy read-only");
    for (row, column, expected_type, expected_text) in [
        (1, 1, ExcelCellValueType::Number, "2"),
        (1, 2, ExcelCellValueType::Number, "3"),
        (1, 6, ExcelCellValueType::Number, "5"),
    ] {
        assert_value2(
            guard.runner(),
            &workbook,
            row,
            column,
            expected_type,
            expected_text,
        );
    }
    assert_formula(
        guard.runner(),
        &workbook,
        1,
        6,
        ExcelCellValueType::Text,
        "=A1+B1",
    );
    guard
        .runner()
        .close_workbook(&workbook)
        .expect("close restored workbook");
    guard.shutdown().expect("shutdown readback Excel runner");
    wait_for_owned_process_exit(process_id);
}

#[cfg(windows)]
#[test]
#[ignore = "TASK-407 human-present real Excel COM write calibration"]
fn test_windows_excel_real_com_write_calibration() {
    assert!(
        excel_process_ids()
            .expect("Excel process inventory")
            .is_empty(),
        "close every EXCEL.EXE before running the TASK-407 acceptance"
    );
    let source_path = std::env::var_os("EXCEL_T22_ACCEPTANCE_WORKBOOK")
        .map(PathBuf::from)
        .expect("EXCEL_T22_ACCEPTANCE_WORKBOOK must name one local .xlsx fixture");
    let source_path = crate::excel_process::validate_workbook_path(&source_path)
        .expect("validate local acceptance fixture");
    let original_digest =
        crate::digest::sha256_hex_bytes(&std::fs::read(&source_path).expect("read source fixture"));
    let files =
        TemporaryCalibrationFiles::from_source(&source_path).expect("create calibration copies");

    let mut guard =
        RealComAcceptanceGuard::new(ExcelComRunner::start().expect("start first Excel runner"));
    guard
        .runner()
        .create_application()
        .expect("create first owned Excel application");
    let process_id = guard
        .runner()
        .application_process_id()
        .expect("first owned process id");
    let identity = process_identity(process_id)
        .expect("query first owned identity")
        .expect("first owned identity");
    guard.set_owned_process(identity.clone());
    eprintln!(
        "TASK-407 source={} working={} shadow={} pid={} start={} exe={} original_digest={}",
        source_path.display(),
        files.workbook.display(),
        files.shadow.display(),
        process_id,
        identity.start_time_100ns,
        identity.executable_path,
        original_digest
    );

    let workbook = guard
        .runner()
        .open_writable_for_acceptance(&files.workbook)
        .expect("open working copy writable");
    verify_initial_fixture(guard.runner(), &workbook);
    calibrate_single_cell_writes(guard.runner(), &workbook);
    calibrate_two_cell_range(guard.runner(), &workbook);
    let written_digest =
        save_close_and_restore(&mut guard, &workbook, &files, process_id, &original_digest);
    eprintln!("TASK-407 confirmed written_digest={written_digest}");
    let source_digest_after = crate::digest::sha256_hex_bytes(
        &std::fs::read(&source_path).expect("read source fixture after acceptance"),
    );
    assert_eq!(
        original_digest, source_digest_after,
        "source fixture must remain byte-identical"
    );
    verify_restored_readback(&files);
    files.cleanup().expect("remove calibration fixtures");
}
