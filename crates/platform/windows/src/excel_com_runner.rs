//! Excel COM runner boundary.
//!
//! Responsibilities:
//! - keep every COM object on one dedicated worker thread;
//! - serialize requests through a bounded FIFO command channel;
//! - expose a narrow public handle for create, process identity, read-only open,
//!   cell text, close, and shutdown.
//! - keep human-present write calibration behind crate tests so it cannot leak
//!   into the production runner surface.
//!
//! Boundaries:
//! - this module exposes no model-visible Tool and is not registered in `ToolBus`;
//! - non-Windows tests use fake backends and never initialize COM;
//! - the public surface intentionally covers smoke acceptance only. Full Excel
//!   domain mapping remains in `agent-core` and a follow-up card.
//!
//! Invariants:
//! 1. the backend runs only on the worker thread and never crosses threads;
//! 2. request capacity is exactly one, and full/disconnected queues fail closed;
//! 3. backend errors preserve the existing `ErrorCode` without reinterpretation.
//!
//! Related documents: ADR-0108, ADR-0110, ADR-0111, TASK-399, TASK-401.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use assistant_platform_api::{ErrorCode, PlatformError, PlatformResult};
#[cfg(test)]
use dispatch::write_acceptance_command;
use dispatch::{send_response, shutdown_command, shutdown_result};
use excel_ownership_journal::ExcelOwnershipJournalStore;
use process_watchdog::{
    ExcelProcessWatchdog, ExcelProcessWatchdogController, WATCHDOG_RESPONSE_WAIT,
    WATCHDOG_TERMINATION_WAIT,
};
#[cfg(test)]
use state::OpenWorkbookState;
use state::RunnerState;

#[path = "excel_com_runner_api.rs"]
mod api;
pub use api::{
    ExcelCalculationInfo, ExcelCellValueType, ExcelNamedRangeInfo, ExcelNamedRangeScope,
    ExcelOwnedProcessIdentity, ExcelRangeSelector, ExcelRangeSnapshot, ExcelReadMode,
    ExcelRuntimeCell, ExcelWorkbookSnapshot, ExcelWorksheetInfo, ExcelWorksheetSelector,
    MAX_EXCEL_COLUMN, MAX_EXCEL_NAMED_RANGES, MAX_EXCEL_RANGE_BYTES, MAX_EXCEL_RANGE_CELLS,
    MAX_EXCEL_ROW, MAX_EXCEL_WORKSHEETS,
};
#[cfg(test)]
use write_acceptance::ExcelWriteAcceptanceRequest;

const REQUEST_QUEUE_CAPACITY: usize = 1;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const WORKER_REAP_TIMEOUT: Duration = Duration::from_secs(5);
const DROP_RESPONSE_TIMEOUT: Duration = Duration::from_secs(5);
const WORKER_THREAD_NAME: &str = "excel-com-runner";
const WORKER_REAP_POLL_INTERVAL: Duration = Duration::from_millis(20);
static NEXT_RUNNER_ID: AtomicU64 = AtomicU64::new(1);

/// Self-contained handle to one workbook owned by an Excel COM runner.
///
/// The handle contains no COM pointer. It can cross threads and is validated by
/// the worker before every workbook operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExcelWorkbookHandle {
    runner_id: u64,
    session_id: u64,
    canonical_path: PathBuf,
    owned_process: ExcelOwnedProcessIdentity,
}

impl ExcelWorkbookHandle {
    pub(super) const fn runner_id(&self) -> u64 {
        self.runner_id
    }

    /// Returns the worker-local session id.
    #[must_use]
    pub const fn session_id(&self) -> u64 {
        self.session_id
    }

    /// Returns the owned Excel process identity.
    #[must_use]
    pub const fn owned_process(&self) -> &ExcelOwnedProcessIdentity {
        &self.owned_process
    }

    /// Returns the canonical workbook path observed during open.
    #[must_use]
    pub fn canonical_path(&self) -> &Path {
        &self.canonical_path
    }
}

/// Text observed from one cell on the active worksheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExcelCellText {
    address: String,
    value: String,
}

impl ExcelCellText {
    /// Returns the normalized input address.
    #[must_use]
    pub fn address(&self) -> &str {
        &self.address
    }

