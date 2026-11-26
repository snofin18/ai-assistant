//! Windows-only startup, readiness, and failed-start cleanup for the Excel runner.
//!
//! The worker still enters COM only through `crate::com::with_com_apartment`.
//! This module keeps startup cleanup bounded and leaves the watchdog thread
//! independent from the STA.

#[cfg(windows)]
use super::super::{
    ErrorCode, ExcelComRunner, ExcelOwnershipJournalStore, ExcelProcessWatchdog, PlatformError,
    PlatformResult, REQUEST_QUEUE_CAPACITY, REQUEST_TIMEOUT, WATCHDOG_RESPONSE_WAIT,
    WORKER_REAP_TIMEOUT, WORKER_THREAD_NAME, mpsc, next_runner_id, run_worker,
    runner_protocol_error, thread, thread_spawn_error,
};
#[cfg(windows)]
use super::WindowsExcelComBackend;
#[cfg(windows)]
use std::sync::Mutex;
#[cfg(windows)]
use std::sync::atomic::{AtomicUsize, Ordering};
#[cfg(windows)]
use std::time::{Duration, Instant};

#[cfg(windows)]
const MAX_QUARANTINED_STARTUP_WORKERS: usize = 1;
#[cfg(windows)]
const STARTUP_REAP_POLL_INTERVAL: Duration = Duration::from_millis(20);
#[cfg(windows)]
static QUARANTINED_STARTUP_WORKERS: AtomicUsize = AtomicUsize::new(0);
#[cfg(windows)]
static STARTUP_LOCK: Mutex<()> = Mutex::new(());

#[cfg(windows)]
/// Starts one Windows Excel COM worker and its independent watchdog.
///
/// This function creates no Excel process. It returns `Fatal` when watchdog,
/// journal-root, worker-thread, STA, or bounded readiness setup fails; failed
/// startup reaps the worker and watchdog within their deadlines. The call is
/// not idempotent because each invocation creates one new runner identity.
pub fn spawn_windows_excel_runner() -> PlatformResult<ExcelComRunner> {
    let _startup_guard = STARTUP_LOCK.try_lock().map_err(|_error| {
        PlatformError::new(
            ErrorCode::Fatal,
            "another Excel runner startup is already in progress",
        )
    })?;
    if QUARANTINED_STARTUP_WORKERS.load(Ordering::Acquire) >= MAX_QUARANTINED_STARTUP_WORKERS {
        return Err(PlatformError::new(
            ErrorCode::Fatal,
            "previous Excel startup worker is still in bounded quarantine",
        ));
    }
    let runner_id = next_runner_id();
    #[cfg(test)]
    let journal_store = ExcelOwnershipJournalStore::for_tests()?;
    #[cfg(not(test))]
    let journal_store = ExcelOwnershipJournalStore::from_environment()?;
    let (mut watchdog, watchdog_controller) = ExcelProcessWatchdog::start_windows()?;
    let (sender, receiver) = mpsc::sync_channel(REQUEST_QUEUE_CAPACITY);
    let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
    let worker = match spawn_windows_worker(
        runner_id,
        receiver,
        ready_sender,
        journal_store,
        watchdog_controller,
    ) {
        Ok(worker) => worker,
        Err(error) => {
            if let Err(cleanup_error) = watchdog.shutdown(WATCHDOG_RESPONSE_WAIT) {
                return Err(PlatformError::new(
                    ErrorCode::Fatal,
                    format!("{error}; watchdog cleanup failed: {cleanup_error}"),
                ));
            }
            return Err(error);
        }
    };
    let (watchdog, worker) = wait_for_windows_worker_ready(watchdog, worker, &ready_receiver)?;
    Ok(ExcelComRunner {
        sender,
        worker: Some(worker),
        watchdog: Some(watchdog),
        shutdown_started: false,
    })
}

#[cfg(windows)]
fn spawn_windows_worker(
    runner_id: u64,
    receiver: mpsc::Receiver<super::super::RunnerCommand>,
    ready_sender: mpsc::SyncSender<PlatformResult<()>>,
    journal_store: ExcelOwnershipJournalStore,
    watchdog_controller: super::super::ExcelProcessWatchdogController,
) -> PlatformResult<thread::JoinHandle<PlatformResult<()>>> {
    let mut ready_sender = Some(ready_sender);
    thread::Builder::new()
        .name(WORKER_THREAD_NAME.to_owned())
        .spawn(move || {
            let initialization = crate::com::with_com_apartment(|_guard| {
                if let Some(sender) = ready_sender.take() {
                    sender.send(Ok(())).map_err(|_| {
                        runner_protocol_error("Excel COM runner readiness receiver disappeared")
                    })?;
                }
                run_worker(
                    WindowsExcelComBackend::new(),
                    &receiver,
                    runner_id,
                    journal_store,
                    watchdog_controller,
                )
            });
            match initialization {
                Ok(_apartment) => Ok(()),
                Err(error) => {
                    if let Some(sender) = ready_sender.take() {
                        // A vanished receiver means startup already returned.
                        sender.send(Err(error.clone())).unwrap_or(());
                    }
                    Err(error)
                }
            }
        })
        .map_err(|error| thread_spawn_error("Excel COM runner", &error))
}

