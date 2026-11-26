//! Platform-private Excel process watchdog.
//!
//! Responsibilities:
//! - keep an owned Excel identity on a thread that never enters the COM STA;
//! - disarm only after the matching process has exited;
//! - force verified process-tree termination when graceful cleanup stalls.
//!
//! Boundaries:
//! - this module exposes no public API, Tool, or serialized handle;
//! - it does not cover a Host process crash after this thread disappears;
//! - it never accepts a PID without complete creation/path identity.
//!
//! Invariants:
//! 1. one watchdog can hold at most one complete owned identity;
//! 2. force termination is allowed only for the currently armed identity;
//! 3. every response and thread reap has an independent deadline.
//!
//! Related documents: ADR-0108 D8/D9, TASK-410, TASK-411, TASK-412.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use assistant_platform_api::{ErrorCode, PlatformError, PlatformResult};

use super::ExcelOwnedProcessIdentity;

pub(super) const WATCHDOG_GRACEFUL_WAIT: Duration = Duration::from_secs(5);
pub(super) const WATCHDOG_TERMINATION_WAIT: Duration = Duration::from_secs(20);
pub(super) const WATCHDOG_RESPONSE_WAIT: Duration = Duration::from_secs(30);
pub(super) const WATCHDOG_THREAD_REAP_WAIT: Duration = Duration::from_secs(2);
const WATCHDOG_THREAD_NAME: &str = "excel-process-watchdog";
const WATCHDOG_POLL_INTERVAL: Duration = Duration::from_millis(20);
const WATCHDOG_TREE_REFRESH_INTERVAL: Duration = Duration::from_millis(50);

/// Bounded evidence returned after a graceful or forced process-tree cleanup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ExcelProcessTerminationReport {
    root_was_alive: bool,
    termination_attempted: bool,
    terminated_process_count: usize,
    converged: bool,
}

impl ExcelProcessTerminationReport {
    pub(super) const fn new(
        root_was_alive: bool,
        termination_attempted: bool,
        terminated_process_count: usize,
        converged: bool,
    ) -> Self {
        Self {
            root_was_alive,
            termination_attempted,
            terminated_process_count,
            converged,
        }
    }

    #[cfg(all(test, windows))]
    pub(super) const fn root_was_alive(self) -> bool {
        self.root_was_alive
    }

    #[cfg(test)]
    pub(super) const fn termination_attempted(self) -> bool {
        self.termination_attempted
    }

    #[cfg(test)]
    pub(super) const fn terminated_process_count(self) -> usize {
        self.terminated_process_count
    }

    pub(super) const fn converged(self) -> bool {
        self.converged
    }
}

/// Private process operations used by the watchdog thread.
pub(super) trait ExcelProcessWatchdogBackend: Send + 'static {
    /// Captures the current verified tree before a potentially blocking callback.
    fn capture_verified_tree(
        &mut self,
        identity: &ExcelOwnedProcessIdentity,
        timeout: Duration,
    ) -> PlatformResult<()>;

    /// Returns true when the original identity has exited or the PID now names
    /// a different process and every captured descendant is also gone.
    fn wait_for_identity_exit(
        &mut self,
        identity: &ExcelOwnedProcessIdentity,
        timeout: Duration,
    ) -> PlatformResult<bool>;

    /// Verifies the armed identity and terminates only its still-matching tree.
    fn terminate_verified_tree(
        &mut self,
        identity: &ExcelOwnedProcessIdentity,
        timeout: Duration,
    ) -> PlatformResult<ExcelProcessTerminationReport>;
}

/// Cloneable command handle used by the Excel runner state and public runner.
#[derive(Clone)]
pub(super) struct ExcelProcessWatchdogController {
    sender: SyncSender<WatchdogCommand>,
}

impl ExcelProcessWatchdogController {
    /// Arms the watchdog with the complete owned identity.
    pub(super) fn arm(
        &self,
        identity: &ExcelOwnedProcessIdentity,
        response_timeout: Duration,
    ) -> PlatformResult<()> {
        let (sender, receiver) = mpsc::sync_channel(1);
        self.sender
            .try_send(WatchdogCommand::Arm {
                identity: identity.clone(),
                response: sender,
            })
            .map_err(|error| watchdog_queue_error(&error))?;
        receive_watchdog_response(&receiver, response_timeout)
    }