    /// Returns the Excel `.Text` value.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }
}

/// Private backend executed exclusively by the dedicated runner thread.
trait ExcelComBackend: 'static {
    fn create_application(&mut self) -> PlatformResult<()>;
    fn application_identity(&mut self) -> PlatformResult<ExcelOwnedProcessIdentity>;
    fn quit_application(&mut self) -> PlatformResult<()>;

    fn open_read_only(&mut self, _path: &Path) -> PlatformResult<()> {
        Err(runner_protocol_error(
            "Excel COM backend does not support read-only workbook open",
        ))
    }

    fn read_active_sheet_text(&self, _address: &str) -> PlatformResult<String> {
        Err(runner_protocol_error(
            "Excel COM backend does not support active-sheet cell text",
        ))
    }

    fn close_workbook(&mut self) -> PlatformResult<()> {
        Err(runner_protocol_error(
            "Excel COM backend does not support workbook close",
        ))
    }

    fn read_workbook(&mut self, _path: &Path) -> PlatformResult<ExcelWorkbookSnapshot> {
        Err(runner_protocol_error(
            "Excel COM backend does not support workbook metadata",
        ))
    }

    fn read_range(
        &mut self,
        _selector: &ExcelRangeSelector,
        _mode: ExcelReadMode,
    ) -> PlatformResult<ExcelRangeSnapshot> {
        Err(runner_protocol_error(
            "Excel COM backend does not support range reads",
        ))
    }

    fn verify_ownership(&self, _identity: &ExcelOwnedProcessIdentity) -> PlatformResult<()> {
        Err(runner_protocol_error(
            "Excel COM backend does not support ownership verification",
        ))
    }

    #[cfg(test)]
    fn execute_write_acceptance(
        &mut self,
        _request: &ExcelWriteAcceptanceRequest,
    ) -> PlatformResult<()> {
        Err(runner_protocol_error(
            "Excel COM backend does not support write acceptance",
        ))
    }
}

/// One response value produced by the worker.
#[derive(Debug, Clone, PartialEq, Eq)]
enum RunnerValue {
    Unit,
    ProcessId(u32),
    WorkbookOpened(ExcelWorkbookHandle),
    CellText(String),
    WorkbookSnapshot(ExcelWorkbookSnapshot),
    RangeSnapshot(ExcelRangeSnapshot),
}

type RunnerResponse = PlatformResult<RunnerValue>;
type ResponseSender = SyncSender<RunnerResponse>;

enum RunnerCommand {
    CreateApplication(ResponseSender),
    ApplicationProcessId(ResponseSender),
    #[cfg(test)]
    QuitApplication(ResponseSender),
    OpenReadOnlyWorkbook {
        path: PathBuf,
        response: ResponseSender,
    },
    ReadActiveSheetText {
        runner_id: u64,
        session_id: u64,
        address: String,
        response: ResponseSender,
    },
    ReadWorkbook {
        runner_id: u64,
        session_id: u64,
        response: ResponseSender,
    },
    ReadRange {
        runner_id: u64,
        session_id: u64,
        selector: ExcelRangeSelector,
        mode: ExcelReadMode,
        response: ResponseSender,
    },
    VerifyOwnership {
        runner_id: u64,
        session_id: u64,
        response: ResponseSender,
    },
    #[cfg(test)]
    WriteAcceptance {
        request: ExcelWriteAcceptanceRequest,
        response: ResponseSender,
    },
    CloseWorkbook {
        runner_id: u64,
        session_id: u64,
        response: ResponseSender,
    },
    Shutdown(ResponseSender),
}

/// Request handle for one dedicated Excel COM worker.
///
/// COM objects and apartment guards remain on the worker thread. The handle
/// itself only carries a bounded command channel.
pub struct ExcelComRunner {
    sender: SyncSender<RunnerCommand>,
    worker: Option<JoinHandle<PlatformResult<()>>>,
    watchdog: Option<ExcelProcessWatchdog>,
    shutdown_started: bool,
}

impl ExcelComRunner {
    /// Starts the production Windows Excel runner without creating Excel.
    ///
    /// # Errors
    ///
    /// Returns `Fatal` when the worker thread or its STA apartment cannot be
    /// initialized within the bounded readiness window.
    #[cfg(windows)]
    pub fn start() -> PlatformResult<Self> {
        windows_backend::spawn_windows_excel_runner()
    }

