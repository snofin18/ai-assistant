//! Test-only repeated real-COM write convergence and watchdog evidence.
//!
//! Responsibilities:
//! - run the writable Excel acceptance against fresh local copies and fresh
//!   owned Excel processes;
//! - leave an external ownership journal before risky COM open/close/quit paths;
//! - expose deterministic failure and hang checkpoints for an independent
//!   PowerShell watchdog.
//!
//! Boundaries:
//! - compiled only in crate tests;
//! - does not add a production runner method, Tool, rollback recipe, or `ToolBus`
//!   registration;
//! - never opens a user workbook, network path, macro workbook, or existing
//!   Excel instance.
//!
//! Invariants:
//! 1. every normal run uses a new worker, owned process, session, and workbook
//!    copy;
//! 2. the ownership journal exists before writable open and survives a killed
//!    child so an external watchdog can revalidate the process identity;
//! 3. a closed handle cannot reach the backend again;
//! 4. every successful run ends with the owned process and all `EXCEL.EXE`
//!    processes gone.
//!
//! Related documents: ADR-0063, ADR-0110, ADR-0111, TASK-407, TASK-409.

#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::print_stderr
)]

use std::fmt::Write as FmtWrite;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(windows)]
use super::write_acceptance::{FailureStage, failure_runner, terminate_verified_process};
use super::*;
use crate::excel_process::{
    ExcelProcessIdentity, excel_process_ids, process_identity, validate_local_workbook_directory,
    validate_workbook_path,
};

const DEFAULT_REPEAT_RUNS: usize = 10;
const MAX_REPEAT_RUNS: usize = 32;
const PROCESS_CONVERGENCE_ATTEMPTS: usize = 600;
const PROCESS_CONVERGENCE_INTERVAL: Duration = Duration::from_millis(100);
const INTENTIONAL_HANG: Duration = Duration::from_secs(600);
const RUN_TOKEN_MAX_BYTES: usize = 128;

