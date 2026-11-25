//! Excel COM runner boundary.
//!
//! Responsibilities:
//! - keep every COM object on one dedicated worker thread;
//! - serialize requests through a bounded FIFO command channel;
//! - expose a narrow public handle for create, process identity, read-only open,
//!   cell text, close, and shutdown.
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
use std::time::Duration;

use assistant_platform_api::{ErrorCode, PlatformError, PlatformResult};

const REQUEST_QUEUE_CAPACITY: usize = 1;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const WORKER_THREAD_NAME: &str = "excel-com-runner";
const MAX_CELL_ADDRESS_BYTES: usize = 32;
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
}

impl ExcelWorkbookHandle {
    /// Returns the worker-local session id.
    #[must_use]
    pub const fn session_id(&self) -> u64 {
        self.session_id
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
    fn application_process_id(&mut self) -> PlatformResult<u32>;
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
}

/// One response value produced by the worker.
#[derive(Debug, Clone, PartialEq, Eq)]
enum RunnerValue {
    Unit,
    ProcessId(u32),
    WorkbookOpened(ExcelWorkbookHandle),
    CellText(String),
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
        let runner_id = next_runner_id();
        let (sender, receiver) = mpsc::sync_channel(REQUEST_QUEUE_CAPACITY);
        let worker = thread::Builder::new()
            .name(WORKER_THREAD_NAME.to_owned())
            .spawn(move || {
                let backend = backend_factory();
                run_worker(backend, &receiver, runner_id)
            })
            .map_err(|error| thread_spawn_error("Excel COM runner", &error))?;
        Ok(Self {
            sender,
            worker: Some(worker),
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
        let path = validate_workbook_path(workbook_path.as_ref())?;
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
        let address = validate_cell_address(address)?;
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
    /// Returns `TargetUnresponsive` when the shutdown request times out and
    /// releases the join handle only after explicitly reporting the need for
    /// process-level cleanup. Repeated shutdown is rejected.
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
                expect_unit(Ok(value))?;
                self.worker.take().map_or(Ok(()), join_worker)
            }
            Err(error) if error.code() == ErrorCode::TargetUnresponsive => Err(PlatformError::new(
                ErrorCode::Fatal,
                format!(
                    "{error}; Excel COM worker was detached after shutdown timed out and requires process-level cleanup"
                ),
            )),
            Err(error) => {
                let cleanup = self.worker.take().map_or(Ok(()), join_worker);
                match cleanup {
                    Ok(()) => Err(error),
                    Err(cleanup_error) => Err(PlatformError::new(
                        ErrorCode::Fatal,
                        format!("{error}; worker cleanup also failed: {cleanup_error}"),
                    )),
                }
            }
        }
    }

    fn dispatch(
        &self,
        command: impl FnOnce(ResponseSender) -> RunnerCommand,
    ) -> PlatformResult<RunnerValue> {
        let (response_sender, response_receiver) = mpsc::sync_channel(1);
        self.sender
            .try_send(command(response_sender))
            .map_err(|error| queue_error(&error))?;
        response_receiver
            .recv_timeout(REQUEST_TIMEOUT)
            .map_err(response_wait_error)?
    }
}

impl Drop for ExcelComRunner {
    fn drop(&mut self) {
        if self.shutdown_started || self.worker.is_none() {
            return;
        }
        self.shutdown_started = true;
        // Drop is the final best-effort path. Explicit `shutdown()` is the only
        // path that can report cleanup failure to the caller.
        if self.dispatch(RunnerCommand::Shutdown).is_ok()
            && let Some(worker) = self.worker.take()
        {
            let _join_result = worker.join();
        }
    }
}

