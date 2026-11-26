//! Tests for the platform-private Excel process watchdog.
//!
//! Responsibilities:
//! - prove arm/disarm and stale-identity behavior with a scripted backend;
//! - prove Windows verified termination on a controlled, non-GUI process tree.
//!
//! Boundaries:
//! - fake tests never create Excel or COM;
//! - the Windows test spawns only a hidden, short-lived helper process tree.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use assistant_platform_api::ErrorCode;

use super::*;
#[cfg(windows)]
use crate::excel_process::{process_identity, process_tree_snapshot};

pub(super) struct NoopWatchdogBackend;

impl ExcelProcessWatchdogBackend for NoopWatchdogBackend {
    fn capture_verified_tree(
        &mut self,
        _identity: &ExcelOwnedProcessIdentity,
        _timeout: Duration,
    ) -> PlatformResult<()> {
        // STUB(TASK-412): fake runner tests never own an OS process.
        Ok(())
    }

    fn wait_for_identity_exit(
        &mut self,
        _identity: &ExcelOwnedProcessIdentity,
        _timeout: Duration,
    ) -> PlatformResult<bool> {
        Ok(true)
    }

    fn terminate_verified_tree(
        &mut self,
        _identity: &ExcelOwnedProcessIdentity,
        _timeout: Duration,
    ) -> PlatformResult<ExcelProcessTerminationReport> {
        Ok(ExcelProcessTerminationReport::new(false, false, 0, true))
    }
}

struct ScriptedBackend {
    state: Arc<Mutex<ScriptedState>>,
}

struct ScriptedState {
    wait_result: bool,
    termination_report: ExcelProcessTerminationReport,
    capture_failures_remaining: usize,
    events: Vec<&'static str>,
}

impl ScriptedBackend {
    fn new(
        wait_result: bool,
        termination_report: ExcelProcessTerminationReport,
    ) -> (Self, Arc<Mutex<ScriptedState>>) {
        let state = Arc::new(Mutex::new(ScriptedState {
            wait_result,
            termination_report,
            capture_failures_remaining: 0,
            events: Vec::new(),
        }));
        (
            Self {
                state: Arc::clone(&state),
            },
            state,
        )
    }
}

impl ExcelProcessWatchdogBackend for ScriptedBackend {
    fn capture_verified_tree(
        &mut self,
        _identity: &ExcelOwnedProcessIdentity,
        _timeout: Duration,
    ) -> PlatformResult<()> {
        let mut state = self.state.lock().expect("scripted state");
        if state.capture_failures_remaining > 0 {
            state.capture_failures_remaining -= 1;
            return Err(PlatformError::new(
                ErrorCode::Fatal,
                "injected capture failure",
            ));
        }
        if state.events.last().copied() != Some("capture") {
            state.events.push("capture");
        }
        drop(state);
        Ok(())
    }

    fn wait_for_identity_exit(
        &mut self,
        _identity: &ExcelOwnedProcessIdentity,
        _timeout: Duration,
    ) -> PlatformResult<bool> {
        let mut state = self.state.lock().expect("scripted state");
        state.events.push("wait");
        Ok(state.wait_result)
    }

    fn terminate_verified_tree(
        &mut self,
        _identity: &ExcelOwnedProcessIdentity,
        _timeout: Duration,
    ) -> PlatformResult<ExcelProcessTerminationReport> {
        let mut state = self.state.lock().expect("scripted state");
        state.events.push("terminate");
        Ok(state.termination_report)
    }
}

fn identity(process_id: u32) -> ExcelOwnedProcessIdentity {
    ExcelOwnedProcessIdentity::new(
        process_id,
        u64::from(process_id) * 10,
        r"C:\Program Files\Microsoft Office\root\Office16\EXCEL.EXE".to_owned(),
    )
    .expect("test identity")
}

#[test]
fn test_watchdog_arms_and_disarms_after_graceful_exit() {
    let (backend, state) = ScriptedBackend::new(
        true,
        ExcelProcessTerminationReport::new(false, false, 0, true),
    );
    let (mut watchdog, controller) =
        ExcelProcessWatchdog::start_with_backend(Box::new(backend)).expect("watchdog");
    let owned = identity(41);
    controller.arm(&owned, Duration::from_secs(1)).expect("arm");
    let report = controller
        .converge_and_disarm(
            &owned,
            Duration::from_millis(10),
            Duration::from_millis(10),
            Duration::from_secs(1),
        )
        .expect("graceful convergence");
    assert!(report.converged());
    assert!(!report.termination_attempted());
    watchdog
        .shutdown(Duration::from_secs(1))
        .expect("watchdog shutdown");
    assert_eq!(
        state.lock().expect("scripted state").events,
        ["capture", "wait"]
    );
}

