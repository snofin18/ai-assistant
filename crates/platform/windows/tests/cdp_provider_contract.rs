//! CDP provider 契约回归（`tests/` 层）。

use assistant_platform_api::{
    CdpConnectOptions, CdpLoadCondition, CdpPoint, CdpProvider, ErrorCode, PlatformError,
    ResolvedCdpSession, ResolvedCdpTarget,
};
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

fn assert_capability_missing<T>(result: Result<T, PlatformError>) -> Result<(), TestError> {
    match result {
        Ok(_) => Err(TestError(PlatformError::new(
            ErrorCode::Fatal,
            "unimplemented CDP operation returned success instead of CapabilityMissing",
        ))),
        Err(error) => {
            assert_eq!(error.code(), ErrorCode::CapabilityMissing);
            assert!(!error.message().trim().is_empty());
            Ok(())
        }
    }
}

#[test]
fn test_cdp_provider_connect_fails_closed_without_real_transport() -> Result<(), TestError> {
    let options = CdpConnectOptions::new(
        "127.0.0.1",
        9222,
        "spike-g://fixture",
        Some("fixture".to_string()),
        Some("spike-g".to_string()),
        200,
    )?;
    let provider = RuntimeCdpProvider::new();
    assert_capability_missing(block_on(provider.connect(&options)))?;
    assert!(
        provider.targets().is_empty(),
        "failed connect must not register a synthetic target"
    );
    Ok(())
}

#[test]
fn test_cdp_provider_primitives_fail_closed_without_real_transport() -> Result<(), TestError> {
    let provider = RuntimeCdpProvider::new();
    let target = ResolvedCdpTarget::new(ResolvedCdpSession::new(7), 11);
    let condition = CdpLoadCondition::NetworkIdle { quiet_ms: 500 };
    let point = CdpPoint::new(10, 10);
    assert_capability_missing(block_on(provider.navigate(
        &target,
        "https://example.test/",
        &condition,
    )))?;
    assert_capability_missing(block_on(
        provider.evaluate_dom(&target, "document.documentElement.outerHTML"),
    ))?;
    assert_capability_missing(block_on(provider.click(&target, &point)))?;
    assert_capability_missing(block_on(provider.capture_screenshot(&target)))?;
    assert_capability_missing(block_on(
        provider.set_download_directory(&target, r"C:\assistant\downloads"),
    ))?;
    assert_capability_missing(block_on(provider.close(&target)))?;
    Ok(())
}
