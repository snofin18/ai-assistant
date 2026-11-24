//! Test backends for the Excel COM runner prototype.
//!
//! Responsibilities:
//! - record stage order, worker thread identity, failures, and panics;
//! - provide deterministic queue blocking without real OS or Excel dependencies.
//!
//! Boundaries:
//! - these backends exist only in crate tests;
//! - they never create COM objects, launch Excel, or touch workbook files.
//!
//! Invariants:
//! 1. every stage is observed on the worker thread;
//! 2. injected failures preserve the requested `ErrorCode`;
//! 3. queue tests release their gate before shutdown.

#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used
)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, ThreadId};
use std::time::Duration;

use assistant_platform_api::ErrorCode;

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    Create,
    ProcessId,
    Quit,
}

#[derive(Default)]
struct Tracker {
    events: Vec<Stage>,
    threads: Vec<ThreadId>,
    failure: Option<(Stage, ErrorCode)>,
    panic_at: Option<Stage>,
}

struct RecordingBackend {
    tracker: Arc<Mutex<Tracker>>,
}

impl RecordingBackend {
    fn new(
        failure: Option<(Stage, ErrorCode)>,
        panic_at: Option<Stage>,
    ) -> (Self, Arc<Mutex<Tracker>>) {
        let tracker = Arc::new(Mutex::new(Tracker {
            failure,
            panic_at,
            ..Tracker::default()
        }));
        (
            Self {
                tracker: Arc::clone(&tracker),
            },
            tracker,
        )
    }

    fn observe(&self, stage: Stage) -> PlatformResult<()> {
        let (should_panic, failure) = {
            let mut tracker = self.tracker.lock().expect("tracker mutex");
            tracker.events.push(stage);
            tracker.threads.push(thread::current().id());
            (tracker.panic_at == Some(stage), tracker.failure)
        };
        assert!(!should_panic, "injected backend panic");
        if let Some((failure_stage, code)) = failure
            && failure_stage == stage
        {
            return Err(PlatformError::new(code, "injected backend failure"));
        }
        Ok(())
    }
}

impl ExcelComBackend for RecordingBackend {
    fn create_application(&mut self) -> PlatformResult<()> {
        self.observe(Stage::Create)
    }

    fn application_process_id(&mut self) -> PlatformResult<u32> {
        self.observe(Stage::ProcessId)?;
        Ok(4242)
    }

    fn quit_application(&mut self) -> PlatformResult<()> {
        self.observe(Stage::Quit)
    }
}

struct CreateGate {
    started: Mutex<bool>,
    started_ready: Condvar,
    released: Mutex<bool>,
    release_ready: Condvar,
}

impl CreateGate {
    fn new() -> Self {
        Self {
            started: Mutex::new(false),
            started_ready: Condvar::new(),
            released: Mutex::new(false),
            release_ready: Condvar::new(),
        }
    }

    fn wait_until_started(&self) {
        let mut started = self.started.lock().expect("started mutex");
        while !*started {
            started = self.started_ready.wait(started).expect("started wait");
        }
        drop(started);
    }

    fn release(&self) {
        *self.released.lock().expect("released mutex") = true;
        self.release_ready.notify_all();
    }

    fn block_create(&self) {
        *self.started.lock().expect("started mutex") = true;
        self.started_ready.notify_all();
        let mut released = self.released.lock().expect("released mutex");
        while !*released {
            released = self.release_ready.wait(released).expect("release wait");
        }
        drop(released);
    }
}

struct BlockingBackend {
    gate: Arc<CreateGate>,
    quit_called: Arc<AtomicBool>,
}

impl ExcelComBackend for BlockingBackend {
    fn create_application(&mut self) -> PlatformResult<()> {
        self.gate.block_create();
        Ok(())
    }

    fn application_process_id(&mut self) -> PlatformResult<u32> {
        Ok(77)
    }

    fn quit_application(&mut self) -> PlatformResult<()> {
        self.quit_called.store(true, Ordering::SeqCst);
        Ok(())
    }
}

fn recording_runner(
    failure: Option<(Stage, ErrorCode)>,
    panic_at: Option<Stage>,
) -> (ExcelComRunner, Arc<Mutex<Tracker>>) {
    let (backend, tracker) = RecordingBackend::new(failure, panic_at);
    let runner = ExcelComRunner::spawn(move || backend).expect("runner");
    (runner, tracker)
}

#[test]
fn test_runner_preserves_fifo_order_on_dedicated_thread() {
    let main_thread = thread::current().id();
    let (mut runner, tracker) = recording_runner(None, None);

    runner.create_application().expect("create");
    assert_eq!(runner.application_process_id().expect("process id"), 4242);
    runner.quit_application().expect("quit");
    runner.shutdown().expect("shutdown");

    let tracker = tracker.lock().expect("tracker mutex");
    assert_eq!(
        tracker.events,
        vec![Stage::Create, Stage::ProcessId, Stage::Quit]
    );
    assert_eq!(tracker.threads.len(), 3);
    assert!(
        tracker
            .threads
            .iter()
            .all(|thread_id| *thread_id != main_thread)
    );
    assert!(tracker.threads.windows(2).all(|pair| pair[0] == pair[1]));
    drop(tracker);
}