#[test]
fn test_watchdog_rejects_stale_identity_without_termination() {
    let (backend, state) = ScriptedBackend::new(
        false,
        ExcelProcessTerminationReport::new(true, true, 1, true),
    );
    let (mut watchdog, controller) =
        ExcelProcessWatchdog::start_with_backend(Box::new(backend)).expect("watchdog");
    let armed = identity(41);
    controller.arm(&armed, Duration::from_secs(1)).expect("arm");
    let foreign = identity(42);
    let error = controller
        .force_terminate(&foreign, Duration::from_millis(10), Duration::from_secs(1))
        .expect_err("stale identity");
    assert_eq!(error.code(), ErrorCode::Fatal);
    assert_eq!(state.lock().expect("scripted state").events, ["capture"]);
    let active = controller
        .force_terminate(&armed, Duration::from_millis(10), Duration::from_secs(1))
        .expect("armed termination");
    assert!(active.converged());
    watchdog
        .shutdown(Duration::from_secs(1))
        .expect("watchdog shutdown");
}

#[test]
fn test_watchdog_force_terminates_when_graceful_wait_expires() {
    let (backend, state) = ScriptedBackend::new(
        false,
        ExcelProcessTerminationReport::new(true, true, 2, true),
    );
    let (mut watchdog, controller) =
        ExcelProcessWatchdog::start_with_backend(Box::new(backend)).expect("watchdog");
    let owned = identity(51);
    controller.arm(&owned, Duration::from_secs(1)).expect("arm");
    let report = controller
        .converge_and_disarm(
            &owned,
            Duration::from_millis(10),
            Duration::from_millis(10),
            Duration::from_secs(1),
        )
        .expect("forced convergence");
    assert!(report.termination_attempted());
    assert_eq!(report.terminated_process_count(), 2);
    watchdog
        .shutdown(Duration::from_secs(1))
        .expect("watchdog shutdown");
    assert_eq!(
        state.lock().expect("scripted state").events,
        ["capture", "wait", "terminate"]
    );
}

#[test]
fn test_watchdog_retains_armed_identity_when_termination_does_not_converge() {
    let (backend, _state) = ScriptedBackend::new(
        false,
        ExcelProcessTerminationReport::new(true, true, 1, false),
    );
    let (mut watchdog, controller) =
        ExcelProcessWatchdog::start_with_backend(Box::new(backend)).expect("watchdog");
    let owned = identity(61);
    controller.arm(&owned, Duration::from_secs(1)).expect("arm");
    let error = controller
        .force_terminate(&owned, Duration::from_millis(10), Duration::from_secs(1))
        .expect_err("non-converged termination");
    assert_eq!(error.code(), ErrorCode::Fatal);
    let shutdown = watchdog
        .shutdown(Duration::from_secs(1))
        .expect_err("armed watchdog must not shut down silently");
    assert_eq!(shutdown.code(), ErrorCode::Fatal);
}

#[test]
fn test_watchdog_capture_error_remains_sticky_after_disarm() {
    let (backend, state) = ScriptedBackend::new(
        false,
        ExcelProcessTerminationReport::new(true, true, 1, true),
    );
    state
        .lock()
        .expect("scripted state")
        .capture_failures_remaining = 1;
    let (mut watchdog, controller) =
        ExcelProcessWatchdog::start_with_backend(Box::new(backend)).expect("watchdog");
    let owned = identity(71);
    let arm_error = controller
        .arm(&owned, Duration::from_secs(1))
        .expect_err("capture failure must fail arm");
    assert_eq!(arm_error.code(), ErrorCode::Fatal);
    let converge_error = controller
        .converge_and_disarm(
            &owned,
            Duration::from_millis(10),
            Duration::from_millis(10),
            Duration::from_secs(1),
        )
        .expect_err("monitor failure must fail convergence");
    assert_eq!(converge_error.code(), ErrorCode::Fatal);
    let shutdown = watchdog
        .shutdown(Duration::from_secs(1))
        .expect_err("monitor failure must remain visible");
    assert_eq!(shutdown.code(), ErrorCode::Fatal);
}

#[test]
fn test_watchdog_response_timeout_is_explicit_and_bounded() {
    let (sender, receiver) = std::sync::mpsc::sync_channel::<PlatformResult<()>>(1);
    let error = receive_watchdog_response(&receiver, Duration::from_millis(10))
        .expect_err("missing response");
    assert_eq!(error.code(), ErrorCode::TargetUnresponsive);
    drop(sender);
}

#[test]
fn test_watchdog_thread_reap_deadline_is_bounded() {
    let (mut watchdog, controller) = ExcelProcessWatchdog::start_for_tests().expect("watchdog");
    let error = watchdog
        .reap_thread(Duration::from_millis(1))
        .expect_err("idle watchdog must not reap before its thread exits");
    assert_eq!(error.code(), ErrorCode::Fatal);
    drop(controller);
    drop(watchdog);
}

