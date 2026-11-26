//! Response and terminal-command helpers for the Excel COM worker loop.

use super::{ExcelComBackend, PlatformResult, ResponseSender, RunnerResponse, RunnerState};

pub(super) fn send_response(sender: &ResponseSender, response: RunnerResponse) -> Result<(), ()> {
    sender.send(response).map_err(|_| ())
}

pub(super) fn shutdown_result(result: RunnerResponse) -> PlatformResult<()> {
    result.map(|_| ())
}

pub(super) fn shutdown_command<B>(
    state: &mut RunnerState,
    backend: &mut B,
    response: ResponseSender,
) -> (ResponseSender, RunnerResponse, bool)
where
    B: ExcelComBackend,
{
    (response, state.shutdown(backend), true)
}

#[cfg(test)]
pub(super) fn write_acceptance_command<B>(
    state: &mut RunnerState,
    backend: &mut B,
    response: ResponseSender,
    request: &super::ExcelWriteAcceptanceRequest,
) -> (ResponseSender, RunnerResponse, bool)
where
    B: ExcelComBackend,
{
    (
        response,
        state.execute_write_acceptance(backend, request),
        false,
    )
}
