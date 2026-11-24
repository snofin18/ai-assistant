//! Test-only Excel COM runner prototype.
//!
//! Responsibilities:
//! - keep every COM object on one dedicated worker thread;
//! - serialize requests through a bounded FIFO command channel;
//! - expose explicit create, process-id, quit, and shutdown operations to tests.
//!
//! Boundaries:
//! - this module is compiled only for crate tests;
//! - it is not connected to production assembly, Adapter declarations, or `ToolBus`;
//! - non-Windows tests use fake backends and never initialize COM.
//!
//! Invariants:
//! 1. the backend runs only on the worker thread and never crosses threads;
//! 2. request capacity is exactly one, and full/disconnected queues fail closed;
//! 3. backend errors preserve the existing `ErrorCode` without reinterpretation.
//!
//! Related documents: ADR-0108, ADR-0110, and TASK-399.

use std::io;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use assistant_platform_api::{ErrorCode, PlatformError, PlatformResult};

const REQUEST_QUEUE_CAPACITY: usize = 1;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const WORKER_THREAD_NAME: &str = "excel-com-runner";

/// Private backend executed exclusively by the dedicated runner thread.
trait ExcelComBackend: 'static {
    fn create_application(&mut self) -> PlatformResult<()>;
    fn application_process_id(&mut self) -> PlatformResult<u32>;
    fn quit_application(&mut self) -> PlatformResult<()>;
}

/// One response value produced by the worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunnerValue {
    Unit,
    ProcessId(u32),
}

type RunnerResponse = PlatformResult<RunnerValue>;
type ResponseSender = SyncSender<RunnerResponse>;

enum RunnerCommand {
    CreateApplication(ResponseSender),
    ApplicationProcessId(ResponseSender),
    QuitApplication(ResponseSender),
    Shutdown(ResponseSender),
}

/// Cloneable request handle for one dedicated Excel COM worker.
struct ExcelComRunner {
    sender: SyncSender<RunnerCommand>,
    worker: Option<JoinHandle<PlatformResult<()>>>,
    shutdown_started: bool,
}

impl ExcelComRunner {
    /// Starts a fake or test backend on one dedicated worker thread.
    fn spawn<B, F>(backend_factory: F) -> PlatformResult<Self>
    where
        B: ExcelComBackend,
        F: FnOnce() -> B + Send + 'static,
    {
        let (sender, receiver) = mpsc::sync_channel(REQUEST_QUEUE_CAPACITY);
        let worker = thread::Builder::new()
            .name(WORKER_THREAD_NAME.to_owned())
            .spawn(move || {
                let backend = backend_factory();
                run_worker(backend, &receiver)
            })
            .map_err(|error| thread_spawn_error("Excel COM runner", &error))?;
        Ok(Self {
            sender,
            worker: Some(worker),
            shutdown_started: false,
        })
    }

    fn create_application(&self) -> PlatformResult<()> {
        expect_unit(self.dispatch(RunnerCommand::CreateApplication))
    }

    fn application_process_id(&self) -> PlatformResult<u32> {
        match self.dispatch(RunnerCommand::ApplicationProcessId)? {
            RunnerValue::ProcessId(process_id) => Ok(process_id),
            RunnerValue::Unit => Err(runner_protocol_error(
                "Excel COM runner returned no process id",
            )),
        }
    }

    fn quit_application(&self) -> PlatformResult<()> {
        expect_unit(self.dispatch(RunnerCommand::QuitApplication))
    }