static NEXT_TEMPORARY_FILE_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FailureCheckpoint {
    None,
    AfterWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HangCheckpoint {
    None,
    AfterOpen,
}

struct RepeatAcceptanceOptions {
    source_workbook: PathBuf,
    evidence_root: PathBuf,
    run_token: String,
    repeat_runs: usize,
    failure_checkpoint: FailureCheckpoint,
    hang_checkpoint: HangCheckpoint,
}

struct TemporaryRunFiles {
    workbook: PathBuf,
    shadow: PathBuf,
}

struct OwnershipJournal {
    path: PathBuf,
    is_active: bool,
}

struct RepeatAcceptanceGuard {
    runner: Option<ExcelComRunner>,
    owned_process: Option<ExcelProcessIdentity>,
    is_journal_ready: bool,
}

struct RunRecord {
    run: usize,
    process_id: u32,
    start_time_100ns: u64,
    original_digest: String,
    written_digest: String,
    restored_digest: String,
}

impl RepeatAcceptanceOptions {
    fn from_environment() -> PlatformResult<Self> {
        let source_workbook = required_environment_path("EXCEL_T22_ACCEPTANCE_WORKBOOK")?;
        let evidence_root = required_environment_path("EXCEL_T22_REPEAT_EVIDENCE_DIR")?;
        create_local_directory(&evidence_root)?;
        let evidence_root = validate_local_workbook_directory(&evidence_root)?;
        let source_workbook = validate_workbook_path(&source_workbook)?;
        let run_token = environment_string("EXCEL_T22_REPEAT_RUN_TOKEN", "direct")?;
        validate_run_token(&run_token)?;
        let repeat_runs = environment_usize(
            "EXCEL_T22_REPEAT_RUNS",
            DEFAULT_REPEAT_RUNS,
            MAX_REPEAT_RUNS,
        )?;
        let failure_checkpoint =
            parse_failure_checkpoint(&environment_string("EXCEL_T22_REPEAT_FAIL_AFTER", "none")?)?;
        let hang_checkpoint =
            parse_hang_checkpoint(&environment_string("EXCEL_T22_REPEAT_HANG_AFTER", "none")?)?;
        if failure_checkpoint != FailureCheckpoint::None && hang_checkpoint != HangCheckpoint::None
        {
            return Err(PlatformError::new(
                ErrorCode::ToolInvalidArgs,
                "TASK-409 cannot inject failure and hang at the same time",
            ));
        }
        let should_run_formal_repetition = failure_checkpoint == FailureCheckpoint::None
            && hang_checkpoint == HangCheckpoint::None;
        if should_run_formal_repetition && repeat_runs != DEFAULT_REPEAT_RUNS {
            return Err(PlatformError::new(
                ErrorCode::ToolInvalidArgs,
                "formal TASK-409 acceptance requires exactly 10 normal runs",
            ));
        }
        if !should_run_formal_repetition && repeat_runs != 1 {
            return Err(PlatformError::new(
                ErrorCode::ToolInvalidArgs,
                "TASK-409 injection scenarios require exactly one run",
            ));
        }
        Ok(Self {
            source_workbook,
            evidence_root,
            run_token,
            repeat_runs,
            failure_checkpoint,
            hang_checkpoint,
        })
    }
}

impl TemporaryRunFiles {
    fn from_source(source: &Path, run_token: &str, run: usize) -> PlatformResult<Self> {
        let directory = validate_local_workbook_directory(&std::env::temp_dir())?;
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| repeat_error(format!("system clock failed: {error}")))?
            .as_nanos();
        let file_id = NEXT_TEMPORARY_FILE_ID.fetch_add(1, Ordering::Relaxed);
        let prefix = format!(
            "assistant-task409-{run_token}-{}-{run:02}-{file_id}",
            std::process::id()
        );
        let workbook = directory.join(format!("{prefix}-{nonce}.xlsx"));
        let shadow = directory.join(format!("{prefix}-{nonce}.shadow.xlsx"));
        fs::copy(source, &workbook)
            .map_err(|error| repeat_io_error("create working copy", &workbook, &error))?;
        if let Err(error) = fs::copy(source, &shadow) {
            remove_if_exists(&workbook)?;
            return Err(repeat_io_error("create shadow copy", &shadow, &error));
        }
        let workbook = validate_workbook_path(&workbook)?;
        let shadow = validate_workbook_path(&shadow)?;
        Ok(Self { workbook, shadow })
    }

    fn cleanup(&self) -> PlatformResult<()> {
        remove_if_exists(&self.workbook)?;
        remove_if_exists(&self.shadow)
    }
}

impl Drop for TemporaryRunFiles {
    fn drop(&mut self) {
        if let Err(error) = self.cleanup() {
            eprintln!("TASK-409 temporary workbook cleanup failed during Drop: {error}");
        }
    }
}

impl OwnershipJournal {
    fn create(
        evidence_root: &Path,
        run: usize,
        phase: &str,
        workbook: &Path,
        identity: &ExcelProcessIdentity,
    ) -> PlatformResult<Self> {
        let path = evidence_root.join(format!("ownership-run-{run:02}-{phase}.json"));
        let temporary_path = evidence_root.join(format!("ownership-run-{run:02}-{phase}.json.tmp"));
        let content = format!(
            "{{\n  \"schema_version\": 1,\n  \"run\": {run},\n  \"process_name\": \"EXCEL\",\n  \"process_id\": {},\n  \"start_time_100ns\": {},\n  \"executable_path\": {},\n  \"workbook_path\": {}\n}}\n",
            identity.process_id,
            identity.start_time_100ns,
            json_string(&identity.executable_path),
            json_string(&workbook.to_string_lossy())
        );
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
            .map_err(|error| {
                repeat_io_error(
                    "create temporary ownership journal",
                    &temporary_path,
                    &error,
                )
            })?;
        file.write_all(content.as_bytes()).map_err(|error| {
            repeat_io_error("write temporary ownership journal", &temporary_path, &error)
        })?;
        file.sync_all().map_err(|error| {
            repeat_io_error("sync temporary ownership journal", &temporary_path, &error)
        })?;
        drop(file);
        fs::rename(&temporary_path, &path)
            .map_err(|error| repeat_io_error("publish ownership journal", &path, &error))?;
        Ok(Self {
            path,
            is_active: true,
        })
    }

    fn complete(mut self) -> PlatformResult<()> {
        remove_if_exists(&self.path)?;
        self.is_active = false;
        Ok(())
    }
}