    /// Captures the current owned tree before a potentially blocking COM call.
    pub(super) fn capture_verified_tree(
        &self,
        identity: &ExcelOwnedProcessIdentity,
        response_timeout: Duration,
    ) -> PlatformResult<()> {
        let (sender, receiver) = mpsc::sync_channel(1);
        self.sender
            .try_send(WatchdogCommand::CaptureTree {
                identity: identity.clone(),
                response: sender,
            })
            .map_err(|error| watchdog_queue_error(&error))?;
        receive_watchdog_response(&receiver, response_timeout)
    }

    /// Waits briefly for graceful exit, then force-terminates the matching tree.
    pub(super) fn converge_and_disarm(
        &self,
        identity: &ExcelOwnedProcessIdentity,
        graceful_timeout: Duration,
        termination_timeout: Duration,
        response_timeout: Duration,
    ) -> PlatformResult<ExcelProcessTerminationReport> {
        let (sender, receiver) = mpsc::sync_channel(1);
        self.sender
            .try_send(WatchdogCommand::ConvergeAndDisarm {
                identity: identity.clone(),
                graceful_timeout,
                termination_timeout,
                response: sender,
            })
            .map_err(|error| watchdog_queue_error(&error))?;
        receive_watchdog_response(&receiver, response_timeout)
    }

    /// Force-terminates the currently armed identity after a cleanup failure.
    #[cfg(test)]
    pub(super) fn force_terminate(
        &self,
        identity: &ExcelOwnedProcessIdentity,
        termination_timeout: Duration,
        response_timeout: Duration,
    ) -> PlatformResult<ExcelProcessTerminationReport> {
        let (sender, receiver) = mpsc::sync_channel(1);
        self.sender
            .try_send(WatchdogCommand::ForceTerminate {
                identity: identity.clone(),
                termination_timeout,
                response: sender,
            })
            .map_err(|error| watchdog_queue_error(&error))?;
        receive_watchdog_response(&receiver, response_timeout)
    }

    /// Force-terminates whichever complete identity is currently armed.
    pub(super) fn force_terminate_armed(
        &self,
        termination_timeout: Duration,
        response_timeout: Duration,
    ) -> PlatformResult<ExcelProcessTerminationReport> {
        let (sender, receiver) = mpsc::sync_channel(1);
        self.sender
            .try_send(WatchdogCommand::ForceTerminateArmed {
                termination_timeout,
                response: sender,
            })
            .map_err(|error| watchdog_queue_error(&error))?;
        receive_watchdog_response(&receiver, response_timeout)
    }

    fn shutdown(&self, response_timeout: Duration) -> PlatformResult<()> {
        let (sender, receiver) = mpsc::sync_channel(1);
        self.sender
            .try_send(WatchdogCommand::Shutdown { response: sender })
            .map_err(|error| watchdog_queue_error(&error))?;
        receive_watchdog_response(&receiver, response_timeout)
    }
}

/// Owns the watchdog thread and its bounded controller.
pub(super) struct ExcelProcessWatchdog {
    controller: ExcelProcessWatchdogController,
    worker: Option<JoinHandle<PlatformResult<()>>>,
}

impl ExcelProcessWatchdog {
    /// Starts the production Windows process-tree backend.
    #[cfg(windows)]
    pub(super) fn start_windows() -> PlatformResult<(Self, ExcelProcessWatchdogController)> {
        let backend = super::process_watchdog_windows::WindowsProcessTreeBackend::new();
        Self::start(Box::new(backend))
    }

    /// Starts a no-op backend for fake runner tests that never own Excel.
    #[cfg(test)]
    pub(super) fn start_for_tests() -> PlatformResult<(Self, ExcelProcessWatchdogController)> {
        Self::start(Box::new(tests::NoopWatchdogBackend))
    }

    #[cfg(test)]
    pub(super) fn start_with_backend(
        backend: Box<dyn ExcelProcessWatchdogBackend>,
    ) -> PlatformResult<(Self, ExcelProcessWatchdogController)> {
        Self::start(backend)
    }