    fn shutdown(&mut self) -> PlatformResult<()> {
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

fn run_worker<B>(mut backend: B, receiver: &Receiver<RunnerCommand>) -> PlatformResult<()>
where
    B: ExcelComBackend,
{
    let mut state = RunnerState::default();
    while let Ok(command) = receiver.recv() {
        match command {
            RunnerCommand::CreateApplication(response) => {
                let result = state.create_application(&mut backend);
                if send_response(&response, result.clone()).is_err() {
                    return state.abandon(&mut backend);
                }
            }
            RunnerCommand::ApplicationProcessId(response) => {
                let result = state.application_process_id(&mut backend);
                if send_response(&response, result.clone()).is_err() {
                    return state.abandon(&mut backend);
                }
            }
            RunnerCommand::QuitApplication(response) => {
                let result = state.quit_application(&mut backend);
                if send_response(&response, result.clone()).is_err() {
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

#[derive(Default)]
struct RunnerState {
    created: bool,
    closed: bool,
    process_id: Option<u32>,
}

impl RunnerState {
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

    fn shutdown<B>(&mut self, backend: &mut B) -> RunnerResponse
    where
        B: ExcelComBackend,
    {
        if self.created {
            self.quit_application(backend)?;
        } else {
            self.closed = true;
        }
        Ok(RunnerValue::Unit)
    }

    fn abandon<B>(&mut self, backend: &mut B) -> PlatformResult<()>
    where
        B: ExcelComBackend,
    {
        if !self.created {
            return Ok(());
        }
        match self.quit_application(backend) {
            Ok(_) => Ok(()),
            Err(error) => Err(PlatformError::new(
                ErrorCode::Fatal,
                format!("Excel COM request receiver disappeared and cleanup failed: {error}"),
            )),
        }
    }
}

fn expect_unit(response: RunnerResponse) -> PlatformResult<()> {
    match response? {
        RunnerValue::Unit => Ok(()),
        RunnerValue::ProcessId(_) => Err(runner_protocol_error(
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

fn join_worker(worker: JoinHandle<PlatformResult<()>>) -> PlatformResult<()> {
    worker.join().map_err(join_error)?
}

fn runner_protocol_error(message: impl Into<String>) -> PlatformError {
    PlatformError::new(ErrorCode::Fatal, message)
}

#[cfg(windows)]
fn spawn_windows_excel_runner() -> PlatformResult<ExcelComRunner> {
    let (sender, receiver) = mpsc::sync_channel(REQUEST_QUEUE_CAPACITY);
    let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
    let mut ready_sender = Some(ready_sender);
    let worker = thread::Builder::new()
        .name(WORKER_THREAD_NAME.to_owned())
        .spawn(move || {
            let initialization = crate::com::with_com_apartment(|_guard| {
                if let Some(sender) = ready_sender.take() {
                    sender.send(Ok(())).map_err(|_| {
                        runner_protocol_error("Excel COM runner readiness receiver disappeared")
                    })?;
                }
                run_worker(WindowsExcelComBackend::new(), &receiver)
            });
            match initialization {
                Ok(_apartment) => Ok(()),
                Err(error) => {
                    if let Some(sender) = ready_sender.take() {
                        // The receiver disappearing means startup already returned;
                        // the worker is exiting and no further notification is needed.
                        sender.send(Err(error.clone())).unwrap_or(());
                    }
                    Err(error)
                }
            }
        })
        .map_err(|error| thread_spawn_error("Excel COM runner", &error))?;

    let initialization = ready_receiver
        .recv_timeout(REQUEST_TIMEOUT)
        .map_err(|error| {
            PlatformError::new(
                ErrorCode::Fatal,
                format!(
                    "Excel COM runner initialization failed before readiness: {error}; worker was detached and requires process-level cleanup"
                ),
            )
        })?;
    if let Err(error) = initialization {
        let joined = worker.join().map_err(join_error);
        joined??;
        return Err(error);
    }
    Ok(ExcelComRunner {
        sender,
        worker: Some(worker),
        shutdown_started: false,
    })
}

#[cfg(windows)]
struct WindowsExcelComBackend {
    application: Option<windows::Win32::System::Com::IDispatch>,
}

#[cfg(windows)]
impl WindowsExcelComBackend {
    const fn new() -> Self {
        Self { application: None }
    }
}

#[cfg(windows)]
impl ExcelComBackend for WindowsExcelComBackend {
    fn create_application(&mut self) -> PlatformResult<()> {
        use windows::Win32::System::Com::{
            CLSCTX_LOCAL_SERVER, CLSIDFromProgID, CoCreateInstance, IDispatch,
        };
        use windows::core::w;

        if self.application.is_some() {
            return Err(runner_protocol_error(
                "Windows Excel backend already owns an application",
            ));
        }
        // SAFETY: the worker thread holds an STA, the ProgID is static and
        // NUL-terminated, and the returned interface is retained in this thread.
        let class_id = unsafe { CLSIDFromProgID(w!("Excel.Application")) }.map_err(|error| {
            crate::error::error_from_hresult(error.code().0, "CLSIDFromProgID(Excel.Application)")
        })?;
        // SAFETY: CLSID resolution succeeded, no aggregation is used, and the
        // returned IDispatch remains owned by the dedicated STA worker.
        let application: IDispatch =
            unsafe { CoCreateInstance(&raw const class_id, None, CLSCTX_LOCAL_SERVER) }.map_err(
                |error| {
                    crate::error::error_from_hresult(
                        error.code().0,
                        "CoCreateInstance(Excel.Application)",
                    )
                },
            )?;
        self.application = Some(application);
        Ok(())
    }

    fn application_process_id(&mut self) -> PlatformResult<u32> {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

        let application = self
            .application
            .as_ref()
            .ok_or_else(|| runner_protocol_error("Windows Excel backend has no application"))?;
        let hwnd_value = get_i4_property(application, "Hwnd")?;
        let pointer_value = isize::try_from(hwnd_value)
            .map_err(|_| runner_protocol_error("Excel Application.Hwnd did not fit in isize"))?;
        let hwnd = HWND(pointer_value as *mut core::ffi::c_void);
        let mut process_id = 0_u32;
        // SAFETY: the HWND came from the owned Excel application and is only
        // queried; the out-parameter points to a live stack value.
        unsafe { GetWindowThreadProcessId(hwnd, Some(&raw mut process_id)) };
        if process_id == 0 {
            return Err(runner_protocol_error(
                "Excel Application.Hwnd did not map to an owned process id",
            ));
        }
        Ok(process_id)
    }

    fn quit_application(&mut self) -> PlatformResult<()> {
        let application = self
            .application
            .as_ref()
            .ok_or_else(|| runner_protocol_error("Windows Excel backend has no application"))?;
        invoke_method(application, "Quit")?;
        self.application = None;
        Ok(())
    }
}

#[cfg(windows)]
fn get_i4_property(
    dispatch: &windows::Win32::System::Com::IDispatch,
    property_name: &str,
) -> PlatformResult<i32> {
    use windows::Win32::System::Com::DISPATCH_PROPERTYGET;
    use windows::Win32::System::Variant::{VARIANT, VT_I4, VariantClear};

    let dispatch_id = member_dispatch_id(dispatch, property_name)?;
    let parameters = windows::Win32::System::Com::DISPPARAMS::default();
    let mut result = VARIANT::default();
    // SAFETY: both the dispatch id and parameter block belong to this STA
    // worker; the result points to a live VARIANT cleared below.
    unsafe {
        dispatch.Invoke(
            dispatch_id,
            &windows::core::GUID::zeroed(),
            0,
            DISPATCH_PROPERTYGET,
            &raw const parameters,
            Some(&raw mut result),
            None,
            None,
        )
    }
    .map_err(|error| {
        crate::error::error_from_hresult(
            error.code().0,
            &format!("IDispatch.Invoke({property_name})"),
        )
    })?;

    // SAFETY: Invoke initialized the VARIANT; the union is read only after
    // checking the variant tag, and VariantClear runs exactly once.
    let value = unsafe {
        let inner = &result.Anonymous.Anonymous;
        if inner.vt != VT_I4 {
            let clear = VariantClear(&raw mut result);
            return match clear {
                Ok(()) => Err(runner_protocol_error(format!(
                    "Excel property {property_name} did not return VT_I4"
                ))),
                Err(error) => Err(crate::error::error_from_hresult(
                    error.code().0,
                    "VariantClear",
                )),
            };
        }
        inner.Anonymous.lVal
    };
    // SAFETY: paired with the successful Invoke above; no reference escapes.
    unsafe { VariantClear(&raw mut result) }
        .map_err(|error| crate::error::error_from_hresult(error.code().0, "VariantClear"))?;
    Ok(value)
}

#[cfg(windows)]
fn invoke_method(
    dispatch: &windows::Win32::System::Com::IDispatch,
    method_name: &str,
) -> PlatformResult<()> {
    use windows::Win32::System::Com::{DISPATCH_METHOD, DISPPARAMS};

    let dispatch_id = member_dispatch_id(dispatch, method_name)?;
    let parameters = DISPPARAMS::default();
    // SAFETY: the call has no arguments and no result; both pointers point to
    // live stack values on the owning STA worker thread.
    unsafe {
        dispatch.Invoke(
            dispatch_id,
            &windows::core::GUID::zeroed(),
            0,
            DISPATCH_METHOD,
            &raw const parameters,
            None,
            None,
            None,
        )
    }
    .map_err(|error| {
        crate::error::error_from_hresult(
            error.code().0,
            &format!("IDispatch.Invoke({method_name})"),
        )
    })
}

#[cfg(windows)]
fn member_dispatch_id(
    dispatch: &windows::Win32::System::Com::IDispatch,
    member_name: &str,
) -> PlatformResult<i32> {
    use windows::core::PCWSTR;

    let mut wide: Vec<u16> = member_name.encode_utf16().collect();
    wide.push(0);
    let name = PCWSTR(wide.as_ptr());
    let mut dispatch_id = 0_i32;
    // SAFETY: `name` points to NUL-terminated UTF-16 storage alive for the
    // synchronous call, and `dispatch_id` is a live out-parameter.
    unsafe {
        dispatch.GetIDsOfNames(
            &windows::core::GUID::zeroed(),
            &raw const name,
            1,
            0,
            &raw mut dispatch_id,
        )
    }
    .map_err(|error| {
        crate::error::error_from_hresult(error.code().0, &format!("GetIDsOfNames({member_name})"))
    })?;
    Ok(dispatch_id)
}

#[cfg(test)]
#[path = "excel_com_runner_tests.rs"]
mod tests;