impl RepeatAcceptanceGuard {
    fn new(runner: ExcelComRunner) -> Self {
        Self {
            runner: Some(runner),
            owned_process: None,
            is_journal_ready: false,
        }
    }

    fn runner(&self) -> &ExcelComRunner {
        self.runner
            .as_ref()
            .expect("repeat acceptance runner is available")
    }

    fn set_owned_process(&mut self, identity: ExcelProcessIdentity) {
        self.owned_process = Some(identity);
    }

    fn mark_journal_ready(&mut self) {
        self.is_journal_ready = true;
    }

    fn shutdown(&mut self) -> PlatformResult<()> {
        if !self.is_journal_ready {
            return Err(repeat_error(
                "refusing graceful Excel shutdown before ownership journal is durable",
            ));
        }
        self.runner
            .take()
            .ok_or_else(|| repeat_error("repeat acceptance runner is already shut down"))?
            .shutdown()
    }
}

impl Drop for RepeatAcceptanceGuard {
    fn drop(&mut self) {
        let Some(mut runner) = self.runner.take() else {
            return;
        };
        if self.is_journal_ready {
            if let Err(error) = runner.shutdown() {
                eprintln!("TASK-409 graceful runner shutdown failed during Drop: {error}");
            }
            return;
        }
        let Some(identity) = self.owned_process.as_ref() else {
            // Without a journal or a verified identity, a graceful Quit is not
            // safe to attempt. The external watchdog observes Excel creation
            // independently and owns the last-resort cleanup path.
            std::mem::forget(runner);
            eprintln!(
                "TASK-409 deferred cleanup to external watchdog because no verified identity was available"
            );
            return;
        };
        match terminate_verified_process(identity) {
            Ok(()) => {
                eprintln!(
                    "TASK-409 force-terminated owned Excel {} before journal publication",
                    identity.process_id
                );
            }
            Err(error) => {
                eprintln!(
                    "TASK-409 pre-journal verified termination failed for Excel {}: {error}",
                    identity.process_id
                );
            }
        }
        // Never issue a graceful Quit without a durable journal. Let the
        // external harness reconcile this error path after the test exits.
        std::mem::forget(runner);
    }
}

impl Drop for OwnershipJournal {
    fn drop(&mut self) {
        if self.is_active {
            eprintln!(
                "TASK-409 ownership journal retained for external watchdog evidence: {}",
                self.path.display()
            );
        }
    }
}

impl RunRecord {
    fn render_evidence_record(&self) -> String {
        format!(
            "{{\"run\":{},\"process_id\":{},\"start_time_100ns\":{},\"original_digest\":{},\"written_digest\":{},\"restored_digest\":{}}}",
            self.run,
            self.process_id,
            self.start_time_100ns,
            json_string(&self.original_digest),
            json_string(&self.written_digest),
            json_string(&self.restored_digest)
        )
    }
}

#[cfg(windows)]
#[test]
#[ignore = "TASK-409 human-present real Excel COM repeated write acceptance"]
fn test_windows_excel_real_com_repeated_write_acceptance() {
    run_real_com_repeated_acceptance().expect("TASK-409 real COM repeated acceptance");
}

fn run_real_com_repeated_acceptance() -> PlatformResult<()> {
    let options = RepeatAcceptanceOptions::from_environment()?;
    assert_no_excel_processes("before TASK-409 acceptance")?;
    let source_digest = sha256_file(&options.source_workbook)?;
    let mut records = Vec::with_capacity(options.repeat_runs);
    for run in 1..=options.repeat_runs {
        let record = execute_real_com_run(&options, run, &source_digest)?;
        append_run_record(&options.evidence_root, &record)?;
        records.push(record);
    }
    let source_digest_after = sha256_file(&options.source_workbook)?;
    if source_digest != source_digest_after {
        return Err(repeat_error(
            "source workbook digest changed during repeated acceptance",
        ));
    }
    assert_no_excel_processes("after TASK-409 acceptance")?;
    write_normal_summary(
        &options.evidence_root,
        options.repeat_runs,
        &source_digest,
        &records,
    )
}

