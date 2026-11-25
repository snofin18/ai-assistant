//! Private adapter from the Excel T2.1 domain runtime to the Windows COM runner.
//!
//! Responsibilities:
//! - map self-contained `assistant-platform-windows` values into the existing
//!   Excel T2.1 domain model;
//! - keep one bounded domain-handle table for the runner's active workbook;
//! - preserve typed platform failures and explicitly close incomplete opens.
//!
//! Boundaries:
//! - this module is compiled only in tests until repeated real-COM acceptance and
//!   production assembly are separately approved;
//! - it never carries a COM pointer and never registers a model-visible Tool;
//! - it does not implement ownership journals, a watchdog, or process-tree kill.
//!
//! Invariants:
//! 1. one runtime owns one runner with at most one open workbook;
//! 2. a domain handle resolves only to the platform handle stored for that id;
//! 3. close removes a mapping only after the platform close succeeds;
//! 4. all platform failures retain their existing `ErrorCode`.
//!
//! Related documents: ADR-0108, ADR-0111, ADR-0112, TASK-403.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use assistant_platform_api::{PlatformError, PlatformResult};
use assistant_platform_windows::excel_com_runner::{
    ExcelCellValueType, ExcelComRunner, ExcelNamedRangeScope, ExcelRangeSelector,
    ExcelRangeSnapshot, ExcelReadMode, ExcelWorkbookHandle, ExcelWorkbookSnapshot,
    ExcelWorksheetSelector,
};
use assistant_protocol::ErrorCode;

use super::{
    CellValueType, ExcelT21Error, ExcelT21Runtime, MAX_RANGE_CELLS, NamedRangeInfo,
    NamedRangeScope, OpenedWorkbook, OwnedProcessIdentity, RangeSelector, RangeSnapshot, ReadMode,
    RuntimeCell, WorkbookHandle, WorkbookIdentity, WorkbookSnapshot, WorksheetInfo,
    WorksheetSelector, invalid_arguments,
};

const MAX_OPEN_WORKBOOKS: usize = 1;

/// Minimal private port used by the adapter and by lifecycle tests.
trait ExcelPlatformRunner: Send + Sync + 'static {
    type Workbook: Clone + Eq + Send + 'static;

    fn start() -> PlatformResult<Self>
    where
        Self: Sized;

    fn create_application(&self) -> PlatformResult<()>;

    fn open_read_only(&self, path: &Path) -> PlatformResult<Self::Workbook>;

    fn workbook_path<'a>(&self, workbook: &'a Self::Workbook) -> &'a Path;

    fn owned_process(&self, workbook: &Self::Workbook) -> PlatformResult<OwnedProcessIdentity>;

    fn read_workbook(&self, workbook: &Self::Workbook) -> PlatformResult<WorkbookSnapshot>;

    fn read_range(
        &self,
        workbook: &Self::Workbook,
        selector: &RangeSelector,
        mode: ReadMode,
    ) -> PlatformResult<RangeSnapshot>;

    fn fingerprint(&self, workbook: &Self::Workbook) -> PlatformResult<String>;

    fn verify_ownership(&self, workbook: &Self::Workbook) -> PlatformResult<()>;

    fn close_workbook(&self, workbook: &Self::Workbook) -> PlatformResult<()>;

    fn shutdown(&mut self) -> PlatformResult<()>;
}

/// Private Excel T2.1 runtime over a platform runner.
struct PlatformExcelT21Runtime<R>
where
    R: ExcelPlatformRunner,
{
    state: Mutex<RuntimeState<R>>,
}

struct RuntimeState<R>
where
    R: ExcelPlatformRunner,
{
    runner: Option<R>,
    is_application_ready: bool,
    is_terminal: bool,
    next_handle: u64,
    workbooks: BTreeMap<u64, WorkbookRecord<R::Workbook>>,
}

#[derive(Clone)]
struct WorkbookRecord<W> {
    platform_workbook: W,
    workbook_identity: WorkbookIdentity,
}