    /// Starts a fake or test backend on one dedicated worker thread.
    #[cfg(test)]
    fn spawn<B, F>(backend_factory: F) -> PlatformResult<Self>
    where
        B: ExcelComBackend,
        F: FnOnce() -> B + Send + 'static,
    {
        Self::spawn_with_journal_store(backend_factory, ExcelOwnershipJournalStore::for_tests()?)
    }

    #[cfg(test)]
    fn spawn_with_journal_store<B, F>(
        backend_factory: F,
        journal_store: ExcelOwnershipJournalStore,
    ) -> PlatformResult<Self>
    where
        B: ExcelComBackend,
        F: FnOnce() -> B + Send + 'static,
    {
        let runner_id = next_runner_id();
        let (mut watchdog, watchdog_controller) = ExcelProcessWatchdog::start_for_tests()?;
        let (sender, receiver) = mpsc::sync_channel(REQUEST_QUEUE_CAPACITY);
        let worker = match thread::Builder::new()
            .name(WORKER_THREAD_NAME.to_owned())
            .spawn(move || {
                let backend = backend_factory();
                run_worker(
                    backend,
                    &receiver,
                    runner_id,
                    journal_store,
                    watchdog_controller,
                )
            }) {
            Ok(worker) => worker,
            Err(error) => {
                let spawn_error = thread_spawn_error("Excel COM runner", &error);
                if let Err(cleanup_error) = watchdog.shutdown(WATCHDOG_RESPONSE_WAIT) {
                    return Err(PlatformError::new(
                        ErrorCode::Fatal,
                        format!("{spawn_error}; watchdog cleanup failed: {cleanup_error}"),
                    ));
                }
                return Err(spawn_error);
            }
        };
        Ok(Self {
            sender,
            worker: Some(worker),
            watchdog: Some(watchdog),
            shutdown_started: false,
        })
    }

    /// Creates one owned Excel application without opening a workbook.
    ///
    /// # Errors
    ///
    /// Returns the backend failure unchanged. Repeated calls fail explicitly.
    pub fn create_application(&self) -> PlatformResult<()> {
        expect_unit(self.dispatch(RunnerCommand::CreateApplication))
    }

    /// Returns the process id derived from the owned Excel `Application.Hwnd`.
    ///
    /// # Errors
    ///
    /// Returns a protocol failure when the application was not created or the
    /// window does not map to a live process.
    pub fn application_process_id(&self) -> PlatformResult<u32> {
        match self.dispatch(RunnerCommand::ApplicationProcessId)? {
            RunnerValue::ProcessId(process_id) => Ok(process_id),
            _ => Err(runner_protocol_error(
                "Excel COM runner returned no process id",
            )),
        }
    }

    /// Opens one local `.xlsx` workbook read-only.
    ///
    /// # Errors
    ///
    /// Returns `ToolInvalidArgs` for non-absolute, non-local, non-`.xlsx`, or
    /// non-file paths; backend open failures retain their typed error.
    pub fn open_read_only(
        &self,
        workbook_path: impl AsRef<Path>,
    ) -> PlatformResult<ExcelWorkbookHandle> {
        let path = crate::excel_process::validate_workbook_path(workbook_path.as_ref())?;
        match self.dispatch(|response| RunnerCommand::OpenReadOnlyWorkbook { path, response })? {
            RunnerValue::WorkbookOpened(workbook) => Ok(workbook),
            _ => Err(runner_protocol_error(
                "Excel COM runner returned no workbook handle",
            )),
        }
    }

    /// Reads `.Text` from one cell on the active worksheet.
    ///
    /// # Errors
    ///
    /// Returns `ToolInvalidArgs` for malformed addresses, or an explicit backend
    /// failure when the workbook/session no longer matches.
    pub fn read_active_sheet_text(
        &self,
        workbook: &ExcelWorkbookHandle,
        address: &str,
    ) -> PlatformResult<ExcelCellText> {
        let address = crate::excel_process::validate_cell_address(address)?;
        let runner_id = workbook.runner_id;
        let session_id = workbook.session_id;
        match self.dispatch(|response| RunnerCommand::ReadActiveSheetText {
            runner_id,
            session_id,
            address: address.clone(),
            response,
        })? {
            RunnerValue::CellText(value) => Ok(ExcelCellText { address, value }),
            _ => Err(runner_protocol_error(
                "Excel COM runner returned no cell text",
            )),
        }
    }