fn execute_real_com_run(
    options: &RepeatAcceptanceOptions,
    run: usize,
    source_digest: &str,
) -> PlatformResult<RunRecord> {
    let files = TemporaryRunFiles::from_source(&options.source_workbook, &options.run_token, run)?;
    let (mut guard, identity, journal) =
        start_owned_runner(options, run, "write", &files.workbook)?;
    let workbook = guard
        .runner()
        .open_writable_for_acceptance(&files.workbook)?;
    if options.hang_checkpoint == HangCheckpoint::AfterOpen {
        write_checkpoint_marker(&options.evidence_root, run, "after-open")?;
        thread::sleep(INTENTIONAL_HANG);
    }
    let scalar_value = f64::from(
        u32::try_from(100_usize.saturating_add(run))
            .map_err(|_| repeat_error("TASK-409 scalar value overflowed u32"))?,
    );
    guard.runner().write_scalar_for_acceptance(
        &workbook,
        a1_cell(1, 1),
        ExcelScalarValue::Number(scalar_value),
    )?;
    if options.failure_checkpoint == FailureCheckpoint::AfterWrite {
        write_checkpoint_marker(&options.evidence_root, run, "after-write")?;
        panic!("TASK-409 injected failure after-write");
    }
    guard.runner().recalculate_for_acceptance(&workbook)?;
    assert_written_cells(guard.runner(), &workbook, scalar_value, run)?;
    guard.runner().save_workbook_for_acceptance(&workbook)?;
    guard.runner().close_workbook(&workbook)?;
    assert_stale_handle_rejected(guard.runner(), &workbook)?;
    guard.shutdown()?;
    wait_for_process_convergence(identity.process_id)?;
    let written_digest = sha256_file(&files.workbook)?;
    if written_digest == source_digest {
        return Err(repeat_error(
            "saved working copy digest did not change after scalar write",
        ));
    }
    verify_saved_working_copy(options, run, &files.workbook, scalar_value)?;
    let readback_digest = sha256_file(&files.workbook)?;
    if readback_digest != written_digest {
        return Err(repeat_error(
            "read-only persistence verification changed the saved working copy",
        ));
    }
    fs::copy(&files.shadow, &files.workbook)
        .map_err(|error| repeat_io_error("restore shadow copy", &files.workbook, &error))?;
    let restored_digest = sha256_file(&files.workbook)?;
    if restored_digest != source_digest {
        return Err(repeat_error(
            "shadow restore did not reproduce the source digest",
        ));
    }
    files.cleanup()?;
    journal.complete()?;
    eprintln!(
        "TASK-409 run {run}/{} passed pid={} start={} written_digest={written_digest}",
        options.repeat_runs, identity.process_id, identity.start_time_100ns
    );
    Ok(RunRecord {
        run,
        process_id: identity.process_id,
        start_time_100ns: identity.start_time_100ns,
        original_digest: source_digest.to_owned(),
        written_digest,
        restored_digest,
    })
}

fn start_owned_runner(
    options: &RepeatAcceptanceOptions,
    run: usize,
    phase: &str,
    workbook: &Path,
) -> PlatformResult<(
    RepeatAcceptanceGuard,
    ExcelProcessIdentity,
    OwnershipJournal,
)> {
    let mut guard = RepeatAcceptanceGuard::new(ExcelComRunner::start()?);
    guard.runner().create_application()?;
    let process_id = guard.runner().application_process_id()?;
    let identity = process_identity(process_id)?.ok_or_else(|| {
        repeat_error(format!(
            "owned Excel process {process_id} disappeared before identity capture"
        ))
    })?;
    guard.set_owned_process(identity.clone());
    let journal =
        OwnershipJournal::create(&options.evidence_root, run, phase, workbook, &identity)?;
    guard.mark_journal_ready();
    Ok((guard, identity, journal))
}

fn verify_saved_working_copy(
    options: &RepeatAcceptanceOptions,
    run: usize,
    workbook_path: &Path,
    scalar_value: f64,
) -> PlatformResult<()> {
    let (mut guard, identity, journal) =
        start_owned_runner(options, run, "readback", workbook_path)?;
    let workbook = guard.runner().open_read_only(workbook_path)?;
    assert_written_cells(guard.runner(), &workbook, scalar_value, run)?;
    guard.runner().close_workbook(&workbook)?;
    assert_stale_handle_rejected(guard.runner(), &workbook)?;
    guard.shutdown()?;
    wait_for_process_convergence(identity.process_id)?;
    journal.complete()
}

