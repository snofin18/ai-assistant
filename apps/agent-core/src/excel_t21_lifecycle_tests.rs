//! Task-end lifecycle extension and tests for the private Excel T2.1 handlers.
//!
//! The tests prove that closing starts before blocking runtime cleanup, failed
//! cleanup retains evidence, and successful cleanup releases every session.

use std::error::Error;
use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use super::super::handlers::{LIFECYCLE_CLOSING, LIFECYCLE_RUNNING, LIFECYCLE_TERMINAL};
use super::*;

type TestResult = Result<(), Box<dyn Error>>;

struct ShutdownGate {
    entered: Mutex<bool>,
    release: Mutex<bool>,
    entered_changed: Condvar,
    release_changed: Condvar,
}

impl ShutdownGate {
    fn new() -> Self {
        Self {
            entered: Mutex::new(false),
            release: Mutex::new(false),
            entered_changed: Condvar::new(),
            release_changed: Condvar::new(),
        }
    }

    fn wait_until_entered(&self) -> io::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut entered = self
            .entered
            .lock()
            .map_err(|_| io::Error::other("shutdown gate entered mutex is poisoned"))?;
        while !*entered {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::other(
                    "Excel task-end shutdown did not reach the blocking runtime",
                ));
            }
            let (next, _timeout) = self
                .entered_changed
                .wait_timeout(entered, remaining)
                .map_err(|_| io::Error::other("shutdown gate wait is poisoned"))?;
            entered = next;
        }
        drop(entered);
        Ok(())
    }

    fn mark_entered(&self) -> io::Result<()> {
        *self
            .entered
            .lock()
            .map_err(|_| io::Error::other("shutdown gate entered mutex is poisoned"))? = true;
        self.entered_changed.notify_all();
        Ok(())
    }

    fn release(&self) -> io::Result<()> {
        *self
            .release
            .lock()
            .map_err(|_| io::Error::other("shutdown gate release mutex is poisoned"))? = true;
        self.release_changed.notify_all();
        Ok(())
    }

    fn wait_for_release(&self) -> io::Result<()> {
        let mut released = self
            .release
            .lock()
            .map_err(|_| io::Error::other("shutdown gate release mutex is poisoned"))?;
        while !*released {
            released = self
                .release_changed
                .wait(released)
                .map_err(|_| io::Error::other("shutdown gate release wait is poisoned"))?;
        }
        drop(released);
        Ok(())
    }
}

struct BlockingShutdownRuntime {
    inner: FakeRuntime,
    gate: Arc<ShutdownGate>,
}

impl BlockingShutdownRuntime {
    fn new(gate: Arc<ShutdownGate>) -> Self {
        Self {
            inner: FakeRuntime::default(),
            gate,
        }
    }
}

impl ExcelT21Runtime for BlockingShutdownRuntime {
    fn open_read_only(&self, path: &std::path::Path) -> Result<OpenedWorkbook, ExcelT21Error> {
        self.inner.open_read_only(path)
    }

    fn close(&self, handle: WorkbookHandle) -> Result<(), ExcelT21Error> {
        self.inner.close(handle)
    }

    fn verify_ownership(
        &self,
        handle: WorkbookHandle,
        workbook: &WorkbookIdentity,
    ) -> Result<(), ExcelT21Error> {
        self.inner.verify_ownership(handle, workbook)
    }

    fn fingerprint(&self, handle: WorkbookHandle) -> Result<String, ExcelT21Error> {
        self.inner.fingerprint(handle)
    }

    fn read_workbook(&self, handle: WorkbookHandle) -> Result<WorkbookSnapshot, ExcelT21Error> {
        self.inner.read_workbook(handle)
    }

    fn read_range(
        &self,
        handle: WorkbookHandle,
        selector: &RangeSelector,
        mode: ReadMode,
    ) -> Result<RangeSnapshot, ExcelT21Error> {
        self.inner.read_range(handle, selector, mode)
    }

    fn shutdown(&self) -> Result<(), ExcelT21Error> {
        self.gate
            .mark_entered()
            .map_err(|error| ExcelT21Error::new(ErrorCode::Fatal, error.to_string()))?;
        self.gate
            .wait_for_release()
            .map_err(|error| ExcelT21Error::new(ErrorCode::Fatal, error.to_string()))?;
        finish_fake_runtime(&self.inner)
    }
}

