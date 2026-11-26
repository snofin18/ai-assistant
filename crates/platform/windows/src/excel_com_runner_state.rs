//! Mutable Excel runner state and command semantics.
//!
//! Responsibilities:
//! - validate workbook/session identity before backend access;
//! - publish lifecycle journals before open/close/quit operations;
//! - retain cleanup evidence on failure and delete it only after success.
//!
//! Boundaries:
//! - no COM object is stored here;
//! - no Tool, process termination, or watchdog behavior is exposed.

use super::excel_ownership_journal::{
    ExcelOwnershipJournalPhase, ExcelOwnershipJournalPublication, ExcelOwnershipJournalStore,
    ExcelOwnershipJournals,
};
use super::process_watchdog::{
    ExcelProcessWatchdogController, WATCHDOG_GRACEFUL_WAIT, WATCHDOG_RESPONSE_WAIT,
    WATCHDOG_TERMINATION_WAIT,
};
use super::{
    ErrorCode, ExcelComBackend, ExcelOwnedProcessIdentity, ExcelRangeSelector, ExcelReadMode,
    ExcelWorkbookHandle, Path, PathBuf, PlatformError, PlatformResult, RunnerResponse, RunnerValue,
    runner_protocol_error,
};

pub(super) struct RunnerState {
    pub(super) runner_id: u64,
    journal_store: ExcelOwnershipJournalStore,
    journals: ExcelOwnershipJournals,
    watchdog: ExcelProcessWatchdogController,
    pub(super) created: bool,
    pub(super) closed: bool,
    pub(super) owned_process: Option<ExcelOwnedProcessIdentity>,
    pub(super) next_session_id: u64,
    pub(super) workbook: Option<OpenWorkbookState>,
}

pub(super) struct OpenWorkbookState {
    pub(super) session_id: u64,
    pub(super) canonical_path: PathBuf,
}

impl RunnerState {
    pub(super) fn new(
        runner_id: u64,
        journal_store: ExcelOwnershipJournalStore,
        watchdog: ExcelProcessWatchdogController,
    ) -> Self {
        Self {
            runner_id,
            journal_store,
            journals: ExcelOwnershipJournals::default(),
            watchdog,
            created: false,
            closed: false,
            owned_process: None,
            next_session_id: 0,
            workbook: None,
        }
    }