    /// Closes one owned workbook without saving.
    ///
    /// # Errors
    ///
    /// Returns `ToolInvalidArgs` for a stale/mismatched handle or the backend
    /// close failure unchanged.
    pub fn close_workbook(&self, workbook: &ExcelWorkbookHandle) -> PlatformResult<()> {
        expect_unit(self.dispatch(|response| RunnerCommand::CloseWorkbook {
            runner_id: workbook.runner_id,
            session_id: workbook.session_id,
            response,
        }))
    }

    #[cfg(test)]
    fn quit_application(&self) -> PlatformResult<()> {
        expect_unit(self.dispatch(RunnerCommand::QuitApplication))
    }

    /// Closes any open workbook, quits Excel, and joins the worker.
    ///
    /// # Errors
    ///
    /// Returns `Fatal` when the bounded shutdown request, verified process
    /// cleanup, or worker reap deadline expires. Repeated shutdown is rejected.
    pub fn shutdown(&mut self) -> PlatformResult<()> {
        if self.shutdown_started {
            return Err(runner_protocol_error(
                "Excel COM runner shutdown was already requested",
            ));
        }
        self.shutdown_started = true;
        let response = self.dispatch(RunnerCommand::Shutdown);
        match response {
            Ok(value) => {
                let worker_result =
                    expect_unit(Ok(value)).and_then(|()| self.reap_worker_bounded());
                let watchdog_result = self.shutdown_watchdog();
                combine_cleanup_results(worker_result, watchdog_result, "runner shutdown")
            }
            Err(error) => {
                let public_error = if error.code() == ErrorCode::TargetUnresponsive {
                    PlatformError::new(
                        ErrorCode::Fatal,
                        format!("Excel cleanup request did not become bounded: {error}"),
                    )
                } else {
                    error
                };
                let force_result = self.force_watchdog_cleanup();
                let force_succeeded = force_result.is_ok();
                let reap_result = self.reap_worker_bounded();
                let cleanup_result =
                    combine_cleanup_results(force_result, reap_result, "forced runner cleanup");
                let watchdog_result = if force_succeeded {
                    self.shutdown_watchdog()
                } else {
                    // Keep the armed supervisor available for the final Drop retry.
                    Ok(())
                };
                let cleanup_result =
                    combine_cleanup_results(cleanup_result, watchdog_result, "watchdog shutdown");
                combine_original_and_cleanup(public_error, cleanup_result)
            }
        }
    }

    fn dispatch(
        &self,
        command: impl FnOnce(ResponseSender) -> RunnerCommand,
    ) -> PlatformResult<RunnerValue> {
        self.dispatch_with_timeout(command, REQUEST_TIMEOUT)
    }

    fn dispatch_with_timeout(
        &self,
        command: impl FnOnce(ResponseSender) -> RunnerCommand,
        timeout: Duration,
    ) -> PlatformResult<RunnerValue> {
        let (response_sender, response_receiver) = mpsc::sync_channel(1);
        self.sender
            .try_send(command(response_sender))
            .map_err(|error| queue_error(&error))?;
        response_receiver
            .recv_timeout(timeout)
            .map_err(response_wait_error)?
    }

    fn force_watchdog_cleanup(&self) -> PlatformResult<()> {
        let watchdog = self.watchdog.as_ref().ok_or_else(|| {
            runner_protocol_error("Excel process watchdog is unavailable during forced cleanup")
        })?;
        watchdog
            .controller()
            .force_terminate_armed(WATCHDOG_TERMINATION_WAIT, WATCHDOG_RESPONSE_WAIT)
            .map(|_report| ())
    }