struct BlockingOpenRuntime {
    inner: FakeRuntime,
    gate: Arc<ShutdownGate>,
}

impl BlockingOpenRuntime {
    fn new(gate: Arc<ShutdownGate>) -> Self {
        Self {
            inner: FakeRuntime::default(),
            gate,
        }
    }
}

impl ExcelT21Runtime for BlockingOpenRuntime {
    fn open_read_only(&self, path: &std::path::Path) -> Result<OpenedWorkbook, ExcelT21Error> {
        self.gate
            .mark_entered()
            .map_err(|error| ExcelT21Error::new(ErrorCode::Fatal, error.to_string()))?;
        self.gate
            .wait_for_release()
            .map_err(|error| ExcelT21Error::new(ErrorCode::Fatal, error.to_string()))?;
        self.inner.open_read_only(path)
    }

    fn close(&self, handle: WorkbookHandle) -> Result<(), ExcelT21Error> {
        self.inner.close(handle)
    }

    fn verify_ownership(
        &self,
        handle: WorkbookHandle,
        workbook: &WorkbookIdentity,
    ) -> Result<(), ExcelT21Error> {
        self.inner.verify_ownership(handle, workbook)
    }

    fn fingerprint(&self, handle: WorkbookHandle) -> Result<String, ExcelT21Error> {
        self.inner.fingerprint(handle)
    }

    fn read_workbook(&self, handle: WorkbookHandle) -> Result<WorkbookSnapshot, ExcelT21Error> {
        self.inner.read_workbook(handle)
    }

    fn read_range(
        &self,
        handle: WorkbookHandle,
        selector: &RangeSelector,
        mode: ReadMode,
    ) -> Result<RangeSnapshot, ExcelT21Error> {
        self.inner.read_range(handle, selector, mode)
    }

    fn shutdown(&self) -> Result<(), ExcelT21Error> {
        finish_fake_runtime(&self.inner)
    }
}

struct ControlledRuntime {
    inner: FakeRuntime,
    close_error: Mutex<Option<(u64, ErrorCode)>>,
    shutdown_error: Mutex<Option<ExcelT21Error>>,
    shutdown_count: AtomicUsize,
}

impl Default for ControlledRuntime {
    fn default() -> Self {
        Self {
            inner: FakeRuntime::default(),
            close_error: Mutex::new(None),
            shutdown_error: Mutex::new(None),
            shutdown_count: AtomicUsize::new(0),
        }
    }
}

impl ControlledRuntime {
    fn set_close_error(&self, error: Option<(u64, ErrorCode)>) -> Result<(), ExcelT21Error> {
        *self.close_error.lock().map_err(|_| {
            ExcelT21Error::new(ErrorCode::Fatal, "controlled close error mutex is poisoned")
        })? = error;
        Ok(())
    }

    fn set_shutdown_error(&self, error: Option<ExcelT21Error>) -> Result<(), ExcelT21Error> {
        *self.shutdown_error.lock().map_err(|_| {
            ExcelT21Error::new(
                ErrorCode::Fatal,
                "controlled shutdown error mutex is poisoned",
            )
        })? = error;
        Ok(())
    }

    fn shutdown_count(&self) -> usize {
        self.shutdown_count.load(Ordering::Acquire)
    }

    fn snapshot(&self) -> Result<(Vec<String>, Vec<u64>, usize), ExcelT21Error> {
        self.inner.snapshot()
    }
}

impl ExcelT21Runtime for ControlledRuntime {
    fn open_read_only(&self, path: &std::path::Path) -> Result<OpenedWorkbook, ExcelT21Error> {
        self.inner.open_read_only(path)
    }

    fn close(&self, handle: WorkbookHandle) -> Result<(), ExcelT21Error> {
        let mut close_error = self.close_error.lock().map_err(|_| {
            ExcelT21Error::new(ErrorCode::Fatal, "controlled close error mutex is poisoned")
        })?;
        if let Some((error_handle, code)) = *close_error
            && error_handle == handle.get()
        {
            *close_error = None;
            return Err(ExcelT21Error::new(code, "scripted close failure"));
        }
        drop(close_error);
        self.inner.close(handle)
    }

    fn verify_ownership(
        &self,
        handle: WorkbookHandle,
        workbook: &WorkbookIdentity,
    ) -> Result<(), ExcelT21Error> {
        self.inner.verify_ownership(handle, workbook)
    }