impl<R> PlatformExcelT21Runtime<R>
where
    R: ExcelPlatformRunner,
{
    /// Starts one runner without creating an Excel application.
    fn start() -> Result<Self, ExcelT21Error> {
        let runner = R::start().map_err(|error| platform_error("start Excel runner", &error))?;
        Ok(Self::with_runner(runner))
    }

    fn with_runner(runner: R) -> Self {
        Self {
            state: Mutex::new(RuntimeState {
                runner: Some(runner),
                is_application_ready: false,
                is_terminal: false,
                next_handle: 0,
                workbooks: BTreeMap::new(),
            }),
        }
    }

    /// Explicitly shuts down the runner and makes this runtime terminal.
    fn shutdown(&self) -> Result<(), ExcelT21Error> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ExcelT21Error::new(ErrorCode::Fatal, "Excel runtime state is poisoned"))?;
        let Some(mut runner) = state.runner.take() else {
            return Err(ExcelT21Error::new(
                ErrorCode::Fatal,
                "Excel runtime was already shut down",
            ));
        };
        let result = runner
            .shutdown()
            .map_err(|error| platform_error("shutdown Excel runner", &error));
        state.workbooks.clear();
        state.is_application_ready = false;
        state.is_terminal = true;
        drop(state);
        result
    }

    #[cfg(test)]
    fn active_workbook_count(&self) -> Result<usize, ExcelT21Error> {
        self.state
            .lock()
            .map(|state| state.workbooks.len())
            .map_err(|_| ExcelT21Error::new(ErrorCode::Fatal, "Excel runtime state is poisoned"))
    }

    fn lock_state(&self) -> Result<std::sync::MutexGuard<'_, RuntimeState<R>>, ExcelT21Error> {
        self.state
            .lock()
            .map_err(|_| ExcelT21Error::new(ErrorCode::Fatal, "Excel runtime state is poisoned"))
    }
}