fn run_worker<B>(
    mut backend: B,
    receiver: &Receiver<RunnerCommand>,
    runner_id: u64,
) -> PlatformResult<()>
where
    B: ExcelComBackend,
{
    let mut state = RunnerState::new(runner_id);
    while let Ok(command) = receiver.recv() {
        match command {
            RunnerCommand::CreateApplication(response) => {
                let result = state.create_application(&mut backend);
                if send_response(&response, result).is_err() {
                    return state.abandon(&mut backend);
                }
            }
            RunnerCommand::ApplicationProcessId(response) => {
                let result = state.application_process_id(&mut backend);
                if send_response(&response, result).is_err() {
                    return state.abandon(&mut backend);
                }
            }
            #[cfg(test)]
            RunnerCommand::QuitApplication(response) => {
                let result = state.quit_application(&mut backend);
                if send_response(&response, result).is_err() {
                    return state.abandon(&mut backend);
                }
            }
            RunnerCommand::OpenReadOnlyWorkbook { path, response } => {
                let result = state.open_read_only(&mut backend, &path);
                if send_response(&response, result).is_err() {
                    return state.abandon(&mut backend);
                }
            }
            RunnerCommand::ReadActiveSheetText {
                runner_id,
                session_id,
                address,
                response,
            } => {
                let result =
                    state.read_active_sheet_text(&backend, runner_id, session_id, &address);
                if send_response(&response, result).is_err() {
                    return state.abandon(&mut backend);
                }
            }
            RunnerCommand::CloseWorkbook {
                runner_id,
                session_id,
                response,
            } => {
                let result = state.close_workbook(&mut backend, runner_id, session_id);
                if send_response(&response, result).is_err() {
                    return state.abandon(&mut backend);
                }
            }
            RunnerCommand::Shutdown(response) => {
                let result = state.shutdown(&mut backend);
                if send_response(&response, result.clone()).is_err() {
                    return state.abandon(&mut backend);
                }
                return result.map(|_| ());
            }
        }
    }
    state.abandon(&mut backend)
}

fn send_response(sender: &ResponseSender, response: RunnerResponse) -> Result<(), ()> {
    sender.send(response).map_err(|_| ())
}

struct RunnerState {
    runner_id: u64,
    created: bool,
    closed: bool,
    process_id: Option<u32>,
    next_session_id: u64,
    workbook: Option<OpenWorkbookState>,
}

struct OpenWorkbookState {
    session_id: u64,
}

impl RunnerState {
    const fn new(runner_id: u64) -> Self {
        Self {
            runner_id,
            created: false,
            closed: false,
            process_id: None,
            next_session_id: 0,
            workbook: None,
        }
    }

    fn create_application<B>(&mut self, backend: &mut B) -> RunnerResponse
    where
        B: ExcelComBackend,
    {
        if self.closed {
            return Err(runner_protocol_error(
                "Excel COM runner is closed and cannot create another application",
            ));
        }
        if self.created {
            return Err(runner_protocol_error(
                "Excel COM application was already created",
            ));
        }
        backend.create_application()?;
        self.created = true;
        Ok(RunnerValue::Unit)
    }

    fn application_process_id<B>(&mut self, backend: &mut B) -> RunnerResponse
    where
        B: ExcelComBackend,
    {
        if !self.created {
            return Err(runner_protocol_error(
                "Excel COM application must be created before reading its process id",
            ));
        }
        if let Some(process_id) = self.process_id {
            return Ok(RunnerValue::ProcessId(process_id));
        }
        let process_id = backend.application_process_id()?;
        if process_id == 0 {
            return Err(runner_protocol_error(
                "Excel COM application returned process id 0",
            ));
        }
        self.process_id = Some(process_id);
        Ok(RunnerValue::ProcessId(process_id))
    }

    fn quit_application<B>(&mut self, backend: &mut B) -> RunnerResponse
    where
        B: ExcelComBackend,
    {
        if !self.created {
            return Err(runner_protocol_error(
                "Excel COM application must be created before quit",
            ));
        }
        backend.quit_application()?;
        self.created = false;
        self.closed = true;
        Ok(RunnerValue::Unit)
    }