    pub(super) fn create_application<B>(&mut self, backend: &mut B) -> RunnerResponse
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
        let identity = match backend.application_identity() {
            Ok(identity) => identity,
            Err(error) => {
                let cleanup = backend.quit_application();
                return match cleanup {
                    Ok(()) => Err(error),
                    Err(cleanup_error) => Err(PlatformError::new(
                        ErrorCode::Fatal,
                        format!(
                            "Excel process identity failed: {error}; cleanup also failed: {cleanup_error}"
                        ),
                    )),
                };
            }
        };
        self.created = true;
        self.owned_process = Some(identity.clone());
        if let Err(watchdog_error) = self.watchdog.arm(&identity, WATCHDOG_RESPONSE_WAIT) {
            let journal_result = self.publish_journal(
                ExcelOwnershipJournalPhase::OwnershipCaptured,
                0,
                None,
                &identity,
            );
            let mut message = format!("Excel process watchdog arm failed: {watchdog_error}");
            if let Err(error) = journal_result {
                message = format!("{message}; ownership journal failed: {error}");
            }
            return Err(PlatformError::new(ErrorCode::Fatal, message));
        }
        self.publish_journal(
            ExcelOwnershipJournalPhase::OwnershipCaptured,
            0,
            None,
            &identity,
        )?;
        Ok(RunnerValue::Unit)
    }

    pub(super) fn application_process_id(&self) -> RunnerResponse {
        if !self.created {
            return Err(runner_protocol_error(
                "Excel COM application must be created before reading its process id",
            ));
        }
        self.owned_process
            .as_ref()
            .map(|identity| RunnerValue::ProcessId(identity.process_id()))
            .ok_or_else(|| runner_protocol_error("Excel COM runner has no owned process identity"))
    }

    pub(super) fn quit_application<B>(&mut self, backend: &mut B) -> RunnerResponse
    where
        B: ExcelComBackend,
    {
        if !self.created {
            return Err(runner_protocol_error(
                "Excel COM application must be created before quit",
            ));
        }
        let identity = self.owned_process.clone().ok_or_else(|| {
            runner_protocol_error("Excel COM runner has no owned process identity")
        })?;
        self.watchdog
            .capture_verified_tree(&identity, WATCHDOG_RESPONSE_WAIT)?;
        backend.quit_application()?;
        self.watchdog.converge_and_disarm(
            &identity,
            WATCHDOG_GRACEFUL_WAIT,
            WATCHDOG_TERMINATION_WAIT,
            WATCHDOG_RESPONSE_WAIT,
        )?;
        self.created = false;
        self.owned_process = None;
        self.closed = true;
        Ok(RunnerValue::Unit)
    }

    pub(super) fn open_read_only<B>(&mut self, backend: &mut B, path: &Path) -> RunnerResponse
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
        let session_id = self
            .next_session_id
            .checked_add(1)
            .ok_or_else(|| runner_protocol_error("Excel workbook session id overflowed"))?;
        self.next_session_id = session_id;
        let owned_process = self.owned_process.clone().ok_or_else(|| {
            runner_protocol_error("Excel COM runner has no owned process identity")
        })?;
        self.publish_journal(
            ExcelOwnershipJournalPhase::Opening,
            session_id,
            Some(path),
            &owned_process,
        )?;
        backend.open_read_only(path)?;
        let workbook = ExcelWorkbookHandle {
            runner_id: self.runner_id,
            session_id,
            canonical_path: path.to_path_buf(),
            owned_process,
        };
        self.workbook = Some(OpenWorkbookState {
            session_id,
            canonical_path: path.to_path_buf(),
        });
        Ok(RunnerValue::WorkbookOpened(workbook))
    }

    pub(super) fn read_active_sheet_text<B>(
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

    pub(super) fn read_workbook<B>(
        &self,
        backend: &mut B,
        runner_id: u64,
        session_id: u64,
    ) -> RunnerResponse
    where
        B: ExcelComBackend,
    {
        let workbook = self.workbook_for(runner_id, session_id)?;
        backend
            .read_workbook(&workbook.canonical_path)
            .map(RunnerValue::WorkbookSnapshot)
    }

    pub(super) fn read_range<B>(
        &self,
        backend: &mut B,
        runner_id: u64,
        session_id: u64,
        selector: &ExcelRangeSelector,
        mode: ExcelReadMode,
    ) -> RunnerResponse
    where
        B: ExcelComBackend,
    {
        self.workbook_for(runner_id, session_id)?;
        backend
            .read_range(selector, mode)
            .map(RunnerValue::RangeSnapshot)
    }

    pub(super) fn verify_ownership<B>(
        &self,
        backend: &B,
        runner_id: u64,
        session_id: u64,
    ) -> RunnerResponse
    where
        B: ExcelComBackend,
    {
        self.workbook_for(runner_id, session_id)?;
        let identity = self.owned_process.as_ref().ok_or_else(|| {
            runner_protocol_error("Excel COM runner has no owned process identity")
        })?;
        backend.verify_ownership(identity)?;
        Ok(RunnerValue::Unit)
    }

    pub(super) fn workbook_for(
        &self,
        runner_id: u64,
        session_id: u64,
    ) -> PlatformResult<&OpenWorkbookState> {
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
        Ok(workbook)
    }

    pub(super) fn close_workbook<B>(
        &mut self,
        backend: &mut B,
        runner_id: u64,
        session_id: u64,
    ) -> RunnerResponse
    where
        B: ExcelComBackend,
    {
        let (canonical_path, identity) = {
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
            let identity = self.owned_process.clone().ok_or_else(|| {
                runner_protocol_error("Excel COM runner has no owned process identity")
            })?;
            (workbook.canonical_path.clone(), identity)
        };
        self.publish_journal(
            ExcelOwnershipJournalPhase::Closing,
            session_id,
            Some(&canonical_path),
            &identity,
        )?;
        backend.close_workbook()?;
        self.workbook = None;
        self.journals.complete_session_closure(session_id)?;
        Ok(RunnerValue::Unit)
    }

    pub(super) fn shutdown<B>(&mut self, backend: &mut B) -> RunnerResponse
    where
        B: ExcelComBackend,
    {
        if let Some(identity) = self.owned_process.clone() {
            let session_id = self
                .workbook
                .as_ref()
                .map_or(0, |workbook| workbook.session_id);
            let workbook_path = self
                .workbook
                .as_ref()
                .map(|workbook| workbook.canonical_path.clone());
            self.publish_journal(
                ExcelOwnershipJournalPhase::Cleanup,
                session_id,
                workbook_path.as_deref(),
                &identity,
            )?;
        }
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
        let cleanup_result = match (close_result, quit_result) {
            (Ok(_), Ok(_)) => Ok(()),
            (Err(close_error), Ok(_)) => Err(cleanup_error(
                "Excel workbook close failed during shutdown",
                close_error,
            )),
            (Ok(_), Err(quit_error)) => Err(cleanup_error(
                "Excel application quit failed during shutdown",
                quit_error,
            )),
            (Err(close_error), Err(quit_error)) => {
                let code = if close_error.code() == ErrorCode::UserInteraction
                    && quit_error.code() == ErrorCode::UserInteraction
                {
                    ErrorCode::UserInteraction
                } else {
                    ErrorCode::Fatal
                };
                Err(PlatformError::new(
                    code,
                    format!(
                        "Excel cleanup failed: workbook close={close_error}; application quit={quit_error}"
                    ),
                ))
            }
        };
        cleanup_result?;
        self.journals.complete()?;
        Ok(RunnerValue::Unit)
    }

    pub(super) fn abandon<B>(&mut self, backend: &mut B) -> PlatformResult<()>
    where
        B: ExcelComBackend,
    {
        if !self.created && self.workbook.is_none() {
            return self.complete_journals_if_idle();
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

    fn publish_journal(
        &mut self,
        phase: ExcelOwnershipJournalPhase,
        session_id: u64,
        workbook_path: Option<&Path>,
        identity: &ExcelOwnedProcessIdentity,
    ) -> PlatformResult<()> {
        self.journals.require_identity(identity)?;
        self.journals.publish(
            &self.journal_store,
            ExcelOwnershipJournalPublication {
                phase,
                runner_id: self.runner_id,
                session_id,
                workbook_path,
                identity,
            },
        )
    }

    fn complete_journals_if_idle(&mut self) -> PlatformResult<()> {
        if self.created || self.workbook.is_some() {
            Ok(())
        } else {
            self.journals.complete()
        }
    }
}

fn cleanup_error(context: &str, error: PlatformError) -> PlatformError {
    if error.code() == ErrorCode::UserInteraction {
        error
    } else {
        PlatformError::new(ErrorCode::Fatal, format!("{context}: {error}"))
    }
}