impl<R> ExcelT21Runtime for PlatformExcelT21Runtime<R>
where
    R: ExcelPlatformRunner,
{
    fn open_read_only(&self, path: &Path) -> Result<OpenedWorkbook, ExcelT21Error> {
        let mut state = self.lock_state()?;
        if state.is_terminal {
            return Err(ExcelT21Error::new(
                ErrorCode::Fatal,
                "Excel runtime is terminal after shutdown",
            ));
        }
        if !state.workbooks.is_empty() {
            return Err(invalid_arguments(
                "Excel runtime already owns an open workbook",
            ));
        }
        let (platform_workbook, snapshot, owned_process) = prepare_open_workbook(&mut state, path)?;

        let handle_value = state.next_handle.checked_add(1).ok_or_else(|| {
            ExcelT21Error::new(ErrorCode::Fatal, "Excel domain workbook handle overflowed")
        })?;
        state.next_handle = handle_value;
        let handle = WorkbookHandle::new(handle_value);
        state.workbooks.insert(
            handle_value,
            WorkbookRecord {
                platform_workbook,
                workbook_identity: snapshot.workbook.clone(),
            },
        );
        debug_assert!(state.workbooks.len() <= MAX_OPEN_WORKBOOKS);
        drop(state);
        Ok(OpenedWorkbook {
            handle,
            owned_process,
            workbook: snapshot.workbook,
            worksheets: snapshot.worksheets,
        })
    }

    fn close(&self, handle: WorkbookHandle) -> Result<(), ExcelT21Error> {
        let mut state = self.lock_state()?;
        let record = state.workbooks.get(&handle.get()).cloned().ok_or_else(|| {
            ExcelT21Error::new(
                ErrorCode::TargetNotFound,
                "Excel domain workbook handle is unknown or already closed",
            )
        })?;
        let runner = state
            .runner
            .as_ref()
            .ok_or_else(|| ExcelT21Error::new(ErrorCode::Fatal, "Excel runner is unavailable"))?;
        runner
            .close_workbook(&record.platform_workbook)
            .map_err(|error| platform_error("close Excel workbook", &error))?;
        if state.workbooks.remove(&handle.get()).is_none() {
            return Err(ExcelT21Error::new(
                ErrorCode::Fatal,
                "Excel workbook mapping disappeared during close",
            ));
        }
        drop(state);
        Ok(())
    }

    fn verify_ownership(
        &self,
        handle: WorkbookHandle,
        workbook: &WorkbookIdentity,
    ) -> Result<(), ExcelT21Error> {
        let state = self.lock_state()?;
        let record = workbook_record(&state, handle)?;
        if *record.workbook_identity != *workbook
            || !workbook_paths_match(
                Path::new(&record.workbook_identity.full_path),
                &record.platform_workbook_path.to_string_lossy(),
            )
        {
            return Err(ExcelT21Error::new(
                ErrorCode::TargetNotFound,
                "Excel workbook identity no longer matches the platform handle",
            ));
        }
        let result = runner(&state)?
            .verify_ownership(record.platform_workbook)
            .map_err(|error| platform_error("verify Excel process ownership", &error));
        drop(state);
        result
    }

    fn fingerprint(&self, handle: WorkbookHandle) -> Result<String, ExcelT21Error> {
        let state = self.lock_state()?;
        let record = workbook_record(&state, handle)?;
        let fingerprint = runner(&state)?
            .fingerprint(record.platform_workbook)
            .map_err(|error| platform_error("fingerprint Excel workbook", &error))?;
        drop(state);
        if fingerprint.trim().is_empty() {
            return Err(ExcelT21Error::new(
                ErrorCode::Fatal,
                "Excel runner returned an empty workbook fingerprint",
            ));
        }
        Ok(fingerprint)
    }

    fn read_workbook(&self, handle: WorkbookHandle) -> Result<WorkbookSnapshot, ExcelT21Error> {
        let state = self.lock_state()?;
        let record = workbook_record(&state, handle)?;
        let snapshot = runner(&state)?
            .read_workbook(record.platform_workbook)
            .map_err(|error| platform_error("read Excel workbook metadata", &error))?;
        if snapshot.workbook != *record.workbook_identity {
            return Err(ExcelT21Error::new(
                ErrorCode::TargetNotFound,
                "Excel workbook identity drifted during metadata read",
            ));
        }
        drop(state);
        Ok(snapshot)
    }

    fn read_range(
        &self,
        handle: WorkbookHandle,
        selector: &RangeSelector,
        mode: ReadMode,
    ) -> Result<RangeSnapshot, ExcelT21Error> {
        let state = self.lock_state()?;
        let record = workbook_record(&state, handle)?;
        let snapshot = runner(&state)?
            .read_range(record.platform_workbook, selector, mode)
            .map_err(|error| platform_error("read Excel range", &error))?;
        drop(state);
        validate_range_shape(&snapshot, selector)?;
        Ok(snapshot)
    }
}

fn prepare_open_workbook<R>(
    state: &mut RuntimeState<R>,
    path: &Path,
) -> Result<(R::Workbook, WorkbookSnapshot, OwnedProcessIdentity), ExcelT21Error>
where
    R: ExcelPlatformRunner,
{
    ensure_application(state)?;
    let platform_workbook = runner(state)?
        .open_read_only(path)
        .map_err(|error| platform_error("open workbook read-only", &error))?;
    let snapshot = match runner(state)?.read_workbook(&platform_workbook) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            return Err(cleanup_failed_open(
                state,
                &platform_workbook,
                platform_error("read workbook metadata during open", &error),
            ));
        }
    };
    if !snapshot.workbook.read_only {
        return Err(cleanup_failed_open(
            state,
            &platform_workbook,
            invalid_arguments("platform opened the workbook without read-only mode"),
        ));
    }
    if !workbook_paths_match(path, &snapshot.workbook.full_path)
        || !workbook_paths_match(
            path,
            &runner(state)?
                .workbook_path(&platform_workbook)
                .to_string_lossy(),
        )
    {
        return Err(cleanup_failed_open(
            state,
            &platform_workbook,
            ExcelT21Error::new(
                ErrorCode::Fatal,
                "platform opened a workbook different from the requested path",
            ),
        ));
    }
    let owned_process = match runner(state)?.owned_process(&platform_workbook) {
        Ok(identity) => identity,
        Err(error) => {
            return Err(cleanup_failed_open(
                state,
                &platform_workbook,
                platform_error("read owned Excel process identity", &error),
            ));
        }
    };
    Ok((platform_workbook, snapshot, owned_process))
}