fn assert_written_cells(
    runner: &ExcelComRunner,
    workbook: &ExcelWorkbookHandle,
    scalar_value: f64,
    run: usize,
) -> PlatformResult<()> {
    let expected_value = format!("{scalar_value:.0}");
    let expected_sum = scalar_value + 3.0;
    let expected_sum_text = format!("{expected_sum:.0}");
    let a1 = read_cell(runner, workbook, 1, 1, ExcelReadMode::Value2)?;
    require_cell(
        &a1,
        ExcelCellValueType::Number,
        &expected_value,
        "A1 Value2",
    )?;
    let a1_formula = read_cell(runner, workbook, 1, 1, ExcelReadMode::Formula)?;
    require_cell(
        &a1_formula,
        ExcelCellValueType::Text,
        &expected_value,
        "A1 Formula",
    )?;
    let f1 = read_cell(runner, workbook, 1, 6, ExcelReadMode::Value2)?;
    require_cell(
        &f1,
        ExcelCellValueType::Number,
        &expected_sum_text,
        "F1 Value2",
    )?;
    let f1_formula = read_cell(runner, workbook, 1, 6, ExcelReadMode::Formula)?;
    require_cell(
        &f1_formula,
        ExcelCellValueType::Text,
        "=A1+B1",
        "F1 Formula",
    )?;
    eprintln!("TASK-409 run {run} raw A1={a1:?}/{a1_formula:?} F1={f1:?}/{f1_formula:?}");
    Ok(())
}

fn read_cell(
    runner: &ExcelComRunner,
    workbook: &ExcelWorkbookHandle,
    row: u32,
    column: u32,
    mode: ExcelReadMode,
) -> PlatformResult<(ExcelCellValueType, String)> {
    let snapshot = runner.read_range(workbook, a1_cell(row, column), mode)?;
    let cell = snapshot.cells().first().ok_or_else(|| {
        repeat_error(format!(
            "Excel {mode:?} read returned no cell for row {row}, column {column}"
        ))
    })?;
    if snapshot.cells().len() != 1 {
        return Err(repeat_error(format!(
            "Excel {mode:?} single-cell read returned {} cells",
            snapshot.cells().len()
        )));
    }
    Ok((cell.value_type(), cell.value_text().to_owned()))
}

fn require_cell(
    actual: &(ExcelCellValueType, String),
    expected_type: ExcelCellValueType,
    expected_text: &str,
    context: &str,
) -> PlatformResult<()> {
    if actual.0 != expected_type || actual.1 != expected_text {
        return Err(repeat_error(format!(
            "{context} mismatch: expected {expected_type:?}/{expected_text}, got {:?}/{}",
            actual.0, actual.1
        )));
    }
    Ok(())
}