    fn open_read_only<B>(&mut self, backend: &mut B, path: &Path) -> RunnerResponse
    where
        B: ExcelComBackend,
    {
        if !self.created || self.closed {
            return Err(runner_protocol_error(
                "Excel COM application must be active before opening a workbook",
            ));
        }
        if self.workbook.is_some() {
            return Err(runner_protocol_error(
                "Excel COM runner already owns an open workbook",
            ));
        }
        backend.open_read_only(path)?;
        let session_id = self
            .next_session_id
            .checked_add(1)
            .ok_or_else(|| runner_protocol_error("Excel workbook session id overflowed"))?;
        self.next_session_id = session_id;
        let workbook = ExcelWorkbookHandle {
            runner_id: self.runner_id,
            session_id,
            canonical_path: path.to_path_buf(),
        };
        self.workbook = Some(OpenWorkbookState { session_id });
        Ok(RunnerValue::WorkbookOpened(workbook))
    }

    fn read_active_sheet_text<B>(
        &self,
        backend: &B,
        runner_id: u64,
        session_id: u64,
        address: &str,
    ) -> RunnerResponse
    where
        B: ExcelComBackend,
    {
        let workbook = self.workbook.as_ref().ok_or_else(|| {
            PlatformError::new(
                ErrorCode::ToolInvalidArgs,
                "Excel COM runner has no open workbook",
            )
        })?;
        if self.runner_id != runner_id || workbook.session_id != session_id {
            return Err(PlatformError::new(
                ErrorCode::ToolInvalidArgs,
                "Excel COM workbook handle does not match the open workbook",
            ));
        }
        backend
            .read_active_sheet_text(address)
            .map(RunnerValue::CellText)
    }

    fn close_workbook<B>(
        &mut self,
        backend: &mut B,
        runner_id: u64,
        session_id: u64,
    ) -> RunnerResponse
    where
        B: ExcelComBackend,
    {
        let workbook = self.workbook.as_ref().ok_or_else(|| {
            PlatformError::new(
                ErrorCode::ToolInvalidArgs,
                "Excel COM runner has no open workbook",
            )
        })?;
        if self.runner_id != runner_id || workbook.session_id != session_id {
            return Err(PlatformError::new(
                ErrorCode::ToolInvalidArgs,
                "Excel COM workbook handle does not match the open workbook",
            ));
        }
        backend.close_workbook()?;
        self.workbook = None;
        Ok(RunnerValue::Unit)
    }

    fn shutdown<B>(&mut self, backend: &mut B) -> RunnerResponse
    where
        B: ExcelComBackend,
    {
        let open_session_id = self.workbook.as_ref().map(|workbook| workbook.session_id);
        let close_result = open_session_id.map_or(Ok(RunnerValue::Unit), |session_id| {
            self.close_workbook(backend, self.runner_id, session_id)
        });
        let quit_result = if self.created {
            self.quit_application(backend)
        } else {
            self.closed = true;
            Ok(RunnerValue::Unit)
        };
        match (close_result, quit_result) {
            (Ok(_), Ok(_)) => Ok(RunnerValue::Unit),
            (Err(close_error), Ok(_)) => Err(PlatformError::new(
                ErrorCode::Fatal,
                format!("Excel workbook close failed during shutdown: {close_error}"),
            )),
            (Ok(_), Err(quit_error)) => Err(quit_error),
            (Err(close_error), Err(quit_error)) => Err(PlatformError::new(
                ErrorCode::Fatal,
                format!(
                    "Excel cleanup failed: workbook close={close_error}; application quit={quit_error}"
                ),
            )),
        }
    }

    fn abandon<B>(&mut self, backend: &mut B) -> PlatformResult<()>
    where
        B: ExcelComBackend,
    {
        if !self.created && self.workbook.is_none() {
            return Ok(());
        }
        let cleanup = self.shutdown(backend);
        cleanup.map_or_else(
            |error| {
                Err(PlatformError::new(
                    ErrorCode::Fatal,
                    format!("Excel COM request receiver disappeared and cleanup failed: {error}"),
                ))
            },
            |_value| Ok(()),
        )
    }
}

fn expect_unit(response: RunnerResponse) -> PlatformResult<()> {
    match response? {
        RunnerValue::Unit => Ok(()),
        _ => Err(runner_protocol_error(
            "Excel COM runner returned an unexpected process id",
        )),
    }
}

