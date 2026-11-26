//! Test-only human-present Excel write calibration support.
//!
//! Responsibilities:
//! - add writable workbook open, scalar property put, recalculation, and save to
//!   the dedicated worker without widening the production public surface;
//! - validate every scalar and range request before crossing the worker channel;
//! - keep workbook/session identity checks identical to the read-only surface.
//!
//! Boundaries:
//! - compiled only for crate tests;
//! - never accepts formulas, arrays, arbitrary COM member names, or user workbooks;
//! - does not register a Tool, populate `tools.json`, or create rollback recipes.
//!
//! Invariants:
//! 1. every request belongs to the current runner and workbook session;
//! 2. finite numbers, bounded text, and non-formula text only;
//! 3. COM objects and property-put arguments never leave the worker thread.

use super::{
    ErrorCode, ExcelComBackend, ExcelComRunner, ExcelRangeSelector, ExcelWorkbookHandle,
    OpenWorkbookState, Path, PathBuf, PlatformError, PlatformResult, RunnerCommand, RunnerState,
    RunnerValue, runner_protocol_error,
};

/// Maximum UTF-8 bytes accepted for one scalar text assignment.
const MAX_EXCEL_SCALAR_TEXT_BYTES: usize = 262_144;

/// Closed scalar values accepted by the test-only property-put calibration.
#[derive(Debug, Clone, PartialEq)]
pub enum ExcelScalarValue {
    /// Assign an empty VARIANT, which clears the target range.
    Blank,
    /// Assign a boolean.
    Boolean(bool),
    /// Assign a finite IEEE-754 number.
    Number(f64),
    /// Assign literal text. Formula-looking text is rejected.
    Text(String),
}

impl ExcelScalarValue {
    pub(crate) fn validate(&self) -> PlatformResult<()> {
        match self {
            Self::Number(value) if !value.is_finite() => Err(PlatformError::new(
                ErrorCode::ToolInvalidArgs,
                "Excel scalar number must be finite",
            )),
            Self::Text(value)
                if value.len() > MAX_EXCEL_SCALAR_TEXT_BYTES
                    || value.contains('\0')
                    || looks_like_formula(value) =>
            {
                Err(PlatformError::new(
                    ErrorCode::ToolInvalidArgs,
                    "Excel scalar text is NUL/formula-like or exceeds 256 KiB",
                ))
            }
            Self::Blank | Self::Boolean(_) | Self::Number(_) | Self::Text(_) => Ok(()),
        }
    }
}

/// Test-only request routed through the existing capacity-one worker queue.
#[derive(Debug, Clone, PartialEq)]
pub enum ExcelWriteAcceptanceRequest {
    OpenWritable {
        path: PathBuf,
    },
    WriteScalar {
        runner_id: u64,
        session_id: u64,
        selector: ExcelRangeSelector,
        scalar: ExcelScalarValue,
    },
    Recalculate {
        runner_id: u64,
        session_id: u64,
    },
    Save {
        runner_id: u64,
        session_id: u64,
    },
}

impl ExcelComRunner {
    pub(crate) fn open_writable_for_acceptance(
        &self,
        workbook_path: impl AsRef<Path>,
    ) -> PlatformResult<ExcelWorkbookHandle> {
        let path = crate::excel_process::validate_workbook_path(workbook_path.as_ref())?;
        match self.dispatch(|response| RunnerCommand::WriteAcceptance {
            request: ExcelWriteAcceptanceRequest::OpenWritable { path },
            response,
        })? {
            RunnerValue::WorkbookOpened(workbook) => Ok(workbook),
            _ => Err(runner_protocol_error(
                "Excel write acceptance returned no workbook handle",
            )),
        }
    }

    pub(crate) fn write_scalar_for_acceptance(
        &self,
        workbook: &ExcelWorkbookHandle,
        selector: ExcelRangeSelector,
        scalar: ExcelScalarValue,
    ) -> PlatformResult<()> {
        selector.validate()?;
        scalar.validate()?;
        self.expect_write_acceptance_unit(ExcelWriteAcceptanceRequest::WriteScalar {
            runner_id: workbook.runner_id(),
            session_id: workbook.session_id(),
            selector,
            scalar,
        })
    }

    pub(crate) fn recalculate_for_acceptance(
        &self,
        workbook: &ExcelWorkbookHandle,
    ) -> PlatformResult<()> {
        self.expect_write_acceptance_unit(ExcelWriteAcceptanceRequest::Recalculate {
            runner_id: workbook.runner_id(),
            session_id: workbook.session_id(),
        })
    }

    pub(crate) fn save_workbook_for_acceptance(
        &self,
        workbook: &ExcelWorkbookHandle,
    ) -> PlatformResult<()> {
        self.expect_write_acceptance_unit(ExcelWriteAcceptanceRequest::Save {
            runner_id: workbook.runner_id(),
            session_id: workbook.session_id(),
        })
    }

    fn expect_write_acceptance_unit(
        &self,
        request: ExcelWriteAcceptanceRequest,
    ) -> PlatformResult<()> {
        match self.dispatch(|response| RunnerCommand::WriteAcceptance { request, response })? {
            RunnerValue::Unit => Ok(()),
            _ => Err(runner_protocol_error(
                "Excel write acceptance returned an unexpected response",
            )),
        }
    }
}