    fn fingerprint(&self, handle: WorkbookHandle) -> Result<String, ExcelT21Error> {
        self.inner.fingerprint(handle)
    }

    fn read_workbook(&self, handle: WorkbookHandle) -> Result<WorkbookSnapshot, ExcelT21Error> {
        self.inner.read_workbook(handle)
    }

    fn read_range(
        &self,
        handle: WorkbookHandle,
        selector: &RangeSelector,
        mode: ReadMode,
    ) -> Result<RangeSnapshot, ExcelT21Error> {
        self.inner.read_range(handle, selector, mode)
    }

    fn shutdown(&self) -> Result<(), ExcelT21Error> {
        self.shutdown_count.fetch_add(1, Ordering::AcqRel);
        let shutdown_error = self
            .shutdown_error
            .lock()
            .map_err(|_| {
                ExcelT21Error::new(
                    ErrorCode::Fatal,
                    "controlled shutdown error mutex is poisoned",
                )
            })?
            .clone();
        if let Some(error) = shutdown_error {
            return Err(error);
        }
        finish_fake_runtime(&self.inner)
    }
}

fn finish_fake_runtime(runtime: &FakeRuntime) -> Result<(), ExcelT21Error> {
    let mut state = runtime.lock_state()?;
    let workbooks = std::mem::take(&mut state.workbooks);
    state.closed_handles.extend(workbooks.into_keys());
    drop(state);
    Ok(())
}

fn open_path<R>(
    handlers: &ExcelT21Handlers<R>,
    call: &ExcelCallContext,
) -> Result<String, ExcelT21Error>
where
    R: ExcelT21Runtime + 'static,
{
    let opened = handlers.open(
        call,
        &arguments(json!({
            "workbook_path":TEST_WORKBOOK_PATH,
            "read_only":true
        }))?,
    )?;
    Ok(token_from(&opened)?.to_owned())
}