fn ensure_application<R>(state: &mut RuntimeState<R>) -> Result<(), ExcelT21Error>
where
    R: ExcelPlatformRunner,
{
    if state.is_application_ready {
        return Ok(());
    }
    let result = runner(state)?.create_application();
    match result {
        Ok(()) => {
            state.is_application_ready = true;
            Ok(())
        }
        Err(error) => {
            let create_error = platform_error("create Excel application", &error);
            let Some(mut runner) = state.runner.take() else {
                state.is_terminal = true;
                return Err(ExcelT21Error::new(
                    ErrorCode::Fatal,
                    format!("{create_error}; Excel runner disappeared before cleanup"),
                ));
            };
            state.is_terminal = true;
            match runner.shutdown() {
                Ok(()) => Err(create_error),
                Err(cleanup_error) => Err(ExcelT21Error::new(
                    ErrorCode::Fatal,
                    format!(
                        "{create_error}; Excel runner cleanup also failed: {}",
                        platform_error("cleanup failed Excel application", &cleanup_error)
                    ),
                )),
            }
        }
    }
}

fn cleanup_failed_open<R>(
    state: &mut RuntimeState<R>,
    workbook: &R::Workbook,
    original: ExcelT21Error,
) -> ExcelT21Error
where
    R: ExcelPlatformRunner,
{
    let close_result = runner(state).and_then(|runner| {
        runner
            .close_workbook(workbook)
            .map_err(|error| platform_error("cleanup failed Excel open", &error))
    });
    if close_result.is_ok() {
        return original;
    }
    let close_error = close_result.err().unwrap_or_else(|| {
        ExcelT21Error::new(
            ErrorCode::Fatal,
            "Excel open cleanup failed without details",
        )
    });
    let shutdown_result = state.runner.take().map_or_else(
        || {
            Err(ExcelT21Error::new(
                ErrorCode::Fatal,
                "Excel runner disappeared during open cleanup",
            ))
        },
        |mut runner| {
            runner
                .shutdown()
                .map_err(|error| platform_error("shutdown failed Excel runner", &error))
        },
    );
    state.workbooks.clear();
    state.is_application_ready = false;
    state.is_terminal = true;
    match shutdown_result {
        Ok(()) => ExcelT21Error::new(
            ErrorCode::Fatal,
            format!(
                "{original}; workbook cleanup failed: {close_error}; runner shutdown completed"
            ),
        ),
        Err(shutdown_error) => ExcelT21Error::new(
            ErrorCode::Fatal,
            format!(
                "{original}; workbook cleanup failed: {close_error}; runner shutdown also failed: {shutdown_error}"
            ),
        ),
    }
}

struct DomainWorkbookRecord<'a, W> {
    platform_workbook: &'a W,
    platform_workbook_path: PathBuf,
    workbook_identity: &'a WorkbookIdentity,
}

fn workbook_record<R>(
    state: &RuntimeState<R>,
    handle: WorkbookHandle,
) -> Result<DomainWorkbookRecord<'_, R::Workbook>, ExcelT21Error>
where
    R: ExcelPlatformRunner,
{
    let record = state.workbooks.get(&handle.get()).ok_or_else(|| {
        ExcelT21Error::new(
            ErrorCode::TargetNotFound,
            "Excel domain workbook handle is unknown or already closed",
        )
    })?;
    let runner = runner(state)?;
    Ok(DomainWorkbookRecord {
        platform_workbook: &record.platform_workbook,
        platform_workbook_path: runner
            .workbook_path(&record.platform_workbook)
            .to_path_buf(),
        workbook_identity: &record.workbook_identity,
    })
}