fn assert_stale_handle_rejected(
    runner: &ExcelComRunner,
    workbook: &ExcelWorkbookHandle,
) -> PlatformResult<()> {
    for error in [
        runner
            .write_scalar_for_acceptance(workbook, a1_cell(1, 1), ExcelScalarValue::Number(1.0))
            .expect_err("closed handle write"),
        runner
            .recalculate_for_acceptance(workbook)
            .expect_err("closed handle recalculate"),
        runner
            .save_workbook_for_acceptance(workbook)
            .expect_err("closed handle save"),
        runner
            .read_range(workbook, a1_cell(1, 1), ExcelReadMode::Value2)
            .expect_err("closed handle read"),
    ] {
        if error.code() != ErrorCode::ToolInvalidArgs {
            return Err(repeat_error(format!(
                "closed Excel handle returned {:?}, expected ToolInvalidArgs",
                error.code()
            )));
        }
    }
    Ok(())
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

fn wait_for_process_convergence(process_id: u32) -> PlatformResult<()> {
    for attempt in 0..PROCESS_CONVERGENCE_ATTEMPTS {
        let process_ids = excel_process_ids()?;
        if !process_ids.contains(&process_id) {
            if process_ids.is_empty() {
                return Ok(());
            }
            return Err(repeat_error(format!(
                "unexpected EXCEL.EXE processes remained after owned PID {process_id}: {process_ids:?}"
            )));
        }
        if attempt > 0 && attempt % 50 == 0 {
            eprintln!(
                "TASK-409 waiting for owned Excel {process_id} teardown: {}s",
                attempt / 10
            );
        }
        thread::sleep(PROCESS_CONVERGENCE_INTERVAL);
    }
    Err(PlatformError::new(
        ErrorCode::TargetUnresponsive,
        format!("owned Excel process {process_id} did not exit within 60 seconds"),
    ))
}

fn assert_no_excel_processes(context: &str) -> PlatformResult<()> {
    let process_ids = excel_process_ids()?;
    if process_ids.is_empty() {
        Ok(())
    } else {
        Err(PlatformError::new(
            ErrorCode::UserInteraction,
            format!("{context}: existing EXCEL.EXE processes detected: {process_ids:?}"),
        ))
    }
}

fn append_run_record(evidence_root: &Path, record: &RunRecord) -> PlatformResult<()> {
    let path = evidence_root.join("runs.jsonl");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| repeat_io_error("open run evidence", &path, &error))?;
    file.write_all(record.render_evidence_record().as_bytes())
        .and_then(|()| file.write_all(b"\n"))
        .map_err(|error| repeat_io_error("append run evidence", &path, &error))
}

fn write_normal_summary(
    evidence_root: &Path,
    requested_runs: usize,
    source_digest: &str,
    records: &[RunRecord],
) -> PlatformResult<()> {
    let path = evidence_root.join("normal-summary.json");
    let mut output = format!(
        "{{\n  \"schema_version\": 1,\n  \"mode\": \"excel-t22-repeated-write\",\n  \"result\": \"passed\",\n  \"requested_runs\": {requested_runs},\n  \"passed_runs\": {},\n  \"source_digest\": {},\n  \"final_excel_process_count\": 0,\n  \"runs\": [\n",
        records.len(),
        json_string(source_digest)
    );
    for (index, record) in records.iter().enumerate() {
        output.push_str("    ");
        output.push_str(&record.render_evidence_record());
        if index + 1 != records.len() {
            output.push(',');
        }
        output.push('\n');
    }
    output.push_str("  ]\n}\n");
    fs::write(&path, output).map_err(|error| repeat_io_error("write summary", &path, &error))
}

fn write_checkpoint_marker(
    evidence_root: &Path,
    run: usize,
    checkpoint: &str,
) -> PlatformResult<()> {
    let path = evidence_root.join(format!("checkpoint-run-{run:02}-{checkpoint}.ready"));
    fs::write(&path, b"ready\n")
        .map_err(|error| repeat_io_error("write checkpoint marker", &path, &error))
}

fn sha256_file(path: &Path) -> PlatformResult<String> {
    let bytes =
        fs::read(path).map_err(|error| repeat_io_error("read digest input", path, &error))?;
    Ok(crate::digest::sha256_hex_bytes(&bytes))
}

fn create_local_directory(path: &Path) -> PlatformResult<()> {
    if !path.is_absolute() {
        return Err(PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            "TASK-409 evidence directory must be absolute",
        ));
    }
    fs::create_dir_all(path)
        .map_err(|error| repeat_io_error("create evidence directory", path, &error))
}

fn required_environment_path(name: &str) -> PlatformResult<PathBuf> {
    std::env::var_os(name).map(PathBuf::from).ok_or_else(|| {
        PlatformError::new(ErrorCode::ToolInvalidArgs, format!("{name} is required"))
    })
}

fn environment_string(name: &str, default: &str) -> PlatformResult<String> {
    std::env::var(name).map_or_else(
        |error| match error {
            std::env::VarError::NotPresent => Ok(default.to_owned()),
            std::env::VarError::NotUnicode(_) => Err(PlatformError::new(
                ErrorCode::ToolInvalidArgs,
                format!("{name} is not valid Unicode"),
            )),
        },
        Ok,
    )
}