#[cfg(windows)]
struct ControlledChildTree {
    child: Option<std::process::Child>,
    identity: ExcelOwnedProcessIdentity,
}

#[cfg(windows)]
impl ControlledChildTree {
    fn spawn() -> Self {
        use std::os::windows::process::CommandExt;
        use windows::Win32::System::Threading::CREATE_NO_WINDOW;

        let mut command = std::process::Command::new("powershell.exe");
        command.args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            "$child=Start-Process -FilePath 'ping.exe' -ArgumentList '-n','30','127.0.0.1' -PassThru -WindowStyle Hidden; Start-Sleep -Seconds 30",
        ]);
        command.stdout(std::process::Stdio::null());
        command.stderr(std::process::Stdio::null());
        command.creation_flags(CREATE_NO_WINDOW.0);
        let child = command.spawn().expect("spawn hidden process tree");
        let process_id = child.id();
        let observed = process_identity(process_id)
            .expect("capture child identity")
            .expect("child is live");
        let identity = ExcelOwnedProcessIdentity::new(
            observed.process_id,
            observed.start_time_100ns,
            observed.executable_path,
        )
        .expect("owned test identity");
        Self {
            child: Some(child),
            identity,
        }
    }

    fn wait_for_descendant(&self) {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            let entries = process_tree_snapshot().expect("process snapshot");
            if entries.iter().any(|entry| {
                entry.parent_process_id == self.identity.process_id()
                    && entry.process_name.eq_ignore_ascii_case("PING.EXE")
            }) {
                return;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "controlled child tree did not produce a descendant"
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}

#[cfg(windows)]
impl Drop for ControlledChildTree {
    fn drop(&mut self) {
        let mut backend = super::super::process_watchdog_windows::WindowsProcessTreeBackend::new();
        let _cleanup = backend.terminate_verified_tree(&self.identity, Duration::from_secs(5));
        if let Some(mut child) = self.child.take() {
            let _status = child.wait();
        }
    }
}

#[cfg(windows)]
#[test]
fn test_windows_watchdog_refuses_stale_root_identity_without_killing() {
    let current_process_id = std::process::id();
    let current = process_identity(current_process_id)
        .expect("capture current process")
        .expect("test process is live");
    let stale = ExcelOwnedProcessIdentity::new(
        current.process_id,
        current.start_time_100ns.saturating_add(1),
        current.executable_path,
    )
    .expect("stale identity");
    let mut backend = super::super::process_watchdog_windows::WindowsProcessTreeBackend::new();
    let report = backend
        .terminate_verified_tree(&stale, Duration::from_millis(10))
        .expect("identity mismatch is convergence, not a kill");
    assert!(!report.root_was_alive());
    assert!(!report.termination_attempted());
    assert_eq!(report.terminated_process_count(), 0);
    assert!(report.converged());
    assert!(
        process_identity(current_process_id)
            .expect("query current process")
            .is_some()
    );
}

#[cfg(windows)]
#[test]
fn test_windows_watchdog_terminates_and_reaps_controlled_process_tree() {
    let mut tree = ControlledChildTree::spawn();
    tree.wait_for_descendant();
    let mut backend = super::super::process_watchdog_windows::WindowsProcessTreeBackend::new();
    let report = backend
        .terminate_verified_tree(&tree.identity, Duration::from_secs(10))
        .expect("verified process-tree termination");
    assert!(report.root_was_alive());
    assert!(report.termination_attempted());
    assert!(report.terminated_process_count() >= 2);
    assert!(report.converged());
    let status = tree
        .child
        .take()
        .expect("controlled child")
        .wait()
        .expect("reap controlled child");
    assert!(!status.success());
    assert!(
        process_identity(tree.identity.process_id())
            .expect("query root")
            .is_none()
    );
}

#[cfg(windows)]
#[test]
fn test_windows_watchdog_detects_descendant_after_root_exit() {
    let mut tree = ControlledChildTree::spawn();
    tree.wait_for_descendant();
    let mut backend = super::super::process_watchdog_windows::WindowsProcessTreeBackend::new();
    backend
        .capture_verified_tree(&tree.identity, Duration::from_secs(1))
        .expect("capture controlled tree");

    let mut root = tree.child.take().expect("controlled root");
    root.kill().expect("kill root only");
    let _status = root.wait().expect("reap root");

    let exited = backend
        .wait_for_identity_exit(&tree.identity, Duration::from_millis(50))
        .expect("wait for tracked tree");
    assert!(
        !exited,
        "a live descendant must prevent graceful convergence"
    );
    let report = backend
        .terminate_verified_tree(&tree.identity, Duration::from_secs(10))
        .expect("terminate tracked descendant");
    assert!(!report.root_was_alive());
    assert!(report.termination_attempted());
    assert!(report.terminated_process_count() >= 1);
    assert!(report.converged());
}