    fn start(
        backend: Box<dyn ExcelProcessWatchdogBackend>,
    ) -> PlatformResult<(Self, ExcelProcessWatchdogController)> {
        let (sender, receiver) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name(WATCHDOG_THREAD_NAME.to_owned())
            .spawn(move || run_watchdog(backend, &receiver))
            .map_err(|error| {
                PlatformError::new(
                    ErrorCode::Fatal,
                    format!("Excel process watchdog thread spawn failed: {error}"),
                )
            })?;
        let controller = ExcelProcessWatchdogController { sender };
        Ok((
            Self {
                controller: controller.clone(),
                worker: Some(worker),
            },
            controller,
        ))
    }

    /// Returns a cloneable command handle for the Excel worker state.
    pub(super) fn controller(&self) -> ExcelProcessWatchdogController {
        self.controller.clone()
    }

    /// Stops an idle watchdog and reaps its thread within a deadline.
    pub(super) fn shutdown(&mut self, response_timeout: Duration) -> PlatformResult<()> {
        let response = self.controller.shutdown(response_timeout);
        let reap = self.reap_thread(WATCHDOG_THREAD_REAP_WAIT);
        match (response, reap) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
            (Err(error), Err(reap_error)) => Err(PlatformError::new(
                ErrorCode::Fatal,
                format!("{error}; watchdog thread reap also failed: {reap_error}"),
            )),
        }
    }

    fn reap_thread(&mut self, timeout: Duration) -> PlatformResult<()> {
        let Some(worker) = self.worker.take() else {
            return Ok(());
        };
        let deadline = Instant::now() + timeout;
        while !worker.is_finished() {
            if Instant::now() >= deadline {
                return Err(PlatformError::new(
                    ErrorCode::Fatal,
                    "Excel process watchdog thread did not exit within its reap deadline",
                ));
            }
            thread::sleep(WATCHDOG_POLL_INTERVAL);
        }
        worker.join().map_err(|_| {
            PlatformError::new(ErrorCode::Fatal, "Excel process watchdog thread panicked")
        })?
    }
}

enum WatchdogCommand {
    Arm {
        identity: ExcelOwnedProcessIdentity,
        response: SyncSender<PlatformResult<()>>,
    },
    CaptureTree {
        identity: ExcelOwnedProcessIdentity,
        response: SyncSender<PlatformResult<()>>,
    },
    ConvergeAndDisarm {
        identity: ExcelOwnedProcessIdentity,
        graceful_timeout: Duration,
        termination_timeout: Duration,
        response: SyncSender<PlatformResult<ExcelProcessTerminationReport>>,
    },
    #[cfg(test)]
    ForceTerminate {
        identity: ExcelOwnedProcessIdentity,
        termination_timeout: Duration,
        response: SyncSender<PlatformResult<ExcelProcessTerminationReport>>,
    },
    ForceTerminateArmed {
        termination_timeout: Duration,
        response: SyncSender<PlatformResult<ExcelProcessTerminationReport>>,
    },
    Shutdown {
        response: SyncSender<PlatformResult<()>>,
    },
}

fn run_watchdog(
    mut backend: Box<dyn ExcelProcessWatchdogBackend>,
    receiver: &Receiver<WatchdogCommand>,
) -> PlatformResult<()> {
    let mut armed: Option<ExcelOwnedProcessIdentity> = None;
    let mut last_monitor_error: Option<PlatformError> = None;
    loop {
        let command = match receive_watchdog_command(
            &mut backend,
            receiver,
            armed.as_ref(),
            &mut last_monitor_error,
        ) {
            Ok(Some(command)) => command,
            Ok(None) => continue,
            Err(()) => break,
        };
        if let WatchdogCommand::Shutdown { response } = command {
            let result = watchdog_shutdown_result(armed.as_ref(), last_monitor_error.as_ref());
            let _response_failure = response.send(result.clone());
            return result;
        }
        handle_watchdog_command(&mut backend, &mut armed, &mut last_monitor_error, command);
    }
    finish_watchdog_state(armed, last_monitor_error)
}