fn validate_workbook_path(path: &Path) -> PlatformResult<PathBuf> {
    if !path.is_absolute() {
        return Err(PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            "Excel workbook path must be absolute",
        ));
    }
    if !has_xlsx_extension(path) {
        return Err(PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            "Excel workbook path must use the .xlsx extension",
        ));
    }
    if path.components().any(|component| {
        matches!(
            component,
            std::path::Component::CurDir | std::path::Component::ParentDir
        )
    }) {
        return Err(PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            "Excel workbook path must not contain . or .. components",
        ));
    }
    let canonical = std::fs::canonicalize(path).map_err(|error| {
        let code = if error.kind() == io::ErrorKind::NotFound {
            ErrorCode::TargetNotFound
        } else if error.kind() == io::ErrorKind::PermissionDenied {
            ErrorCode::PlatformPermission
        } else {
            ErrorCode::ToolInvalidArgs
        };
        PlatformError::new(
            code,
            format!("canonicalize Excel workbook path failed: {error}"),
        )
    })?;
    if !canonical.is_file() {
        return Err(PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            "Excel workbook path must identify a regular file",
        ));
    }
    if !has_xlsx_extension(&canonical) {
        return Err(PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            "canonical Excel workbook target must use the .xlsx extension",
        ));
    }
    let canonical_text = canonical.to_string_lossy();
    if canonical_text.starts_with(r"\\?\UNC\") {
        return Err(PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            "Excel workbook path must be local, not a UNC share",
        ));
    }
    let canonical = canonical_text
        .strip_prefix(r"\\?\")
        .map_or_else(|| canonical.clone(), PathBuf::from);
    #[cfg(windows)]
    crate::excel_process::ensure_local_drive(&canonical)?;
    Ok(canonical)
}

fn validate_cell_address(address: &str) -> PlatformResult<String> {
    let normalized = address.trim().to_ascii_uppercase();
    if normalized.is_empty()
        || normalized.len() > MAX_CELL_ADDRESS_BYTES
        || !is_single_a1_cell(&normalized)
    {
        return Err(PlatformError::new(
            ErrorCode::ToolInvalidArgs,
            "Excel cell address must be a bounded single A1 cell reference",
        ));
    }
    Ok(normalized)
}

fn has_xlsx_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("xlsx"))
}

fn is_single_a1_cell(address: &str) -> bool {
    let bytes = address.as_bytes();
    let mut position = 0;
    if bytes.get(position) == Some(&b'$') {
        position += 1;
    }
    let column_start = position;
    while bytes.get(position).is_some_and(u8::is_ascii_uppercase) {
        position += 1;
    }
    let column_length = position.saturating_sub(column_start);
    if !(1..=3).contains(&column_length) {
        return false;
    }
    let column = bytes
        .get(column_start..position)
        .unwrap_or_default()
        .iter()
        .fold(0_u32, |value, byte| {
            value
                .saturating_mul(26)
                .saturating_add(u32::from(byte.saturating_sub(b'A') + 1))
        });
    if !(1..=16_384).contains(&column) {
        return false;
    }
    if bytes.get(position) == Some(&b'$') {
        position += 1;
    }
    let row_start = position;
    while bytes.get(position).is_some_and(u8::is_ascii_digit) {
        position += 1;
    }
    let row_length = position.saturating_sub(row_start);
    if !(1..=7).contains(&row_length)
        || bytes.get(row_start) == Some(&b'0')
        || position != bytes.len()
    {
        return false;
    }
    bytes
        .get(row_start..position)
        .and_then(|digits| std::str::from_utf8(digits).ok())
        .and_then(|digits| digits.parse::<u32>().ok())
        .is_some_and(|value| (1..=1_048_576).contains(&value))
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

fn join_worker(worker: JoinHandle<PlatformResult<()>>) -> PlatformResult<()> {
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

#[cfg(windows)]
#[path = "excel_com_runner_windows.rs"]
mod windows_backend;

#[cfg(test)]
#[path = "excel_com_runner_tests.rs"]
mod tests;