    fn reap_worker_bounded(&mut self) -> PlatformResult<()> {
        let Some(worker) = self.worker.as_ref() else {
            return Ok(());
        };
        let deadline = Instant::now() + WORKER_REAP_TIMEOUT;
        while !worker.is_finished() {
            if Instant::now() >= deadline {
                return Err(PlatformError::new(
                    ErrorCode::Fatal,
                    "Excel COM worker did not reach its reap deadline",
                ));
            }
            thread::sleep(WORKER_REAP_POLL_INTERVAL);
        }
        let worker = self
            .worker
            .take()
            .ok_or_else(|| runner_protocol_error("Excel COM worker handle disappeared"))?;
        worker.join().map_err(join_error)?
    }

    fn shutdown_watchdog(&mut self) -> PlatformResult<()> {
        self.watchdog.take().map_or(Ok(()), |mut watchdog| {
            watchdog.shutdown(WATCHDOG_RESPONSE_WAIT)
        })
    }
}

impl Drop for ExcelComRunner {
    fn drop(&mut self) {
        if self.shutdown_started && self.worker.is_none() && self.watchdog.is_none() {
            return;
        }
        self.shutdown_started = true;
        // Drop is the final best-effort path. Explicit `shutdown()` is the only
        // path that can report cleanup failure to the caller.
        let response = self.dispatch_with_timeout(RunnerCommand::Shutdown, DROP_RESPONSE_TIMEOUT);
        let cleanup_result = match response {
            Ok(value) => expect_unit(Ok(value)).and_then(|()| self.reap_worker_bounded()),
            Err(_error) => {
                let force_result = self.force_watchdog_cleanup();
                let reap_result = self.reap_worker_bounded();
                combine_cleanup_results(force_result, reap_result, "Drop forced cleanup")
            }
        };
        let watchdog_result = self.shutdown_watchdog();
        match combine_cleanup_results(cleanup_result, watchdog_result, "Drop watchdog shutdown") {
            Ok(()) => {}
            Err(_error) => {
                // Drop cannot return an error. Failed cleanup leaves the durable
                // TASK-411 journal on disk for the incident path.
            }
        }
    }
}

fn run_worker<B>(
    mut backend: B,
    receiver: &Receiver<RunnerCommand>,
    runner_id: u64,
    journal_store: ExcelOwnershipJournalStore,
    watchdog: ExcelProcessWatchdogController,
) -> PlatformResult<()>
where
    B: ExcelComBackend,
{
    let mut state = RunnerState::new(runner_id, journal_store, watchdog);
    while let Ok(command) = receiver.recv() {
        let (response, result, is_shutdown) = match command {
            RunnerCommand::CreateApplication(response) => {
                (response, state.create_application(&mut backend), false)
            }
            RunnerCommand::ApplicationProcessId(response) => {
                (response, state.application_process_id(), false)
            }
            #[cfg(test)]
            RunnerCommand::QuitApplication(response) => {
                (response, state.quit_application(&mut backend), false)
            }
            RunnerCommand::OpenReadOnlyWorkbook { path, response } => {
                (response, state.open_read_only(&mut backend, &path), false)
            }
            RunnerCommand::ReadActiveSheetText {
                runner_id,
                session_id,
                address,
                response,
            } => (
                response,
                state.read_active_sheet_text(&backend, runner_id, session_id, &address),
                false,
            ),
            RunnerCommand::ReadWorkbook {
                runner_id,
                session_id,
                response,
            } => (
                response,
                state.read_workbook(&mut backend, runner_id, session_id),
                false,
            ),
            RunnerCommand::ReadRange {
                runner_id,
                session_id,
                selector,
                mode,
                response,
            } => (
                response,
                state.read_range(&mut backend, runner_id, session_id, &selector, mode),
                false,
            ),
            RunnerCommand::VerifyOwnership {
                runner_id,
                session_id,
                response,
            } => (
                response,
                state.verify_ownership(&backend, runner_id, session_id),
                false,
            ),
            #[cfg(test)]
            RunnerCommand::WriteAcceptance { request, response } => {
                write_acceptance_command(&mut state, &mut backend, response, &request)
            }
            RunnerCommand::CloseWorkbook {
                runner_id,
                session_id,
                response,
            } => (
                response,
                state.close_workbook(&mut backend, runner_id, session_id),
                false,
            ),
            RunnerCommand::Shutdown(response) => {
                shutdown_command(&mut state, &mut backend, response)
            }
        };
        if send_response(&response, result).is_err() {
            return state.abandon(&mut backend);
        }
        if is_shutdown {
            // The terminal response is the caller-visible result. Returning it
            // again from the worker would be joined as a second cleanup failure.
            return shutdown_result(Ok(RunnerValue::Unit));
        }
    }
    state.abandon(&mut backend)
}