fn receive_watchdog_command(
    backend: &mut Box<dyn ExcelProcessWatchdogBackend>,
    receiver: &Receiver<WatchdogCommand>,
    armed: Option<&ExcelOwnedProcessIdentity>,
    last_monitor_error: &mut Option<PlatformError>,
) -> Result<Option<WatchdogCommand>, ()> {
    if armed.is_some() {
        match receiver.recv_timeout(WATCHDOG_TREE_REFRESH_INTERVAL) {
            Ok(command) => Ok(Some(command)),
            Err(RecvTimeoutError::Timeout) => {
                refresh_armed_tree(backend, armed, last_monitor_error);
                Ok(None)
            }
            Err(RecvTimeoutError::Disconnected) => Err(()),
        }
    } else {
        receiver.recv().map(Some).map_err(|_error| ())
    }
}

fn handle_watchdog_command(
    backend: &mut Box<dyn ExcelProcessWatchdogBackend>,
    armed: &mut Option<ExcelOwnedProcessIdentity>,
    last_monitor_error: &mut Option<PlatformError>,
    command: WatchdogCommand,
) {
    match command {
        WatchdogCommand::Arm { identity, response } => {
            handle_arm(
                armed,
                backend.as_mut(),
                identity,
                &response,
                last_monitor_error,
            );
        }
        WatchdogCommand::CaptureTree { identity, response } => {
            let result = require_armed(armed.as_ref(), &identity).and_then(|()| {
                backend.capture_verified_tree(&identity, WATCHDOG_TREE_REFRESH_INTERVAL)
            });
            set_monitor_result(last_monitor_error, &result);
            let _response_failure = response.send(result);
        }
        WatchdogCommand::ConvergeAndDisarm {
            identity,
            graceful_timeout,
            termination_timeout,
            response,
        } => {
            handle_converge_and_disarm(
                armed,
                backend.as_mut(),
                &identity,
                WatchdogTimeouts {
                    graceful: graceful_timeout,
                    termination: termination_timeout,
                },
                last_monitor_error.as_ref(),
                &response,
            );
        }
        #[cfg(test)]
        WatchdogCommand::ForceTerminate {
            identity,
            termination_timeout,
            response,
        } => {
            handle_force_terminate(
                armed,
                backend.as_mut(),
                &identity,
                termination_timeout,
                last_monitor_error.as_ref(),
                &response,
            );
        }
        WatchdogCommand::ForceTerminateArmed {
            termination_timeout,
            response,
        } => {
            handle_force_terminate_armed(
                armed,
                backend.as_mut(),
                termination_timeout,
                last_monitor_error.as_ref(),
                &response,
            );
        }
        WatchdogCommand::Shutdown { .. } => {}
    }
}

fn watchdog_shutdown_result(
    armed: Option<&ExcelOwnedProcessIdentity>,
    last_monitor_error: Option<&PlatformError>,
) -> PlatformResult<()> {
    last_monitor_error.map_or_else(
        || shutdown_watchdog_result(armed),
        |error| {
            Err(PlatformError::new(
                ErrorCode::Fatal,
                format!("Excel watchdog tree monitor failed: {error}"),
            ))
        },
    )
}

fn finish_watchdog_state(
    armed: Option<ExcelOwnedProcessIdentity>,
    last_monitor_error: Option<PlatformError>,
) -> PlatformResult<()> {
    last_monitor_error.map_or_else(
        || {
            armed.map_or(Ok(()), |identity| {
                Err(watchdog_error(format!(
                    "Excel watchdog controller disconnected while process {} was still armed",
                    identity.process_id()
                )))
            })
        },
        Err,
    )
}

fn set_monitor_result(last_monitor_error: &mut Option<PlatformError>, result: &PlatformResult<()>) {
    if let Err(error) = result
        && last_monitor_error.is_none()
    {
        *last_monitor_error = Some(error.clone());
    }
}

fn require_armed(
    armed: Option<&ExcelOwnedProcessIdentity>,
    requested: &ExcelOwnedProcessIdentity,
) -> PlatformResult<()> {
    match armed {
        Some(current) if current == requested => Ok(()),
        Some(current) => Err(watchdog_error(format!(
            "Excel watchdog identity mismatch: armed={} requested={}",
            current.process_id(),
            requested.process_id()
        ))),
        None => Err(watchdog_error(
            "Excel watchdog has no armed owned process identity",
        )),
    }
}

