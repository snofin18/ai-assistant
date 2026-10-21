//! TASK-048 的 CDP provider 契约回归（`tests/` 层）。

use assistant_platform_api::{CdpConnectOptions, CdpPoint, CdpProvider, PlatformError};
use assistant_platform_windows::cdp::runtime::RuntimeCdpProvider;

#[derive(Debug)]
struct TestError(PlatformError);

impl From<PlatformError> for TestError {
    fn from(failure: PlatformError) -> Self {
        Self(failure)
    }
}

impl std::fmt::Display for TestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    use std::task::{Context, Poll, Waker};
    let mut future = Box::pin(future);
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

fn connect_session(
    provider: &RuntimeCdpProvider,
    options: &CdpConnectOptions,
) -> Result<assistant_platform_api::CdpSessionSnapshot, TestError> {
    Ok(block_on(provider.connect(options))?)
}

#[test]
fn test_cdp_provider_connect_keeps_target_identity() -> Result<(), TestError> {
    let options = CdpConnectOptions::new(
        "127.0.0.1",
        9222,
        "spike-g://fixture",
        Some("fixture".to_string()),
        Some("spike-g".to_string()),
        200,
    )?;
    let provider = RuntimeCdpProvider::new();
    let session = connect_session(&provider, &options)?;
    assert_eq!(session.target_identity().marker(), Some("spike-g"));
    Ok(())
}

#[test]
fn test_cdp_provider_click_and_close_are_success_shape() -> Result<(), TestError> {
    let options = CdpConnectOptions::new(
        "127.0.0.1",
        9222,
        "spike-g://fixture",
        Some("fixture".to_string()),
        None,
        200,
    )?;
    let provider = RuntimeCdpProvider::new();
    let session = connect_session(&provider, &options)?;
    let point = CdpPoint::new(10, 10);
    let result = block_on(provider.click(&session.target(), &point));
    assert!(result.is_ok(), "click must succeed");
    let result = block_on(provider.close(&session.target()));
    assert!(result.is_ok(), "close must succeed");
    Ok(())
}
