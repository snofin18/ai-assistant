//! TASK-401 compile boundary for the Excel COM runner.
//!
//! This test proves the app layer can consume the Windows platform handle
//! without registering a model-visible Tool or coupling domain types into
//! `assistant-platform-api`.

#[cfg(windows)]
use assistant_platform_api::PlatformResult;
#[cfg(windows)]
use assistant_platform_windows::excel_com_runner::ExcelComRunner;

#[cfg(windows)]
trait IsSendAndSync: Send + Sync {}

#[cfg(windows)]
impl IsSendAndSync for ExcelComRunner {}

#[cfg(windows)]
const fn assert_send_and_sync<T: IsSendAndSync>() {}

#[cfg(windows)]
#[test]
fn test_agent_core_can_consume_excel_com_runner_boundary() {
    let start: fn() -> PlatformResult<ExcelComRunner> = ExcelComRunner::start;
    assert!(std::mem::size_of_val(&start) > 0);
    assert_send_and_sync::<ExcelComRunner>();
}