fn assert_failure_code_preserved(stage: Stage, code: ErrorCode) {
    let (mut runner, tracker) = recording_runner(Some((stage, code)), None);

    match stage {
        Stage::Create => {
            let error = runner.create_application().expect_err("create failure");
            assert_eq!(error.code(), code);
        }
        Stage::ProcessId => {
            runner.create_application().expect("create");
            let error = runner
                .application_process_id()
                .expect_err("process failure");
            assert_eq!(error.code(), code);
        }
        Stage::Quit => {
            runner.create_application().expect("create");
            let error = runner.quit_application().expect_err("quit failure");
            assert_eq!(error.code(), code);
        }
    }

    let shutdown = runner.shutdown();
    if stage == Stage::Quit {
        assert!(
            shutdown.is_err(),
            "shutdown must not hide a failed explicit quit"
        );
    } else {
        shutdown.expect("shutdown");
    }
    let failure = {
        let tracker = tracker.lock().expect("tracker mutex");
        tracker.failure
    };
    assert_eq!(failure, Some((stage, code)));
}

#[test]
fn test_runner_preserves_backend_error_codes_without_downgrade() {
    assert_failure_code_preserved(Stage::Create, ErrorCode::TargetNotFound);
    assert_failure_code_preserved(Stage::ProcessId, ErrorCode::PlatformPermission);
    assert_failure_code_preserved(Stage::Quit, ErrorCode::UserInteraction);
}

#[test]
fn test_runner_request_queue_capacity_is_one() {
    let gate = Arc::new(CreateGate::new());
    let quit_called = Arc::new(AtomicBool::new(false));
    let mut runner = ExcelComRunner::spawn({
        let gate = Arc::clone(&gate);
        let quit_called = Arc::clone(&quit_called);
        move || BlockingBackend { gate, quit_called }
    })
    .expect("runner");

    thread::scope(|scope| {
        let create = scope.spawn(|| runner.create_application());
        gate.wait_until_started();

        let (queued_sender, queued_receiver) = mpsc::sync_channel(1);
        runner
            .sender
            .try_send(RunnerCommand::ApplicationProcessId(queued_sender))
            .expect("second request should occupy the queue");

        let full = runner
            .dispatch(RunnerCommand::QuitApplication)
            .expect_err("third dispatch must fail full");
        assert_eq!(full.code(), ErrorCode::TargetUnresponsive);

        gate.release();
        create.join().expect("create thread").expect("create");
        assert_eq!(
            queued_receiver
                .recv_timeout(Duration::from_secs(1))
                .expect("queued response")
                .expect("queued process id"),
            RunnerValue::ProcessId(77)
        );
    });
    runner.shutdown().expect("shutdown");
    assert!(quit_called.load(Ordering::SeqCst));
}

#[test]
fn test_worker_abandons_created_application_when_response_receiver_disappears() {
    let gate = Arc::new(CreateGate::new());
    let quit_called = Arc::new(AtomicBool::new(false));
    let runner = ExcelComRunner::spawn({
        let gate = Arc::clone(&gate);
        let quit_called = Arc::clone(&quit_called);
        move || BlockingBackend { gate, quit_called }
    })
    .expect("runner");
    let (response_sender, response_receiver) = mpsc::sync_channel(1);
    runner
        .sender
        .try_send(RunnerCommand::CreateApplication(response_sender))
        .expect("create command");
    gate.wait_until_started();
    drop(response_receiver);
    gate.release();

    let mut runner = runner;
    let shutdown = runner
        .shutdown()
        .expect_err("worker should have disconnected after response loss");
    assert_eq!(shutdown.code(), ErrorCode::Fatal);
    assert!(quit_called.load(Ordering::SeqCst));
}

#[test]
fn test_runner_disconnect_and_repeated_shutdown_fail_explicitly() {
    let (mut runner, _tracker) = recording_runner(None, None);
    runner.shutdown().expect("first shutdown");

    let disconnected = runner
        .application_process_id()
        .expect_err("disconnected runner must fail");
    assert_eq!(disconnected.code(), ErrorCode::Fatal);
    let repeated = runner.shutdown().expect_err("second shutdown must fail");
    assert_eq!(repeated.code(), ErrorCode::Fatal);
}

#[test]
fn test_runner_thread_panic_is_explicit_and_not_reported_as_success() {
    let (mut runner, _tracker) = recording_runner(None, Some(Stage::Create));
    let create = runner
        .create_application()
        .expect_err("panicking worker must fail");
    assert_eq!(create.code(), ErrorCode::Fatal);
    let shutdown = runner
        .shutdown()
        .expect_err("panicked worker shutdown must fail");
    assert_eq!(shutdown.code(), ErrorCode::Fatal);
}

#[test]
fn test_runner_state_rejects_out_of_order_operations() {
    let (mut runner, _tracker) = recording_runner(None, None);
    let before_create = runner
        .application_process_id()
        .expect_err("process id before create must fail");
    assert_eq!(before_create.code(), ErrorCode::Fatal);
    runner.create_application().expect("create");
    let duplicate = runner
        .create_application()
        .expect_err("duplicate create must fail");
    assert_eq!(duplicate.code(), ErrorCode::Fatal);
    runner.quit_application().expect("quit");
    let after_quit = runner
        .quit_application()
        .expect_err("duplicate quit must fail");
    assert_eq!(after_quit.code(), ErrorCode::Fatal);
    runner.shutdown().expect("shutdown");
}

#[cfg(windows)]
#[test]
fn test_windows_idispatch_backend_starts_without_creating_excel() {
    let mut runner = spawn_windows_excel_runner().expect("Windows COM runner");
    runner.shutdown().expect("idle Windows runner shutdown");
}