fn handle_arm(
    armed: &mut Option<ExcelOwnedProcessIdentity>,
    backend: &mut dyn ExcelProcessWatchdogBackend,
    identity: ExcelOwnedProcessIdentity,
    response: &SyncSender<PlatformResult<()>>,
    last_monitor_error: &mut Option<PlatformError>,
) {
    let result = match armed.as_ref() {
        None => {
            *armed = Some(identity);
            Ok(())
        }
        Some(current) if *current == identity => Ok(()),
        Some(current) => Err(watchdog_error(format!(
            "Excel watchdog is already armed for process {}; rejected process {}",
            current.process_id(),
            identity.process_id()
        ))),
    }
    .and_then(|()| {
        let identity = armed
            .as_ref()
            .ok_or_else(|| watchdog_error("Excel watchdog arm identity disappeared"))?;
        backend.capture_verified_tree(identity, WATCHDOG_TREE_REFRESH_INTERVAL)
    });
    if result.is_ok() {
        *last_monitor_error = None;
    } else {
        *last_monitor_error = result.clone().err();
    }
    let _response_failure = response.send(result);
}

fn refresh_armed_tree(
    backend: &mut Box<dyn ExcelProcessWatchdogBackend>,
    armed: Option<&ExcelOwnedProcessIdentity>,
    last_monitor_error: &mut Option<PlatformError>,
) {
    let Some(identity) = armed else {
        return;
    };
    match backend.capture_verified_tree(identity, WATCHDOG_TREE_REFRESH_INTERVAL) {
        Ok(()) => {}
        Err(error) if last_monitor_error.is_none() => *last_monitor_error = Some(error),
        Err(_error) => {}
    }
}

fn handle_converge_and_disarm(
    armed: &mut Option<ExcelOwnedProcessIdentity>,
    backend: &mut dyn ExcelProcessWatchdogBackend,
    identity: &ExcelOwnedProcessIdentity,
    timeouts: WatchdogTimeouts,
    last_monitor_error: Option<&PlatformError>,
    response: &SyncSender<PlatformResult<ExcelProcessTerminationReport>>,
) {
    if let Some(error) = last_monitor_error {
        let result = terminate_after_monitor_error(backend, identity, timeouts.termination, error);
        let _response_failure = response.send(result);
        return;
    }
    let result = require_armed(armed.as_ref(), identity).and_then(|()| {
        if backend.wait_for_identity_exit(identity, timeouts.graceful)? {
            *armed = None;
            Ok(ExcelProcessTerminationReport::new(false, false, 0, true))
        } else {
            let report = backend.terminate_verified_tree(identity, timeouts.termination)?;
            if report.converged() {
                *armed = None;
                Ok(report)
            } else {
                Err(watchdog_error(
                    "verified Excel termination did not converge within its deadline",
                ))
            }
        }
    });
    let _response_failure = response.send(result);
}

#[derive(Clone, Copy)]
struct WatchdogTimeouts {
    graceful: Duration,
    termination: Duration,
}

#[cfg(test)]
fn handle_force_terminate(
    armed: &mut Option<ExcelOwnedProcessIdentity>,
    backend: &mut dyn ExcelProcessWatchdogBackend,
    identity: &ExcelOwnedProcessIdentity,
    termination_timeout: Duration,
    last_monitor_error: Option<&PlatformError>,
    response: &SyncSender<PlatformResult<ExcelProcessTerminationReport>>,
) {
    if let Some(error) = last_monitor_error {
        let result = terminate_after_monitor_error(backend, identity, termination_timeout, error);
        let _response_failure = response.send(result);
        return;
    }
    let current = armed.clone();
    let result = match current {
        None => Ok(ExcelProcessTerminationReport::new(false, false, 0, true)),
        Some(current) if current != *identity => Err(watchdog_error(format!(
            "Excel watchdog identity mismatch: armed={} requested={}",
            current.process_id(),
            identity.process_id()
        ))),
        Some(current) => terminate_armed_identity(
            armed,
            backend,
            &current,
            termination_timeout,
            "forced Excel termination",
        ),
    };
    let _response_failure = response.send(result);
}