impl RunnerState {
    pub(super) fn execute_write_acceptance<B>(
        &mut self,
        backend: &mut B,
        request: &ExcelWriteAcceptanceRequest,
    ) -> super::RunnerResponse
    where
        B: ExcelComBackend,
    {
        validate_backend_request(self, request)?;
        if let ExcelWriteAcceptanceRequest::OpenWritable { path } = request {
            backend.execute_write_acceptance(request)?;
            return self.open_writable_handle(path);
        }
        let (runner_id, session_id) = request_identity(request)?;
        self.workbook_for(runner_id, session_id)?;
        backend.execute_write_acceptance(request)?;
        Ok(RunnerValue::Unit)
    }

    fn open_writable_handle(&mut self, path: &Path) -> super::RunnerResponse {
        let session_id = self
            .next_session_id
            .checked_add(1)
            .ok_or_else(|| runner_protocol_error("Excel workbook session id overflowed"))?;
        self.next_session_id = session_id;
        let owned_process = self.owned_process.clone().ok_or_else(|| {
            runner_protocol_error("Excel COM runner has no owned process identity")
        })?;
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
}

fn validate_backend_request(
    state: &RunnerState,
    request: &ExcelWriteAcceptanceRequest,
) -> PlatformResult<()> {
    if !state.created || state.closed {
        return Err(runner_protocol_error(
            "Excel COM application must be active before write acceptance",
        ));
    }
    match request {
        ExcelWriteAcceptanceRequest::OpenWritable { .. } if state.workbook.is_some() => Err(
            runner_protocol_error("Excel COM runner already owns an open workbook"),
        ),
        ExcelWriteAcceptanceRequest::OpenWritable { .. }
        | ExcelWriteAcceptanceRequest::Recalculate { .. }
        | ExcelWriteAcceptanceRequest::Save { .. } => Ok(()),
        ExcelWriteAcceptanceRequest::WriteScalar {
            selector, scalar, ..
        } => {
            selector.validate()?;
            scalar.validate()
        }
    }
}

fn request_identity(request: &ExcelWriteAcceptanceRequest) -> PlatformResult<(u64, u64)> {
    match request {
        ExcelWriteAcceptanceRequest::OpenWritable { .. } => Err(runner_protocol_error(
            "Excel writable open request has no workbook session",
        )),
        ExcelWriteAcceptanceRequest::WriteScalar {
            runner_id,
            session_id,
            ..
        }
        | ExcelWriteAcceptanceRequest::Recalculate {
            runner_id,
            session_id,
        }
        | ExcelWriteAcceptanceRequest::Save {
            runner_id,
            session_id,
        } => Ok((*runner_id, *session_id)),
    }
}

fn looks_like_formula(value: &str) -> bool {
    value
        .chars()
        .find(|character| {
            !is_default_ignorable(*character)
                && !character.is_whitespace()
                && !character.is_control()
        })
        .is_some_and(|character| matches!(character, '=' | '+' | '-' | '@'))
}

fn is_default_ignorable(character: char) -> bool {
    matches!(
        character,
        '\u{00AD}'
            | '\u{034F}'
            | '\u{061C}'
            | '\u{115F}'..='\u{1160}'
            | '\u{17B4}'..='\u{17B5}'
            | '\u{180B}'..='\u{180F}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{206F}'
            | '\u{3164}'
            | '\u{FE00}'..='\u{FE0F}'
            | '\u{FEFF}'
            | '\u{FFA0}'
            | '\u{FFF0}'..='\u{FFF8}'
            | '\u{1BCA0}'..='\u{1BCA3}'
            | '\u{1D173}'..='\u{1D17A}'
            | '\u{E0000}'..='\u{E0FFF}'
    )
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_scalar_validate_rejects_non_finite_number_and_formula_text() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(
                ExcelScalarValue::Number(value)
                    .validate()
                    .expect_err("non-finite number")
                    .code(),
                ErrorCode::ToolInvalidArgs
            );
        }
        assert_eq!(
            ExcelScalarValue::Text("\u{200B}=A1".to_owned())
                .validate()
                .expect_err("formula-like text")
                .code(),
            ErrorCode::ToolInvalidArgs
        );
        for value in ["+A1", "-1", "@SUM(A1:A2)", "  =A1"] {
            assert_eq!(
                ExcelScalarValue::Text(value.to_owned())
                    .validate()
                    .expect_err("formula-like text prefix")
                    .code(),
                ErrorCode::ToolInvalidArgs
            );
        }
        assert_eq!(
            ExcelScalarValue::Text("left\0right".to_owned())
                .validate()
                .expect_err("NUL text")
                .code(),
            ErrorCode::ToolInvalidArgs
        );
        assert_eq!(
            ExcelScalarValue::Text("a".repeat(MAX_EXCEL_SCALAR_TEXT_BYTES + 1))
                .validate()
                .expect_err("oversized text")
                .code(),
            ErrorCode::ToolInvalidArgs
        );
    }

    #[test]
    fn test_scalar_validate_accepts_closed_values_and_bounded_text() {
        for value in [
            ExcelScalarValue::Blank,
            ExcelScalarValue::Boolean(true),
            ExcelScalarValue::Number(-1.25),
            ExcelScalarValue::Text("TASK-407".to_owned()),
            ExcelScalarValue::Text("a".repeat(MAX_EXCEL_SCALAR_TEXT_BYTES)),
        ] {
            value.validate().expect("valid scalar");
        }
    }
}
