//! Contract tests for the pure capture pipeline and bounded scroll plan.

use std::error::Error;
use std::future::{Future, ready};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::task::{Context, Poll, Waker};

use assistant_capture::{
    CaptureError, CapturePipeline, MAX_SCROLL_STEPS, PrivacyMode, RedactionPolicy,
    ScrollCleanupError, ScrollCleanupPlan, ScrollMergeDirection,
};
use assistant_dlp::{ImageDimensions, RedactError, RedactionRegion, RedactionRule};
use assistant_platform_api::{
    CaptureOptions, ErrorCode, FocusPolicy, ImageRef, LocalHandleId, PlatformError, PlatformResult,
    ResolvedWindow, TargetDescriptor, WindowFilter, WindowInfo, WindowProvider, WindowState,
};

fn block_on<F: Future>(future: F) -> F::Output {
    let mut context = Context::from_waker(Waker::noop());
    let mut future = Box::pin(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

struct FakeWindowProvider {
    window: ResolvedWindow,
    result: PlatformResult<ImageRef>,
    capture_calls: AtomicUsize,
    last_redact: AtomicBool,
}

impl FakeWindowProvider {
    fn new(result: PlatformResult<ImageRef>) -> Self {
        Self {
            window: ResolvedWindow::new(LocalHandleId::new(7), "fixture-window".to_owned()),
            result,
            capture_calls: AtomicUsize::new(0),
            last_redact: AtomicBool::new(false),
        }
    }

    fn success() -> Self {
        Self::new(Ok(ImageRef::new("blob-1".to_owned(), 100, 80)))
    }

    const fn window(&self) -> &ResolvedWindow {
        &self.window
    }

    fn capture_calls(&self) -> usize {
        self.capture_calls.load(Ordering::SeqCst)
    }

    fn last_redact(&self) -> bool {
        self.last_redact.load(Ordering::SeqCst)
    }
}

impl WindowProvider for FakeWindowProvider {
    fn list_windows(
        &self,
        _filter: &WindowFilter,
    ) -> impl Future<Output = PlatformResult<Vec<WindowInfo>>> + Send {
        ready(Ok(Vec::new()))
    }

    fn resolve_window(
        &self,
        _descriptor: &TargetDescriptor,
    ) -> impl Future<Output = PlatformResult<ResolvedWindow>> + Send {
        ready(Ok(self.window.clone()))
    }

    fn window_state(
        &self,
        _window: &ResolvedWindow,
    ) -> impl Future<Output = PlatformResult<WindowState>> + Send {
        ready(Ok(WindowState::new(false, true, false)))
    }

    fn bring_to_front(
        &self,
        _window: &ResolvedWindow,
        _policy: FocusPolicy,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        ready(Ok(()))
    }

    fn capture(
        &self,
        _window: &ResolvedWindow,
        options: &CaptureOptions,
    ) -> impl Future<Output = PlatformResult<ImageRef>> + Send {
        self.capture_calls.fetch_add(1, Ordering::SeqCst);
        self.last_redact.store(options.redact(), Ordering::SeqCst);
        ready(self.result.clone())
    }
}

#[test]
fn test_persist_blob_required_redaction_returns_occlusions() -> Result<(), Box<dyn Error>> {
    let provider = FakeWindowProvider::success();
    let pipeline = CapturePipeline::new(PrivacyMode::PersistBlob);
    let bounds = ImageDimensions::new(100, 80)?;
    let rules = [RedactionRule::password_field(RedactionRegion::new(
        4, 5, 20, 10,
    )?)];
    let outcome = block_on(pipeline.capture(
        &provider,
        provider.window(),
        bounds,
        RedactionPolicy::Required(&rules),
    ))?;

    assert_eq!(provider.capture_calls(), 1);
    assert!(provider.last_redact());
    assert_eq!(
        outcome.retained_image().map(ImageRef::blob_id),
        Some("blob-1")
    );
    assert_eq!(outcome.image_dimensions(), bounds);
    assert_eq!(outcome.occlusions().len(), 1);
    assert_eq!(outcome.occlusions().first().map(|rect| rect.x()), Some(4));
    Ok(())
}

#[test]
fn test_never_persist_drops_blob_reference_but_keeps_dimensions() -> Result<(), Box<dyn Error>> {
    let provider = FakeWindowProvider::success();
    let pipeline = CapturePipeline::new(PrivacyMode::NeverPersist);
    let bounds = ImageDimensions::new(100, 80)?;
    let outcome = block_on(pipeline.capture(
        &provider,
        provider.window(),
        bounds,
        RedactionPolicy::Disabled,
    ))?;

    assert_eq!(provider.capture_calls(), 1);
    assert!(!provider.last_redact());
    assert!(outcome.retained_image().is_none());
    assert_eq!(outcome.image_dimensions(), bounds);
    assert!(outcome.occlusions().is_empty());
    Ok(())
}

#[test]
fn test_required_redaction_empty_rules_fails_before_provider_call() -> Result<(), Box<dyn Error>> {
    let provider = FakeWindowProvider::success();
    let pipeline = CapturePipeline::new(PrivacyMode::PersistBlob);
    let result = block_on(pipeline.capture(
        &provider,
        provider.window(),
        ImageDimensions::new(100, 80)?,
        RedactionPolicy::Required(&[]),
    ));

    assert!(matches!(
        result,
        Err(CaptureError::Redaction(RedactError::NoRules))
    ));
    assert_eq!(provider.capture_calls(), 0);
    Ok(())
}

#[test]
fn test_disabled_redaction_keeps_persist_blob_behavior() -> Result<(), Box<dyn Error>> {
    let provider = FakeWindowProvider::success();
    let pipeline = CapturePipeline::new(PrivacyMode::PersistBlob);
    let outcome = block_on(pipeline.capture(
        &provider,
        provider.window(),
        ImageDimensions::new(100, 80)?,
        RedactionPolicy::Disabled,
    ))?;

    assert!(!provider.last_redact());
    assert!(outcome.retained_image().is_some());
    assert!(outcome.occlusions().is_empty());
    Ok(())
}

#[test]
fn test_platform_error_code_is_preserved() -> Result<(), Box<dyn Error>> {
    let provider = FakeWindowProvider::new(Err(PlatformError::new(
        ErrorCode::CapabilityMissing,
        "capture unavailable",
    )));
    let pipeline = CapturePipeline::new(PrivacyMode::NeverPersist);
    let result = block_on(pipeline.capture(
        &provider,
        provider.window(),
        ImageDimensions::new(100, 80)?,
        RedactionPolicy::Disabled,
    ));

    match result {
        Err(CaptureError::Platform(error)) => {
            assert_eq!(error.code(), ErrorCode::CapabilityMissing);
        }
        Err(other) => return Err(format!("unexpected error: {other}").into()),
        Ok(_) => return Err("expected platform failure".into()),
    }
    Ok(())
}

#[test]
fn test_zero_dimension_provider_result_fails_explicitly() -> Result<(), Box<dyn Error>> {
    let provider = FakeWindowProvider::new(Ok(ImageRef::new("blob-zero".to_owned(), 0, 80)));
    let pipeline = CapturePipeline::new(PrivacyMode::NeverPersist);
    let result = block_on(pipeline.capture(
        &provider,
        provider.window(),
        ImageDimensions::new(100, 80)?,
        RedactionPolicy::Disabled,
    ));

    assert!(matches!(
        result,
        Err(CaptureError::Redaction(RedactError::ZeroDimension))
    ));
    Ok(())
}

#[test]
fn test_scroll_cleanup_plan_is_bounded_and_directional() -> Result<(), Box<dyn Error>> {
    let plan = ScrollCleanupPlan::new(ScrollMergeDirection::TopToBottom, MAX_SCROLL_STEPS, 12)?;
    assert_eq!(plan.direction(), ScrollMergeDirection::TopToBottom);
    assert_eq!(plan.steps(), MAX_SCROLL_STEPS);
    assert_eq!(plan.overlap_pixels(), 12);

    assert!(matches!(
        ScrollCleanupPlan::new(ScrollMergeDirection::BottomToTop, 0, 1),
        Err(ScrollCleanupError::ZeroSteps)
    ));
    assert!(matches!(
        ScrollCleanupPlan::new(ScrollMergeDirection::BottomToTop, MAX_SCROLL_STEPS + 1, 1),
        Err(ScrollCleanupError::StepLimitExceeded)
    ));
    Ok(())
}