#[cfg(windows)]
fn wait_for_windows_worker_ready(
    watchdog: ExcelProcessWatchdog,
    worker: thread::JoinHandle<PlatformResult<()>>,
    ready_receiver: &mpsc::Receiver<PlatformResult<()>>,
) -> PlatformResult<(ExcelProcessWatchdog, thread::JoinHandle<PlatformResult<()>>)> {
    match ready_receiver.recv_timeout(REQUEST_TIMEOUT) {
        Ok(Ok(())) => Ok((watchdog, worker)),
        Ok(Err(error)) => Err(reap_failed_startup(watchdog, worker, error)),
        Err(error) => {
            let readiness_error = PlatformError::new(
                ErrorCode::Fatal,
                format!("Excel COM runner initialization failed before readiness: {error}"),
            );
            Err(reap_failed_startup(watchdog, worker, readiness_error))
        }
    }
}

#[cfg(windows)]
fn reap_failed_startup(
    mut watchdog: ExcelProcessWatchdog,
    worker: thread::JoinHandle<PlatformResult<()>>,
    primary_error: PlatformError,
) -> PlatformError {
    let reap = reap_failed_startup_worker(worker, WORKER_REAP_TIMEOUT);
    let watchdog_shutdown = watchdog.shutdown(WATCHDOG_RESPONSE_WAIT);
    if reap.is_ok() && watchdog_shutdown.is_ok() {
        primary_error
    } else {
        PlatformError::new(
            ErrorCode::Fatal,
            format!(
                "{primary_error}; readiness cleanup reap={}; watchdog={}",
                result_summary(reap),
                result_summary(watchdog_shutdown)
            ),
        )
    }
}

#[cfg(windows)]
fn reap_failed_startup_worker(
    worker: thread::JoinHandle<PlatformResult<()>>,
    timeout: Duration,
) -> PlatformResult<()> {
    let deadline = Instant::now() + timeout;
    while !worker.is_finished() {
        if Instant::now() >= deadline {
            return quarantine_startup_worker(worker);
        }
        thread::sleep(STARTUP_REAP_POLL_INTERVAL);
    }
    worker
        .join()
        .map_err(|_| PlatformError::new(ErrorCode::Fatal, "Excel startup worker panicked"))?
}

#[cfg(windows)]
fn quarantine_startup_worker(worker: thread::JoinHandle<PlatformResult<()>>) -> PlatformResult<()> {
    QUARANTINED_STARTUP_WORKERS.fetch_add(1, Ordering::AcqRel);
    let spawn = thread::Builder::new()
        .name("excel-startup-reaper".to_owned())
        .spawn(move || {
            match worker.join() {
                Ok(_) | Err(_) => {}
            }
            QUARANTINED_STARTUP_WORKERS.fetch_sub(1, Ordering::AcqRel);
        });
    spawn.map_err(|error| {
        QUARANTINED_STARTUP_WORKERS.fetch_sub(1, Ordering::AcqRel);
        PlatformError::new(
            ErrorCode::Fatal,
            format!(
                "Excel startup worker exceeded its reap deadline and quarantine thread spawn failed: {error}"
            ),
        )
    })?;
    Err(PlatformError::new(
        ErrorCode::Fatal,
        "Excel startup worker exceeded its reap deadline and was moved to bounded quarantine",
    ))
}

#[cfg(windows)]
fn result_summary(result: PlatformResult<()>) -> String {
    match result {
        Ok(()) => "ok".to_owned(),
        Err(error) => error.message().to_owned(),
    }
}

#[cfg(all(test, windows))]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn test_startup_reaper_takes_ownership_after_deadline() {
        let worker = thread::spawn(|| {
            thread::sleep(Duration::from_millis(50));
            Ok(())
        });
        let error = reap_failed_startup_worker(worker, Duration::ZERO)
            .expect_err("worker must enter quarantine");
        assert_eq!(error.code(), ErrorCode::Fatal);
        let deadline = Instant::now() + Duration::from_secs(2);
        while QUARANTINED_STARTUP_WORKERS.load(Ordering::Acquire) != 0 {
            assert!(
                Instant::now() < deadline,
                "quarantined startup worker was not reaped"
            );
            thread::sleep(STARTUP_REAP_POLL_INTERVAL);
        }
    }
}