impl<R> ExcelT21Handlers<R>
where
    R: ExcelT21Runtime + 'static,
{
    /// Invalidates the task token and closes its runtime handle.
    pub(crate) fn cleanup_task(&self, task_id: &str) -> Result<(), ExcelT21Error> {
        self.ensure_lifecycle_running("excel.t21.cleanup")?;
        self.cleanup_expired_sessions()?;
        let closing = self
            .sessions
            .lock()
            .map_err(|_| {
                ExcelT21Error::new(ErrorCode::Fatal, "Excel session registry is poisoned")
            })?
            .begin_close_task(task_id);
        if let Some(closing) = closing {
            self.runtime.close(closing.handle)?;
            self.sessions
                .lock()
                .map_err(|_| {
                    ExcelT21Error::new(ErrorCode::Fatal, "Excel session registry is poisoned")
                })?
                .finish_close(closing.handle)?;
        }
        Ok(())
    }

    pub(crate) fn fail_open_after_cleanup(
        &self,
        handle: WorkbookHandle,
        error: ExcelT21Error,
    ) -> Result<ExcelToolResult, ExcelT21Error> {
        self.runtime.close(handle).map_err(|close_error| {
            ExcelT21Error::new(
                ErrorCode::Fatal,
                format!(
                    "excel.workbook.open: open validation failed and cleanup failed: {close_error}"
                ),
            )
        })?;
        Err(error)
    }

    pub(crate) fn cleanup_expired_sessions(&self) -> Result<(), ExcelT21Error> {
        self.ensure_lifecycle_running("excel.t21.cleanup")?;
        let expired = self
            .sessions
            .lock()
            .map_err(|_| {
                ExcelT21Error::new(ErrorCode::Fatal, "Excel session registry is poisoned")
            })?
            .expire_sessions();
        for session in expired {
            self.runtime.close(session.handle).map_err(|error| {
                cleanup_mapping_error("expired Excel session cleanup failed", error)
            })?;
            self.sessions
                .lock()
                .map_err(|_| {
                    ExcelT21Error::new(ErrorCode::Fatal, "Excel session registry is poisoned")
                })?
                .finish_close(session.handle)?;
        }
        Ok(())
    }

    fn ensure_lifecycle_running(&self, tool: &str) -> Result<(), ExcelT21Error> {
        match self.lifecycle.load(Ordering::Acquire) {
            LIFECYCLE_RUNNING => Ok(()),
            LIFECYCLE_CLOSING => Err(ExcelT21Error::new(
                ErrorCode::Fatal,
                format!("{tool}: Excel task-end cleanup is in progress"),
            )),
            _ => Err(ExcelT21Error::new(
                ErrorCode::Fatal,
                format!("{tool}: Excel task-end cleanup is terminal"),
            )),
        }
    }

    /// Invalidates all sessions and shuts down the private runner exactly once.
    pub(crate) fn shutdown(&self) -> Result<(), ExcelT21Error> {
        if self
            .lifecycle
            .compare_exchange(
                LIFECYCLE_RUNNING,
                LIFECYCLE_CLOSING,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_err()
        {
            return Err(ExcelT21Error::new(
                ErrorCode::Fatal,
                "Excel task-end cleanup is already in progress or terminal",
            ));
        }
        let mut cleanup_errors = Vec::new();
        let closing = self.sessions.lock().map_or_else(
            |_| {
                cleanup_errors.push(ExcelT21Error::new(
                    ErrorCode::Fatal,
                    "Excel session registry is poisoned",
                ));
                Vec::new()
            },
            |mut sessions| sessions.begin_close_all(),
        );
        for session in closing {
            match self.runtime.close(session.handle) {
                Ok(()) => {
                    if let Err(error) = self
                        .sessions
                        .lock()
                        .map_err(|_| {
                            ExcelT21Error::new(
                                ErrorCode::Fatal,
                                "Excel session registry is poisoned",
                            )
                        })
                        .and_then(|mut sessions| sessions.finish_close(session.handle))
                    {
                        cleanup_errors.push(error);
                    }
                }
                Err(error) => cleanup_errors.push(cleanup_mapping_error(
                    "task-end Excel workbook close failed",
                    error,
                )),
            }
        }
        let runner_shutdown = self
            .runtime
            .shutdown()
            .map_err(|error| cleanup_mapping_error("task-end Excel runner shutdown failed", error));
        if runner_shutdown.is_ok() {
            if let Err(error) = self
                .sessions
                .lock()
                .map(|mut sessions| {
                    sessions.finish_all_closing();
                })
                .map_err(|_| {
                    ExcelT21Error::new(ErrorCode::Fatal, "Excel session registry is poisoned")
                })
            {
                cleanup_errors.push(error);
            }
        } else if let Err(error) = runner_shutdown {
            cleanup_errors.push(error);
        }
        self.lifecycle.store(LIFECYCLE_TERMINAL, Ordering::Release);
        combine_cleanup_errors(cleanup_errors)
    }
}

fn cleanup_mapping_error(context: &str, error: ExcelT21Error) -> ExcelT21Error {
    if error.code() == ErrorCode::UserInteraction {
        error
    } else {
        ExcelT21Error::new(ErrorCode::Fatal, format!("{context}: {error}"))
    }
}

fn combine_cleanup_errors(errors: Vec<ExcelT21Error>) -> Result<(), ExcelT21Error> {
    if errors.is_empty() {
        return Ok(());
    }
    let code = if errors
        .iter()
        .all(|error| error.code() == ErrorCode::UserInteraction)
    {
        ErrorCode::UserInteraction
    } else {
        ErrorCode::Fatal
    };
    Err(ExcelT21Error::new(
        code,
        errors
            .into_iter()
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
            .join("; "),
    ))
}

#[test]
fn test_excel_t21_lifecycle_shutdown_invalidates_token_before_blocking_cleanup() -> TestResult {
    let gate = Arc::new(ShutdownGate::new());
    let runtime = Arc::new(BlockingShutdownRuntime::new(Arc::clone(&gate)));
    let (registry, _clock, _tokens) = test_registry("bus-lifecycle")?;
    let handlers = Arc::new(ExcelT21Handlers::new(Arc::clone(&runtime), registry));
    let call = test_context("bus-lifecycle", "task-lifecycle")?;
    let token = open_path(&handlers, &call)?;

    let shutdown_handlers = Arc::clone(&handlers);
    let shutdown_thread = thread::spawn(move || shutdown_handlers.shutdown());
    gate.wait_until_entered()?;

    let started = Instant::now();
    let blocked_read = handlers
        .workbook_read(&call, &arguments(json!({"session_id":token}))?)
        .err()
        .ok_or_else(|| invalid_arguments("read during cleanup unexpectedly passed"))?;
    assert_eq!(blocked_read.code(), ErrorCode::Fatal);
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "new work waited for blocking cleanup instead of failing in closing state"
    );

    gate.release()?;
    shutdown_thread
        .join()
        .map_err(|_| io::Error::other("shutdown lifecycle test thread panicked"))??;
    assert_eq!(runtime.inner.snapshot()?.1.len(), 1);
    Ok(())
}