fn validate_range_shape(
    snapshot: &RangeSnapshot,
    selector: &RangeSelector,
) -> Result<(), ExcelT21Error> {
    if snapshot.address.trim().is_empty() || snapshot.rows == 0 || snapshot.columns == 0 {
        return Err(invalid_arguments(
            "Excel range snapshot has an empty address or zero dimensions",
        ));
    }
    let expected = u64::from(snapshot.rows)
        .checked_mul(u64::from(snapshot.columns))
        .ok_or_else(|| invalid_arguments("Excel range cell count overflowed"))?;
    if expected > MAX_RANGE_CELLS
        || expected
            != u64::try_from(snapshot.cells.len())
                .map_err(|_| invalid_arguments("Excel range cell count does not fit into u64"))?
    {
        return Err(invalid_arguments(
            "Excel range snapshot is partial or exceeds the cell bound",
        ));
    }
    if snapshot.cells.iter().any(|cell| {
        cell.row == 0
            || cell.column == 0
            || cell.row > snapshot.rows
            || cell.column > snapshot.columns
    }) {
        return Err(invalid_arguments(
            "Excel range cell coordinate is outside its declared dimensions",
        ));
    }
    if let RangeSelector::A1 { start, end, .. } = selector {
        let address = snapshot
            .address
            .rsplit_once('!')
            .map_or(snapshot.address.as_str(), |(_, range)| range);
        let (reported_start, reported_end) = super::parse_a1_range(address)?;
        if reported_start != *start || reported_end != *end {
            return Err(invalid_arguments(
                "Excel range address does not match the requested A1 bounds",
            ));
        }
    }
    Ok(())
}

fn platform_error(context: &str, error: &PlatformError) -> ExcelT21Error {
    ExcelT21Error::new(error.code(), format!("{context}: {}", error.message()))
}

fn runner<R>(state: &RuntimeState<R>) -> Result<&R, ExcelT21Error>
where
    R: ExcelPlatformRunner,
{
    state
        .runner
        .as_ref()
        .ok_or_else(|| ExcelT21Error::new(ErrorCode::Fatal, "Excel runner is unavailable"))
}

fn workbook_paths_match(left: impl AsRef<Path>, right: &str) -> bool {
    normalize_path(left.as_ref()) == normalize_path(Path::new(right))
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .to_lowercase()
        .trim_end_matches('\\')
        .to_owned()
}

impl ExcelPlatformRunner for ExcelComRunner {
    type Workbook = ExcelWorkbookHandle;

    fn start() -> PlatformResult<Self> {
        Self::start()
    }

    fn create_application(&self) -> PlatformResult<()> {
        Self::create_application(self)
    }

    fn open_read_only(&self, path: &Path) -> PlatformResult<Self::Workbook> {
        Self::open_read_only(self, path)
    }

    fn workbook_path<'a>(&self, workbook: &'a Self::Workbook) -> &'a Path {
        workbook.canonical_path()
    }

    fn owned_process(&self, workbook: &Self::Workbook) -> PlatformResult<OwnedProcessIdentity> {
        let identity = workbook.owned_process();
        Ok(OwnedProcessIdentity {
            process_id: identity.process_id(),
            start_ticks: identity.start_time_100ns(),
            executable_path: identity.executable_path().to_owned(),
        })
    }

    fn read_workbook(&self, workbook: &Self::Workbook) -> PlatformResult<WorkbookSnapshot> {
        Self::read_workbook(self, workbook).map(|snapshot| map_workbook_snapshot(&snapshot))
    }

    fn read_range(
        &self,
        workbook: &Self::Workbook,
        selector: &RangeSelector,
        mode: ReadMode,
    ) -> PlatformResult<RangeSnapshot> {
        let platform_mode = map_read_mode(mode);
        let snapshot =
            Self::read_range(self, workbook, map_range_selector(selector), platform_mode)?;
        if snapshot.mode() != platform_mode {
            return Err(PlatformError::new(
                ErrorCode::ToolInvalidArgs,
                "Excel runner returned a different range read mode",
            ));
        }
        Ok(map_range_snapshot(&snapshot))
    }

    fn fingerprint(&self, workbook: &Self::Workbook) -> PlatformResult<String> {
        Self::fingerprint_workbook(self, workbook)
    }

    fn verify_ownership(&self, workbook: &Self::Workbook) -> PlatformResult<()> {
        Self::verify_ownership(self, workbook)
    }

    fn close_workbook(&self, workbook: &Self::Workbook) -> PlatformResult<()> {
        Self::close_workbook(self, workbook)
    }

    fn shutdown(&mut self) -> PlatformResult<()> {
        Self::shutdown(self)
    }
}