fn environment_usize(name: &str, default: usize, maximum: usize) -> PlatformResult<usize> {
    let parsed = match std::env::var(name) {
        Ok(value) => value.parse::<usize>().map_err(|error| {
            PlatformError::new(
                ErrorCode::ToolInvalidArgs,
                format!("{name} is not an integer: {error}"),
            )
        })?,
        Err(std::env::VarError::NotPresent) => default,
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(PlatformError::new(
                ErrorCode::ToolInvalidArgs,
                format!("{name} is not valid Unicode"),
            ));
        }
    };
    if !(1..=maximum).contains(&parsed) {
        return Err(PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            format!("{name} must be in 1..={maximum}"),
        ));
    }
    Ok(parsed)
}

fn parse_failure_checkpoint(value: &str) -> PlatformResult<FailureCheckpoint> {
    match value {
        "none" => Ok(FailureCheckpoint::None),
        "after-write" => Ok(FailureCheckpoint::AfterWrite),
        _ => Err(PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            "EXCEL_T22_REPEAT_FAIL_AFTER must be none or after-write",
        )),
    }
}

fn parse_hang_checkpoint(value: &str) -> PlatformResult<HangCheckpoint> {
    match value {
        "none" => Ok(HangCheckpoint::None),
        "after-open" => Ok(HangCheckpoint::AfterOpen),
        _ => Err(PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            "EXCEL_T22_REPEAT_HANG_AFTER must be none or after-open",
        )),
    }
}

fn validate_run_token(value: &str) -> PlatformResult<()> {
    if value.is_empty()
        || value.len() > RUN_TOKEN_MAX_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            "EXCEL_T22_REPEAT_RUN_TOKEN must be bounded ASCII alphanumeric/hyphen",
        ));
    }
    Ok(())
}

fn json_string(value: &str) -> String {
    let mut output = String::with_capacity(value.len().saturating_add(2));
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character <= '\u{1F}' => {
                write!(output, "\\u{:04x}", u32::from(character))
                    .expect("writing an escape sequence to String cannot fail");
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

fn remove_if_exists(path: &Path) -> PlatformResult<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(repeat_io_error("remove TASK-409 file", path, &error)),
    }
}

fn repeat_io_error(context: &str, path: &Path, error: &std::io::Error) -> PlatformError {
    PlatformError::new(
        ErrorCode::Fatal,
        format!("{context} failed for {}: {error}", path.display()),
    )
}

fn repeat_error(message: impl Into<String>) -> PlatformError {
    PlatformError::new(ErrorCode::Fatal, message)
}

#[test]
fn test_write_acceptance_failure_injection_closes_session_and_rejects_stale_handle() {
    for fail_at in [
        FailureStage::Open,
        FailureStage::Write,
        FailureStage::Recalculate,
        FailureStage::Save,
    ] {
        let path = temporary_workbook();
        let mut runner = failure_runner(fail_at).expect("failure runner");
        runner
            .create_application()
            .expect("create fake application");
        let open = runner.open_writable_for_acceptance(&path);
        let workbook = match open {
            Ok(workbook) => workbook,
            Err(error) => {
                assert_eq!(fail_at, FailureStage::Open);
                assert_eq!(error.code(), ErrorCode::Fatal);
                runner.shutdown().expect("shutdown after open failure");
                std::fs::remove_file(path).expect("remove fake workbook");
                continue;
            }
        };
        let write = runner.write_scalar_for_acceptance(
            &workbook,
            a1_cell(1, 1),
            ExcelScalarValue::Number(1.0),
        );
        if fail_at == FailureStage::Write {
            assert_eq!(write.expect_err("write failure").code(), ErrorCode::Fatal);
        } else {
            write.expect("write success");
        }
        if fail_at != FailureStage::Recalculate {
            runner
                .recalculate_for_acceptance(&workbook)
                .expect("recalculate success");
        }
        if fail_at != FailureStage::Save {
            runner
                .save_workbook_for_acceptance(&workbook)
                .expect("save success");
        }
        runner
            .close_workbook(&workbook)
            .expect("close after failure");
        assert_stale_handle_rejected(&runner, &workbook).expect("stale handle rejection");
        runner.shutdown().expect("shutdown after failure");
        std::fs::remove_file(path).expect("remove fake workbook");
    }
}