fn handle_force_terminate_armed(
    armed: &mut Option<ExcelOwnedProcessIdentity>,
    backend: &mut dyn ExcelProcessWatchdogBackend,
    termination_timeout: Duration,
    last_monitor_error: Option<&PlatformError>,
    response: &SyncSender<PlatformResult<ExcelProcessTerminationReport>>,
) {
    if let Some(error) = last_monitor_error {
        let Some(identity) = armed.as_ref() else {
            let _response_failure = response.send(Err(watchdog_error(format!(
                "Excel watchdog tree monitor failed without an armed identity: {error}"
            ))));
            return;
        };
        let result = terminate_after_monitor_error(backend, identity, termination_timeout, error);
        let _response_failure = response.send(result);
        return;
    }
    let current = armed.clone();
    let result = current.map_or(
        Ok(ExcelProcessTerminationReport::new(false, false, 0, true)),
        |current| {
            terminate_armed_identity(
                armed,
                backend,
                &current,
                termination_timeout,
                "armed Excel termination",
            )
        },
    );
    let _response_failure = response.send(result);
}

fn terminate_after_monitor_error(
    backend: &mut dyn ExcelProcessWatchdogBackend,
    identity: &ExcelOwnedProcessIdentity,
    termination_timeout: Duration,
    monitor_error: &PlatformError,
) -> PlatformResult<ExcelProcessTerminationReport> {
    match backend.terminate_verified_tree(identity, termination_timeout) {
        Ok(report) => Err(watchdog_error(format!(
            "Excel watchdog tree capture was incomplete: {monitor_error}; known-tree termination converged={}, terminated={}",
            report.converged(),
            report.terminated_process_count
        ))),
        Err(termination_error) => Err(watchdog_error(format!(
            "Excel watchdog tree capture was incomplete: {monitor_error}; known-tree termination also failed: {termination_error}"
        ))),
    }
}

fn terminate_armed_identity(
    armed: &mut Option<ExcelOwnedProcessIdentity>,
    backend: &mut dyn ExcelProcessWatchdogBackend,
    identity: &ExcelOwnedProcessIdentity,
    termination_timeout: Duration,
    context: &str,
) -> PlatformResult<ExcelProcessTerminationReport> {
    let report = backend.terminate_verified_tree(identity, termination_timeout)?;
    if report.converged() {
        *armed = None;
        Ok(report)
    } else {
        Err(watchdog_error(format!(
            "{context} did not converge within its deadline"
        )))
    }
}

fn shutdown_watchdog_result(armed: Option<&ExcelOwnedProcessIdentity>) -> PlatformResult<()> {
    armed.map_or(Ok(()), |identity| {
        Err(watchdog_error(format!(
            "Excel watchdog still owns process {} and cannot shut down",
            identity.process_id()
        )))
    })
}

fn receive_watchdog_response<T>(
    receiver: &Receiver<PlatformResult<T>>,
    timeout: Duration,
) -> PlatformResult<T> {
    match receiver.recv_timeout(timeout) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => Err(PlatformError::new(
            ErrorCode::TargetUnresponsive,
            "Excel process watchdog response timed out",
        )),
        Err(RecvTimeoutError::Disconnected) => Err(watchdog_error(
            "Excel process watchdog disconnected without producing a response",
        )),
    }
}

fn watchdog_queue_error(error: &TrySendError<WatchdogCommand>) -> PlatformError {
    match error {
        TrySendError::Full(_) => PlatformError::new(
            ErrorCode::TargetUnresponsive,
            "Excel process watchdog command queue is full",
        ),
        TrySendError::Disconnected(_) => {
            watchdog_error("Excel process watchdog disconnected before accepting the command")
        }
    }
}

fn watchdog_error(message: impl Into<String>) -> PlatformError {
    PlatformError::new(ErrorCode::Fatal, message)
}

#[cfg(test)]
#[path = "excel_process_watchdog_tests.rs"]
mod tests;