fn map_workbook_snapshot(snapshot: &ExcelWorkbookSnapshot) -> WorkbookSnapshot {
    WorkbookSnapshot {
        workbook: WorkbookIdentity {
            full_path: snapshot.canonical_path().to_owned(),
            read_only: snapshot.read_only(),
        },
        worksheets: snapshot.worksheets().iter().map(map_worksheet).collect(),
        named_ranges: snapshot
            .named_ranges()
            .iter()
            .map(map_named_range)
            .collect(),
        calculation: super::CalculationInfo {
            mode: snapshot.calculation().mode().to_owned(),
            state: snapshot.calculation().state().to_owned(),
        },
    }
}

fn map_worksheet(
    worksheet: &assistant_platform_windows::excel_com_runner::ExcelWorksheetInfo,
) -> WorksheetInfo {
    WorksheetInfo {
        name: worksheet.name().to_owned(),
        ordinal: worksheet.ordinal(),
        used_range_address: worksheet.used_range_address().map(str::to_owned),
    }
}

fn map_named_range(
    named_range: &assistant_platform_windows::excel_com_runner::ExcelNamedRangeInfo,
) -> NamedRangeInfo {
    NamedRangeInfo {
        name: named_range.name().to_owned(),
        scope: match named_range.scope() {
            ExcelNamedRangeScope::Workbook => NamedRangeScope::Workbook,
            ExcelNamedRangeScope::Worksheet(name) => NamedRangeScope::Worksheet(name.clone()),
        },
        refers_to: named_range.refers_to().to_owned(),
    }
}

fn map_range_selector(selector: &RangeSelector) -> ExcelRangeSelector {
    match selector {
        RangeSelector::NamedRange(name) => ExcelRangeSelector::NamedRange(name.clone()),
        RangeSelector::A1 {
            worksheet,
            start,
            end,
        } => ExcelRangeSelector::A1 {
            worksheet: match worksheet {
                WorksheetSelector::Name(name) => ExcelWorksheetSelector::Name(name.clone()),
                WorksheetSelector::Ordinal(ordinal) => ExcelWorksheetSelector::Ordinal(*ordinal),
            },
            start_row: start.row,
            start_column: start.column,
            end_row: end.row,
            end_column: end.column,
        },
    }
}

const fn map_read_mode(mode: ReadMode) -> ExcelReadMode {
    match mode {
        ReadMode::Value2 => ExcelReadMode::Value2,
        ReadMode::Formula => ExcelReadMode::Formula,
        ReadMode::Text => ExcelReadMode::Text,
    }
}

fn map_range_snapshot(snapshot: &ExcelRangeSnapshot) -> RangeSnapshot {
    RangeSnapshot {
        address: snapshot.address().to_owned(),
        rows: snapshot.rows(),
        columns: snapshot.columns(),
        cells: snapshot
            .cells()
            .iter()
            .map(|cell| RuntimeCell {
                row: cell.row(),
                column: cell.column(),
                value_type: map_cell_value_type(cell.value_type()),
                value_text: cell.value_text().to_owned(),
            })
            .collect(),
    }
}

const fn map_cell_value_type(value_type: ExcelCellValueType) -> CellValueType {
    match value_type {
        ExcelCellValueType::Blank => CellValueType::Blank,
        ExcelCellValueType::Number => CellValueType::Number,
        ExcelCellValueType::Text => CellValueType::Text,
        ExcelCellValueType::Boolean => CellValueType::Boolean,
        ExcelCellValueType::Error => CellValueType::Error,
    }
}

#[cfg(test)]
#[path = "excel_t21_platform_runtime_tests.rs"]
mod tests;