#[test]
fn test_excel_t21_lifecycle_open_racing_shutdown_cannot_escape_closing() -> TestResult {
    let gate = Arc::new(ShutdownGate::new());
    let runtime = Arc::new(BlockingOpenRuntime::new(Arc::clone(&gate)));
    let (registry, _clock, _tokens) = test_registry("bus-lifecycle")?;
    let handlers = Arc::new(ExcelT21Handlers::new(Arc::clone(&runtime), registry));
    let call = test_context("bus-lifecycle", "task-open-race")?;
    let open_handlers = Arc::clone(&handlers);
    let open_call = call;
    let open_thread = thread::spawn(move || open_path(open_handlers.as_ref(), &open_call));
    gate.wait_until_entered()?;

    handlers.shutdown()?;
    gate.release()?;
    let open_result = open_thread
        .join()
        .map_err(|_| io::Error::other("open lifecycle test thread panicked"))?;
    let failure = open_result
        .err()
        .ok_or_else(|| invalid_arguments("open escaped terminal cleanup"))?;
    assert_eq!(failure.code(), ErrorCode::Fatal);
    let sessions = handlers
        .sessions
        .lock()
        .map_err(|_| io::Error::other("session registry is poisoned"))?;
    assert_eq!(sessions.active_session_count(), 0);
    assert_eq!(sessions.closing.len(), 0);
    drop(sessions);
    assert_eq!(runtime.inner.snapshot()?.2, 0);
    Ok(())
}

#[test]
fn test_excel_t21_lifecycle_shutdown_releases_all_sessions_after_runtime_convergence() -> TestResult
{
    let runtime = Arc::new(ControlledRuntime::default());
    let (registry, _clock, _tokens) = test_registry("bus-lifecycle")?;
    let handlers = ExcelT21Handlers::new(Arc::clone(&runtime), registry);
    let first = test_context("bus-lifecycle", "task-one")?;
    let second = test_context("bus-lifecycle", "task-two")?;
    let _first_token = open_path(&handlers, &first)?;
    let second_token = open_path(&handlers, &second)?;

    handlers.shutdown()?;
    let sessions = handlers
        .sessions
        .lock()
        .map_err(|_| io::Error::other("session registry is poisoned"))?;
    assert_eq!(sessions.active_session_count(), 0);
    assert_eq!(sessions.closing.len(), 0);
    drop(sessions);

    let (opened_paths, closed_handles, open_workbooks) = runtime.snapshot()?;
    assert_eq!(opened_paths.len(), 2);
    assert_eq!(closed_handles.len(), 2);
    assert_eq!(open_workbooks, 0);
    let repeated = handlers
        .shutdown()
        .err()
        .ok_or_else(|| invalid_arguments("repeated shutdown unexpectedly passed"))?;
    assert_eq!(repeated.code(), ErrorCode::Fatal);

    let stale = handlers
        .workbook_read(&second, &arguments(json!({"session_id":second_token}))?)
        .err()
        .ok_or_else(|| invalid_arguments("terminal token unexpectedly passed"))?;
    assert_eq!(stale.code(), ErrorCode::Fatal);
    Ok(())
}

#[test]
fn test_excel_t21_lifecycle_shutdown_preserves_user_interaction_and_releases_after_convergence()
-> TestResult {
    let runtime = Arc::new(ControlledRuntime::default());
    let (registry, _clock, _tokens) = test_registry("bus-lifecycle")?;
    let handlers = ExcelT21Handlers::new(Arc::clone(&runtime), registry);
    let call = test_context("bus-lifecycle", "task-user-interaction")?;
    let _token = open_path(&handlers, &call)?;
    runtime.set_close_error(Some((1, ErrorCode::UserInteraction)))?;

    let failure = handlers
        .shutdown()
        .err()
        .ok_or_else(|| invalid_arguments("user-interaction cleanup unexpectedly passed"))?;
    assert_eq!(failure.code(), ErrorCode::UserInteraction);
    assert_eq!(runtime.shutdown_count(), 1);
    let sessions = handlers
        .sessions
        .lock()
        .map_err(|_| io::Error::other("session registry is poisoned"))?;
    assert_eq!(sessions.active_session_count(), 0);
    assert_eq!(sessions.closing.len(), 0);
    Ok(())
}