fn expect_unit(response: RunnerResponse) -> PlatformResult<()> {
    match response? {
        RunnerValue::Unit => Ok(()),
        _ => Err(runner_protocol_error(
            "Excel COM runner returned an unexpected process id",
        )),
    }
}

fn queue_error(error: &TrySendError<RunnerCommand>) -> PlatformError {
    match error {
        TrySendError::Full(_) => PlatformError::new(
            ErrorCode::TargetUnresponsive,
            "Excel COM runner request queue is full",
        ),
        TrySendError::Disconnected(_) => runner_protocol_error(
            "Excel COM runner worker disconnected before accepting the request",
        ),
    }
}

fn response_wait_error(error: RecvTimeoutError) -> PlatformError {
    match error {
        RecvTimeoutError::Timeout => PlatformError::new(
            ErrorCode::TargetUnresponsive,
            "Excel COM runner request timed out before producing a response",
        ),
        RecvTimeoutError::Disconnected => runner_protocol_error(
            "Excel COM runner worker disconnected without producing a response",
        ),
    }
}

fn thread_spawn_error(context: &str, error: &io::Error) -> PlatformError {
    PlatformError::new(
        ErrorCode::Fatal,
        format!("{context} worker thread spawn failed: {error}"),
    )
}

fn join_error(_panic: Box<dyn std::any::Any + Send>) -> PlatformError {
    runner_protocol_error("Excel COM runner worker thread panicked")
}

#[cfg(test)]
fn join_worker_bounded(
    worker: JoinHandle<PlatformResult<()>>,
    timeout: Duration,
) -> PlatformResult<()> {
    let deadline = Instant::now() + timeout;
    while !worker.is_finished() {
        if Instant::now() >= deadline {
            return Err(PlatformError::new(
                ErrorCode::Fatal,
                "Excel COM worker did not reach its reap deadline",
            ));
        }
        thread::sleep(WORKER_REAP_POLL_INTERVAL);
    }
    worker.join().map_err(join_error)?
}

fn runner_protocol_error(message: impl Into<String>) -> PlatformError {
    PlatformError::new(ErrorCode::Fatal, message)
}

fn next_runner_id() -> u64 {
    loop {
        let runner_id = NEXT_RUNNER_ID.fetch_add(1, Ordering::Relaxed);
        if runner_id != 0 {
            return runner_id;
        }
    }
}

fn combine_cleanup_results(
    first: PlatformResult<()>,
    second: PlatformResult<()>,
    context: &str,
) -> PlatformResult<()> {
    match (first, second) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(first_error), Err(second_error)) => Err(PlatformError::new(
            ErrorCode::Fatal,
            format!("{context}: {first_error}; cleanup also failed: {second_error}"),
        )),
    }
}

fn combine_original_and_cleanup(
    original: PlatformError,
    cleanup: PlatformResult<()>,
) -> PlatformResult<()> {
    match cleanup {
        Ok(()) => Err(original),
        Err(cleanup_error) => Err(PlatformError::new(
            ErrorCode::Fatal,
            format!("{original}; cleanup also failed: {cleanup_error}"),
        )),
    }
}

#[cfg(windows)]
#[path = "excel_com_runner_windows.rs"]
mod windows_backend;

#[cfg(any(windows, test))]
#[path = "excel_process_watchdog.rs"]
mod process_watchdog;

#[cfg(windows)]
#[path = "excel_process_watchdog_windows.rs"]
mod process_watchdog_windows;

#[path = "excel_com_runner_dispatch.rs"]
mod dispatch;

#[path = "excel_ownership_journal.rs"]
mod excel_ownership_journal;

#[path = "excel_com_runner_state.rs"]
mod state;

#[cfg(test)]
#[path = "excel_com_runner_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "excel_com_runner_write.rs"]
mod write_acceptance;

#[cfg(test)]
pub(crate) use write_acceptance::ExcelScalarValue;