#[test]
fn test_excel_t21_lifecycle_shutdown_failure_retains_closing_session_evidence() -> TestResult {
    let runtime = Arc::new(ControlledRuntime::default());
    let (registry, _clock, _tokens) = test_registry("bus-lifecycle")?;
    let handlers = ExcelT21Handlers::new(Arc::clone(&runtime), registry);
    let call = test_context("bus-lifecycle", "task-failure")?;
    let _token = open_path(&handlers, &call)?;
    runtime.set_close_error(Some((1, ErrorCode::Fatal)))?;
    runtime.set_shutdown_error(Some(ExcelT21Error::new(
        ErrorCode::Fatal,
        "scripted runner shutdown failure",
    )))?;

    let failure = handlers
        .shutdown()
        .err()
        .ok_or_else(|| invalid_arguments("failed cleanup unexpectedly passed"))?;
    assert_eq!(failure.code(), ErrorCode::Fatal);
    let sessions = handlers
        .sessions
        .lock()
        .map_err(|_| io::Error::other("session registry is poisoned"))?;
    assert_eq!(sessions.active_session_count(), 0);
    assert_eq!(sessions.closing.len(), 1);
    drop(sessions);
    assert_eq!(runtime.snapshot()?.2, 1);
    Ok(())
}

#[test]
fn test_excel_t21_lifecycle_expired_cleanup_preserves_user_interaction() -> TestResult {
    let runtime = Arc::new(ControlledRuntime::default());
    let (registry, clock, _tokens) = test_registry("bus-lifecycle")?;
    let handlers = ExcelT21Handlers::new(Arc::clone(&runtime), registry);
    let call = test_context("bus-lifecycle", "task-expired")?;
    let _token = open_path(&handlers, &call)?;
    clock.advance(SESSION_TTL_MS);
    runtime.set_close_error(Some((1, ErrorCode::UserInteraction)))?;

    let failure = handlers
        .cleanup_task("task-expired")
        .err()
        .ok_or_else(|| invalid_arguments("user-interaction expiry cleanup unexpectedly passed"))?;
    assert_eq!(failure.code(), ErrorCode::UserInteraction);
    let sessions = handlers
        .sessions
        .lock()
        .map_err(|_| io::Error::other("session registry is poisoned"))?;
    assert_eq!(sessions.active_session_count(), 0);
    assert_eq!(sessions.closing.len(), 1);
    Ok(())
}

#[test]
fn test_workbook_path_parser_rejects_non_local_or_non_xlsx_shapes() -> Result<(), ExcelT21Error> {
    assert_eq!(
        parse_workbook_path(concat!(env!("CARGO_MANIFEST_DIR"), "/book.xlsx"))?,
        std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/book.xlsx"))
    );
    for invalid in [
        "book.xlsx",
        "\\\\server\\share\\book.xlsx",
        "https://example.com/book.xlsx",
        concat!(env!("CARGO_MANIFEST_DIR"), "/book.xlsm"),
        concat!(env!("CARGO_MANIFEST_DIR"), "/../book.xlsx"),
        concat!(env!("CARGO_MANIFEST_DIR"), "/book.xlsx:stream"),
    ] {
        assert_eq!(
            parse_workbook_path(invalid)
                .err()
                .ok_or_else(|| invalid_arguments("invalid path unexpectedly parsed"))?
                .code(),
            ErrorCode::ToolInvalidArgs
        );
    }
    Ok(())
}

#[test]
fn test_registry_rejects_invalid_opaque_token_length() -> Result<(), ExcelT21Error> {
    let clock = Arc::new(FakeClock::default());
    let mut registry =
        SessionRegistry::new("bus-a", clock, Arc::new(FixedTokenSource("short-token")))?;
    assert_eq!(
        registry
            .register("task-a", opened(1))
            .err()
            .ok_or_else(|| invalid_arguments("short token unexpectedly passed"))?
            .code(),
        ErrorCode::ToolInvalidArgs
    );
    Ok(())
}
